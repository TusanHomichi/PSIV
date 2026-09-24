"""Force a chosen formation into a battle and capture it with the oracle.

    oracle/force_battle.py --formation 0x5E --out build/forced/helex

The oracle can replay a battle exactly, but only battles an existing tape
happens to reach. This makes any formation reachable on demand: take a base tape
that walks into an encounter, force the formation that encounter will use, drive
the fight with a scripted input policy, and write the RNG trace and RAM log the
replay machinery consumes. `docs/BATTLE_ORACLE_FORCED.md` is the ledger: the
mechanism with its citations, the captures and their limits.

`Battle_SetupEnemyData` (`ps4.asm:11813`) picks the formation in one pass inside
one frame - no frame boundary separates the choice from the enemy build - so
only its two inputs can be patched:

* **which group**: `Battle_EnemyFormationIndexes[Field_Map_Index]`, the position
  grid's cell for a world map, or the vehicle tables (`selectors`);
* **which of the group's 32 entries**: `UpdateRNGSeed2`'s roll
  (`ps4.asm:86097`), `(hv_counter + Main_Frame_Count - RNG_Seed_high) & $1F`.

The group is forced with a one-shot `--ram-patch` one frame after the encounter
fires, and written back one frame after the formation draw (the vehicle index
excepted: the battle reads it every frame). The entry is forced by patching
`RNG_Seed`'s high word to `K - entry`, where `K` is the roll's unpatched
`hv + frame_count` sum, measured by a probe run - the draw is the first
`UpdateRNGSeed2` call of its frame. That patch lands one frame *before* the
draw, because `oracle/rng_trace.py check` insists a frame's first call start
from the seed the log holds for the frame before it.

`attack` (default) is tape 07's own fight input - one `C` press every 16 frames,
which takes COMD -> ATTACK -> the default target per member - and `defend` is
tape 14's horizontal menu walk to DEFEND. `--delay N` pads N idle frames at the
seam, after the encounter has fired and before the policy: the formation is
already fixed, but every roll in the fight shifts, which is the knob for a
battle whose enemy never rolls the ability the capture is for.

`--vehicle N` chooses which `VehicleData` record the forced battle loads, for a
formation that sits in one of the four vehicle tables; `selectors` carries the
table itself, and the module docstrings below the citations each phase rests on.

The package, by layer:

* `errors` - the one exception a refused request raises;
* `tape` - the tape format and the composed input policies;
* `runs` - running `psiv_oracle`, and reading its RAM log and RNG trace;
* `pack` - `generated/` and `oracle/ram_map.json`'s fields;
* `selectors` - how a group is forced, and which vehicle fights it;
* `scout` - the base tape's own run: where its battle starts;
* `draw` - the formation draw and the `--ram-patch` list;
* `capture` - what a finished run's log says the battle did;
* `phases` - the five runs a capture is, and the report;
* `cli` - the argument parser and the process exit status.

One run does five oracle runs: `scout` (the base tape as it stands: where its
first battle starts, and the selector cells it will write back, cached in
scout.json), `probe` (forced group, seed untouched: the draw's frame and
`hv + frame_count`), `preview` (probe plus the seed patch, untrimmed: the battle
window), `capture` (the same run with the tape trimmed just past the battle's
end) and `verify` (the capture again, byte-compared). The capture and the verify
run share one tape path and their output basenames, differing only in their
output directory, which is what makes their logs comparable byte for byte
(`oracle/host/provenance.h`). A preview run that outlives a defeat walks into
the game-over sequence, where a new game re-initializes `Main_Frame_Count` and
the host refuses to close the trace: its status is reported, not required.
"""

from .capture import (Capture, ability_uses, classify, enemy_slots, matches,
                      read_capture)
from .cli import main
from .draw import GROUP_ENTRIES, Draw, find_draw, patch_specs
from .errors import ForceError
from .pack import Pack, cell_from_row, describe, field_layout, parse_formation
from .phases import (build_report, capture_phase, plan, probe_phase, run)
from .runs import (GROUPS, ORACLE, ROOT, Run, battle_window, by_frame, hp_of,
                   read_rows, run_oracle, seed_of, sha256)
from .scout import SCOUT_GROUPS, SELECTOR_CELLS, scout
from .selectors import (REGION_VEHICLES, VEHICLE_NAMES, VEHICLE_TABLES,
                        Selector, check_vehicle, check_vehicle_for_group,
                        choose_selector, selector_for_group)
from .tape import (PRESS_FRAMES, PROBE_REPEATS, RELEASE_FRAMES, TAIL_FRAMES,
                   Step, compose, emit_tape, expand_tape, policy_steps,
                   tape_frames, trim_tape)

__all__ = [
    "Capture", "Draw", "ForceError", "GROUPS", "GROUP_ENTRIES", "ORACLE",
    "PRESS_FRAMES", "PROBE_REPEATS", "Pack", "REGION_VEHICLES",
    "RELEASE_FRAMES", "ROOT", "Run", "SCOUT_GROUPS", "SELECTOR_CELLS",
    "Selector", "Step", "TAIL_FRAMES", "VEHICLE_NAMES", "VEHICLE_TABLES",
    "ability_uses", "battle_window", "build_report", "by_frame",
    "capture_phase", "cell_from_row", "check_vehicle",
    "check_vehicle_for_group", "choose_selector", "classify", "compose",
    "describe", "emit_tape", "enemy_slots", "expand_tape", "field_layout",
    "find_draw", "hp_of", "main", "matches", "parse_formation", "patch_specs",
    "plan", "policy_steps", "probe_phase", "read_capture", "read_rows", "run",
    "run_oracle", "scout", "seed_of", "selector_for_group", "sha256",
    "tape_frames", "trim_tape",
]
