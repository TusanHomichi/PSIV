"""Retail vehicle field art: the three Nemesis sheets and mapping tables.

The field loader selects these blobs in ``loc_51A7A``
(``ps4.asm:107728-107747``), then ``FieldObj_LandRover`` and its siblings
stream them through ``art_ptr = RAM_Start``. The mapping pointer tables and
their two-frame sequences are the same ``FieldObj_Animate`` format used by
the party, so this module deliberately reuses the proven composer rather than
inventing a second sprite decoder.
"""

from __future__ import annotations

import hashlib
from dataclasses import dataclass
from typing import Sequence

from ..gfx import RGB
from ..nemesis import TILE_SIZE, decompress as nemesis_decompress
from .compose import Sheet, TileSource
from .field import _sheet_for_sequences, _table_sequences, sprite_palette
from .records import FACINGS, SpriteError, _bounds

# ``SprMapsPtrs_*`` and ``art_ptr`` values, cross-checked against the retail
# field-object init blocks at ps4.asm:93123-93207. These are ROM addresses,
# not generated art ids.
VEHICLE_RECORDS = (
    (1, "LandRover", 0x047EF4, 0x296320),
    (2, "IceDigger", 0x047F04, 0x296A54),
    (3, "Hydrofoil", 0x047F14, 0x2971A4),
)


@dataclass(frozen=True)
class VehicleSprite:
    """One selector's decompressed art and composed field sheet."""

    vehicle_index: int
    symbol: str
    art_offset: int
    mappings_addr: int
    compressed_size: int
    art_size: int
    sheet: Sheet
    art_sha256: str


def vehicle_sprites(rom: bytes, map_palette: Sequence[RGB]) -> list[VehicleSprite]:
    """Extract all three vehicle sheets from the retail Nemesis streams.

    Vehicle routines write ``$60`` to the sprite palette selector, which is
    CRAM line 3. The caller supplies the current pack's map palette blob in
    the retail ``0,1,3`` order; a runtime pack has one baked sheet, so the
    palette source is the first selected map (recorded in ``VEHICLES.md``).
    """

    if len(map_palette) < 48:
        raise SpriteError(
            f"vehicle palette has {len(map_palette)} colours; expected the 48-colour map blob"
        )
    sprites = []
    for index, symbol, mappings_addr, art_offset in VEHICLE_RECORDS:
        _bounds(rom, art_offset, 2)
        data, consumed = nemesis_decompress(rom, art_offset)
        if not data or len(data) % TILE_SIZE:
            raise SpriteError(
                f"{symbol} Nemesis art at 0x{art_offset:06X} decoded to "
                f"{len(data)} bytes, not a non-empty tile stream"
            )
        source = TileSource()
        source.add(0, data)
        sheet = _sheet_for_sequences(
            rom,
            _table_sequences(rom, mappings_addr, len(FACINGS)),
            source,
            art_tile=0,
            tile_props=0,
            streamed=True,
            palette=sprite_palette(rom, map_palette, 3),
            palette_line=3,
            required=[name for _, name in FACINGS],
        )
        sprites.append(
            VehicleSprite(
                vehicle_index=index,
                symbol=symbol,
                art_offset=art_offset,
                mappings_addr=mappings_addr,
                compressed_size=consumed,
                art_size=len(data),
                sheet=sheet,
                art_sha256=hashlib.sha256(data).hexdigest(),
            )
        )
    return sprites
