#!/usr/bin/env python3
"""Decode retail battle layout state captured by ``psiv_oracle``.

The Genesis display is a set of pattern-name words, not a picture.  This
decoder keeps that distinction intact: the visible plane matrices remain
lossless numbers, while chrome, text, residual tile runs, SAT entries, and
CRAM are convenient structured views over the same bytes.  Some captures also
carry the VDP Window name table as an optional 64x32 region; that plane is
decoded separately because it has no work-RAM buffer in the current host dump.
"""

from __future__ import annotations

import argparse
import json
from collections import Counter
from pathlib import Path
from typing import Any, Iterable

from psiv_tools.text import WINDOW_CHARSET
from oracle.scroll_state import (
    ScrollDecodeError,
    decode_scroll_state,
)


CELL_PIXELS = 8
PLANE_WIDTH_CELLS = 64
PLANE_HEIGHT_CELLS = 32
VISIBLE_WIDTH_CELLS = 40
VISIBLE_HEIGHT_CELLS = 28
SPRITE_ENTRY_BYTES = 8
SPRITE_ENTRY_COUNT = 0x280 // SPRITE_ENTRY_BYTES

# The battle uses the same three window-art patterns and flip vocabulary as
# dialogue.  Palette and priority are deliberately not part of the key: the
# battle calls Battle_SetupWindow with palette line 3, while dialogue's
# extracted role table records the otherwise identical words on line 2.
CHROME_ROLES: dict[tuple[int, bool, bool], str] = {
    (0x6E9, False, False): "corner_top_left",
    (0x6EA, False, False): "edge_top",
    (0x6E9, True, False): "corner_top_right",
    (0x6F3, False, False): "edge_left",
    (0x680, False, False): "fill",
    (0x6F3, True, False): "edge_right",
    (0x6E9, False, True): "corner_bottom_left",
    (0x6EA, False, True): "edge_bottom",
    (0x6E9, True, True): "corner_bottom_right",
    (0x6F4, False, False): "status_separator_top",
    (0x6F5, False, False): "status_separator",
    (0x6F4, False, True): "status_separator_bottom",
}

TOP_EDGE_ROLES = {"edge_top", "status_separator_top"}
BOTTOM_EDGE_ROLES = {"edge_bottom", "status_separator_bottom"}

# Retail title mappings are copied into the visible plane at these exact
# cells.  Keeping the rectangles here, beside the decoder rather than in a
# screenshot fixture, makes the layout contract useful to both the oracle
# reports and the runtime-pack emitter.
TITLE_ELEMENTS: tuple[dict[str, Any], ...] = (
    {
        "name": "sega_logo",
        "cell_rect": {"x": 12, "y": 11, "width": 17, "height": 5},
        "base_tile": 0x10,
        "columns": 17,
        "rows": 5,
        "pattern_range": [0x10, 0x55],
        "palette_line": 0,
    },
    {
        "name": "title_logo",
        "cell_rect": {"x": 11, "y": 3, "width": 17, "height": 13},
        "base_tile": 0x10,
        "columns": 17,
        "rows": 13,
        "pattern_range": [0x10, 0x91],
        "palette_line": 0,
    },
    {
        "name": "subtitle",
        "cell_rect": {"x": 5, "y": 17, "width": 29, "height": 3},
        "base_tile": 0x92,
        "columns": 29,
        "rows": 3,
        "pattern_range": [0x92, 0xC9],
        "palette_line": 0,
    },
    {
        "name": "press_start",
        "cell_rect": {"x": 11, "y": 22, "width": 18, "height": 1},
        "base_tile": 0xD8,
        "columns": 18,
        "rows": 1,
        "pattern_range": [0xD8, 0xE2],
        "palette_line": 2,
    },
    {
        "name": "copyright",
        "cell_rect": {"x": 12, "y": 25, "width": 17, "height": 1},
        "base_tile": 0xCA,
        "columns": 17,
        "rows": 1,
        "pattern_range": [0xCA, 0xD6],
        "palette_line": 1,
    },
)


class LayoutDecodeError(ValueError):
    """The state file cannot be decoded as the expected retail layout."""


def _read_state(path: Path) -> dict[str, Any]:
    try:
        state = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as exc:
        raise LayoutDecodeError(f"cannot read state {path}: {exc}") from exc
    if state.get("kind") != "psiv_oracle_state":
        raise LayoutDecodeError(f"{path}: not a psiv_oracle_state document")
    if state.get("format_version") != 1:
        raise LayoutDecodeError(f"{path}: unsupported state format")
    return state


def _region_bytes(state: dict[str, Any], name: str, expected_size: int) -> bytes:
    try:
        region = state["regions"][name]
        raw = bytes.fromhex(region["bytes_hex"])
    except (KeyError, TypeError, ValueError) as exc:
        raise LayoutDecodeError(f"state region {name!r} is malformed") from exc
    if len(raw) != expected_size or region.get("size_bytes") != expected_size:
        raise LayoutDecodeError(
            f"state region {name!r} is {len(raw)} bytes, expected {expected_size}"
        )
    return raw


def _optional_region(
    state: dict[str, Any], names: tuple[str, ...], expected_size: int
) -> tuple[str, bytes, dict[str, Any]] | None:
    """Read the first present alias for an optional state region."""
    regions = state.get("regions", {})
    for name in names:
        if name not in regions:
            continue
        region = regions[name]
        if not isinstance(region, dict):
            raise LayoutDecodeError(f"state region {name!r} is malformed")
        try:
            raw = bytes.fromhex(region["bytes_hex"])
        except (KeyError, TypeError, ValueError) as exc:
            raise LayoutDecodeError(f"state region {name!r} is malformed") from exc
        if len(raw) != expected_size or region.get("size_bytes") != expected_size:
            raise LayoutDecodeError(
                f"state region {name!r} is {len(raw)} bytes, expected {expected_size}"
            )
        return name, raw, region
    return None


def decode_pattern_word(word: int) -> dict[str, Any]:
    """Decode one VDP pattern-name word."""
    return {
        "word": f"0x{word:04X}",
        "pattern": word & 0x07FF,
        "pattern_hex": f"0x{word & 0x07FF:03X}",
        "palette_line": (word >> 13) & 0x03,
        "priority": bool(word & 0x8000),
        "flip_h": bool(word & 0x0800),
        "flip_v": bool(word & 0x1000),
    }


def _word_matrix(raw: bytes) -> list[list[int]]:
    if len(raw) != 0x1000:
        raise LayoutDecodeError(f"plane buffer is {len(raw)} bytes, expected 4096")
    words = [int.from_bytes(raw[i : i + 2], "big") for i in range(0, len(raw), 2)]
    if len(words) != PLANE_WIDTH_CELLS * PLANE_HEIGHT_CELLS:
        raise LayoutDecodeError("plane buffer does not contain a 64x32 word grid")
    return [
        words[row * PLANE_WIDTH_CELLS : (row + 1) * PLANE_WIDTH_CELLS]
        for row in range(PLANE_HEIGHT_CELLS)
    ]


def _role(word: int) -> str | None:
    decoded = decode_pattern_word(word)
    return CHROME_ROLES.get(
        (decoded["pattern"], decoded["flip_h"], decoded["flip_v"])
    )


def _cell(word: int, x: int, y: int) -> dict[str, Any]:
    return {"cell_x": x, "cell_y": y, **decode_pattern_word(word)}


def _rectangle_kind(x: int, y: int, width: int, height: int) -> str:
    known = {
        (2, 1, 12, 3): "enemy_name_window",
        (3, 5, 8, 7): "command_window",
        (2, 21, 36, 6): "status_strip",
        (10, 16, 20, 5): "victory_window",
    }
    return known.get((x, y, width, height), "window")


def _edge_matches(roles: list[list[str | None]], x: int, y: int,
                  x2: int, y2: int) -> bool:
    if roles[y][x] != "corner_top_left" or roles[y][x2] != "corner_top_right":
        return False
    if roles[y2][x] != "corner_bottom_left" or roles[y2][x2] != "corner_bottom_right":
        return False
    if any(roles[y][xx] not in TOP_EDGE_ROLES for xx in range(x + 1, x2)):
        return False
    if any(roles[y2][xx] not in BOTTOM_EDGE_ROLES for xx in range(x + 1, x2)):
        return False
    if any(roles[yy][x] != "edge_left" for yy in range(y + 1, y2)):
        return False
    if any(roles[yy][x2] != "edge_right" for yy in range(y + 1, y2)):
        return False
    return True


def _find_rectangles(words: list[list[int]]) -> list[dict[str, Any]]:
    roles = [[_role(word) for word in row[:VISIBLE_WIDTH_CELLS]]
             for row in words[:VISIBLE_HEIGHT_CELLS]]
    rectangles: list[dict[str, Any]] = []
    seen: set[tuple[int, int, int, int]] = set()
    for y in range(VISIBLE_HEIGHT_CELLS - 2):
        for x in range(VISIBLE_WIDTH_CELLS - 2):
            if roles[y][x] != "corner_top_left":
                continue
            for y2 in range(y + 2, VISIBLE_HEIGHT_CELLS):
                for x2 in range(x + 2, VISIBLE_WIDTH_CELLS):
                    if not _edge_matches(roles, x, y, x2, y2):
                        continue
                    key = (x, y, x2, y2)
                    if key in seen:
                        continue
                    seen.add(key)
                    width, height = x2 - x + 1, y2 - y + 1
                    rectangles.append({
                        "kind": _rectangle_kind(x, y, width, height),
                        "x_cell": x,
                        "y_cell": y,
                        "width_cells": width,
                        "height_cells": height,
                        "pixel_rect": {
                            "x": x * CELL_PIXELS,
                            "y": y * CELL_PIXELS,
                            "width": width * CELL_PIXELS,
                            "height": height * CELL_PIXELS,
                        },
                        "interior": {
                            "x_cell": x + 1,
                            "y_cell": y + 1,
                            "width_cells": width - 2,
                            "height_cells": height - 2,
                        },
                    })
    rectangles.sort(key=lambda rect: (rect["y_cell"], rect["x_cell"]))
    return rectangles


def _interior_cells(rect: dict[str, Any]) -> Iterable[tuple[int, int]]:
    x = rect["x_cell"] + 1
    y = rect["y_cell"] + 1
    width = rect["width_cells"] - 2
    height = rect["height_cells"] - 2
    for yy in range(y, y + height):
        for xx in range(x, x + width):
            yield xx, yy


def _find_text_runs(words: list[list[int]], rectangles: list[dict[str, Any]]) -> list[dict[str, Any]]:
    runs: list[dict[str, Any]] = []
    for rect_index, rect in enumerate(rectangles):
        ix = rect["x_cell"] + 1
        iy = rect["y_cell"] + 1
        iw = rect["width_cells"] - 2
        ih = rect["height_cells"] - 2
        for y in range(iy, iy + ih):
            glyph_x = [
                x for x in range(ix, ix + iw)
                if (words[y][x] & 0x07FF) - 0x680 in WINDOW_CHARSET
                and WINDOW_CHARSET[(words[y][x] & 0x07FF) - 0x680] != " "
            ]
            if not glyph_x:
                continue
            groups: list[list[int]] = []
            for x in glyph_x:
                if not groups or x - groups[-1][-1] > 2:
                    groups.append([x])
                else:
                    groups[-1].append(x)
            for group in groups:
                start, end = group[0], group[-1]
                cells = [_cell(words[y][x], x, y) for x in range(start, end + 1)]
                chars = [
                    WINDOW_CHARSET[(words[y][x] & 0x07FF) - 0x680]
                    for x in range(start, end + 1)
                ]
                text = "".join(chars)
                if not text.strip():
                    continue
                runs.append({
                    "window_index": rect_index,
                    "window_kind": rect["kind"],
                    "text": text,
                    "cell_x": start,
                    "cell_y": y,
                    "length_cells": end - start + 1,
                    "pixel_x": start * CELL_PIXELS,
                    "pixel_y": y * CELL_PIXELS,
                    "charset": "window",
                    "font_vram_base": "0x680",
                    "cells": cells,
                })
    runs.sort(key=lambda run: (run["cell_y"], run["cell_x"]))
    return runs


def _find_unframed_text_runs(
    words: list[list[int]], existing: list[dict[str, Any]]
) -> list[dict[str, Any]]:
    """Find Window-charset runs when the VDP plane has no detected border."""
    occupied = {
        (cell["cell_x"], cell["cell_y"])
        for run in existing
        for cell in run["cells"]
    }
    runs: list[dict[str, Any]] = []
    for y in range(VISIBLE_HEIGHT_CELLS):
        glyph_x = [
            x
            for x in range(VISIBLE_WIDTH_CELLS)
            if (words[y][x] & 0x07FF) - 0x680 in WINDOW_CHARSET
            and WINDOW_CHARSET[(words[y][x] & 0x07FF) - 0x680] != " "
            and (x, y) not in occupied
        ]
        if not glyph_x:
            continue
        groups: list[list[int]] = []
        for x in glyph_x:
            if not groups or x - groups[-1][-1] > 2:
                groups.append([x])
            else:
                groups[-1].append(x)
        for group in groups:
            start, end = group[0], group[-1]
            cells = [_cell(words[y][x], x, y) for x in range(start, end + 1)]
            chars = [
                WINDOW_CHARSET[(words[y][x] & 0x07FF) - 0x680]
                for x in range(start, end + 1)
            ]
            text = "".join(chars)
            if not text.strip():
                continue
            runs.append({
                "window_index": None,
                "window_kind": "vdp_window_plane",
                "text": text,
                "cell_x": start,
                "cell_y": y,
                "length_cells": end - start + 1,
                "pixel_x": start * CELL_PIXELS,
                "pixel_y": y * CELL_PIXELS,
                "charset": "window",
                "font_vram_base": "0x680",
                "cells": cells,
            })
    runs.sort(key=lambda run: (run["cell_y"], run["cell_x"]))
    return runs


def _window_chrome_cells(words: list[list[int]], rectangles: list[dict[str, Any]],
                         text_runs: list[dict[str, Any]]) -> tuple[set[tuple[int, int]], list[dict[str, Any]]]:
    text_cells = {
        (cell["cell_x"], cell["cell_y"])
        for run in text_runs
        for cell in run["cells"]
    }
    chrome_cells: set[tuple[int, int]] = set()
    special: list[dict[str, Any]] = []
    for rect_index, rect in enumerate(rectangles):
        x0, y0 = rect["x_cell"], rect["y_cell"]
        x1 = x0 + rect["width_cells"]
        y1 = y0 + rect["height_cells"]
        for y in range(y0, y1):
            for x in range(x0, x1):
                role = _role(words[y][x])
                border = x in (x0, x1 - 1) or y in (y0, y1 - 1)
                if not role or (not border and (x, y) in text_cells):
                    continue
                if border or role == "fill" or role.startswith("status_separator"):
                    chrome_cells.add((x, y))
                if role.startswith("status_separator"):
                    special.append({
                        "window_index": rect_index,
                        "role": role,
                        **_cell(words[y][x], x, y),
                    })
    return chrome_cells, special


def _tile_runs(words: list[list[int]], excluded: set[tuple[int, int]]) -> list[dict[str, Any]]:
    runs: list[dict[str, Any]] = []
    for y in range(VISIBLE_HEIGHT_CELLS):
        x = 0
        while x < VISIBLE_WIDTH_CELLS:
            if words[y][x] == 0 or (x, y) in excluded:
                x += 1
                continue
            word = words[y][x]
            end = x + 1
            while (
                end < VISIBLE_WIDTH_CELLS
                and words[y][end] == word
                and (end, y) not in excluded
            ):
                end += 1
            runs.append({
                "cell_x": x,
                "cell_y": y,
                "length_cells": end - x,
                "pixel_x": x * CELL_PIXELS,
                "pixel_y": y * CELL_PIXELS,
                **decode_pattern_word(word),
            })
            x = end
    return runs


def decode_plane(
    raw: bytes,
    name: str,
    *,
    allow_unframed_text: bool = False,
    buffer_address: str | None = None,
    source_region: str | None = None,
) -> dict[str, Any]:
    words = _word_matrix(raw)
    rectangles = _find_rectangles(words)
    text_runs = _find_text_runs(words, rectangles)
    if allow_unframed_text:
        text_runs.extend(_find_unframed_text_runs(words, text_runs))
        text_runs.sort(key=lambda run: (run["cell_y"], run["cell_x"]))
    chrome_cells, special_cells = _window_chrome_cells(words, rectangles, text_runs)
    text_cells = {
        (cell["cell_x"], cell["cell_y"])
        for run in text_runs
        for cell in run["cells"]
    }
    tile_runs = _tile_runs(words, chrome_cells | text_cells)
    role_counts = Counter(
        role for row in words[:VISIBLE_HEIGHT_CELLS]
        for word in row[:VISIBLE_WIDTH_CELLS]
        if (role := _role(word)) is not None
    )
    visible = [row[:VISIBLE_WIDTH_CELLS] for row in words[:VISIBLE_HEIGHT_CELLS]]
    decoded = {
        "name": name,
        "buffer_address": buffer_address
        or ("0xFFFF8000" if name == "plane_a" else "0xFFFF9000"),
        "grid": {
            "width_cells": PLANE_WIDTH_CELLS,
            "height_cells": PLANE_HEIGHT_CELLS,
            "row_stride_bytes": 0x80,
            "visible_width_cells": VISIBLE_WIDTH_CELLS,
            "visible_height_cells": VISIBLE_HEIGHT_CELLS,
            "cell_pixels": CELL_PIXELS,
        },
        "visible_words": [[f"0x{word:04X}" for word in row] for row in visible],
        "chrome_role_counts": dict(sorted(role_counts.items())),
        "chrome_rectangles": rectangles,
        "chrome_special_cells": special_cells,
        "text_runs": text_runs,
        "tile_runs": tile_runs,
        "visible_nonzero_cells": sum(word != 0 for row in visible for word in row),
        "omitted_zero_cells": sum(word == 0 for row in visible for word in row),
    }
    if source_region is not None:
        decoded["source_region"] = source_region
    if allow_unframed_text:
        decoded["text_scan"] = "framed_and_unframed_window_charset_runs"
    return decoded


def decode_sprites(raw: bytes) -> dict[str, Any]:
    if len(raw) != SPRITE_ENTRY_COUNT * SPRITE_ENTRY_BYTES:
        raise LayoutDecodeError("sprite table is not 80 eight-byte entries")
    entries: list[dict[str, Any]] = []
    for index in range(SPRITE_ENTRY_COUNT):
        chunk = raw[index * SPRITE_ENTRY_BYTES : (index + 1) * SPRITE_ENTRY_BYTES]
        y, size_link, attr, x = (
            int.from_bytes(chunk[offset : offset + 2], "big")
            for offset in (0, 2, 4, 6)
        )
        if not any((y, size_link, attr, x)):
            continue
        width = ((size_link >> 10) & 0x03) + 1
        height = ((size_link >> 8) & 0x03) + 1
        decoded_attr = decode_pattern_word(attr)
        entries.append({
            "index": index,
            "raw": chunk.hex().upper(),
            "stored_x": x,
            "stored_y": y,
            "screen_x": x - 128,
            "screen_y": y - 128,
            "hidden": y == 0,
            "visible": y != 0,
            "size_cells": {"width": width, "height": height},
            "size_pixels": {"width": width * CELL_PIXELS, "height": height * CELL_PIXELS},
            "link": size_link & 0x7F,
            **decoded_attr,
        })
    return {
        "buffer_address": "0xFFFFFC00",
        "entry_bytes": SPRITE_ENTRY_BYTES,
        "entry_capacity": SPRITE_ENTRY_COUNT,
        "coordinate_system": {
            "origin": "visible_screen_top_left",
            "screen_x": "SAT_x - 128",
            "screen_y": "SAT_y - 128",
            "note": "Y=0 is the Genesis hidden-sprite sentinel; retained with hidden=true.",
        },
        "entries": entries,
        "visible_entry_count": sum(entry["visible"] for entry in entries),
    }


def decode_cram(raw: bytes) -> dict[str, Any]:
    words = [int.from_bytes(raw[i : i + 2], "big") for i in range(0, len(raw), 2)]
    lines = []
    for line in range(4):
        line_words = words[line * 16 : (line + 1) * 16]
        lines.append({
            "line": line,
            "words": [f"0x{word:04X}" for word in line_words],
        })
    return {
        "buffer_address": "0xFFFFFB00",
        "word_count": len(words),
        "format": "Genesis CRAM words, preserved big-endian; four 16-color lines",
        "lines": lines,
    }


def _title_rect_summary(
    words: list[list[int]], spec: dict[str, Any], phase: str
) -> dict[str, Any]:
    rect = spec["cell_rect"]
    cells = [
        words[y][x]
        for y in range(rect["y"], rect["y"] + rect["height"])
        for x in range(rect["x"], rect["x"] + rect["width"])
    ]
    nonzero = [word for word in cells if word]
    patterns = [word & 0x07FF for word in nonzero]
    palette_lines = sorted({(word >> 13) & 0x03 for word in nonzero})
    expected_count = spec["columns"] * spec["rows"]
    expected_low, expected_high = spec["pattern_range"]
    observed = sorted(patterns)
    # The Sega and title mappings overlap at VRAM $010.  The phase is the
    # disambiguator for that one deliberate reuse; the state still reports
    # the raw observed count and range so this never becomes screenshot lore.
    active = {
        "sega_logo": phase == "sega_logo",
        "title_logo": phase in {"title_reveal", "title"},
        "subtitle": phase in {"title_reveal", "title"} and bool(nonzero),
        "press_start": phase == "title" and bool(nonzero),
        "copyright": phase == "title" and bool(nonzero),
    }[spec["name"]]
    return {
        "name": spec["name"],
        "active": active,
        "cell_rect": rect,
        "pixel_rect": {
            "x": rect["x"] * CELL_PIXELS,
            "y": rect["y"] * CELL_PIXELS,
            "width": rect["width"] * CELL_PIXELS,
            "height": rect["height"] * CELL_PIXELS,
        },
        "mapping": {
            "base_tile": f"0x{spec['base_tile']:03X}",
            "columns": spec["columns"],
            "rows": spec["rows"],
            "expected_pattern_range": [
                f"0x{expected_low:03X}", f"0x{expected_high:03X}"
            ],
            "expected_cell_count": expected_count,
            "palette_line": spec["palette_line"],
        },
        "observed": {
            "nonzero_cells": len(nonzero),
            "coverage": len(nonzero) / expected_count,
            "pattern_range": (
                [f"0x{min(patterns):03X}", f"0x{max(patterns):03X}"]
                if patterns else None
            ),
            "palette_lines": palette_lines,
            "patterns_within_declared_range": all(
                expected_low <= pattern <= expected_high for pattern in patterns
            ),
            "observed_range_matches_declared": bool(patterns)
            and min(patterns) == expected_low
            and max(patterns) == expected_high,
        },
    }


def _title_palette_summary(cram: dict[str, Any]) -> dict[str, Any]:
    lines = {
        str(line["line"]): [int(word, 16) for word in line["words"]]
        for line in cram["lines"]
    }
    return {
        "line_count": 4,
        "colors_per_line": 16,
        "raw_word_format": "big-endian Genesis CRAM words; decimal values",
        "lines": lines,
        "active_logo_lines": [0],
        "active_press_start_lines": [2],
        "active_copyright_lines": [1],
    }


def decode_title_summary(
    plane_a: dict[str, Any], plane_b: dict[str, Any], cram: dict[str, Any], frame: int
) -> dict[str, Any]:
    """Add the title-specific contract over the lossless plane decode."""
    phase = (
        "sega_logo"
        if frame <= 225 and plane_a["visible_nonzero_cells"]
        else "menu"
        if any(run["text"] in {"START", "CONTINUE", "ERASE DATA"}
               for run in plane_a["text_runs"])
        else "title"
        if plane_a["visible_nonzero_cells"]
        else "title_reveal"
    )
    words_a = [
        [int(word, 16) for word in row]
        for row in plane_a["visible_words"]
    ]
    words_b = [
        [int(word, 16) for word in row]
        for row in plane_b["visible_words"]
    ]
    elements = {
        spec["name"]: _title_rect_summary(words_a, spec, phase)
        for spec in TITLE_ELEMENTS
    }
    background_words = [word for row in words_b for word in row if word]
    background = {
        "cell_rect": {"x": 0, "y": 0, "width": 40, "height": 28},
        "pixel_rect": {"x": 0, "y": 0, "width": 320, "height": 224},
        "mapping": {
            "base_tile": "0x0E3",
            "palette_line": 3,
            "priority": True,
            "destination": "plane_b",
        },
        "observed": {
            "nonzero_cells": len(background_words),
            "pattern_range": (
                [
                    f"0x{min(word & 0x07FF for word in background_words):03X}",
                    f"0x{max(word & 0x07FF for word in background_words):03X}",
                ]
                if background_words else None
            ),
            "palette_lines": sorted({(word >> 13) & 0x03 for word in background_words}),
            "priority_values": sorted({bool(word & 0x8000) for word in background_words}),
        },
    }
    menu_rectangles = plane_a["chrome_rectangles"]
    menu_runs = [
        {
            key: value
            for key, value in run.items()
            if key != "cells"
        }
        for run in plane_a["text_runs"]
        if run["text"].strip()
    ]
    labels = [run["text"] for run in menu_runs]
    window_kind = (
        "no_save_data"
        if any(rect["x_cell"] == 13 and rect["y_cell"] == 12
               and rect["width_cells"] == 14 and rect["height_cells"] == 3
               for rect in menu_rectangles)
        else "save_slots"
        if any(rect["x_cell"] == 13 and rect["y_cell"] == 10
               and rect["width_cells"] == 14 and rect["height_cells"] == 7
               for rect in menu_rectangles)
        else None
    )
    return {
        "phase": phase,
        "elements": elements,
        "background": background,
        "palette_cycle": _title_palette_summary(cram),
        "menu": {
            "window_kind": window_kind,
            "window_rectangles": menu_rectangles,
            "text_runs": menu_runs,
            "options": labels,
            "continue_present": "CONTINUE" in labels,
            "continue_gate": "save_present" if "CONTINUE" in labels else "no_valid_save_slots",
        },
    }


def _self_check(layout: dict[str, Any], expected_texts: list[str]) -> dict[str, Any]:
    planes = layout["planes"]
    text_runs = [run for plane in planes.values() for run in plane["text_runs"]]
    decoded = [run["text"] for run in text_runs]
    missing = [text for text in expected_texts if text not in decoded]
    checks = {
        "plane_a_visible_matrix": len(planes["plane_a"]["visible_words"]) == 28
        and all(len(row) == 40 for row in planes["plane_a"]["visible_words"]),
        "plane_b_visible_matrix": len(planes["plane_b"]["visible_words"]) == 28
        and all(len(row) == 40 for row in planes["plane_b"]["visible_words"]),
        "text_glyphs_have_window_charset_codes": all(
            all(0 <= (cell["pattern"] - 0x680) in WINDOW_CHARSET for cell in run["cells"])
            for run in text_runs
        ),
        "expected_texts_present": not missing,
    }
    if not all(checks.values()):
        raise LayoutDecodeError(
            "layout self-check failed: "
            + ", ".join(name for name, passed in checks.items() if not passed)
            + (f"; missing={missing!r}" if missing else "")
        )
    return {
        "passed": True,
        "checks": checks,
        "expected_texts": expected_texts,
        "decoded_text_runs": decoded,
    }


def decode_layout(
    state_path: Path,
    expected_texts: list[str],
    label: str | None = None,
    *,
    title: bool = False,
    grand_cross: int = 0,
) -> dict[str, Any]:
    state = _read_state(state_path)
    plane_a = decode_plane(_region_bytes(state, "plane_a", 0x1000), "plane_a")
    plane_b = decode_plane(_region_bytes(state, "plane_b", 0x1000), "plane_b")
    planes: dict[str, dict[str, Any]] = {"plane_a": plane_a, "plane_b": plane_b}
    window_region = _optional_region(
        state, ("window_plane", "vdp_window_plane", "vdp_window", "window"), 0x1000
    )
    window_metadata: dict[str, Any] = {
        "present": window_region is not None,
        "name_table": "VDP Window plane",
        "expected_size_bytes": 0x1000,
        "expected_grid": "64x32 pattern-name words",
    }
    if window_region is not None:
        region_name, raw, region = window_region
        planes["window_plane"] = decode_plane(
            raw,
            "window_plane",
            allow_unframed_text=True,
            buffer_address=region.get("address", "0x0000F000"),
            source_region=region_name,
        )
        window_metadata["source_region"] = region_name
        window_metadata["address"] = region.get("address", "0x0000F000")
    layout: dict[str, Any] = {
        "format_version": 1,
        "kind": "psiv_title_layout" if title else "psiv_battle_layout",
        "capture": {
            "label": label or state_path.stem,
            "frame": state["frame"],
            "state_path": str(state_path),
            "state_format": state["format_version"],
        },
        "screen": {
            "width_pixels": 320,
            "height_pixels": 224,
            "visible_width_cells": VISIBLE_WIDTH_CELLS,
            "visible_height_cells": VISIBLE_HEIGHT_CELLS,
            "plane_width_cells": PLANE_WIDTH_CELLS,
            "plane_height_cells": PLANE_HEIGHT_CELLS,
            "cell_pixels": CELL_PIXELS,
        },
        "pattern_name_word": {
            "bits": "priority bit 15; palette line bits 14-13; vflip bit 12; hflip bit 11; pattern bits 10-0",
            "word_byte_order": "big-endian in the state JSON",
        },
        "planes": planes,
        "window_plane": window_metadata,
        "sprites": decode_sprites(_region_bytes(state, "sprite_table", 0x280)),
        "cram": decode_cram(_region_bytes(state, "cram", 0x80)),
        "scroll": decode_scroll_state(state, grand_cross),
    }
    if title:
        layout["title"] = decode_title_summary(
            plane_a, plane_b, layout["cram"], state["frame"]
        )
    layout["self_check"] = _self_check(layout, expected_texts)
    return layout


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("state", type=Path, help="JSON emitted by --dump-state")
    parser.add_argument("--output", "-o", required=True, type=Path)
    parser.add_argument("--label", help="capture label stored in the layout")
    parser.add_argument(
        "--expect-text",
        action="append",
        default=[],
        help="require this decoded battle string; repeat for the frame self-check",
    )
    parser.add_argument(
        "--title",
        action="store_true",
        help="add the retail title placement, palette, and menu contract",
    )
    parser.add_argument(
        "--grand-cross", type=int, default=0,
        help="record the retail grand_cross variant used by placement math",
    )
    args = parser.parse_args(argv)
    try:
        layout = decode_layout(
            args.state, args.expect_text, args.label, title=args.title,
            grand_cross=args.grand_cross,
        )
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(layout, indent=2) + "\n")
    except (LayoutDecodeError, ScrollDecodeError, OSError) as exc:
        parser.error(str(exc))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
