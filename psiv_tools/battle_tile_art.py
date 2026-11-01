"""The battle's tile art: Nemesis streams the pack carries as CRAM-index rows.

`battle_art_pack.emit_battle_art` writes these files. They hold no PNG: the
renderer picks each tile's palette line.
"""

from __future__ import annotations

from typing import Any

# The battle command art: two Nemesis streams `loc_7382` decompresses into VRAM
# when a battle loads (`ps4.asm:10978-10986`): the command and status icon
# tiles and the command-cursor sprite tiles. The pixels are CRAM indices; the
# plane word or sprite attribute that draws a tile picks its palette line.
COMMAND_UI_ART_NAME = "battle/art/command_ui.json"
COMMAND_UI_STREAMS = (
    # name, ROM offset, tile count, first VRAM tile (VRAM $2BC0 and $2F80)
    ("icons", 0x27D860, 30, 0x15E),
    ("cursor", 0x27DA70, 8, 0x17C),
)
# The damage-number tiles: one Nemesis stream decompressed to VRAM $BC80
# (tile $5E4) as a battle is set up (`ps4.asm:10007-10009`). The battle draws
# its damage numbers from these patterns. CRAM indices, as above.
DAMAGE_DIGIT_ART_NAME = "battle/art/damage_digits.json"
DAMAGE_DIGIT_STREAM = ("digits", 0x27D778, 28, 0x5E4)
# `loc_7634` copies fifteen words from `loc_76A4` into CRAM line 3,
# beginning at index 1 (`ps4.asm:11213-11217,11250`). Index 0 is transparent.
COMMAND_UI_PALETTE_OFFSET = 0x76A4
COMMAND_UI_PALETTE_WORDS = 15


def _stream_tiles(rom: bytes, name: str, offset: int, count: int,
                  first_tile: int) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    """One Nemesis stream's description and its tiles as CRAM-index rows.

    Each tile is eight strings of eight hex digits, one digit per pixel, the
    order the VDP reads a 4bpp pattern. A stream that does not decompress to
    its declared tile count is an extraction error, not a short pack.
    """
    from .nemesis import decompress

    data, consumed = decompress(rom, offset)
    if len(data) != count * 32:
        raise ValueError(
            f"battle art stream {name} at ${offset:X} holds {len(data) // 32} "
            f"tiles, expected {count}"
        )
    stream = {
        "name": name,
        "rom_offset": f"0x{offset:X}",
        "compressed_bytes": consumed,
        "tile_count": count,
        "first_vram_tile": first_tile,
    }
    tiles = []
    for index in range(count):
        tile = data[index * 32:(index + 1) * 32]
        rows = [
            "".join(
                f"{(tile[row * 4 + column // 2] >> (4 if column % 2 == 0 else 0)) & 15:X}"
                for column in range(8)
            )
            for row in range(8)
        ]
        tiles.append({"vram_tile": first_tile + index, "rows": rows})
    return stream, tiles


def build_command_ui(rom: bytes) -> dict[str, Any]:
    """The command icon and cursor tiles as CRAM-index rows, keyed by VRAM tile."""
    streams = []
    tiles = []
    for name, offset, count, first_tile in COMMAND_UI_STREAMS:
        stream, stream_tiles = _stream_tiles(rom, name, offset, count, first_tile)
        streams.append(stream)
        tiles.extend(stream_tiles)
    palette_bytes = rom[
        COMMAND_UI_PALETTE_OFFSET:COMMAND_UI_PALETTE_OFFSET + 2 * COMMAND_UI_PALETTE_WORDS
    ]
    if len(palette_bytes) != 2 * COMMAND_UI_PALETTE_WORDS:
        raise ValueError("command art palette at $76A4 runs past the ROM")
    return {
        "kind": "battle_command_ui_art",
        "source": "loc_7382 (ps4.asm:10978): NemDecomp to VRAM $2BC0 and $2F80",
        "palette_line_3": [
            int.from_bytes(palette_bytes[i:i + 2], "big")
            for i in range(0, len(palette_bytes), 2)
        ],
        "streams": streams,
        "tile_count": len(tiles),
        "tiles": tiles,
    }


def build_damage_digits(rom: bytes) -> dict[str, Any]:
    """The damage-number tiles as CRAM-index rows, keyed by VRAM tile."""
    stream, tiles = _stream_tiles(rom, *DAMAGE_DIGIT_STREAM)
    return {
        "kind": "battle_damage_digit_art",
        "source": "ps4.asm:10007-10009: NemDecomp to VRAM $BC80",
        "streams": [stream],
        "tile_count": len(tiles),
        "tiles": tiles,
    }
