"""The Zol slugs' Fusion sprites: `BattleObj_Fusion`'s five mapping frames.

`EnemyAttack_Blob` (ps4.asm:23040) sends a Fusion turn to `loc_10796`
(ps4.asm:23062), which Nemesis-loads the pieces' art to VRAM `$A000` and
spawns objects `$140` (`BattleObj_Fusion`, ps4.asm:35675) and `$144`
(`BattleObj_Fusion2`, ps4.asm:35850). Both draw their own mapping record,
which `move.l #mapping, $C(a4)` selects as their frame counter runs:
the standing and flat slug (`loc_1AA5A`, `loc_1AA74`) while they slide
together, then the growing MetaSlug (`loc_1AA82`, `loc_1AAA8`,
`loc_1AACE`) before `loc_14D46` seats the real one. Their pattern word is
`#$A000` (ps4.asm:35726), which selects no CRAM line bits beyond the
enemy line the mapping words leave clear, so the frames are drawn on an
enemy line like the slugs' own bodies.

This module verifies those instructions in the image, decodes the five
records, and emits one indexed PNG per record in the enemy palette of the
Zol slug that casts it. The timing (when each record is selected, the slide
target and the seat) is the runtime's, cited there; the pack carries only
the art and its geometry.
"""

from __future__ import annotations

from pathlib import Path
from typing import Any

from . import png
from .battle_animation_mapping import decode_mapping_record
from .core import be16, be32
from .nemesis import decompress

#: `BattleObj_Fusion`: `bset #7, (a4)` / `bne.w BattleObj_FusionMain` /
#: `jsr (loc_12618).l` (ps4.asm:35675-35678).
FUSION_ROUTINE = 0x01A090
FUSION_SIGNATURE = bytes.fromhex("08D40007 6600")
FUSION_SIGNATURE_TAIL = bytes.fromhex("4EB900012618")
#: `BattleObj_Fusion2`, where the first object's code ends (ps4.asm:35850).
FUSION2_ROUTINE_BOUND = 0x01A300
#: `loc_10796`'s `lea (loc_209766).l, a0` / `jsr (NemDecomp).l`
#: (ps4.asm:23064-23065).
ART_LEA = 0x0107A0
#: `move.l #imm32, $C(a4)`: 297C, the pointer, then the field word.
SET_MAPPING = bytes.fromhex("297C")
MAPPING_FIELD = 0x000C
#: The Zol slug, whose turn this is and whose palette the pieces use.
ZOL_SLUG = 34

FUSION_ART_NAME = "battle/art/fusion.json"
FUSION_PNG_DIRECTORY = "battle/art/fusion"


class FusionArtError(ValueError):
    """The image does not hold the Fusion object this module reads."""


def _hex(value: int) -> str:
    return f"0x{value:06X}"


def fusion_mappings(rom: bytes) -> list[int]:
    """The distinct mapping records `BattleObj_Fusion` selects, in order."""
    head = rom[FUSION_ROUTINE:FUSION_ROUTINE + len(FUSION_SIGNATURE)]
    tail_at = FUSION_ROUTINE + len(FUSION_SIGNATURE) + 2
    tail = rom[tail_at:tail_at + len(FUSION_SIGNATURE_TAIL)]
    if head != FUSION_SIGNATURE or tail != FUSION_SIGNATURE_TAIL:
        raise FusionArtError(
            f"BattleObj_Fusion at {_hex(FUSION_ROUTINE)} does not start with its "
            "first-run test and loc_12618 call"
        )
    mappings: list[int] = []
    for offset in range(FUSION_ROUTINE, FUSION2_ROUTINE_BOUND - 7, 2):
        if rom[offset:offset + 2] != SET_MAPPING:
            continue
        if be16(rom, offset + 6) != MAPPING_FIELD:
            continue
        pointer = be32(rom, offset + 2)
        if pointer not in mappings:
            mappings.append(pointer)
    if len(mappings) != 5:
        raise FusionArtError(
            f"BattleObj_Fusion selects {len(mappings)} mapping records, not 5"
        )
    return mappings


def fusion_art_offset(rom: bytes) -> int:
    """The Nemesis stream `loc_10796` loads for the pieces."""
    if be16(rom, ART_LEA) != 0x41F9 or be16(rom, ART_LEA + 6) != 0x4EB9:
        raise FusionArtError(
            f"loc_10796 at {_hex(ART_LEA)} is not `lea (art).l, a0` / `jsr (NemDecomp).l`"
        )
    return be32(rom, ART_LEA + 2)


def _tile_pixels(data: bytes) -> list[list[int]]:
    tiles = []
    for start in range(0, len(data) - len(data) % 32, 32):
        row: list[int] = []
        for byte in data[start:start + 32]:
            row += [byte >> 4, byte & 0x0F]
        tiles.append(row)
    return tiles


def vdp_size(size: int) -> tuple[int, int]:
    """A sprite's (columns, rows) of tiles: the VDP's size byte keeps the
    horizontal count in bits 3-2 and the vertical one in bits 1-0. Tape 33's
    frames of these pieces fit only this reading, pixel for pixel."""
    return ((size >> 2) & 0x03) + 1, (size & 0x03) + 1


def render_record(tiles: list[list[int]], record: dict[str, Any]) -> dict[str, Any]:
    """One mapping record as an indexed image and its origin.

    A VDP sprite's tiles run down each column first; a flipped entry reads
    its own tiles mirrored. `origin` is the record's top-left relative to the
    object's position (the record's smallest x and y).
    """
    entries = [
        {**entry, "width_tiles": vdp_size(entry["size"])[0],
         "height_tiles": vdp_size(entry["size"])[1]}
        for entry in record["entries"]
    ]
    left = min(entry["x"] for entry in entries)
    top = min(entry["y"] for entry in entries)
    right = max(entry["x"] + entry["width_tiles"] * 8 for entry in entries)
    bottom = max(entry["y"] + entry["height_tiles"] * 8 for entry in entries)
    width, height = right - left, bottom - top
    pixels = bytearray(width * height)
    for entry in entries:
        columns, rows = entry["width_tiles"], entry["height_tiles"]
        if (not entry["attributes_valid"]
                or entry["tile_index"] + columns * rows > len(tiles)):
            raise FusionArtError(f"mapping entry {entry['rom_offset']} is outside the art")
        for y in range(rows * 8):
            source_y = rows * 8 - 1 - y if entry["v_flip"] else y
            for x in range(columns * 8):
                source_x = columns * 8 - 1 - x if entry["h_flip"] else x
                tile = tiles[entry["tile_index"] + (source_x // 8) * rows + source_y // 8]
                pixel = tile[(source_y % 8) * 8 + source_x % 8]
                if pixel:
                    pixels[(entry["y"] - top + y) * width + entry["x"] - left + x] = pixel
    return {"origin": [left, top], "size": [width, height], "pixels": bytes(pixels)}


def emit_fusion_art(rom: bytes, out_dir: str | Path) -> tuple[dict[str, Any], int, int]:
    """Write the Fusion frames; return the payload, PNG bytes and PNG count."""
    from .battle_art_pack import enemy_palette

    art, _ = decompress(rom, fusion_art_offset(rom))
    tiles = _tile_pixels(art)
    palette = enemy_palette(rom, ZOL_SLUG)
    directory = Path(out_dir)
    (directory / FUSION_PNG_DIRECTORY).mkdir(parents=True, exist_ok=True)
    frames: list[dict[str, Any]] = []
    total = 0
    for index, mapping in enumerate(fusion_mappings(rom)):
        record = decode_mapping_record(rom, mapping, len(tiles))
        image = render_record(tiles, record)
        name = f"{FUSION_PNG_DIRECTORY}/{mapping:06X}.png"
        data = png.encode_indexed(image["size"][0], image["size"][1], image["pixels"],
                                  palette, (0,))
        (directory / name).write_bytes(data)
        total += len(data)
        frames.append({
            "index": index,
            "mapping": _hex(mapping),
            "sprites": record["sprite_count"],
            "png": name,
            "origin": image["origin"],
            "size": image["size"],
        })
    payload = {
        "kind": "battle_fusion_art",
        "source": {
            "routine": "BattleObj_Fusion",
            "rom_offset": _hex(FUSION_ROUTINE),
            "art": "loc_10796",
            "art_rom_offset": _hex(fusion_art_offset(rom)),
            "tiles": len(tiles),
        },
        "palette_enemy_id": ZOL_SLUG,
        "count": len(frames),
        "frames": frames,
    }
    return payload, total, len(frames)
