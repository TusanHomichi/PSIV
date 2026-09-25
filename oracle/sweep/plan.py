"""The list: which formations the Motavia sweep captures, computed from the pack.

`Battle_SetupEnemyData` (`ps4.asm:11813`) picks a group and then an entry in it,
so "which formations can the Motavia overworld and its vehicle tables draw" is
the *union* of the ids the ten Motavia groups hold:

* groups **0-7** are the overworld's own - a map's
  `Battle_EnemyFormationIndexes` byte, or the position grid's cell for the
  chunk the party stands on (`loc_7E4C`, `ps4.asm:11841-11859`);
* groups **8, 9, 10** are the Motavia vehicle tables, which
  `Battle_SetupEnemyData` picks by region when the map byte is `<= 1` and
  `Vehicle_Index` is nonzero (`ps4.asm:11825-11839`); group 13 is Dezolis's and
  is not part of this sweep.

The two sets are disjoint in the pack as it stands - the vehicle tables' 16
formations sit in no on-foot group - but nothing here assumes that: a formation
is collected once, with every group it appears in, and a formation reachable
both ways is recorded as such (`kind` is `vehicle` only when it sits in a
vehicle table *and* nowhere on foot).
"""
from __future__ import annotations

import dataclasses
import json
import pathlib

from ..force.errors import ForceError
from ..force.pack import Pack
from ..force.selectors import REGION_DEFAULT_VEHICLE, VEHICLE_TABLES

#: The Motavia overworld's own groups: on foot.
FOOT_GROUPS = (0, 1, 2, 3, 4, 5, 6, 7)
#: The Motavia vehicle tables (`ps4.asm:11825-11839`).
VEHICLE_GROUPS = (8, 9, 10)
MOTAVIA_GROUPS = FOOT_GROUPS + VEHICLE_GROUPS
#: `Field_Map_Index` for Motavia, which is what the vehicle tables are picked
#: on (and the region whose default machine the sweep fights them with).
MOTA_MAP_INDEX = 0


@dataclasses.dataclass(frozen=True)
class Formation:
    """One formation the sweep captures, and how it is reached."""

    formation: int
    #: Every Motavia group holding it, ascending.
    groups: tuple[int, ...]
    #: `foot` for a formation reachable in groups 0-7, `vehicle` for one that
    #: is only in a vehicle table.
    kind: str
    #: The `--vehicle` the capture is told to use, for `kind == "vehicle"`.
    vehicle: int | None

    @property
    def hex(self) -> str:
        return f"0x{self.formation:02X}"

    def as_json(self) -> dict:
        return {"formation": self.formation, "hex": self.hex,
                "groups": list(self.groups), "kind": self.kind,
                "vehicle": self.vehicle}


def list_formations(pack: Pack) -> list[Formation]:
    """Every distinct formation id in the Motavia groups, in id order."""
    foot: dict[int, list[int]] = {}
    vehicle: dict[int, list[int]] = {}
    for group in MOTAVIA_GROUPS:
        entries = pack.groups.get(group)
        if entries is None:
            raise ForceError(
                f"generated/formation_indexes.json has no group {group}: the "
                "sweep's Motavia groups are 0-7 (on foot) and 8, 9, 10 (the "
                "vehicle tables)")
        seen = foot if group in FOOT_GROUPS else vehicle
        for formation in set(entries):
            seen.setdefault(formation, []).append(group)
    formations = []
    for formation in sorted(set(foot) | set(vehicle)):
        groups = tuple(sorted(set(foot.get(formation, []))
                              | set(vehicle.get(formation, []))))
        on_foot = formation in foot
        # A formation the overworld draws is captured on foot: the group is
        # what picks the table, and a vehicle only reaches one when the party
        # is mounted (which a tape's field prefix can be, but this sweep's is
        # not).
        index = None if on_foot else REGION_DEFAULT_VEHICLE[MOTA_MAP_INDEX]
        formations.append(Formation(
            formation=formation, groups=groups,
            kind="foot" if on_foot else "vehicle", vehicle=index))
    return formations


def write_list(path: pathlib.Path, formations: list[Formation],
               source: dict) -> None:
    """The committed list: what the sweep captures, and where the ids came from."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps({
        "source": source,
        "count": len(formations),
        "formations": [entry.as_json() for entry in formations],
    }, indent=1) + "\n")
