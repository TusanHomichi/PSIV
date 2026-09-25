"""The instruction reader: one routine walked into the paths it can take.

`_Decoder` is not a 68000 emulator and must not become one. It knows three
classes of instruction and treats them differently: the forms it models
(immediates, `lea`, stores through an address register, conditional branches,
the flag tests and `GetMapLayoutOffset`); the forms it recognises but does not
model (calls, bulk copies, `trap #1`), which have known lengths so stepping
over one cannot lose alignment and which are recorded as `deferred`; and
everything else, which stops the entry, because an unknown opcode has an
unknown length and guessing it would silently mis-read every byte after it.

It forks at each conditional branch, so a write comes out together with the
gates that reach it, and it runs `dbf` loops for real when the counter is a
known constant -- which is how the table-driven door routines unroll into
concrete coordinates.
"""

from __future__ import annotations

from .anchors import (
    FIELD_OBJ_LAST,
    FIELD_OBJ_SECONDARY,
    FIELD_OBJ_STRIDE,
    FLAG_BANK_ADDRESSES,
    GET_MAP_LAYOUT_OFFSET,
    OBJECT_DIALOGUE_OFFSET,
)
from .geometry import MAP_LAYOUT
from .model import Gate, MapEffectsError, Path, Write

#: Conditional branches that follow an arithmetic compare rather than a flag
#: test. The routines that use them compare the player's position, which this
#: slice does not model, so both arms are walked and the compare is recorded as
#: `deferred`. `beq`/`bne` are excluded: those are the flag tests.
COMPARE_BRANCHES = {
    0x6200: "bhi", 0x6300: "bls", 0x6400: "bcc", 0x6500: "bcs",
    0x6A00: "bpl", 0x6B00: "bmi", 0x6C00: "bge", 0x6D00: "blt",
    0x6E00: "bgt", 0x6F00: "ble",
}


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
        self._walk(self.start, {}, (), [], [], [])
        return self.paths

    def _walk(self, pc, regs, gates, writes, deferred, conditions, steps=0):
        if len(self.paths) >= self.MAX_PATHS:
            raise MapEffectsError(f"more than {self.MAX_PATHS} paths")
        regs, writes, deferred = dict(regs), list(writes), list(deferred)
        conditions = list(conditions)
        while True:
            steps += 1
            if steps > self.STEPS:
                self.fail(pc, f"no rts within {self.STEPS} instructions")
            op = self.w(pc)

            if op == 0x4E75:  # rts
                self.paths.append(Path(
                    gates=gates, writes=writes, aborts=bool(regs.get("d7")),
                    deferred=deferred, conditions=conditions,
                ))
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

            # move.w #imm,dN / move.b #imm,dN / moveq #imm,dN
            if op & 0xF1FF == 0x303C:
                regs[f"d{(op >> 9) & 7}"] = self.w(pc + 2)
                pc += 4
                continue
            # The byte form, which the flag-clear routines use for ids that fit
            # in one: `move.b #$0B, d0 / jsr (clear door)`.
            if op & 0xF1FF == 0x103C:
                regs[f"d{(op >> 9) & 7}"] = self.w(pc + 2) & 0xFF
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

            # `cmpi.w #imm, d16(aN)`. Two routines compare the player object's
            # position before deciding whether to put a flag back; the compare
            # itself is not a flag test, so it is recorded as a condition this
            # slice does not model and both arms are walked.
            if op & 0xFFF8 == 0x0C68:
                note = (
                    f"position compare at 0x{pc:06X}: "
                    f"#{self.w(pc + 2):#06x} against ${self.w(pc + 4):02X}"
                    f"(a{op & 7})"
                )
                if note not in conditions:
                    conditions.append(note)
                regs["pending_unmodelled"] = note
                pc += 6
                continue

            # A branch on that compare. There is no flag condition to record,
            # so both arms are walked ungated and the note above is what says
            # the path is conditional on something unmodelled.
            if op & 0xFF00 in COMPARE_BRANCHES:
                if regs.pop("pending_unmodelled", None) is None:
                    self.fail(
                        pc,
                        f"a {COMPARE_BRANCHES[op & 0xFF00]} with no compare before it",
                    )
                if op & 0xFF:
                    target, after = pc + 2 + (((op & 0xFF) ^ 0x80) - 0x80), pc + 2
                else:
                    target, after = pc + 2 + self.sw(pc + 2), pc + 4
                for destination in (target, after):
                    self._walk(destination, regs, gates, writes, deferred,
                               conditions)
                return

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
                for destination, required in (
                    (target, kind == "bne"), (after, kind != "bne")
                ):
                    arm = Gate(bank, flag, required)
                    # A routine with a backward branch re-tests a flag it has
                    # already branched on, and one of the two arms then
                    # contradicts a gate this path already carries. That arm is
                    # unreachable in a single map load -- a flag does not change
                    # between two tests -- so it is dropped rather than walked,
                    # which is what stops the same writes being emitted twice
                    # under impossible conditions.
                    if any(g.bank == arm.bank and g.flag == arm.flag
                           and g.required != arm.required for g in gates):
                        continue
                    # Re-testing a flag the same way adds no condition.
                    extended = gates if arm in gates else gates + (arm,)
                    self._walk(destination, regs, extended, writes, deferred,
                               conditions)
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
        if target in self.clears:
            # A map-load flag clear. The bank comes from which door was called,
            # exactly as the test and set blocks are bound. The address is
            # emitted beside the name because the name of the `$F140` bank is
            # under review -- a consumer keying on `bank_address` is safe from
            # the rename.
            flag = regs.get("d0")
            if flag is None:
                self.fail(pc, "a flag clear with no id in d0")
            bank = self.clears[target]
            writes.append(Write("flag_clear", pc, {
                "bank": bank,
                "bank_address": f"0xFFFF{FLAG_BANK_ADDRESSES[bank]:04X}",
                "flag": flag,
                "flag_hex": f"0x{flag:02X}",
                "door": f"0x{target:06X}",
            }))
            return pc + size
        note = f"call to 0x{target:06X}"
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
