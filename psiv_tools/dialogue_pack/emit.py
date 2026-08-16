"""One call: `emit_dialogue(rom_bytes, out_dir)`.

Everything this writes lands under `dialogue/` and every path in the returned
fragment is relative to the pack root, so the fragment drops straight into
`psiv_tools.pack`'s manifest.
"""

from __future__ import annotations

import hashlib
from pathlib import Path
from typing import Any

from .. import png
from ..gfx import NEMESIS_ART, compose_sheet, decode_tiles, decompress_art
from ..text import extract_dialogue
from .art import (
    emit_portraits,
    emit_scroll_arrow,
    font_json,
    font_strip,
    glyph_bitmaps,
)
from .chrome import emit_window
from .common import (
    DIALOGUE_DIRECTORY,
    DIALOGUE_FORMAT_VERSION,
    FONT_JSON_NAME,
    FONT_PNG_NAME,
    MENU_FONT_PNG_NAME,
    PORTRAITS_DIRECTORY,
    PORTRAITS_NAME,
    TREES_NAME,
    WINDOW_JSON_NAME,
    WINDOW_PNG_NAME,
    write_json,
)
from .flow import CTRL_NAMES, CTRL_NOTES, Census, system_messages, tree_json
from .signatures import check_signatures
from .window import (
    BACKGROUND_COLOR_INDEX,
    GLYPH_COUNT,
    GLYPH_HEIGHT,
    GLYPH_WIDTH,
    dialogue_palette,
    interaction_json,
    window_json,
)

def emit_dialogue(rom_bytes: bytes, out_dir: str | Path) -> dict[str, Any]:
    """Emit the dialogue half of the runtime pack; return the manifest fragment.

    `out_dir` is the pack root, the same directory `psiv_tools.pack.build_pack`
    writes `manifest.json` into; everything this module writes lands under
    `dialogue/` and every path in the returned fragment is relative to that
    root, so the fragment drops straight into the manifest.

    The output holds Sega-derived pixels and text and is never committed.
    """
    root = Path(out_dir)
    signatures = check_signatures(rom_bytes)
    palette = dialogue_palette(rom_bytes)
    (root / DIALOGUE_DIRECTORY).mkdir(parents=True, exist_ok=True)

    glyphs = glyph_bitmaps(rom_bytes)
    width, height, pixels = font_strip(glyphs)
    font_image = png.encode_indexed(
        width, height, pixels, list(palette), (BACKGROUND_COLOR_INDEX,)
    )
    (root / FONT_PNG_NAME).write_bytes(font_image)
    font = font_json(rom_bytes, glyphs, palette, font_image)
    font_sha = write_json(root / FONT_JSON_NAME, font)

    scroll_arrow = emit_scroll_arrow(rom_bytes, root, palette)
    window, window_image = emit_window(rom_bytes, root, palette, scroll_arrow)
    window_sha = write_json(root / WINDOW_JSON_NAME, window)

    menu_spec = next(spec for spec in NEMESIS_ART if spec["label"] == "ArtNem_Font")
    menu_raw, _ = decompress_art(
        rom_bytes,
        int(menu_spec["rom_offset"]),
        menu_spec["label"],
        compressed_size=int(menu_spec["compressed_size"]),
    )
    menu_tiles = decode_tiles(menu_raw)
    menu_width, menu_height, menu_pixels = compose_sheet(menu_tiles, 16)
    menu_image = png.encode_indexed(menu_width, menu_height, menu_pixels, list(palette))
    (root / MENU_FONT_PNG_NAME).write_bytes(menu_image)
    menu_sha = hashlib.sha256(menu_image).hexdigest()

    portraits, portrait_bytes = emit_portraits(rom_bytes, root, palette)
    portraits_sha = write_json(root / PORTRAITS_NAME, portraits)

    decoded = extract_dialogue(rom_bytes)
    messages = system_messages(rom_bytes)
    census = Census()
    trees = []
    for tree in decoded["trees"]:
        record, entries = tree_json(tree)
        trees.append(record)
        for entry in entries:
            census.add_entry(tree["tree"], entry, entry["segments"], entry["pages"])

    tree_window = window_json(scroll_arrow)
    payload = {
        "format_version": DIALOGUE_FORMAT_VERSION,
        "kind": "dialogue_trees",
        "region": decoded["region"],
        "tree_count": len(trees),
        "entry_count": decoded["total_entries"],
        "charset": "dialogue",
        "window": tree_window,
        "interaction": interaction_json(),
        "control_codes": {
            f"0x{code:02X}": {"ctrl": name, "note": CTRL_NOTES[code]}
            for code, name in sorted(CTRL_NAMES.items())
        },
        "font": FONT_JSON_NAME,
        "chrome": WINDOW_JSON_NAME,
        "system_messages": messages,
        "portraits": PORTRAITS_NAME,
        "trees": trees,
    }
    trees_sha = write_json(root / TREES_NAME, payload)

    return {
        "format_version": DIALOGUE_FORMAT_VERSION,
        "directory": DIALOGUE_DIRECTORY,
        "trees": {
            "path": TREES_NAME,
            "sha256": trees_sha,
            "tree_count": len(trees),
            "entry_count": decoded["total_entries"],
            "system_message_count": messages["count"],
        },
        "font": {
            "path": FONT_JSON_NAME,
            "sha256": font_sha,
            "png": FONT_PNG_NAME,
            "png_sha256": font["png_sha256"],
            "glyph_count": GLYPH_COUNT,
            "glyph_size": [GLYPH_WIDTH, GLYPH_HEIGHT],
        },
        "chrome": {
            "path": WINDOW_JSON_NAME,
            "sha256": window_sha,
            "png": WINDOW_PNG_NAME,
            "png_sha256": window["png_sha256"],
            "tile_count": window["tile"]["count"],
            "roles": {
                name: role["tile"] for name, role in sorted(window["roles"].items())
            },
            "rect": window["text_window"]["rect"],
            "scroll_arrow": scroll_arrow,
        },
        "menu_font": {
            "path": MENU_FONT_PNG_NAME,
            "png_sha256": menu_sha,
            "tile_count": len(menu_tiles),
        },
        "portraits": {
            "path": PORTRAITS_NAME,
            "sha256": portraits_sha,
            "directory": PORTRAITS_DIRECTORY,
            "count": portraits["count"],
            "distinct_art": portraits["distinct_art"],
            "bytes": portrait_bytes,
        },
        "window": tree_window,
        "signatures": signatures,
        "census": census.to_json(
            (entry["id"] for entry in portraits["portraits"]),
            (glyph["byte"] for glyph in font["glyphs"] if glyph["blank"]),
        ),
    }
