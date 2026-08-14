"""The field-mode joins: palettes, the party, a map's staged art, timing.

Where a jump-table routine, a map record and a pattern bank meet and become
a sheet. `psiv_tools.pack` is the only caller that needs anything here.
"""

from __future__ import annotations

import hashlib
from dataclasses import dataclass
from typing import Any, Sequence

from ..gfx import COLORS_PER_LINE, PALETTE_LINE_SIZE, RGB, decode_palette, palette_rgb
from ..kosinski import decompress as kosinski_decompress
from ..nemesis import TILE_SIZE, decompress as nemesis_decompress
from .compose import (
    Frame,
    Sheet,
    SheetSequence,
    SpriteCensus,
    TileSource,
    build_sheet,
    compose_frame,
    mapping_box,
    union_box,
)
from .objects import FieldObjectRoutine, scan_field_objects
from .records import (
    FACINGS,
    FACING_NAMES,
    RAM_BASE,
    ROM_LIMIT,
    AnimationSequence,
    Mapping,
    SpriteError,
    _bounds,
    _hex,
    _s16,
    _u8,
    _u32,
    decode_mapping,
    decode_sequence,
)

# ---------------------------------------------------------------------------
# Retail offsets for the party art, the movement table and the fixed palette.
# ---------------------------------------------------------------------------
#: `CharFieldArtPtrs` / `CharSpriteMappingsPtrs`, 11 longs each.
CHAR_FIELD_ART_PTRS = 0x05E732
CHAR_SPRITE_MAPPINGS_PTRS = 0x05E75E
PARTY_SLOTS = 11
PARTY_ART_BYTES = 2304
PARTY_ART_TILES = PARTY_ART_BYTES // TILE_SIZE

#: `FieldObj_MovementsTbl`: three 16-entry blocks of eight bytes, selected by
#: `FieldObj_Step_Offset` (0 slow, 1 normal, 2 fast). The table ends at
#: `loc_47C28`, exactly 3 * 16 * 8 bytes in.
MOVEMENTS_TBL = 0x047AA8
MOVEMENT_BLOCKS = 3
MOVEMENT_ENTRIES = 16
MOVEMENT_ENTRY_SIZE = 8
MOVEMENT_NORMAL_BLOCK = 1
#: `FieldObj_Move` does `andi.w #3, d1 / lsl.w #7, d1`, so the selector reaches
#: a fourth block the labelled table does not contain. Nothing in retail ever
#: stores a 3, so it is dormant; the clone widens the mask to `#7`.
MOVEMENT_SELECTOR_MASK_SITE = 0x0477B8
MOVEMENT_SELECTOR_MASK = 3

#: `Pal_Init_Line_3` -- copied into CRAM line 2 by `loc_53F14` for every map,
#: so a sprite whose `$13` says line 2 is the same colours everywhere.
PAL_INIT_LINE_3 = 0x296300
PAL_INIT_LINE_3_CRAM_LINE = 2

#: The map palette blob holds CRAM lines 0, 1 and 3 in that order; line 2 is
#: the fixed one above. Mirrors `layouts.MAP_PALETTE_LINES`.
MAP_PALETTE_LINE_ORDER = (0, 1, 3)


def char_field_art_pointers(rom: bytes) -> tuple[int, ...]:
    """The eleven `CharFieldArtPtrs` entries, in table order."""
    return tuple(_u32(rom, CHAR_FIELD_ART_PTRS + slot * 4) for slot in range(PARTY_SLOTS))


def party_art(rom: bytes, offset: int) -> TileSource:
    """One uncompressed 72-pattern field-art blob."""
    _bounds(rom, offset, PARTY_ART_BYTES)
    source = TileSource()
    source.add(0, rom[offset:offset + PARTY_ART_BYTES])
    return source


# ---------------------------------------------------------------------------
# Palettes
# ---------------------------------------------------------------------------
def pal_init_line_3(rom: bytes) -> list[RGB]:
    """CRAM line 2 for every field map, from `loc_53F14`'s fixed copy."""
    _bounds(rom, PAL_INIT_LINE_3, PALETTE_LINE_SIZE)
    return palette_rgb(
        decode_palette(rom[PAL_INIT_LINE_3:PAL_INIT_LINE_3 + PALETTE_LINE_SIZE])
    )


def sprite_palette(rom: bytes, map_palette: Sequence[RGB], line: int) -> list[RGB]:
    """The 16 colours a field sprite on CRAM line `line` actually draws with.

    `map_palette` is the map record's blob as `layouts.decode_map_palette`
    returns it: 48 colours, CRAM lines 0, 1 and 3 in that order. Line 2 is not
    in there because `loc_53F14` fills it from `Pal_Init_Line_3` instead, which
    is exactly why the party -- every one of whose routines stores `$40` --
    looks the same on every map.
    """
    if line == PAL_INIT_LINE_3_CRAM_LINE:
        return pal_init_line_3(rom)
    if line not in MAP_PALETTE_LINE_ORDER:
        raise SpriteError(f"CRAM line {line} is not one a field sprite can select")
    if len(map_palette) < len(MAP_PALETTE_LINE_ORDER) * COLORS_PER_LINE:
        raise SpriteError(
            f"map palette holds {len(map_palette)} colours, expected "
            f"{len(MAP_PALETTE_LINE_ORDER) * COLORS_PER_LINE}"
        )
    position = MAP_PALETTE_LINE_ORDER.index(line)
    return [tuple(c) for c in map_palette[position * COLORS_PER_LINE:(position + 1) * COLORS_PER_LINE]]


# ---------------------------------------------------------------------------
# Party sprites
# ---------------------------------------------------------------------------
@dataclass(frozen=True)
class PartySprite:
    slot: int
    symbol: str
    object_id: int
    art_offset: int
    mappings_addr: int
    sheet: Sheet
    art_sha256: str


#: The eleven `CharFieldArtPtrs` entries in table order. These are the field
#: object symbols of the same characters, which is how the two tables are tied
#: together: `loc_5E6DE` writes slot N's art and mappings into the object it
#: also gives object id `(N + 1) * 4`.
PARTY_SYMBOLS: tuple[str, ...] = (
    "Chaz", "Alys", "Hahn", "Rune", "Gryz", "Rika", "Demi", "Wren", "Raja",
    "Kyra", "Seth",
)


def party_sprites(rom: bytes, routines: Sequence[FieldObjectRoutine] | None = None) -> list[PartySprite]:
    """Compose all eleven party field sprites straight out of the cartridge.

    The art and mapping tables are read from the ROM, not hard-coded per
    character, and cross-checked against the eleven `FieldObjectsJmpTbl`
    routines: `FieldObj_Chaz` stores `Art_ChazField` and `SprMapsPtrs_Chaz`
    itself, so the two tables and the twenty-two immediates in the routines
    have to agree or something is being read from the wrong place.
    """
    if routines is None:
        routines = scan_field_objects(rom)
    by_symbol = {r.symbol: r for r in routines}
    palette = pal_init_line_3(rom)

    sprites = []
    for slot, symbol in enumerate(PARTY_SYMBOLS):
        art = _u32(rom, CHAR_FIELD_ART_PTRS + slot * 4)
        table = _u32(rom, CHAR_SPRITE_MAPPINGS_PTRS + slot * 4)
        routine = by_symbol[symbol]
        if routine.art_ptr != art or routine.mappings_addr != table:
            raise SpriteError(
                f"FieldObj_{symbol} sets art {routine.art_ptr and hex(routine.art_ptr)} / "
                f"mappings {routine.mappings_addr and hex(routine.mappings_addr)}, but "
                f"CharFieldArtPtrs slot {slot} says {hex(art)} / {hex(table)}"
            )
        if routine.palette_line != PAL_INIT_LINE_3_CRAM_LINE:
            raise SpriteError(
                f"FieldObj_{symbol} draws on CRAM line {routine.palette_line}, not the "
                f"line {PAL_INIT_LINE_3_CRAM_LINE} the party is expected to use"
            )
        source = party_art(rom, art)
        if len(source) != PARTY_ART_TILES:
            raise SpriteError(
                f"{symbol}'s field art decodes to {len(source)} patterns, expected "
                f"{PARTY_ART_TILES}"
            )
        sheet = _sheet_for_sequences(
            rom, _table_sequences(rom, table, len(FACINGS)), source,
            art_tile=0, tile_props=0, streamed=True,
            palette=palette, palette_line=PAL_INIT_LINE_3_CRAM_LINE,
            required=[name for _, name in FACINGS],
        )
        sprites.append(
            PartySprite(
                slot=slot,
                symbol=symbol,
                object_id=routine.object_id,
                art_offset=art,
                mappings_addr=table,
                sheet=sheet,
                art_sha256=hashlib.sha256(rom[art:art + PARTY_ART_BYTES]).hexdigest(),
            )
        )
    return sprites


def _sheet_for_sequences(
    rom: bytes,
    wanted: Sequence[tuple[str, int, int]],
    source: TileSource,
    *,
    art_tile: int,
    tile_props: int,
    streamed: bool,
    palette: Sequence[RGB],
    palette_line: int,
    required: Sequence[str] = (),
    census: SpriteCensus | None = None,
) -> Sheet:
    """Compose the named sequences into one sheet, failing closed on missing art.

    `wanted` is `(name, facing, sequence offset)`. A sequence whose frames name
    patterns the source does not hold is dropped rather than drawn with holes
    -- except when its name is in `required`, where a hole means the caller
    asked for something the cartridge cannot provide and should hear about it.
    """
    decoded = [
        (name, facing, sequence, [decode_mapping(rom, f.mapping_offset) for f in sequence.frames])
        for name, facing, sequence in (
            (name, facing, decode_sequence(rom, offset)) for name, facing, offset in wanted
        )
    ]
    box = union_box(mapping_box(m) for _, _, _, mappings in decoded for m in mappings)

    composed: list[tuple[str, int, AnimationSequence, list[Frame]]] = []
    for name, facing, sequence, mappings in decoded:
        frames: list[Frame] = []
        missing: list[int] = []
        for mapping in mappings:
            frame, holes = compose_frame(
                mapping, source, box, art_tile=art_tile, tile_props=tile_props,
                streamed=streamed, census=census,
            )
            missing += holes
            frames.append(frame)
        if census is not None:
            census.note("sequences")
            if sequence.per_frame_durations:
                census.note("per_frame_duration_sequences")
        if missing:
            if name in required:
                raise SpriteError(
                    f"sequence {_hex(sequence.rom_offset)} ({name}) names patterns "
                    f"{sorted(set(missing))[:8]} that the art source does not hold"
                )
            continue
        composed.append((name, facing, sequence, frames))

    if not composed:
        raise SpriteError("no sequence in this set composed at all")
    return build_sheet(composed, box, palette, palette_line)


def _table_sequences(
    rom: bytes, table: int, facings: int
) -> list[tuple[str, int, int]]:
    return [
        (name, facing, _u32(rom, table + facing))
        for facing, name in FACINGS[:facings]
    ]


# ---------------------------------------------------------------------------
# Walk timing
# ---------------------------------------------------------------------------
@dataclass(frozen=True)
class StepTiming:
    block: int
    #: the step constant's whole-pixel part; the constant is 16.16 per frame
    step_word: int
    frames_per_cell: int

    @property
    def pixels_per_frame(self) -> int:
        return self.step_word // 256


def step_timing(rom: bytes, cell_pixels: int = 16) -> list[StepTiming]:
    """What one cell of walking costs, per `FieldObj_Step_Offset` block.

    `FieldObj_Move` stores `duration << 8` into `x_step_duration` and
    `sign_extend(word) << 8` into `x_step_constant`;
    `FieldObj_UpdateStepDuration` then subtracts `|constant| >> 8`, which is
    `|word|`, from the duration every frame, and `FieldObj_UpdatePosition` adds
    the constant to a 16.16 position. So a block's frames per step is
    `(duration << 8) / |word|` and its pixels per frame is `|word| / 256`.

    All three blocks are read from the cartridge rather than taken from the
    clone. They happen to agree byte for byte, but the clone is a Grand Cross
    build and its `FieldObj_Move` masks the selector with `#7` where retail
    masks with `#3`, so the two disagree about how many blocks are even
    reachable.
    """
    timings = []
    for block in range(MOVEMENT_BLOCKS):
        speeds = set()
        for entry in range(MOVEMENT_ENTRIES):
            base = MOVEMENTS_TBL + block * MOVEMENT_ENTRIES * MOVEMENT_ENTRY_SIZE
            base += entry * MOVEMENT_ENTRY_SIZE
            durations = (_u8(rom, base), _u8(rom, base + 1))
            steps = (_s16(rom, base + 2), _s16(rom, base + 4))
            for duration, step in zip(durations, steps):
                if not duration or not step:
                    continue
                word = abs(step)
                total = duration << 8
                if total % word or word % 256:
                    raise SpriteError(
                        f"movement block {block} entry {entry} does not divide into "
                        f"whole frames: duration 0x{total:X}, step 0x{word:X}"
                    )
                speeds.add((total // word, word))
        if len(speeds) != 1:
            raise SpriteError(
                f"movement block {block} holds {len(speeds)} distinct speeds; the table "
                "is not three uniform blocks"
            )
        frames_per_cell, word = speeds.pop()
        if (word // 256) * frames_per_cell != cell_pixels:
            raise SpriteError(
                f"movement block {block} covers {(word // 256) * frames_per_cell} pixels "
                f"per step, not the {cell_pixels}-pixel collision cell"
            )
        timings.append(
            StepTiming(block=block, step_word=word, frames_per_cell=frames_per_cell)
        )
    return timings


def step_timing_json(rom: bytes, cell_pixels: int = 16) -> dict[str, Any]:
    timings = step_timing(rom, cell_pixels)
    return {
        "table": _hex(MOVEMENTS_TBL),
        "selector": "FieldObj_Step_Offset",
        "cell_pixels": cell_pixels,
        "normal_block": MOVEMENT_NORMAL_BLOCK,
        "frames_per_cell": timings[MOVEMENT_NORMAL_BLOCK].frames_per_cell,
        "blocks": [
            {
                "block": t.block,
                "step_constant": f"0x{t.step_word:04X}",
                "pixels_per_frame": t.pixels_per_frame,
                "frames_per_cell": t.frames_per_cell,
            }
            for t in timings
        ],
        "note": (
            "Animation is not locked to the step: FieldObj_Animate free-runs on "
            "its own per-sequence duration and FieldObj_Move only resets it to "
            "frame 0 when the object stops moving."
        ),
    }


# ---------------------------------------------------------------------------
# Map-staged art and the objects that draw from it
# ---------------------------------------------------------------------------
@dataclass(frozen=True)
class StagedArt:
    """Everything a map record loads for its sprites to draw out of.

    `vram` is the pattern bank the VDP would hold: the Kosinski tilesets
    (`loc_519D2`) first, then the Nemesis sprite art (`loc_51A1A`) on top,
    exactly in record order. `ram` holds the blobs the record decompresses to
    work RAM instead, keyed by the address the loader computes, for the objects
    that stream their art from there.
    """

    vram: TileSource
    ram: dict[int, TileSource]


def tile_source_from_patterns(patterns: Any) -> TileSource:
    """Wrap a `layouts.TilePatterns` as a `TileSource`.

    `TilePatterns` holds all 2,048 pattern slots and says separately which ones
    a map actually filled; only the filled ones are real art, so the unfilled
    ones must stay holes here rather than become blank tiles a sprite would
    silently draw.
    """
    return TileSource({index: patterns.tiles[index] for index in patterns.loaded})


def stage_map_art(rom: bytes, record: dict[str, Any], vram: TileSource | None = None) -> StagedArt:
    """Replay a map record's tileset and sprite lists into a pattern bank.

    `vram` lets a caller hand in a bank that already holds the tilesets, since
    `psiv_tools.layouts.decode_tilesets` has usually built one for the layout
    render already; without it the tilesets are decompressed here.
    """
    bank = TileSource() if vram is None else vram
    if vram is None:
        for entry in record["tilesets"]["entries"]:
            data, _ = kosinski_decompress(rom, int(entry["source_offset"], 16))
            bank.add(entry["vram_tile"], data)

    ram: dict[int, TileSource] = {}
    for entry in record["sprites"]["entries"]:
        offset = int(entry["source_offset"], 16)
        data, _ = nemesis_decompress(rom, offset)
        if entry["destination"] == "vram":
            bank.add(entry["tile_number"], data)
        else:
            # loc_51A3E: `lsl.w #5, d0` then `moveq #-1, d1 / move.w d0, d1`.
            address = RAM_BASE | ((entry["tile_number"] << 5) & 0xFFFF)
            source = TileSource()
            source.add(0, data)
            ram[address] = source
    for entry in record["sprite_data"]["entries"]:
        data, _ = kosinski_decompress(rom, int(entry["source_offset"], 16))
        if len(data) % TILE_SIZE:
            continue  # Kosinski RAM blobs are not always patterns.
        source = TileSource()
        source.add(0, data)
        ram.setdefault(int(entry["ram_address"], 16), source)
    return StagedArt(vram=bank, ram=ram)


@dataclass(frozen=True)
class ObjectSprite:
    """A placed field object resolved to a sheet, or to a reason it has none."""

    sheet: Sheet | None
    facing: int
    facing_name: str | None
    idle_sequence: str | None
    walk_sequence: str | None
    reason: str | None = None
    missing_patterns: tuple[int, ...] = ()


STATIC_SEQUENCE = "static"


def object_sprite(
    rom: bytes,
    routine: FieldObjectRoutine,
    extents: dict[int, int],
    art: StagedArt,
    map_palette: Sequence[RGB],
    *,
    facing: int,
    art_tile: int,
    census: SpriteCensus | None = None,
) -> ObjectSprite:
    """Compose one placed object's sheet the way the cartridge would draw it.

    Returns a sprite with `sheet` `None`, and a reason, whenever the cartridge
    itself draws nothing: the object suppresses sprite building, it has no
    mapping data, or the art its frames name is not in the bank the record
    filled. Nothing is invented to cover a hole.
    """
    facing_name = FACING_NAMES.get(facing)
    if not routine.builds_sprites:
        return ObjectSprite(
            None, facing, facing_name, None, None,
            "FieldObj_" + routine.symbol + " sets render_flags bit 1, and "
            "Field_FillSpriteAttributes returns on it: the object is a trigger, not art",
        )
    if routine.sprite_tile_props is None:
        return ObjectSprite(
            None, facing, facing_name, None, None,
            f"FieldObj_{routine.symbol} never stores sprite tile properties, so it "
            "has no palette line and draws nothing of its own",
        )
    line = routine.palette_line
    palette = sprite_palette(rom, map_palette, line)

    tile_props = routine.sprite_tile_props
    if routine.streams_art:
        if routine.art_ptr is None:
            return ObjectSprite(
                None, facing, facing_name, None, None,
                f"FieldObj_{routine.symbol} reaches FieldObj_Animate but stores no "
                "art_ptr, so the art it streams is whatever the object slot held",
            )
        source = _streamed_source(rom, routine.art_ptr, art)
        if source is None:
            return ObjectSprite(
                None, facing, facing_name, None, None,
                f"FieldObj_{routine.symbol} streams art from "
                f"0x{routine.art_ptr:08X}, which this map record does not load",
            )
        effective_tile, streamed = 0, True
    else:
        source, streamed = art.vram, False
        effective_tile = routine.art_tile if routine.art_tile is not None else art_tile

    if routine.animates and routine.mappings_addr is not None:
        table = routine.mappings_addr
        owned = extents[table]
        slot = facing // 4
        if facing % 4 == 0 and slot < owned:
            wanted = _table_sequences(rom, table, owned)
            name = FACING_NAMES[facing]
        else:
            # The one AiedoPub object that faces $10 lands here: `facing_dir`
            # is added to `mappings_addr` as a byte offset with no bound at
            # all, so the cartridge follows the long that sits past the end of
            # this object's table. That is reproduced rather than corrected,
            # and named for the byte so nobody reads it as a real direction.
            name = f"facing_0x{facing:02X}"
            wanted = [(name, facing, _u32(rom, table + facing))]
        try:
            sheet = _sheet_for_sequences(
                rom, wanted, source,
                art_tile=effective_tile, tile_props=tile_props, streamed=streamed,
                palette=palette, palette_line=line, required=[name], census=census,
            )
        except SpriteError as error:
            return ObjectSprite(None, facing, facing_name, None, None, str(error))
        return ObjectSprite(sheet, facing, name, f"idle_{name}", f"walk_{name}")

    if routine.mappings is None:
        return ObjectSprite(
            None, facing, facing_name, None, None,
            f"FieldObj_{routine.symbol} stores no mapping pointer",
        )
    mapping = decode_mapping(rom, routine.mappings)
    box = mapping_box(mapping)
    frame, missing = compose_frame(
        mapping, source, box, art_tile=effective_tile, tile_props=tile_props,
        streamed=streamed, census=census,
    )
    if missing:
        return ObjectSprite(
            None, facing, facing_name, None, None,
            f"FieldObj_{routine.symbol} names patterns this map record does not load",
            tuple(sorted(set(missing))),
        )
    sequence = SheetSequence(
        facing=facing, frames=(0,),
        durations=((routine.mappings_duration or 0) + 1,),
        sequence_offset=routine.mappings,
    )
    sheet = Sheet(
        frames=(frame,), sequences={STATIC_SEQUENCE: sequence}, box=box,
        palette=tuple(palette), palette_line=line, mirrors={},
    )
    return ObjectSprite(sheet, facing, facing_name, STATIC_SEQUENCE, STATIC_SEQUENCE)


def _streamed_source(rom: bytes, art_ptr: int, art: StagedArt) -> TileSource | None:
    """The art blob a streaming object DMAs out of, or `None` if it is not here.

    A ROM pointer is only accepted when it is one of the eleven
    `CharFieldArtPtrs` entries. Every streaming object in retail points at one
    of those, and the blobs carry no length of their own, so reading 2,304
    bytes from an address that is not in the table would be inventing a size.
    """
    if art_ptr >= ROM_LIMIT:
        return art.ram.get(art_ptr)
    if art_ptr not in char_field_art_pointers(rom):
        return None
    return party_art(rom, art_ptr)
