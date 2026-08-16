"""Names, errors and the three helpers the other modules share.

The file layout of the dialogue half of a pack is one interface, so the paths
live here rather than being spelled out wherever a file happens to be written.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

#: Tracks `psiv_tools.pack.PACK_FORMAT_VERSION`. It is repeated rather than
#: imported because `pack` imports this module, not the other way round; the
#: test suite asserts the two are equal.
DIALOGUE_FORMAT_VERSION = 1

DIALOGUE_DIRECTORY = "dialogue"
PORTRAITS_DIRECTORY = f"{DIALOGUE_DIRECTORY}/portraits"
TREES_NAME = f"{DIALOGUE_DIRECTORY}/trees.json"
FONT_JSON_NAME = f"{DIALOGUE_DIRECTORY}/font.json"
FONT_PNG_NAME = f"{DIALOGUE_DIRECTORY}/font.png"
MENU_FONT_PNG_NAME = f"{DIALOGUE_DIRECTORY}/menu_font.png"
WINDOW_JSON_NAME = f"{DIALOGUE_DIRECTORY}/window.json"
WINDOW_PNG_NAME = f"{DIALOGUE_DIRECTORY}/window.png"
PORTRAITS_NAME = f"{DIALOGUE_DIRECTORY}/portraits.json"


class DialoguePackError(ValueError):
    pass

def rom_slice(rom: bytes, offset: int, size: int, what: str) -> bytes:
    if offset < 0 or offset + size > len(rom):
        raise DialoguePackError(
            f"{what} at 0x{offset:06X} (+{size}) runs past the end of the ROM"
        )
    return rom[offset:offset + size]


def safe_name(label: str) -> str:
    return "".join(c if c.isalnum() or c in "-_" else "_" for c in label)


def write_json(path: Path, payload: dict[str, Any]) -> str:
    """Write a pack JSON file and return its sha256.

    Sorted keys, fixed indent, no timestamps and nothing derived from the
    filesystem, so building the same pack twice produces the same bytes.
    """
    data = (json.dumps(payload, indent=2, sort_keys=True) + "\n").encode("utf-8")
    path.write_bytes(data)
    return hashlib.sha256(data).hexdigest()
