"""Map effects: the cartridge's flag-gated patches to a loaded map.

When a map loads, the last section of its record is a `$FFFF`-terminated list
of indexes into `MapDataManagerJmpTbl`. `MapDataManager` (`0x051B38`) walks it
and calls each routine, and those routines are what make a map react to the
story: Igglanova is gone after you beat it, Zema's doors open, Alys is not
standing in the academy once you have found her.

Three facts about the mechanism decide the shape of everything below, and all
three are read from the cartridge rather than assumed.

**They run at map load and only at map load.** `MapDataManager` is reached from
exactly two `jsr` sites in the image, both inside a map loader, and
`Map_Data_Manager_Addr` is written at both and read nowhere. A flag that
changes while the player is standing on a map changes nothing until the map is
loaded again. A runtime that re-applies these on a flag change is not
reproducing the cartridge.

**A routine can abort the rest of the list.** The dispatcher is
`jsr (a1,d0.w)` followed by `bne.s` to `GoPast_FFFF_Terminator`, so a routine
that returns non-zero skips every remaining entry for that map. Every retail
routine ends `moveq #0,d7 / rts`, so it never fires -- but the emitted data
carries the bit per path, because a consumer that ignores it would apply
patches the cartridge would have skipped.

**Object patches address record objects, not RAM.** `LoadMapObjects` fills
slots through `Field_LoadObject`, a linear first-free scan from
`Field_Obj_Secondary` (`$FFFFC300`) in `$40` steps, so on a fresh load the
record's object N is at `$C300 + N * $40`. Every object write in the game --
106 of them -- lands on an aligned slot inside its own map's object count, so
this module emits the record index and never a RAM address.

Decoding
--------

The routines are small, straight-line-with-forward-branches 68000. This module
walks them with a reader for exactly the instruction forms they use, forking at
each conditional branch so that a write is emitted together with the flag
conditions that reach it. That is why the output is a list of *paths*: a
routine like `MapDataMan_VahFortMovPlatforms` writes the same chunk ids at one
of two rows depending on a temporary flag, and a model that flattened it would
have to pick one.

Anything outside the reader's vocabulary aborts that entry, which is then
reported with its offset and bytes rather than half-decoded. Slice 1 covers the
kinds that change what the player can see, walk through and talk to:

    object_despawn    clear an object slot -- the object never spawns
    object_rewrite    write a different object id into a slot
    object_dialogue   change an object's dialogue id
    layout_write      write chunk ids into `Map_Layout` through
                      `GetMapLayoutOffset` (doors, bridges, blocked paths)
    layout_replace    decompress an entirely different layout over a plane

Palette, scroll, chunk-table and temporary-flag-only routines are later slices
and are reported as `deferred` rather than silently dropped.
"""

from __future__ import annotations

import re
import struct
from dataclasses import dataclass, field
from typing import Any

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

MAP_LAYOUT = {0xA000: "fg", 0xB000: "bg"}

#: Kosinski, for `layout_replace`.
KOS_DECOMP = None  # resolved from the cartridge; see `_kos_decomp`

SLICE_ONE_KINDS = (
    "object_despawn", "object_rewrite", "object_dialogue",
    "layout_write", "layout_replace",
)


class MapEffectsError(ValueError):
    pass


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


# ---------------------------------------------------------------------------
# What a routine does
# ---------------------------------------------------------------------------
@dataclass(frozen=True)
class Gate:
    """One flag test a path passed through."""

    bank: str
    flag: int
    required: bool  # True = the flag must be set for this path

    def to_json(self) -> dict[str, Any]:
        return {
            "bank": self.bank,
            "flag": self.flag,
            "flag_hex": f"0x{self.flag:02X}",
            "symbol": _flag_symbol(self.bank, self.flag),
            "required": "set" if self.required else "clear",
        }


@dataclass(frozen=True)
class Write:
    """One effect, in the coordinates a runtime speaks."""

    kind: str
    at: int
    detail: dict[str, Any]

    def to_json(self) -> dict[str, Any]:
        return {"kind": self.kind, "at": f"0x{self.at:06X}", **self.detail}


@dataclass
class Path:
    gates: tuple[Gate, ...] = ()
    writes: list[Write] = field(default_factory=list)
    aborts: bool = False
    #: Effects this slice recognises but does not decode, so that a routine
    #: mixing Slice 1 with later work says so instead of looking complete.
    deferred: list[str] = field(default_factory=list)


class _Decoder:
    """A reader for the instruction forms these routines are built from.

    Not a 68000 emulator and must not become one. It knows three classes of
    instruction and treats them differently, which is what lets it decode a
    routine that mixes a Slice-1 effect with a later slice's work:

    * forms it models -- immediates, `lea`, stores through an address register,
      conditional branches, the flag tests and `GetMapLayoutOffset`;
    * forms it recognises but does not model -- calls to other routines, bulk
      copies, `trap #1`. These have known lengths, so stepping over one cannot
      lose alignment; each is recorded as a `deferred` note on the path so the
      output says out loud that the routine does more than Slice 1 covers;
    * everything else, which stops the entry. An unknown opcode has an unknown
      length, and guessing it would silently mis-read every byte after it.
    """

    #: A step budget rather than an address range, because a decoded loop
    #: re-executes the same addresses. Retail's longest routine unrolls in far
    #: fewer than this.
    STEPS = 4000
    MAX_PATHS = 64

    #: `move.b`/`move.w #imm,<ea>` and `clr.w <ea>` for a0..a6, as
    #: `opcode -> (register, has displacement, size)`.
    STORES: dict[int, tuple[str, bool, str | None]] = {}
    for _reg in range(7):
        STORES[0x4250 | _reg] = (f"a{_reg}", False, None)          # clr.w (aN)
        STORES[0x4268 | _reg] = (f"a{_reg}", True, None)           # clr.w d16(aN)
        STORES[0x10BC | (_reg << 9)] = (f"a{_reg}", False, "b")    # move.b #imm,(aN)
        STORES[0x117C | (_reg << 9)] = (f"a{_reg}", True, "b")     # move.b #imm,d16(aN)
        STORES[0x30BC | (_reg << 9)] = (f"a{_reg}", False, "w")    # move.w #imm,(aN)
        STORES[0x317C | (_reg << 9)] = (f"a{_reg}", True, "w")     # move.w #imm,d16(aN)
    del _reg

    #: The same stores with a data register as the source instead of an
    #: immediate, as `opcode -> (address register, has displacement, source)`.
    REG_STORES: dict[int, tuple[str, bool, str]] = {}
    for _a in range(7):
        for _d in range(8):
            REG_STORES[0x1080 | (_a << 9) | _d] = (f"a{_a}", False, f"d{_d}")
            REG_STORES[0x1140 | (_a << 9) | _d] = (f"a{_a}", True, f"d{_d}")
            REG_STORES[0x3080 | (_a << 9) | _d] = (f"a{_a}", False, f"d{_d}")
            REG_STORES[0x3140 | (_a << 9) | _d] = (f"a{_a}", True, f"d{_d}")
    del _a, _d

    #: Recognised but not modelled. `opcode -> (length, note)`. Every one has a
    #: known length, which is what makes stepping over it safe.
    DEFERRED_OPS: dict[int, tuple[int, str]] = {
        0x4E41: (2, "trap #1 bulk copy"),
        0x4E40: (2, "trap #0 bulk clear"),
        0x08A8: (6, "bclr on a memory bit"),
        0x08B8: (6, "bclr on a memory bit"),
    }
    #: `move.<size> (aN)+,(aM)+`, the bodies of the palette and art copy loops.
    for _size in (0x1000, 0x2000, 0x3000):
        for _dst in range(7):
            for _src in range(7):
                DEFERRED_OPS[_size | (_dst << 9) | 0xD8 | _src] = (2, "postincrement bulk copy")
    del _size, _dst, _src

    def __init__(self, rom: bytes, start: int, tests: dict[int, str],
                 kos: int, clears: dict[int, str]):
        self.rom = rom
        self.start = start
        self.tests = tests
        self.clears = clears
        self.kos = kos
        self.paths: list[Path] = []

    def w(self, at): return int.from_bytes(self.rom[at:at + 2], "big")
    def sw(self, at): return int.from_bytes(self.rom[at:at + 2], "big", signed=True)
    def l(self, at): return int.from_bytes(self.rom[at:at + 4], "big")

    def fail(self, at, why):
        raise MapEffectsError(
            f"+0x{at - self.start:X}: {why} (bytes {self.rom[at:at + 6].hex()})"
        )

    def run(self) -> list[Path]:
        self._walk(self.start, {}, (), [], [])
        return self.paths

    def _walk(self, pc, regs, gates, writes, deferred, steps=0):
        if len(self.paths) >= self.MAX_PATHS:
            raise MapEffectsError(f"more than {self.MAX_PATHS} paths")
        regs, writes, deferred = dict(regs), list(writes), list(deferred)
        while True:
            steps += 1
            if steps > self.STEPS:
                self.fail(pc, f"no rts within {self.STEPS} instructions")
            op = self.w(pc)

            if op == 0x4E75:  # rts
                self.paths.append(Path(gates, writes, bool(regs.get("d7")), deferred))
                return

            # `dbf dN,disp`. With the counter known this runs the loop for real,
            # which is how the table-driven door routines unroll into concrete
            # writes; with it unknown, falling through is the loop's exit and
            # the body is recorded as deferred.
            if op & 0xFFF8 == 0x51C8:
                counter = f"d{op & 7}"
                value = regs.get(counter)
                if value is None:
                    if "loop" not in deferred:
                        deferred.append("loop")
                    pc += 4
                    continue
                value = (value - 1) & 0xFFFF
                regs[counter] = value
                pc = pc + 2 + self.sw(pc + 2) if value != 0xFFFF else pc + 4
                continue

            # addq/subq on a data register: the table walk's index step.
            if op & 0xF1C0 in (0x5040, 0x5140):
                reg = f"d{op & 7}"
                if regs.get(reg) is None:
                    self.fail(pc, f"addq/subq on {reg}, which is not tracked")
                amount = ((op >> 9) & 7) or 8
                sign = -1 if op & 0x0100 else 1
                regs[reg] = (regs[reg] + sign * amount) & 0xFFFF
                pc += 2
                continue

            if op in self.DEFERRED_OPS:
                size, note = self.DEFERRED_OPS[op]
                if note not in deferred:
                    deferred.append(note)
                pc += size
                continue

            # move.w #imm,dN / moveq #imm,dN
            if op & 0xF1FF == 0x303C:
                regs[f"d{(op >> 9) & 7}"] = self.w(pc + 2)
                pc += 4
                continue
            if op & 0xF100 == 0x7000:
                regs[f"d{(op >> 9) & 7}"] = op & 0xFF
                pc += 2
                continue

            # lea (abs).w / (abs).l / d16(aN) -> aM
            if op & 0xF1FF == 0x41F8:
                regs[f"a{(op >> 9) & 7}"] = self.w(pc + 2); pc += 4; continue
            if op & 0xF1FF == 0x41F9:
                regs[f"a{(op >> 9) & 7}"] = self.l(pc + 2); pc += 6; continue
            if op & 0xF1F8 == 0x41E8:
                source = regs.get(f"a{op & 7}")
                if not isinstance(source, int):
                    self.fail(pc, "lea from an address register that is not a plain address")
                regs[f"a{(op >> 9) & 7}"] = source + self.sw(pc + 2)
                pc += 4
                continue

            # calls
            if op == 0x4EB9:
                pc = self._call(pc, self.l(pc + 2), 6, regs, writes, deferred); continue
            if op == 0x6100:
                pc = self._call(pc, pc + 2 + self.sw(pc + 2), 4, regs, writes, deferred); continue
            if op & 0xFF00 == 0x6100:
                pc = self._call(pc, pc + 2 + (((op & 0xFF) ^ 0x80) - 0x80), 2,
                                regs, writes, deferred); continue

            # branches
            if op & 0xFF00 in (0x6700, 0x6600, 0x6000):
                kind = {0x6700: "beq", 0x6600: "bne", 0x6000: "bra"}[op & 0xFF00]
                if op & 0xFF:
                    target, after = pc + 2 + (((op & 0xFF) ^ 0x80) - 0x80), pc + 2
                else:
                    target, after = pc + 2 + self.sw(pc + 2), pc + 4
                if kind == "bra":
                    pc = target
                    continue
                gate = regs.pop("pending_gate", None)
                if gate is None:
                    self.fail(pc, f"{kind} with no flag test before it")
                bank, flag = gate
                # The test leaves Z set when the flag is CLEAR, so `beq` is the
                # clear arm and `bne` the set arm.
                self._walk(target, regs, gates + (Gate(bank, flag, kind == "bne"),),
                           writes, deferred)
                self._walk(after, regs, gates + (Gate(bank, flag, kind != "bne"),),
                           writes, deferred)
                return

            # `clr.w (xxx).w` and `move.b #imm,(xxx).w`: a store to an absolute
            # address rather than through a register.
            if op in (0x4278, 0x11FC, 0x31FC):
                address = self.w(pc + 2 if op == 0x4278 else pc + 4)
                value = None if op == 0x4278 else self.w(pc + 2)
                size = 4 if op == 0x4278 else (6 if op == 0x31FC else 6)
                self._absolute(pc, address, value, op == 0x4278, writes, deferred)
                pc += size
                continue

            # `move.b (aN,dM.w),dK` -- a read from a table in the cartridge.
            # With the base and index both known this is a constant, which is
            # what turns the door tables into concrete coordinates.
            if op & 0xF1F8 == 0x1030:
                dest = f"d{(op >> 9) & 7}"
                base = regs.get(f"a{op & 7}")
                extension = self.w(pc + 2)
                index_reg = f"d{(extension >> 12) & 7}"
                index = regs.get(index_reg)
                disp = (extension & 0xFF) - (0x100 if extension & 0x80 else 0)
                if (isinstance(base, int) and index is not None
                        and 0 < base + index + disp < len(self.rom)):
                    regs[dest] = self.rom[base + index + disp]
                else:
                    regs.pop(dest, None)
                pc += 4
                continue

            # Any other indexed or absolute load leaves a register holding
            # something this reader cannot know; forget it so a later store
            # using it fails loudly instead of inventing a value.
            if op & 0xF1F8 == 0x3030 or op & 0xF1FF in (0x1038, 0x3038):
                regs.pop(f"d{(op >> 9) & 7}", None)
                pc += 4
                continue

            size = self._store(pc, op, regs, writes, deferred)
            if size:
                pc += size
                continue
            self.fail(pc, f"opcode 0x{op:04X} is outside this decoder's vocabulary")

    def _call(self, pc, target, size, regs, writes, deferred):
        if target in self.tests:
            flag = regs.get("d0")
            if flag is None:
                self.fail(pc, "a flag test with no id in d0")
            regs["pending_gate"] = (self.tests[target], flag)
            return pc + size
        if target == GET_MAP_LAYOUT_OFFSET:
            for name in ("d1", "d2", "d3"):
                if regs.get(name) is None:
                    self.fail(pc, f"GetMapLayoutOffset with {name} unset")
            regs["a1"] = ("layout", "bg" if regs["d3"] else "fg", regs["d1"], regs["d2"])
            return pc + size
        if target == self.kos:
            source, plane = regs.get("a0"), regs.get("a1")
            if not isinstance(source, int) or plane not in MAP_LAYOUT:
                self.fail(pc, "KosDecomp with an unrecognised source or destination")
            writes.append(Write("layout_replace", pc, {
                "plane": MAP_LAYOUT[plane], "source": f"0x{source:06X}",
            }))
            return pc + size
        note = (f"{self.clears[target]} write" if target in self.clears
                else f"call to 0x{target:06X}")
        if note not in deferred:
            deferred.append(note)
        return pc + size

    def _absolute(self, pc, address, value, is_clear, writes, deferred) -> None:
        """A store straight to an absolute short address."""
        if FIELD_OBJ_SECONDARY <= address <= FIELD_OBJ_LAST + OBJECT_DIALOGUE_OFFSET:
            index, within = divmod(address - FIELD_OBJ_SECONDARY, FIELD_OBJ_STRIDE)
            if within == 0 and (is_clear or value == 0):
                writes.append(Write("object_despawn", pc, {"object_index": index}))
                return
        note = f"write to 0xFFFF{address:04X}"
        if note not in deferred:
            deferred.append(note)

    def _store(self, pc, op, regs, writes, deferred) -> int:
        if op in self.STORES:
            reg, has_disp, value_size = self.STORES[op]
            if value_size is None:
                value, after = None, pc + 2
            elif value_size == "w":
                value, after = self.w(pc + 2), pc + 4
            else:
                value, after = self.w(pc + 2) & 0xFF, pc + 4
        elif op in self.REG_STORES:
            reg, has_disp, source = self.REG_STORES[op]
            value_size = "w" if op & 0xF000 == 0x3000 else "b"
            value, after = regs.get(source), pc + 2
            if value is None:
                self.fail(pc, f"a store sources {source}, which is not tracked")
        else:
            return 0
        disp = self.sw(after) if has_disp else 0
        size = (after - pc) + (2 if has_disp else 0)
        base = regs.get(reg)
        if base is None:
            self.fail(pc, f"a store through {reg} before it is loaded")

        if isinstance(base, tuple):
            writes.append(Write("layout_write", pc, {
                "plane": base[1], "chunk_x": base[2], "chunk_y": base[3],
                "displacement": disp, "chunk_id": value,
                "note": "displacement is in layout bytes; resolve with the map's row size",
            }))
            return size
        address = base + disp
        if FIELD_OBJ_SECONDARY <= address <= FIELD_OBJ_LAST + OBJECT_DIALOGUE_OFFSET:
            index, within = divmod(address - FIELD_OBJ_SECONDARY, FIELD_OBJ_STRIDE)
            if within == 0 and value_size is None:
                writes.append(Write("object_despawn", pc, {"object_index": index}))
            elif within == 0 and value_size == "w" and value == 0:
                writes.append(Write("object_despawn", pc, {"object_index": index}))
            elif within == 0 and value_size == "w":
                writes.append(Write("object_rewrite", pc, {
                    "object_index": index, "object_id": value}))
            elif within == OBJECT_DIALOGUE_OFFSET and value_size == "b":
                writes.append(Write("object_dialogue", pc, {
                    "object_index": index, "dialogue_id": value}))
            else:
                note = f"write at +0x{within:X} of object slot {index}"
                if note not in deferred:
                    deferred.append(note)
            return size
        # Party slots, palettes, scroll values and the rest: recognised as a
        # write, not modelled by this slice.
        note = f"write to 0xFFFF{address:04X}"
        if note not in deferred:
            deferred.append(note)
        return size


# ---------------------------------------------------------------------------
# One jump-table entry
# ---------------------------------------------------------------------------
@dataclass
class Entry:
    index: int
    rom_offset: int
    paths: list[Path]
    undecoded: str | None
    #: The routine's bytes, for provenance. Bounded by the furthest byte any
    #: path read, so it is the routine as decoded rather than a fixed window.
    raw_hex: str = ""

    @property
    def kinds(self) -> tuple[str, ...]:
        return tuple(sorted({w.kind for p in self.paths for w in p.writes}))

    def to_json(self, widths: dict[str, int] | None = None) -> dict[str, Any]:
        """This entry as the pack emits it.

        `widths` is the map's layout width per plane. A `layout_write`'s
        displacement is in layout bytes and only becomes a coordinate once the
        row size is known, so an entry serialised without them leaves those
        writes unresolved -- which is why the pack always passes them.
        """
        out: dict[str, Any] = {
            "entry": self.index,
            "entry_hex": f"0x{self.index:02X}",
            "routine": f"0x{self.rom_offset:06X}",
            "raw_hex": self.raw_hex,
        }
        if self.undecoded is not None:
            out["decoded"] = False
            out["reason"] = self.undecoded
            return out
        out["decoded"] = True
        out["kinds"] = list(self.kinds)
        out["paths"] = [
            {
                "gates": [g.to_json() for g in path.gates],
                "unconditional": not path.gates,
                "aborts_remaining_entries": path.aborts,
                "deferred": path.deferred,
                "writes": [
                    resolve_layout_write(w, widths)
                    if w.kind == "layout_write" and widths else w.to_json()
                    for w in path.writes
                ],
            }
            for path in self.paths
            # A path that does nothing and gates nothing is the routine's own
            # "flag not set, return" arm; it carries no information.
            if path.writes or path.aborts
        ]
        return out


def decode_entry(rom: bytes, index: int, *, table: int | None = None,
                 tests: dict[int, str] | None = None, kos: int | None = None,
                 clears: dict[int, str] | None = None) -> Entry:
    table = dispatch_table(rom) if table is None else table
    tests = flag_tests(rom) if tests is None else tests
    clears = flag_clears(rom) if clears is None else clears
    kos = _kos_decomp(rom) if kos is None else kos
    address = routine_address(rom, table, index)
    decoder = _Decoder(rom, address, tests, kos, clears)
    try:
        paths = decoder.run()
    except MapEffectsError as exc:
        return Entry(index, address, [], str(exc),
                     rom[address:address + 0x40].hex())
    end = max((w.at for p in paths for w in p.writes), default=address) + 8
    return Entry(index, address, paths, None, rom[address:end].hex())


# ---------------------------------------------------------------------------
# Per map
# ---------------------------------------------------------------------------
#: `psiv_tools.symbols` has no event-flag table beyond the eight the overworld
#: hooks test, so these are resolved where a symbol exists and left null where
#: the disassembly names none. The id is always emitted.
def _flag_symbol(bank: str, flag: int) -> str | None:
    from .symbols import EVENT_FLAG_SYMBOLS

    return EVENT_FLAG_SYMBOLS.get(flag) if bank == "event_flags" else None


def resolve_layout_write(write: Write, widths: dict[str, int]) -> dict[str, Any]:
    """A layout write with its displacement folded into the coordinate.

    `GetMapLayoutOffset` multiplies the row by the *map's* row size, so a
    displacement off the returned pointer only becomes a coordinate once the
    map is known. Two maps sharing a routine resolve it differently, which is
    why this is done at emission rather than at decode.
    """
    detail = dict(write.detail)
    # `GetMapLayoutOffset` takes the row size of the plane it was asked for.
    width = widths[detail["plane"]]
    offset = detail["chunk_y"] * width + detail["chunk_x"] + detail["displacement"]
    chunk_y, chunk_x = divmod(offset, width)
    return {
        "kind": write.kind,
        "at": f"0x{write.at:06X}",
        "plane": detail["plane"],
        "chunk_x": chunk_x,
        "chunk_y": chunk_y,
        "cell_x": chunk_x * 2,
        "cell_y": chunk_y * 2,
        "chunk_id": detail["chunk_id"],
    }


def _widths(record: dict[str, Any]) -> dict[str, int]:
    """The map's layout width per plane, in chunks. The record stores size-1."""
    dimensions = record["dimensions"]
    return {
        "fg": dimensions["fg_row_size"] + 1,
        "bg": dimensions["bg_row_size"] + 1,
    }


def extract_map_effects(rom: bytes, maps: list[dict[str, Any]]) -> dict[str, Any]:
    """Every map's `MapDataManager` list, decoded, with the census.

    `maps` is `psiv_tools.maps.extract_maps(rom)["maps"]`. The per-map lists are
    keyed by map id so `psiv_tools.pack` can drop each one into that map's own
    record: the data arrives with the map it patches, and a kind a consumer does
    not know fails that map's build rather than the whole pack.
    """
    ctx = dict(table=dispatch_table(rom), tests=flag_tests(rom),
               kos=_kos_decomp(rom), clears=flag_clears(rom))
    real = [record for record in maps if not record["is_null"]]
    referenced = sorted({i for r in real for i in r["map_data_manager"]["ids"]})
    entries = {index: decode_entry(rom, index, **ctx) for index in referenced}

    per_map: dict[int, list[dict[str, Any]]] = {}
    past_end: list[dict[str, Any]] = []
    kinds: dict[str, int] = {}
    banks: dict[str, int] = {}
    flags: dict[str, set[int]] = {}
    maps_per_kind: dict[str, set[int]] = {}
    undecoded: list[dict[str, Any]] = []
    unconditional = gated = 0
    replacements: list[dict[str, Any]] = []

    for record in real:
        ids = record["map_data_manager"]["ids"]
        if not ids:
            continue
        widths = _widths(record)
        emitted = [entries[index].to_json(widths) for index in ids]
        per_map[record["id"]] = emitted
        for entry in emitted:
            if not entry["decoded"]:
                continue
            for path in entry["paths"]:
                if path["gates"]:
                    gated += 1
                else:
                    unconditional += 1
                for gate in path["gates"]:
                    banks[gate["bank"]] = banks.get(gate["bank"], 0) + 1
                    flags.setdefault(gate["bank"], set()).add(gate["flag"])
                for write in path["writes"]:
                    kinds[write["kind"]] = kinds.get(write["kind"], 0) + 1
                    maps_per_kind.setdefault(write["kind"], set()).add(record["id"])
                    index = write.get("object_index")
                    if index is not None and index >= record["objects"]["count"]:
                        past_end.append({
                            "map": record["id"], "entry": entry["entry_hex"],
                            "at": write["at"], "object_index": index,
                            "map_objects": record["objects"]["count"],
                        })
                    if write["kind"] == "layout_replace":
                        replacements.append({
                            "map": record["id"], "plane": write["plane"],
                            "source": write["source"],
                        })

    for index in referenced:
        entry = entries[index]
        if entry.undecoded is not None:
            undecoded.append({
                "entry": index,
                "entry_hex": f"0x{index:02X}",
                "routine": f"0x{entry.rom_offset:06X}",
                "reason": entry.undecoded,
                "maps": sorted(r["id"] for r in real
                               if index in r["map_data_manager"]["ids"]),
            })

    return {
        "per_map": per_map,
        "replacements": replacements,
        "census": {
            "jump_table": f"0x{ctx['table']:06X}",
            "table_entries": MAP_DATA_MANAGER_ROUTINES,
            "referenced_entries": len(referenced),
            "unreferenced_entries": MAP_DATA_MANAGER_ROUTINES - len(referenced),
            "maps_with_entries": len(per_map),
            "map_entry_pairs": sum(len(v) for v in per_map.values()),
            "decoded_entries": len(referenced) - len(undecoded),
            "undecoded_entries": undecoded,
            "kinds": {k: kinds[k] for k in sorted(kinds)},
            "maps_per_kind": {k: len(maps_per_kind[k]) for k in sorted(maps_per_kind)},
            "gate_banks": {k: banks[k] for k in sorted(banks)},
            "flags_per_bank": {k: sorted(flags[k]) for k in sorted(flags)},
            # Some routines clear more slots than the map has objects. On a
            # fresh load those slots are empty, so the write is a no-op -- the
            # routine is written for the largest map that uses it. A consumer
            # should ignore an index past the map's object count rather than
            # treat it as a defect.
            "object_writes_past_map_object_count": past_end,
            "paths_gated": gated,
            "paths_unconditional": unconditional,
            "evaluated": "map load only",
            "note": (
                "MapDataManager is reached from two jsr sites, both in a map "
                "loader, and Map_Data_Manager_Addr is written at both and read "
                "nowhere: these apply when a map is built and are never "
                "re-evaluated while it is loaded. A routine returning non-zero "
                "aborts the rest of that map's list, which is what "
                "aborts_remaining_entries carries; no retail routine does."
            ),
        },
    }
