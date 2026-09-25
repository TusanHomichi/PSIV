"""The decoded vocabulary: what a flag gate, a write, a path and its kinds are.

Everything the decoder produces is one of these, and everything a consumer
reads is these serialised. `Write.at` is the routine-local address the write
was decoded from, which is provenance and not a RAM address: the coordinates a
runtime speaks are the object index or the layout cell the detail carries.

The vocabulary is deliberately narrow. `Gate` records one flag test with the
polarity the branch selected; `Path` records the writes a run of instructions
reaches together with the gates that reach them, the conditions this slice
recognises but does not model, and the effects it recognises but does not
decode. A path that mixes a Slice-1 write with later work says so in
`deferred` rather than looking complete.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any

SLICE_ONE_KINDS = (
    "object_despawn", "object_rewrite", "object_dialogue",
    "layout_write", "layout_replace", "flag_clear",
)


class MapEffectsError(ValueError):
    pass


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
    #: Conditions this slice recognises but does not model -- currently the
    #: player-position compares two Garuberk routines make. A path carrying one
    #: is NOT unconditional, and saying so would tell a consumer to apply it on
    #: every load.
    conditions: list[str] = field(default_factory=list)
    #: Effects this slice recognises but does not decode, so that a routine
    #: mixing Slice 1 with later work says so instead of looking complete.
    deferred: list[str] = field(default_factory=list)


#: `psiv_tools.symbols` has no event-flag table beyond the eight the overworld
#: hooks test, so these are resolved where a symbol exists and left null where
#: the disassembly names none. The id is always emitted.
def _flag_symbol(bank: str, flag: int) -> str | None:
    from ..symbols import EVENT_FLAG_SYMBOLS

    return EVENT_FLAG_SYMBOLS.get(flag) if bank == "event_flags" else None
