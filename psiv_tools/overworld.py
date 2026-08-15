"""The two overworlds' paged layouts: MapID 0 (Motavia) and MapID 1 (Dezolis).

Every other field map stores two Kosinski layout blobs and a pointer to each in
its record. The overworlds store neither. `loc_539E2` and `loc_53A04` -- the
loader steps that read those two pointers -- open with

    move.w  (Field_Map_Index).w, d0
    andi.w  #$FFFE, d0
    beq.w   loc_53A26           ; loc_53A8E for the BG plane

so map indexes 0 and 1 branch away before the `movea.l (a0),a0`, consume no
record bytes, and build their planes from fixed tables instead. This module is
that other path, transcribed from `loc_53A26` / `loc_53A8E` / `loc_53BB6` /
`loc_53CAE` / `GetMapLayoutChunkFG` / `GetMapLayoutChunkBG` rather than from a
description of the format.

The format
----------

A plane is sixteen **pages**. A page is 1,024 bytes of raw, uncompressed chunk
ids -- eight rows of 128 -- and `loc_53BB6` copies one with

    move.w  #$FF, d2
loc_53BBA:
    move.l  (a0)+, (a1)+
    dbf     d2, loc_53BBA

256 longs, no decompressor anywhere in the path. Nothing about an overworld
layout is compressed; the chunk *definitions* it indexes still are, and come
through the record's ordinary `Map_LoadChunks` list.

`loc_53A26` picks the page from the camera:

    d0 = (Camera_Y_Pos_FG >> 8) & $F      page index, 16 pages, wraps
    d2 = (d0 & 3) << 10                   where it lands in Map_Layout_FG
    a2 = page table; a0 = (a2 + d0 * 4)   one long per page

and then copies four consecutive pages, stepping `d0` by 4 (mod $40, i.e. mod
16 pages), `d1` by 1 (mod 16) and `d2` by $400 (mod $FFF). So `Map_Layout_FG`
is a 4,096-byte rolling window holding four pages -- 32 chunk rows -- keyed by
page index mod 4. `RefreshMapLayout` streams one further page in whenever the
camera crosses a 256-pixel boundary, which is the same copy against the same
table.

`GetMapLayoutChunkFG` confirms both numbers from the other end:

    andi.w  #$1F, d1     ; 32 rows in the window
    lsl.w   #7, d1       ; 128 bytes per row
    add.w   d1, d0

and `GetChunkAndCollision` adds the matching `andi.w #$1F,d6` on the chunk row,
guarded by the same `Field_Map_Index & $FFFE` test, so the wrap is the
overworld's alone.

Dimensions therefore come out as 128 chunks wide by 16 pages * 8 rows = 128
chunks tall: 4,096 x 4,096 pixels, 256 x 256 collision cells. Both records
agree -- their four dimension bytes are $7F $7F $7F $7F -- and
`GetChunkAndCollision` wraps the walker's position at `(size + 1) << 5` = 4,096
pixels on both axes, which is what makes an overworld a globe rather than a
room.

Which table serves which planet
-------------------------------

`Field_Map_Index` is a *word* at $FFFFEC28, so `btst #0,($FFFFEC29).w` -- the
test every one of these routines uses to choose a table -- is bit 0 of the map
index. Clear selects the first table and set the second, i.e. MapID 0 is
Motavia and MapID 1 is Dezolis:

    plane   Motavia (map 0)     Dezolis (map 1)
    FG      loc_107DC2          loc_115584
    BG      loc_10BE02          loc_1175C4

Each table is 16 longs and its pages follow it contiguously, so a table's own
address is the proof of the previous table's extent: Motavia's FG pages run
$107E02..$10BE02, which is exactly where its BG table starts, and its BG pages
end at $10FE42, which is the map record's palette pointer.

Dezolis is half a world. Its tables hold eight distinct pages and then repeat
the eighth for indexes 8..$F, so chunk rows 64..127 are eight copies of rows
56..63 -- a solid rim with one walkable row along its top edge. The layout is
still 128 rows because that is what the record declares and what the position
wrap uses; the repetition is recorded as an anomaly rather than trimmed away.

Event patches
-------------

The copy routines do not stop at copying. `loc_53BB6` ends with

    add.w   d1, d1
    lea     (loc_53BDC).l, a2   ; loc_53BFC when Field_Map_Index bit 0 is set
    adda.w  (a2,d1.w), a2
    jmp     (a2)

a 16-entry jump table indexed by the page just copied, and the entries that are
not `rts` rewrite cells of that page from event flags -- doors that only exist
once the story has opened them. Five of the six overworld doorways that cover
no map-change cell in the stored layout are one of these (the two spaceports,
Machine Center, and the three transitions into The Edge); the sixth is Dezolis'
spaceport. `decode_patches` reads those routines out of the cartridge with a
decoder for exactly the instruction forms they use, so the patch data is ROM
data and an unrecognised opcode raises instead of being skipped.

Two things this module deliberately does not model, both reported rather than
guessed:

* `MapDataMan_Dezolis` (`MapDataManagerJmpTbl` entry $26, listed by Dezolis'
  record and called a second time from `loc_51B18`) rewrites *chunk
  definitions* in `Chunk_Table` -- not layout cells -- once
  `EventFlag_DarkForce2` or `EventFlag_EclipseTorch` is set, which changes both
  the picture and the collision of every cell using those chunk ids. It is a
  `MapDataManager` routine like the ones that open Zema's doors, and this pack
  emits no `MapDataManager` effects for any map.
* `MapDataMan_MovingPlatforms` (entry $14, listed by both overworlds) only
  clears six temporary event flags and touches no layout at all.
"""

from __future__ import annotations

import hashlib
import struct
from dataclasses import dataclass
from typing import Any, Sequence

from .layouts import (
    CHUNK_PIXELS_X,
    CHUNK_PIXELS_Y,
    COLLISION_CELL_TILES,
    PLANE_BG,
    PLANE_FG,
    PLANES,
    Blob,
    Layout,
    MapLayout,
    MapLayoutSpec,
    collision_at,
    decode_chunks,
    decode_collision,
    decode_tilesets,
)

#: `Field_Map_Index` values that take the paged path, i.e. the two the
#: `andi.w #$FFFE` test leaves at zero.
MOTAVIA = 0x00
DEZOLIS = 0x01
OVERWORLD_MAP_IDS = (MOTAVIA, DEZOLIS)

#: `move.w #$FF,d2` around a `move.l (a0)+,(a1)+`: 256 longs.
PAGE_BYTES = 0x400
#: `GetMapLayoutChunkFG`: `lsl.w #7,d1`.
ROW_BYTES = 128
PAGE_ROWS = PAGE_BYTES // ROW_BYTES
#: `andi.w #$F,d0` on the page index, `andi.w #$3F,d0` on its byte offset.
PAGE_COUNT = 16
#: `andi.w #$1F,d1` in `GetMapLayoutChunkFG` and `andi.w #$1F,d6` in
#: `GetChunkAndCollision`: the RAM window is four pages deep.
WINDOW_ROWS = 32
WINDOW_PAGES = WINDOW_ROWS // PAGE_ROWS

WIDTH_CHUNKS = ROW_BYTES
HEIGHT_CHUNKS = PAGE_COUNT * PAGE_ROWS
WIDTH_PIXELS = WIDTH_CHUNKS * CHUNK_PIXELS_X
HEIGHT_PIXELS = HEIGHT_CHUNKS * CHUNK_PIXELS_Y

#: The two `Map_Layout` regions, as `GetMapLayoutChunkFG`/`...BG` name them. A
#: patch reaches the other plane by a negative displacement, so the writes are
#: resolved as RAM addresses rather than as offsets within one plane.
MAP_LAYOUT_FG = 0xA000
MAP_LAYOUT_BG = 0xB000
LAYOUT_RAM_SPAN = 0x1000

#: `GetChunkAndCollision` type 1, the only one `MapTransTile_MapChange` fires
#: from. `psiv_tools.pack` names the same code for the same reason and the test
#: suite pins that the two agree.
MAP_CHANGE_COLLISION_TYPE = 0x1


class OverworldError(ValueError):
    pass


# ---------------------------------------------------------------------------
# Code sites
# ---------------------------------------------------------------------------
#: `lea (abs).l, a2`, the only addressing form these tables are reached by.
_LEA_A2 = bytes.fromhex("45F9")
#: `btst #0,($FFFFEC29).w` followed by `beq.s *+8`, which skips the second
#: `lea`. Pinning the whole shape is what proves bit 0 clear takes the first
#: table rather than the second.
_BTST_BIT0_MAP_INDEX = bytes.fromhex("083800 00EC29") + bytes.fromhex("6706")


def _table_selector(first: int, second: int) -> bytes:
    return (
        _LEA_A2 + struct.pack(">I", first)
        + _BTST_BIT0_MAP_INDEX
        + _LEA_A2 + struct.pack(">I", second)
    )


@dataclass(frozen=True)
class CodeSite:
    """One `lea default / btst / lea alternate` selector in the load path."""

    label: str
    rom_offset: int
    motavia: int
    dezolis: int

    @property
    def signature(self) -> bytes:
        return _table_selector(self.motavia, self.dezolis)

    def table_for(self, map_id: int) -> int:
        return self.dezolis if map_id & 1 else self.motavia

    def to_json(self) -> dict[str, Any]:
        return {
            "label": self.label,
            "rom_offset": f"0x{self.rom_offset:06X}",
            "selector": "btst #0,(Field_Map_Index+1); clear = Motavia, set = Dezolis",
            "motavia": f"0x{self.motavia:06X}",
            "dezolis": f"0x{self.dezolis:06X}",
        }


#: The four selectors this module depends on, at their retail addresses. The
#: page tables are the ones `loc_53A26`/`loc_53A8E` load; the jump tables are
#: the ones `loc_53BB6`/`loc_53CAE` dispatch through after copying a page.
PAGE_TABLE_SITES: dict[str, CodeSite] = {
    PLANE_FG: CodeSite("loc_53A26", 0x053A44, 0x107DC2, 0x115584),
    PLANE_BG: CodeSite("loc_53A8E", 0x053AAC, 0x10BE02, 0x1175C4),
}
PATCH_TABLE_SITES: dict[str, CodeSite] = {
    PLANE_FG: CodeSite("loc_53BB6", 0x053BC2, 0x053BDC, 0x053BFC),
    PLANE_BG: CodeSite("loc_53CAE", 0x053CBA, 0x053CD4, 0x053CF4),
}

#: `move.w #$FF,d2 / move.l (a0)+,(a1)+ / dbf d2 / add.w d1,d1` -- the raw copy
#: itself, checked so that "no compression" is a property of the cartridge and
#: not of this module's reading of it.
PAGE_COPY_SITES: dict[str, int] = {PLANE_FG: 0x053BB6, PLANE_BG: 0x053CAE}
PAGE_COPY_SIGNATURE = bytes.fromhex("343C00FF" "22D8" "51CAFFFC" "D241")

#: `GetMapLayoutChunkFG` / `...BG`, which fix the 32-row window and 128-byte
#: row stride every patch displacement is measured against.
GET_MAP_LAYOUT_CHUNK_FG = 0x053EE6
GET_MAP_LAYOUT_CHUNK_BG = 0x053EEC
GET_MAP_LAYOUT_CHUNK_SIGNATURE = bytes.fromhex(
    "43F8A000" "6004" "43F8B000" "0241001F" "EF49" "D041" "43F10000" "4E75"
)

EVENT_FLAGS_TEST = 0x057624

#: Only the flags these nine routines test. `psiv_tools.symbols` has no event
#: flag table yet; if a second module needs one, this belongs there.
EVENT_FLAG_SYMBOLS: dict[int, str] = {
    0x34: "EventFlag_BioPlantEscape",
    0x35: "EventFlag_RikaJoined",
    0x43: "EventFlag_MachineCenter",
    0x65: "EventFlag_ZioNurvus",
    0x66: "EventFlag_MotaSpaceport",
    0x82: "EventFlag_DezoSpaceport",
    0x9E: "EventFlag_DarkForce2",
    0xDA: "EventFlag_Reunion",
}


def is_overworld(map_id: int) -> bool:
    """The loader's own test: `Field_Map_Index & $FFFE == 0`."""
    return not map_id & 0xFFFE


def _require_overworld(map_id: int) -> None:
    if not is_overworld(map_id):
        raise OverworldError(
            f"map 0x{map_id:03X} reads its layout from its record; only "
            f"{list(OVERWORLD_MAP_IDS)} take the paged path"
        )


def _check_bytes(rom: bytes, offset: int, expected: bytes, what: str) -> None:
    actual = rom[offset:offset + len(expected)]
    if actual != expected:
        raise OverworldError(
            f"{what} at 0x{offset:06X}: expected {expected.hex()}, got {actual.hex()}"
        )


def check_code_sites(rom: bytes) -> dict[str, Any]:
    """Prove every address this module hardcodes against the retail opcodes.

    None of the tables below is reachable from data: they are `lea` operands
    inside the load path, so the only honest way to bind them is to check the
    instructions that name them. A ROM that disagrees fails here rather than
    silently decoding sixteen kilobytes of something else.
    """
    for plane, site in PAGE_TABLE_SITES.items():
        _check_bytes(rom, site.rom_offset, site.signature, f"{plane.upper()} page table selector")
    for plane, site in PATCH_TABLE_SITES.items():
        _check_bytes(rom, site.rom_offset, site.signature, f"{plane.upper()} patch table selector")
    for plane, offset in PAGE_COPY_SITES.items():
        _check_bytes(rom, offset, PAGE_COPY_SIGNATURE, f"{plane.upper()} page copy loop")
    _check_bytes(
        rom, GET_MAP_LAYOUT_CHUNK_FG, GET_MAP_LAYOUT_CHUNK_SIGNATURE, "GetMapLayoutChunkFG"
    )
    return {
        "page_tables": {plane: site.to_json() for plane, site in PAGE_TABLE_SITES.items()},
        "patch_tables": {plane: site.to_json() for plane, site in PATCH_TABLE_SITES.items()},
        "page_copy": {
            "routines": {plane: f"0x{offset:06X}" for plane, offset in PAGE_COPY_SITES.items()},
            "signature_hex": PAGE_COPY_SIGNATURE.hex(),
            "compression": None,
            "longs_copied": PAGE_BYTES // 4,
        },
        "get_map_layout_chunk": {
            "fg": f"0x{GET_MAP_LAYOUT_CHUNK_FG:06X}",
            "bg": f"0x{GET_MAP_LAYOUT_CHUNK_BG:06X}",
            "signature_hex": GET_MAP_LAYOUT_CHUNK_SIGNATURE.hex(),
            "window_rows": WINDOW_ROWS,
            "row_bytes": ROW_BYTES,
        },
    }


# ---------------------------------------------------------------------------
# Page tables
# ---------------------------------------------------------------------------
@dataclass(frozen=True)
class Page:
    """One 1,024-byte block of chunk ids, as one table entry names it."""

    index: int
    rom_offset: int
    sha256: str
    #: The lowest page index sharing this offset, when the table repeats one.
    aliases: int | None

    @property
    def first_row(self) -> int:
        return self.index * PAGE_ROWS

    def to_json(self) -> dict[str, Any]:
        return {
            "index": self.index,
            "first_row": self.first_row,
            "rom_offset": f"0x{self.rom_offset:06X}",
            "rom_end": f"0x{self.rom_offset + PAGE_BYTES:06X}",
            "sha256": self.sha256,
            "aliases_page": self.aliases,
        }


@dataclass(frozen=True)
class PageTable:
    """The sixteen longs one plane of one overworld streams its rows from."""

    plane: str
    map_id: int
    rom_offset: int
    pages: tuple[Page, ...]

    @property
    def distinct_pages(self) -> tuple[Page, ...]:
        return tuple(page for page in self.pages if page.aliases is None)

    @property
    def data_start(self) -> int:
        return min(page.rom_offset for page in self.pages)

    @property
    def data_end(self) -> int:
        return max(page.rom_offset for page in self.pages) + PAGE_BYTES

    def cells(self, rom: bytes) -> bytes:
        """The whole plane, pages concatenated in table order."""
        return b"".join(rom[p.rom_offset:p.rom_offset + PAGE_BYTES] for p in self.pages)

    def to_json(self) -> dict[str, Any]:
        return {
            "plane": self.plane,
            "rom_offset": f"0x{self.rom_offset:06X}",
            "entry_size": 4,
            "page_count": len(self.pages),
            "distinct_pages": len(self.distinct_pages),
            "page_bytes": PAGE_BYTES,
            "page_rows": PAGE_ROWS,
            "compression": None,
            "data_rom_offset": f"0x{self.data_start:06X}",
            "data_rom_end": f"0x{self.data_end:06X}",
            "pages": [page.to_json() for page in self.pages],
        }


def read_page_table(rom: bytes, map_id: int, plane: str) -> PageTable:
    """Read one plane's sixteen page pointers, in table order.

    The pages of a table are contiguous and in order in the cartridge, which is
    what makes the next table's address a bound on this one's data; a table that
    is not is a table this module is reading in the wrong place, so it raises.
    """
    _require_overworld(map_id)
    if plane not in PLANES:
        raise OverworldError(f"Plane must be one of {PLANES}, got {plane!r}")
    site = PAGE_TABLE_SITES[plane]
    _check_bytes(rom, site.rom_offset, site.signature, f"{plane.upper()} page table selector")
    offset = site.table_for(map_id)
    if offset + PAGE_COUNT * 4 > len(rom):
        raise OverworldError(f"Page table at 0x{offset:06X} runs past the end of the ROM")

    pointers = struct.unpack_from(f">{PAGE_COUNT}I", rom, offset)
    seen: dict[int, int] = {}
    pages: list[Page] = []
    for index, pointer in enumerate(pointers):
        if pointer + PAGE_BYTES > len(rom):
            raise OverworldError(
                f"Page {index} of the {plane.upper()} table at 0x{offset:06X} points at "
                f"0x{pointer:06X}, which does not hold {PAGE_BYTES} bytes"
            )
        pages.append(Page(
            index=index,
            rom_offset=pointer,
            sha256=hashlib.sha256(rom[pointer:pointer + PAGE_BYTES]).hexdigest(),
            aliases=seen.get(pointer),
        ))
        seen.setdefault(pointer, index)

    distinct = [page.rom_offset for page in pages if page.aliases is None]
    expected = list(range(distinct[0], distinct[0] + len(distinct) * PAGE_BYTES, PAGE_BYTES))
    if distinct != expected:
        raise OverworldError(
            f"The {len(distinct)} distinct pages of the {plane.upper()} table at "
            f"0x{offset:06X} are not contiguous and ascending; the table is being "
            "read in the wrong place"
        )
    return PageTable(plane=plane, map_id=map_id, rom_offset=offset, pages=tuple(pages))


def _plane_blob(rom: bytes, table: PageTable) -> Blob:
    """One plane's page data as a single `Blob`.

    `Blob` describes a Kosinski stream everywhere else in this project. An
    overworld plane is copied raw, so the compressed and decompressed lengths
    are the same number and that is the point: the region is exactly as long as
    the rows it produces.
    """
    span = table.data_end - table.data_start
    return Blob(
        rom_offset=table.data_start,
        compressed_length=span,
        decompressed_length=span,
        sha256=hashlib.sha256(rom[table.data_start:table.data_end]).hexdigest(),
    )


def decode_plane(rom: bytes, table: PageTable) -> Layout:
    """Assemble one plane's 128x128 chunk grid from its page table."""
    cells = table.cells(rom)
    if len(cells) != WIDTH_CHUNKS * HEIGHT_CHUNKS:
        raise OverworldError(
            f"{len(table.pages)} pages of {PAGE_BYTES} bytes make {len(cells)} cells, "
            f"not the {WIDTH_CHUNKS * HEIGHT_CHUNKS} a "
            f"{WIDTH_CHUNKS}x{HEIGHT_CHUNKS} grid needs"
        )
    anomaly = None
    aliased = [page for page in table.pages if page.aliases is not None]
    if aliased:
        first = aliased[0]
        anomaly = {
            "plane": table.plane,
            "kind": "aliased_pages",
            "table_offset": f"0x{table.rom_offset:06X}",
            "page_count": len(table.pages),
            "distinct_pages": len(table.distinct_pages),
            "aliased_pages": [
                {"index": page.index, "aliases_page": page.aliases} for page in aliased
            ],
            "effect": (
                f"chunk rows {first.first_row}..{HEIGHT_CHUNKS - 1} repeat rows "
                f"{first.aliases * PAGE_ROWS}..{first.aliases * PAGE_ROWS + PAGE_ROWS - 1}; "
                "the table stores the same page pointer for those indexes"
            ),
        }
    return Layout(
        plane=table.plane,
        width_chunks=WIDTH_CHUNKS,
        height_chunks=HEIGHT_CHUNKS,
        cells=cells,
        blob=_plane_blob(rom, table),
        anomaly=anomaly,
    )


# ---------------------------------------------------------------------------
# Event patches
# ---------------------------------------------------------------------------
@dataclass(frozen=True)
class PatchWrite:
    """One store into `Map_Layout`, resolved to plane and chunk coordinates."""

    plane: str
    chunk_x: int
    chunk_y: int
    chunk_ids: tuple[int, ...]
    displacement: int

    def to_json(self) -> dict[str, Any]:
        # `cell_x`/`cell_y` are the top-left of the 2x2 block of collision cells
        # the first written chunk covers, so a consumer can line a patch up with
        # `collision.rows` without redoing the arithmetic.
        return {
            "plane": self.plane,
            "chunk_x": self.chunk_x,
            "chunk_y": self.chunk_y,
            "cell_x": self.chunk_x * COLLISION_CELL_TILES,
            "cell_y": self.chunk_y * COLLISION_CELL_TILES,
            "chunk_ids": [f"0x{value:02X}" for value in self.chunk_ids],
            "displacement": self.displacement,
        }


@dataclass(frozen=True)
class LayoutPatch:
    """One event-flag-gated block of writes inside one page's hook routine."""

    plane: str
    page: int
    routine: int
    event_flag: int
    writes: tuple[PatchWrite, ...]

    @property
    def event_symbol(self) -> str | None:
        return EVENT_FLAG_SYMBOLS.get(self.event_flag)

    def to_json(self) -> dict[str, Any]:
        return {
            "triggered_by_plane": self.plane,
            "page": self.page,
            "routine": f"0x{self.routine:06X}",
            "event_flag": {
                "id": self.event_flag,
                "id_hex": f"0x{self.event_flag:02X}",
                "symbol": self.event_symbol,
            },
            "writes": [write.to_json() for write in self.writes],
        }


class _PatchDecoder:
    """A reader for the instruction forms the nine hook routines are made of.

    This is not a 68000 disassembler and must not become one. It accepts the
    fourteen encodings these routines use and raises on everything else, so a
    routine that grows an instruction this module has not read is a failure and
    not a silently truncated patch.
    """

    _RTS = 0x4E75
    _MOVE_W_IMM_D0 = 0x303C
    _MOVE_W_IMM_D1 = 0x323C
    _JSR_ABS_L = 0x4EB9
    _BSR_W = 0x6100
    _MOVEQ_0_D0 = 0x7000
    _BEQ_SHORT = 0x67

    #: The eight stores into `(a1)`, as `size in bytes, source, has displacement`.
    #: A byte immediate is still encoded as a word, high half ignored.
    _STORES: dict[int, tuple[int, str, bool]] = {
        0x22BC: (4, "immediate", False),   # move.l #imm,(a1)
        0x237C: (4, "immediate", True),    # move.l #imm,d(a1)
        0x2280: (4, "d0", False),          # move.l d0,(a1)
        0x2340: (4, "d0", True),           # move.l d0,d(a1)
        0x12BC: (1, "immediate", False),   # move.b #imm,(a1)
        0x137C: (1, "immediate", True),    # move.b #imm,d(a1)
        0x1280: (1, "d0", False),          # move.b d0,(a1)
        0x1340: (1, "d0", True),           # move.b d0,d(a1)
    }

    def __init__(self, rom: bytes, plane: str, page: int, routine: int):
        self.rom = rom
        self.plane = plane
        self.page = page
        self.routine = routine
        self.pos = routine

    def fail(self, message: str) -> None:
        raise OverworldError(
            f"{self.plane.upper()} page {self.page} hook at 0x{self.routine:06X}, "
            f"offset 0x{self.pos:06X}: {message}"
        )

    def word(self) -> int:
        value = int.from_bytes(self.rom[self.pos:self.pos + 2], "big")
        self.pos += 2
        return value

    def signed_word(self) -> int:
        value = self.word()
        return value - 0x10000 if value & 0x8000 else value

    def long(self) -> int:
        value = int.from_bytes(self.rom[self.pos:self.pos + 4], "big")
        self.pos += 4
        return value

    def decode(self) -> list[LayoutPatch]:
        patches: list[LayoutPatch] = []
        while True:
            opcode = self.word()
            if opcode == self._RTS:
                return patches
            if opcode != self._MOVE_W_IMM_D0:
                self.fail(f"opcode 0x{opcode:04X} is not `move.w #imm,d0` or `rts`")
            flag = self.word()
            if self.word() != self._JSR_ABS_L or self.long() != EVENT_FLAGS_TEST:
                self.fail("the flag load is not followed by `jsr (EventFlags_Test).l`")
            branch = self.word()
            if branch >> 8 != self._BEQ_SHORT or not branch & 0xFF:
                self.fail(f"expected a short `beq` after the flag test, got 0x{branch:04X}")
            end = self.pos + (branch & 0xFF)
            patches.append(LayoutPatch(
                plane=self.plane,
                page=self.page,
                routine=self.routine,
                event_flag=flag,
                writes=tuple(self.block(end)),
            ))

    def block(self, end: int) -> list[PatchWrite]:
        """One flag's writes, up to where its `beq` lands."""
        column = row = None
        base_plane: str | None = None
        d0_is_zero = False
        writes: list[PatchWrite] = []
        while self.pos < end:
            opcode = self.word()
            if opcode == self._MOVE_W_IMM_D0:
                column = self.word()
            elif opcode == self._MOVE_W_IMM_D1:
                row = self.word()
            elif opcode == self._BSR_W:
                target = self.pos + self.signed_word()
                if target == GET_MAP_LAYOUT_CHUNK_FG:
                    base_plane = PLANE_FG
                elif target == GET_MAP_LAYOUT_CHUNK_BG:
                    base_plane = PLANE_BG
                else:
                    self.fail(f"`bsr.w 0x{target:06X}` is not GetMapLayoutChunkFG/BG")
                if column is None or row is None:
                    self.fail("GetMapLayoutChunk is reached without both coordinates set")
                d0_is_zero = False
            elif opcode == self._MOVEQ_0_D0:
                d0_is_zero = True
            elif opcode in self._STORES:
                if base_plane is None:
                    self.fail("a store runs before GetMapLayoutChunk has set a1")
                writes.append(self._store(opcode, base_plane, column, row, d0_is_zero))
            else:
                self.fail(f"opcode 0x{opcode:04X} is outside this decoder's vocabulary")
        if self.pos != end:
            self.fail(f"the block overruns its `beq` target 0x{end:06X}")
        return writes

    def _store(
        self, opcode: int, base_plane: str, column: int, row: int, d0_is_zero: bool
    ) -> PatchWrite:
        size, source, has_displacement = self._STORES[opcode]
        if source == "immediate":
            value = self.long() if size == 4 else self.word() & 0xFF
        else:
            if not d0_is_zero:
                self.fail("a store reads d0 without a preceding `moveq #0,d0`")
            value = 0
        displacement = self.signed_word() if has_displacement else 0
        payload = tuple(value.to_bytes(size, "big"))
        return self._resolve(base_plane, column, row, displacement, payload)

    def _resolve(
        self,
        base_plane: str,
        column: int,
        row: int,
        displacement: int,
        payload: tuple[int, ...],
    ) -> PatchWrite:
        """Turn `d(a1)` into a plane and a chunk coordinate.

        `GetMapLayoutChunk` leaves `a1` at `plane + (row & $1F) * 128 + column`.
        A displacement of $F000 reaches the same cell of the other plane and one
        of $EF80 reaches the row above it there, so the only reading that works
        is the arithmetic one: resolve the RAM address, then say which of the two
        4KB regions it landed in and how far its row moved.
        """
        window_row = row & (WINDOW_ROWS - 1)
        base = (MAP_LAYOUT_FG if base_plane == PLANE_FG else MAP_LAYOUT_BG)
        address = base + window_row * ROW_BYTES + column + displacement
        if MAP_LAYOUT_FG <= address < MAP_LAYOUT_FG + LAYOUT_RAM_SPAN:
            plane = PLANE_FG
        elif MAP_LAYOUT_BG <= address < MAP_LAYOUT_BG + LAYOUT_RAM_SPAN:
            plane = PLANE_BG
        else:
            self.fail(
                f"displacement {displacement} puts the store at 0x{address:04X}, "
                "outside both Map_Layout regions"
            )
        offset = address & (LAYOUT_RAM_SPAN - 1)
        # Row moves are small and signed; the window is modular, so pick the
        # representative nearest zero rather than the positive remainder.
        delta = (offset // ROW_BYTES - window_row + WINDOW_ROWS // 2) % WINDOW_ROWS
        delta -= WINDOW_ROWS // 2
        chunk_x, chunk_y = offset % ROW_BYTES, row + delta
        if not 0 <= chunk_x < WIDTH_CHUNKS or not 0 <= chunk_y < HEIGHT_CHUNKS:
            self.fail(f"the store resolves to chunk ({chunk_x}, {chunk_y}), off the map")
        if chunk_x + len(payload) > WIDTH_CHUNKS:
            self.fail(f"a {len(payload)}-byte store at column {chunk_x} runs past the row")
        return PatchWrite(
            plane=plane,
            chunk_x=chunk_x,
            chunk_y=chunk_y,
            chunk_ids=payload,
            displacement=displacement,
        )


def read_patch_table(rom: bytes, map_id: int, plane: str) -> tuple[int, ...]:
    """The sixteen hook routine addresses one plane dispatches through.

    Entries are `dc.w routine - table`, which `adda.w (a2,d1.w),a2 / jmp (a2)`
    turns into an absolute address.
    """
    _require_overworld(map_id)
    site = PATCH_TABLE_SITES[plane]
    _check_bytes(rom, site.rom_offset, site.signature, f"{plane.upper()} patch table selector")
    base = site.table_for(map_id)
    offsets = struct.unpack_from(f">{PAGE_COUNT}h", rom, base)
    return tuple(base + offset for offset in offsets)


def decode_patches(rom: bytes, map_id: int) -> tuple[LayoutPatch, ...]:
    """Every event-gated layout write both planes' page hooks perform.

    A hook routine runs after its page is copied, so a patch belongs to a page
    and reaches only the 32-row window that page is part of; the writes are
    reported at absolute chunk coordinates, which is where they land for the
    only page index that can reach them.
    """
    _require_overworld(map_id)
    _check_bytes(
        rom, GET_MAP_LAYOUT_CHUNK_FG, GET_MAP_LAYOUT_CHUNK_SIGNATURE, "GetMapLayoutChunkFG"
    )
    patches: list[LayoutPatch] = []
    for plane in PLANES:
        for page, routine in enumerate(read_patch_table(rom, map_id, plane)):
            patches.extend(_PatchDecoder(rom, plane, page, routine).decode())
    return tuple(patches)


# ---------------------------------------------------------------------------
# One overworld, end to end
# ---------------------------------------------------------------------------
@dataclass(frozen=True)
class Overworld:
    """The decoded layout section MapID 0 and 1 do not carry in their records."""

    map_id: int
    symbol: str | None
    tables: dict[str, PageTable]
    layout: MapLayout
    patches: tuple[LayoutPatch, ...]
    anomalies: tuple[dict[str, Any], ...]

    @property
    def collision(self):
        return self.layout.collision

    def to_json(self) -> dict[str, Any]:
        """The format facts and provenance, without the patch list.

        The patches themselves belong to the map record a consumer loads, not
        to a summary of the format, so `psiv_tools.pack` emits them there and
        this reports only how many there are.
        """
        return {
            "map_id": self.map_id,
            "symbol": self.symbol,
            "source": "paged tables; the record carries no layout pointers",
            "width_chunks": WIDTH_CHUNKS,
            "height_chunks": HEIGHT_CHUNKS,
            "width_pixels": WIDTH_PIXELS,
            "height_pixels": HEIGHT_PIXELS,
            "page_bytes": PAGE_BYTES,
            "page_rows": PAGE_ROWS,
            "page_count": PAGE_COUNT,
            "window_rows": WINDOW_ROWS,
            "window_pages": WINDOW_PAGES,
            "row_bytes": ROW_BYTES,
            "compression": None,
            "wraps": {"x_pixels": WIDTH_PIXELS, "y_pixels": HEIGHT_PIXELS},
            "tables": {plane: table.to_json() for plane, table in self.tables.items()},
            "layout_fg": self.layout.fg.to_json(),
            "layout_bg": self.layout.bg.to_json(),
            "collision": self.layout.collision.to_json(),
            "patch_count": len(self.patches),
            "patched_events": sorted({
                patch.event_symbol or f"0x{patch.event_flag:02X}" for patch in self.patches
            }),
            "anomalies": list(self.anomalies),
        }


def overworld_spec(record: dict[str, Any]) -> MapLayoutSpec:
    """The `MapLayoutSpec` an overworld record describes.

    Every field but two comes from the record exactly as it does for an
    interior map. The exceptions are `layout_fg`/`layout_bg`, which for these
    two maps are not pointers the record stores at all: they are the page table
    addresses `loc_53A26`/`loc_53A8E` load instead, kept in the same fields so
    that one spec type describes both paths.
    """
    map_id = record["id"]
    _require_overworld(map_id)
    if record["layout"]["present"]:
        raise OverworldError(
            f"map 0x{map_id:03X} ({record['symbol']}) carries a layout section; it is "
            "not one of the paged world maps"
        )
    dimensions = record["dimensions"]
    return MapLayoutSpec.from_header(
        chunk_blobs=[int(p, 16) for p in record["chunks"]["pointers"]],
        layout_fg=PAGE_TABLE_SITES[PLANE_FG].table_for(map_id),
        layout_bg=PAGE_TABLE_SITES[PLANE_BG].table_for(map_id),
        dimension_bytes=[
            dimensions["fg_row_size"],
            dimensions["fg_column_size"],
            dimensions["bg_row_size"],
            dimensions["bg_column_size"],
        ],
        collision_plane=record["scroll"]["mode"],
        tilesets=[
            (entry["vram_tile"], int(entry["source_offset"], 16))
            for entry in record["tilesets"]["entries"]
        ],
        palette=int(record["palette"]["pointer"], 16),
        label=record["symbol"],
    )


def decode_overworld(
    rom: bytes,
    map_id: int,
    record: dict[str, Any] | None = None,
    *,
    with_tiles: bool = False,
) -> Overworld:
    """Decode one overworld's layout, collision and event patches.

    `record` is the map's entry from `psiv_tools.maps.extract_maps`; the chunk
    blobs, tilesets, palette and collision plane all come from it, because the
    paged path changes only where the two plane *layouts* come from. Without a
    record the chunk table and dimensions are still read, but from the map
    record this function extracts for itself.
    """
    _require_overworld(map_id)
    if record is None:
        from .maps import extract_maps

        record = extract_maps(rom)["maps"][map_id]
    if record["id"] != map_id:
        raise OverworldError(
            f"record 0x{record['id']:03X} was handed to map 0x{map_id:03X}"
        )
    check_code_sites(rom)

    spec = overworld_spec(record)
    tables = {plane: read_page_table(rom, map_id, plane) for plane in PLANES}
    planes = {plane: decode_plane(rom, table) for plane, table in tables.items()}
    fg, bg = planes[PLANE_FG], planes[PLANE_BG]
    if (spec.width_chunks_fg, spec.height_chunks_fg) != (WIDTH_CHUNKS, HEIGHT_CHUNKS):
        raise OverworldError(
            f"map 0x{map_id:03X} declares a "
            f"{spec.width_chunks_fg}x{spec.height_chunks_fg} FG grid, but the paged "
            f"path always builds {WIDTH_CHUNKS}x{HEIGHT_CHUNKS}"
        )
    if (spec.width_chunks_bg, spec.height_chunks_bg) != (WIDTH_CHUNKS, HEIGHT_CHUNKS):
        raise OverworldError(
            f"map 0x{map_id:03X} declares a "
            f"{spec.width_chunks_bg}x{spec.height_chunks_bg} BG grid, but the paged "
            f"path always builds {WIDTH_CHUNKS}x{HEIGHT_CHUNKS}"
        )

    chunks = decode_chunks(rom, spec.chunk_blobs)
    for layout in (fg, bg):
        highest = max(layout.cells)
        if highest >= len(chunks):
            raise OverworldError(
                f"{layout.plane.upper()} layout names chunk 0x{highest:02X} but the "
                f"map loads only {len(chunks)} chunks"
            )
    decoded = MapLayout(
        spec=spec,
        chunks=chunks,
        fg=fg,
        bg=bg,
        collision=decode_collision(chunks, bg if spec.collision_plane else fg),
        patterns=decode_tilesets(rom, spec.tilesets) if with_tiles else None,
    )
    return Overworld(
        map_id=map_id,
        symbol=record["symbol"],
        tables=tables,
        layout=decoded,
        patches=decode_patches(rom, map_id),
        anomalies=tuple(
            layout.anomaly for layout in (fg, bg) if layout.anomaly is not None
        ),
    )


def patched_cells(patches: Sequence[LayoutPatch], plane: str) -> dict[tuple[int, int], int]:
    """Every chunk cell one plane's patches rewrite, as `(x, y) -> chunk id`.

    Later patches win, which is the order the routines run in.
    """
    out: dict[tuple[int, int], int] = {}
    for patch in patches:
        for write in patch.writes:
            if write.plane != plane:
                continue
            for step, chunk_id in enumerate(write.chunk_ids):
                out[(write.chunk_x + step, write.chunk_y)] = chunk_id
    return out


def apply_patches(layout: Layout, patches: Sequence[LayoutPatch]) -> Layout:
    """One plane with the writes of `patches` applied, in routine order."""
    cells = bytearray(layout.cells)
    for (chunk_x, chunk_y), chunk_id in patched_cells(patches, layout.plane).items():
        cells[chunk_y * WIDTH_CHUNKS + chunk_x] = chunk_id
    return Layout(
        plane=layout.plane,
        width_chunks=layout.width_chunks,
        height_chunks=layout.height_chunks,
        cells=bytes(cells),
        blob=layout.blob,
        anomaly=layout.anomaly,
    )


def opening_patches(
    overworld: Overworld, cells: Sequence[tuple[int, int]]
) -> tuple[LayoutPatch, ...]:
    """The patches that turn one of `cells` into a map-change cell.

    A doorway whose trigger rectangle covers no map-change cell in the stored
    layout is a door the story has not opened yet, and this is how the pack
    proves which event opens it instead of asserting it from the disassembly's
    comments: apply one patch to the collision plane and see whether the cell
    the transition needs becomes type 1.
    """
    plane = overworld.layout.collision_layout
    chunks = overworld.layout.chunks
    before = {cell: collision_at(chunks, plane, *cell) for cell in cells}
    found: list[LayoutPatch] = []
    for patch in overworld.patches:
        if not any(write.plane == plane.plane for write in patch.writes):
            continue
        after = apply_patches(plane, [patch])
        if any(
            before[cell] != MAP_CHANGE_COLLISION_TYPE
            and collision_at(chunks, after, *cell) == MAP_CHANGE_COLLISION_TYPE
            for cell in cells
        ):
            found.append(patch)
    return tuple(found)
