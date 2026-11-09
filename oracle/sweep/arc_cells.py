"""Augment legacy stock-log captures with party battle attack/defence cells.

GELUN changes atk_pow_battle ($26), while the extractor's legacy party `atk`
column is the derived $24 cell; battle defence is $2A rather than derived $28.
The RAM log's <member>_atk_bat / _dfs_bat columns observe these changes, using
Log.signed(start, column) against Log.signed(end, column), and the replay
comparator checks them. Legacy logs lack enemy $26/$2A columns: their DEBAN
proof is damage afterwards. Scripted captures log enemy battle cells directly
and need no augmentation (oracle.fixture.observations.EFFECT_FIELDS).
"""
from __future__ import annotations

import json
import pathlib
from .. import fixture
from .forced_cases import ROOT

MEMBERS = ((1, "alys"), (2, "chaz"), (3, "hahn"))
EXTRA = ("atk_bat", "dfs_bat")

def augment(path: pathlib.Path, log_path: str) -> int:
    """Append the battle attack and defence cells to each ability action.

    Returns how many cells moved. A fixture the extractor wrote is read back,
    changed in `effect.stats` only, and written in the same minified layout.
    """
    document = json.loads(path.read_text())
    ram_map = fixture.load_ram_map(ROOT / "oracle" / "ram_map.json")
    log = fixture.Log(fixture.load_rows(log_path), ram_map)
    moved = 0
    for round_ in document["rounds"]:
        for action in round_["actions"]:
            if action["kind"] == "attack":
                continue
            for fighter, prefix in MEMBERS:
                for field in EXTRA:
                    column = f"{prefix}_{field}"
                    if not log.has(column):
                        continue
                    before = log.signed(action["start_frame"], column)
                    after = log.signed(action["end_frame"], column)
                    if before != after:
                        action["effect"]["stats"].append(
                            [fighter, field, before, after])
                        moved += 1
    document["provenance"]["effect_cells_added"] = list(EXTRA)
    path.write_text(json.dumps(document, separators=(",", ":"),
                               sort_keys=False) + "\n")
    return moved
