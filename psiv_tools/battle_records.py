"""The two record files a party is seated from.

`enemies.json` and `formations.json` describe what the party fights. These two
describe the party itself:

    characters.json   the eleven `InitialCharStats` records, plus the state
                      `InitializeCharStats` leaves in RAM once it has applied
                      one -- HP and TP mirrored into their maxima, equipment
                      summed into the derived stats, element resistances built
    equipment.json    all 160 `InventoryData` records in their battle-relevant
                      parts, and the cartridge rules that give the type byte,
                      the bonus bytes and the element byte their meaning

The record fields come from `psiv_tools.core`, which already proved them, and
the rules from `psiv_tools.battle_rules`, which decodes the five routines they
live in. This module only reshapes the two into the pack's files, the way
`psiv_tools.battle_pack` does for the other four.

The `initialized` block
-----------------------

Each character record carries what RAM holds *after* `InitializeCharStats` has
finished with it: base and modified stats, `atk_pow` / `dfs_pow` / `magic_dfs`,
the fourteen element properties and the two weapon-element slots. It is
derived, not stored -- the point is that a runtime implementing the two update
routines has a conformance vector per character to check itself against.
`docs/battle/BATTLE_SCOUT.md` section 12 works the first two by hand and gets Chaz
`atk_pow` 18 / `dfs_pow` 10 and Alys 13 / 18, which is what this emits.
"""

from __future__ import annotations

from typing import Any

from .battle_rules import (
    BONUS_FIELDS,
    EQUIPMENT_SLOTS,
    BattleRecordError,
    read_char_init,
    read_rules,
    type_rules,
)
from .core import (
    CHARACTER_NAMES,
    ELEMENTS,
    ITEM_TYPES,
    PROFESSIONS,
    PROPERTY_LEVELS,
    PROPERTY_NAMES,
    SKILL_NAMES,
    TABLES,
    TECHNIQUE_NAMES,
    extract_characters,
    extract_items,
)

# ---------------------------------------------------------------------------
# Equipment
# ---------------------------------------------------------------------------
def _characters(mask: int) -> list[dict[str, Any]]:
    return [
        {"character_id": index, "symbol": name}
        for index, name in enumerate(CHARACTER_NAMES)
        if mask & (1 << index)
    ]


def build_equipment(rom: bytes, display: dict[int, str]) -> dict[str, Any]:
    """`equipment.json`: every inventory record's battle-relevant half."""
    rules = read_rules(rom)
    types = type_rules(rules)
    records = extract_items(rom)
    spec = TABLES["items"]

    items = []
    for record in records:
        item_type = record["type"]["id"]
        rule = types.get(item_type)
        if rule is None:
            raise BattleRecordError(
                f"item {record['id']} has type {item_type}, which no rule covers"
            )
        mask = int(record["used_by_mask"], 16)
        items.append({
            "id": record["id"],
            "symbol": record["symbol"],
            "display_name": display.get(record["id"]),
            "rom_offset": record["rom_offset"],
            "type": {"id": item_type, "name": rule["name"]},
            "equippable": rule["equippable"],
            "slot": rule["slot"],
            "is_weapon": rule["is_weapon"],
            "multi_target": rule["multi_target"],
            "two_handed": rule.get("two_handed", False),
            "equippable_by_mask": record["used_by_mask"],
            "equippable_by": _characters(mask),
            "bonuses": record["bonuses"],
            "element": {
                **record["attack_defense_element"],
                "role": rule["element_role"],
            },
            "post_attack_effect_id": record["post_attack_effect_id"],
            "meseta_cost": record["meseta_cost"],
            "raw_hex": record["raw_hex"],
        })

    return {
        "kind": "battle_equipment",
        "count": len(items),
        "source": {
            "label": "InventoryData",
            "rom_offset": records[0]["rom_offset"],
            "record_bytes": spec["record_size"],
            "fields": {
                "usable_by": rules["equip"]["usable_by_offset"],
                "type": rules["equip"]["type_offset"],
                "bonuses": "0x0B-0x11",
                "element": rules["elements"]["element_offset"],
                "post_attack_effect": "0x13",
            },
            "note": (
                "bytes 0x00-0x07 are the item's battle effect and are emitted "
                "in abilities.json as item_effects; bytes 0x14-0x15 are the "
                "shop price, which the shops section carries"
            ),
        },
        "rules": rules,
        "types": [types[key] for key in sorted(types)],
        "items": items,
        "census": _equipment_census(items, rules),
    }


def _equipment_census(items: list[dict[str, Any]],
                      rules: dict[str, Any]) -> dict[str, Any]:
    counts: dict[int, int] = {}
    for item in items:
        counts[item["type"]["id"]] = counts.get(item["type"]["id"], 0) + 1
    bonus_fields = sorted(BONUS_FIELDS.values())
    max_type = rules["equip"]["max_equippable_type"]
    return {
        "types_present": sorted(counts),
        "type_distribution": {str(key): counts[key] for key in sorted(counts)},
        "types_declared_unused": [
            key for key in ITEM_TYPES if key not in counts
        ],
        "equippable": sum(1 for item in items if item["equippable"]),
        "weapons": sum(1 for item in items if item["is_weapon"]),
        "multi_target_weapons": sum(1 for item in items if item["multi_target"]),
        "two_handed_weapons": sum(1 for item in items if item["two_handed"]),
        "with_element": sum(1 for item in items if item["element"]["id"]),
        "elements_used": sorted({
            item["element"]["id"] for item in items if item["element"]["id"]
        }),
        "with_post_attack_effect": sum(
            1 for item in items if item["post_attack_effect_id"]
        ),
        "with_any_bonus": sum(
            1 for item in items if any(item["bonuses"].values())
        ),
        "bonus_range": {
            field: {
                "min": min(item["bonuses"][field] for item in items),
                "max": max(item["bonuses"][field] for item in items),
            }
            for field in bonus_fields
        },
        "equippable_by_character": {
            name: sum(
                1 for item in items
                if item["equippable"]
                and any(who["character_id"] == index for who in item["equippable_by"])
            )
            for index, name in enumerate(CHARACTER_NAMES)
        },
        # The equip list is filtered on the type byte first, so a mask on an
        # unequippable record can never be reached. One retail record has one.
        "equip_mask_on_unequippable": [
            {"id": item["id"], "symbol": item["symbol"],
             "display_name": item["display_name"], "type": item["type"]["id"],
             "mask": item["equippable_by_mask"]}
            for item in items
            if not item["equippable"] and item["equippable_by_mask"] != "0x0000"
        ],
        "equippable_by_nobody": [
            item["id"] for item in items
            if item["equippable"] and item["equippable_by_mask"] == "0x0000"
        ],
        "mask_bits_above_party": [
            item["id"] for item in items
            if int(item["equippable_by_mask"], 16) >> len(CHARACTER_NAMES)
        ],
        "max_equippable_type": max_type,
    }


# ---------------------------------------------------------------------------
# Characters
# ---------------------------------------------------------------------------
def _slots(raw: bytes, names: list[str]) -> list[dict[str, Any]]:
    """A positional ability array, with the ids the cartridge stores resolved."""
    out = []
    for slot, value in enumerate(raw):
        if not value:
            continue
        index = value - 1
        out.append({
            "slot": slot,
            "id": value,
            "symbol": names[index] if 0 <= index < len(names) else None,
        })
    return out


def build_characters(rom: bytes, display: dict[str, dict[int, str]]) -> dict[str, Any]:
    """`characters.json`: the eleven records, and the RAM they produce."""
    init = read_char_init(rom)
    rules = read_rules(rom)
    types = type_rules(rules)
    items = {record["id"]: record for record in extract_items(rom)}
    records = extract_characters(rom)

    characters = []
    for record in records:
        raw = bytes.fromhex(record["raw_hex"])
        techniques = _slots(raw[34:50], TECHNIQUE_NAMES)
        skills = _slots(raw[50:58], SKILL_NAMES)
        uses = raw[58:66]
        for entry in skills:
            entry["uses"] = uses[entry["slot"]]
            entry["max_uses"] = uses[entry["slot"]]
        for entry in techniques:
            entry["display_name"] = display["techniques"].get(entry["id"])
        for entry in skills:
            entry["display_name"] = display["skills"].get(entry["id"])

        equipment = {}
        for slot in EQUIPMENT_SLOTS:
            item_id = record["equipment_ids"][slot]
            if not item_id:
                equipment[slot] = None
                continue
            item = items.get(item_id)
            if item is None:
                raise BattleRecordError(
                    f"character {record['id']} equips item {item_id}, which has no record"
                )
            equipment[slot] = {
                "item_id": item_id,
                "symbol": item["symbol"],
                "display_name": display["items"].get(item_id),
                "type": item["type"]["id"],
            }

        profession = record["profession"]["id"]
        characters.append({
            "character_id": record["id"],
            "symbol": record["name"],
            "display_name": display["characters"].get(record["id"]),
            "rom_offset": record["rom_offset"],
            "profession": {
                "id": profession,
                "symbol": PROFESSIONS[profession],
                "display_name": display["professions"].get(profession),
            },
            "level": record["level"],
            "experience": record["experience"],
            "hp": record["hp"],
            "max_hp": record["hp"],
            "tp": record["tp"],
            "max_tp": record["tp"],
            "stats": record["stats"],
            "properties": record["properties"],
            "equipment": equipment,
            "techniques": {
                "slots": list(raw[34:50]),
                "known": techniques,
            },
            "skills": {
                "slots": list(raw[50:58]),
                "uses": list(uses),
                "known": skills,
            },
            "initialized": _initialize(record, equipment, items, types, rules),
            "raw_hex": record["raw_hex"],
        })

    return {
        "kind": "battle_characters",
        "count": len(characters),
        "source": {
            "label": "InitialCharStats",
            "rom_offset": f"0x{init['table']:06X}",
            "record_bytes": init["record_bytes"],
            "applied_by": {
                "label": "InitializeCharStats",
                "rom_offset": init["routine"],
                "character_stats_ram": init["character_stats_ram"],
                "struct_bytes": init["struct_bytes"],
                "note": (
                    "runs once for all eleven characters whether or not they are "
                    "in the party, and finishes each with UpdateCharModStats and "
                    "UpdateCharElems"
                ),
            },
            "record_layout": init["fields"],
        },
        "characters": characters,
        "census": _character_census(characters, types, items),
    }


def _initialize(record: dict[str, Any], equipment: dict[str, Any],
                items: dict[int, dict[str, Any]], types: dict[int, dict[str, Any]],
                rules: dict[str, Any]) -> dict[str, Any]:
    """What `InitializeCharStats` leaves in RAM for one character.

    A conformance vector, not source data: it is `UpdateCharModStats` and
    `UpdateCharElems` run over this record with the rules decoded above, so an
    engine that implements them has something per character to check against.
    """
    equipped = [
        items[slot["item_id"]] for slot in equipment.values() if slot is not None
    ]
    stats = {**record["stats"]}
    derived: dict[str, Any] = {}
    for entry in rules["derived_stats"]["passes"]:
        base = stats[entry["base_stat"]]
        total = base
        for item in equipped:
            for field in entry["bonuses"]:
                total += item["bonuses"][field]
        if entry["result_bytes"] == 1:
            # `add.b` into a byte register: the sum wraps rather than saturating.
            derived[entry["stat"]] = total & 0xFF
        else:
            derived[entry["stat"]] = total & 0xFFFF

    elements = rules["elements"]
    props = {name: 0 for name in PROPERTY_NAMES}
    weapon_elements: dict[str, int] = {}
    for slot in EQUIPMENT_SLOTS:
        entry = equipment[slot]
        if entry is None:
            if slot in ("right_hand", "left_hand"):
                weapon_elements[slot] = 0
            continue
        item = items[entry["item_id"]]
        element = item["attack_defense_element"]["id"]
        is_hand = slot in ("right_hand", "left_hand")
        if is_hand and entry["type"] != elements["shield_type"]:
            weapon_elements[slot] = element
            continue
        if is_hand:
            weapon_elements[slot] = 0
        if element:
            props[PROPERTY_NAMES[element - 1]] = elements["resistance_value"]
    for index, name in enumerate(PROPERTY_NAMES):
        if not props[name]:
            props[name] = record["properties"][name]["value"]

    return {
        "stats": {
            name: {"base": value, "mod": derived[f"{name}_mod"]}
            for name, value in stats.items()
        },
        **{key: value for key, value in derived.items() if not key.endswith("_mod")},
        "element_props": {
            name: {"value": value, "meaning": PROPERTY_LEVELS.get(value)}
            for name, value in props.items()
        },
        "weapon_elements": {
            slot: {"id": value, "name": ELEMENTS.get(value)}
            for slot, value in sorted(weapon_elements.items())
        },
    }


def _character_census(characters: list[dict[str, Any]],
                      types: dict[int, dict[str, Any]],
                      items: dict[int, dict[str, Any]]) -> dict[str, Any]:
    def filled(character: dict[str, Any]) -> int:
        return sum(1 for slot in character["equipment"].values() if slot is not None)

    refused = []
    downgraded = []
    for character in characters:
        for slot, entry in character["equipment"].items():
            if entry is None:
                continue
            item = items[entry["item_id"]]
            mask = int(item["used_by_mask"], 16)
            if not types[entry["type"]]["equippable"]:
                reason = "type is not equippable"
            elif not mask & (1 << character["character_id"]):
                reason = "usable_by does not include this character"
            elif types[entry["type"]]["slot"] != slot and not (
                slot == "left_hand" and types[entry["type"]]["is_weapon"]
            ):
                reason = f"the equip menu never places this type in {slot}"
            else:
                continue
            refused.append({
                "character_id": character["character_id"], "slot": slot,
                "item_id": entry["item_id"], "reason": reason,
            })
        # An item's element grant is written unconditionally, so armour can
        # replace an immunity (0) with mere resistance (1).
        for name, prop in character["initialized"]["element_props"].items():
            before = character["properties"][name]["value"]
            if prop["value"] > before:
                downgraded.append({
                    "character_id": character["character_id"], "element": name,
                    "record_value": before, "initialized_value": prop["value"],
                })

    left_hand_weapons = [
        character["character_id"] for character in characters
        if character["equipment"]["left_hand"]
        and types[character["equipment"]["left_hand"]["type"]]["is_weapon"]
    ]
    two_handed_conflicts = [
        character["character_id"] for character in characters
        if character["equipment"]["right_hand"]
        and types[character["equipment"]["right_hand"]["type"]].get("two_handed")
        and character["equipment"]["left_hand"]
    ]
    return {
        "professions_used": sorted({
            character["profession"]["id"] for character in characters
        }),
        "starting_levels": sorted({character["level"] for character in characters}),
        "hp": {
            "min": min(character["hp"] for character in characters),
            "max": max(character["hp"] for character in characters),
        },
        "tp": {
            "min": min(character["tp"] for character in characters),
            "max": max(character["tp"] for character in characters),
        },
        "with_techniques": sum(
            1 for character in characters if character["techniques"]["known"]
        ),
        "with_skills": sum(
            1 for character in characters if character["skills"]["known"]
        ),
        "techniques_known": {
            "min": min(len(c["techniques"]["known"]) for c in characters),
            "max": max(len(c["techniques"]["known"]) for c in characters),
        },
        "skills_known": {
            "min": min(len(c["skills"]["known"]) for c in characters),
            "max": max(len(c["skills"]["known"]) for c in characters),
        },
        "equipment_slots_filled": {
            "min": min(filled(character) for character in characters),
            "max": max(filled(character) for character in characters),
        },
        "starting_with_experience": [
            character["character_id"] for character in characters
            if character["experience"]
        ],
        # Every initial item passes both halves of the equip filter, so nothing
        # a character starts with is something they could not re-equip -- with
        # one structural exception, below.
        "equipment_the_owner_cannot_equip": refused,
        "element_props_weakened_by_equipment": downgraded,
        # `EquipItemType_OneHanded` always writes the right hand, and only a
        # shield is ever placed in the left. A character who starts with a
        # weapon there can never put one back once it is removed.
        "weapon_in_left_hand": left_hand_weapons,
        "two_handed_with_left_hand_occupied": two_handed_conflicts,
    }
