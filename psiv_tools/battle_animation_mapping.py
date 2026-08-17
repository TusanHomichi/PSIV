"""Decode the retail six-byte VDP sprite mapping records."""

from __future__ import annotations

from typing import Any

from .core import be16, be32

MAPPING_HEADER_BYTES = 1
MAPPING_ENTRY_BYTES = 6


class BattleMappingError(ValueError):
    """A pointer does not describe a bounded retail sprite mapping."""


def _hex(value: int) -> str:
    return f"0x{value:06X}"


def _u16(rom: bytes, offset: int) -> int:
    if offset < 0 or offset + 2 > len(rom):
        raise BattleMappingError(f"word at {_hex(offset)} runs past the ROM")
    return be16(rom, offset)


def _signed_byte(value: int) -> int:
    return value - 0x100 if value & 0x80 else value


def decode_mapping_record(
    rom: bytes, offset: int, bank_patterns: int | None = None
) -> dict[str, Any]:
    """Decode one count-minus-one plus six-byte-entry mapping record."""
    if offset < 0 or offset + MAPPING_HEADER_BYTES > len(rom):
        raise BattleMappingError(f"mapping record at {_hex(offset)} is outside the ROM")
    sprite_count = rom[offset] + 1
    record_end = offset + MAPPING_HEADER_BYTES + sprite_count * MAPPING_ENTRY_BYTES
    if sprite_count > 64 or record_end > len(rom):
        raise BattleMappingError(
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
            # Palette bits select a different CRAM line in the VDP.  The
            # current line-relative PNG surface cannot claim those pixels.
            "attributes_valid": not bool(tile_word & 0x6000),
        })
    return {
        "rom_offset": _hex(offset),
        "record_header_bytes": MAPPING_HEADER_BYTES,
        "entry_bytes": MAPPING_ENTRY_BYTES,
        "sprite_count": sprite_count,
        "entries": entries,
        "all_entries_valid": all(
            entry["tile_bank_valid"] and entry["attributes_valid"]
            for entry in entries
        ),
        "record_end": _hex(record_end),
    }
