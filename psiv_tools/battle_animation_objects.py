"""Retail battle-object helpers shared by the animation census and renderer.

The battle routines use four different presentation contracts.  The old scout
only understood ``loc_256AE``'s fixed record, which left variable timers,
selector tables, and attack PLC art looking like missing enemy frames.  This
module keeps those byte-level decoders separate from the census policy so the
policy can say why a record is exact, partial, hidden, or external.
"""

from __future__ import annotations

from typing import Any

from .core import be16, be32
from .gfx import decode_tile
from .nemesis import TILE_SIZE
from .nemesis import decompress as nemesis_decompress
from .nemesis import read_header as nemesis_header

FRAME_HELPERS = {
    0x025270: "loc_25270",
    0x0256AE: "loc_256AE",
    0x0256F4: "loc_256F4",
    0x0258AC: "loc_258AC",
    0x025978: "loc_25978",
}
LOAD_PLC1 = 0x00B790
PLC_OFFSET = 0x27E88A


def _u16(rom: bytes, offset: int) -> int:
    return be16(rom, offset)


def _u32(rom: bytes, offset: int) -> int:
    return be32(rom, offset)


def _s16(value: int) -> int:
    return value - 0x10000 if value & 0x8000 else value


def _s32(value: int) -> int:
    return value - 0x100000000 if value & 0x80000000 else value


def _hex(value: int) -> str:
    return f"0x{value:06X}"


def _call_target(rom: bytes, offset: int) -> int | None:
    opcode = _u16(rom, offset)
    if opcode in (0x4EB9, 0x4EF9):
        return _u32(rom, offset + 2)
    if opcode in (0x4EBA, 0x4EFA):
        return offset + 4 + _s16(_u16(rom, offset + 2))
    return None


def helper_calls(rom: bytes, start: int, end: int) -> list[dict[str, Any]]:
    """Return absolute and PC-relative calls to known battle helpers."""
    calls: list[dict[str, Any]] = []
    for offset in range(start, max(start, end - 3), 2):
        target = _call_target(rom, offset)
        if target in FRAME_HELPERS or target == LOAD_PLC1:
            calls.append({
                "rom_offset": _hex(offset),
                "target": _hex(target),
                "helper": "LoadPLC1" if target == LOAD_PLC1 else FRAME_HELPERS[target],
            })
    return calls


def _pointer_assignments(rom: bytes, start: int, end: int) -> list[dict[str, Any]]:
    assignments: list[dict[str, Any]] = []
    for offset in range(start, max(start, end - 9), 2):
        if rom[offset:offset + 2] != bytes.fromhex("29 7C"):
            continue
        field = _u16(rom, offset + 6)
        if field not in (0x08, 0x28):
            continue
        pointer = _u32(rom, offset + 2)
        assignments.append({
            "rom_offset": _hex(offset),
            "field": f"${field:02X}(a4)",
            "pointer": _hex(pointer),
            "pointer_value": pointer,
        })
    return assignments


def _latest_assignment(
    assignments: list[dict[str, Any]], field: str, call_offset: int
) -> dict[str, Any] | None:
    eligible = [
        assignment for assignment in assignments
        if assignment["field"] == field
        and int(assignment["rom_offset"], 16) <= call_offset
    ]
    return eligible[-1] if eligible else None


def _valid_pointer(rom: bytes, pointer: int) -> bool:
    return pointer < len(rom) and not pointer & 1


def _mapping_pointers(rom: bytes, pointer: int, count: int, base: int) -> list[int] | None:
    end = base + count * 4
    if count < 1 or count > 64 or end > len(rom):
        return None
    values = [_u32(rom, base + index * 4) for index in range(count)]
    if any(not _valid_pointer(rom, value) for value in values):
        return None
    return values


def _fixed_sequence(
    rom: bytes, assignment: dict[str, Any], call: dict[str, Any]
) -> dict[str, Any] | None:
    data = assignment["pointer_value"]
    if data + 2 > len(rom):
        return None
    duration = rom[data]
    count = rom[data + 1]
    pointers = _mapping_pointers(rom, data, count, data + 2)
    if pointers is None or duration == 0:
        return None
    return {
        "object_id": assignment.get("object_id"),
        "assignment_offset": assignment["rom_offset"],
        "mapping_offset": _hex(data),
        "frame_duration": duration,
        "frame_durations": [duration] * count,
        "frame_count": count,
        "total_frames": duration * count,
        "mapping_pointers": [_hex(value) for value in pointers],
        "frame_timer_helper": call["helper"],
        "timing_model": "fixed",
    }


def _variable_sequence(
    rom: bytes, assignment: dict[str, Any], call: dict[str, Any]
) -> dict[str, Any] | None:
    data = assignment["pointer_value"]
    if data + 1 > len(rom):
        return None
    count = rom[data]
    if not 1 <= count <= 64:
        return None
    durations_start = data + 1
    # Retail aligns the longword pointer list after an even duration count:
    # +1+N+(1 when N is even, otherwise 0).
    pointers_start = durations_start + count + (0 if count & 1 else 1)
    if pointers_start + count * 4 > len(rom):
        return None
    durations = list(rom[durations_start:durations_start + count])
    pointers = [_u32(rom, pointers_start + index * 4) for index in range(count)]
    if not durations or not all(durations) or any(not _valid_pointer(rom, value) for value in pointers):
        return None
    return {
        "object_id": assignment.get("object_id"),
        "assignment_offset": assignment["rom_offset"],
        "mapping_offset": _hex(data),
        "frame_duration": durations[0],
        "frame_durations": durations,
        "frame_count": count,
        "total_frames": sum(durations),
        "mapping_pointers": [_hex(value) for value in pointers],
        "frame_timer_helper": call["helper"],
        "timing_model": "variable",
        "duration_table_end": _hex(pointers_start),
    }


def _selector_sequence(
    rom: bytes,
    data_assignment: dict[str, Any],
    table_assignment: dict[str, Any],
    call: dict[str, Any],
) -> dict[str, Any] | None:
    data = data_assignment["pointer_value"]
    table = table_assignment["pointer_value"]
    if data + 1 > len(rom):
        return None
    count = rom[data]
    if not 1 <= count <= 64 or data + 1 + count * 2 > len(rom):
        return None
    durations = list(rom[data + 1:data + 1 + count])
    selectors = list(rom[data + 1 + count:data + 1 + count * 2])
    pointers: list[int | None] = []
    for selector in selectors:
        pointer_offset = table + selector
        if pointer_offset + 4 > len(rom):
            return None
        pointer = _u32(rom, pointer_offset)
        # Retail uses a negative longword to hide a sprite for this tick.
        pointers.append(None if pointer & 0x80000000 else pointer)
    if not durations or not all(durations):
        return None
    if any(pointer is not None and not _valid_pointer(rom, pointer) for pointer in pointers):
        return None
    return {
        "object_id": data_assignment.get("object_id"),
        "assignment_offset": data_assignment["rom_offset"],
        "table_assignment_offset": table_assignment["rom_offset"],
        "mapping_offset": _hex(data),
        "selector_table_offset": _hex(table),
        "frame_duration": durations[0],
        "frame_durations": durations,
        "frame_count": count,
        "total_frames": sum(durations),
        "mapping_pointers": [None if value is None else _hex(value) for value in pointers],
        "selectors": selectors,
        "frame_timer_helper": call["helper"],
        "timing_model": "selector",
        "hidden_frames": sum(value is None for value in pointers),
    }


def frame_sequences(
    rom: bytes, start: int, end: int, object_id: int | None = None
) -> list[dict[str, Any]]:
    """Decode every candidate sequence selected by the object's helper calls."""
    assignments = _pointer_assignments(rom, start, end)
    for assignment in assignments:
        assignment["object_id"] = object_id
    calls = helper_calls(rom, start, end)
    candidates: list[dict[str, Any]] = []
    for call in calls:
        call_offset = int(call["rom_offset"], 16)
        data_assignments = [
            assignment for assignment in assignments if assignment["field"] == "$08(a4)"
        ]
        for data_assignment in data_assignments:
            sequence: dict[str, Any] | None
            if call["helper"] in ("loc_256AE", "loc_256F4"):
                sequence = (
                    _fixed_sequence(rom, data_assignment, call)
                    if call["helper"] == "loc_256AE"
                    else _variable_sequence(rom, data_assignment, call)
                )
            elif call["helper"] == "loc_25270":
                table_assignments = [
                    assignment for assignment in assignments if assignment["field"] == "$28(a4)"
                ]
                sequence = next(
                    (
                        _selector_sequence(rom, data_assignment, table_assignment, call)
                        for table_assignment in table_assignments
                        if _selector_sequence(rom, data_assignment, table_assignment, call) is not None
                    ),
                    None,
                )
            else:
                sequence = None
            if sequence is None:
                continue
            sequence["helper_call_offset"] = call["rom_offset"]
            key = (
                sequence["frame_timer_helper"],
                sequence["mapping_offset"],
                tuple(sequence["mapping_pointers"]),
            )
            if not any(existing.get("_candidate_key") == key for existing in candidates):
                sequence["_candidate_key"] = key
                candidates.append(sequence)
    for candidate in candidates:
        candidate.pop("_candidate_key", None)
    return candidates


def _plc_id_before(rom: bytes, start: int, call_offset: int) -> int | None:
    for offset in range(call_offset - 2, max(start - 1, call_offset - 40), -2):
        opcode = _u16(rom, offset)
        if 0x7000 <= opcode <= 0x70FF:
            return opcode & 0xFF
        if opcode == 0x303C:
            return _u16(rom, offset + 2)
    return None


def _plc_base(rom: bytes, start: int, call_offset: int) -> tuple[int | None, str]:
    base: int | None = None
    adjustment = 0
    source = "unresolved"
    for offset in range(max(start, call_offset - 48), call_offset, 2):
        opcode = _u16(rom, offset)
        if opcode == 0x322C and _u16(rom, offset + 2) == 0x10:
            base = 0
            source = "$10(a4)"
        elif opcode == 0x0641:
            adjustment += _s16(_u16(rom, offset + 2))
    if base is None:
        return None, source
    byte_address = adjustment
    if byte_address % 0x20:
        return None, f"{source}+{adjustment:#x} (not pattern aligned)"
    return byte_address // 0x20, f"{source}+{adjustment:#x}"


def plc_loads(rom: bytes, start: int, end: int) -> list[dict[str, Any]]:
    """Extract ``LoadPLC1`` calls and the VRAM pattern base they prove."""
    loads: list[dict[str, Any]] = []
    for call in helper_calls(rom, start, end):
        if int(call["target"], 16) != LOAD_PLC1:
            continue
        call_offset = int(call["rom_offset"], 16)
        plc_id = _plc_id_before(rom, start, call_offset)
        base_pattern, base_source = _plc_base(rom, start, call_offset)
        loads.append({
            "call_rom_offset": call["rom_offset"],
            "loader": "LoadPLC1",
            "loader_rom_offset": _hex(LOAD_PLC1),
            "plc_id": plc_id,
            "base_pattern": base_pattern,
            "base_source": base_source,
        })
    return loads


def plc_record(rom: bytes, plc_id: int) -> dict[str, Any] | None:
    """Decode one retail PLC record and its Nemesis streams."""
    if plc_id < 0 or PLC_OFFSET + plc_id * 2 + 2 > len(rom):
        return None
    relative = _s16(_u16(rom, PLC_OFFSET + plc_id * 2))
    record_offset = PLC_OFFSET + relative
    if record_offset < 0 or record_offset + 2 > len(rom):
        return None
    stream_count = _u16(rom, record_offset) + 1
    if not 1 <= stream_count <= 128 or record_offset + 2 + stream_count * 4 > len(rom):
        return None
    streams = []
    for index in range(stream_count):
        stream_offset = _u32(rom, record_offset + 2 + index * 4)
        if stream_offset + 2 > len(rom):
            return None
        header = nemesis_header(rom, stream_offset)
        streams.append({
            "index": index,
            "stream_rom_offset": _hex(stream_offset),
            "tile_count": header.tile_count,
            "stream_header": f"0x{header.raw:04X}",
        })
    return {
        "plc_id": plc_id,
        "table_rom_offset": _hex(PLC_OFFSET + plc_id * 2),
        "record_rom_offset": _hex(record_offset),
        "stream_count": stream_count,
        "streams": streams,
    }


def plc_art(
    rom: bytes, loads: list[dict[str, Any]]
) -> tuple[list[bytes], list[dict[str, Any]], list[dict[str, Any]]]:
    """Decode loaded PLC patterns and return tiles, ranges, and evidence."""
    by_pattern: dict[int, bytes] = {}
    ranges: list[dict[str, Any]] = []
    evidence: list[dict[str, Any]] = []
    seen: set[tuple[int, int, int | None]] = set()
    for load in loads:
        plc_id = load.get("plc_id")
        base = load.get("base_pattern")
        if plc_id is None or base is None:
            evidence.append({**load, "status": "partial", "reason": "PLC id or VRAM base is not statically recoverable"})
            continue
        key = (plc_id, base, int(load["call_rom_offset"], 16))
        if key in seen:
            continue
        seen.add(key)
        record = plc_record(rom, plc_id)
        if record is None:
            evidence.append({**load, "status": "deferred", "reason": "PLC table record is not a valid retail record"})
            continue
        pattern = base
        load_evidence = {**load, **record, "status": "exact"}
        load_ranges = []
        for stream in record["streams"]:
            stream_offset = int(stream["stream_rom_offset"], 16)
            try:
                decompressed, consumed = nemesis_decompress(rom, stream_offset)
            except ValueError as exc:
                load_evidence["status"] = "partial"
                load_evidence["reason"] = str(exc)
                continue
            tiles = [
                decode_tile(decompressed[offset:offset + TILE_SIZE])
                for offset in range(0, len(decompressed), TILE_SIZE)
            ]
            expected = stream["tile_count"]
            if len(tiles) != expected:
                load_evidence["status"] = "partial"
                load_evidence["reason"] = f"Nemesis header says {expected} tiles, decoder produced {len(tiles)}"
                continue
            first = pattern
            for tile in tiles:
                by_pattern[pattern] = tile
                pattern += 1
            load_ranges.append({
                "kind": "attack_plc",
                "plc_id": plc_id,
                "call_rom_offset": load["call_rom_offset"],
                "record_rom_offset": record["record_rom_offset"],
                "stream_rom_offset": stream["stream_rom_offset"],
                "first_pattern": first,
                "pattern_count": len(tiles),
                "last_pattern_exclusive": pattern,
                "compressed_bytes_read": consumed,
            })
        load_evidence["ranges"] = load_ranges
        ranges.extend(load_ranges)
        evidence.append(load_evidence)
    if not by_pattern:
        return [], ranges, evidence
    max_pattern = max(by_pattern) + 1
    tiles = [bytes(TILE_SIZE) for _ in range(max_pattern)]
    for pattern, tile in by_pattern.items():
        tiles[pattern] = tile
    return tiles, ranges, evidence


def object_evidence(
    rom: bytes, start: int, end: int, object_id: int | None = None
) -> dict[str, Any]:
    """Return helper, sequence, PLC, and assignment evidence for one object."""
    assignments = _pointer_assignments(rom, start, end)
    for assignment in assignments:
        assignment.pop("pointer_value", None)
        assignment["object_id"] = object_id
    calls = helper_calls(rom, start, end)
    sequences = frame_sequences(rom, start, end, object_id)
    loads = plc_loads(rom, start, end)
    helper_names = sorted({call["helper"] for call in calls})
    if sequences:
        classification = "mapping_sequence"
        reason = "a retail frame helper consumes a proven timed mapping record"
    elif "loc_25978" in helper_names:
        classification = "projectile_effect_graph"
        reason = "the object reads a tile/DMA stream but does not call a mapping timer"
    elif "loc_258AC" in helper_names:
        classification = "palette_or_plane_effect"
        reason = "the object consumes palette/effect state rather than VDP mapping frames"
    elif loads:
        classification = "tile_upload_only"
        reason = "the object constructs or uploads attack art without a mapping loop"
    else:
        classification = "state_machine_without_sprite_mapping"
        reason = "no direct mapping timer or proven sprite-art consumer is present"
    return {
        "object_id": object_id,
        "rom_span": [_hex(start), _hex(end)],
        "helpers": calls,
        "helper_names": helper_names,
        "assignments": assignments,
        "frame_sequences": sequences,
        "plc_loads": loads,
        "classification": classification,
        "reason": reason,
    }
