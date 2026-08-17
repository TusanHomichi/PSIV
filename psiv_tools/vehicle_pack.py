"""Vehicle sprite variants for the runtime pack.

The field composer is shared with ordinary sprites, but vehicle palettes are
selected by the current map's CRAM line 3. Keeping this small pack-level join
out of ``pack.py`` leaves the main emitter below the repository's touch-time
file-size threshold and gives the palette policy a single testable seam.
"""

from __future__ import annotations

from pathlib import Path
from typing import Any, Mapping, Sequence

from .gfx import RGB
from .sprites import vehicle_sprites
from .sprites.emit import emit_vehicles


def emit_vehicle_index(
    rom: bytes,
    directory: Path,
    base_palette: Sequence[RGB],
    palettes: Mapping[tuple[RGB, ...], Sequence[int]],
    format_version: int,
) -> tuple[dict[str, Any], int, list[dict[str, Any]]]:
    """Compose the base sheets and every distinct selected-map palette."""
    base_vehicles = vehicle_sprites(rom, base_palette)
    entries, total = emit_vehicles(directory, base_vehicles)
    base_key = tuple(base_palette[32:48])
    variants: list[dict[str, Any]] = []
    for variant_number, (palette_key, map_ids) in enumerate(palettes.items()):
        if palette_key == base_key:
            variant_entries: list[dict[str, Any]] = []
            sheet_ids = [entry["id"] for entry in entries]
        else:
            variant_palette = [(0, 0, 0)] * 32 + list(palette_key)
            variant_vehicles = vehicle_sprites(rom, variant_palette)
            variant_entries, variant_bytes = emit_vehicles(
                directory, variant_vehicles, f"_pal{variant_number:02d}"
            )
            total += variant_bytes
            sheet_ids = [entry["id"] for entry in variant_entries]
        variants.append({
            "map_ids": list(map_ids),
            "sheet_ids": sheet_ids,
            "sheets": variant_entries,
        })
    index = {
        "format_version": format_version,
        "kind": "field_vehicles",
        "sheet_count": len(entries),
        "palette_source": (
            "each selected map palette, CRAM line 3; base is first selected map"
        ),
        "sheets": entries,
        "palette_variants": variants,
    }
    return index, total, entries
