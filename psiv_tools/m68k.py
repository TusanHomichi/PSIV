"""Just enough 68000 to read tables and short routines out of retail.

Several extractors need the same handful of moves: check that a byte signature
occurs exactly as often as retail contains it, read a big-endian field, assert
that a specific opcode sits at an address, follow a `bra.w`, and resolve a
jump table a dispatcher reaches PC-relative. This is that handful, and nothing
more -- it is deliberately not a disassembler. Anything that needs to *walk*
instructions models its own opcode set and fails closed on the rest, the way
`psiv_tools.map_effects` and `psiv_tools.battle_rules` do.

Every function here raises [`DecodeError`]. Modules that want their own error
type subclass it, so a caller can catch either the specific one or the base.
"""

from __future__ import annotations

import re
import struct


class DecodeError(ValueError):
    """A retail image did not hold what the decoder required."""


def find_all(rom: bytes, signature: str) -> list[int]:
    """Every offset at which `signature` (hex) occurs."""
    pattern = bytes.fromhex(signature)
    return [match.start() for match in re.finditer(re.escape(pattern), rom)]


def find_exactly(rom: bytes, signature: str, expected: int, label: str) -> list[int]:
    """`find_all`, refusing any count but `expected`.

    The pin discipline the whole toolchain uses: a signature that occurs a
    different number of times than retail holds means either the wrong build or
    a wrong signature, and both must stop the extraction rather than decode
    whatever happens to be there.
    """
    hits = find_all(rom, signature)
    if len(hits) != expected:
        raise DecodeError(
            f"{label}: signature {signature} occurs {len(hits)} times, not the "
            f"{expected} retail has"
        )
    return hits


def w(rom: bytes, at: int) -> int:
    """The big-endian word at `at`."""
    return struct.unpack_from(">H", rom, at)[0]


def sw(rom: bytes, at: int) -> int:
    """The signed big-endian word at `at`."""
    return struct.unpack_from(">h", rom, at)[0]


def l(rom: bytes, at: int) -> int:  # noqa: E743 - reads as the 68000 size suffix
    """The big-endian long at `at`."""
    return struct.unpack_from(">I", rom, at)[0]


def expect(rom: bytes, at: int, opcode: int, what: str) -> None:
    """Assert that `opcode` is the word at `at`."""
    if w(rom, at) != opcode:
        raise DecodeError(
            f"0x{at:06X}: expected {what} (0x{opcode:04X}), found 0x{w(rom, at):04X}"
        )


def bra_target(rom: bytes, at: int) -> int:
    """The target of the `bra.w` at `at`."""
    expect(rom, at, 0x6000, "bra.w")
    return at + 2 + sw(rom, at + 2)


def pc_relative_table(rom: bytes, at: int, opcode: int, what: str) -> int:
    """The table a `jmp`/`jsr (d8,PC,dN.w)` at `at` dispatches through.

    Both dispatchers in retail use the plain word-indexed form with an
    eight-bit displacement, so an extension word carrying anything in its high
    byte means the instruction is not the one the caller thinks it is.
    """
    expect(rom, at, opcode, what)
    extension = w(rom, at + 2)
    if extension & 0xFF00:
        raise DecodeError(
            f"0x{at:06X}: {what} extension 0x{extension:04X} is not the plain "
            "word-indexed form the dispatchers use"
        )
    return at + 2 + (extension & 0xFF)


def self_bounded_bra_table(rom: bytes, table: int, what: str,
                           limit: int = 64) -> int:
    """How many `bra.w` entries a table has, from its own lowest target.

    Every entry jumps forward past the end of the table, so the table ends
    where the nearest target begins. This is how the ability-effect table,
    both equipment jump tables and the shop portrait groups are bounded: by the
    cartridge, not by a constant.
    """
    lowest: int | None = None
    count = 0
    while lowest is None or table + count * 4 < lowest:
        target = bra_target(rom, table + count * 4)
        if target <= table + count * 4:
            raise DecodeError(
                f"{what}: entry {count} jumps backwards to 0x{target:06X}"
            )
        lowest = target if lowest is None else min(lowest, target)
        count += 1
        if count > limit:
            raise DecodeError(f"{what}: no entry bounds the table")
    return count
