"""Turning mapping records into pixels: tile banks, frames and sheets.

Mega Drive sprite hardware behaviour -- column-major patterns, whole-piece
flips, colour 0 transparent -- plus the bookkeeping that folds a facing's
frames into a deduplicated sheet.
"""

from __future__ import annotations

import hashlib
import struct
from dataclasses import dataclass
from typing import Any, Iterable, Sequence

from ..gfx import TILE_HEIGHT, TILE_WIDTH, RGB, decode_tile
from ..nemesis import TILE_SIZE
from .records import (
    HFLIP_BIT,
    TILE_INDEX_MASK,
    VFLIP_BIT,
    AnimationSequence,
    Mapping,
    Piece,
    SpriteError,
    _hex,
)

# ---------------------------------------------------------------------------
# Art sources
# ---------------------------------------------------------------------------
class TileSource:
    """A bank of 8x8 palette-index tiles with holes in it.

    VRAM is the model: a map fills some pattern numbers and leaves the rest
    holding whatever the previous map left there, so `tile` answers `None`
    rather than a guess and every caller has to decide what that means.
    """

    def __init__(self, tiles: dict[int, bytes] | None = None) -> None:
        self._tiles: dict[int, bytes] = dict(tiles or {})

    def add(self, first: int, data: bytes) -> None:
        if len(data) % TILE_SIZE:
            raise SpriteError(
                f"art at tile {first} is {len(data)} bytes, not a whole number of patterns"
            )
        for index in range(len(data) // TILE_SIZE):
            self._tiles[first + index] = decode_tile(
                data[index * TILE_SIZE:(index + 1) * TILE_SIZE]
            )

    def tile(self, index: int) -> bytes | None:
        return self._tiles.get(index)

    def __contains__(self, index: int) -> bool:
        return index in self._tiles

    def __len__(self) -> int:
        return len(self._tiles)


# ---------------------------------------------------------------------------
# Frame composition
# ---------------------------------------------------------------------------
@dataclass(frozen=True)
class Frame:
    """One composed animation frame, palette-indexed, origin-relative."""

    width: int
    height: int
    origin_x: int
    origin_y: int
    pixels: bytes

    def sha256(self) -> str:
        return hashlib.sha256(self.pixels).hexdigest()


@dataclass(frozen=True)
class Box:
    left: int
    top: int
    right: int
    bottom: int

    @property
    def width(self) -> int:
        return self.right - self.left

    @property
    def height(self) -> int:
        return self.bottom - self.top


def mapping_box(mapping: Mapping) -> Box:
    left = min(p.x for p in mapping.pieces)
    top = min(p.y for p in mapping.pieces)
    right = max(p.x + p.width for p in mapping.pieces)
    bottom = max(p.y + p.height for p in mapping.pieces)
    return Box(left, top, right, bottom)


def union_box(boxes: Iterable[Box]) -> Box:
    boxes = list(boxes)
    if not boxes:
        raise SpriteError("cannot take the union of no boxes")
    return Box(
        min(b.left for b in boxes),
        min(b.top for b in boxes),
        max(b.right for b in boxes),
        max(b.bottom for b in boxes),
    )


def effective_tile_word(art_tile: int, tile_word: int, tile_props: int) -> int:
    """`Field_FillSpriteAttributes`' arithmetic, byte for byte.

    High bytes are added, `$13(a4)` is OR-ed into the result, *then* the low
    bytes are added and their carry is pushed back into the high byte. Doing it
    as a plain 16-bit add and OR is the same answer for every retail placement,
    but not in general, so it is written the way the routine writes it.
    """
    high = ((art_tile >> 8) + (tile_word >> 8)) & 0xFF
    high |= tile_props
    low = (art_tile & 0xFF) + (tile_word & 0xFF)
    if low > 0xFF:
        high = (high + 1) & 0xFF
        low &= 0xFF
    return (high << 8) | low


def _blit_piece(
    pixels: bytearray,
    width: int,
    height: int,
    piece: Piece,
    origin_x: int,
    origin_y: int,
    first_tile: int,
    source: TileSource,
    hflip: bool,
    vflip: bool,
) -> list[int]:
    """Draw one sprite piece; returns the tile indices the source was missing.

    Mega Drive sprite patterns run down each column before moving right, and a
    flip flag mirrors the whole piece rather than each tile, which is why the
    column and row are chosen before the pixels are read.
    """
    missing: list[int] = []
    across, down = piece.width_tiles, piece.height_tiles
    base_x = origin_x + piece.x
    base_y = origin_y + piece.y
    for column in range(across):
        for row in range(down):
            index = first_tile + column * down + row
            tile = source.tile(index & TILE_INDEX_MASK)
            if tile is None:
                missing.append(index & TILE_INDEX_MASK)
                continue
            target_column = across - 1 - column if hflip else column
            target_row = down - 1 - row if vflip else row
            for y in range(TILE_HEIGHT):
                source_y = TILE_HEIGHT - 1 - y if vflip else y
                out_y = base_y + target_row * TILE_HEIGHT + y
                if not 0 <= out_y < height:
                    continue
                line = tile[source_y * TILE_WIDTH:(source_y + 1) * TILE_WIDTH]
                if hflip:
                    line = line[::-1]
                for x in range(TILE_WIDTH):
                    value = line[x]
                    if not value:
                        continue
                    out_x = base_x + target_column * TILE_WIDTH + x
                    if 0 <= out_x < width:
                        pixels[out_y * width + out_x] = value
    return missing


class SpriteCensus:
    """Counters for the places retail data is narrower than the format allows.

    The same principle the pack manifest already applies to collision types and
    facing bytes: a consumer that infers the observed set from the legal one
    gets it wrong in both directions, so the counts are emitted rather than
    assumed. Every counter here is per drawn piece or frame, weighted by
    placement, because that is what "observed" means for art.
    """

    KEYS = (
        "frames",
        "pieces",
        "hflip_pieces",
        "vflip_pieces",
        "vflip_pieces_streamed",
        "tile_word_carries",
        "nonzero_mapping_header_bytes",
        "sequences",
        "per_frame_duration_sequences",
    )

    def __init__(self) -> None:
        self.counts: dict[str, int] = {key: 0 for key in self.KEYS}

    def note(self, key: str, by: int = 1) -> None:
        if key not in self.counts:
            raise SpriteError(f"{key!r} is not a sprite census counter")
        self.counts[key] += by

    def to_json(self) -> dict[str, int]:
        return dict(self.counts)


def compose_frame(
    mapping: Mapping,
    source: TileSource,
    box: Box,
    *,
    art_tile: int = 0,
    tile_props: int = 0,
    streamed: bool = False,
    census: SpriteCensus | None = None,
) -> tuple[Frame, list[int]]:
    """Draw one mapping into `box`, returning the frame and any missing tiles.

    `streamed` picks `loc_44C5E`'s reading of the pattern word -- an index into
    the object's own art blob, with only the H-flip bit honoured, because that
    routine rebuilds the tile number from a running counter and takes the flip
    from the mapping byte directly. Otherwise the word is added to `art_tile`
    and names VRAM, where both flips apply.
    """
    width, height = box.width, box.height
    pixels = bytearray(width * height)
    origin_x, origin_y = -box.left, -box.top
    missing: list[int] = []
    if census is not None:
        census.note("frames")
        census.note("pieces", len(mapping.pieces))
        if mapping.header_byte:
            census.note("nonzero_mapping_header_bytes")
    for piece in mapping.pieces:
        if streamed:
            word = piece.tile_word
            first = word & TILE_INDEX_MASK
            hflip = bool(word & HFLIP_BIT)
            # `loc_44C5E` rebuilds the pattern number from a running counter
            # and takes only the H-flip bit out of the mapping, so a V-flip in
            # a streamed frame would simply not happen. None do; the census
            # counts them so that stays a fact rather than an assumption.
            vflip = False
            if census is not None and piece.tile_word & VFLIP_BIT:
                census.note("vflip_pieces_streamed")
        else:
            word = effective_tile_word(art_tile, piece.tile_word, tile_props)
            first = word & TILE_INDEX_MASK
            hflip = bool(word & HFLIP_BIT)
            vflip = bool(word & VFLIP_BIT)
            if census is not None and (art_tile & 0xFF) + (piece.tile_word & 0xFF) > 0xFF:
                census.note("tile_word_carries")
        if census is not None:
            if hflip:
                census.note("hflip_pieces")
            if vflip:
                census.note("vflip_pieces")
        missing += _blit_piece(
            pixels, width, height, piece, origin_x, origin_y, first, source, hflip, vflip
        )
    return Frame(width, height, origin_x, origin_y, bytes(pixels)), missing


# ---------------------------------------------------------------------------
# Sheets
# ---------------------------------------------------------------------------
@dataclass(frozen=True)
class SheetSequence:
    """One facing's animation, as frame indices into a sheet."""

    facing: int
    frames: tuple[int, ...]
    durations: tuple[int, ...]
    sequence_offset: int

    def to_json(self) -> dict[str, Any]:
        return {
            "frames": [
                {"index": index, "duration_ticks": duration}
                for index, duration in zip(self.frames, self.durations)
            ],
            "loop": True,
            "sequence_offset": _hex(self.sequence_offset),
        }


@dataclass(frozen=True)
class Sheet:
    """A set of distinct frames plus the sequences that index them."""

    frames: tuple[Frame, ...]
    sequences: dict[str, SheetSequence]
    box: Box
    palette: tuple[RGB, ...]
    palette_line: int
    mirrors: dict[int, int]

    @property
    def frame_count(self) -> int:
        return len(self.frames)

    def identity(self) -> str:
        """A content hash: same pixels, geometry, palette and timing, same sheet."""
        digest = hashlib.sha256()
        digest.update(
            struct.pack(">iiiiB", self.box.left, self.box.top, self.box.right,
                        self.box.bottom, self.palette_line)
        )
        for colour in self.palette:
            digest.update(bytes(colour))
        for frame in self.frames:
            digest.update(frame.pixels)
        for name in sorted(self.sequences):
            sequence = self.sequences[name]
            digest.update(name.encode("ascii"))
            digest.update(bytes(sequence.frames))
            digest.update(struct.pack(f">{len(sequence.durations)}H", *sequence.durations))
        return digest.hexdigest()

    def strip(self) -> tuple[int, int, bytes]:
        """The frames laid out left to right in one row."""
        width = self.box.width * len(self.frames)
        height = self.box.height
        pixels = bytearray(width * height)
        for index, frame in enumerate(self.frames):
            left = index * self.box.width
            for y in range(height):
                start = y * width + left
                pixels[start:start + self.box.width] = frame.pixels[
                    y * self.box.width:(y + 1) * self.box.width
                ]
        return width, height, bytes(pixels)


def _mirror(frame: Frame, box: Box) -> bytes:
    """A frame reflected in the vertical line through its own box."""
    out = bytearray(len(frame.pixels))
    for y in range(box.height):
        row = frame.pixels[y * box.width:(y + 1) * box.width]
        out[y * box.width:(y + 1) * box.width] = row[::-1]
    return bytes(out)


def build_sheet(
    composed: Sequence[tuple[str, int, AnimationSequence, list[Frame]]],
    box: Box,
    palette: Sequence[RGB],
    palette_line: int,
) -> Sheet:
    """Fold per-facing frame lists into one deduplicated sheet.

    `composed` is `(name, facing, sequence, frames)` in emission order; each
    entry becomes an `idle_<name>` of its first frame and a `walk_<name>` of
    the whole cycle, which is the split `FieldObj_Move` makes when it resets
    `mappings_idx` on a stopped object.

    Identical frames collapse: a walk cycle is idle, step, idle, step, so the
    idle frame is one image referenced twice. Frames that are another frame's
    exact horizontal reflection are recorded in `mirrors` -- retail builds the
    right-facing mappings out of the left-facing tiles with the pattern word's
    H-flip bit, so a consumer that would rather flip at draw time can.
    """
    frames: list[Frame] = []
    by_pixels: dict[bytes, int] = {}
    sequences: dict[str, SheetSequence] = {}

    for name, facing, sequence, composed_frames in composed:
        indices = []
        for frame in composed_frames:
            index = by_pixels.get(frame.pixels)
            if index is None:
                index = len(frames)
                by_pixels[frame.pixels] = index
                frames.append(frame)
            indices.append(index)
        durations = tuple(f.ticks for f in sequence.frames)
        sequences[f"walk_{name}"] = SheetSequence(
            facing=facing,
            frames=tuple(indices),
            durations=durations,
            sequence_offset=sequence.rom_offset,
        )
        sequences[f"idle_{name}"] = SheetSequence(
            facing=facing,
            frames=(indices[0],),
            durations=(durations[0],),
            sequence_offset=sequence.rom_offset,
        )

    mirrors: dict[int, int] = {}
    for index, frame in enumerate(frames):
        reflected = _mirror(frame, box)
        for other in range(index):
            if frames[other].pixels == reflected:
                mirrors[index] = other
                break

    return Sheet(
        frames=tuple(frames),
        sequences=sequences,
        box=box,
        palette=tuple(palette),
        palette_line=palette_line,
        mirrors=mirrors,
    )
