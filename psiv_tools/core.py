from __future__ import annotations

import hashlib
import json
import zlib
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .formations import extract_formation_indexes, extract_formations
from .symbols import ENEMY_SKILL_SYMBOLS, ENEMY_SYMBOLS, ITEM_SYMBOLS

EXPECTED_SHA256 = "511f35cc11f88316f8b8940e28ab298bd75a4da193672a80172884d6eb913b6a"
EXPECTED_SIZE = 3_145_728
EXPECTED_INTERNAL_CHECKSUM = 0x05CB

TABLES = {
    "characters": {"offset": 0x2A8ACA, "record_size": 66, "count": 11},
    "techniques": {"offset": 0x2A9BE8, "record_size": 8, "count": 40},
    "skills": {"offset": 0x2A9D28, "record_size": 8, "count": 54},
    "combos": {"offset": 0x285424, "record_size": 8, "count": 15},
    "vehicles": {"offset": 0x2855E2, "record_size": 26, "count": 3},
    "enemies": {"offset": 0x2816BC, "record_size": 48, "count": 153},
    "enemy_skills": {"offset": 0x28336C, "record_size": 8, "count": 112},
    "items": {"offset": 0x2A8E28, "record_size": 22, "count": 160},
    "level_pointers": {"offset": 0x004074, "record_size": 6, "count": 11},
}

CHARACTER_NAMES = [
    "Chaz", "Alys", "Hahn", "Rune", "Gryz", "Rika",
    "Demi", "Wren", "Raja", "Kyra", "Seth",
]
PROFESSIONS = ["Hunter", "Scholar", "Wizard", "Motavian", "Numan", "Android", "Priest", "Esper"]
PROPERTY_NAMES = [
    "physical", "energy", "fire", "gravity", "water", "anti_evil", "electric",
    "holyword", "brose", "biological", "psychic", "mechanical", "efess", "destroy",
]
PROPERTY_LEVELS = {0: "immune", 1: "resistant", 2: "normal", 3: "weak", 4: "very_weak"}

TECHNIQUE_NAMES = [
    "Foi", "Gifoi", "Nafoi", "Wat", "Giwat", "Nawat", "Tsu", "Githu", "Nathu", "Zan",
    "Gizan", "Nazan", "Gra", "Gigra", "Nagra", "Megid", "Brose", "Vol", "Savol", "Gelun",
    "Doran", "Seals", "Rimit", "Res", "Gires", "Nares", "Sar", "Gisar", "Nasar", "Shift",
    "Saner", "Deban", "Feeve", "Anti", "Rimpa", "Rever", "Regen", "Arows", "Ryuka", "Hinas",
]
SKILL_NAMES = [
    "Crosscut", "Rayblade", "DblSlash", "Flaeli", "Flare", "Vortex", "Astral", "Airslash",
    "Disrupt", "Hewn", "Tandle", "Efess", "Legeon", "Burstroc", "Posibolt", "Sweeping",
    "Phonon", "StFire", "Corrsion", "Explode", "Eliminat", "Diem", "Spark", "Death",
    "Holyword", "Dthspell", "Negatis", "Illusion", "Telele", "Shadow", "Earth", "Hijammer",
    "Moonshad", "Crash", "StasisBm", "Bindwa", "Mindblst", "Barrier", "WarCry", "Blessing",
    "Warla", "Recover", "Medice", "Miracle", "MedicPw", "Ataraxia", "Vision", "Eliminat2",
    "Flaeli2", "Hewn2", "Tandle2", "Spark2", "Barrier2", "Recover2",
]
COMBO_NAMES = [
    "None", "Paradinblw", "Firestorm", "Blizzard", "Condctthnd", "Grandcross", "Silentwave",
    "Shootnstar", "Triblaster", "Holocaust", "BlackHole", "Circuitbrk", "Purfylight",
    "LethalImg", "Destruct",
]
VEHICLE_NAMES = ["LandRover", "IceDigger", "Hydrofoil"]

ITEM_TYPES = {
    0: "none_or_unused",
    1: "one_handed_single_target_weapon",
    2: "one_handed_multi_target_weapon",
    3: "two_handed_single_target_weapon",
    4: "two_handed_multi_target_weapon",
    5: "shield",
    6: "headwear_or_ring",
    7: "body_equipment",
    8: "disposable_item",
    9: "plot_item",
    10: "field_only_item",
}

BASIC_ATTACK_STATUS = {0x00: "none", 0x1B: "poison", 0x1C: "paralyze"}

TARGETS = {
    1: "single_enemy", 2: "all_enemies", 3: "self", 4: "single_human", 5: "all_humans",
    6: "single_android", 7: "all_androids", 8: "single_character", 9: "all_characters",
}
USABILITY = {1: "battle_only", 2: "field_only", 3: "battle_and_field"}
STATS = {0: "none", 1: "strength", 2: "mental", 3: "agility", 4: "dexterity", 5: "attack", 6: "defense", 7: "magic_defense"}
ELEMENTS = {
    0: "none", 1: "physical", 2: "energy", 3: "fire", 4: "gravity", 5: "water_ice",
    6: "anti_evil", 7: "electric", 8: "holyword", 9: "brose", 10: "biological",
    11: "psychic", 12: "mechanical", 13: "efess", 14: "destroy",
}


class RomError(ValueError):
    pass


def _ascii(data: bytes) -> str:
    return data.decode("ascii", errors="replace").rstrip(" \x00")


def be16(data: bytes, offset: int = 0) -> int:
    return int.from_bytes(data[offset:offset + 2], "big")


def be32(data: bytes, offset: int = 0) -> int:
    return int.from_bytes(data[offset:offset + 4], "big")


def i8(value: int) -> int:
    return value - 256 if value >= 128 else value


def genesis_checksum(data: bytes) -> int:
    total = 0
    payload = data[0x200:]
    for i in range(0, len(payload), 2):
        word = payload[i] << 8
        if i + 1 < len(payload):
            word |= payload[i + 1]
        total = (total + word) & 0xFFFF
    return total


def hash_info(data: bytes) -> dict[str, str]:
    return {
        "crc32": f"{zlib.crc32(data) & 0xFFFFFFFF:08x}",
        "md5": hashlib.md5(data).hexdigest(),
        "sha1": hashlib.sha1(data).hexdigest(),
        "sha256": hashlib.sha256(data).hexdigest(),
    }


def read_rom(path: str | Path, *, require_known_build: bool = True) -> bytes:
    path = Path(path)
    data = path.read_bytes()
    if len(data) < 0x200:
        raise RomError("File is too small to contain a Mega Drive/Genesis header")
    if require_known_build:
        sha = hashlib.sha256(data).hexdigest()
        if sha != EXPECTED_SHA256:
            raise RomError(
                "Unsupported ROM build. This PoC intentionally accepts only the verified US retail image "
                f"({EXPECTED_SHA256}); got {sha}."
            )
    return data


def inspect_rom(data: bytes) -> dict[str, Any]:
    hashes = hash_info(data)
    stored_checksum = be16(data, 0x18E)
    calculated_checksum = genesis_checksum(data)
    return {
        "format": "Sega Mega Drive / Genesis ROM",
        "size_bytes": len(data),
        "size_mib": len(data) / (1024 * 1024),
        "console": _ascii(data[0x100:0x110]),
        "copyright": _ascii(data[0x110:0x120]),
        "domestic_title": _ascii(data[0x120:0x150]),
        "international_title": _ascii(data[0x150:0x180]),
        "product_code": _ascii(data[0x180:0x18E]),
        "io_support": _ascii(data[0x190:0x1A0]),
        "rom_start": be32(data, 0x1A0),
        "rom_end": be32(data, 0x1A4),
        "ram_start": be32(data, 0x1A8),
        "ram_end": be32(data, 0x1AC),
        "sram_id": data[0x1B0:0x1B4].hex(),
        "sram_start": be32(data, 0x1B4),
        "sram_end": be32(data, 0x1B8),
        "region": _ascii(data[0x1F0:0x200]),
        "initial_stack_pointer": be32(data, 0x000),
        "reset_vector": be32(data, 0x004),
        "stored_checksum": f"{stored_checksum:04x}",
        "calculated_checksum": f"{calculated_checksum:04x}",
        "checksum_ok": stored_checksum == calculated_checksum,
        "known_build": hashes["sha256"] == EXPECTED_SHA256,
        "hashes": hashes,
    }


def _named_id(value: int, names: list[str]) -> dict[str, Any] | None:
    if value == 0:
        return None
    idx = value - 1
    return {"id": value, "name": names[idx] if 0 <= idx < len(names) else None}


def _named_symbol_id(value: int, symbols: list[str]) -> dict[str, Any] | None:
    if value == 0:
        return None
    idx = value - 1
    return {"id": value, "symbol": symbols[idx] if 0 <= idx < len(symbols) else None}


def extract_characters(data: bytes) -> list[dict[str, Any]]:
    spec = TABLES["characters"]
    out = []
    for idx, name in enumerate(CHARACTER_NAMES):
        offset = spec["offset"] + idx * spec["record_size"]
        r = data[offset:offset + spec["record_size"]]
        if len(r) != 66:
            raise RomError(f"Character record {idx} runs past end of ROM")

        profession_id = be16(r, 0)
        properties_raw = list(r[16:30])
        properties = {
            prop: {"value": value, "meaning": PROPERTY_LEVELS.get(value)}
            for prop, value in zip(PROPERTY_NAMES, properties_raw)
        }
        techs = [_named_id(v, TECHNIQUE_NAMES) for v in r[34:50]]
        skills = []
        for slot, (skill_id, uses) in enumerate(zip(r[50:58], r[58:66])):
            if skill_id:
                named = _named_id(skill_id, SKILL_NAMES) or {"id": skill_id, "name": None}
                skills.append({"slot": slot, **named, "initial_uses": uses})

        out.append({
            "id": idx,
            "name": name,
            "rom_offset": f"0x{offset:06X}",
            "profession": {
                "id": profession_id,
                "name": PROFESSIONS[profession_id] if 0 <= profession_id < len(PROFESSIONS) else None,
            },
            "level": be16(r, 2),
            "experience": be32(r, 4),
            "hp": be16(r, 8),
            "tp": be16(r, 10),
            "stats": {
                "strength": r[12], "mental": r[13], "agility": r[14], "dexterity": r[15],
            },
            "properties": properties,
            "equipment_ids": {
                "right_hand": r[30], "left_hand": r[31], "head": r[32], "body": r[33],
            },
            "initial_techniques": [x for x in techs if x is not None],
            "initial_skills": skills,
            "raw_hex": r.hex(),
        })
    return out


def _decode_targeting(value: int) -> dict[str, Any]:
    target = value & 0x0F
    usability = (value >> 4) & 0x0F
    return {
        "raw": value,
        "target_id": target,
        "target": TARGETS.get(target),
        "usability_id": usability,
        "usability": USABILITY.get(usability),
    }


def _ability_common(r: bytes) -> dict[str, Any]:
    return {
        "effect_id": r[0],
        "targeting": _decode_targeting(r[2]),
        "power_or_hit_chance": r[3],
        "resistance_stat": {"id": r[4], "name": STATS.get(r[4])},
        "element": {"id": r[5], "name": ELEMENTS.get(r[5])},
        "reserved": [r[6], r[7]],
        "raw_hex": r.hex(),
    }


def extract_techniques(data: bytes) -> list[dict[str, Any]]:
    spec = TABLES["techniques"]
    out = []
    for idx, name in enumerate(TECHNIQUE_NAMES, start=1):
        offset = spec["offset"] + (idx - 1) * 8
        r = data[offset:offset + 8]
        entry = {
            "id": idx,
            "name": name,
            "rom_offset": f"0x{offset:06X}",
            "tp_cost": r[1],
            **_ability_common(r),
        }
        out.append(entry)
    return out


def extract_skills(data: bytes) -> list[dict[str, Any]]:
    spec = TABLES["skills"]
    out = []
    for idx, name in enumerate(SKILL_NAMES, start=1):
        offset = spec["offset"] + (idx - 1) * 8
        r = data[offset:offset + 8]
        stat_id = r[1] & 0x7F
        entry = {
            "id": idx,
            "name": name,
            "rom_offset": f"0x{offset:06X}",
            "relevant_stat": {"id": stat_id, "name": STATS.get(stat_id)},
            "requires_weapon": bool(r[1] & 0x80),
            **_ability_common(r),
        }
        out.append(entry)
    return out


def extract_combos(data: bytes) -> list[dict[str, Any]]:
    spec = TABLES["combos"]
    out = []
    for idx, name in enumerate(COMBO_NAMES):
        offset = spec["offset"] + idx * 8
        r = data[offset:offset + 8]
        out.append({
            "id": idx,
            "name": name,
            "rom_offset": f"0x{offset:06X}",
            "secondary_value": r[1],
            **_ability_common(r),
        })
    return out


def extract_vehicles(data: bytes) -> list[dict[str, Any]]:
    spec = TABLES["vehicles"]
    out = []
    for idx, name in enumerate(VEHICLE_NAMES, start=1):
        offset = spec["offset"] + (idx - 1) * spec["record_size"]
        r = data[offset:offset + spec["record_size"]]
        props = {
            prop: {"value": value, "meaning": PROPERTY_LEVELS.get(value)}
            for prop, value in zip(PROPERTY_NAMES, r[9:23])
        }
        out.append({
            "id": idx,
            "name": name,
            "rom_offset": f"0x{offset:06X}",
            "hp": be16(r, 0),
            "stats": {"strength": r[2], "mental": r[3], "agility": r[4], "dexterity": r[5]},
            "attack": r[6],
            "defense": r[7],
            "magic_defense": r[8],
            "properties": props,
            "option_bitfield": r[23],
            "option_uses": [r[24], r[25]],
            "raw_hex": r.hex(),
        })
    return out


def extract_items(data: bytes) -> list[dict[str, Any]]:
    spec = TABLES["items"]
    out = []
    for idx, symbol in enumerate(ITEM_SYMBOLS, start=1):
        offset = spec["offset"] + (idx - 1) * spec["record_size"]
        r = data[offset:offset + spec["record_size"]]
        if len(r) != spec["record_size"]:
            raise RomError(f"Item record {idx} runs past end of ROM")

        used_by_mask = be16(r, 8)
        used_by = [
            {"id": char_id, "name": name}
            for char_id, name in enumerate(CHARACTER_NAMES)
            if used_by_mask & (1 << char_id)
        ]
        item_type = r[10]
        out.append({
            "id": idx,
            "symbol": symbol,
            "rom_offset": f"0x{offset:06X}",
            "effect": {
                "effect_id": r[0],
                # The public disassembly does not give every one of bytes 2-8 a
                # universally safe semantic for every item class. Preserve them
                # explicitly rather than inventing meaning.
                "parameter_2": r[1],
                "targeting_or_parameter_3": r[2],
                "power_or_hit_chance": r[3],
                "resistance_stat": {"id": r[4], "name": STATS.get(r[4])},
                "element": {"id": r[5], "name": ELEMENTS.get(r[5])},
                "reserved_7": r[6],
                "battle_object_or_graphic_id": r[7],
            },
            "used_by_mask": f"0x{used_by_mask:04X}",
            "used_by": used_by,
            "type": {"id": item_type, "name": ITEM_TYPES.get(item_type)},
            "bonuses": {
                "strength": i8(r[11]),
                "mental": i8(r[12]),
                "agility": i8(r[13]),
                "dexterity": i8(r[14]),
                "attack": i8(r[15]),
                "defense": i8(r[16]),
                "magic_defense": i8(r[17]),
            },
            "attack_defense_element": {"id": r[18], "name": ELEMENTS.get(r[18])},
            "post_attack_effect_id": r[19],
            "meseta_cost": be16(r, 20),
            "raw_hex": r.hex(),
        })
    return out


def extract_enemy_skills(data: bytes) -> list[dict[str, Any]]:
    spec = TABLES["enemy_skills"]
    out = []
    for idx, symbol in enumerate(ENEMY_SKILL_SYMBOLS, start=1):
        offset = spec["offset"] + (idx - 1) * spec["record_size"]
        r = data[offset:offset + spec["record_size"]]
        if len(r) != spec["record_size"]:
            raise RomError(f"Enemy skill record {idx} runs past end of ROM")
        out.append({
            "id": idx,
            "symbol": symbol,
            "rom_offset": f"0x{offset:06X}",
            "effect_id": r[0],
            "relevant_stat": {"id": r[1], "name": STATS.get(r[1])},
            "target_id": r[2],
            "power_or_hit_chance": r[3],
            "resistance_stat": {"id": r[4], "name": STATS.get(r[4])},
            "element": {"id": r[5], "name": ELEMENTS.get(r[5])},
            "reserved": [r[6], r[7]],
            "raw_hex": r.hex(),
        })
    return out


def extract_enemies(data: bytes) -> list[dict[str, Any]]:
    spec = TABLES["enemies"]
    out = []
    for idx, symbol in enumerate(ENEMY_SYMBOLS):
        offset = spec["offset"] + idx * spec["record_size"]
        r = data[offset:offset + spec["record_size"]]
        if len(r) != spec["record_size"]:
            raise RomError(f"Enemy record {idx} runs past end of ROM")

        props = {
            prop: {"value": value, "meaning": PROPERTY_LEVELS.get(value)}
            for prop, value in zip(PROPERTY_NAMES, r[12:26])
        }
        out.append({
            "id": idx,
            "symbol": symbol,
            "rom_offset": f"0x{offset:06X}",
            "hp": be16(r, 0),
            "basic_attack": {
                "element": {"id": r[2], "name": ELEMENTS.get(r[2])},
                "status_effect": {"id": r[3], "name": BASIC_ATTACK_STATUS.get(r[3])},
            },
            "stats": {
                "strength": r[4],
                "mental": r[5],
                "agility": r[6],
                "dexterity": r[7],
                "attack": be16(r, 8),
                "defense": r[10],
                "magic_defense": r[11],
            },
            "properties": props,
            "reserved_27_28": list(r[26:28]),
            "ai": {
                "condition_ids": list(r[28:32]),
                "conditional_ability_ids": list(r[32:36]),
                "conditional_abilities": [
                    _named_symbol_id(v, ENEMY_SKILL_SYMBOLS) for v in r[32:36]
                ],
                "regular_ability_ids": list(r[36:44]),
                "regular_abilities": [
                    _named_symbol_id(v, ENEMY_SKILL_SYMBOLS) for v in r[36:44]
                ],
            },
            "experience_reward": be16(r, 44),
            "meseta_reward": be16(r, 46),
            "raw_hex": r.hex(),
        })
    return out


def extract_level_progression(data: bytes) -> dict[str, Any]:
    spec = TABLES["level_pointers"]
    pointer_records = []
    characters = []

    for idx, name in enumerate(CHARACTER_NAMES):
        p_off = spec["offset"] + idx * spec["record_size"]
        starting_level = be16(data, p_off)
        table_offset = be32(data, p_off + 2)
        record_count = 99 - starting_level
        pointer_records.append({
            "character_id": idx,
            "character": name,
            "rom_offset": f"0x{p_off:06X}",
            "starting_level": starting_level,
            "table_offset": f"0x{table_offset:06X}",
            "record_count": record_count,
            "raw_hex": data[p_off:p_off + 6].hex(),
        })

        levels = []
        for rec_idx in range(record_count):
            offset = table_offset + rec_idx * 22
            r = data[offset:offset + 22]
            if len(r) != 22:
                raise RomError(f"Level record for {name} runs past end of ROM")
            level = starting_level + rec_idx + 1
            tech_id = r[12]
            skill_id = r[13]
            levels.append({
                "level": level,
                "rom_offset": f"0x{offset:06X}",
                "experience_required": be32(r, 0),
                "hp": be16(r, 4),
                "tp": be16(r, 6),
                "stats": {
                    "strength": r[8],
                    "mental": r[9],
                    "agility": r[10],
                    "dexterity": r[11],
                },
                "new_technique": _named_id(tech_id, TECHNIQUE_NAMES),
                "new_skill": _named_id(skill_id, SKILL_NAMES),
                "skill_uses": list(r[14:22]),
                "raw_hex": r.hex(),
            })

        characters.append({
            "character_id": idx,
            "character": name,
            "starting_level": starting_level,
            "table_offset": f"0x{table_offset:06X}",
            "record_count": record_count,
            "levels": levels,
        })

    primary_start = be32(data, spec["offset"] + 2)
    last_starting_level = pointer_records[-1]["starting_level"]
    last_table = int(pointer_records[-1]["table_offset"], 16)
    primary_end = last_table + (99 - last_starting_level) * 22
    primary = data[primary_start:primary_end]
    mirror_start = 0x2A3A42
    mirror = data[mirror_start:mirror_start + len(primary)]

    return {
        "pointer_table": {
            "rom_offset": f"0x{spec['offset']:06X}",
            "record_size": spec["record_size"],
            "count": spec["count"],
            "records": pointer_records,
        },
        "table_block": {
            "primary_start": f"0x{primary_start:06X}",
            "primary_end_exclusive": f"0x{primary_end:06X}",
            "size_bytes": len(primary),
            "mirror_start": f"0x{mirror_start:06X}",
            "mirror_end_exclusive": f"0x{mirror_start + len(primary):06X}",
            "mirror_is_exact": primary == mirror,
        },
        "total_level_records": sum(c["record_count"] for c in characters),
        "characters": characters,
    }


def validate_known_layout(data: bytes) -> list[dict[str, Any]]:
    checks = [
        ("Chaz character record", 0x2A8ACA, bytes.fromhex("00000001000000000019000a08060705")),
        ("Foi technique record", 0x2A9BE8, bytes.fromhex("0103111807030000")),
        ("Crosscut skill record", 0x2A9D28, bytes.fromhex("0185115006100000")),
        ("Paradinblw combo record", 0x28542C, bytes.fromhex("0100010007060000")),
        ("LandRover vehicle prefix", 0x2855E2, bytes.fromhex("02e440003c46c85032")),
        ("Helex enemy prefix", 0x2816BC, bytes.fromhex("005a01000a072d2200a0")),
        ("Enemy skill table start", 0x28336C, bytes.fromhex("0d00030100000000")),
        ("Dagger item record", 0x2A8E28, bytes.fromhex("00000000000000000405010000000002000001000028")),
        ("Character level pointer table", 0x004074, bytes.fromhex("0001002856b0000700285f1c")),
        ("Chaz level 2 record", 0x2856B0, bytes.fromhex("00000015001f000d0907080600000400000000000000")),
    ]
    results = []
    for label, offset, expected in checks:
        actual = data[offset:offset + len(expected)]
        results.append({
            "label": label,
            "offset": f"0x{offset:06X}",
            "ok": actual == expected,
            "expected_hex": expected.hex(),
            "actual_hex": actual.hex(),
        })
    return results


def extract_all(data: bytes) -> dict[str, Any]:
    validations = validate_known_layout(data)
    if not all(v["ok"] for v in validations):
        bad = [v["label"] for v in validations if not v["ok"]]
        raise RomError("Known table signatures do not match: " + ", ".join(bad))
    formations = extract_formations(data)
    known_formation_ids = {f["id"] for f in formations["formations"]}
    return {
        "metadata": inspect_rom(data),
        "layout_validation": validations,
        "tables": {k: {**v, "offset": f"0x{v['offset']:06X}"} for k, v in TABLES.items()},
        "characters": extract_characters(data),
        "techniques": extract_techniques(data),
        "skills": extract_skills(data),
        "combos": extract_combos(data),
        "vehicles": extract_vehicles(data),
        "items": extract_items(data),
        "enemies": extract_enemies(data),
        "enemy_skills": extract_enemy_skills(data),
        "progression": extract_level_progression(data),
        "formations": formations,
        "formation_indexes": extract_formation_indexes(data, known_formation_ids),
    }


def write_extract(data: bytes, output_dir: str | Path) -> dict[str, Any]:
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    result = extract_all(data)
    for key in ["metadata", "layout_validation", "tables", "characters", "techniques", "skills", "combos", "vehicles", "items", "enemies", "enemy_skills", "progression", "formations", "formation_indexes"]:
        (output_dir / f"{key}.json").write_text(json.dumps(result[key], indent=2) + "\n", encoding="utf-8")
    return result
