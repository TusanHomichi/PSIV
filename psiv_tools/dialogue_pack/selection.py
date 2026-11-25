"""Decode type-0 interaction tree selection from the US retail instructions.

`Interaction_DisplayDialogue`, ps4.asm:118299-118327, is at $058844 in
retail. The English branch includes the first world's high-entry override.
The pointer table is ROM data; only its instruction operands locate it here.
"""

from __future__ import annotations

import hashlib
import struct
from typing import Any

from ..text import dialogue_tree_specs
from .common import DialoguePackError, rom_slice

SELECTION_ROUTINE = 0x058844
WORLD_COUNT = 6  # Destination menu's dbf #5, ps4.asm:133524-133538.
# Pinned by `(offset_within_routine, length, sha256)` (#461).
SELECTION_PINS: tuple[tuple[int, int, str], ...] = (
    (0, 18, "bd22f58d82d598811b289763156ba3ff71d70e5a1c38e29a90e0687c1a6213e1"),
    (20, 4, "d83cc5cb75686878810ffa58c0682b59c3a1eacc6584d3fbcf49f6f496f04735"),
    (26, 2, "df8a7fada9461dc56be0853d003ebb3bc2a4436093c0bdaf84c4ed8c64471417"),
    (32, 14, "b11de8a6ab939268cfda45b94a49f7eabb3f0572d01b9ba1180e07f276954130"),
)


def extract_selection(rom: bytes) -> dict[str, Any]:
    """Read the live-world table and the unsigned first-world entry cutoff.

    Verify the instruction grammar rather than trusting the fork's build
    addresses or copying the table into source. Unknown tree pointers fail.
    """
    code = rom_slice(rom, SELECTION_ROUTINE, 46, "Interaction_DisplayDialogue")
    for offset, length, digest in SELECTION_PINS:
        if hashlib.sha256(code[offset:offset + length]).hexdigest() != digest:
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
