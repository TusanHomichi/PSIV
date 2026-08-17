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
from collections import Counter
from typing import Any

from .battle_animation_mapping import BattleMappingError
from .battle_animation_mapping import MAPPING_ENTRY_BYTES, MAPPING_HEADER_BYTES
from .battle_animation_mapping import decode_mapping_record
from .battle_animation_objects import frame_sequences as decode_frame_sequences
from .battle_animation_objects import object_evidence as decode_object_evidence
from .battle_animation_objects import plc_art as decode_plc_art
from .battle_animation_objects import plc_loads as decode_plc_loads
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


def _signed_word(value: int) -> int:
    return value - 0x10000 if value & 0x8000 else value


def _signed_long(value: int) -> int:
    return value - 0x100000000 if value & 0x80000000 else value


def _coordinate_sites(rom: bytes, start: int, end: int, object_id: int) -> list[dict[str, Any]]:
    """Find direct retail writes to a battle object's coordinate fields.

    This is intentionally a small opcode decoder rather than a disassembler
    dependency.  It recognizes the immediate/add-quick forms and the
    fixed-point commits used by the attack objects.  Other coordinate writes
    remain visible as a partial/deferred reason instead of being guessed.
    """
    sites: list[dict[str, Any]] = []

    def add_site(offset: int, field: int, kind: str, **values: Any) -> None:
        axis = {
            FIGHTER_X_FIELD: "x",
            FIXED_POINT_X_FIELD: "x",
            FIGHTER_Y_FIELD: "y",
            FIXED_POINT_Y_FIELD: "y",
        }.get(field, "fixed")
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
        if opcode == 0x06AC:  # addi.l #imm,d16(a4): fixed-point accumulator
            field = _u16(rom, offset + 6)
            if field in (0x20, 0x24, FIXED_POINT_X_FIELD, FIXED_POINT_Y_FIELD):
                add_site(
                    offset,
                    field,
                    "fixed_point_delta",
                    delta_fixed=_signed_long(_u32(rom, offset + 2)),
                    operation="addi.l",
                    accumulator_field=f"${field:02X}(a4)",
                )
            continue
        if (opcode & 0xF1FF) == 0x202C:  # move.l d16(a4),Dn
            field = _u16(rom, offset + 2)
            if field in (0x20, 0x24, FIXED_POINT_X_FIELD, FIXED_POINT_Y_FIELD):
                add_site(
                    offset,
                    field,
                    "fixed_point_load",
                    source_field=f"${field:02X}(a4)",
                    register=f"d{(opcode >> 9) & 0x07}",
                    operation="move.l d16(a4),Dn",
                )
            continue
        if (opcode & 0xF1FF) == 0xD0AC and (opcode & 0x00C0) == 0x0080:
            field = _u16(rom, offset + 2)
            if field in (FIXED_POINT_X_FIELD, FIXED_POINT_Y_FIELD):
                add_site(
                    offset,
                    field,
                    "fixed_point_add",
                    register=f"d{(opcode >> 9) & 0x07}",
                    operation="add.l Dn,d16(a4)",
                )
            continue
        if opcode == 0x297C:  # move.l #imm,d16(a4)
            field = _u16(rom, offset + 6)
            if field in (0x20, 0x24, FIXED_POINT_X_FIELD, FIXED_POINT_Y_FIELD):
                add_site(
                    offset,
                    field,
                    "fixed_point_absolute",
                    value_fixed=_signed_long(_u32(rom, offset + 2)),
                    operation="move.l #imm",
                )
            continue
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
    tracks = [
        {
            "object_id": object_id,
            "status": "exact",
            "kind": "object_track",
            "writes": object_sites,
            "reason": "reachable effect/projectile object kept separate from the selected mapping object",
        }
        for object_id, object_sites in by_object.items()
        if object_id != primary_id and object_sites
    ]
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
            status = "exact"
            reason = "selected mapping object stays on the shared fighter pivot; reachable effect tracks are recorded separately"
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
                known_kinds = {
                    "delta",
                    "absolute",
                    "compare",
                    "fixed_point_absolute",
                    "fixed_point_delta",
                    "fixed_point_load",
                    "fixed_point_add",
                    "fixed_point_commit",
                }
                if primary and all(site["kind"] in known_kinds for site in primary):
                    absolutes = [site for site in primary if site["kind"] == "absolute"]
                    offsets = {"x": 0, "y": 0}
                    if absolutes:
                        first = absolutes[0]
                        offsets[first["axis"]] = first["value"] - BASE_OBJECT_Y
                    runtime = {
                        "kind": (
                            "fixed_point_branch"
                            if any(site["kind"].startswith("fixed_point") for site in primary)
                            else "branch_table"
                        ),
                        "initial_offset_pixels": [offsets["x"], offsets["y"]],
                        "step_pixels": [0, 0],
                        "branches": primary,
                    }
                    status = "exact"
                    reason = "every selected-object $2C/$2E/$30/$34 write is decoded; branch and fixed-point variants are retained as an exact structural track"
                else:
                    status = "partial"
                    reason = "coordinate writes include an opcode outside the decoded $2C/$2E/$30/$34 track"

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
        "tracks": tracks,
        "track_completion": {
            "status": "exact" if all(
                site["kind"] in {
                    "delta",
                    "absolute",
                    "compare",
                    "fixed_point_absolute",
                    "fixed_point_delta",
                    "fixed_point_load",
                    "fixed_point_add",
                    "fixed_point_commit",
                }
                for site in sites
            ) else "partial",
            "fields": ["$2C(a4)", "$2E(a4)", "$30(a4)", "$34(a4)"],
            "reason": "all statically recognized coordinate writes are retained per object; unresolved opcodes remain explicitly partial",
        },
        "runtime": runtime,
    }


def _frame_sequence_evidence(
    rom: bytes,
    sequence: dict[str, Any] | None,
    bank_patterns: int | None,
    plc_ranges: list[dict[str, Any]] | None = None,
) -> tuple[dict[str, Any] | None, dict[str, int | str]]:
    if sequence is None:
        return None, {"status": "deferred", "reason": "no positive-duration mapping record"}
    plc_ranges = plc_ranges or []

    def pattern_sources(first: int, last: int) -> list[dict[str, Any]]:
        sources: list[dict[str, Any]] = []
        cursor = first
        while cursor < last:
            if bank_patterns is not None and cursor < bank_patterns:
                stop = min(last, bank_patterns)
                sources.append({
                    "kind": "enemy_upload",
                    "first_pattern": cursor,
                    "pattern_count": stop - cursor,
                    "last_pattern_exclusive": stop,
                    "source": "loc_27F3AE bank order Art #3, Art #1, Art #2",
                })
                cursor = stop
                continue
            match = next(
                (
                    source for source in plc_ranges
                    if source["first_pattern"] <= cursor < source["last_pattern_exclusive"]
                ),
                None,
            )
            if match is None:
                return []
            stop = min(last, match["last_pattern_exclusive"])
            sources.append({
                key: value for key, value in match.items()
                if key not in ("compressed_bytes_read",)
            } | {
                "first_pattern": cursor,
                "pattern_count": stop - cursor,
                "last_pattern_exclusive": stop,
            })
            cursor = stop
        return sources

    records = []
    for pointer in sequence["mapping_pointers"]:
        if pointer is None:
            records.append({
                "rom_offset": None,
                "record_header_bytes": MAPPING_HEADER_BYTES,
                "entry_bytes": MAPPING_ENTRY_BYTES,
                "sprite_count": 0,
                "entries": [],
                "all_entries_valid": True,
                "record_end": None,
                "hidden": True,
                "reason": "selector table stores a negative pointer: retail hides this frame",
            })
            continue
        offset = int(pointer, 16)
        try:
            record = decode_mapping_record(rom, offset, None)
            for entry in record["entries"]:
                entry["tile_sources"] = pattern_sources(
                    entry["tile_index"], entry["tile_span_end"]
                )
                entry["tile_bank_valid"] = bool(entry["tile_sources"])
            record["all_entries_valid"] = all(
                entry["tile_bank_valid"] and entry["attributes_valid"]
                for entry in record["entries"]
            )
            records.append(record)
        except BattleMappingError as exc:
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
        external = sum(
            1 for record in records for entry in record["entries"]
            for source in entry.get("tile_sources", [])
            if source["kind"] == "attack_plc"
        )
        hidden = sum(record.get("hidden", False) for record in records)
        reason = "every selected six-byte VDP mapping resolves against proven enemy or attack PLC art"
        if external:
            reason += f"; {external} sprite span(s) use routine LoadPLC1 art"
        if hidden:
            reason += f"; {hidden} selector frame(s) are retail hide records"
    elif exact:
        status = "partial"
        reason = f"{len(records) - exact} selected mapping record(s) lack proven enemy/PLC coverage or set palette bits"
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
) -> tuple[
    list[int],
    list[dict[str, Any]],
    list[dict[str, Any]],
    list[dict[str, Any]],
    list[dict[str, Any]],
]:
    objects: list[int] = []
    sound_writes: list[dict[str, Any]] = []
    mappings: list[dict[str, Any]] = []
    object_records: list[dict[str, Any]] = []
    attack_plc_loads: list[dict[str, Any]] = []
    seen: set[int] = set()

    def visit(object_id: int) -> None:
        if object_id in seen:
            return
        seen.add(object_id)
        objects.append(object_id)
        start, end = object_spans[object_pointers[object_id]]
        sound_writes.extend(_sound_writes(rom, start, end, object_id))
        evidence = decode_object_evidence(rom, start, end, object_id)
        object_records.append(evidence)
        mappings.extend(evidence["frame_sequences"])
        attack_plc_loads.extend(evidence["plc_loads"])
        for _, child in _object_refs(rom, start, end, object_pointers):
            visit(child)

    for object_id in root_ids:
        visit(object_id)
    return objects, sound_writes, mappings, object_records, attack_plc_loads


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
        objects, writes, mappings, object_records, attack_plc_loads = _walk_animation(
            rom, root_ids, object_pointers, object_spans
        )
        writes = _sound_writes(rom, start, end, None) + writes
        direct_evidence = decode_object_evidence(rom, start, end, None)
        object_records.insert(0, direct_evidence)
        mappings = direct_evidence["frame_sequences"] + mappings
        attack_plc_loads = direct_evidence["plc_loads"] + attack_plc_loads
        unique_loads: list[dict[str, Any]] = []
        seen_loads: set[tuple[Any, ...]] = set()
        for load in attack_plc_loads:
            key = (
                load.get("call_rom_offset"),
                load.get("plc_id"),
                load.get("base_pattern"),
            )
            if key not in seen_loads:
                seen_loads.add(key)
                unique_loads.append(load)
        _, plc_ranges, plc_evidence = decode_plc_art(rom, unique_loads)
        exact = next((write for write in writes if write["dispatch"] == "Sound_Index"), None)
        if exact is None:
            raise BattleAnimationError(
                f"enemy {enemy_id} ({ENEMY_SYMBOLS[enemy_id]}) has no direct Sound_Index write"
            )

        sequence_choices: list[tuple[dict[str, Any], dict[str, Any], dict[str, Any]]] = []
        for mapping in mappings:
            sequence, composition = _frame_sequence_evidence(
                rom, mapping, art_banks[enemy_id], plc_ranges
            )
            if sequence is not None:
                valid_records = sum(
                    record["all_entries_valid"] for record in sequence["mapping_records"]
                )
                external_spans = sum(
                    1 for record in sequence["mapping_records"]
                    for entry in record.get("entries", [])
                    for source in entry.get("tile_sources", [])
                    if source["kind"] == "attack_plc"
                )
                rank = {
                    "exact": 3,
                    "partial": 2,
                    "deferred": 1,
                }[composition["status"]]
                sequence_choices.append((
                    sequence,
                    composition,
                    {
                        "rank": rank,
                        "valid_records": valid_records,
                        "external_spans": external_spans,
                        "hidden_frames": sequence.get("hidden_frames", 0),
                    },
                ))
        if sequence_choices:
            frame_sequence, composition, _ = max(
                sequence_choices,
                key=lambda choice: (
                    choice[2]["rank"],
                    choice[2]["valid_records"],
                    -choice[2]["external_spans"],
                    -choice[2]["hidden_frames"],
                ),
            )
        else:
            frame_sequence = None
            frame_reason = "no fixed, variable, or selector mapping record passed structural decode"
            helper_names = sorted({
                helper for record in object_records
                for helper in record.get("helper_names", [])
            })
            assignment_count = sum(
                len(record.get("assignments", [])) for record in object_records
            )
            if "loc_25978" in helper_names:
                frame_reason = "routine constructs projectile/tile-stream state through loc_25978; no mapping timer loop consumes it"
            elif "loc_258AC" in helper_names:
                frame_reason = "routine consumes palette/effect state through loc_258AC; no enemy VDP mapping sequence is selected"
            elif attack_plc_loads:
                frame_reason = "routine uploads attack PLC art but constructs no timed enemy mapping record"
            elif helper_names:
                frame_reason = (
                    f"routine calls {', '.join(helper_names)} but no structurally valid "
                    f"fixed, variable, or selector record follows its {assignment_count} "
                    "immediate $08/$28 pointer assignment(s)"
                )
            elif assignment_count:
                frame_reason = (
                    f"routine has {assignment_count} immediate $08/$28 pointer assignment(s) "
                    "but no retail frame-helper call to consume a timed mapping record"
                )
            composition = {"status": "deferred", "reason": frame_reason}
        if frame_sequence is not None and frame_sequence.get("object_id") == 0xFFFF:
            frame_sequence["object_id"] = None
        movement = _movement_evidence(
            rom, objects, object_pointers, object_spans, frame_sequence
        )
        if frame_sequence is not None:
            routine_classification = {
                "fixed": "fixed_mapping",
                "variable": "variable_mapping",
                "selector": "selector_mapping",
            }.get(frame_sequence.get("timing_model"), "mapping_sequence")
            classification_reason = "selected sequence is consumed by a retail mapping timer helper"
        elif any(record["classification"] == "projectile_effect_graph" for record in object_records):
            routine_classification = "projectile_effect_graph"
            classification_reason = "reachable objects construct a separate projectile/effect stream instead of an enemy frame loop"
        elif any(record["classification"] == "palette_or_plane_effect" for record in object_records):
            routine_classification = "palette_or_plane_effect"
            classification_reason = "reachable objects only consume palette or plane-effect state"
        elif attack_plc_loads:
            routine_classification = "tile_upload_only"
            classification_reason = "the routine has proven LoadPLC1 art but no timed mapping consumer"
        else:
            routine_classification = "state_machine_without_sprite_mapping"
            classification_reason = "the object graph has no proven timed enemy mapping consumer"
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
            "frame_sequence_why_not": None if frame_sequence is not None else composition["reason"],
            "routine_classification": routine_classification,
            "routine_classification_reason": classification_reason,
            "object_records": object_records,
            "attack_plc": {
                "loads": unique_loads,
                "ranges": plc_ranges,
                "records": plc_evidence,
            },
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
    routine_classifications = Counter(
        animation["routine_classification"] for animation in animations
    )
    deferred_routine_classifications = Counter(
        animation["routine_classification"]
        for animation in animations
        if animation["frame_sequence"] is None
    )
    deferred_routines = {
        animation["routine_offset"] for animation in animations
        if animation["frame_sequence"] is None
    }
    object_classifications = Counter(
        record["classification"]
        for animation in animations
        for record in animation["object_records"]
    )
    attack_plc_loads = [
        load for animation in animations for load in animation["attack_plc"]["loads"]
    ]
    attack_plc_records = {
        (record.get("plc_id"), record.get("record_rom_offset"))
        for animation in animations
        for record in animation["attack_plc"]["records"]
        if record.get("status") == "exact"
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
            "routine_classifications": dict(sorted(routine_classifications.items())),
            "deferred_routine_classifications": dict(
                sorted(deferred_routine_classifications.items())
            ),
            "deferred_routine_bodies": len(deferred_routines),
            "object_classifications": dict(sorted(object_classifications.items())),
            "attack_plc_loads": len(attack_plc_loads),
            "attack_plc_distinct_records": len(attack_plc_records),
        },
    }

# Art emission is deliberately kept out of the census module.
from .battle_animation_art import emit_enemy_attack_art
