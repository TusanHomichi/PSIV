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

from . import png
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

# `Battle_FillSpriteAttributes` consumes one count-minus-one byte followed by
# six bytes per VDP sprite: Y, size, tile high, tile low, X and mirrored X.
# Keeping these here makes the format boundary visible next to the decoder;
# the old wave-5 scout deliberately stopped before this half.
MAPPING_HEADER_BYTES = 1
MAPPING_ENTRY_BYTES = 6
FIGHTER_X_FIELD = 0x2C
FIGHTER_Y_FIELD = 0x2E
FIXED_POINT_X_FIELD = 0x30
FIXED_POINT_Y_FIELD = 0x34
BASE_OBJECT_Y = 0x00D8


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


def _signed_byte(value: int) -> int:
    return value - 0x100 if value & 0x80 else value


def _signed_word(value: int) -> int:
    return value - 0x10000 if value & 0x8000 else value


def _mapping_record(rom: bytes, offset: int, bank_patterns: int | None = None) -> dict[str, Any]:
    """Decode one retail VDP sprite mapping record.

    The record is not a five-byte tuple.  The two X bytes are both present;
    the renderer chooses the second only when the object's mirror flag is
    set.  The count byte is count-minus-one because the 68000 loop is `dbf`.
    """
    if offset < 0 or offset + MAPPING_HEADER_BYTES > len(rom):
        raise BattleAnimationError(f"mapping record at {_hex(offset)} is outside the ROM")
    sprite_count = rom[offset] + 1
    record_end = offset + MAPPING_HEADER_BYTES + sprite_count * MAPPING_ENTRY_BYTES
    if sprite_count > 64 or record_end > len(rom):
        raise BattleAnimationError(
            f"mapping record at {_hex(offset)} has {sprite_count} sprites or runs past ROM"
        )
    entries: list[dict[str, Any]] = []
    for index in range(sprite_count):
        entry_offset = offset + MAPPING_HEADER_BYTES + index * MAPPING_ENTRY_BYTES
        y = _signed_byte(rom[entry_offset])
        size = rom[entry_offset + 1]
        tile_word = _u16(rom, entry_offset + 2)
        x = _signed_byte(rom[entry_offset + 4])
        x_mirror = _signed_byte(rom[entry_offset + 5])
        width_tiles = (size & 0x03) + 1
        height_tiles = ((size >> 2) & 0x03) + 1
        tile_index = tile_word & 0x07FF
        tile_span_end = tile_index + width_tiles * height_tiles
        entries.append({
            "rom_offset": _hex(entry_offset),
            "y": y,
            "size": size,
            "tile_word": f"0x{tile_word:04X}",
            "tile_index": tile_index,
            "h_flip": bool(tile_word & 0x0800),
            "v_flip": bool(tile_word & 0x1000),
            "priority": bool(tile_word & 0x8000),
            "palette_bits": (tile_word >> 13) & 0x03,
            "x": x,
            "x_mirror": x_mirror,
            "width_tiles": width_tiles,
            "height_tiles": height_tiles,
            "tile_span_end": tile_span_end,
            "tile_bank_valid": (
                bank_patterns is None or tile_span_end <= bank_patterns
            ),
            # Palette bits would select a different CRAM line in the VDP.
            # The current line-relative PNG surface cannot claim those pixels.
            "attributes_valid": not bool(tile_word & 0x6000),
        })
    valid = all(entry["tile_bank_valid"] and entry["attributes_valid"] for entry in entries)
    return {
        "rom_offset": _hex(offset),
        "record_header_bytes": MAPPING_HEADER_BYTES,
        "entry_bytes": MAPPING_ENTRY_BYTES,
        "sprite_count": sprite_count,
        "entries": entries,
        "all_entries_valid": valid,
        "record_end": _hex(record_end),
    }


def _coordinate_sites(rom: bytes, start: int, end: int, object_id: int) -> list[dict[str, Any]]:
    """Find direct retail writes to a battle object's coordinate fields.

    This is intentionally a small opcode decoder rather than a disassembler
    dependency.  It recognizes the immediate/add-quick forms and the
    fixed-point commits used by the attack objects.  Other coordinate writes
    remain visible as a partial/deferred reason instead of being guessed.
    """
    sites: list[dict[str, Any]] = []

    def add_site(offset: int, field: int, kind: str, **values: Any) -> None:
        axis = "x" if field == FIGHTER_X_FIELD else "y"
        sites.append({
            "object_id": object_id,
            "rom_offset": _hex(offset),
            "field": f"${field:02X}(a4)",
            "axis": axis,
            "kind": kind,
            **values,
        })

    for offset in range(start, max(start, end - 7), 2):
        opcode = _u16(rom, offset)
        if opcode in (0x066C, 0x046C):  # addi/subi.w #imm,d16(a4)
            field = _u16(rom, offset + 4)
            if field in (FIGHTER_X_FIELD, FIGHTER_Y_FIELD):
                immediate = _signed_word(_u16(rom, offset + 2))
                add_site(
                    offset,
                    field,
                    "delta",
                    delta=immediate if opcode == 0x066C else -immediate,
                    operation="addi.w" if opcode == 0x066C else "subi.w",
                )
            continue
        # addq/subq.w #n,d16(a4), where the quick value is in bits 3..1 of
        # the high byte.  The low byte is the a4 displacement mode.
        if 0x5000 <= opcode <= 0x5FFF and (opcode & 0x00FF) == 0x6C:
            field = _u16(rom, offset + 2)
            if field in (FIGHTER_X_FIELD, FIGHTER_Y_FIELD):
                quick = (opcode >> 1) & 0x07 or 8
                is_add = not bool(opcode & 0x0001)
                add_site(
                    offset,
                    field,
                    "delta",
                    delta=quick if is_add else -quick,
                    operation="addq.w" if is_add else "subq.w",
                )
            continue
        if opcode == 0x397C:  # move.w #imm,d16(a4)
            field = _u16(rom, offset + 4)
            if field in (FIGHTER_X_FIELD, FIGHTER_Y_FIELD):
                add_site(
                    offset,
                    field,
                    "absolute",
                    value=_u16(rom, offset + 2),
                    operation="move.w #imm",
                )
            continue
        if opcode == 0x0C6C:  # cmpi.w #imm,d16(a4)
            field = _u16(rom, offset + 4)
            if field in (FIGHTER_X_FIELD, FIGHTER_Y_FIELD):
                add_site(
                    offset,
                    field,
                    "compare",
                    value=_u16(rom, offset + 2),
                    operation="cmpi.w #imm",
                )
            continue
        if opcode == 0x396C:  # move.w d16(a4),d16(a4)
            field = _u16(rom, offset + 4)
            if field in (FIGHTER_X_FIELD, FIGHTER_Y_FIELD):
                add_site(
                    offset,
                    field,
                    "fixed_point_commit",
                    source_field=f"${_u16(rom, offset + 2):02X}(a4)",
                    operation="move.w d16(a4),d16(a4)",
                )
    return sites


def _movement_evidence(
    rom: bytes,
    object_ids: list[int],
    object_pointers: dict[int, int],
    object_spans: dict[int, tuple[int, int]],
    frame_sequence: dict[str, Any] | None,
) -> dict[str, Any]:
    sites: list[dict[str, Any]] = []
    by_object: dict[int, list[dict[str, Any]]] = {}
    for object_id in object_ids:
        object_sites = _coordinate_sites(
            rom, *object_spans[object_pointers[object_id]], object_id
        )
        by_object[object_id] = object_sites
        sites.extend(object_sites)

    primary_id = None if frame_sequence is None else frame_sequence.get("object_id")
    primary = by_object.get(primary_id or -1, [])
    runtime: dict[str, Any] | None = None
    status = "deferred"
    reason = "no positive-duration mapping record selects a movement clock"

    if frame_sequence is not None:
        # A single immediate delta (or no coordinate write at all) is safe:
        # loc_12618 supplies the exact pivot and the object only offsets it.
        if primary and all(site["kind"] == "delta" for site in primary):
            offsets = {"x": 0, "y": 0}
            for site in primary:
                offsets[site["axis"]] += site["delta"]
            runtime = {
                "kind": "static_offset",
                "initial_offset_pixels": [offsets["x"], offsets["y"]],
                "step_pixels": [0, 0],
            }
            status = "exact"
            reason = "selected object uses only direct relative coordinate deltas"
        elif not primary:
            runtime = {
                "kind": "static_offset",
                "initial_offset_pixels": [0, 0],
                "step_pixels": [0, 0],
            }
            status = "exact" if not sites else "partial"
            reason = (
                "selected mapping object stays on the shared fighter pivot"
                if not sites else
                "selected mapping object is static, but another reachable object writes coordinates"
            )
        else:
            # The clean lunge form is: absolute start, one immediate delta,
            # and a compare limit on the same axis.  This is the retail shape
            # used by object $370 (TwinArms/SoldrFiend family).
            absolutes = [site for site in primary if site["kind"] == "absolute"]
            deltas = [site for site in primary if site["kind"] == "delta"]
            compares = [site for site in primary if site["kind"] == "compare"]
            axes = {site["axis"] for site in primary if site["kind"] != "compare"}
            if (
                len(absolutes) == 1
                and len(deltas) == 1
                and len(compares) == 1
                and len(axes) == 1
                and all(site["axis"] == next(iter(axes)) for site in primary)
            ):
                axis = next(iter(axes))
                start_value = absolutes[0]["value"]
                step = deltas[0]["delta"]
                limit = compares[0]["value"]
                runtime = {
                    "kind": "linear",
                    "axis": axis,
                    "start_hardware": start_value,
                    "step_hardware": step,
                    "limit_hardware": limit,
                    "initial_offset_pixels": [
                        start_value - BASE_OBJECT_Y if axis == "x" else 0,
                        start_value - BASE_OBJECT_Y if axis == "y" else 0,
                    ],
                    "step_pixels": [step if axis == "x" else 0, step if axis == "y" else 0],
                }
                status = "exact"
                reason = "absolute start, per-tick delta, and terminal compare are all retail-proven"
            else:
                status = "partial"
                reason = "coordinate writes include a branch, absolute placement, or fixed-point helper"

    return {
        "status": status,
        "reason": reason,
        "fields": {
            "x": "$2C(a4)",
            "y": "$2E(a4)",
            "fixed_point_x": "$30(a4)",
            "fixed_point_y": "$34(a4)",
            "shared_init": "loc_12618",
        },
        "writes": sites,
        "runtime": runtime,
    }


def _frame_sequence_evidence(
    rom: bytes,
    sequence: dict[str, Any] | None,
    bank_patterns: int | None,
) -> tuple[dict[str, Any] | None, dict[str, int | str]]:
    if sequence is None:
        return None, {"status": "deferred", "reason": "no positive-duration mapping record"}
    records = []
    for pointer in sequence["mapping_pointers"]:
        offset = int(pointer, 16)
        try:
            records.append(_mapping_record(rom, offset, bank_patterns))
        except BattleAnimationError as exc:
            # A pointer can be a valid in-ROM address and still be a state
            # table, not a sprite record.  Preserve that fact as deferred
            # evidence rather than making the whole 153-enemy extraction
            # pretend it is renderable.
            records.append({
                "rom_offset": _hex(offset),
                "record_header_bytes": MAPPING_HEADER_BYTES,
                "entry_bytes": MAPPING_ENTRY_BYTES,
                "sprite_count": None,
                "entries": [],
                "all_entries_valid": False,
                "record_end": None,
                "decode_error": str(exc),
            })
    exact = sum(record["all_entries_valid"] for record in records)
    if exact == len(records):
        status = "exact"
        reason = "every selected six-byte VDP mapping resolves within the enemy art bank"
    elif exact:
        status = "partial"
        reason = f"{len(records) - exact} selected mapping record(s) exceed the art bank or set palette bits"
    else:
        status = "deferred"
        reason = "none of the selected mapping records resolves within the enemy art bank"
    sequence = dict(sequence)
    sequence["mapping_records"] = records
    return sequence, {"status": status, "reason": reason}


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
    # The art table is a separate wave-1 authority, but it is needed here to
    # classify a mapping record as exact/partial/deferred.  Import locally so
    # the animation scout remains usable without creating an art-module import
    # cycle during pack construction.
    from .battle_art import build_tile_bank, enemy_art_bounds, enemy_records

    art_records = enemy_records(rom)
    art_bounds = enemy_art_bounds(art_records, len(rom))
    art_banks: dict[int, int] = {}
    for record in art_records:
        art_banks[record["id"]] = len(build_tile_bank(rom, record, art_bounds)[0])
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
        frame_sequence, composition = _frame_sequence_evidence(
            rom, frame_sequence, art_banks[enemy_id]
        )
        movement = _movement_evidence(
            rom, objects, object_pointers, object_spans, frame_sequence
        )
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
            "movement": movement,
            "composition": composition,
            "movement_proven": movement["status"] == "exact",
            "flash_timing_proven": frame_sequence is not None,
            "sprite_sheet_proven": composition["status"] == "exact",
        })

    exact_count = len(animations)
    timed = sum(animation["frame_sequence"] is not None for animation in animations)
    sequence_records = sum(animation["frame_sequence"] is not None for animation in animations)
    movement_census = {
        status: sum(animation["movement"]["status"] == status for animation in animations)
        for status in ("exact", "partial", "deferred")
    }
    composition_census = {
        status: sum(animation["composition"]["status"] == status for animation in animations)
        for status in ("exact", "partial", "deferred")
    }
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
            "enemy_art_table": {
                "label": "loc_27F3AE",
                "rom_offset": "0x27F3AE",
                "entry_count": ENEMY_ATTACK_COUNT,
                "entry_bytes": 20,
                "fields": {
                    "art_1": "+0x02",
                    "art_2": "+0x06",
                    "art_3": "+0x0A",
                    "body_mapping": "+0x0E",
                    "half_width": "+0x12",
                    "height": "+0x13",
                },
                "bank_order": [3, 1, 2],
                "grand_cross": 0,
            },
            "mapping_record": {
                "consumer": "Battle_FillSpriteAttributes",
                "grand_cross": 0,
                "header_bytes": MAPPING_HEADER_BYTES,
                "entry_bytes": MAPPING_ENTRY_BYTES,
                "entry_fields": [
                    "y_offset",
                    "size",
                    "tile_word",
                    "x_offset",
                    "x_offset_mirrored",
                ],
                "tile_index_mask": "0x07FF",
                "h_flip_bit": "0x0800",
                "v_flip_bit": "0x1000",
                "palette_mask": "0x6000",
            },
            "movement_fields": {
                "x": "$2C(a4)",
                "y": "$2E(a4)",
                "fixed_point_x": "$30(a4)",
                "fixed_point_y": "$34(a4)",
                "shared_init": "loc_12618",
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
            # Legacy headline retained for consumers that only knew the old
            # boolean.  It now means anything not exact, with the detailed
            # three-way census beside it.
            "movement_deferred": movement_census["partial"] + movement_census["deferred"],
            "movement_exact": movement_census["exact"],
            "movement_partial": movement_census["partial"],
            "movement_status_deferred": movement_census["deferred"],
            "flash_timing": timed,
            "sprite_sheet_deferred": composition_census["partial"] + composition_census["deferred"],
            "sprite_sheet_exact": composition_census["exact"],
            "sprite_sheet_partial": composition_census["partial"],
            "sprite_sheet_status_deferred": composition_census["deferred"],
        },
    }


def write_enemy_animations(rom: bytes, path: str | Path, display: dict[int, str] | None = None) -> None:
    """Write a standalone provenance-heavy scout record for review tools."""
    import json

    payload = build_enemy_animations(rom, display)
    target = Path(path)
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


ENEMY_ATTACK_ART_DIRECTORY = "battle/art/enemy_attacks"
ENEMY_ATTACK_ART_NAME = "battle/art/enemy_attacks.json"


def _attack_safe(value: str) -> str:
    return "".join(char if char.isalnum() or char in "-_" else "_" for char in value)


def _attack_movement_pixels(movement: dict[str, Any] | None) -> dict[str, Any]:
    """Return only the normalized motion contract the Godot layer consumes."""
    runtime = None if movement is None else movement.get("runtime")
    if not runtime:
        return {
            "kind": "static_offset",
            "initial_offset_pixels": [0, 0],
            "step_pixels": [0, 0],
            "limit_offset_pixels": None,
        }
    limit_offset = None
    if runtime.get("limit_hardware") is not None:
        limit = runtime["limit_hardware"] - BASE_OBJECT_Y
        limit_offset = [limit if runtime.get("axis") == "x" else 0,
                        limit if runtime.get("axis") == "y" else 0]
    return {
        "kind": runtime["kind"],
        "initial_offset_pixels": runtime["initial_offset_pixels"],
        "step_pixels": runtime["step_pixels"],
        "limit_hardware": runtime.get("limit_hardware"),
        "limit_offset_pixels": limit_offset,
    }


def _attack_canvas(
    animation: dict[str, Any],
    art_record: dict[str, Any],
) -> tuple[int, int, int, int]:
    """Find one stable transparent canvas for all of an enemy's frames."""
    body_width = 2 * art_record["half_width_cells"] * 8
    body_top = (15 - art_record["height_cells"]) * 8
    pivot_x = body_width
    pivot_y = BASE_OBJECT_Y - 0x80 - body_top
    positions: list[tuple[int, int, int, int]] = []
    sequence = animation["frame_sequence"]
    for record in sequence["mapping_records"]:
        for entry in record["entries"]:
            x = pivot_x + entry["x"]
            y = pivot_y + entry["y"]
            width = entry["width_tiles"] * 8
            height = entry["height_tiles"] * 8
            positions.append((x, y, width, height))
    if not positions:
        raise BattleAnimationError(
            f"enemy {animation['enemy_id']} has an exact composition with no sprites"
        )
    min_x = min(0, *(x for x, _, _, _ in positions))
    min_y = min(0, *(y for _, y, _, _ in positions))
    max_x = max(body_width, *(x + width for x, _, width, _ in positions))
    max_y = max(art_record["height_cells"] * 8, *(y + height for _, y, _, height in positions))
    return min_x, min_y, max_x - min_x, max_y - min_y


def _render_attack_mapping(
    tiles: list[bytes],
    record: dict[str, Any],
    canvas: tuple[int, int, int, int],
    pivot: tuple[int, int],
    palette: list[tuple[int, int, int]],
) -> bytes:
    min_x, min_y, width, height = canvas
    pixels = bytearray(width * height)
    pivot_x, pivot_y = pivot
    for entry in record["entries"]:
        tile_word = int(entry["tile_word"], 16)
        tile_index = entry["tile_index"]
        width_tiles = entry["width_tiles"]
        height_tiles = entry["height_tiles"]
        h_flip = entry["h_flip"]
        v_flip = entry["v_flip"]
        sprite_x = pivot_x + entry["x"] - min_x
        sprite_y = pivot_y + entry["y"] - min_y
        for y in range(height_tiles * 8):
            source_y = height_tiles * 8 - 1 - y if v_flip else y
            tile_row, pixel_y = divmod(source_y, 8)
            for x in range(width_tiles * 8):
                source_x = width_tiles * 8 - 1 - x if h_flip else x
                tile_col, pixel_x = divmod(source_x, 8)
                tile = tiles[tile_index + tile_row * width_tiles + tile_col]
                pixel = tile[pixel_y * 8 + pixel_x]
                if pixel:
                    pixels[(sprite_y + y) * width + sprite_x + x] = pixel
    return png.encode_indexed(width, height, pixels, palette, (0,))


def emit_enemy_attack_art(
    rom: bytes,
    out_dir: str | Path,
    animations_payload: dict[str, Any] | None = None,
) -> tuple[dict[str, Any], int, int]:
    """Emit exact frame-index sheets and normalized movement metadata.

    A frame is emitted only after the mapping record has passed the art-bank
    and palette-bit checks in :func:`build_enemy_animations`.  The remaining
    records stay in the index with their census reason and no PNG, so a Godot
    consumer cannot accidentally animate a guessed tile bank.
    """
    from .battle_art import enemy_art_bounds, enemy_records, build_tile_bank
    from .battle_art_pack import enemy_palette
    payload = animations_payload or build_enemy_animations(rom)
    records = enemy_records(rom)
    bounds = enemy_art_bounds(records, len(rom))
    by_id = {record["id"]: record for record in records}
    directory = Path(out_dir)
    attack_dir = directory / ENEMY_ATTACK_ART_DIRECTORY
    attack_dir.mkdir(parents=True, exist_ok=True)
    entries: list[dict[str, Any]] = []
    total_bytes = 0
    png_count = 0

    for animation in payload["animations"]:
        enemy_id = animation["enemy_id"]
        composition = animation["composition"]
        entry: dict[str, Any] = {
            "id": enemy_id,
            "symbol": animation["symbol"],
            "status": composition["status"],
            "reason": composition["reason"],
            "movement": {
                "status": animation["movement"]["status"],
                "reason": animation["movement"]["reason"],
                "runtime": _attack_movement_pixels(animation["movement"]),
                "writes": animation["movement"]["writes"],
            },
            "frames": [],
            "source": {
                "enemy_art_record": _hex(0x27F3AE + enemy_id * 20),
                "mapping_consumer": "Battle_FillSpriteAttributes",
                "grand_cross": 0,
            },
        }
        if composition["status"] != "exact":
            entries.append(entry)
            continue

        art_record = by_id[enemy_id]
        tiles, art_sources = build_tile_bank(rom, art_record, bounds)
        canvas = _attack_canvas(animation, art_record)
        body_width = 2 * art_record["half_width_cells"] * 8
        body_top = (15 - art_record["height_cells"]) * 8
        pivot = (body_width, BASE_OBJECT_Y - 0x80 - body_top)
        palette = enemy_palette(rom, enemy_id)
        for frame_index, record in enumerate(animation["frame_sequence"]["mapping_records"]):
            image = _render_attack_mapping(tiles, record, canvas, pivot, palette)
            name = (
                f"{enemy_id:03d}_{_attack_safe(animation['symbol'])}_"
                f"frame{frame_index:02d}.png"
            )
            relative = f"{ENEMY_ATTACK_ART_DIRECTORY}/{name}"
            (directory / relative).write_bytes(image)
            total_bytes += len(image)
            png_count += 1
            entry["frames"].append({
                "index": frame_index,
                "mapping_rom_offset": record["rom_offset"],
                "png": relative,
                "png_sha256": hashlib.sha256(image).hexdigest(),
                "duration": animation["frame_sequence"]["frame_duration"],
            })
        entry.update({
            "origin_pixels": [canvas[0], canvas[1]],
            "width_pixels": canvas[2],
            "height_pixels": canvas[3],
            "frame_duration": animation["frame_sequence"]["frame_duration"],
            "total_frames": animation["frame_sequence"]["total_frames"],
            "art": [
                {
                    "field": source["field"],
                    "rom_offset": source["rom_offset"],
                    "first_pattern": source["first_pattern"],
                    "pattern_count": source["tile_count"],
                }
                for source in art_sources
            ],
        })
        entries.append(entry)

    return {
        "kind": "battle_enemy_attack_art",
        "count": len(entries),
        "source": {
            "enemy_attack_table": "0x00D0A4",
            "enemy_art_table": "0x27F3AE",
            "mapping_consumer": "Battle_FillSpriteAttributes",
            "mapping_header_bytes": MAPPING_HEADER_BYTES,
            "mapping_entry_bytes": MAPPING_ENTRY_BYTES,
            "art_bank_order": [3, 1, 2],
            "rom_sha256": payload["source"]["rom_sha256"],
            "grand_cross": 0,
        },
        "census": {
            "enemies": len(entries),
            "exact": sum(entry["status"] == "exact" for entry in entries),
            "partial": sum(entry["status"] == "partial" for entry in entries),
            "deferred": sum(entry["status"] == "deferred" for entry in entries),
            "frames": png_count,
        },
        "enemies": entries,
    }, total_bytes, png_count
