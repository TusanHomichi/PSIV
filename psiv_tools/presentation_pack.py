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
from dataclasses import replace
from typing import Any

# Running the file directly puts `psiv_tools/` on sys.path rather than the
# repository root.  Keep the documented `python3 psiv_tools/...` invocation
# working as well as `python -m` and the test runner.
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from psiv_tools import gfx, png, planes
from psiv_tools.enigma import decompress as enigma_decompress
from psiv_tools.layouts import decode_map_palette
from psiv_tools.nemesis import decompress as nemesis_decompress
from psiv_tools.sprites.compose import TileSource, compose_frame, mapping_box
from psiv_tools.sprites.emit import sheet_json, sheet_png
from psiv_tools.sprites.field import (
    _sheet_for_sequences,
    _table_sequences,
    object_sprite,
    pal_init_line_3,
    stage_map_art,
    sprite_palette,
)
from psiv_tools.sprites.objects import facing_table_extents, scan_field_objects
from psiv_tools.sprites.records import FACINGS, decode_mapping, decode_sequence
from psiv_tools.maps import extract_maps

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

# These are the complete LoadArt sites in the transcribed scene stream.  The
# source and destination are kept here rather than inferred from the object
# record: LoadArt is a scene write, and a later object can consume the VRAM it
# leaves behind.  `map_id` is the map whose palette/tiles are live at the site.
LOAD_ART_RECORDS = (
    {
        "scene": "Event_PsycoWandChest",
        "source_rom_addr": 0x001D5DF0,
        "destination_tile": 0x02E6,
        "map_id": 0x087,
        "object_ids": [0x01FC],
    },
    {
        "scene": "Event_RuneFlaeli",
        "source_rom_addr": 0x001D628C,
        "destination_tile": 0x0179,
        "map_id": 0x19F,
        "object_ids": [0x0214],
    },
    {
        "scene": "Event_RuneFlaeli",
        "source_rom_addr": 0x001D642E,
        "destination_tile": 0x0191,
        "map_id": 0x19F,
        "object_ids": [],
    },
    {
        "scene": "Event_RuneFlaeli",
        "source_rom_addr": 0x001D6A2E,
        "destination_tile": 0x01FB,
        "map_id": 0x19F,
        "object_ids": [],
    },
    {
        "scene": "Event_RuneFlaeli",
        "source_rom_addr": 0x001D6BC8,
        "destination_tile": 0x020C,
        "map_id": 0x19F,
        "object_ids": [],
    },
    {
        "scene": "Event_Alshline",
        "source_rom_addr": 0x0012951A,
        "destination_tile": 0x03A5,
        "map_id": 0x024,
        "object_ids": [0x0188],
    },
    {
        "scene": "MeetingRika",
        "source_rom_addr": 0x001289FA,
        "destination_tile": 0x04A5,
        "map_id": 0x024,
        "object_ids": [0x0194],
    },
)

# Scene-constructed objects are keyed by the literal `(object id, art tile)`
# pair passed to ObjectAnimation.  Rika streams her ordinary field-art blob
# from work RAM; the other customers consume the preceding LoadArt upload.
TEMPORARY_OBJECT_RECORDS = (
    {
        "scene": "MeetingRika",
        "object_id": 0x0018,
        "art_tile": 0x026A,
        "symbol": "Rika",
        "source_rom_addr": 0x00292D00,
        "source_kind": "raw_field_art",
        "map_id": 0x0AC,
        "load_art": None,
    },
    {
        "scene": "MeetingRika",
        "object_id": 0x0018,
        "art_tile": 0x055C,
        "symbol": "Rika",
        "source_rom_addr": 0x00292D00,
        "source_kind": "raw_field_art",
        "map_id": 0x0AC,
        "load_art": None,
    },
    {
        "scene": "MeetingRika",
        "object_id": 0x0194,
        "art_tile": 0x04A5,
        "symbol": "Holt",
        "source_rom_addr": 0x001289FA,
        "source_kind": "nemesis",
        "map_id": 0x024,
        "load_art": (0x001289FA, 0x04A5),
    },
    {
        "scene": "Event_Dorin",
        "object_id": 0x0214,
        "art_tile": 0x0179,
        "symbol": "RuneFlaeli",
        "source_rom_addr": 0x001D628C,
        "source_kind": "nemesis",
        "map_id": 0x19F,
        "load_art": (0x001D628C, 0x0179),
    },
    {
        "scene": "Event_Alshline",
        "object_id": 0x0188,
        "art_tile": 0x03A5,
        "symbol": "Igglanova",
        "source_rom_addr": 0x0012951A,
        "source_kind": "nemesis",
        "map_id": 0x024,
        "load_art": (0x0012951A, 0x03A5),
    },
    {
        "scene": "Event_PsycoWandChest",
        "object_id": 0x01FC,
        "art_tile": 0x02E6,
        "symbol": "ChestBarrierSplinter",
        "source_rom_addr": 0x001D5DF0,
        "source_kind": "nemesis",
        "map_id": 0x087,
        "load_art": (0x001D5DF0, 0x02E6),
    },
)

SHOPKEEPER_PORTRAIT_ART = 0x0029DE1E
SHOPKEEPER_PORTRAIT_MAPPING = 0x002A2B36
SHOPKEEPER_PORTRAIT_TILE = 0x055C


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


def _map_palette(rom: bytes, record: dict[str, Any]) -> list[tuple[int, int, int]]:
    pointer = int(record["palette"]["pointer"], 16)
    return [tuple(entry["rgb"]) for entry in decode_map_palette(rom, pointer)]


def _art_blob(rom: bytes, spec: dict[str, Any]) -> tuple[bytes, int, str]:
    address = int(spec["source_rom_addr"])
    if spec["source_kind"] == "raw_field_art":
        raw = rom[address:address + 2304]
        if len(raw) != 2304:
            raise ValueError(f"raw field art at 0x{address:06X} is truncated")
        return raw, len(raw), "none"
    raw, consumed = nemesis_decompress(rom, address)
    return raw, consumed, "nemesis"


def _write_art_preview(
    rom: bytes,
    spec: dict[str, Any],
    raw: bytes,
    output: Path,
    maps: dict[int, dict[str, Any]],
    routines: dict[int, Any],
) -> dict[str, Any]:
    tiles = gfx.decode_tiles(raw)
    columns = min(16, max(1, len(tiles)))
    width, height, pixels = gfx.compose_sheet(tiles, columns)
    record = maps.get(spec["map_id"])
    if record is not None:
        palette = sprite_palette(rom, _map_palette(rom, record), 3)
    else:
        palette = pal_init_line_3(rom)
    output.write_bytes(png.encode_indexed(width, height, pixels, palette, transparent=(0,)))
    return {
        "png": f"presentation/load_art/{output.name}",
        "png_sha256": hashlib.sha256(output.read_bytes()).hexdigest(),
        "size_pixels": [width, height],
        "tile_count": len(tiles),
        "preview_palette_line": 3,
        "object_symbols": [routines[oid].symbol for oid in spec["object_ids"] if oid in routines],
    }


def _emit_load_art(
    rom: bytes,
    root: Path,
    maps: dict[int, dict[str, Any]],
    routines: dict[int, Any],
) -> list[dict[str, Any]]:
    directory = root / "presentation" / "load_art"
    directory.mkdir(parents=True, exist_ok=True)
    records = []
    for spec in LOAD_ART_RECORDS:
        raw, consumed, compression = _art_blob(rom, {**spec, "source_kind": "nemesis"})
        path = directory / f"load_art_{spec['destination_tile']:03x}.png"
        preview = _write_art_preview(rom, spec, raw, path, maps, routines)
        records.append({
            **spec,
            "source_rom_addr": f"0x{spec['source_rom_addr']:06X}",
            "destination_tile": f"0x{spec['destination_tile']:04X}",
            "map_id": f"0x{spec['map_id']:03X}",
            "compressed_size": consumed,
            "decompressed_size": len(raw),
            "compression": compression,
            "decompressed_sha256": hashlib.sha256(raw).hexdigest(),
            **preview,
        })
    return records


def _partial_sheet(
    rom: bytes,
    routine: Any,
    extents: dict[int, int],
    art: Any,
    art_tile: int,
    palette: list[tuple[int, int, int]],
) -> Any:
    """Compose a diagnostic sheet, making absent VRAM tiles transparent.

    This is only for the chest splinter effect.  Its four directional retail
    mappings name 66 patterns outside the Nemesis upload; retaining transparent
    holes is materially more useful than dropping the effect or silently
    borrowing unrelated map VRAM.  The manifest marks the result partial and
    lists the exact missing pattern range.
    """
    filled = {
        index: art.vram.tile(index) or bytes(64)
        for index in range(0x800)
    }
    source = replace(art, vram=TileSource(filled))
    wanted = _table_sequences(rom, routine.mappings_addr, extents[routine.mappings_addr])
    return _sheet_for_sequences(
        rom,
        wanted,
        source.vram,
        art_tile=art_tile,
        tile_props=routine.sprite_tile_props or 0,
        streamed=False,
        palette=palette,
        palette_line=routine.palette_line or 0,
        required=[name for _, name, _ in wanted],
    )


def _missing_patterns_for_sequences(
    rom: bytes,
    wanted: list[tuple[str, int, int]],
    source: TileSource,
    art_tile: int,
    tile_props: int,
) -> list[int]:
    """Name every VRAM pattern a sequence maps outside the staged bank."""
    missing: set[int] = set()
    for _, _, sequence_offset in wanted:
        sequence = decode_sequence(rom, sequence_offset)
        for frame in sequence.frames:
            mapping = decode_mapping(rom, frame.mapping_offset)
            _, holes = compose_frame(
                mapping,
                source,
                mapping_box(mapping),
                art_tile=art_tile,
                tile_props=tile_props,
                streamed=False,
            )
            missing.update(holes)
    return sorted(missing)


def _emit_temporary_objects(
    rom: bytes,
    root: Path,
    maps: dict[int, dict[str, Any]],
    routines: dict[int, Any],
) -> list[dict[str, Any]]:
    directory = root / "presentation" / "temporary_objects"
    directory.mkdir(parents=True, exist_ok=True)
    scanned = list(routines.values())
    extents = facing_table_extents(scanned)
    records = []
    for spec in TEMPORARY_OBJECT_RECORDS:
        routine = routines[spec["object_id"]]
        raw, consumed, compression = _art_blob(rom, spec)
        map_record = maps[spec["map_id"]]
        art = stage_map_art(rom, map_record)
        if spec["source_kind"] == "raw_field_art":
            ram = dict(art.ram)
            source = TileSource()
            source.add(0, raw)
            ram[spec["source_rom_addr"]] = source
            art = replace(art, ram=ram)
        else:
            art.vram.add(spec["art_tile"], raw)
        palette = _map_palette(rom, map_record)
        rendered = object_sprite(
            rom,
            routine,
            extents,
            art,
            palette,
            facing=0,
            art_tile=spec["art_tile"],
        )
        status = "exact"
        missing: list[int] = []
        sheet = rendered.sheet
        if sheet is None and spec["object_id"] == 0x01FC:
            wanted = _table_sequences(rom, routine.mappings_addr, extents[routine.mappings_addr])
            missing = _missing_patterns_for_sequences(
                rom,
                wanted,
                art.vram,
                spec["art_tile"],
                routine.sprite_tile_props or 0,
            )
            sheet = _partial_sheet(
                rom,
                routine,
                extents,
                art,
                spec["art_tile"],
                sprite_palette(rom, palette, routine.palette_line or 0),
            )
            status = "partial_transparent_holes"
        if sheet is None:
            raise ValueError(
                f"temporary object 0x{spec['object_id']:04X} did not render: "
                f"{rendered.reason}"
            )
        image = sheet_png(sheet)
        name = f"temporary_{spec['object_id']:04x}_{spec['art_tile']:03x}.png"
        path = directory / name
        path.write_bytes(image)
        record = {
            **spec,
            "object_id": spec["object_id"],
            "art_tile": spec["art_tile"],
            "source_rom_addr": f"0x{spec['source_rom_addr']:06X}",
            "map_id": f"0x{spec['map_id']:03X}",
            "source_compression": compression,
            "source_size": len(raw),
            "source_consumed": consumed,
            "render_status": status,
            "missing_patterns": missing,
            "load_art": (
                {
                    "source_rom_addr": f"0x{spec['load_art'][0]:06X}",
                    "destination_tile": f"0x{spec['load_art'][1]:04X}",
                }
                if spec["load_art"] is not None else None
            ),
        }
        record.pop("source_kind", None)
        record["source_rom_addr"] = f"0x{spec['source_rom_addr']:06X}"
        record.update(sheet_json(
            sheet,
            f"presentation/temporary_objects/{name}",
            image,
        ))
        records.append(record)
    return records


def _emit_generic_portrait(rom: bytes, root: Path) -> dict[str, Any]:
    raw, consumed = nemesis_decompress(rom, SHOPKEEPER_PORTRAIT_ART)
    tiles = gfx.decode_tiles(raw)
    if len(tiles) != 36:
        raise ValueError(f"shopkeeper portrait decoded {len(tiles)} tiles, expected 36")
    width, height, pixels = gfx.compose_sheet(tiles, 6)
    image = png.encode_indexed(width, height, pixels, pal_init_line_3(rom), transparent=(0,))
    directory = root / "presentation" / "portraits"
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / "shopkeeper_2.png"
    path.write_bytes(image)
    return {
        "id": "shopkeeper_2",
        "png": "presentation/portraits/shopkeeper_2.png",
        "png_sha256": hashlib.sha256(image).hexdigest(),
        "size_pixels": [width, height],
        "art": f"0x{SHOPKEEPER_PORTRAIT_ART:06X}",
        "art_consumed": consumed,
        "mapping": f"0x{SHOPKEEPER_PORTRAIT_MAPPING:06X}",
        "destination_tile": f"0x{SHOPKEEPER_PORTRAIT_TILE:04X}",
        "tile_count": len(tiles),
        "palette": f"0x{0x296300:06X}",
    }


def emit_presentation(rom_bytes: bytes, out_dir: str | Path) -> dict[str, object]:
    """Emit every currently decoded scene-presentation surface."""
    root = Path(out_dir)
    panel_dir = root / "presentation" / "panels"
    panel_dir.mkdir(parents=True, exist_ok=True)
    records = [
        decode_panel(rom_bytes, panel_id, panel_dir) for panel_id in PANEL_IDS
    ]
    opening = decode_opening_background(
        rom_bytes, root / "presentation" / "opening_background.png"
    )
    maps = {record["id"]: record for record in extract_maps(rom_bytes)["maps"]}
    routines = {routine.object_id: routine for routine in scan_field_objects(rom_bytes)}
    load_art = _emit_load_art(rom_bytes, root, maps, routines)
    temporary_objects = _emit_temporary_objects(rom_bytes, root, maps, routines)
    portraits = [_emit_generic_portrait(rom_bytes, root)]
    manifest = {
        "format": "psiv-scene-presentation-v2",
        "source_rom_sha256": hashlib.sha256(rom_bytes).hexdigest(),
        "record_table": f"0x{ROM_PANEL_TABLE:06X}",
        "record_size": PANEL_RECORD_SIZE,
        "screen": [SCREEN_WIDTH, SCREEN_HEIGHT],
        "opening_background": opening,
        "palettes": decode_palette_requests(rom_bytes),
        "panels": records,
        "load_art": load_art,
        "temporary_objects": temporary_objects,
        "portraits": portraits,
        "coverage": {
            "panel_records": len(records),
            "load_art_records": len(load_art),
            "temporary_object_keys": len(temporary_objects),
            "generic_portraits": len(portraits),
        },
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
