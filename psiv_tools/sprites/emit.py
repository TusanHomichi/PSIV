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
from dataclasses import dataclass
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
from .objects import (
    FIELD_OBJECTS_JMP_TBL,
    FIELD_OBJ_DO_OBJ_COLLISION,
    INTERACTION_CHK_OBJECTS,
    RENDER_FLAG_CAMERA_BYPASS,
    RENDER_FLAG_INTERACTABLE,
    FieldObjectRoutine,
)
from .records import SpriteError

SPRITES_DIRECTORY = "sprites"
PARTY_SPRITES_DIRECTORY = f"{SPRITES_DIRECTORY}/party"
NPC_SPRITES_DIRECTORY = f"{SPRITES_DIRECTORY}/npcs"
PARTY_SPRITES_NAME = f"{SPRITES_DIRECTORY}/party.json"
NPC_SPRITES_NAME = f"{SPRITES_DIRECTORY}/npcs.json"
VEHICLE_SPRITES_DIRECTORY = f"{SPRITES_DIRECTORY}/vehicles"
VEHICLE_SPRITES_NAME = f"{SPRITES_DIRECTORY}/vehicles.json"

#: Colour 0 of a Mega Drive sprite is transparent, so the sheets carry a tRNS
#: chunk marking it rather than a colour the renderer has to know to skip.
SPRITE_TRANSPARENT_INDEX = 0

#: How much of a sheet's content hash goes into its file name.
SHEET_ID_LENGTH = 8


@dataclass(frozen=True)
class NpcMetadata:
    """Everything the pack knows about one placed object beyond its record.

    `interactable` rides here rather than in `sprite` because it is true of
    objects that have no sprite at all: `FieldObj_InvisibleBlock` draws nothing
    and is still both a talk trigger and a wall.
    """

    sprite: dict[str, Any] | None
    sprite_reason: str | None
    interactable: bool
    camera_bypass: bool

    def to_json(self) -> dict[str, Any]:
        return {
            "sprite": self.sprite,
            "sprite_reason": self.sprite_reason,
            "interactable": self.interactable,
            "camera_bypass": self.camera_bypass,
        }


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

    def register(self, sheet: Sheet, symbol: str, placed: bool = True) -> str:
        """Register a sheet and return its name.

        `placed` is False for a sheet nothing stands on -- an alternate a
        palette copy switches to. Those are real files a runtime may draw, but
        counting them as placements would make `placements` stop meaning "how
        many placed NPCs draw this", which is the number the census reports.
        """
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
        if placed:
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


def emit_vehicles(
    root: Path, vehicles: Sequence[Any], name_suffix: str = ""
) -> tuple[list[dict[str, Any]], int]:
    """Write the three selector-addressed vehicle field sheets.

    A suffix is used only for an additional map-palette variant. The vehicle
    selector remains the source of truth; the JSON variant table points back
    to these ids in selector order.
    """
    directory = root / VEHICLE_SPRITES_DIRECTORY
    directory.mkdir(parents=True, exist_ok=True)
    entries = []
    total = 0
    for sprite in vehicles:
        image = sheet_png(sprite.sheet)
        label = f"{sprite.symbol}{name_suffix}"
        name = f"{_safe(label)}.png"
        (directory / name).write_bytes(image)
        total += len(image)
        entries.append({
            "id": label,
            "vehicle_index": sprite.vehicle_index,
            "symbol": sprite.symbol,
            "art": {
                "rom_offset": f"0x{sprite.art_offset:06X}",
                "size_bytes": sprite.art_size,
                "compressed_size_bytes": sprite.compressed_size,
                "tile_count": sprite.art_size // 32,
                "compression": "nemesis",
                "sha256": sprite.art_sha256,
            },
            "mappings_table": f"0x{sprite.mappings_addr:06X}",
            **sheet_json(sprite.sheet, f"{VEHICLE_SPRITES_DIRECTORY}/{name}", image),
        })
    return entries, total


def field_objects_json(
    routines: Sequence[FieldObjectRoutine], placements: dict[int, int]
) -> dict[str, Any]:
    """The per-object-type interaction table, with its provenance stated once.

    `interactable` is `render_flags` bit 3 and `camera_bypass` is bit 0.
    Emitting them per type rather than
    only per placement matters because the bit is a property of the
    `FieldObjectsJmpTbl` routine, not of the map record -- every `Xanafalgue`
    in the game is un-talkable for the same reason, and a consumer that wants
    to know *why* should not have to read 359 map files to find out.

    `placements` is how many objects of each type the emitted maps carry, so a
    reader can tell "no map uses this" from "this is off everywhere".
    """
    types = [
        {
            "object_id": routine.object_id,
            "symbol": routine.symbol,
            "routine_offset": f"0x{routine.rom_offset:06X}",
            "interactable": routine.interactable,
            "source": routine.interactable_source,
            "changes_at_runtime": routine.interactable_changes_at_runtime,
            "camera_bypass": routine.camera_bypass,
            "camera_bypass_source": routine.camera_bypass_source,
            "camera_bypass_changes_at_runtime": routine.camera_bypass_changes_at_runtime,
            "placements": placements.get(routine.object_id, 0),
        }
        for routine in routines
    ]
    return {
        "render_flags_bit": RENDER_FLAG_INTERACTABLE,
        "camera_render_flags_bit": RENDER_FLAG_CAMERA_BYPASS,
        "table": f"0x{FIELD_OBJECTS_JMP_TBL:06X}",
        "count": len(types),
        # Both readers do the identical `btst #3, $2(a3) / beq -> skip`, so the
        # bit is not "can be talked to" alone: an object with it clear is also
        # invisible to the walker's object collision.
        "tested_by": [
            {"routine": "Interaction_ChkObjects", "rom_offset": f"0x{INTERACTION_CHK_OBJECTS:06X}",
             "effect": "the talk probe skips the object"},
            {"routine": "FieldObj_DoObjCollision", "rom_offset": f"0x{FIELD_OBJ_DO_OBJ_COLLISION:06X}",
             "effect": "the object does not block the walker"},
        ],
        # The complete set of encodings retail uses to write the bit. A sweep of
        # every 68000 form that can write a byte at $2(a4) over the field-object
        # code region finds only these two: no ori/andi, no register-operand bit
        # instruction, no move.b over the whole byte.
        "instruction_forms": ["bset #3, $2(a4)", "bclr #3, $2(a4)"],
        "unwritten_bit_is_clear_because": (
            "GameMode_LoadFieldMap zero-fills $400 longwords from "
            "Field_Objects_Memory via trap #0 before LoadMapObjects parses the "
            "record, so a routine that writes neither leaves the bit clear"
        ),
        "types": types,
    }


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
) -> tuple[list[NpcMetadata], list[dict[str, Any]]]:
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

    references: list[NpcMetadata] = []
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
            references.append(
                NpcMetadata(None, sprite.reason, routine.interactable, routine.camera_bypass)
            )
            artless.append({
                "npc_index": entry["index"],
                "object_id": entry["object_id"],
                "symbol": entry["symbol"],
                "reason": sprite.reason,
                "missing_patterns": list(sprite.missing_patterns),
            })
            continue
        sheet_id = registry.register(sprite.sheet, entry["symbol"] or "FieldObj")
        references.append(
            NpcMetadata(
                _sprite_reference(sprite, sheet_id), None,
                routine.interactable, routine.camera_bypass,
            )
        )
    return references, artless
