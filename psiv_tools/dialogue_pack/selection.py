"""Decode type-0 interaction tree selection from the US retail instructions.

`Interaction_DisplayDialogue`, ps4.asm:118299-118327, is at $058844 in
retail. The English branch includes the first world's high-entry override.
The pointer table is ROM data; only its instruction operands locate it here.
"""

from __future__ import annotations

import struct
from typing import Any

from ..text import dialogue_tree_specs
from .common import DialoguePackError, rom_slice

SELECTION_ROUTINE = 0x058844
WORLD_COUNT = 6  # Destination menu's dbf #5, ps4.asm:133524-133538.


def extract_selection(rom: bytes) -> dict[str, Any]:
    """Read the live-world table and the unsigned first-world entry cutoff.

    Verify the instruction grammar rather than trusting the fork's build
    addresses or copying the table into source. Unknown tree pointers fail.
    """
    code = rom_slice(rom, SELECTION_ROUTINE, 46, "Interaction_DisplayDialogue")
    checks = (
        (0, "48e7088070001038f400d040d040660a0c04"),
        (20, "6504303c"),
        (26, "41f9"),
        (32, "203000004eb900053f004cdf0110"),
    )
    for offset, signature in checks:
        if code[offset:offset + len(bytes.fromhex(signature))] != bytes.fromhex(signature):
            raise DialoguePackError(
                f"Interaction_DisplayDialogue opcode mismatch at "
                f"0x{SELECTION_ROUTINE + offset:06X}"
            )
    cutoff, override_offset = struct.unpack_from(">Hxx2xH", code, 18)
    if cutoff > 0xFF or override_offset % 4 or override_offset < WORLD_COUNT * 4:
        raise DialoguePackError("invalid byte cutoff or world-tree override offset")
    table = struct.unpack_from(">I", code, 28)[0]
    pointers = rom_slice(rom, table, override_offset + 4, "WorldDialogueTreePtrs")
    identities = {spec["start"]: spec["tree"] for spec in dialogue_tree_specs(rom)}

    def tree_at(offset: int) -> int:
        pointer = struct.unpack_from(">I", pointers, offset)[0]
        if pointer not in identities:
            raise DialoguePackError(f"world dialogue pointer 0x{pointer:06X} is not a tree")
        return identities[pointer]

    return {
        "routine_offset": f"0x{SELECTION_ROUTINE:06X}",
        "table_offset": f"0x{table:06X}",
        "world_trees": [tree_at(index * 4) for index in range(WORLD_COUNT)],
        "first_world_override": {"entry_from": cutoff, "tree": tree_at(override_offset)},
    }
