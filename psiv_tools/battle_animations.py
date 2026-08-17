"""Retail enemy attack-object extraction.

The enemy records identify *which* enemy is acting, but the presentation
dispatches through ``EnemyAttackOffs`` and then creates one or more battle
objects.  Those objects own both the fixed mapping sequence and the exact
``Sound_Index`` write.  This module follows that path from the verified retail
ROM; it does not copy labels or addresses from the Grand Cross disassembly.

The extractor is deliberately additive.  It emits a separate pack file so the
existing enemy record shape remains the 48-byte ``Battle_EnemyData`` projection.
"""

from __future__ import annotations

import hashlib
from pathlib import Path
from typing import Any

from .core import EXPECTED_SHA256, EXPECTED_SIZE, be16, be32
from .sound_defs import SFX_NAMES
from .symbols import ENEMY_SYMBOLS

ENEMY_ATTACK_OFFS = 0x00D0A4
ENEMY_ATTACK_COUNT = 153
ENEMY_ATTACK_END = ENEMY_ATTACK_OFFS + ENEMY_ATTACK_COUNT * 2

# Battle_RunObjects dispatches these pointer groups by object-id range.  The
# IDs are spaced by four because each range is a longword table; keeping the
# range in the descriptor prevents the common mistake of treating them as
# consecutive byte IDs.
OBJECT_TABLES = (
    ("battle_objects_group_2", 0x01252C, 0x040, 0x128),
    ("battle_objects_group_3", 0x015E2C, 0x12C, 0x23C),
    ("battle_objects_group_4", 0x01B6F6, 0x240, 0x31C),
    ("battle_objects_group_5", 0x020590, 0x320, 0x3FC),
    ("battle_objects_group_8", 0x026EC0, 0x700, 0x814),
    ("battle_objects_group_9", 0x02D88E, 0x818, 0x8BC),
    ("battle_objects_group_10", 0x033456, 0x8C0, 0x934),
)

SOUND_INDEX_ADDRESS = bytes.fromhex("00 FF 50 0A")
FRAME_UPDATE_CALL = bytes.fromhex("4E B9 00 02 56 AE")
FRAME_UPDATE_JUMP = bytes.fromhex("4E F9 00 02 56 AE")


class BattleAnimationError(ValueError):
    """The ROM does not satisfy the retail battle-animation anchors."""


def _hex(value: int) -> str:
    return f"0x{value:06X}"


def _sfx_name(sound_id: int) -> str | None:
    index = sound_id - 0xB5
    return SFX_NAMES[index] if 0 <= index < len(SFX_NAMES) else None


def _u16(rom: bytes, offset: int) -> int:
    if offset < 0 or offset + 2 > len(rom):
        raise BattleAnimationError(f"word at {_hex(offset)} runs past the ROM")
    return be16(rom, offset)


def _u32(rom: bytes, offset: int) -> int:
    if offset < 0 or offset + 4 > len(rom):
        raise BattleAnimationError(f"longword at {_hex(offset)} runs past the ROM")
    return be32(rom, offset)


def _check_retail_rom(rom: bytes) -> None:
    if len(rom) != EXPECTED_SIZE:
        raise BattleAnimationError(
            f"enemy animation extraction needs the {EXPECTED_SIZE}-byte retail ROM; "
            f"got {len(rom)}"
        )
    digest = hashlib.sha256(rom).hexdigest()
    if digest != EXPECTED_SHA256:
        raise BattleAnimationError(
            "enemy animation extraction is byte-authorized only for the verified "
            f"US retail ROM {EXPECTED_SHA256}; got {digest}"
        )

    # These bytes bound the retail pointer table: the preceding call/return is
    # Enemy_Attack's tail, and the following longwords are the next AI table.
    before = bytes.fromhex("4E BA E6 02 4E 75")
    after = bytes.fromhex("00 28 3B 40 3A 52")
    if rom[ENEMY_ATTACK_OFFS - len(before):ENEMY_ATTACK_OFFS] != before:
        raise BattleAnimationError("EnemyAttackOffs retail pre-table guard changed")
    if rom[ENEMY_ATTACK_END:ENEMY_ATTACK_END + len(after)] != after:
        raise BattleAnimationError("EnemyAttackOffs retail post-table guard changed")


def _attack_routines(rom: bytes) -> list[int]:
    routines = [ENEMY_ATTACK_OFFS + _u16(rom, ENEMY_ATTACK_OFFS + i * 2)
                for i in range(ENEMY_ATTACK_COUNT)]
    if len(set(routines)) != 74:
        raise BattleAnimationError(
            f"EnemyAttackOffs should resolve to 74 routines, got {len(set(routines))}"
        )
    if min(routines) != 0x00D218 or max(routines) != 0x010ECC:
        raise BattleAnimationError("EnemyAttackOffs routine range is not retail")
    return routines


def _object_pointers(rom: bytes) -> tuple[dict[int, int], list[dict[str, Any]]]:
    pointers: dict[int, int] = {}
    tables: list[dict[str, Any]] = []
    for name, offset, first_id, last_id in OBJECT_TABLES:
        if (last_id - first_id) % 4:
            raise BattleAnimationError(f"{name} has an unaligned id range")
        count = (last_id - first_id) // 4 + 1
        for index in range(count):
            object_id = first_id + index * 4
            pointer = _u32(rom, offset + index * 4)
            if pointer & 1 or pointer >= len(rom):
                raise BattleAnimationError(
                    f"{name} entry {_hex(object_id)} points outside retail ROM"
                )
            if object_id in pointers:
                raise BattleAnimationError(f"duplicate battle object id {_hex(object_id)}")
            pointers[object_id] = pointer
        tables.append({
            "name": name,
            "rom_offset": _hex(offset),
            "entry_count": count,
            "entry_bytes": 4,
            "grand_cross": 0,
        })
    if len(pointers) != 382:
        raise BattleAnimationError(f"retail object pointer census changed: {len(pointers)}")
    return pointers, tables


def _spans(pointers: dict[int, int], rom_length: int) -> dict[int, tuple[int, int]]:
    targets = sorted(set(pointers.values()))
    spans: dict[int, tuple[int, int]] = {}
    for index, start in enumerate(targets):
        end = targets[index + 1] if index + 1 < len(targets) else min(rom_length, start + 0x400)
        spans[start] = (start, end)
    return spans


def _attack_spans(routines: list[int]) -> dict[int, tuple[int, int]]:
    targets = sorted(set(routines))
    spans = {
        start: (start, targets[index + 1] if index + 1 < len(targets) else 0x010F00)
        for index, start in enumerate(targets)
    }
    # ForcedFly branches into MonsterFly's shared body after its first test.
    # Its table entry starts six bytes earlier, so scanning only to the next
    # unique pointer would drop the actual object creation and SFX write.
    spans[0x010D86] = (0x010D86, 0x010DB8)
    return spans


def _object_refs(rom: bytes, start: int, end: int, pointers: dict[int, int]) -> list[tuple[int, int]]:
    refs = []
    for offset in range(start, max(start, end - 5), 2):
        opcode = rom[offset:offset + 2]
        if opcode == bytes.fromhex("32 BC"):
            object_id = _u16(rom, offset + 2)
        elif opcode == bytes.fromhex("33 7C") and _u16(rom, offset + 4) == 0:
            # One retail routine is assembled through the legacy raw `_move.w`
            # spelling. Its destination word is part of the six-byte opcode.
            object_id = _u16(rom, offset + 2)
        else:
            continue
        if object_id in pointers:
            refs.append((offset, object_id))
    return refs


def _sound_writes(rom: bytes, start: int, end: int, object_id: int | None) -> list[dict[str, Any]]:
    writes = []
    for offset in range(start, max(start, end - 7), 2):
        if rom[offset:offset + 2] == bytes.fromhex("13 FC") and rom[offset + 4:offset + 8] == SOUND_INDEX_ADDRESS:
            sound_id = rom[offset + 3]
            writes.append({
                "object_id": object_id,
                "rom_offset": _hex(offset),
                "dispatch": "Sound_Index",
                "sound_id": sound_id,
                "sound_name": _sfx_name(sound_id),
            })
        elif rom[offset:offset + 2] == bytes.fromhex("13 7C") and rom[offset + 4:offset + 6] == bytes.fromhex("00 10"):
            sound_id = rom[offset + 3]
            writes.append({
                "object_id": object_id,
                "rom_offset": _hex(offset),
                "dispatch": "object_local_sound",
                "sound_id": sound_id,
                "sound_name": _sfx_name(sound_id),
            })
    return writes


def _mapping_records(rom: bytes, start: int, end: int, object_id: int) -> list[dict[str, Any]]:
    records = []
    for offset in range(start, max(start, end - 9), 2):
        if rom[offset:offset + 2] != bytes.fromhex("29 7C") or _u16(rom, offset + 6) != 8:
            continue
        mapping_offset = _u32(rom, offset + 2)
        if mapping_offset + 2 > len(rom):
            continue
        duration = rom[mapping_offset]
        frame_count = rom[mapping_offset + 1]
        record_end = mapping_offset + 2 + frame_count * 4
        if not 1 <= frame_count <= 64 or record_end > len(rom):
            continue
        pointers = [_u32(rom, mapping_offset + 2 + index * 4) for index in range(frame_count)]
        if any(pointer & 1 or pointer >= len(rom) for pointer in pointers):
            continue
        uses_frame_timer = (
            FRAME_UPDATE_CALL in rom[start:end] or FRAME_UPDATE_JUMP in rom[start:end]
        )
        records.append({
            "object_id": object_id,
            "assignment_offset": _hex(offset),
            "mapping_offset": _hex(mapping_offset),
            "frame_duration": duration,
            "frame_count": frame_count,
            "total_frames": duration * frame_count,
            "mapping_pointers": [_hex(pointer) for pointer in pointers],
            "frame_timer_helper": "loc_256AE" if uses_frame_timer else None,
        })
    return records


def _walk_animation(
    rom: bytes,
    root_ids: list[int],
    object_pointers: dict[int, int],
    object_spans: dict[int, tuple[int, int]],
) -> tuple[list[int], list[dict[str, Any]], list[dict[str, Any]]]:
    objects: list[int] = []
    sound_writes: list[dict[str, Any]] = []
    mappings: list[dict[str, Any]] = []
    seen: set[int] = set()

    def visit(object_id: int) -> None:
        if object_id in seen:
            return
        seen.add(object_id)
        objects.append(object_id)
        start, end = object_spans[object_pointers[object_id]]
        sound_writes.extend(_sound_writes(rom, start, end, object_id))
        mappings.extend(_mapping_records(rom, start, end, object_id))
        for _, child in _object_refs(rom, start, end, object_pointers):
            visit(child)

    for object_id in root_ids:
        visit(object_id)
    return objects, sound_writes, mappings


def build_enemy_animations(rom: bytes, display: dict[int, str] | None = None) -> dict[str, Any]:
    """Decode the retail enemy attack dispatch and battle-object records."""
    _check_retail_rom(rom)
    routines = _attack_routines(rom)
    object_pointers, object_tables = _object_pointers(rom)
    object_spans = _spans(object_pointers, len(rom))
    attack_spans = _attack_spans(routines)
    display = display or {}
    animations: list[dict[str, Any]] = []

    for enemy_id, routine in enumerate(routines):
        start, end = attack_spans[routine]
        root_refs = _object_refs(rom, start, end, object_pointers)
        root_ids: list[int] = []
        for _, object_id in root_refs:
            if object_id not in root_ids:
                root_ids.append(object_id)
        objects, writes, mappings = _walk_animation(
            rom, root_ids, object_pointers, object_spans
        )
        writes = _sound_writes(rom, start, end, None) + writes
        mappings = _mapping_records(rom, start, end, 0xFFFF) + mappings
        exact = next((write for write in writes if write["dispatch"] == "Sound_Index"), None)
        if exact is None:
            raise BattleAnimationError(
                f"enemy {enemy_id} ({ENEMY_SYMBOLS[enemy_id]}) has no direct Sound_Index write"
            )

        frame_sequence = next(
            (
                mapping for mapping in mappings
                if mapping["frame_timer_helper"] == "loc_256AE"
                and mapping["frame_duration"] > 0
            ),
            None,
        )
        if frame_sequence is not None and frame_sequence["object_id"] == 0xFFFF:
            frame_sequence["object_id"] = None
        animations.append({
            "enemy_id": enemy_id,
            "symbol": ENEMY_SYMBOLS[enemy_id],
            "display_name": display.get(enemy_id),
            "routine_offset": _hex(routine),
            "root_object_ids": root_ids,
            "object_ids": objects,
            "sfx_id": exact["sound_id"],
            "sfx_name": exact["sound_name"],
            "dispatch": {
                "kind": exact["dispatch"],
                "rom_offset": exact["rom_offset"],
                "object_id": exact["object_id"],
            },
            "sfx_writes": writes,
            "frame_sequence": frame_sequence,
            "movement_proven": False,
            "flash_timing_proven": frame_sequence is not None,
            "sprite_sheet_proven": False,
        })

    exact_count = len(animations)
    timed = sum(animation["frame_sequence"] is not None for animation in animations)
    sequence_records = sum(animation["frame_sequence"] is not None for animation in animations)
    return {
        "kind": "battle_enemy_animations",
        "count": len(animations),
        "source": {
            "label": "EnemyAttackOffs",
            "rom_offset": _hex(ENEMY_ATTACK_OFFS),
            "table_end": _hex(ENEMY_ATTACK_END),
            "record_count": ENEMY_ATTACK_COUNT,
            "record_bytes": 2,
            "rom_size": EXPECTED_SIZE,
            "rom_sha256": EXPECTED_SHA256,
            "grand_cross": 0,
            "reference_clone": "reference/ps4disasm",
            "reference_grand_cross": 1,
            "reference_role": "labels and semantic guide only; not byte authority",
            "retail_guards": {
                "before_table": "4EBAE6024E75",
                "after_table": "00283B403A52",
            },
            "tables": [
                {
                    "name": "EnemyAttackOffs",
                    "rom_offset": _hex(ENEMY_ATTACK_OFFS),
                    "entry_count": ENEMY_ATTACK_COUNT,
                    "entry_bytes": 2,
                    "grand_cross": 0,
                },
                *object_tables,
            ],
        },
        "animations": animations,
        "census": {
            "enemy_count": len(animations),
            "distinct_attack_routines": len(set(routines)),
            "exact_sfx": exact_count,
            "generic_sfx": 0,
            "frame_sequence_records": sequence_records,
            "timed_frame_sequences": timed,
            "frame_sequence_deferred": len(animations) - timed,
            "movement_deferred": len(animations),
            "flash_timing": timed,
            "sprite_sheet_deferred": len(animations),
        },
    }


def write_enemy_animations(rom: bytes, path: str | Path, display: dict[int, str] | None = None) -> None:
    """Write a standalone provenance-heavy scout record for review tools."""
    import json

    payload = build_enemy_animations(rom, display)
    target = Path(path)
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
