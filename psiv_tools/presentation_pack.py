"""Extract the retail scene-panel records into a Godot presentation pack.

Panel_Create is a small VDP transaction, not a generic sprite.  The record
contains one Nemesis art pointer, two Enigma plane mappings, two CRAM pointers,
and the screen-space placement.  This tool decodes that record directly from
the cartridge and emits transparent indexed PNGs so the Godot shell can stage
Panel_Create/Panel_Destroy and commit them on DmaPlanes without inventing art.

The generated PNGs are runtime assets, not source-of-truth data: the JSON
keeps every ROM address and decoded dimension needed to audit them.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import sys

# Running the file directly puts `psiv_tools/` on sys.path rather than the
# repository root.  Keep the documented `python3 psiv_tools/...` invocation
# working as well as `python -m` and the test runner.
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from psiv_tools import gfx, png, planes
from psiv_tools.enigma import decompress as enigma_decompress
from psiv_tools.nemesis import decompress as nemesis_decompress

ROM_PANEL_TABLE = 0x07B000
PANEL_RECORD_SIZE = 0x1E
SCREEN_WIDTH = 320
SCREEN_HEIGHT = 224

# These are the records used by the transcribed story scenes.  Keeping the
# list explicit prevents accidentally treating unused ROM bytes as panels.
PANEL_IDS = tuple(range(0x1A, 0x23)) + (0x25, 0x26, 0x33, 0x34, 0x3B, 0x3C)

OPENING_ART = 0x001C_F1F2
OPENING_MAPPING = 0x001D_25BE
OPENING_PALETTE = 0x001D_2A3C
PALETTE_REQUESTS = ((0x001D_2A3C, 64), (0x001D_E0B8, 16))


def be(data: bytes, offset: int, size: int) -> int:
    return int.from_bytes(data[offset:offset + size], "big")


def _palette(data: bytes, address: int) -> list[tuple[int, int, int]]:
    raw = data[address:address + 32]
    if len(raw) != 32:
        raise ValueError(f"palette at 0x{address:06X} runs past the ROM")
    return gfx.palette_rgb(gfx.decode_palette(raw))


def _layer_pixels(
    data: bytes,
    art: list[bytes],
    mapping_address: int,
    base_tile: int,
    columns: int,
    rows: int,
    palette_address: int,
) -> tuple[int, int, bytes, list[tuple[int, int, int]]]:
    words, _ = enigma_decompress(
        data,
        mapping_address,
        base_tile=base_tile,
        max_words=columns * rows,
    )
    if len(words) != columns * rows:
        raise ValueError(
            f"mapping at 0x{mapping_address:06X} decoded {len(words)} words, "
            f"expected {columns}x{rows}"
        )
    width, height, indexed = planes.compose(
        planes.decode_cells(words),
        columns,
        art,
        art_vram_tile=base_tile & 0x07FF,
    )
    return width, height, indexed, _palette(data, palette_address)


def decode_panel(data: bytes, panel_id: int, out_dir: Path) -> dict[str, object]:
    offset = ROM_PANEL_TABLE + panel_id * PANEL_RECORD_SIZE
    record = data[offset:offset + PANEL_RECORD_SIZE]
    if len(record) != PANEL_RECORD_SIZE:
        raise ValueError(f"panel 0x{panel_id:02X} record runs past the ROM")

    destination = be(record, 0, 2)
    art_address = be(record, 2, 4)
    columns, rows, x, y = record[6:10]
    art_a = be(record, 10, 2)
    palette_a = be(record, 12, 4)
    mapping_a = be(record, 16, 4)
    art_b = be(record, 20, 2)
    palette_b = be(record, 22, 4)
    mapping_b = be(record, 26, 4)

    art_raw, art_consumed = nemesis_decompress(data, art_address)
    art_tiles = gfx.decode_tiles(art_raw)
    width_a, height_a, layer_a, colors_a = _layer_pixels(
        data, art_tiles, mapping_a, art_a, columns, rows, palette_a
    )
    if (width_a, height_a) != (columns * 8, rows * 8):
        raise ValueError("Panel A dimensions disagree with its record")

    width_b = height_b = 0
    layer_b = b""
    colors_b: list[tuple[int, int, int]] = []
    if art_b != 0xFFFF and mapping_b:
        width_b, height_b, layer_b, colors_b = _layer_pixels(
            data, art_tiles, mapping_b, art_b, columns, rows, palette_b
        )
        if (width_b, height_b) != (width_a, height_a):
            raise ValueError("Panel B dimensions disagree with Panel A")

    # Palette index 0 is transparent.  A occupies 1..16 and B occupies
    # 17..32; B is behind A, matching the VDP plane priority for these panels.
    palette = [(0, 0, 0)] + colors_a + colors_b
    palette += [(0, 0, 0)] * (33 - len(palette))
    pixels = bytearray(SCREEN_WIDTH * SCREEN_HEIGHT)

    def blit(layer: bytes, layer_width: int, layer_height: int, base: int) -> None:
        for row in range(layer_height):
            sy = y * 8 + row
            if not 0 <= sy < SCREEN_HEIGHT:
                continue
            for col in range(layer_width):
                sx = x * 8 + col
                if not 0 <= sx < SCREEN_WIDTH:
                    continue
                value = layer[row * layer_width + col] & 0x0F
                if value:
                    pixels[sy * SCREEN_WIDTH + sx] = base + value

    if layer_b:
        blit(layer_b, width_b, height_b, 17)
    blit(layer_a, width_a, height_a, 1)

    png_name = f"panel_{panel_id:02x}.png"
    (out_dir / png_name).write_bytes(
        png.encode_indexed(
            SCREEN_WIDTH,
            SCREEN_HEIGHT,
            pixels,
            palette,
            transparent=(0,),
        )
    )
    return {
        "id": panel_id,
        "png": f"presentation/panels/{png_name}",
        "record_offset": f"0x{offset:06X}",
        "art_address": f"0x{art_address:06X}",
        "art_consumed": art_consumed,
        "destination_tile": destination,
        "columns": columns,
        "rows": rows,
        "x_cells": x,
        "y_cells": y,
        "art_tile_a": art_a,
        "palette_a": f"0x{palette_a:06X}",
        "mapping_a": f"0x{mapping_a:06X}",
        "art_tile_b": art_b,
        "palette_b": f"0x{palette_b:06X}" if art_b != 0xFFFF else None,
        "mapping_b": f"0x{mapping_b:06X}" if art_b != 0xFFFF else None,
        "decoded_size": [width_a, height_a],
    }


def decode_opening_background(data: bytes, output: Path) -> dict[str, object]:
    art_raw, art_consumed = nemesis_decompress(data, OPENING_ART)
    art_tiles = gfx.decode_tiles(art_raw)
    words, mapping_consumed = enigma_decompress(
        data, OPENING_MAPPING, base_tile=0x2010, max_words=40 * 16
    )
    if len(words) != 40 * 16:
        raise ValueError(f"opening mapping decoded {len(words)} words, expected 640")
    width, height, pixels = planes.compose(
        planes.decode_cells(words), 40, art_tiles, art_vram_tile=0x10
    )
    colours = gfx.palette_rgb(
        gfx.decode_palette(data[OPENING_PALETTE:OPENING_PALETTE + 128])
    )
    output.write_bytes(
        png.encode_indexed(width, height, pixels, colours, transparent=(0,))
    )
    return {
        "art": f"0x{OPENING_ART:06X}",
        "art_consumed": art_consumed,
        "mapping": f"0x{OPENING_MAPPING:06X}",
        "mapping_consumed": mapping_consumed,
        "palette": f"0x{OPENING_PALETTE:06X}",
        "size": [width, height],
        # Measured from the oracle video surface: the narration image band is
        # rows 40..167 of the 320x224 frame (oracle/frames/opening/*.png).
        "screen_y": 40,
    }


def decode_palette_requests(data: bytes) -> list[dict[str, object]]:
    records = []
    for address, words in PALETTE_REQUESTS:
        raw = data[address : address + words * 2]
        if len(raw) != words * 2:
            raise ValueError(f"palette at 0x{address:06X} runs past the ROM")
        records.append(
            {
                "rom_addr": f"0x{address:06X}",
                "words": words,
                "raw_hex": raw.hex(),
            }
        )
    return records


def emit_presentation(rom_bytes: bytes, out_dir: str | Path) -> dict[str, object]:
    """Emit the scene-panel presentation pack; returns the written manifest."""
    root = Path(out_dir)
    panel_dir = root / "presentation" / "panels"
    panel_dir.mkdir(parents=True, exist_ok=True)
    records = [
        decode_panel(rom_bytes, panel_id, panel_dir) for panel_id in PANEL_IDS
    ]
    opening = decode_opening_background(
        rom_bytes, root / "presentation" / "opening_background.png"
    )
    manifest = {
        "format": "psiv-scene-panels-v1",
        "source_rom_sha256": hashlib.sha256(rom_bytes).hexdigest(),
        "record_table": f"0x{ROM_PANEL_TABLE:06X}",
        "record_size": PANEL_RECORD_SIZE,
        "screen": [SCREEN_WIDTH, SCREEN_HEIGHT],
        "opening_background": opening,
        "palettes": decode_palette_requests(rom_bytes),
        "panels": records,
    }
    (root / "presentation" / "panels.json").write_text(
        json.dumps(manifest, indent=2) + "\n",
        encoding="utf-8",
    )
    return manifest


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--rom", type=Path, default=Path("Phantasy Star IV (USA).md"))
    parser.add_argument("--out", type=Path, default=Path("runtime-pack"))
    args = parser.parse_args()

    manifest = emit_presentation(args.rom.read_bytes(), args.out)
    print(
        f"decoded {len(manifest['panels'])} scene panels into "
        f"{args.out / 'presentation' / 'panels'}"
    )


if __name__ == "__main__":
    main()
