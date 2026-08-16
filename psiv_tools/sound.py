"""Raw sound extraction for the verified Phantasy Star IV retail ROM.

The cartridge uses a modified SMPS 68k driver.  This module preserves that
format instead of pretending it is MIDI: records, relative pointers, command
bytes, voices, envelopes and the Z80 DAC tables are emitted exactly as found.
The decoder is only a verifier and census tool.  A transcribed interpreter is
the consumer of the raw records.

The source layout is pinned to ``reference/ps4disasm/sound`` and to the ROM
hash in :mod:`psiv_tools.core`.  The reference directory is intentionally not
the authority at runtime; the byte anchors and region hashes below make a
clone with a drifting disassembly or ROM fail closed.
"""

from __future__ import annotations

from collections import Counter, deque
import hashlib
import json
from pathlib import Path
from typing import Any, Iterable

from .core import EXPECTED_SHA256, EXPECTED_SIZE
from .warps import PackError
from .sound_defs import (
    ANCHORS,
    COMMAND_SPECS,
    EXPECTED_SHORT_BYTES,
    KNOWN_REGIONS,
    META_SPECS,
    MUSIC_NAMES,
    SFX_NAMES,
    SPECIAL_NAMES,
)


SOUND_FORMAT_VERSION = 1
SOUND_DIRECTORY = "sound"


class SoundError(PackError):
    """The sound data cannot be decoded or its provenance is not verified."""


def _hx(value: int, width: int = 2) -> str:
    return f"0x{value:0{width}X}"


def _rom_offset(value: int) -> str:
    return _hx(value, 6)


def _be16(data: bytes, offset: int) -> int:
    if offset < 0 or offset + 2 > len(data):
        raise SoundError(f"read past ROM at {_rom_offset(offset)}")
    return int.from_bytes(data[offset:offset + 2], "big")


def _be32(data: bytes, offset: int) -> int:
    if offset < 0 or offset + 4 > len(data):
        raise SoundError(f"read past ROM at {_rom_offset(offset)}")
    return int.from_bytes(data[offset:offset + 4], "big")


def _s16(value: int) -> int:
    return value - 0x10000 if value & 0x8000 else value


def _range(data: bytes, start: int, end: int, label: str) -> bytes:
    if start < 0 or end < start or end > len(data):
        raise SoundError(
            f"{label} range {_rom_offset(start)}:{_rom_offset(end)} is outside the ROM"
        )
    return data[start:end]


def _verify_provenance(rom: bytes) -> None:
    if len(rom) != EXPECTED_SIZE:
        raise SoundError(
            f"sound extraction requires {EXPECTED_SIZE} ROM bytes; got {len(rom)}"
        )
    digest = hashlib.sha256(rom).hexdigest()
    if digest != EXPECTED_SHA256:
        raise SoundError(
            "sound extraction refuses an unverified ROM clone: "
            f"expected {EXPECTED_SHA256}, got {digest}"
        )
    for offset, expected_hex in ANCHORS:
        expected = bytes.fromhex(expected_hex)
        actual = _range(rom, offset, offset + len(expected), "anchor")
        if actual != expected:
            raise SoundError(
                f"sound provenance anchor mismatch at {_rom_offset(offset)}: "
                f"expected {expected.hex()}, got {actual.hex()}"
            )
    for name, label, start, end, expected_sha in KNOWN_REGIONS:
        actual = _range(rom, start, end, label)
        digest = hashlib.sha256(actual).hexdigest()
        if digest != expected_sha:
            raise SoundError(
                f"sound region {label} ({name}) changed at "
                f"{_rom_offset(start)}:{_rom_offset(end)}"
            )
        expected_short = EXPECTED_SHORT_BYTES.get(name)
        if expected_short is not None and actual != bytes.fromhex(expected_short):
            raise SoundError(f"sound constant {label} bytes do not match the disassembly")


def _read_pointer_table(
    rom: bytes,
    offset: int,
    count: int,
    end_limit: int,
    label: str,
) -> tuple[list[int], bytes]:
    raw = _range(rom, offset, offset + count * 4, label)
    pointers = [_be32(raw, i * 4) for i in range(count)]
    if any(pointer < offset + count * 4 or pointer >= end_limit for pointer in pointers):
        raise SoundError(f"{label} contains a pointer outside its data region")
    if pointers != sorted(pointers) or len(set(pointers)) != len(pointers):
        raise SoundError(f"{label} pointers are not strictly increasing")
    return pointers, raw


def _raw_file(raw_files: dict[str, bytes], path: str, data: bytes) -> None:
    if path in raw_files:
        raise SoundError(f"duplicate raw output path {path}")
    raw_files[path] = bytes(data)


def _region_json(
    rom: bytes,
    raw_files: dict[str, bytes],
    name: str,
    source_label: str,
    start: int,
    end: int,
    raw_path: str,
) -> dict[str, Any]:
    data = _range(rom, start, end, source_label)
    _raw_file(raw_files, raw_path, data)
    return {
        "source_label": source_label,
        "rom_offset": _rom_offset(start),
        "size_bytes": len(data),
        "raw_file": raw_path,
        "raw_sha256": hashlib.sha256(data).hexdigest(),
        "raw_hex": data.hex(),
    }


def _env_span(rom: bytes, start: int, limit: int, kind: str, index: int) -> tuple[int, int, int]:
    """Return an envelope's terminal span and terminal control byte.

    84h is a two-byte modulation command and therefore cannot terminate a
    modulation envelope.  All other control bytes are terminal for the
    current table's stream; the driver handles their looping/reset behavior.
    """
    pos = start
    while pos < limit:
        value = rom[pos]
        if kind == "modulation" and value == 0x84:
            pos += 2
            if pos > limit:
                raise SoundError(f"modulation envelope {index} truncates CHG_MULT")
            continue
        if value in (0x80, 0x81, 0x82, 0x83):
            return start, pos + 1, value
        pos += 1
    raise SoundError(f"{kind} envelope {index} has no terminal control byte")


def _format_decode_counts(counts: Counter[int]) -> dict[str, int]:
    return {f"{value:02X}": counts[value] for value in sorted(counts)}


class _DecodedTrack:
    def __init__(self) -> None:
        self.command_counts: Counter[int] = Counter()
        self.meta_counts: Counter[int] = Counter()
        self.fm_instruments: set[int] = set()
        self.psg_instruments: set[int] = set()
        self.covered: set[int] = set()
        self.byte_values: set[int] = set()
        self.states = 0
        self.max_call_depth = 0
        self.has_track_end = False


def _decode_track(
    rom: bytes,
    start: int,
    end: int,
    track_id: str,
    flow_start: int | None = None,
) -> _DecodedTrack:
    """Walk every statically reachable driver path in one track.

    The state includes the GOSUB return stack.  F7 branches include both the
    taken and exhausted paths because its counter is runtime state; a later
    interpreter can therefore see every command byte present in the record.
    """
    floor = start if flow_start is None else flow_start
    if not floor <= start < end:
        raise SoundError(f"{track_id}: empty or inverted track range")
    result = _DecodedTrack()
    work: deque[tuple[int, tuple[int, ...]]] = deque([(start, ())])
    visited: set[tuple[int, tuple[int, ...]]] = set()
    while work:
        pos, call_stack = work.popleft()
        state = (pos, call_stack)
        if state in visited:
            continue
        visited.add(state)
        result.states += 1
        result.max_call_depth = max(result.max_call_depth, len(call_stack))
        if len(call_stack) > 32:
            raise SoundError(f"{track_id}: GOSUB stack exceeds static safety limit at {_rom_offset(pos)}")
        if pos < floor or pos >= end:
            raise SoundError(f"{track_id}: control flow leaves track at {_rom_offset(pos)}")

        opcode = rom[pos]
        successors: list[tuple[int, tuple[int, ...]]] = []
        if opcode < 0x80:
            size = 1
        elif opcode < 0xE0:
            # A note/rest is followed by a delay only when the next byte has
            # bit 7 clear.  A high byte is the next token; the 68k driver backs
            # its pointer up after looking at it.
            if pos + 1 >= end:
                raise SoundError(f"{track_id}: note at {_rom_offset(pos)} lacks lookahead")
            size = 2 if rom[pos + 1] < 0x80 else 1
            successors.append((pos + size, call_stack))
        elif opcode == 0xFF:
            result.command_counts[opcode] += 1
            if pos + 1 >= end:
                raise SoundError(f"{track_id}: FF at {_rom_offset(pos)} lacks meta opcode")
            meta = rom[pos + 1]
            if meta not in META_SPECS:
                raise SoundError(
                    f"{track_id}: undocumented FF meta {_hx(meta)} at {_rom_offset(pos + 1)}"
                )
            result.meta_counts[meta] += 1
            if pos + 2 >= end:
                raise SoundError(f"{track_id}: FF 00 at {_rom_offset(pos)} lacks payload")
            size = 3 if rom[pos + 2] == 0 else 7
            successors.append((pos + size, call_stack))
        else:
            spec = COMMAND_SPECS.get(opcode)
            if spec is None:
                raise SoundError(f"{track_id}: undocumented command {_hx(opcode)} at {_rom_offset(pos)}")
            result.command_counts[opcode] += 1
            size = int(spec["length"])
            if opcode == 0xEF:
                if pos + 1 >= end:
                    raise SoundError(f"{track_id}: EF at {_rom_offset(pos)} lacks instrument")
                result.fm_instruments.add(rom[pos + 1])
            elif opcode == 0xF5:
                if pos + 1 >= end:
                    raise SoundError(f"{track_id}: F5 at {_rom_offset(pos)} lacks instrument")
                result.psg_instruments.add(rom[pos + 1])
            if opcode == 0xF2:
                result.has_track_end = True
            elif opcode == 0xF6:
                target = pos + 2 + _s16(_be16(rom, pos + 1))
                successors.append((target, call_stack))
            elif opcode == 0xF7:
                target = pos + 4 + _s16(_be16(rom, pos + 3))
                successors.append((target, call_stack))
                successors.append((pos + size, call_stack))
            elif opcode == 0xF8:
                if len(call_stack) >= 32:
                    raise SoundError(f"{track_id}: GOSUB stack exceeds static safety limit")
                target = pos + 2 + _s16(_be16(rom, pos + 1))
                successors.append((target, call_stack + (pos + size,)))
            elif opcode == 0xF9:
                if not call_stack:
                    raise SoundError(f"{track_id}: RETURN without GOSUB at {_rom_offset(pos)}")
                successors.append((call_stack[-1], call_stack[:-1]))
            else:
                successors.append((pos + size, call_stack))

        if pos + size > end:
            raise SoundError(
                f"{track_id}: {_hx(opcode)} at {_rom_offset(pos)} truncates at "
                f"{_rom_offset(end)}"
            )
        result.covered.update(range(pos, pos + size))
        result.byte_values.update(rom[pos:pos + size])
        for successor, successor_stack in successors:
            if successor < floor or successor >= end:
                raise SoundError(
                    f"{track_id}: {_hx(opcode)} at {_rom_offset(pos)} targets "
                    f"outside track at {_rom_offset(successor)}"
                )
            work.append((successor, successor_stack))
    return result


def decode_track(rom: bytes, start: int, end: int, track_id: str = "track") -> dict[str, Any]:
    """Public JSON-shaped verifier for one raw sequence range."""
    decoded = _decode_track(rom, start, end, track_id)
    return {
        "track_id": track_id,
        "rom_offset": _rom_offset(start),
        "end_offset_exclusive": _rom_offset(end),
        "reachable_instruction_states": decoded.states,
        "reachable_byte_count": len(decoded.covered),
        "command_opcodes_seen": [f"0x{value:02X}" for value in sorted(decoded.command_counts)],
        "command_counts": _format_decode_counts(decoded.command_counts),
        "meta_opcodes_seen": [f"0x{value:02X}" for value in sorted(decoded.meta_counts)],
        "meta_counts": _format_decode_counts(decoded.meta_counts),
        "fm_instruments": sorted(decoded.fm_instruments),
        "psg_instruments": sorted(decoded.psg_instruments),
        "sequence_byte_values_seen": [f"0x{value:02X}" for value in sorted(decoded.byte_values)],
        "max_call_depth": decoded.max_call_depth,
        "has_track_end": decoded.has_track_end,
    }


def _voice_json(rom: bytes, start: int, index: int) -> dict[str, Any]:
    raw = _range(rom, start + index * 25, start + (index + 1) * 25, "FM voice")
    return {
        "index": index,
        "rom_offset": _rom_offset(start + index * 25),
        "size_bytes": 25,
        "raw_hex": raw.hex(),
        "algorithm_feedback": raw[0],
        "operator_registers": list(raw[1:21]),
        "operator_total_levels": list(raw[21:25]),
    }


def _voice_table_shape(
    rom: bytes,
    voice_start: int,
    record_end: int,
    label: str,
    highest_used: int,
) -> tuple[int, int]:
    """Return all voice definitions and the legal alignment pad size."""
    voice_bytes = record_end - voice_start
    voice_count, padding = divmod(voice_bytes, 25)
    if padding and rom[record_end - padding:record_end] != b"\x00" * padding:
        raise SoundError(f"{label}: nonzero bytes after the FM voice table")
    if highest_used >= voice_count:
        raise SoundError(f"{label}: used FM voice index exceeds the full voice table")
    return voice_count, padding


def _track_json(
    rom: bytes,
    record_start: int,
    record_end: int,
    sequence_start: int,
    track_start: int,
    track_index: int,
    track_kind: str,
    track_id: str,
    initial: dict[str, Any],
    raw_descriptor_hex: str,
) -> tuple[dict[str, Any], _DecodedTrack]:
    decoded = _decode_track(rom, track_start, record_end, track_id, sequence_start)
    return {
        "track_id": track_id,
        "kind": track_kind,
        "index": track_index,
        "source": {
            "rom_offset": _rom_offset(track_start),
            "record_relative_offset": track_start - record_start,
            "record_end_exclusive": _rom_offset(record_end),
            "raw_descriptor_hex": raw_descriptor_hex,
        },
        "initial": initial,
        "instruments_used": sorted(decoded.fm_instruments),
        "psg_instruments_used": sorted(decoded.psg_instruments),
        "decode": {
            "reachable_instruction_states": decoded.states,
            "reachable_byte_count": len(decoded.covered),
            "command_opcodes_seen": [f"0x{value:02X}" for value in sorted(decoded.command_counts)],
            "command_counts": _format_decode_counts(decoded.command_counts),
            "meta_opcodes_seen": [f"0x{value:02X}" for value in sorted(decoded.meta_counts)],
            "meta_counts": _format_decode_counts(decoded.meta_counts),
            "sequence_byte_values_seen": [
                f"0x{value:02X}" for value in sorted(decoded.byte_values)
            ],
            "max_call_depth": decoded.max_call_depth,
            "has_track_end": decoded.has_track_end,
        },
    }, decoded


def _record_file_name(prefix: str, sound_id: int, name: str) -> str:
    return f"raw/{prefix}/{sound_id:02X}_{name}.bin"


def _record_header_common(rom: bytes, start: int, end: int, label: str) -> tuple[int, int, int]:
    if start + 4 > end:
        raise SoundError(f"{label}: header is truncated")
    voice_start = start + _be16(rom, start)
    if voice_start < start or voice_start > end:
        raise SoundError(f"{label}: voice table pointer leaves record")
    tick_multiplier = rom[start + 2]
    track_count = rom[start + 3]
    if track_count == 0:
        raise SoundError(f"{label}: zero track count")
    return voice_start, tick_multiplier, track_count


def _music_record(
    rom: bytes,
    raw_files: dict[str, bytes],
    sound_id: int,
    name: str,
    start: int,
    end: int,
    map_usage_count: int,
) -> tuple[dict[str, Any], list[_DecodedTrack]]:
    label = f"music {_hx(sound_id)} {name}"
    if start + 6 > end:
        raise SoundError(f"{label}: header is truncated")
    voice_start = start + _be16(rom, start)
    fm_count = rom[start + 2]
    psg_count = rom[start + 3]
    tick_multiplier = rom[start + 4]
    tempo_reload = rom[start + 5]
    if fm_count == 0 and psg_count == 0:
        raise SoundError(f"{label}: no channels")
    descriptor_end = start + 6 + fm_count * 4 + psg_count * 6
    if descriptor_end > end:
        raise SoundError(f"{label}: channel descriptors exceed record")
    if voice_start < descriptor_end or voice_start > end:
        raise SoundError(f"{label}: voice table pointer is not after the header")

    tracks: list[dict[str, Any]] = []
    decoded_tracks: list[_DecodedTrack] = []
    descriptor_pos = start + 6
    for index in range(fm_count):
        track_start = start + _be16(rom, descriptor_pos)
        initial_word = _be16(rom, descriptor_pos + 2)
        if track_start < descriptor_end or track_start >= end:
            raise SoundError(f"{label}: FM track {index} pointer leaves record")
        track, decoded = _track_json(
            rom, start, end, descriptor_end, track_start, index, "fm", f"music:{sound_id:02X}:fm:{index}",
            {"transpose": initial_word >> 8, "volume": initial_word & 0xFF},
            rom[descriptor_pos:descriptor_pos + 4].hex(),
        )
        tracks.append(track)
        decoded_tracks.append(decoded)
        descriptor_pos += 4
    for index in range(psg_count):
        track_start = start + _be16(rom, descriptor_pos)
        initial_word = _be16(rom, descriptor_pos + 2)
        mod_env = rom[descriptor_pos + 4]
        vol_env = rom[descriptor_pos + 5]
        if track_start < descriptor_end or track_start >= end:
            raise SoundError(f"{label}: PSG track {index} pointer leaves record")
        track, decoded = _track_json(
            rom, start, end, descriptor_end, track_start, index, "psg", f"music:{sound_id:02X}:psg:{index}",
            {
                "transpose": initial_word >> 8,
                "volume": initial_word & 0xFF,
                "modulation_envelope": mod_env,
                "volume_envelope": vol_env,
            },
            rom[descriptor_pos:descriptor_pos + 6].hex(),
        )
        tracks.append(track)
        decoded_tracks.append(decoded)
        descriptor_pos += 6

    max_voice = max((max(decoded.fm_instruments, default=-1) for decoded in decoded_tracks), default=-1)
    voice_count, voice_padding = _voice_table_shape(rom, voice_start, end, label, max_voice)
    voices = [_voice_json(rom, voice_start, index) for index in range(voice_count)]

    raw_path = _record_file_name("music", sound_id, name)
    raw = _range(rom, start, end, label)
    _raw_file(raw_files, raw_path, raw)
    return {
        "id": sound_id,
        "id_hex": _hx(sound_id),
        "symbol": name,
        "record": {
            "rom_offset": _rom_offset(start),
            "end_offset_exclusive": _rom_offset(end),
            "size_bytes": len(raw),
            "raw_file": raw_path,
            "raw_sha256": hashlib.sha256(raw).hexdigest(),
        },
        "header": {
            "voice_table_relative": _be16(rom, start),
            "voice_table_rom_offset": _rom_offset(voice_start),
            "fm_channel_count": fm_count,
            "psg_channel_count": psg_count,
            "tick_multiplier": tick_multiplier,
            "tempo_reload": tempo_reload,
            "header_size_bytes": descriptor_end - start,
            "voice_count": voice_count,
            "voice_padding_bytes": voice_padding,
        },
        "map_usage_count": map_usage_count,
        "voices": voices,
        "tracks": tracks,
    }, decoded_tracks


def _sfx_record(
    rom: bytes,
    raw_files: dict[str, bytes],
    sound_id: int,
    name: str,
    start: int,
    end: int,
    special: bool,
) -> tuple[dict[str, Any], list[_DecodedTrack]]:
    label = f"{'special ' if special else ''}SFX {_hx(sound_id)} {name}"
    voice_start, tick_multiplier, track_count = _record_header_common(rom, start, end, label)
    descriptor_end = start + 4 + track_count * 6
    if descriptor_end > end or voice_start < descriptor_end:
        raise SoundError(f"{label}: descriptor/header layout is invalid")

    tracks: list[dict[str, Any]] = []
    decoded_tracks: list[_DecodedTrack] = []
    descriptor_pos = start + 4
    for index in range(track_count):
        channel_word = _be16(rom, descriptor_pos)
        track_start = start + _be16(rom, descriptor_pos + 2)
        initial_word = _be16(rom, descriptor_pos + 4)
        if track_start < descriptor_end or track_start >= end:
            raise SoundError(f"{label}: track {index} pointer leaves record")
        track, decoded = _track_json(
            rom, start, end, descriptor_end, track_start, index, "sfx", f"sfx:{sound_id:02X}:{index}",
            {
                "channel_word": channel_word,
                "channel_byte": channel_word & 0xFF,
                "transpose": initial_word >> 8,
                "volume": initial_word & 0xFF,
            },
            rom[descriptor_pos:descriptor_pos + 6].hex(),
        )
        tracks.append(track)
        decoded_tracks.append(decoded)
        descriptor_pos += 6

    max_voice = max((max(decoded.fm_instruments, default=-1) for decoded in decoded_tracks), default=-1)
    voice_count, voice_padding = _voice_table_shape(rom, voice_start, end, label, max_voice)
    voices = [_voice_json(rom, voice_start, index) for index in range(voice_count)]

    prefix = "special" if special else "sfx"
    raw_path = _record_file_name(prefix, sound_id, name)
    raw = _range(rom, start, end, label)
    _raw_file(raw_files, raw_path, raw)
    return {
        "id": sound_id,
        "id_hex": _hx(sound_id),
        "symbol": name,
        "record": {
            "rom_offset": _rom_offset(start),
            "end_offset_exclusive": _rom_offset(end),
            "size_bytes": len(raw),
            "raw_file": raw_path,
            "raw_sha256": hashlib.sha256(raw).hexdigest(),
        },
        "header": {
            "voice_table_relative": _be16(rom, start),
            "voice_table_rom_offset": _rom_offset(voice_start),
            "tick_multiplier": tick_multiplier,
            "track_count": track_count,
            "header_size_bytes": descriptor_end - start,
            "voice_count": voice_count,
            "voice_padding_bytes": voice_padding,
        },
        "voices": voices,
        "tracks": tracks,
    }, decoded_tracks


def _driver_data(rom: bytes, raw_files: dict[str, bytes]) -> dict[str, Any]:
    regions: dict[str, Any] = {}
    for name, label, start, end, _ in KNOWN_REGIONS:
        path = f"raw/driver/{name}.bin"
        regions[name] = _region_json(rom, raw_files, name, label, start, end, path)

    general_start, general_end = 0xD1A60, 0xD1A80
    general = [_be32(rom, general_start + i * 4) for i in range(8)]
    mod_ptrs = [_be32(rom, 0xD1A80 + i * 4) for i in range(8)]
    vol_ptrs = [_be32(rom, 0xD1B44 + i * 4) for i in range(10)]
    env_limit = 0xD1C40
    modulation = []
    for index, pointer in enumerate(mod_ptrs):
        start, end, terminal = _env_span(rom, pointer, env_limit, "modulation", index)
        path = f"raw/driver/modulation_envelope_{index:02d}.bin"
        _raw_file(raw_files, path, _range(rom, start, end, "modulation envelope"))
        modulation.append({
            "index": index,
            "rom_offset": _rom_offset(start),
            "size_bytes": end - start,
            "terminal": _hx(terminal),
            "raw_file": path,
            "raw_sha256": hashlib.sha256(rom[start:end]).hexdigest(),
            "raw_hex": rom[start:end].hex(),
        })
    volume = []
    for index, pointer in enumerate(vol_ptrs):
        start, end, terminal = _env_span(rom, pointer, env_limit, "volume", index)
        path = f"raw/driver/volume_envelope_{index:02d}.bin"
        _raw_file(raw_files, path, _range(rom, start, end, "volume envelope"))
        volume.append({
            "index": index,
            "rom_offset": _rom_offset(start),
            "size_bytes": end - start,
            "terminal": _hx(terminal),
            "raw_file": path,
            "raw_sha256": hashlib.sha256(rom[start:end]).hexdigest(),
            "raw_hex": rom[start:end].hex(),
        })

    dac_table = []
    dac_raw = _range(rom, 0xD1A3E, 0xD1A60, "zBankTbl")
    for index in range(len(dac_raw) // 2):
        bank = dac_raw[index * 2]
        sample = dac_raw[index * 2 + 1]
        dac_table.append({"sound_id": 0x81 + index, "bank": bank, "sample": sample})

    return {
        "provenance": {
            "rom_sha256": hashlib.sha256(rom).hexdigest(),
            "rom_size_bytes": len(rom),
            "source_tree": "reference/ps4disasm/sound",
            "source_files": [
                "DefCFlag.txt", "DefDrv.txt", "Notes.txt", "Pointers.txt",
                "ps4.sound_driver.asm", "ps4.dac_driver.asm",
            ],
            "clone_drift_guard": "ROM hash, structural anchors, and fixed region hashes are verified before extraction",
        },
        "timing": {
            "update_sound_rom_offset": _rom_offset(0xD0008),
            "global_tempo": {
                "music_header_byte": 5,
                "reload_register": "$2(a6)",
                "counter_register": "$1(a6)",
                "semantics": "Each UpdateSound call decrements the counter; zero reloads it from the header byte and increments $E on each active channel.",
                "zero_behavior": "A zero reload disables the global tempo tick path.",
            },
            "channel_tick_multiplier": {
                "music_header_byte": 4,
                "sfx_header_byte": 2,
                "register": "$2(a5)",
                "semantics": "TickMultiplier multiplies a note/delay byte by this value before loading channel $E/$F.",
            },
            "note_tokens": {
                "delay": "bytes 0x00-0x7F consume one byte and load that delay",
                "rest_or_note": "0x80 is rest; 0x81-0xDF are notes; a following byte below 0x80 is consumed as delay, while a high-bit byte is the next token",
            },
        },
        "command_vocabulary": {
            "primary": {f"0x{value:02X}": {"opcode": value, **spec} for value, spec in COMMAND_SPECS.items()},
            "meta": {f"0x{value:02X}": {"opcode": value, **spec} for value, spec in META_SPECS.items()},
            "primary_count": len(COMMAND_SPECS),
            "meta_count": len(META_SPECS),
            "total_documented_entries": len(COMMAND_SPECS) + len(META_SPECS),
        },
        "loop_commands": {
            "F6": "signed big-endian relative word at opcode+1; target is opcode+2+offset",
            "F7": "index byte, count byte, signed big-endian relative word at opcode+3; taken target is opcode+4+offset and exhausted fallthrough is opcode+5",
            "F8": "signed big-endian relative word at opcode+1; target is opcode+2+offset and return address is opcode+3",
            "F9": "pop the most recent F8 return address; an empty stack is undecodable",
        },
        "fixed_constants": {
            "fm_init_bytes": list(rom[0xD07D2:0xD07D9]),
            "psg_init_bytes": list(rom[0xD07DA:0xD07DD]),
            "spc_fm3_registers": list(rom[0xD0490:0xD0498]),
            "fm_algorithm_operator_masks": list(rom[0xD1290:0xD1298]),
            "fm_operator_registers": list(rom[0xD1308:0xD131C]),
            "fm_volume_registers": list(rom[0xD131C:0xD1320]),
            "fm3_frequency_values": list(rom[0xD1510:0xD1518]),
            "fm_frequencies_big_endian_words": [
                _be16(rom, 0xD0DE2 + i * 2) for i in range(12)
            ],
            "psg_frequency_table_raw_hex": rom[0xD0FA2:0xD102E].hex(),
            "pan_animation_sequences": {
                "0": rom[0xD053C:0xD053E].hex(),
                "1": rom[0xD053E:0xD0541].hex(),
                "2": rom[0xD0541:0xD0545].hex(),
            },
        },
        "regions": regions,
        "envelopes": {
            "modulation_control_bytes": {"0x80": "RESET", "0x81": "HOLD", "0x82": "LOOP", "0x83": "STOP", "0x84": "CHG_MULT then one multiplier byte"},
            "volume_control_bytes": {"0x80": "RESET", "0x81": "HOLD", "0x82": "JUMP2IDX then one index byte", "0x83": "OFF"},
            "modulation": modulation,
            "volume": volume,
        },
        "pointer_tables": {
            "driver_block": {
                "rom_offset": _rom_offset(general_start),
                "pointers": [_rom_offset(pointer) for pointer in general],
                "meaning": [
                    "SndPriorities", "SpcSFXPtrs", "MusicPtrs", "SFXPtrs",
                    "ModEnvPtrs", "VolEnvPtrs", "regular SFX id base", "UpdateSound",
                ],
            },
            "music": {"rom_offset": _rom_offset(0xD1C40), "id_first": 0x81, "count": 52},
            "sfx": {"rom_offset": _rom_offset(0xDE4B6), "id_first": 0xB5, "count": 67},
            "special_sfx": {"rom_offset": _rom_offset(0xDE5C2), "id_first": 0xF8, "count": 3},
        },
        "dac": {
            "z80_driver": regions["z80_dac_driver"],
            "bank_table": regions["dac_bank_table"],
            "bank_table_entries": dac_table,
            "bank_rom_bases": [_rom_offset(value) for value in (0xE0000, 0xE8000, 0xF0000, 0xF8000)],
            "table_entry_size_bytes": 8,
            "table_word_encoding": "ROM stores each Z80 little-endian word as the two bytes read by ld e,(hl); ld d,(hl+1); swap bytes when presenting the address/length.",
        },
    }


def _dac_samples(rom: bytes, raw_files: dict[str, bytes], driver: dict[str, Any]) -> list[dict[str, Any]]:
    samples = []
    bank_specs = ((0, 0xE0000, 10), (1, 0xE8000, 2), (2, 0xF0000, 4), (3, 0xF8000, 1))
    table = driver["dac"]["bank_table_entries"]
    for bank, base, count in bank_specs:
        table_path = f"raw/dac/bank_{bank:02X}_table.bin"
        table_raw = _range(rom, base, base + count * 8, "DAC bank table")
        _raw_file(raw_files, table_path, table_raw)
        bank_samples = []
        for local_index in range(count):
            entry = table[sum(spec[2] for spec in bank_specs[:bank]) + local_index]
            entry_offset = base + local_index * 8
            stored_start = _be16(rom, entry_offset)
            stored_length = _be16(rom, entry_offset + 2)
            z80_start = ((stored_start & 0xFF) << 8) | (stored_start >> 8)
            length = ((stored_length & 0xFF) << 8) | (stored_length >> 8)
            if not 0x8000 <= z80_start <= 0xFFFF or length == 0:
                raise SoundError(f"DAC sample {_hx(entry['sound_id'])} has invalid Z80 range")
            sample_offset = base + (z80_start - 0x8000)
            sample_end = sample_offset + length
            if sample_end > base + 0x8000:
                raise SoundError(f"DAC sample {_hx(entry['sound_id'])} leaves bank {_hx(bank)}")
            raw_path = f"raw/dac/bank_{bank:02X}_sample_{local_index:02X}.bin"
            raw = _range(rom, sample_offset, sample_end, "DAC sample")
            _raw_file(raw_files, raw_path, raw)
            bank_samples.append({
                "sound_id": entry["sound_id"],
                "bank": bank,
                "sample": local_index,
                "table_rom_offset": _rom_offset(entry_offset),
                "z80_start": _hx(z80_start, 4),
                "length_bytes": length,
                "rom_offset": _rom_offset(sample_offset),
                "raw_file": raw_path,
                "raw_sha256": hashlib.sha256(raw).hexdigest(),
            })
        samples.extend(bank_samples)
        driver["dac"].setdefault("banks", []).append({
            "bank": bank,
            "rom_base": _rom_offset(base),
            "entry_count": count,
            "table_raw_file": table_path,
            "samples": bank_samples,
        })
    return samples


def _extract(rom: bytes, map_music_ids: Iterable[int] | None) -> tuple[dict[str, Any], dict[str, bytes]]:
    _verify_provenance(rom)
    raw_files: dict[str, bytes] = {}
    driver = _driver_data(rom, raw_files)
    _dac_samples(rom, raw_files, driver)

    music_ptrs, _ = _read_pointer_table(rom, 0xD1C40, 52, 0xDE4B6, "MusicPtrs")
    sfx_ptrs, _ = _read_pointer_table(rom, 0xDE4B6, 67, 0xDF68A, "SFXPtrs")
    special_ptrs, _ = _read_pointer_table(rom, 0xDE5C2, 3, 0xDF78C, "SpcSFXPtrs")
    map_usage = Counter(int(value) for value in (map_music_ids or ()))

    music = []
    music_decoded: list[_DecodedTrack] = []
    for index, start in enumerate(music_ptrs):
        end = music_ptrs[index + 1] if index + 1 < len(music_ptrs) else 0xDE4B6
        record, decoded = _music_record(
            rom, raw_files, 0x81 + index, MUSIC_NAMES[index], start, end,
            map_usage.get(0x81 + index, 0),
        )
        music.append(record)
        music_decoded.extend(decoded)

    regular_sfx = []
    regular_decoded: list[_DecodedTrack] = []
    for index, start in enumerate(sfx_ptrs):
        end = sfx_ptrs[index + 1] if index + 1 < len(sfx_ptrs) else 0xDF68A
        record, decoded = _sfx_record(
            rom, raw_files, 0xB5 + index, SFX_NAMES[index], start, end, False
        )
        regular_sfx.append(record)
        regular_decoded.extend(decoded)

    special_sfx = []
    special_decoded: list[_DecodedTrack] = []
    for index, start in enumerate(special_ptrs):
        end = special_ptrs[index + 1] if index + 1 < len(special_ptrs) else 0xDF78C
        record, decoded = _sfx_record(
            rom, raw_files, 0xF8 + index, SPECIAL_NAMES[index], start, end, True
        )
        special_sfx.append(record)
        special_decoded.extend(decoded)

    all_decoded = music_decoded + regular_decoded + special_decoded
    command_seen: set[int] = set()
    meta_seen: set[int] = set()
    command_counts: Counter[int] = Counter()
    meta_counts: Counter[int] = Counter()
    sequence_values: set[int] = set()
    max_depth = 0
    no_end = 0
    instrument_census = []
    for record_group in (music, regular_sfx, special_sfx):
        for record in record_group:
            for track in record["tracks"]:
                instrument_census.append({
                    "track_id": track["track_id"],
                    "instruments_used": track["instruments_used"],
                    "psg_instruments_used": track["psg_instruments_used"],
                })
    for decoded in all_decoded:
        command_seen.update(decoded.command_counts)
        meta_seen.update(decoded.meta_counts)
        command_counts.update(decoded.command_counts)
        meta_counts.update(decoded.meta_counts)
        sequence_values.update(decoded.byte_values)
        max_depth = max(max_depth, decoded.max_call_depth)
        no_end += int(not decoded.has_track_end)

    census = {
        "music_record_count": len(music),
        "regular_sfx_record_count": len(regular_sfx),
        "special_sfx_record_count": len(special_sfx),
        "music_track_count": len(music_decoded),
        "regular_sfx_track_count": len(regular_decoded),
        "special_sfx_track_count": len(special_decoded),
        "track_count": len(all_decoded),
        "command_opcodes_seen": [f"0x{value:02X}" for value in sorted(command_seen)],
        "command_vocabulary_size": len(command_seen),
        "documented_primary_command_count": len(COMMAND_SPECS),
        "meta_opcodes_seen": [f"0x{value:02X}" for value in sorted(meta_seen)],
        "command_counts": _format_decode_counts(command_counts),
        "meta_counts": _format_decode_counts(meta_counts),
        "sequence_byte_values_seen": [f"0x{value:02X}" for value in sorted(sequence_values)],
        "max_gosub_call_depth": max_depth,
        "tracks_without_static_track_end": no_end,
        "instruments_used_per_track": instrument_census,
        "unresolved": [],
    }
    payload = {
        "format_version": SOUND_FORMAT_VERSION,
        "kind": "raw_sound",
        "rom": {"sha256": hashlib.sha256(rom).hexdigest(), "size_bytes": len(rom)},
        "driver": driver,
        "music": {"id_first": 0x81, "records": music},
        "sfx": {
            "regular": {"id_first": 0xB5, "records": regular_sfx},
            "special": {"id_first": 0xF8, "records": special_sfx},
        },
        "census": census,
        "unresolved": [],
    }
    return payload, raw_files


def extract_sound(rom: bytes, map_music_ids: Iterable[int] | None = None) -> dict[str, Any]:
    """Extract verified sound records without writing files."""
    return _extract(rom, map_music_ids)[0]


def _write_json(path: Path, payload: dict[str, Any]) -> str:
    data = (json.dumps(payload, indent=2, sort_keys=True) + "\n").encode("utf-8")
    path.write_bytes(data)
    return hashlib.sha256(data).hexdigest()


def emit_sound(
    rom: bytes,
    out_dir: str | Path,
    map_music_ids: Iterable[int] | None = None,
) -> dict[str, Any]:
    """Write ``sound/`` and return the pack manifest's sound fragment."""
    payload, raw_files = _extract(rom, map_music_ids)
    root = Path(out_dir) / SOUND_DIRECTORY
    root.mkdir(parents=True, exist_ok=True)

    driver_payload = {
        "format_version": SOUND_FORMAT_VERSION,
        "kind": "sound_driver",
        "rom": payload["rom"],
        "driver": payload["driver"],
    }
    music_payload = {
        "format_version": SOUND_FORMAT_VERSION,
        "kind": "music_records",
        "rom": payload["rom"],
        "records": payload["music"],
    }
    sfx_payload = {
        "format_version": SOUND_FORMAT_VERSION,
        "kind": "sfx_records",
        "rom": payload["rom"],
        "records": payload["sfx"],
    }
    driver_sha = _write_json(root / "driver.json", driver_payload)
    music_sha = _write_json(root / "music.json", music_payload)
    sfx_sha = _write_json(root / "sfx.json", sfx_payload)

    file_inventory: list[dict[str, Any]] = []
    for rel, data in sorted(raw_files.items()):
        path = root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        file_inventory.append({
            "file": f"{SOUND_DIRECTORY}/{rel}",
            "size_bytes": len(data),
            "sha256": hashlib.sha256(data).hexdigest(),
        })
    for name, sha in (("driver", driver_sha), ("music", music_sha), ("sfx", sfx_sha)):
        file_inventory.append({
            "file": f"{SOUND_DIRECTORY}/{name}.json",
            "size_bytes": (root / f"{name}.json").stat().st_size,
            "sha256": sha,
        })
    file_inventory.sort(key=lambda entry: entry["file"])

    index = {
        "format_version": SOUND_FORMAT_VERSION,
        "kind": "sound_index",
        "rom": payload["rom"],
        "driver": {"file": "sound/driver.json", "sha256": driver_sha},
        "music": {"file": "sound/music.json", "sha256": music_sha},
        "sfx": {"file": "sound/sfx.json", "sha256": sfx_sha},
        "census": payload["census"],
        "unresolved": payload["unresolved"],
        "files": file_inventory,
    }
    index_sha = _write_json(root / "index.json", index)
    index_size = (root / "index.json").stat().st_size
    file_inventory.append({
        "file": "sound/index.json",
        "size_bytes": index_size,
        "sha256": index_sha,
    })
    file_inventory.sort(key=lambda entry: entry["file"])

    census = payload["census"]
    return {
        "directory": SOUND_DIRECTORY,
        "index": "sound/index.json",
        "index_sha256": index_sha,
        "driver": {"file": "sound/driver.json", "sha256": driver_sha},
        "music": {"file": "sound/music.json", "sha256": music_sha, "records": len(payload["music"]["records"])},
        "sfx": {
            "file": "sound/sfx.json",
            "sha256": sfx_sha,
            "regular_records": len(payload["sfx"]["regular"]["records"]),
            "special_records": len(payload["sfx"]["special"]["records"]),
        },
        "track_count": census["track_count"],
        "command_vocabulary_size": census["command_vocabulary_size"],
        "command_opcodes_seen": census["command_opcodes_seen"],
        "unresolved": census["unresolved"],
        "files": file_inventory,
    }
