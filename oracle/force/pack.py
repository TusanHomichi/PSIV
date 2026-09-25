"""The generated pack (`generated/`) and `oracle/ram_map.json`'s fields.

`generated/` is the project's own extraction of the cartridge's tables:
`formations.json` (each formation's enemy slots and their ids),
`formation_indexes.json` (the 68 x 32 table of formation ids per group),
`encounters.json` (which group a map's `Battle_EnemyFormationIndexes` byte
holds, and each position grid's cells) and `enemies.json` (the enemy records,
including the ability ids its AI can roll). `oracle/ram_map.json` is the source
of truth for addresses; `oracle/ram_map.tsv`, which the C host reads, is
generated from it and checked for drift by `oracle/verify.sh`.
"""
from __future__ import annotations

import dataclasses
import json
import pathlib

from .errors import ForceError


def field_layout(ram_map: pathlib.Path) -> dict[str, dict]:
    """{name: field} from ram_map.json, the source of truth for addresses."""
    try:
        document = json.loads(ram_map.read_text())
    except (OSError, ValueError) as error:
        raise ForceError(f"cannot read {ram_map}: {error}")
    return {field["name"]: field for field in document["fields"]}


def patch_spec(frame: int, layout: dict[str, dict], name: str,
               value: int) -> str:
    """One `--ram-patch` spec (`<frame>:<address>:<hex>`) for a named cell.

    The one place a patch is spelled, so a cell's address, width and rendering
    are decided once, from `oracle/ram_map.json`: the group selector, the seed
    word and the durable party patch all write through it.
    """
    field = layout.get(name)
    if field is None:
        raise ForceError(f"oracle/ram_map.json has no field {name}: the patch "
                         "needs it")
    size = int(field["size"])
    if not 0 <= value < 1 << (size * 8):
        raise ForceError(f"{name} = {value} does not fit {size} byte(s)")
    return f"{frame}:{field['addr']}:{value:0{size * 2}X}"


def cell_from_row(row: dict, layout: dict[str, dict], name: str) -> int:
    """One selector cell as the log renders it (ram_map's hex/decimal flag)."""
    field = layout.get(name)
    if field is None:
        raise ForceError(f"oracle/ram_map.json has no {name} field")
    if name not in row:
        raise ForceError(
            f"the scout log has no {name} column; the scout runs with "
            "`--groups` (see oracle/force/scout.py, SCOUT_GROUPS)")
    return int(row[name], 16) if field.get("hex") else int(row[name])


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


def parse_formation(text: str, pack: Pack) -> int:
    """A `--formation` value, checked against the pack's own ids."""
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


def describe(pack: Pack, formation: int) -> str:
    """`formation 83 (#$53) = 1:DesrtLeach(id 81, hp 1040)`, from the pack."""
    parts = [f"{entry['slot']}:{pack.enemies[entry['id']]['symbol']}"
             f"(id {entry['id']}, hp {entry['maxhp']})"
             for entry in pack.enemies_of(formation)]
    return f"formation {formation} (#${formation:02X}) = " + ", ".join(parts)
