"""Where the mechanism's routines, flag banks and buffers are in the image.

Every address here is read out of the retail image rather than transcribed
from the disassembly's prose, because three of them are things a clone gets
wrong: the jump table names itself from its own dispatcher, the flag blocks are
read door by door, and `KosDecomp` is identified by the call every
layout-replace routine makes. Each derivation raises `MapEffectsError` when the
image disagrees with the shape the mechanism assumes, so a disagreement fails
loudly instead of decoding as something else.

The two layout buffers live in `psiv_tools.map_effects.geometry`, which is what
resolves a write against them; this module reads them back from
`GetMapLayoutOffset` and checks the distance between them that a cross-plane
displacement assumes.
"""

from __future__ import annotations

import re

from .geometry import MAP_LAYOUT, MAP_LAYOUT_BYTES
from .model import MapEffectsError

#: `MapDataManager`'s dispatcher, which names its own jump table.
_DISPATCH = re.compile(
    rb"\x30\x10\x0c\x40\xff\xff\x67.\x43\xf9(....)\xd0\x40\xd0\x40\x2f\x08\x4e\xb1\x00\x00",
    re.S,
)

#: `records.py`'s bound: the record walker refuses an index past this.
MAP_DATA_MANAGER_ROUTINES = 0xA0

#: The four flag banks share one test routine with four doors, `movem.l` then
#: the `lea` that picks the bank. Same shape as the setter block.
FLAG_TEST_BLOCK = 0x057624
FLAG_SET_BLOCK = 0x057666
FLAG_CLEAR_BLOCK = 0x0576A8
FLAG_TEST_STRIDE = 0x0A
#: The clear block is one door short: nothing clears a town flag.
FLAG_CLEAR_DOORS = 3
#: The banks by the address their door's `lea` names, using the constants
#: file's own names for those addresses.
#:
#: There are FOUR doors and no more. The disassembly also names a
#: `TempEveFlags_Test` reading `Temp_Event_Flags = $FFFFF156`, and no routine
#: anywhere in the retail image loads `$FFFFF156` -- the routine the clone
#: labels that way is the `$FFFFF140` door, the same one it elsewhere calls
#: `ChestFlags_Test`. `$F156` is 22 bytes into that bank, so what the
#: disassembly's prose calls a temporary event flag is a bit of the `$F140`
#: bank at a high id, not a bank of its own. A consumer modelling five banks
#: will not line up with the cartridge.
FLAG_BANKS = {
    0xF100: "event_flags",
    0xF120: "extended_event_flags",
    0xF140: "chest_flags",
    0xF160: "town_flags",
}
#: The inverse, so a write can carry the bank's address beside its name. The
#: `$F140` name is under review -- core-lane's map-load scout finds that bank
#: carries temp event flags, not chest flags -- and an address does not move
#: when a name does.
FLAG_BANK_ADDRESSES = {name: address for address, name in FLAG_BANKS.items()}

#: `tst.w d3 / ... / adda.w d1,a1`: a1 = plane + (row_size + 1) * d2 + d1, so
#: d1 is a chunk column, d2 a chunk row and d3 picks the plane. The row size is
#: the *map's*, so a displacement off that pointer can only be resolved per map.
GET_MAP_LAYOUT_OFFSET = 0x053514

#: `Field_LoadObject`'s first slot and its stride.
FIELD_OBJ_SECONDARY = 0xC300
FIELD_OBJ_STRIDE = 0x40
FIELD_OBJ_LAST = 0xCFC0
#: `LoadMapObjects` writes the dialogue id to `$14(a4)`.
OBJECT_DIALOGUE_OFFSET = 0x14

#: Kosinski, for `layout_replace`.
KOS_DECOMP = None  # resolved from the cartridge; see `_kos_decomp`


# ---------------------------------------------------------------------------
# Anchors, all derived from the retail image
# ---------------------------------------------------------------------------
def dispatch_table(rom: bytes) -> int:
    """`MapDataManagerJmpTbl`, from the dispatcher that names it."""
    matches = list(_DISPATCH.finditer(rom))
    if len(matches) != 1:
        raise MapEffectsError(
            f"the MapDataManager dispatcher matches {len(matches)} times, not once"
        )
    return int.from_bytes(matches[0].group(1), "big")


def routine_address(rom: bytes, table: int, index: int) -> int:
    """One `bra.w` entry, resolved."""
    if not 0 <= index < MAP_DATA_MANAGER_ROUTINES:
        raise MapEffectsError(f"entry {index} is outside the jump table")
    entry = table + index * 4
    if int.from_bytes(rom[entry:entry + 2], "big") != 0x6000:
        raise MapEffectsError(f"jump table entry {index} is not a `bra.w`")
    return entry + 2 + int.from_bytes(rom[entry + 2:entry + 4], "big", signed=True)


def flag_tests(rom: bytes) -> dict[int, str]:
    """The four flag-test entry points, by address, with the bank each reads."""
    return _flag_block(rom, FLAG_TEST_BLOCK, "test", len(FLAG_BANKS))


def _flag_block(rom: bytes, base: int, what: str, expected: int) -> dict[int, str]:
    """One flag block: `movem.l` then the `lea` that picks the bank, per door.

    The blocks are not all the same width. Test and set have a door for each of
    the four banks; clear has three, because nothing in the cartridge clears a
    town flag. The count is read rather than assumed and then checked, so a
    block that changes shape fails here instead of decoding as something else.
    """
    out: dict[int, str] = {}
    for i in range(len(FLAG_BANKS)):
        address = base + i * FLAG_TEST_STRIDE
        if rom[address:address + 4] != b"\x48\xe7\xe0\x80":
            break
        if rom[address + 4:address + 6] != b"\x41\xf8":
            break
        bank = int.from_bytes(rom[address + 6:address + 8], "big")
        if bank not in FLAG_BANKS:
            break
        out[address] = FLAG_BANKS[bank]
    if len(out) != expected:
        raise MapEffectsError(
            f"the flag {what} block at 0x{base:06X} has {len(out)} doors, not {expected}"
        )
    return out


def flag_clears(rom: bytes) -> dict[int, str]:
    """The four flag-clearing entry points. Slice 3 work, recognised here so a
    routine that clears a flag is reported rather than refused."""
    return _flag_block(rom, FLAG_CLEAR_BLOCK, "clear", FLAG_CLEAR_DOORS)


def _kos_decomp(rom: bytes) -> int:
    """`KosDecomp`, located by the call the layout-replace routines make.

    Every `layout_replace` routine is `lea blob,a0 / lea plane,a1 / jsr Kos`,
    so the address falls out of the one instruction they all share.
    """
    pattern = re.compile(rb"\x41\xf9....\x43\xf8[\xa0\xb0]\x00\x4e\xb9(....)", re.S)
    counts: dict[int, int] = {}
    for match in pattern.finditer(rom):
        target = int.from_bytes(match.group(1), "big")
        counts[target] = counts.get(target, 0) + 1
    # One site elsewhere in the image happens to load $A000 into a1 for its own
    # reasons; the layout-replace routines are the several that agree.
    agreed = [target for target, n in counts.items() if n > 1]
    if len(agreed) != 1:
        raise MapEffectsError(
            f"{len(agreed)} routines are called by more than one layout-replace "
            "site; KosDecomp cannot be identified"
        )
    return agreed[0]


def map_layout_bases(rom: bytes) -> dict[str, int]:
    """The two layout buffer addresses, read out of `GetMapLayoutOffset`.

    The routine picks a plane with `d3` and loads its buffer with
    `lea (Map_Layout_FG).w, a1` / `lea (Map_Layout_BG).w, a1`. Both `lea`s are
    read back here rather than trusted, because the distance between them is
    what makes a cross-plane displacement resolvable at all.
    """
    bases: dict[str, int] = {}
    for probe in range(GET_MAP_LAYOUT_OFFSET, GET_MAP_LAYOUT_OFFSET + 0x20, 2):
        if int.from_bytes(rom[probe:probe + 2], "big") != 0x43F8:  # lea (xxx).w, a1
            continue
        address = int.from_bytes(rom[probe + 2:probe + 4], "big")
        if address not in MAP_LAYOUT:
            raise MapEffectsError(
                f"0x{probe:06X}: GetMapLayoutOffset loads an unknown layout "
                f"buffer 0xFFFF{address:04X}"
            )
        bases[MAP_LAYOUT[address]] = address
    if sorted(bases) != ["bg", "fg"]:
        raise MapEffectsError(
            f"GetMapLayoutOffset names {sorted(bases)}, not both layout buffers"
        )
    if abs(bases["bg"] - bases["fg"]) != MAP_LAYOUT_BYTES:
        raise MapEffectsError(
            f"the layout buffers are 0x{abs(bases['bg'] - bases['fg']):X} apart, "
            f"not the 0x{MAP_LAYOUT_BYTES:X} a cross-plane displacement assumes"
        )
    return bases
