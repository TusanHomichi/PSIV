"""The pack's battle half: enemies, formations, levels and abilities.

`generated/` already holds all of this, decoded and provenance-heavy. This
module does not re-derive any of it -- it imports the same extractors the
`extract` command uses and reshapes their output for the runtime, the way
`psiv_tools.pack` does for field maps. Six files under `battle/`:

    enemies.json     153 records: stats, the fourteen element properties, the
                     AI lists, rewards, and what a basic attack is
    formations.json  504 formations + 27 boss formations, the 68 encounter
                     groups, the per-map bindings and both overworld position
                     grids
    levels.json      the eleven characters' progression, 937 records
    abilities.json   the 8-byte records the damage pipeline consumes:
                     techniques, skills, enemy skills, and every item's
                     embedded battle effect
    characters.json  the eleven `InitialCharStats` records and the RAM state
                     `InitializeCharStats` builds from them -- what a party is
                     seated from
    equipment.json   every inventory record's battle half, and the cartridge
                     rules that give the type, bonus and element bytes meaning

The last two are built by `psiv_tools.battle_records`, which decodes the five
routines those rules live in rather than transcribing them.

Field names come from the extractors, which proved them against the cartridge,
with one deliberate exception: the formation header is renamed to what the
fields mean rather than what an early pass guessed they meant. The mapping is
in `FORMATION_HEADER_RENAMES` and is the only place the two vocabularies
differ.

Effect ids
----------

Every ability record's first byte is an index into `AbilityEffectsOffs`, the
44-entry word table at `$0061BE` that `TRAP #2` dispatches through with
`add.w d0,d0 / adda.w (a0,d0.w),a0 / jsr (a0)` -- no bounds check. The table's
length is derived here rather than carried: each entry is an offset from the
table's own base, and the lowest target is `AbilityEffect_None` at `$006216`,
immediately after the table, so the entry count falls out as
`(0x6216 - 0x61BE) / 2`.

One retail record is past that end. Enemy skill 112 `BLACK WAVE` declares
effect `$2C`, and reading the word one entry past the table gives `$4E75`,
which as an offset points at the odd address `$00B033` -- an address error, as
`docs/BATTLE_SCOUT.md` section 11 finding 2 records. It is dormant because the
only enemy that uses it appears in none of the 531 formations. The record is
emitted with `effect_out_of_range: true` so a consumer rejects it deliberately
rather than jumping where the cartridge would have.
"""

from __future__ import annotations

import hashlib
import json
import struct
from pathlib import Path
from typing import Any

from .core import (
    extract_enemies,
    extract_enemy_skills,
    extract_items,
    extract_level_progression,
    extract_skills,
    extract_techniques,
)
from .battle_records import build_characters, build_equipment
from .formations import extract_formation_indexes, extract_formations
from .maps.encounters import extract_encounter_binding
from .text import extract_names

#: `extract_all` attaches the cartridge's own display names to the records the
#: symbols came from; the pack does the same rather than shipping records that
#: only a disassembly reader can identify. See SOURCE_NOTES on the boundary:
#: the symbol disambiguates duplicates the display text does not.
DISPLAY_NAME_TABLES = {
    "enemies": "enemy_names",
    "items": "item_names",
    "enemy_skills": "enemy_skill_names",
    "techniques": "technique_names",
    "skills": "skill_names",
    "characters": "character_names",
    "professions": "profession_names",
}


def _display_names(rom: bytes) -> dict[str, dict[int, str]]:
    names = extract_names(rom)
    return {
        record_key: {entry["id"]: entry["name"] for entry in names[table]}
        for record_key, table in DISPLAY_NAME_TABLES.items()
    }

#: Bumped with `psiv_tools.pack.PACK_FORMAT_VERSION`; the battle files carry the
#: same number so one pack is one version.
BATTLE_DIRECTORY = "battle"
ENEMIES_NAME = f"{BATTLE_DIRECTORY}/enemies.json"
FORMATIONS_NAME = f"{BATTLE_DIRECTORY}/formations.json"
LEVELS_NAME = f"{BATTLE_DIRECTORY}/levels.json"
ABILITIES_NAME = f"{BATTLE_DIRECTORY}/abilities.json"
CHARACTERS_NAME = f"{BATTLE_DIRECTORY}/characters.json"
EQUIPMENT_NAME = f"{BATTLE_DIRECTORY}/equipment.json"

#: `AbilityEffectsOffs`, and the routine that sits immediately after it.
ABILITY_EFFECTS_OFFS = 0x0061BE
ABILITY_EFFECT_NONE = 0x006216

#: The formation header, from the extractor's names to the pack's. The left
#: column is what `psiv_tools.formations` calls them; the right is what they
#: do. `surprise_agility` and `run_agility` are compared against agility, but
#: what a consumer needs from them is the chance, so they are named for that.
FORMATION_HEADER_RENAMES = {
    "surprise_agility": "ambush_chance",
    "run_agility": "run_chance",
    "item_drop_rate": "drop_rate",
    "dropped_item": "drop_item",
    "enemy_count": "count",
    "enemy_count_matches_entries": "count_matches_entries",
}

#: Emitted per record so a consumer can index the whole set without a lookup
#: table of its own.
ABILITY_KINDS = ("techniques", "skills", "enemy_skills", "item_effects")


class BattlePackError(ValueError):
    pass


# ---------------------------------------------------------------------------
# Ability effect ids
# ---------------------------------------------------------------------------
def ability_effect_count(rom: bytes) -> int:
    """How many entries `AbilityEffectsOffs` really has.

    Each entry is an offset from the table's base, so the table ends where its
    lowest target begins -- which is `AbilityEffect_None`, the `rts` placed
    immediately after it. Deriving the count this way means the bound comes
    from the cartridge instead of from a comment.
    """
    base = ABILITY_EFFECTS_OFFS
    if rom[ABILITY_EFFECT_NONE:ABILITY_EFFECT_NONE + 2] != b"\x4e\x75":
        raise BattlePackError(
            f"0x{ABILITY_EFFECT_NONE:06X} is not AbilityEffect_None's `rts`; the "
            "ability effect table cannot be bounded"
        )
    count = (ABILITY_EFFECT_NONE - base) // 2
    targets = [
        base + word for word in struct.unpack_from(f">{count}H", rom, base)
    ]
    if min(targets) != ABILITY_EFFECT_NONE:
        raise BattlePackError(
            f"the lowest AbilityEffectsOffs target is 0x{min(targets):06X}, not the "
            f"0x{ABILITY_EFFECT_NONE:06X} that bounds the table"
        )
    return count


def _out_of_range_target(rom: bytes, effect_id: int) -> str:
    """Where the dispatcher would jump for an effect id past the table."""
    base = ABILITY_EFFECTS_OFFS
    word = int.from_bytes(rom[base + effect_id * 2:base + effect_id * 2 + 2], "big")
    return f"0x{base + word:06X}"


# ---------------------------------------------------------------------------
# Enemies
# ---------------------------------------------------------------------------
def build_enemies(rom: bytes, effect_count: int,
                  display: dict[int, str]) -> dict[str, Any]:
    records = extract_enemies(rom)
    enemies = []
    for record in records:
        enemies.append({
            "id": record["id"],
            "symbol": record["symbol"],
            "display_name": display.get(record["id"]),
            "rom_offset": record["rom_offset"],
            "hp": record["hp"],
            "stats": record["stats"],
            # `Battle_SetupEnemyData`'s two attack bytes: what element a plain
            # attack carries and what it inflicts.
            "attack": record["basic_attack"],
            "properties": record["properties"],
            "ai": {
                key: record["ai"][key]
                for key in ("condition_ids", "conditional_ability_ids",
                            "regular_ability_ids")
            },
            "rewards": {
                "experience": record["experience_reward"],
                "meseta": record["meseta_reward"],
            },
        })

    properties = sorted({name for e in enemies for name in e["properties"]})
    values: dict[int, int] = {}
    for enemy in enemies:
        for slot in enemy["properties"].values():
            values[slot["value"]] = values.get(slot["value"], 0) + 1
    return {
        "kind": "battle_enemies",
        "count": len(enemies),
        "source": {"label": "Battle_EnemyData", "rom_offset": records[0]["rom_offset"],
                   "record_bytes": len(records[0]["raw_hex"]) // 2},
        "properties": properties,
        "enemies": enemies,
        "census": {
            "property_values": {str(v): c for v, c in sorted(values.items())},
            "property_slots": len(properties),
            "hp": _range(e["hp"] for e in enemies),
            "experience": _range(e["rewards"]["experience"] for e in enemies),
            "meseta": _range(e["rewards"]["meseta"] for e in enemies),
            "with_conditional_ai": sum(
                1 for e in enemies if any(e["ai"]["conditional_ability_ids"])
            ),
            "ability_ids_used": sorted({
                value for e in enemies
                for key in ("conditional_ability_ids", "regular_ability_ids")
                for value in e["ai"][key]
            }),
        },
    }


def _range(values) -> dict[str, int]:
    materialised = list(values)
    return {"min": min(materialised), "max": max(materialised)}


# ---------------------------------------------------------------------------
# Formations and encounters
# ---------------------------------------------------------------------------
def _formation(record: dict[str, Any]) -> dict[str, Any]:
    """One formation, with the header renamed to what its fields mean."""
    out: dict[str, Any] = {}
    for key, value in record.items():
        if key in ("raw_hex", "source", "block_offset"):
            continue
        if key == "enemies":
            out["enemies"] = [
                {"slot": slot["slot"], "enemy_id": slot["enemy"]["id"],
                 "position": slot["position"], "groups": slot["groups"]}
                for slot in value
            ]
            continue
        if key == "dropped_item":
            out["drop_item"] = None if value is None else value["id"]
            continue
        out[FORMATION_HEADER_RENAMES.get(key, key)] = value
    return out


def build_formations(rom: bytes) -> dict[str, Any]:
    formations = extract_formations(rom)
    known = {record["id"] for record in formations["formations"]}
    indexes = extract_formation_indexes(rom, known)
    encounters = extract_encounter_binding(rom, indexes["group_count"])

    normal = [_formation(record) for record in formations["formations"]]
    bosses = [_formation(record) for record in formations["boss_formations"]]
    mismatched = [f["id"] for f in normal + bosses if not f["count_matches_entries"]]

    grids = [
        {key: grid[key] for key in
         ("name", "label", "map", "rom_offset", "columns", "rows",
          "cell_size_pixels", "indexing", "groups_used", "fallback_group",
          "fallback_cells", "cells")}
        for grid in encounters["position_grids"]
    ]
    # Only the two overworlds carry a position grid and the vehicle groups; a
    # missing key there is the shape of the data, not a gap, so the projection
    # fills it with null rather than dropping the field per map.
    bindings = [
        {
            **{key: entry[key] for key in
               ("map_id", "map_symbol", "in_table", "value", "mode", "group")},
            "position_grid": entry.get("position_grid"),
            "groups_available": entry.get("groups_available"),
            "vehicle_groups": entry.get("vehicle_groups"),
        }
        for entry in encounters["maps"]
    ]
    return {
        "kind": "battle_formations",
        "formation_count": len(normal),
        "boss_formation_count": len(bosses),
        "formations": normal,
        "boss_formations": bosses,
        "encounter_groups": {
            "source": indexes["source"],
            "group_size_bytes": indexes["group_size_bytes"],
            "entries_per_group": indexes["entries_per_group"],
            "group_count": indexes["group_count"],
            "groups": [
                {"group": group["group"], "formation_ids": group["formation_ids"]}
                for group in indexes["groups"]
            ],
        },
        "map_bindings": bindings,
        "position_grids": grids,
        "vehicle_groups": encounters["vehicle_groups"],
        "table": encounters["table"],
        "census": {
            # The ROM's own inconsistency: one formation declares more enemies
            # than it lists. `SOURCE_NOTES` has the blast radius -- the count
            # byte's only reader is the Slasher hit effect's positioning.
            "count_mismatches": mismatched,
            "enemies_per_formation": _range(len(f["enemies"]) for f in normal + bosses),
            "formations_with_a_drop": sum(
                1 for f in normal + bosses if f["drop_item"] is not None
            ),
            "unrunnable_formations": sum(
                1 for f in normal + bosses if not f["can_run"]
            ),
            "formation_ids_referenced": len({
                value for group in indexes["groups"] for value in group["formation_ids"]
            }),
            "maps_with_encounters": sum(
                1 for entry in bindings if entry["mode"] != "none"
            ),
            "table_covers_maps": encounters["table"]["count"],
            "map_ids": encounters["table"]["map_count"],
            "maps_outside_table": encounters["table"]["maps_outside_table"],
        },
    }


# ---------------------------------------------------------------------------
# Levels
# ---------------------------------------------------------------------------
def build_levels(rom: bytes) -> dict[str, Any]:
    progression = extract_level_progression(rom)
    characters = [
        {
            "character_id": character["character_id"],
            "character": character["character"],
            "starting_level": character["starting_level"],
            "table_offset": character["table_offset"],
            "record_count": character["record_count"],
            "levels": [
                {key: level[key] for key in
                 ("level", "experience_required", "hp", "tp", "stats",
                  "new_technique", "new_skill", "skill_uses")}
                for level in character["levels"]
            ],
        }
        for character in progression["characters"]
    ]
    return {
        "kind": "battle_levels",
        "character_count": len(characters),
        "total_records": progression["total_level_records"],
        "pointer_table": progression["pointer_table"],
        "characters": characters,
        "census": {
            "starting_levels": sorted({c["starting_level"] for c in characters}),
            "levels_per_character": _range(c["record_count"] for c in characters),
            "highest_level": max(
                level["level"] for c in characters for level in c["levels"]
            ),
            "characters_learning_techniques": sum(
                1 for c in characters
                if any(level["new_technique"] for level in c["levels"])
            ),
            "characters_learning_skills": sum(
                1 for c in characters
                if any(level["new_skill"] for level in c["levels"])
            ),
        },
    }


# ---------------------------------------------------------------------------
# Abilities
# ---------------------------------------------------------------------------
def _ability(record: dict[str, Any], kind: str, rom: bytes, effect_count: int,
             display: dict[int, str]) -> dict[str, Any]:
    """One 8-byte ability record, with its effect id checked against the table."""
    out = {key: value for key, value in record.items() if key != "raw_hex"}
    out["kind"] = kind
    out["display_name"] = display.get(record["id"])
    effect = record["effect_id"]
    if effect >= effect_count:
        out["effect_out_of_range"] = True
        out["effect_dispatch_target"] = _out_of_range_target(rom, effect)
    else:
        out["effect_out_of_range"] = False
    return out


def build_abilities(rom: bytes, effect_count: int,
                    display: dict[str, dict[int, str]]) -> dict[str, Any]:
    kinds = {
        "techniques": extract_techniques(rom),
        "skills": extract_skills(rom),
        "enemy_skills": extract_enemy_skills(rom),
    }
    abilities = {
        kind: [
            _ability(record, kind, rom, effect_count, display[kind])
            for record in records
        ]
        for kind, records in kinds.items()
    }
    # An item's battle behaviour is the same eight-byte record, embedded in its
    # 22-byte inventory entry, so the damage pipeline reads items through the
    # same path as everything else.
    abilities["item_effects"] = [
        {
            "id": item["id"],
            "symbol": item["symbol"],
            "display_name": display["items"].get(item["id"]),
            "rom_offset": item["rom_offset"],
            "kind": "item_effects",
            **{key: value for key, value in item["effect"].items()},
            "effect_out_of_range": item["effect"]["effect_id"] >= effect_count,
        }
        for item in extract_items(rom)
    ]
    for record in abilities["item_effects"]:
        if record["effect_out_of_range"]:
            record["effect_dispatch_target"] = _out_of_range_target(
                rom, record["effect_id"]
            )

    out_of_range = [
        {"kind": kind, "id": record["id"],
         "symbol": record.get("symbol") or record.get("name"),
         "display_name": record.get("display_name"),
         "effect_id": record["effect_id"],
         "effect_id_hex": f"0x{record['effect_id']:02X}",
         "dispatch_target": record["effect_dispatch_target"]}
        for kind in ABILITY_KINDS
        for record in abilities[kind]
        if record["effect_out_of_range"]
    ]
    effects_used = sorted({
        record["effect_id"] for kind in ABILITY_KINDS for record in abilities[kind]
    })
    return {
        "kind": "battle_abilities",
        "record_bytes": 8,
        "effects": {
            "table": f"0x{ABILITY_EFFECTS_OFFS:06X}",
            "label": "AbilityEffectsOffs",
            "count": effect_count,
            "bounds_checked": False,
            "note": (
                "TRAP #2 dispatches with `add.w d0,d0 / adda.w (a0,d0.w),a0 / "
                "jsr (a0)` and no bound, so an effect id past the table jumps "
                "wherever the following bytes point. Reject an ability whose "
                "effect_out_of_range is true rather than reproducing that."
            ),
        },
        **{kind: abilities[kind] for kind in ABILITY_KINDS},
        "counts": {kind: len(abilities[kind]) for kind in ABILITY_KINDS},
        "census": {
            # Observed, including the one past the table -- that is what the
            # data holds. The unused list is bounded by the table, so the two
            # do not add up to it, and that gap is the anomaly below.
            "effect_ids_used": effects_used,
            "effect_ids_in_table_unused": [
                value for value in range(effect_count) if value not in effects_used
            ],
            "effect_out_of_range": out_of_range,
            "elements_used": sorted({
                record["element"]["id"] for kind in ABILITY_KINDS
                for record in abilities[kind] if record.get("element")
            }),
            "tp_costs": _range(record["tp_cost"] for record in abilities["techniques"]),
            "skills_requiring_a_weapon": sum(
                1 for record in abilities["skills"] if record["requires_weapon"]
            ),
        },
    }


# ---------------------------------------------------------------------------
# Emission
# ---------------------------------------------------------------------------
def _write(directory: Path, name: str, payload: dict[str, Any], version: int) -> str:
    path = directory / name
    path.parent.mkdir(parents=True, exist_ok=True)
    text = json.dumps({"format_version": version, **payload}, indent=2, sort_keys=True) + "\n"
    data = text.encode("utf-8")
    path.write_bytes(data)
    return hashlib.sha256(data).hexdigest()


def emit_battle(rom: bytes, out_dir: str | Path, version: int) -> dict[str, Any]:
    """Write `battle/` and return the manifest fragment describing it.

    Deterministic like the rest of the pack: sorted keys, fixed indent, nothing
    derived from the filesystem or the clock.
    """
    directory = Path(out_dir)
    effect_count = ability_effect_count(rom)
    display = _display_names(rom)

    payloads = {
        ENEMIES_NAME: build_enemies(rom, effect_count, display["enemies"]),
        FORMATIONS_NAME: build_formations(rom),
        LEVELS_NAME: build_levels(rom),
        ABILITIES_NAME: build_abilities(rom, effect_count, display),
        CHARACTERS_NAME: build_characters(rom, display),
        EQUIPMENT_NAME: build_equipment(rom, display["items"]),
    }
    # The two record files cross-check each other: nothing a character starts
    # with may be something the equip filter would refuse them.
    refused = payloads[CHARACTERS_NAME]["census"]["equipment_the_owner_cannot_equip"]
    if refused:
        raise BattlePackError(
            f"initial equipment the equip filter refuses: {refused}"
        )
    shas = {
        name: _write(directory, name, payload, version)
        for name, payload in payloads.items()
    }

    enemies = payloads[ENEMIES_NAME]
    formations = payloads[FORMATIONS_NAME]
    levels = payloads[LEVELS_NAME]
    abilities = payloads[ABILITIES_NAME]
    characters = payloads[CHARACTERS_NAME]
    equipment = payloads[EQUIPMENT_NAME]
    return {
        "directory": BATTLE_DIRECTORY,
        "files": {
            "enemies": {"file": ENEMIES_NAME, "sha256": shas[ENEMIES_NAME],
                        "count": enemies["count"]},
            "formations": {"file": FORMATIONS_NAME, "sha256": shas[FORMATIONS_NAME],
                           "count": formations["formation_count"],
                           "boss_count": formations["boss_formation_count"],
                           "encounter_groups": formations["encounter_groups"]["group_count"]},
            "levels": {"file": LEVELS_NAME, "sha256": shas[LEVELS_NAME],
                       "characters": levels["character_count"],
                       "records": levels["total_records"]},
            "abilities": {"file": ABILITIES_NAME, "sha256": shas[ABILITIES_NAME],
                          **abilities["counts"]},
            "characters": {"file": CHARACTERS_NAME, "sha256": shas[CHARACTERS_NAME],
                           "count": characters["count"]},
            "equipment": {"file": EQUIPMENT_NAME, "sha256": shas[EQUIPMENT_NAME],
                          "count": equipment["count"],
                          "equippable": equipment["census"]["equippable"]},
        },
        "ability_effects": abilities["effects"],
        # A headline only; equipment.json carries the decoded rules in full.
        "equipment_rules": {
            "routines": {
                name: equipment["rules"][name]["routine"]
                for name in ("equip", "attack", "derived_stats", "elements")
            },
            "max_equippable_type": equipment["rules"]["equip"]["max_equippable_type"],
            "weapon_types": equipment["rules"]["attack"]["weapon_types"],
            "multi_target_types": equipment["rules"]["attack"]["multi_target_types"],
            "derived_stats": [
                entry["stat"] for entry in equipment["rules"]["derived_stats"]["passes"]
            ],
        },
        "census": {
            "enemies": enemies["census"],
            "formations": formations["census"],
            "levels": levels["census"],
            "abilities": abilities["census"],
            "characters": characters["census"],
            "equipment": equipment["census"],
        },
    }
