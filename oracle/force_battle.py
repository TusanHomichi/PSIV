#!/usr/bin/env python3
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
  grid's cell for a world map, or the vehicle tables (`choose_selector`);
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
from __future__ import annotations

import argparse
import csv
import dataclasses
import hashlib
import json
import pathlib
import subprocess
import sys

ORACLE = pathlib.Path(__file__).resolve().parent
ROOT = ORACLE.parent
BIN = ORACLE / "bin" / "psiv_oracle"
CORE = ORACLE / "core" / "genesis_plus_gx_libretro.so"
ROM = ROOT / "Phantasy Star IV (USA).md"
DEFAULT_TAPE = ORACLE / "tapes" / "07_first_battle.tape"
DEFAULT_DATA_DIR = ROOT / "generated"
DEFAULT_RAM_MAP = ORACLE / "ram_map.json"
DEFAULT_RAM_MAP_TSV = ORACLE / "ram_map.tsv"

#: What every evidence run logs: the fight's own columns plus the RNG chain
#: `oracle/rng_trace.py check` re-derives.
#: What every evidence run logs: the fight's own columns, the RNG chain
#: `oracle/rng_trace.py check` re-derives, and the vehicle cells a vehicle
#: battle's party-side fighter is built from (`Vehicle_Index` and the saved
#: `Vehicle_Stats` record), which a fixture's `vehicle` section reads.
GROUPS = "core,battle,bhit,enemy,chars,rng,vehicle"
#: The scout adds the field cells a selector patches, so the values it writes
#: back after the draw are read from the run rather than assumed.
SCOUT_GROUPS = GROUPS + ",pos"
#: Cells a selector may need; `Field_Map_Index` and `Main_Frame_Count`'s frame
#: come from the core group, the party position from pos, the vehicle from
#: vehicle, and the Motavia background from battle.
SELECTOR_CELLS = ("map_index", "c1_x_px", "c1_y_px", "vehicle_index",
                  "mota_battle_bg_index")


def field_layout(ram_map: pathlib.Path) -> dict[str, dict]:
    """{name: field} from ram_map.json, the source of truth for addresses."""
    try:
        document = json.loads(ram_map.read_text())
    except (OSError, ValueError) as error:
        raise ForceError(f"cannot read {ram_map}: {error}")
    return {field["name"]: field for field in document["fields"]}


def cell_from_row(row: dict, layout: dict[str, dict], name: str) -> int:
    """One selector cell as the log renders it (ram_map's hex/decimal flag)."""
    field = layout.get(name)
    if field is None:
        raise ForceError(f"oracle/ram_map.json has no {name} field")
    if name not in row:
        raise ForceError(
            f"the scout log has no {name} column; the scout runs with "
            f"--groups {SCOUT_GROUPS}")
    return int(row[name], 16) if field.get("hex") else int(row[name])


#: One `C` press per 16 frames, as `oracle/tapes/07_first_battle.tape` and
#: `09_second_battle.tape` do it: 4 held, 12 released.
PRESS_FRAMES = 4
RELEASE_FRAMES = 12
#: Frames of log kept after the battle's last in-battle frame.
TAIL_FRAMES = 60
#: The probe only needs the formation draw, so it stops well before a fight
#: can finish: an untrimmed run that outlives a defeat walks into the game-over
#: sequence, where a new game re-initializes `Main_Frame_Count` and the host
#: rightly refuses to close the trace. Its input frames up to the draw are the
#: capture's own, which is what makes the probe's draw facts hold for it.
PROBE_REPEATS = 200
#: The roll is masked to five bits (`andi.w #$1F`), so groups hold 32 entries.
GROUP_ENTRIES = 32


class ForceError(Exception):
    """The request cannot be satisfied; the message is the reason."""


# ---------------------------------------------------------------------------
# Tape format (`oracle/host/tape.c`): "<frames> <buttons> [mark]", repeat/end.
# ---------------------------------------------------------------------------

@dataclasses.dataclass(frozen=True)
class Step:
    frames: int
    buttons: str
    mark: str = ""


def expand_tape(text: str) -> list[Step]:
    """The tape's steps with `repeat` blocks expanded, in frame order."""
    steps: list[Step] = []
    stack: list[tuple[int, int]] = []
    for raw in text.splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("repeat"):
            stack.append((len(steps), int(line.split()[1])))
            continue
        if line == "end":
            if not stack:
                raise ForceError("tape has an 'end' without 'repeat'")
            start, count = stack.pop()
            block = steps[start:]
            for _ in range(count - 1):
                steps.extend(block)
            continue
        parts = line.split()
        if len(parts) < 2:
            raise ForceError(f"tape line is not '<frames> <buttons>': {line!r}")
        if int(parts[0]) < 1:
            raise ForceError(f"tape line has no frames: {line!r}")
        steps.append(Step(int(parts[0]), parts[1],
                          parts[2] if len(parts) > 2 else ""))
    if stack:
        raise ForceError("tape ends inside a 'repeat' block")
    if not steps:
        raise ForceError("tape has no steps")
    return steps


def tape_frames(steps: list[Step]) -> int:
    return sum(step.frames for step in steps)


def emit_tape(steps: list[Step], header: str = "") -> str:
    """A tape file for these steps, merging runs of markless equal steps."""
    lines: list[Step] = []
    for step in steps:
        if step.frames <= 0:
            continue
        if lines and not step.mark and not lines[-1].mark \
                and lines[-1].buttons == step.buttons:
            lines[-1] = Step(lines[-1].frames + step.frames, step.buttons)
            continue
        lines.append(step)
    body = "".join(
        f"{s.frames} {s.buttons}" + (f" {s.mark}" if s.mark else "") + "\n"
        for s in lines)
    return header + body


def trim_tape(steps: list[Step], upto: int) -> list[Step]:
    """The steps covering frames 1..`upto` (1-based, inclusive)."""
    out: list[Step] = []
    frame = 1
    for step in steps:
        if frame > upto:
            break
        out.append(Step(min(step.frames, upto - frame + 1), step.buttons,
                        step.mark))
        frame += step.frames
    return out


def policy_steps(name: str, repeats: int) -> list[Step]:
    """The scripted input policy as tape steps."""
    if name == "attack":
        return [Step(PRESS_FRAMES, "C"), Step(RELEASE_FRAMES, ".")] * repeats
    if name == "defend":
        # Tape 14: the per-character command menu is HORIZONTAL, so four Right
        # presses step ATTACK -> TECHNIQUE -> SKILL -> ITEM -> DEFEND; the C
        # before them picks COMD and the one after confirms DEFEND.
        block = [Step(PRESS_FRAMES, "C"), Step(RELEASE_FRAMES, ".")]
        for _ in range(4):
            block += [Step(PRESS_FRAMES, "R"), Step(10, ".")]
        block += [Step(PRESS_FRAMES, "C"), Step(RELEASE_FRAMES, ".")]
        return block * repeats
    raise ForceError(f"unknown policy {name!r} (attack, defend)")


def compose(base_steps: list[Step], cut: int, delay: int, repeats: int,
            policy: str) -> list[Step]:
    """Field prefix (frames 1..cut) + optional delay + policy + tail."""
    steps = trim_tape(base_steps, cut)
    if delay:
        steps.append(Step(delay, ".", "delay"))
    steps += policy_steps(policy, repeats)
    steps.append(Step(TAIL_FRAMES * 10, ".", "policy_end"))
    return steps


# ---------------------------------------------------------------------------
# Pack data (`generated/`, the project's own extractors).
# ---------------------------------------------------------------------------

@dataclasses.dataclass
class Pack:
    formations: dict[int, dict]
    enemies: dict[int, dict]
    groups: dict[int, list[int]]
    maps: dict[int, dict]
    grids: dict[int, dict]

    @property
    def formation_ids(self) -> set[int]:
        return set(self.formations)

    @classmethod
    def load(cls, data_dir: pathlib.Path) -> "Pack":
        try:
            forms = json.loads((data_dir / "formations.json").read_text())
            indexes = json.loads((data_dir / "formation_indexes.json").read_text())
            encounters = json.loads((data_dir / "encounters.json").read_text())
            enemies = json.loads((data_dir / "enemies.json").read_text())
        except (OSError, KeyError, ValueError) as exc:
            raise ForceError(f"cannot read the pack in {data_dir}: {exc}")
        return cls(
            formations={f["id"]: f for f in forms["formations"]},
            enemies={e["id"]: e for e in enemies},
            groups={g["group"]: g["formation_ids"] for g in indexes["groups"]},
            maps={m["map_id"]: m for m in encounters["maps"]},
            grids={g["map"]["id"]: g for g in encounters["position_grids"]},
        )

    def group_entries(self, group: int) -> list[int]:
        try:
            return self.groups[group]
        except KeyError:
            raise ForceError(f"group {group} is not in formation_indexes.json")

    def entries_for(self, formation: int) -> dict[int, list[int]]:
        """{group: [entry index, ...]} for every group holding a formation."""
        out: dict[int, list[int]] = {}
        for group, ids in sorted(self.groups.items()):
            hits = [i for i, fid in enumerate(ids) if fid == formation]
            if hits:
                out[group] = hits
        return out

    def enemies_of(self, formation: int) -> list[dict]:
        record = self.formations[formation]
        return [{"slot": entry["slot"], "id": entry["enemy"]["id"],
                 "maxhp": self.enemies[entry["enemy"]["id"]]["hp"]}
                for entry in record["enemies"]]

    def ability_ids(self, enemy_id: int) -> list[int]:
        """Every ability id this enemy can roll, regular and conditional."""
        ai = self.enemies[enemy_id].get("ai", {})
        ids = list(ai.get("regular_ability_ids", []))
        ids += list(ai.get("conditional_ability_ids", []))
        return sorted({value for value in ids if value})

    def grid_cell(self, world: int, group: int) -> tuple[int, int] | None:
        grid = self.grids.get(world)
        if grid is None:
            return None
        for y, row in enumerate(grid["cells"]):
            for x, value in enumerate(row):
                if value == group:
                    return x, y
        return None


# ---------------------------------------------------------------------------
# The group selector.
# ---------------------------------------------------------------------------

@dataclasses.dataclass
class Selector:
    """How a group is forced, and which RAM cells that takes."""

    group: int
    kind: str                       # "map" | "grid" | "vehicle"
    label: str
    #: (ram_map field, value) written one frame after the encounter fires
    cells: list[tuple[str, int]]
    #: which of those cells are written back one frame after the draw (the
    #: vehicle index is deliberately not one of them)
    restore: list[str]


def selector_for_group(pack: Pack, group: int) -> Selector | None:
    """How this one group can be forced, if it can be."""
    map_id = next((map_id for map_id, record in sorted(pack.maps.items())
                   if record.get("in_table") and record.get("group") == group),
                  None)
    if map_id is not None:
        return Selector(
            group, "map",
            f"map {map_id} ({pack.maps[map_id]['map_symbol']}), whose "
            "Battle_EnemyFormationIndexes byte is the group index",
            [("map_index", map_id)], ["map_index"])
    for world in (0, 1):
        cell = pack.grid_cell(world, group)
        if cell is None:
            continue
        grid = ("Battle_MotaFormationGroupIndexes" if world == 0
                else "Battle_DezoFormationGroupIndexes")
        return Selector(
            group, "grid",
            f"world map {world}, position-grid cell ({cell[0]},{cell[1]}) of "
            f"{grid}",
            [("map_index", world), ("c1_x_px", cell[0] * 64),
             ("c1_y_px", cell[1] * 64)],
            ["map_index", "c1_x_px", "c1_y_px"])
    if group in (8, 9, 10, 13):
        return Selector(
            group, "vehicle",
            f"vehicle table {group} (Vehicle_Index set): a battle the vehicle "
            "fights alone",
            [("map_index", 1 if group == 13 else 0), ("vehicle_index", 1),
             ("mota_battle_bg_index", {9: 1, 10: 3}.get(group, 0))],
            ["map_index", "mota_battle_bg_index"])
    return None


def choose_selector(pack: Pack, formation: int) -> Selector:
    """Force this formation's group through the least invasive selector."""
    entries = pack.entries_for(formation)
    if not entries:
        raise ForceError(
            f"formation {formation} (#${formation:02X}) is in no group of "
            "generated/formation_indexes.json, so no encounter can draw it")
    candidates = [s for s in (selector_for_group(pack, group)
                              for group in entries) if s is not None]
    if not candidates:
        raise ForceError(
            f"formation {formation} sits in group(s) {sorted(entries)} and "
            "none of them is reachable: no map carries the group, neither "
            "position grid has a cell for it, and it is not a vehicle table")
    order = {"map": 0, "grid": 1, "vehicle": 2}
    candidates.sort(key=lambda selector: order[selector.kind])
    return candidates[0]


# ---------------------------------------------------------------------------
# Running the oracle.
# ---------------------------------------------------------------------------

@dataclasses.dataclass
class Run:
    log: pathlib.Path
    trace: pathlib.Path
    stderr: str
    status: int


def run_oracle(tape: pathlib.Path, out_dir: pathlib.Path, stem: str,
               patches: list[str], groups: str = GROUPS) -> Run:
    out_dir.mkdir(parents=True, exist_ok=True)
    log = out_dir / f"{stem}.csv"
    trace = out_dir / f"{stem}_rolls.csv"
    argv = [str(BIN), "--core", str(CORE), "--rom", str(ROM),
            "--map", str(DEFAULT_RAM_MAP_TSV), "--tape", str(tape),
            "--groups", groups, "--rng-trace", str(trace), "--out", str(log)]
    for spec in patches:
        argv += ["--ram-patch", spec]
    proc = subprocess.run(argv, capture_output=True, text=True)
    return Run(log, trace, proc.stderr, proc.returncode)


def sha256(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read_rows(path: pathlib.Path) -> list[dict]:
    with open(path) as handle:
        return list(csv.DictReader(line for line in handle
                                   if not line.startswith("#")))


def by_frame(rows: list[dict]) -> dict[int, dict]:
    return {int(row["frame"]): row for row in rows}


def battle_window(rows: list[dict]) -> tuple[int, int] | None:
    """The first and last frame of `Game_Mode_Index` $10/$14, as verify.sh."""
    frames = [int(r["frame"]) for r in rows
              if r["game_mode"] in ("0010", "0014")]
    return (frames[0], frames[-1]) if frames else None


def seed_of(row: dict) -> int:
    return int(row["rng_seed"], 16)


def hp_of(row: dict, column: str) -> int:
    """One fighter's HP as the cartridge stores it: a signed 16-bit word.

    The log renders the columns unsigned, so a dead fighter reads 65511, not
    -25; `oracle/battle_fixture.py` applies the same rule (`log.signed`)."""
    value = int(row[column])
    return value - 0x10000 if value > 0x7FFF else value


# ---------------------------------------------------------------------------
# The scout: where the base tape's first battle starts.
# ---------------------------------------------------------------------------

def scout(base_tape: pathlib.Path, base_text: str, out_dir: pathlib.Path,
          cache: pathlib.Path, refresh: bool, dry_run: bool,
          layout: dict[str, dict]) -> dict:
    digest = hashlib.sha256(base_text.encode()).hexdigest()
    if cache.exists() and not refresh:
        cached = json.loads(cache.read_text())
        if cached.get("base_tape") == str(base_tape) and \
                cached.get("base_sha256") == digest:
            return cached
    if dry_run:
        raise ForceError(
            f"no usable scout data at {cache}: a dry run cannot start one "
            "(the scout is the base tape's own oracle run and it is what says "
            "where the encounter fires). Run without --dry-run once.")
    run = run_oracle(base_tape, out_dir / "scout", "scout", [], SCOUT_GROUPS)
    if run.status != 0:
        raise ForceError(f"the scout run failed (exit {run.status}):\n"
                         f"{run.stderr.strip()}")
    rows = read_rows(run.log)
    window = battle_window(rows)
    if window is None:
        raise ForceError(
            f"{base_tape} never enters a battle: no Game_Mode_Index $10/$14 "
            "frame in its own log")
    row = by_frame(rows).get(window[0])
    if row is None:
        raise ForceError(f"the scout log has no frame {window[0]}")
    facts = {
        "base_tape": str(base_tape),
        "base_sha256": digest,
        "battle_first": window[0],
        "battle_last": window[1],
        "cells": {name: cell_from_row(row, layout, name)
                  for name in SELECTOR_CELLS},
        "scout_log": str(run.log),
        "scout_log_sha256": sha256(run.log),
    }
    cache.parent.mkdir(parents=True, exist_ok=True)
    cache.write_text(json.dumps(facts, indent=2, sort_keys=True) + "\n")
    return facts


# ---------------------------------------------------------------------------
# The draw: the frame the formation is rolled.
# ---------------------------------------------------------------------------

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


# ---------------------------------------------------------------------------
# Patch list.
# ---------------------------------------------------------------------------

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


# ---------------------------------------------------------------------------
# Reading a capture.
# ---------------------------------------------------------------------------

@dataclasses.dataclass
class Capture:
    window: tuple[int, int]
    outcome: str
    enemies: list[dict]
    party: list[dict]
    vehicle_hp: int
    abilities: dict[str, int]
    rewards: dict
    log_path: pathlib.Path
    trace_path: pathlib.Path
    log_sha256: str
    trace_sha256: str


def enemy_slots(rows: list[dict], window: tuple[int, int]) -> list[dict]:
    """The enemy slots the battle built, as (slot, id, maxhp) at the end."""
    first, last = window
    count = max((int(row["enemy_count"]) for row in rows
                 if first <= int(row["frame"]) <= last), default=0)
    end = by_frame(rows)[last]
    return [{"slot": slot, "id": int(end[f"e{slot}_id"]),
             "maxhp": int(end[f"e{slot}_maxhp"])}
            for slot in range(1, 5) if slot <= count]


def classify(rows: list[dict], window: tuple[int, int],
             vehicle: bool = False) -> str:
    """What the log says the battle did, from the party's and enemies' HP.

    `victory` needs every built enemy slot at or below zero HP, `defeat` every
    party member. A battle that ends with both sides standing is reported as
    `withdrawal` rather than guessed at - an enemy escape, or a vehicle battle
    whose wrecked vehicle the party's own HP columns cannot show (the vehicle
    is the party-side fighter there, and `vehicle_fighter_hp` says so).
    """
    if window[1] >= int(rows[-1]["frame"]):
        return "unfinished"
    end = by_frame(rows)[window[1]]
    slots = enemy_slots(rows, window)
    if slots and all(hp_of(end, f"e{entry['slot']}_hp") <= 0
                     for entry in slots):
        return "victory"
    if vehicle and hp_of(end, "vehicle_fighter_hp") <= 0:
        return "defeat"
    if all(hp_of(end, f"{who}_hp") <= 0 for who in ("chaz", "alys", "hahn")):
        return "defeat"
    return "withdrawal"


def ability_uses(rows: list[dict], window: tuple[int, int],
                 from_frame: int) -> dict[str, int]:
    """{"eN=0xID": first frame}: the ability each enemy actually ran."""
    first, last = window
    seen: dict[str, int] = {}
    for row in rows:
        frame = int(row["frame"])
        if not max(first, from_frame) <= frame <= last:
            continue
        for slot in (1, 2, 3, 4):
            value = int(row[f"e{slot}_ability"], 16)
            if value:
                seen.setdefault(f"e{slot}=0x{value:02X}", frame)
    return seen


def read_capture(run: Run, draw_frame: int, vehicle: bool = False) -> Capture:
    rows = read_rows(run.log)
    window = battle_window(rows)
    if window is None:
        raise ForceError(f"the run in {run.log.parent} holds no battle: "
                         "game mode $10/$14 never appears")
    end = by_frame(rows)[window[1]]
    return Capture(
        window=window,
        outcome=classify(rows, window, vehicle),
        enemies=enemy_slots(rows, window),
        party=[{"who": who, "hp": hp_of(end, f"{who}_hp")}
               for who in ("chaz", "alys", "hahn")],
        vehicle_hp=hp_of(end, "vehicle_fighter_hp")
        if "vehicle_fighter_hp" in end else 0,
        abilities=ability_uses(rows, window, draw_frame + 1),
        rewards={"experience": int(end["battle_exp_total"]),
                 "meseta": int(end["battle_meseta_total"])},
        log_path=run.log, trace_path=run.trace,
        log_sha256=sha256(run.log),
        trace_sha256=sha256(run.trace),
    )


def matches(capture: Capture, pack: Pack, formation: int) -> bool:
    return capture.enemies == pack.enemies_of(formation)


def describe(pack: Pack, formation: int) -> str:
    parts = [f"{entry['slot']}:{pack.enemies[entry['id']]['symbol']}"
             f"(id {entry['id']}, hp {entry['maxhp']})"
             for entry in pack.enemies_of(formation)]
    return f"formation {formation} (#${formation:02X}) = " + ", ".join(parts)


# ---------------------------------------------------------------------------
# CLI.
# ---------------------------------------------------------------------------

def parse_formation(text: str, pack: Pack) -> int:
    try:
        value = int(text, 0)
    except ValueError:
        raise ForceError(f"--formation {text!r} is not a number (try 0x5E)")
    if value not in pack.formation_ids:
        raise ForceError(
            f"formation {value} (#${value:02X}) is not in "
            f"generated/formations.json, whose ids run "
            f"{min(pack.formation_ids)}..{max(pack.formation_ids)}")
    return value


def plan(args, pack: Pack, layout: dict, selector: Selector, formation: int,
         facts: dict, base_steps: list[fb.Step]) -> dict:
    """The composed tape, its header and the selector-only patch list."""
    cut = facts["battle_first"]
    stem = f"forced_{formation:02X}_{args.policy}" \
        + (f"_d{args.delay}" if args.delay else "")
    header = (
        f"# Forced battle capture written by oracle/force_battle.py\n"
        f"# {describe(pack, formation)}\n"
        f"# group {selector.group} ({selector.kind}) forced via "
        f"{selector.label}\n"
        f"# base tape {args.base_tape}, frames 1..{cut}: its encounter fires at "
        f"f{cut}, the formation is drawn a few frames later\n"
        f"# policy {args.policy}"
        + (f", delay {args.delay} idle frames at the seam" if args.delay else "")
        + f", {args.repeats} block(s)\n")
    steps = compose(base_steps, cut, args.delay, args.repeats, args.policy)
    out = pathlib.Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    tape = out / f"{stem}.tape"
    full_tape = out / f"{stem}.full.tape"
    full_tape.write_text(emit_tape(steps, header))
    return {"cut": cut, "stem": stem, "header": header, "steps": steps,
            "out": out, "tape": tape, "full_tape": full_tape,
            "selector_specs": patch_specs(selector, facts, layout)}


def probe_phase(plan_facts: dict, args, selector: Selector, pack: Pack,
                base_steps: list[Step]) -> Draw:
    """Run the probe and hand back the draw, with the model's own checks.

    The probe forces the group and leaves the seed alone, so its log says which
    formation the group's draw produced - which must be the group table's entry
    - and its trace says where the draw is and what `hv + frame_count` was.
    """
    out, stem = plan_facts["out"], plan_facts["stem"]
    steps = compose(base_steps, plan_facts["cut"], args.delay,
                    min(args.repeats, PROBE_REPEATS), args.policy)
    tape = out / f"{stem}.probe.tape"
    tape.write_text(emit_tape(steps, plan_facts["header"]))
    run = run_oracle(tape, out / "probe", stem, plan_facts["selector_specs"])
    if run.status != 0:
        raise ForceError(f"the probe run failed (exit {run.status}):\n"
                         f"{run.stderr.strip()}")
    rows = read_rows(run.log)
    window = battle_window(rows)
    if window is None:
        raise ForceError("the probe never entered a battle: the forced group "
                         "patch did not survive to the load")
    draw = find_draw(read_rows(run.trace), rows, plan_facts["cut"])
    drawn = pack.group_entries(selector.group)[draw.index]
    print(f"probe: the formation is drawn at f{draw.frame} "
          f"({draw.rows_in_frame} call(s) that frame), roll ${draw.roll:04X} "
          f"& 31 = {draw.index} -> formation {drawn}, K = {draw.k}")
    if tape_frames(steps) < draw.frame:
        raise ForceError(f"the probe tape ({tape_frames(steps)} frames) does "
                         f"not reach the draw at f{draw.frame}")
    built = enemy_slots(rows, window)
    if built != pack.enemies_of(drawn):
        raise ForceError(
            f"the probe built {built}, but group {selector.group}"
            f"[{draw.index}] is formation {drawn} = {pack.enemies_of(drawn)}: "
            "the forcing model does not explain this run")
    # The seed patch must land where nothing else moves the seed, or the trace
    # it produces cannot be checked.
    before = by_frame(rows).get(draw.frame_before)
    if before is None or seed_of(before) != draw.seed_before:
        raise ForceError(
            f"the seed at the draw (${draw.seed_before:08X}) is not the seed "
            f"the probe log holds at f{draw.frame_before} "
            f"(${seed_of(before) if before else 0:08X}): a one-frame-earlier "
            "patch would not fix the draw")
    if any(int(row["frame"]) == draw.frame_before
           for row in read_rows(run.trace)):
        raise ForceError(
            f"f{draw.frame_before} has an UpdateRNGSeed2 call of its own, so "
            "the seed patch would break the trace's chain")
    return draw


def capture_phase(plan_facts: dict, specs: list[str], draw: Draw, pack: Pack,
                  selector: Selector, formation: int) -> tuple[Capture, int]:
    """Preview, trim, capture and re-run; hand back the capture and the trim.

    The preview is the untrimmed run: it says when the battle ended, which is
    what the trim needs, and it may walk into the game-over sequence after a
    defeat (its exit status is reported, not required). The capture and its
    re-run are the evidence, and the tool has already proved their inputs are
    identical apart from their output directory.
    """
    out, stem, tape = plan_facts["out"], plan_facts["stem"], plan_facts["tape"]
    preview = run_oracle(plan_facts["full_tape"], out / "preview", stem, specs)
    if preview.status != 0:
        print(f"note: the untrimmed preview run ended with exit "
              f"{preview.status} (post-battle game-over sequence); the trimmed "
              "capture is the evidence", file=sys.stderr)
    preview_capture = read_capture(preview, draw.frame,
                                  selector.kind == "vehicle")
    window = preview_capture.window
    if not matches(preview_capture, pack, formation):
        raise ForceError(
            f"the capture built {preview_capture.enemies}, not formation "
            f"{formation} ({pack.enemies_of(formation)}); the seed patch missed")
    print(f"capture: f{window[0]}-{window[1]}, outcome "
          f"{preview_capture.outcome}, enemies "
          f"{[(e['slot'], e['id'], e['maxhp']) for e in preview_capture.enemies]}")
    print("         abilities used: " + (", ".join(
        f"{key} at f{value}" for key, value in
        sorted(preview_capture.abilities.items())) or "none"))
    if preview_capture.outcome == "unfinished":
        raise ForceError("the battle had not ended when the tape ran out; "
                         "re-run with more --repeats")

    trim_to = min(tape_frames(plan_facts["steps"]), window[1] + TAIL_FRAMES)
    tape.write_text(emit_tape(trim_tape(plan_facts["steps"], trim_to),
                              plan_facts["header"]))
    capture = run_oracle(tape, out / "capture", stem, specs)
    rerun = run_oracle(tape, out / "verify", stem, specs)
    for label, run in (("capture", capture), ("verify", rerun)):
        if run.status != 0:
            raise ForceError(f"the {label} run failed (exit {run.status}):\n"
                             f"{run.stderr.strip()}")
    final = read_capture(capture, draw.frame, selector.kind == "vehicle")
    if final.window != window:
        raise ForceError(f"trimming the tape moved the battle, {window} -> "
                         f"{final.window}")
    if not matches(final, pack, formation):
        raise ForceError(f"the trimmed capture built {final.enemies}, not "
                         f"{pack.enemies_of(formation)}")
    if final.log_sha256 != sha256(rerun.log) \
            or final.trace_sha256 != sha256(rerun.trace):
        raise ForceError("two runs of the same tape and patches differ")
    return final, preview.status


def build_report(args, pack: Pack, selector: Selector, formation: int,
                 forced_index: int, facts: dict, plan_facts: dict, draw: Draw,
                 specs: list[str], final: Capture, preview_status: int,
                 cut: int, missing: list[str]) -> dict:
    """Everything a reader needs to re-run the capture and judge it."""
    entries = pack.entries_for(formation)
    return {
        "formation": formation,
        "formation_hex": f"0x{formation:02X}",
        "formation_enemies": pack.enemies_of(formation),
        "formation_ability_ids": sorted(
            {value for entry in pack.enemies_of(formation)
             for value in pack.ability_ids(entry["id"])}),
        "action": describe(pack, formation),
        "policy": args.policy,
        "delay": args.delay,
        "repeats": args.repeats,
        "base_tape": str(args.base_tape),
        "base_battle_first": cut,
        "selector": {"group": selector.group, "kind": selector.kind,
                     "label": selector.label, "entry": forced_index,
                     "entries": entries[selector.group],
                     "cells": selector.cells, "restore": selector.restore,
                     "cells_before": facts["cells"]},
        "draw": {"frame": draw.frame, "hv": f"0x{draw.hv:04X}",
                 "frame_count": draw.frame_count,
                 "seed_before": f"0x{draw.seed_before:08X}",
                 "roll": f"0x{draw.roll:04X}", "index_before": draw.index,
                 "index_forced": forced_index, "k": draw.k},
        "patches": specs,
        "tape": str(plan_facts["tape"]),
        "tape_frames": tape_frames(expand_tape(plan_facts["tape"].read_text())),
        "battle_first": final.window[0], "battle_last": final.window[1],
        "outcome": final.outcome, "enemies": final.enemies,
        "party": final.party, "vehicle_fighter_hp": final.vehicle_hp,
        "abilities": final.abilities, "rewards": final.rewards,
        "trace": str(final.trace_path), "trace_sha256": final.trace_sha256,
        "log": str(final.log_path), "log_sha256": final.log_sha256,
        "rerun_log": str(pathlib.Path(final.log_path).parent.parent
                         / "verify" / pathlib.Path(final.log_path).name),
        "rerun_log_sha256": final.log_sha256,
        "preview_status": preview_status,
        "rng_trace_check": "passed",
        "require_ability": [f"0x{value:02X}" for value in args.require_ability],
        "require_ability_met": not missing,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Force a formation into a battle and capture it.")
    parser.add_argument("--formation", required=True,
                        help="formation id, decimal or 0x-prefixed")
    parser.add_argument("--out", required=True, help="output directory")
    parser.add_argument("--policy", default="attack",
                        choices=("attack", "defend"))
    parser.add_argument("--delay", type=int, default=0,
                        help="idle frames between the field prefix and the "
                             "policy; shifts every roll in the fight")
    parser.add_argument("--repeats", type=int, default=900,
                        help="policy blocks (attack: one 16-frame press)")
    parser.add_argument("--base-tape", default=str(DEFAULT_TAPE))
    parser.add_argument("--data-dir", default=str(DEFAULT_DATA_DIR),
                        help="the generated pack directory")
    parser.add_argument("--ram-map", default=str(DEFAULT_RAM_MAP))
    parser.add_argument("--require-ability", action="append", default=[],
                        type=lambda v: int(v, 0),
                        help="fail unless this enemy ability id was used")
    parser.add_argument("--scout", default="", help="scout cache path")
    parser.add_argument("--refresh-scout", action="store_true")
    parser.add_argument("--dry-run", action="store_true",
                        help="write the tape and the selector patches, then "
                             "stop (needs scout data, starts no oracle run)")
    args = parser.parse_args(argv)
    try:
        return _run(args)
    except ForceError as error:
        print(f"force_battle: {error}", file=sys.stderr)
        return 2


def _run(args) -> int:
    out = pathlib.Path(args.out)
    layout = field_layout(pathlib.Path(args.ram_map))
    pack = Pack.load(pathlib.Path(args.data_dir))
    formation = parse_formation(args.formation, pack)
    selector = choose_selector(pack, formation)
    if args.delay < 0 or args.repeats < 1:
        raise ForceError("--delay must be >= 0 and --repeats >= 1")
    base_tape = pathlib.Path(args.base_tape)
    try:
        base_text = base_tape.read_text()
    except OSError as error:
        raise ForceError(str(error)) from error
    base_steps = expand_tape(base_text)
    forced_index = pack.entries_for(formation)[selector.group][0]
    facts = scout(base_tape, base_text, out,
                  pathlib.Path(args.scout) if args.scout else out / "scout.json",
                  args.refresh_scout, args.dry_run, layout)
    plan_facts = plan(args, pack, layout, selector, formation, facts, base_steps)

    print(describe(pack, formation))
    print(f"group {selector.group} via {selector.label}; entry {forced_index} of"
          f" {len(pack.group_entries(selector.group))}")
    print(f"base tape {base_tape}: first battle at f{plan_facts['cut']}, so the "
          f"tape is {tape_frames(plan_facts['steps'])} frames ({args.policy}"
          + (f", delay {args.delay}" if args.delay else "") + ")")
    if selector.kind == "vehicle":
        print("note: the vehicle tables are only reachable with Vehicle_Index "
              "set, so the vehicle fights this battle alone (loc_78EE, "
              "ps4.asm:11408)")
    stem = plan_facts["stem"]
    if args.dry_run:
        patches = plan_facts["out"] / f"{stem}.patches.txt"
        patches.write_text(
            "# selector patches only: --dry-run stops before the probe that "
            "measures\nthe draw frame the seed patch is placed against\n"
            + "\n".join(plan_facts["selector_specs"]) + "\n")
        print(f"dry run: wrote {plan_facts['full_tape']} and {patches}")
        print(f"  selector patches: "
              f"{' '.join(plan_facts['selector_specs'])}")
        return 0

    draw = probe_phase(plan_facts, args, selector, pack, base_steps)
    specs = patch_specs(selector, facts, layout, draw, forced_index)
    (plan_facts["out"] / f"{stem}.patches.txt").write_text(
        f"# --ram-patch list for {stem}\n"
        f"# f{facts['battle_first'] + 1}: the group selector, one frame after "
        f"the encounter fires\n"
        f"# f{draw.frame_before}: RNG_Seed's high word, one frame before the "
        f"formation draw (K = {draw.k}, entry {forced_index})\n"
        f"# f{draw.frame + 1}: the selector cells written back\n"
        + "\n".join(specs) + "\n")
    print("patches: " + "  ".join(specs))

    final, preview_status = capture_phase(plan_facts, specs, draw, pack,
                                         selector, formation)
    checked = subprocess.run(
        [sys.executable, str(ORACLE / "rng_trace.py"), "check",
         str(final.trace_path), str(final.log_path)],
        capture_output=True, text=True)
    sys.stdout.write(checked.stdout)
    if checked.returncode != 0:
        sys.stderr.write(checked.stderr)
        print("force_battle: rng_trace.py check failed", file=sys.stderr)
        return 1

    missing = [f"0x{value:02X}" for value in args.require_ability
               if not any(key.endswith(f"=0x{value:02X}")
                          for key in final.abilities)]
    report = build_report(args, pack, selector, formation, forced_index, facts,
                          plan_facts, draw, specs, final, preview_status,
                          plan_facts["cut"], missing)
    (plan_facts["out"] / "report.json").write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(f"capture: f{report['battle_first']}-{report['battle_last']}, "
          f"outcome {report['outcome']}, {report['tape_frames']} tape frames, "
          "two runs byte-identical")
    print(f"  trace {report['trace_sha256']}")
    print(f"  log   {report['log_sha256']}")
    print(f"  report {plan_facts['out'] / 'report.json'}")
    if missing:
        print(f"force_battle: required abilit"
              + ("y " if len(missing) == 1 else "ies ")
              + f"{', '.join(missing)} never fired; try another --delay",
              file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except ForceError as error:
        print(f"force_battle: {error}", file=sys.stderr)
        sys.exit(2)
