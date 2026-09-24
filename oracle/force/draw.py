"""The draw: the frame the formation is rolled, and the patch list that forces it.

`UpdateRNGSeed2` (`ps4.asm:86097`) is the one input the tool can patch to
choose *which* of a group's 32 entries is drawn:

```
    move.w  $8(a5), d0             ; the VDP HV counter
    add.w   (Main_Frame_Count).w, d0
    sub.w   (RNG_Seed).w, d0       ; d0 = the roll:  (hv + frame_count - seed_high)
    ror     (RNG_Seed).w
```

With `K = (hv + Main_Frame_Count) & 31`, the entry is `(K - seed_high) & 31`, so
patching `RNG_Seed`'s high word (`$FFFFEF0C`, two bytes) to `K - entry` puts the
draw on any entry. `K` is not computed: the probe run measures it, and the draw
is the first call at or after the battle's first frame (`Battle_SetupEnemyData`
draws the formation before it builds anything, `ps4.asm:11858-11862`).

The patch lands one frame *before* that call, because `oracle/rng_trace.py
check` insists a frame's first call start from the seed the log holds for the
frame before it; `phases.probe_phase` verifies from the probe's own log that
nothing advances the seed during the load before anything is written.
"""
from __future__ import annotations

import dataclasses

from .errors import ForceError
from .runs import by_frame
from .selectors import Selector

#: The roll is masked to five bits (`andi.w #$1F`), so groups hold 32 entries.
GROUP_ENTRIES = 32


@dataclasses.dataclass
class Draw:
    frame: int
    hv: int
    frame_count: int
    seed_before: int
    roll: int
    index: int
    k: int          # (hv + frame_count) & 31
    rows_in_frame: int

    @property
    def frame_before(self) -> int:
        return self.frame - 1

    def seed_for(self, index: int) -> int:
        """The `RNG_Seed` high word that puts the draw on `index`."""
        return (self.k - index) & (GROUP_ENTRIES - 1)


def find_draw(trace_rows: list[dict], log_rows: list[dict],
              not_before: int) -> Draw:
    """The draw = the first UpdateRNGSeed2 call at/after the battle's start."""
    calls = [row for row in trace_rows if int(row["frame"]) >= not_before]
    if not calls:
        raise ForceError(
            "the probe's trace holds no UpdateRNGSeed2 call at or after the "
            "battle's first frame: the battle never drew a formation")
    row = calls[0]
    frame = int(row["frame"])
    before = by_frame(log_rows).get(frame - 1)
    if before is None:
        raise ForceError(f"the probe log has no frame {frame - 1}")
    hv = int(row["hv"], 16)
    count = int(row["frame_count"])
    roll = int(row["roll"], 16)
    return Draw(frame=frame, hv=hv, frame_count=count,
                seed_before=int(row["seed_before"], 16), roll=roll,
                index=roll & (GROUP_ENTRIES - 1),
                k=(hv + count) & (GROUP_ENTRIES - 1),
                rows_in_frame=sum(1 for r in calls if int(r["frame"]) == frame))


def patch_specs(selector: Selector, facts: dict, layout: dict[str, dict],
                draw: Draw | None = None, index: int | None = None) -> list[str]:
    """The `--ram-patch` list: selector cells, seed word, then the restores."""
    def spec(frame: int, name: str, value: int) -> str:
        field = layout.get(name)
        if field is None:
            raise ForceError(f"oracle/ram_map.json has no field {name}: the "
                             "selector needs it")
        size = int(field["size"])
        if not 0 <= value < 1 << (size * 8):
            raise ForceError(f"{name} = {value} does not fit {size} byte(s)")
        return f"{frame}:{field['addr']}:{value:0{size * 2}X}"

    specs = [spec(facts["battle_first"] + 1, name, value)
             for name, value in selector.cells]
    if draw is not None and index is not None:
        specs.append(spec(draw.frame_before, "rng_hi", draw.seed_for(index)))
        specs += [spec(draw.frame + 1, name, facts["cells"][name])
                  for name in selector.restore]
    return specs
