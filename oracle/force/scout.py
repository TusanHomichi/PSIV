"""The scout: where the base tape's first battle starts, and what it wrote.

`Battle_SetupEnemyData` (`ps4.asm:11813`) reads the group out of RAM cells the
field's own run has already written, so the tool cannot invent them: it runs
the base tape as it stands (once per output directory, cached in `scout.json`),
reads the encounter's first frame and the cells a selector will overwrite, and
writes those cells' *original* values into every patch list so the load is the
only part of the run that sees fixture RAM.
"""
from __future__ import annotations

import hashlib
import json
import pathlib

from . import runs
from .errors import ForceError
from .pack import cell_from_row
from .runs import by_frame, battle_window, read_rows, sha256

#: The scout adds the field cells a selector patches, so the values it writes
#: back after the draw are read from the run rather than assumed.
SCOUT_GROUPS = runs.GROUPS + ",pos"
#: Cells a selector may need; `Field_Map_Index` and `Main_Frame_Count`'s frame
#: come from the core group, the party position from pos, the vehicle from
#: vehicle, and the Motavia background from battle.
SELECTOR_CELLS = ("map_index", "c1_x_px", "c1_y_px", "vehicle_index",
                  "mota_battle_bg_index")


def scout(base_tape: pathlib.Path, base_text: str, out_dir: pathlib.Path,
          cache: pathlib.Path, refresh: bool, dry_run: bool,
          layout: dict[str, dict]) -> dict:
    """The base tape's own run: its first battle, and the selector cells."""
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
    run = runs.run_oracle(base_tape, out_dir / "scout", "scout", [], SCOUT_GROUPS)
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
