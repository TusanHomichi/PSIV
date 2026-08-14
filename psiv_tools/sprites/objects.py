"""`FieldObjectsJmpTbl` and the one-time initialisation each routine runs.

Which sequence table an object follows, which art it draws, which CRAM line
it draws on and whether it draws at all are all immediates in that block, so
the block is read out of the cartridge rather than out of the clone.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Sequence

from ..symbols import FIELD_OBJECT_SYMBOLS
from .records import (
    FACINGS,
    SPRITE_TILE_PROPS_LINES,
    SpriteError,
    _hex,
    _s8,
    _s16,
    _u8,
    _u16,
    _u32,
)

# ---------------------------------------------------------------------------
# Retail offsets and the 68000 encodings the init-block scanner reads.
# ---------------------------------------------------------------------------
#: `FieldObjectsJmpTbl`. 222 `bra.w` entries; entry N is object id N * 4.
FIELD_OBJECTS_JMP_TBL = 0x04AA6C
FIELD_OBJECT_COUNT = 222
FIELD_OBJECT_STRIDE = 4

#: `FieldObj_Animate`, whose only job before falling into `FieldObj_Animate2`
#: is `bset #5, render_flags(a4)` -- the flag that switches the sprite builder
#: to the streaming path.
FIELD_OBJ_ANIMATE = 0x044728
FIELD_OBJ_ANIMATE_SIGNATURE = bytes.fromhex("08ec00050002302c0006226c0008")
FIELD_OBJ_ANIMATE2 = FIELD_OBJ_ANIMATE + 6

#: 68000 encodings the init-block scanner recognises. `(d16, A4)` is
#: mode 5 register 4, so the destination half of every one of these is fixed.
_MOVE_L_TO_A4 = 0x297C  # move.l #imm32, d16(a4)
_MOVE_W_TO_A4 = 0x397C  # move.w #imm16, d16(a4)
_MOVE_B_TO_A4 = 0x197C  # move.b #imm8,  d16(a4)
_BSET_TO_A4 = 0x08EC    # bset #imm, d16(a4)
_BCLR_TO_A4 = 0x08AC    # bclr #imm, d16(a4)
_BCHG_TO_A4 = 0x086C    # bchg #imm, d16(a4)
_BSET_A4_IND = 0x08D4   # bset #imm, (a4)
_MOVEQ = 0x7000         # moveq #imm8, dN  (mask 0xF100)
#: move.<size> dN, d16(a4) -- mask 0xFFF8, the low three bits are the register.
_MOVE_D_TO_A4 = (("b", 0x1940), ("w", 0x3940), ("l", 0x2940))
_BIT_TO_A4 = {_BSET_TO_A4: "bset", _BCLR_TO_A4: "bclr", _BCHG_TO_A4: "bchg"}
_BRA_W = 0x6000
_BSR_W = 0x6100
_JMP_ABS_L = 0x4EF9
_JSR_ABS_L = 0x4EB9

#: Field object structure offsets, from `ps4.constants.asm` plus the two the
#: constants file leaves unnamed.
OFF_RENDER_FLAGS = 0x02
OFF_MAPPINGS_ADDR = 0x08
OFF_MAPPINGS = 0x0C
OFF_MAPPINGS_DURATION = 0x11
#: unnamed in the disassembly; the battle struct calls its twin
#: `battle_sprite_tile_props1`.
OFF_SPRITE_TILE_PROPS = 0x13
OFF_ART_TILE = 0x16
OFF_ART_PTR = 0x18

#: `render_flags` bit 1: `Field_FillSpriteAttributes` returns on it.
RENDER_FLAG_NO_SPRITES = 1


# ---------------------------------------------------------------------------
# The field-object jump table and its init blocks
# ---------------------------------------------------------------------------
@dataclass(frozen=True)
class FieldObjectRoutine:
    """What one `FieldObjectsJmpTbl` routine sets up before it starts running."""

    index: int
    symbol: str
    rom_offset: int
    init_start: int
    init_end: int
    mappings_addr: int | None
    mappings: int | None
    art_ptr: int | None
    art_tile: int | None
    sprite_tile_props: int | None
    alternate_tile_props: tuple[int, ...]
    mappings_duration: int | None
    builds_sprites: bool
    starts_hidden: bool
    animates: bool
    streams_art: bool

    @property
    def object_id(self) -> int:
        return self.index * FIELD_OBJECT_STRIDE

    @property
    def palette_line(self) -> int | None:
        if self.sprite_tile_props is None:
            return None
        return SPRITE_TILE_PROPS_LINES[self.sprite_tile_props]

    def to_json(self) -> dict[str, Any]:
        return {
            "index": self.index,
            "object_id": self.object_id,
            "symbol": self.symbol,
            "routine_offset": _hex(self.rom_offset),
            "mappings_addr": None if self.mappings_addr is None else _hex(self.mappings_addr),
            "mappings": None if self.mappings is None else _hex(self.mappings),
            "art_ptr": None if self.art_ptr is None else f"0x{self.art_ptr:08X}",
            "art_tile": self.art_tile,
            "sprite_tile_props": (
                None if self.sprite_tile_props is None else f"0x{self.sprite_tile_props:02X}"
            ),
            "alternate_tile_props": [f"0x{v:02X}" for v in self.alternate_tile_props],
            "palette_line": self.palette_line,
            "mappings_duration": self.mappings_duration,
            "builds_sprites": self.builds_sprites,
            "starts_hidden": self.starts_hidden,
            "animates": self.animates,
            "streams_art": self.streams_art,
        }


def _check_jump_table(rom: bytes) -> None:
    for index in range(FIELD_OBJECT_COUNT):
        entry = FIELD_OBJECTS_JMP_TBL + index * 4
        if _u16(rom, entry) != _BRA_W:
            raise SpriteError(
                f"FieldObjectsJmpTbl entry {index} at {_hex(entry)} is not a bra.w; "
                "the table is not where this build thinks it is"
            )
    after = FIELD_OBJECTS_JMP_TBL + FIELD_OBJECT_COUNT * 4
    if _u16(rom, after) != _BRA_W:
        raise SpriteError(
            f"FieldObjectsJmpTbl is longer than {FIELD_OBJECT_COUNT} entries"
        )
    if _u16(rom, FIELD_OBJ_ANIMATE) != _BSET_TO_A4:
        raise SpriteError("FieldObj_Animate does not start with bset #5, render_flags(a4)")
    got = rom[FIELD_OBJ_ANIMATE:FIELD_OBJ_ANIMATE + len(FIELD_OBJ_ANIMATE_SIGNATURE)]
    if got != FIELD_OBJ_ANIMATE_SIGNATURE:
        raise SpriteError(
            f"FieldObj_Animate at {_hex(FIELD_OBJ_ANIMATE)} does not match its signature"
        )


def _routine_targets(rom: bytes) -> list[int]:
    targets = []
    for index in range(FIELD_OBJECT_COUNT):
        entry = FIELD_OBJECTS_JMP_TBL + index * 4
        targets.append(entry + 2 + _s16(rom, entry + 2))
    return targets


def _init_bounds(rom: bytes, routine: int) -> tuple[int, int] | None:
    """`bset #7,(a4) / bne.s <main>` -- the block between them runs once."""
    if _u16(rom, routine) != _BSET_A4_IND or _u16(rom, routine + 2) != 0x0007:
        return None
    if _u8(rom, routine + 4) != 0x66:  # bne.s
        return None
    displacement = _s8(rom, routine + 5)
    if displacement <= 0:
        return None
    return routine + 6, routine + 6 + displacement


def _scan_init(rom: bytes, start: int, end: int) -> dict[tuple[str, int], list[int]]:
    """Stores into the object structure over one span of a routine.

    Immediates are the common form, but several routines clear a register with
    `moveq #0, d0` and store *that* into three or four fields at once, so a
    register whose value came from a `moveq` is followed. Any other write to a
    register makes it unknown again, which keeps the model honest without
    turning this into a 68000 interpreter.
    """
    found: dict[tuple[str, int], list[int]] = {}
    known: dict[int, int] = {}
    offset = start
    while offset < end - 2:
        word = _u16(rom, offset)
        if word == _MOVE_L_TO_A4 and offset + 8 <= end:
            found.setdefault(("l", _u16(rom, offset + 6)), []).append(_u32(rom, offset + 2))
            offset += 8
            continue
        if word == _MOVE_W_TO_A4 and offset + 6 <= end:
            found.setdefault(("w", _u16(rom, offset + 4)), []).append(_u16(rom, offset + 2))
            offset += 6
            continue
        if word == _MOVE_B_TO_A4 and offset + 6 <= end:
            found.setdefault(("b", _u16(rom, offset + 4)), []).append(_u8(rom, offset + 3))
            offset += 6
            continue
        if word in _BIT_TO_A4 and offset + 6 <= end:
            key = _BIT_TO_A4[word]
            found.setdefault((key, _u16(rom, offset + 4)), []).append(_u16(rom, offset + 2))
            offset += 6
            continue
        if word & 0xF100 == _MOVEQ:  # moveq #imm8, dN
            known[(word >> 9) & 7] = word & 0xFF
            offset += 2
            continue
        register_store = next(
            (size for size, opcode in _MOVE_D_TO_A4 if word & 0xFFF8 == opcode), None
        )
        if register_store is not None and offset + 4 <= end:
            value = known.get(word & 7)
            if value is not None:
                found.setdefault((register_store, _u16(rom, offset + 2)), []).append(value)
            offset += 4
            continue
        # `move.<size> <ea>, Dn` -- destination mode 0, register in bits 11-9.
        if word & 0xC000 == 0 and (word >> 12) & 3 and word & 0x01C0 == 0:
            known.pop((word >> 9) & 7, None)
        offset += 2
    return found


def _reaches_animate(rom: bytes, start: int, end: int) -> tuple[bool, bool]:
    """Does this routine's body branch to `FieldObj_Animate` / `_Animate2`?"""
    animate = animate2 = False
    offset = start
    while offset <= end - 4:
        word = _u16(rom, offset)
        target = None
        if word in (_BRA_W, _BSR_W):
            target = offset + 2 + _s16(rom, offset + 2)
        elif word in (_JMP_ABS_L, _JSR_ABS_L) and offset + 6 <= end:
            target = _u32(rom, offset + 2)
        if target == FIELD_OBJ_ANIMATE:
            animate = True
        elif target == FIELD_OBJ_ANIMATE2:
            animate2 = True
        offset += 2
    return animate, animate2


def scan_field_objects(rom: bytes) -> list[FieldObjectRoutine]:
    """Read every `FieldObjectsJmpTbl` routine's one-time initialisation.

    The routines are not laid out in table order, so a routine's body is taken
    to run to the next routine start in *address* order, which is what bounds
    the branch scan. That bound only has to be right enough to find the tail
    branch into `FieldObj_Animate`, and every routine ends with one.
    """
    _check_jump_table(rom)
    targets = _routine_targets(rom)
    starts = sorted(set(targets))

    routines = []
    for index, routine in enumerate(targets):
        symbol = FIELD_OBJECT_SYMBOLS[index]
        bounds = _init_bounds(rom, routine)
        position = starts.index(routine)
        body_end = starts[position + 1] if position + 1 < len(starts) else routine + 0x200
        animate, animate2 = _reaches_animate(rom, routine, body_end)
        if bounds is None:
            routines.append(
                FieldObjectRoutine(
                    index=index, symbol=symbol, rom_offset=routine,
                    init_start=routine, init_end=routine,
                    mappings_addr=None, mappings=None, art_ptr=None, art_tile=None,
                    sprite_tile_props=None, alternate_tile_props=(),
                    mappings_duration=None, builds_sprites=False, starts_hidden=False,
                    animates=animate or animate2, streams_art=animate,
                )
            )
            continue
        start, end = bounds
        stores = _scan_init(rom, start, end)
        body = _scan_init(rom, end, body_end)

        def one(key: tuple[str, int]) -> int | None:
            values = stores.get(key)
            if not values:
                return None
            return values[-1]

        # `FieldObj_Elevator` and two others store their tile properties in the
        # per-frame part of the routine rather than the init block, because the
        # value depends on a flag. The first store there is the unconditional
        # one; the rest go into `alternate_tile_props` so nothing pretends the
        # first is the only value the object ever draws with.
        props_values = stores.get(("b", OFF_SPRITE_TILE_PROPS)) or body.get(
            ("b", OFF_SPRITE_TILE_PROPS)
        ) or []
        props = props_values[0] if props_values else None
        for value in props_values:
            if value not in SPRITE_TILE_PROPS_LINES:
                raise SpriteError(
                    f"{symbol} stores sprite tile properties 0x{value:02X}, which is not "
                    "one of the four CRAM-line values retail uses"
                )
        # `bset #1, render_flags` means "do not build sprites". Several objects
        # set it at load and clear it once they should appear (the Zio Fort
        # barrier beams switch on and off as the player walks past), so only an
        # object whose body never clears it is genuinely art-less.
        hidden = RENDER_FLAG_NO_SPRITES in stores.get(("bset", OFF_RENDER_FLAGS), [])
        unhides = any(
            RENDER_FLAG_NO_SPRITES in body.get((key, OFF_RENDER_FLAGS), [])
            for key in ("bclr", "bchg")
        )
        routines.append(
            FieldObjectRoutine(
                index=index,
                symbol=symbol,
                rom_offset=routine,
                init_start=start,
                init_end=end,
                mappings_addr=one(("l", OFF_MAPPINGS_ADDR)),
                mappings=one(("l", OFF_MAPPINGS)),
                art_ptr=one(("l", OFF_ART_PTR)),
                art_tile=one(("w", OFF_ART_TILE)),
                sprite_tile_props=props,
                alternate_tile_props=tuple(sorted(set(props_values[1:]) - {props})),
                mappings_duration=one(("b", OFF_MAPPINGS_DURATION)),
                builds_sprites=not hidden or unhides,
                starts_hidden=hidden,
                animates=animate or animate2,
                streams_art=animate,
            )
        )
    return routines


def facing_table_extents(routines: Sequence[FieldObjectRoutine]) -> dict[int, int]:
    """How many facings each `mappings_addr` table owns.

    The tables are packed back to back with no count anywhere, and plenty of
    objects only ever face one way, so a table can be one long. The extent of
    each is therefore the distance to the next table any routine references,
    capped at the four facings a `facing_dir` word can select.
    """
    addresses = sorted({r.mappings_addr for r in routines if r.mappings_addr is not None})
    extents = {}
    for position, address in enumerate(addresses):
        following = addresses[position + 1] if position + 1 < len(addresses) else address + 16
        extents[address] = max(1, min(len(FACINGS), (following - address) // 4))
    return extents
