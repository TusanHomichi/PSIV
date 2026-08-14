"""Writing sprite sheets into the runtime pack.

The layout of the sprite half of a pack -- where the PNGs go, what the two
index files hold, and how sheets are deduplicated across maps -- lives here
rather than in `psiv_tools.pack`, because all of it is knowledge about sprites
and none of it is knowledge about maps. `psiv_tools.pack` owns the manifest and
calls in.

A sheet is keyed by content: same pixels, same geometry, same palette and same
frame timing is the same sheet. Field sprite art is shared hard -- one
72-pattern NPC sheet serves whole towns -- and the same art under two maps'
palettes is genuinely two different pictures, so the palette is part of the key.
"""

from __future__ import annotations

import hashlib
from pathlib import Path
from typing import Any, Sequence

from .. import png
from ..gfx import COLORS_PER_LINE, palette_rgb
from ..layouts import decode_map_palette
from .compose import Sheet, SpriteCensus
from .field import (
    PAL_INIT_LINE_3_CRAM_LINE,
    PARTY_ART_BYTES,
    PARTY_ART_TILES,
    ObjectSprite,
    PartySprite,
    object_sprite,
    stage_map_art,
    tile_source_from_patterns,
)
from .objects import FieldObjectRoutine
from .records import SpriteError

SPRITES_DIRECTORY = "sprites"
PARTY_SPRITES_DIRECTORY = f"{SPRITES_DIRECTORY}/party"
NPC_SPRITES_DIRECTORY = f"{SPRITES_DIRECTORY}/npcs"
PARTY_SPRITES_NAME = f"{SPRITES_DIRECTORY}/party.json"
NPC_SPRITES_NAME = f"{SPRITES_DIRECTORY}/npcs.json"

#: Colour 0 of a Mega Drive sprite is transparent, so the sheets carry a tRNS
#: chunk marking it rather than a colour the renderer has to know to skip.
SPRITE_TRANSPARENT_INDEX = 0

#: How much of a sheet's content hash goes into its file name.
SHEET_ID_LENGTH = 8


def sheet_png(sheet: Sheet) -> bytes:
    """One sheet as an indexed PNG, frames left to right in a single row.

    The palette baked in is the CRAM line the object's `$13` byte selects, so
    the image is finished art rather than indices a consumer has to colour.
    """
    width, height, pixels = sheet.strip()
    palette = list(sheet.palette)
    if len(palette) != COLORS_PER_LINE:
        raise SpriteError(
            f"sprite sheet palette holds {len(palette)} colours, not {COLORS_PER_LINE}"
        )
    return png.encode_indexed(width, height, pixels, palette, (SPRITE_TRANSPARENT_INDEX,))


def sheet_json(sheet: Sheet, path: str, image: bytes) -> dict[str, Any]:
    """The geometry, timing and palette a renderer needs for one sheet.

    `origin_x`/`origin_y` are where the object's own position sits inside a
    frame: the cartridge adds each piece's offsets to `sprite_x_pos` and
    `sprite_y_pos`, so a frame drawn at `(x - origin_x, y - origin_y)` lands
    exactly where the VDP would have put it.
    """
    return {
        "png": path,
        "png_sha256": hashlib.sha256(image).hexdigest(),
        "frame_width": sheet.box.width,
        "frame_height": sheet.box.height,
        "frame_count": sheet.frame_count,
        "origin_x": -sheet.box.left,
        "origin_y": -sheet.box.top,
        "palette": {
            "cram_line": sheet.palette_line,
            "source": (
                "Pal_Init_Line_3"
                if sheet.palette_line == PAL_INIT_LINE_3_CRAM_LINE
                else "map palette blob"
            ),
            "transparent_index": SPRITE_TRANSPARENT_INDEX,
            "colors": [list(colour) for colour in sheet.palette],
        },
        "mirrors": {str(index): other for index, other in sorted(sheet.mirrors.items())},
        "sequences": {
            name: sequence.to_json() for name, sequence in sorted(sheet.sequences.items())
        },
    }


class SheetRegistry:
    """Deduplicates sheets across maps and hands out their file names.

    Field sprite art is shared hard -- one 72-pattern NPC sheet serves whole
    towns -- so the pack keys sheets by content: same pixels, same geometry,
    same palette and same frame timing is the same sheet. The name carries the
    symbol of the first object that asked for it, which is a debugging
    convenience and nothing more; the identity is the hash.
    """

    def __init__(self, directory: str) -> None:
        self.directory = directory
        self._by_identity: dict[str, str] = {}
        self._sheets: dict[str, Sheet] = {}
        self._users: dict[str, int] = {}
        self._names: set[str] = set()

    def register(self, sheet: Sheet, symbol: str) -> str:
        identity = sheet.identity()
        name = self._by_identity.get(identity)
        if name is None:
            name = f"{_safe(symbol)}_{identity[:SHEET_ID_LENGTH]}"
            if name in self._names:
                raise SpriteError(
                    f"two different sheets both want the name {name!r}; the "
                    f"{SHEET_ID_LENGTH}-character content hash is not enough"
                )
            self._names.add(name)
            self._by_identity[identity] = name
            self._sheets[name] = sheet
            self._users[name] = 0
        self._users[name] += 1
        return name

    def emit(self, root: Path) -> tuple[list[dict[str, Any]], int]:
        """Write every registered sheet; returns the JSON entries and byte count."""
        directory = root / self.directory
        directory.mkdir(parents=True, exist_ok=True)
        entries = []
        total = 0
        for name in sorted(self._sheets):
            sheet = self._sheets[name]
            image = sheet_png(sheet)
            (directory / f"{name}.png").write_bytes(image)
            total += len(image)
            entries.append({
                "id": name,
                "placements": self._users[name],
                **sheet_json(sheet, f"{self.directory}/{name}.png", image),
            })
        return entries, total


def _safe(label: str) -> str:
    return "".join(c if c.isalnum() or c in "-_" else "_" for c in label)


def emit_party(root: Path, party: Sequence[PartySprite]) -> tuple[list[dict[str, Any]], int]:
    """Write the eleven playable characters' sheets.

    These are not deduplicated and not keyed by content: a party sheet is
    identified by the character, because that is how a runtime asks for one,
    and every character has their own 2,304-byte art blob anyway.
    """
    directory = root / PARTY_SPRITES_DIRECTORY
    directory.mkdir(parents=True, exist_ok=True)
    entries = []
    total = 0
    for sprite in party:
        image = sheet_png(sprite.sheet)
        name = f"{_safe(sprite.symbol)}.png"
        (directory / name).write_bytes(image)
        total += len(image)
        entries.append({
            "id": sprite.symbol,
            "slot": sprite.slot,
            "symbol": sprite.symbol,
            "object_id": sprite.object_id,
            "art": {
                "rom_offset": f"0x{sprite.art_offset:06X}",
                "size_bytes": PARTY_ART_BYTES,
                "tile_count": PARTY_ART_TILES,
                "compression": None,
                "sha256": sprite.art_sha256,
            },
            "mappings_table": f"0x{sprite.mappings_addr:06X}",
            **sheet_json(sprite.sheet, f"{PARTY_SPRITES_DIRECTORY}/{name}", image),
        })
    return entries, total


def _sprite_reference(sprite: ObjectSprite, sheet_id: str) -> dict[str, Any]:
    return {
        "sheets": NPC_SPRITES_NAME,
        "sheet": sheet_id,
        "facing": sprite.facing_name,
        "idle_sequence": sprite.idle_sequence,
        "walk_sequence": sprite.walk_sequence,
    }


def resolve_map_sprites(
    rom: bytes,
    record: dict[str, Any],
    decoded,
    routines: Sequence[FieldObjectRoutine],
    extents: dict[int, int],
    registry: SheetRegistry,
    census: SpriteCensus,
) -> tuple[list[tuple[dict[str, Any] | None, str | None]], list[dict[str, Any]]]:
    """Resolve every object of one map to a sheet, or to a reason it has none.

    The pattern bank the objects draw from is the one the record itself fills:
    the layout's Kosinski tilesets, which `decode_layout_section` has already
    decompressed, plus the Nemesis sprite list on top. Anything an object names
    outside that bank is a hole, and a hole means no sheet rather than a frame
    with guessed pixels in it.
    """
    art = stage_map_art(rom, record, tile_source_from_patterns(decoded.patterns))
    palette = palette_rgb(decode_map_palette(rom, decoded.spec.palette))
    by_id = {routine.object_id: routine for routine in routines}

    references: list[tuple[dict[str, Any] | None, str | None]] = []
    artless: list[dict[str, Any]] = []
    for entry in record["objects"]["entries"]:
        routine = by_id.get(entry["object_id"])
        if routine is None:
            raise SpriteError(
                f"map 0x{record['id']:03X} object {entry['index']} names field object "
                f"0x{entry['object_id']:X}, outside FieldObjectsJmpTbl"
            )
        sprite = object_sprite(
            rom, routine, extents, art, palette,
            facing=entry["facing_dir"], art_tile=entry["art_tile"], census=census,
        )
        if sprite.sheet is None:
            references.append((None, sprite.reason))
            artless.append({
                "npc_index": entry["index"],
                "object_id": entry["object_id"],
                "symbol": entry["symbol"],
                "reason": sprite.reason,
                "missing_patterns": list(sprite.missing_patterns),
            })
            continue
        sheet_id = registry.register(sprite.sheet, entry["symbol"] or "FieldObj")
        references.append((_sprite_reference(sprite, sheet_id), None))
    return references, artless
