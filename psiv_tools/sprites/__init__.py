"""Field sprites: mapping records, animation tables and composed frames.

Field mode draws every character out of three pieces of cartridge data, and all
three are transcribed here from the routines that consume them rather than from
a description of the format.

## The object's init block

`FieldObjectsJmpTbl` (retail `0x04AA6C`, 222 `bra.w` entries) is indexed by an
object record's id byte-offset. Every routine but `FieldObj_None` opens with::

    bset    #7, (a4)
    bne.s   <main>
    ...one-time initialisation...

so the initialisation block is exactly the bytes between the `bne.s` and its
target, and `scan_field_objects` reads it straight out of the ROM. What it
takes from there:

* `mappings_addr` (`$8`) -- a table of longs indexed by `facing_dir`, which is
  0/4/8/$C for down/up/right/left. Four longs is the full table; plenty of
  objects share a shorter one with their neighbour, so the table's extent comes
  from where the next referenced table starts.
* `mappings` (`$C`) -- the frame this object starts on. An object that never
  reaches `FieldObj_Animate`/`FieldObj_Animate2` never leaves it, which is how
  a static sprite is expressed.
* `art_ptr` (`$18`) -- set only by objects that stream their own art. See
  below.
* `art_tile` (`$16`) -- a few routines pin their own VRAM tile; otherwise the
  map record's object entry supplies it.
* `$13` -- the byte OR-ed into the high half of every sprite's pattern word.
  Retail uses exactly $00/$20/$40/$60, i.e. CRAM lines 0-3. This is where a
  field sprite's palette comes from and there is nothing else in the chain that
  can change it.
* `render_flags` bit 1 -- `Field_FillSpriteAttributes` returns immediately when
  it is set, so an object that sets it is invisible by construction.

## Sprite mappings (one frame)

`Field_FillSpriteAttributes` reads::

    byte    piece count minus one   (dbf loop, so 0 means one piece)
    byte    copied to $4(a4)
    then per piece, six bytes:
    byte    Y offset, signed, added to sprite_y_pos
    byte    Mega Drive sprite size, %0000wwhh (tiles across-1, down-1)
    word    pattern word, *added* to art_tile
    byte    X offset, signed, added to sprite_x_pos
    byte    X offset used by the mirrored builder

The sixth byte is real data and the retail field path never reads it:
`Field_UpdateObjects` reaches `Field_BuildSprites` or `Field_BuildSprites2`,
both of which call `Field_FillSpriteAttributes`, and that routine does
`move.b (a1)+,d0 / addq.w #1,a1` -- it takes byte 4 and steps over byte 5. The
builder that would use it, `loc_44BE0`, is only reachable from `loc_44904`,
which nothing in field mode calls. So field sprites are never mirrored as
whole objects; where the cartridge mirrors a character it does it with the
H-flip bit inside the pattern word, and `Mappings_CharIdleRight` is
`Mappings_CharIdleLeft`'s tile with bit 11 set.

The pattern word is *added* to `art_tile` as a proper 16-bit sum (the high
bytes are added, then the low bytes, with the carry pushed back into the high
byte), and only after that is `$13` OR-ed in. The addition is why a piece can,
in principle, carry a tile sum into the flip and palette bits; the pack counts
whether any placed object actually does.

## Animation sequences

`FieldObj_Animate` follows `mappings_addr + facing_dir` to a sequence record
and there are two forms of it:

* byte 0 clear of bit 7: byte 0 is the frame count, byte 1 is one duration for
  every frame, then `count` longs.
* byte 0 with bit 7 set: `count = byte 0 & $7F`, bytes 1..count are per-frame
  durations, then the longs, starting at the next even offset after them
  (`loc_4477E` forces its index odd and reads from `1(a1,d0.w)`).

The counter is `subq.b #1, mappings_duration(a4) / bpl`, so a frame stays up
for `duration + 1` game frames, and `Field_RunObjects` runs once per rendered
frame.

Standing still is not a separate sequence. `FieldObj_Move` writes
`clr.b mappings_idx / move.b #1, mappings_duration` whenever both step
constants come out zero, so an idle object sits on frame 0 of whichever
direction it faces. That is the whole idle/walk split: `idle_<dir>` is frame 0,
`walk_<dir>` is the sequence.

## Where the pixels come from

Two paths, and which one an object takes is decided by
`bset #5, render_flags(a4)` at the top of `FieldObj_Animate`:

* **staged** (`FieldObj_Animate2`, no bit 5) -- the pattern word names VRAM,
  filled by the map record's Kosinski tileset list (`loc_519D2`) and Nemesis
  sprite list (`loc_51A1A`). This is every NPC.
* **streamed** (`FieldObj_Animate`, bit 5) -- `loc_44C5E` DMAs the tiles the
  frame names out of `art_ptr` into VRAM each time the frame changes, so the
  pattern word is an index into the object's own art blob. This is the party,
  the party-lookalike NPCs, and the objects whose art the map decompressed to
  work RAM (`$FFFE` entries, whose destination is `$FFFF0000 | tile << 5`).

The eleven party art blobs are uncompressed 2,304-byte, 72-pattern binaries
listed by `CharFieldArtPtrs`, paired with `CharSpriteMappingsPtrs`.

## The modules

* `records` -- the two record formats and the pattern-word arithmetic.
* `objects` -- `FieldObjectsJmpTbl` and each routine's init block.
* `compose` -- tile banks, frame composition, sheets, the census.
* `field` -- palettes, the party, a map's staged art, walk timing.
* `emit` -- writing sheets into a runtime pack; the only part that touches
  the filesystem, and the only part `psiv_tools.pack` needs beyond the above.

Decoded Sega pixels never enter a committed file: `emit` writes only into the
gitignored pack directory.
"""

from __future__ import annotations

from .compose import (
    Box,
    Frame,
    Sheet,
    SheetSequence,
    SpriteCensus,
    TileSource,
    build_sheet,
    compose_frame,
    effective_tile_word,
    mapping_box,
    union_box,
)
from .field import (
    CHAR_FIELD_ART_PTRS,
    CHAR_SPRITE_MAPPINGS_PTRS,
    MAP_PALETTE_LINE_ORDER,
    MOVEMENTS_TBL,
    MOVEMENT_BLOCKS,
    MOVEMENT_ENTRIES,
    MOVEMENT_ENTRY_SIZE,
    MOVEMENT_NORMAL_BLOCK,
    MOVEMENT_SELECTOR_MASK,
    MOVEMENT_SELECTOR_MASK_SITE,
    PAL_INIT_LINE_3,
    PAL_INIT_LINE_3_CRAM_LINE,
    PARTY_ART_BYTES,
    PARTY_ART_TILES,
    PARTY_SLOTS,
    PARTY_SYMBOLS,
    STATIC_SEQUENCE,
    ObjectSprite,
    PartySprite,
    StagedArt,
    StepTiming,
    char_field_art_pointers,
    object_sprite,
    pal_init_line_3,
    party_art,
    party_sprites,
    sprite_palette,
    stage_map_art,
    step_timing,
    step_timing_json,
    tile_source_from_patterns,
)
from .objects import (
    FIELD_OBJECTS_CLEAR_SIGNATURE,
    FIELD_OBJECTS_CLEAR_SITE,
    FIELD_OBJECTS_JMP_TBL,
    FIELD_OBJECTS_MEMORY_CLEAR_LONGS,
    FIELD_OBJ_DO_OBJ_COLLISION,
    RENDER_FLAG_CAMERA_BYPASS,
    INTERACTION_BTST_SIGNATURE,
    INTERACTION_CHK_OBJECTS,
    INTERACTION_LOOP_SIGNATURE,
    RENDER_FLAG_INTERACTABLE,
    FIELD_OBJECT_COUNT,
    FIELD_OBJECT_STRIDE,
    FIELD_OBJ_ANIMATE,
    FIELD_OBJ_ANIMATE2,
    FIELD_OBJ_ANIMATE_SIGNATURE,
    OFF_ART_PTR,
    OFF_ART_TILE,
    OFF_MAPPINGS,
    OFF_MAPPINGS_ADDR,
    OFF_MAPPINGS_DURATION,
    OFF_RENDER_FLAGS,
    OFF_SPRITE_TILE_PROPS,
    RENDER_FLAG_NO_SPRITES,
    FieldObjectRoutine,
    facing_table_extents,
    scan_field_objects,
)
from .vehicles import VEHICLE_RECORDS, VehicleSprite, vehicle_sprites
from .records import (
    FACINGS,
    FACING_NAMES,
    HFLIP_BIT,
    PIECE_SIZE,
    PRIORITY_BIT,
    RAM_BASE,
    ROM_LIMIT,
    SEQUENCE_PER_FRAME_FLAG,
    SPRITE_TILE_PROPS_LINES,
    TILE_INDEX_MASK,
    VFLIP_BIT,
    AnimationSequence,
    Mapping,
    Piece,
    SequenceFrame,
    SpriteError,
    decode_mapping,
    decode_sequence,
)

__all__ = [name for name in dir() if not name.startswith("_")]
