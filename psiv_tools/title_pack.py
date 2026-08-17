"""Emit the retail Sega/title front door into the ignored runtime pack.

The title screen is a small, self-contained presentation surface: Nemesis art
is loaded into the same VRAM ranges as the cartridge, Enigma mappings place it
on the 320x224 frame, and the four background pieces are concatenated in the
order the retail plane dump proves.  Only provenance and hashes belong in
tracked source; the PNGs and the generated layout JSON are runtime-pack
artifacts and remain ignored.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

from . import png
from .planes import (
    MAPPINGS,
    art_tiles,
    compose,
    decode_cells,
    decode_mapping,
    named_cram,
    render,
)

TITLE_DIRECTORY = "title"
TITLE_PALETTE = "Pal_TitleScreen"

TITLE_ART_LABELS = {
    "sega_logo": "MapEni_SegaLogo",
    "title_logo": "MapEni_TitlePSTitle",
    "subtitle": "MapEni_TitleTheEndOfTheMillennium",
    "press_start": "MapEni_PressStartButton",
    "copyright": "MapEni_TitleCopyrightText",
}

BACKGROUND_PIECES = (
    ("MapEni_TitleBGLeftPart", 0),
    ("MapEni_TitleBarBGBottomPart", 48),
    ("MapEni_TitleBGRightPart", 112),
)


def _mapping(label: str) -> dict[str, Any]:
    try:
        return next(spec for spec in MAPPINGS if spec["label"] == label)
    except StopIteration as exc:
        raise ValueError(f"title mapping {label!r} is not in planes.MAPPINGS") from exc


def _write_json(path: Path, payload: dict[str, Any]) -> str:
    data = (json.dumps(payload, indent=2, sort_keys=True) + "\n").encode("utf-8")
    path.write_bytes(data)
    return hashlib.sha256(data).hexdigest()


def _decode_mapping_asset(
    rom: bytes,
    spec: dict[str, Any],
    cram: list[tuple[int, int, int]],
    path: Path,
    *,
    transparent: bool,
) -> dict[str, Any]:
    words, mapping = decode_mapping(
        rom,
        spec["rom_offset"],
        spec["label"],
        base_tile=spec["base_tile"],
        compressed_size=spec["compressed_size"],
        expected_words=spec["columns"] * spec["rows"],
    )
    tiles, art = art_tiles(rom, spec["art"])
    image = render(
        decode_cells(words),
        spec["columns"],
        tiles,
        cram,
        spec["art_vram_tile"],
        transparent_backdrop=transparent,
    )
    path.write_bytes(image)
    mapping = {
        **mapping,
        "source": spec["source"],
        "columns": spec["columns"],
        "rows": spec["rows"],
        "art_label": spec["art"],
        "art_vram_tile": f"0x{spec['art_vram_tile']:03X}",
        "art_tile_count": len(tiles),
        "spans_art_exactly": mapping["tile_range"]
        == [spec["art_vram_tile"], spec["art_vram_tile"] + len(tiles) - 1],
    }
    if "note" in spec:
        mapping["note"] = spec["note"]
    return {
        "png": str(path.relative_to(path.parents[1])),
        "png_sha256": hashlib.sha256(image).hexdigest(),
        "size_pixels": [spec["columns"] * 8, spec["rows"] * 8],
        "mapping": mapping,
        "art": art,
    }


def _background(
    rom: bytes,
    cram: list[tuple[int, int, int]],
    root: Path,
) -> tuple[dict[str, Any], dict[str, dict[str, Any]], dict[str, Any]]:
    canvas = bytearray(320 * 224)
    pieces: dict[str, dict[str, Any]] = {}
    placements = []
    for label, x in BACKGROUND_PIECES:
        spec = _mapping(label)
        words, mapping = decode_mapping(
            rom,
            spec["rom_offset"],
            spec["label"],
            base_tile=spec["base_tile"],
            compressed_size=spec["compressed_size"],
            expected_words=spec["columns"] * spec["rows"],
        )
        tiles, art = art_tiles(rom, spec["art"])
        width, height, pixels = compose(
            decode_cells(words), spec["columns"], tiles, spec["art_vram_tile"]
        )
        if height != 224 or x + width > 320:
            raise ValueError(
                f"title background piece {label} is {width}x{height} at x={x}, "
                "not a slice of the 320x224 frame"
            )
        for row in range(height):
            start = row * width
            destination = row * 320 + x
            canvas[destination:destination + width] = pixels[start:start + width]
        piece_name = label.removeprefix("MapEni_").lower() + ".png"
        piece_path = root / TITLE_DIRECTORY / piece_name
        piece_image = png.encode_indexed(width, height, pixels, cram)
        piece_path.write_bytes(piece_image)
        piece_key = label.removeprefix("MapEni_")
        pieces[piece_key] = {
            "png": str(piece_path.relative_to(root)),
            "png_sha256": hashlib.sha256(piece_image).hexdigest(),
            "size_pixels": [width, height],
            "mapping": {
                **mapping,
                "source": spec["source"],
                "columns": spec["columns"],
                "rows": spec["rows"],
                "art_label": spec["art"],
                "art_vram_tile": f"0x{spec['art_vram_tile']:03X}",
                "art_tile_count": len(tiles),
                "spans_art_exactly": mapping["tile_range"]
                == [spec["art_vram_tile"], spec["art_vram_tile"] + len(tiles) - 1],
            },
            "art": art,
        }
        placements.append({
            "piece": piece_key,
            "x_pixels": x,
            "y_pixels": 0,
            "width_pixels": width,
            "height_pixels": height,
        })

    # The title routine first places this 8-cell strip at x=6 while the
    # background transfer is still in flight.  It is a real Enigma mapping,
    # not a crop of the settled planet, and is what the oracle sees at frames
    # 300 and 400.
    transfer_path = root / TITLE_DIRECTORY / "titlebarbgtoppart.png"
    transfer = _decode_mapping_asset(
        rom,
        _mapping("MapEni_TitleBarBGTopPart"),
        cram,
        transfer_path,
        transparent=False,
    )
    image = png.encode_indexed(320, 224, bytes(canvas), cram)
    path = root / TITLE_DIRECTORY / "background.png"
    path.write_bytes(image)
    return {
        "png": str(path.relative_to(root)),
        "png_sha256": hashlib.sha256(image).hexdigest(),
        "size_pixels": [320, 224],
        "placements": placements,
        "source": "plane_b at title frame 401: left + bottom + right",
    }, pieces, transfer


def emit_title(rom_bytes: bytes, out_dir: str | Path) -> dict[str, Any]:
    """Emit title art, its composed background, and numeric provenance."""
    root = Path(out_dir)
    title_root = root / TITLE_DIRECTORY
    title_root.mkdir(parents=True, exist_ok=True)
    cram = named_cram(rom_bytes, TITLE_PALETTE)
    assets: dict[str, dict[str, Any]] = {}
    for name, label in TITLE_ART_LABELS.items():
        spec = _mapping(label)
        path = title_root / f"{name}.png"
        assets[name] = _decode_mapping_asset(
            rom_bytes, spec, cram, path, transparent=True
        )

    background, pieces, background_transfer = _background(rom_bytes, cram, root)
    layout = {
        "format_version": 1,
        "kind": "psiv_title_pack",
        "screen": {"width_pixels": 320, "height_pixels": 224},
        "palette": {
            "name": TITLE_PALETTE,
            "cram_lines": [
                [list(colour) for colour in cram[line * 16:(line + 1) * 16]]
                for line in range(4)
            ],
        },
        "assets": assets,
        "background": background,
        "background_pieces": pieces,
        "background_transfer": background_transfer,
        "placement_contract": {
            "sega_logo": {"x_cell": 12, "y_cell": 11, "width_cells": 17, "height_cells": 5},
            "title_logo": {"x_cell": 11, "y_cell": 3, "width_cells": 17, "height_cells": 13},
            "subtitle": {"x_cell": 5, "y_cell": 17, "width_cells": 29, "height_cells": 3},
            "press_start": {"x_cell": 11, "y_cell": 22, "width_cells": 18, "height_cells": 1},
            "copyright": {"x_cell": 12, "y_cell": 25, "width_cells": 17, "height_cells": 1},
        },
    }
    layout_path = title_root / "layout.json"
    layout_sha = _write_json(layout_path, layout)
    return {
        "format_version": 1,
        "directory": TITLE_DIRECTORY,
        "layout": {
            "path": str(layout_path.relative_to(root)),
            "sha256": layout_sha,
            "screen": [320, 224],
            "asset_count": len(assets),
        },
        "background": {
            "png": background["png"],
            "png_sha256": background["png_sha256"],
            "pieces": len(pieces),
        },
        "background_transfer": {
            "png": background_transfer["png"],
            "png_sha256": background_transfer["png_sha256"],
            "size_pixels": background_transfer["size_pixels"],
        },
        "assets": {
            name: {
                "png": asset["png"],
                "png_sha256": asset["png_sha256"],
                "size_pixels": asset["size_pixels"],
            }
            for name, asset in assets.items()
        },
    }
