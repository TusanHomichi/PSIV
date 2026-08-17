"""Battle art in the runtime pack: enemy bodies, overlays, poses, backgrounds.

`psiv_tools.battle_art` and `psiv_tools.planes` decode; this writes. It emits
under `battle/art/`, beside the battle data `psiv_tools.battle_pack` already
writes, and returns the `art` subtree of that module's manifest fragment.

Four shape decisions, each of which is a claim about the cartridge rather than
a preference, so each is stated with what proves it.

## Enemy PNGs carry no CRAM line

An enemy occupies CRAM line 1 or line 2, and which one is a property of the
*battle slot* -- bit 7 of the `Enemy_Positions` entry -- not of the enemy. Baking
either line into the image would make an enemy that appears in both slots need
two files, and would hide the choice from the consumer that actually makes it.

So the emitted pixels are line-relative CRAM indices, 0-15, exactly as
`loc_10ED4` leaves them. That works cleanly because no enemy mapping word in the
cartridge sets a palette bit (all 153 checked), so the mapping composes at line
0 and the indices come out unshifted.

The palette is line-agnostic for a stronger reason than convention: of the
sixteen entries, only index 15 differs between line 1 (`$0CC4`) and line 2
(`$062E`), and **no enemy body pixel references index 15 or 14 at all** -- the
153 bodies use indices 0 through 13 and nothing else. Indices 1, 2 and 14 are
fixed values the loader writes identically for both lines. The PNG therefore
carries the whole line-independent palette and a black placeholder at 15, and
`palette_layout.index_15_by_cram_line` carries both real values for whoever
draws the UI.

## Poses are keyed on index

`loc_8244` reaches a pose with `movea.l (a0,d0.w),a0` where `d0 = 4 * pose`, so
a pose *is* its position in the PLC block and pose 0 is idle for all eleven
characters. The names come from `reference/ps4disasm` and are advisory: they
travel as `{pose, label, label_source}` so a consumer can show "Cast" in a
debug view without any code keying on the string.

## Backgrounds bake their palette, and enemies do not

A background is the opposite case from an enemy body, so it gets the opposite
treatment. `loc_6C3C` loads exactly one palette with the art -- CRAM line 0,
index 0 forced black, thirteen colours after it -- and there is no slot, no
line choice and no variant. Baking it is the honest answer; the JSON carries
the colours anyway.

What a background *does* need is the answer to "which one". Three tables feed
`Battle_SetupBackground`, tried in this order, and all three are emitted
because a background nobody can select would be dead data:

* `EventBattleBGIndexes` (27) when `Event_Battle_Index` is not negative;
* `Battle_BackgroundIndexes` (416, one per map id) when `Field_Map_Index` is
  not zero;
* `MotaBattleBGIndexes` (42, one per Motavia terrain byte) otherwise, storing
  index+1 with 0 reserved for index 0.

Plus one rule that is not a table: `loc_6C24` swaps index 4 for index 5 once
`EventFlag_DarkForce2` is set, and that is the *only* way background 5 is ever
reached. All 32 are selectable once it is counted.

`Battle_BackgroundIndexes` is 416 bytes for a 417-map id space, the same
off-by-one `Battle_EnemyFormationIndexes` has. MapID `$1A0` reads the byte past
the end, which belongs to `loc_6E48`; it is dormant because that map has random
battles disabled.

## Enemy overlays are additive

`loc_7C56` copies animated Art #2 tiles into an Art #3 staging bank. The
strict decoder and per-piece frame emitter live in
`psiv_tools.battle_enemy_overlays`; this module adds their JSON and PNGs
beside the existing body files. A frame is full-body-sized on purpose: colour
zero in a replacement tile must clear the old body pixel, so consumers copy
the declared body placements rather than alpha-stack an old body underneath.

Sega pixels: the output is gitignored pack, never committed.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any, Sequence

from . import planes, png
from .battle_art import (
    BLANK_PATTERN,
    CHARACTER_BASE_WORD,
    CHARACTER_COLUMNS,
    CHARACTER_COUNT,
    CHARACTER_CRAM_LINE,
    CHARACTER_PALETTE_COLORS,
    CHARACTER_PALETTE_FIRST_INDEX,
    CHARACTER_PALETTE_TABLE,
    CHARACTER_PLC_TABLE,
    CHARACTER_ROWS,
    CHARACTER_SLOT_WORDS,
    ENEMY_ART_BANK_ORDER,
    ENEMY_ART_TABLE,
    ENEMY_COUNT,
    ENEMY_CRAM_LINES,
    ENEMY_FIXED_COLORS,
    ENEMY_PALETTE_COLORS,
    ENEMY_PALETTE_FIRST_INDEX,
    ENEMY_PALETTE_TABLE,
    RAW_MAPPING_ENEMY_IDS,
    UI_COLOR_14,
    UI_COLOR_SITES,
    BattleArtError,
    build_tile_bank,
    character_art_bank,
    character_art_extents,
    character_cram_line,
    character_records,
    enemy_art_bounds,
    enemy_mapping,
    enemy_palette_words,
    enemy_records,
    verify_ui_colors,
)
from .battle_enemy_overlays import (
    OVERLAY_ART_DIRECTORY,
    OVERLAY_ART_NAME,
    emit_enemy_overlays,
)
from .enigma import decompress as enigma_decompress
from .gfx import (
    BATTLE_BG_ART_SYMBOLS,
    BATTLE_BG_PALETTE_COLORS,
    BATTLE_BG_PALETTE_FIRST_INDEX,
    BATTLE_BG_PALETTE_SYMBOLS,
    BATTLE_BG_TABLE,
    COLORS_PER_LINE,
    RGB,
    decode_palette,
    decode_tiles,
    decompress_art,
    palette_rgb,
)
from .nemesis import TILE_SIZE

BATTLE_DIRECTORY = "battle"
ART_DIRECTORY = f"{BATTLE_DIRECTORY}/art"
ENEMY_ART_NAME = f"{ART_DIRECTORY}/enemies.json"
CHARACTER_ART_NAME = f"{ART_DIRECTORY}/characters.json"
ENEMY_PNG_DIRECTORY = f"{ART_DIRECTORY}/enemies"
CHARACTER_PNG_DIRECTORY = f"{ART_DIRECTORY}/characters"
# Public pack names for the additive dynamic-tile extension.
ENEMY_OVERLAY_PNG_DIRECTORY = OVERLAY_ART_DIRECTORY
ENEMY_OVERLAY_ART_NAME = OVERLAY_ART_NAME
# Attack-object frames are emitted by the animation scout because their
# mapping records and movement provenance live there.  They sit beside the
# body and dynamic replacement indices in the same battle-art namespace.
ENEMY_ATTACK_ART_DIRECTORY = "battle/art/enemy_attacks"
ENEMY_ATTACK_ART_NAME = "battle/art/enemy_attacks.json"

#: CRAM index 0 is the backdrop in every line, so it is transparent rather than
#: a colour, and the entry the PNG carries for it is never drawn.
TRANSPARENT_INDEX = 0

#: Index 15 is the one entry that differs between an enemy's two possible CRAM
#: lines. No enemy body pixel references it, so the PNG carries a placeholder
#: and the real values stay in the JSON.
LINE_DEPENDENT_INDEX = 15
PLACEHOLDER_COLOR: RGB = (0, 0, 0)

#: Pose names transcribed from `reference/ps4disasm`, which labels every
#: mapping `MapEni_<Character>Battle<Label>`. Advisory only: the cartridge
#: addresses a pose by its index in the PLC block and knows no names. The
#: oracle test re-derives these from the clone when it is checked out.
POSE_LABELS: dict[str, tuple[str, ...]] = {
    "Chaz": ("Idle", "Cast"),
    "Alys": ("Idle", "Cast", "Slasher", "ThrowSlasher", "TwoSlashers",
             "ThrowTwoSlashers"),
    "Hahn": ("Idle", "Cast", "ArmsRaised"),
    "Rune": ("Idle", "Cast", "ChargeMagic"),
    "Gryz": ("Idle", "Cast", "ArmsRaised"),
    "Rika": ("Idle", "Cast"),
    "Demi": ("Idle", "Cast", "Charge", "LoadGun", "FireGun"),
    "Wren": ("Idle", "Cast", "Charge", "LoadGun", "FireGun"),
    "Raja": ("Idle", "Cast", "ArmsRaised", "ArmsRaised2"),
    "Kyra": ("Idle", "Cast", "Slasher", "ThrowSlasher", "TwoSlashers",
             "ThrowTwoSlashers", "ChargeMagic", "Cast2"),
    "Seth": ("Idle", "Cast"),
}
POSE_LABEL_SOURCE = "ps4disasm"

#: The four `loc_76A4` entries, in `Vehicle_Index` order.
VEHICLE_VARIANTS: tuple[str, ...] = ("on_foot", "land_rover", "ice_digger", "hydrofoil")

BACKGROUND_PNG_DIRECTORY = f"{ART_DIRECTORY}/backgrounds"
BACKGROUND_ART_NAME = f"{ART_DIRECTORY}/backgrounds.json"

# ---------------------------------------------------------------------------
# Which background a battle uses
# ---------------------------------------------------------------------------
# `Battle_SetupBackground` tries three sources in order, and every offset below
# was followed out of that routine's own operands rather than taken from the
# clone: the two `lea (pc)` displacements give the first two tables, and
# `GetMotaBattleBGIndex`'s `lea (abs).l` gives the third.
#
# Each table's length is bracketed by what follows it, so none of the three
# counts is a guess:
#
#   EventBattleBGIndexes  0x006C8C + 27, `even` -> 0x006CA8
#   Battle_BackgroundIndexes 0x006CA8 + 416    -> 0x006E48 (`loc_6E48`)
#   MotaBattleBGIndexes   0x0586EC + 42        -> 0x058716 (`loc_58716`)
BATTLE_SETUP_BACKGROUND = 0x006BFC
#: `moveq #0,d0 / move.b (Event_Battle_Index).l,d0 / bmi.s`, the routine's head.
BATTLE_SETUP_SIGNATURE = bytes.fromhex("7000103900ffecfc6b0a41fa")

EVENT_BG_INDEXES = 0x006C8C
EVENT_BG_COUNT = 27  # one per boss/event battle
FIELD_MAP_BG_INDEXES = 0x006CA8
#: 416 for a 417-map id space, exactly like `Battle_EnemyFormationIndexes`.
FIELD_MAP_BG_COUNT = 416
MOTA_BG_INDEXES = 0x0586EC
MOTA_BG_COUNT = 42

#: `Battle_BackgroundIndexes` stores this for a map that never picks one.
NO_BACKGROUND = 0xFF

#: `loc_6C24`: index 4 becomes 5 once `EventFlag_DarkForce2` is set, which is
#: the only way background 5 is ever reached.
DARK_FORCE_2_SWAP = (4, 5)
DARK_FORCE_2_FLAG = "EventFlag_DarkForce2"

#: `MotaBattleBGIndexes` stores index+1 and reserves 0 for "index 0", so the
#: stored byte is not the background number: `beq.s + / subq.b #1,d0`.
MOTA_STORED_BIAS = 1


def _safe(label: str) -> str:
    return "".join(c if c.isalnum() or c in "-_" else "_" for c in label)


def _rgb(words: Sequence[int]) -> list[RGB]:
    return palette_rgb(decode_palette(b"".join(w.to_bytes(2, "big") for w in words)))


def _write(directory: Path, name: str, payload: dict[str, Any], version: int) -> str:
    """Write one pack JSON and return its sha256, deterministically."""
    path = directory / name
    path.parent.mkdir(parents=True, exist_ok=True)
    text = json.dumps({"format_version": version, **payload}, indent=2, sort_keys=True) + "\n"
    data = text.encode("utf-8")
    path.write_bytes(data)
    return hashlib.sha256(data).hexdigest()


# ---------------------------------------------------------------------------
# Enemies
# ---------------------------------------------------------------------------
def enemy_palette(rom: bytes, enemy_id: int) -> list[RGB]:
    """The sixteen colours an enemy's CRAM line holds, minus the line choice.

    Index 0 is the transparent backdrop and index 15 is the only entry that
    differs between lines 1 and 2, so both are placeholders here; everything
    between them is what `loc_10ED4` writes for either line.
    """
    words = [0x0000] * COLORS_PER_LINE
    for index, value in ENEMY_FIXED_COLORS.items():
        words[index] = value
    for offset, value in enumerate(enemy_palette_words(rom, enemy_id)):
        words[ENEMY_PALETTE_FIRST_INDEX + offset] = value
    words[14] = UI_COLOR_14
    colors = _rgb(words)
    colors[TRANSPARENT_INDEX] = PLACEHOLDER_COLOR
    colors[LINE_DEPENDENT_INDEX] = PLACEHOLDER_COLOR
    return colors


def enemy_body_png(
    rom: bytes, record: dict[str, Any], bounds: dict[int, int] | None = None
) -> tuple[bytes, list[int], dict[str, Any]]:
    """One enemy body as a line-agnostic indexed PNG.

    Returns the image, the composed CRAM indices, and the mapping metadata. The
    mapping words go in unmodified: they carry no palette bits, so the compose
    step leaves the indices in 0-15 and the caller's palette is a single line.
    """
    tiles, sources = build_tile_bank(rom, record, bounds)
    words, mapping = enemy_mapping(rom, record)
    if any(word & 0x6000 for word in words):
        raise BattleArtError(
            f"enemy 0x{record['id']:02X} ({record['symbol']}): its mapping sets "
            "palette bits, so the body cannot be emitted without a CRAM line"
        )
    cells = planes.decode_cells(words)
    width, height, pixels = planes.compose(cells, mapping["columns"], tiles)
    image = png.encode_indexed(
        width, height, pixels, enemy_palette(rom, record["id"]), (TRANSPARENT_INDEX,)
    )
    mapping["holes"] = sum(
        1 for word in words if (word & 0x07FF) and tiles[word & 0x07FF] == BLANK_PATTERN
    )
    mapping["bank_patterns"] = len(tiles)
    mapping["patterns_used"] = max(word & 0x07FF for word in words) + 1
    mapping["sources"] = sources
    return image, list(pixels), mapping


def build_enemies(rom: bytes, directory: Path) -> tuple[dict[str, Any], int]:
    """Write the 153 enemy body PNGs; return the index payload and byte count."""
    records = enemy_records(rom)
    bounds = enemy_art_bounds(records, len(rom))
    (directory / ENEMY_PNG_DIRECTORY).mkdir(parents=True, exist_ok=True)

    entries: list[dict[str, Any]] = []
    total_bytes = 0
    indices_seen: set[int] = set()
    for record in records:
        image, pixels, mapping = enemy_body_png(rom, record, bounds)
        indices_seen.update(pixels)
        name = f"{record['id']:03d}_{_safe(record['symbol'] or 'unknown')}.png"
        path = f"{ENEMY_PNG_DIRECTORY}/{name}"
        (directory / path).write_bytes(image)
        total_bytes += len(image)
        entries.append({
            "id": record["id"],
            "symbol": record["symbol"],
            "png": path,
            "png_sha256": hashlib.sha256(image).hexdigest(),
            "record_offset": f"0x{record['record_offset']:06X}",
            # The record stores a half-width; `loc_10ED4` doubles it with
            # `add.w d1,d1` before drawing, so `width_cells` is the drawn grid
            # and `half_width_cells` is the byte as stored.
            "half_width_cells": record["half_width_cells"],
            "width_cells": mapping["columns"],
            "height_cells": mapping["rows"],
            "width_pixels": mapping["columns"] * 8,
            "height_pixels": mapping["rows"] * 8,
            # A VRAM reservation, not a tile count: `loc_7BFA` adds it to the
            # running allocator and it bounds nothing.
            "vram_patterns": record["vram_patterns"],
            "base_pattern": 0,
            "bank_patterns": mapping["bank_patterns"],
            "patterns_used": mapping["patterns_used"],
            "body_complete": mapping["holes"] == 0,
            "body_holes": mapping["holes"],
            "mapping": {
                "rom_offset": mapping["rom_offset"],
                "format": mapping["format"],
                "cell_count": mapping["cell_count"],
            },
            "art": [
                {
                    "field": source["field"],
                    "rom_offset": source["rom_offset"],
                    "compression": "nemesis",
                    "pattern_count": source["tile_count"],
                    "first_pattern": source["first_pattern"],
                }
                for source in mapping["sources"]
            ],
            "palette_words": [
                f"0x{word:04X}" for word in enemy_palette_words(rom, record["id"])
            ],
            "palette_rom_offset": (
                f"0x{ENEMY_PALETTE_TABLE['rom_offset'] + record['id'] * 22:06X}"
            ),
        })

    payload = {
        "kind": "battle_enemy_art",
        "count": len(entries),
        "art_table": {
            "label": ENEMY_ART_TABLE["label"],
            "rom_offset": f"0x{ENEMY_ART_TABLE['rom_offset']:06X}",
            "entry_size": ENEMY_ART_TABLE["entry_size"],
            "entry_count": ENEMY_COUNT,
        },
        "art_bank_order": [field + 1 for field in ENEMY_ART_BANK_ORDER],
        "raw_mapping_enemy_ids": sorted(RAW_MAPPING_ENEMY_IDS),
        "palette_layout": _enemy_palette_layout(sorted(indices_seen)),
        "body_layer_only": {
            "overlay": "loc_7C56 EnemySpriteMappingsOffs piece lists",
            "note": (
                "This index is the static plane body. The additive "
                "enemy_overlays.json index carries the dynamic tile replacement "
                "frames and body placements."
            ),
        },
        "enemies": entries,
    }
    return payload, total_bytes


def _enemy_palette_layout(indices_used: Sequence[int]) -> dict[str, Any]:
    by_line = {
        str(line): f"0x{value:04X}"
        for _, line, index, value in UI_COLOR_SITES
        if index == LINE_DEPENDENT_INDEX
    }
    return {
        "table": {
            "label": ENEMY_PALETTE_TABLE["label"],
            "rom_offset": f"0x{ENEMY_PALETTE_TABLE['rom_offset']:06X}",
            "entry_size": ENEMY_PALETTE_TABLE["entry_size"],
            "entry_count": ENEMY_COUNT,
        },
        "cram_lines": list(ENEMY_CRAM_LINES),
        "line_chosen_by": "bit 7 of the battle slot's Enemy_Positions entry",
        "png_pixels_are": "CRAM indices within the enemy's line, 0-15",
        "first_table_index": ENEMY_PALETTE_FIRST_INDEX,
        "table_color_count": ENEMY_PALETTE_COLORS,
        "fixed_colors": {
            str(index): f"0x{value:04X}" for index, value in sorted(ENEMY_FIXED_COLORS.items())
        },
        "ui_color_14": f"0x{UI_COLOR_14:04X}",
        "index_15_by_cram_line": by_line,
        "transparent_index": TRANSPARENT_INDEX,
        "placeholder_indices": [TRANSPARENT_INDEX, LINE_DEPENDENT_INDEX],
        "body_color_indices_used": list(indices_used),
        "note": (
            "Index 15 is the only entry that differs between CRAM lines 1 and 2, "
            "and no enemy body pixel references it or index 14. The PNG palette "
            "is therefore the whole line-independent set, with black placeholders "
            "at the transparent index and at 15."
        ),
    }


# ---------------------------------------------------------------------------
# Characters
# ---------------------------------------------------------------------------
def character_palettes(rom: bytes) -> list[dict[str, Any]]:
    """CRAM line 3 in each of its four `Vehicle_Index` variants."""
    variants = []
    for index, name in enumerate(VEHICLE_VARIANTS):
        words = character_cram_line(rom, index)
        variants.append({
            "vehicle_index": index,
            "name": name,
            "rom_offset": (
                f"0x{CHARACTER_PALETTE_TABLE['rom_offset'] + index * CHARACTER_PALETTE_TABLE['entry_size']:06X}"
            ),
            "words": [f"0x{word:04X}" for word in words[1:]],
            "colors": [list(color) for color in _rgb(words)],
        })
    return variants


def character_pose_png(rom: bytes, pose: dict[str, Any], patterns: int) -> tuple[bytes, list[int]]:
    """One character pose as an indexed PNG, at the on-foot palette.

    The mapping words go in without `CHARACTER_BASE_WORD`: that immediate adds
    the priority bit and palette line 3, neither of which is a pixel. Dropping
    it leaves the composed values as plain CRAM indices, the same convention the
    enemy bodies use.
    """
    tiles = character_art_bank(rom, pose["art_offset"], patterns)
    words, _ = enigma_decompress(rom, pose["mapping_offset"], max_words=2048)
    words = words[:CHARACTER_SLOT_WORDS]
    if any(word & 0x6000 for word in words):
        raise BattleArtError(
            f"pose mapping at 0x{pose['mapping_offset']:06X} sets palette bits"
        )
    cells = planes.decode_cells(words)
    width, height, pixels = planes.compose(cells, CHARACTER_COLUMNS, tiles)
    palette = _rgb(character_cram_line(rom, 0))
    palette[TRANSPARENT_INDEX] = PLACEHOLDER_COLOR
    image = png.encode_indexed(width, height, pixels, palette, (TRANSPARENT_INDEX,))
    return image, list(pixels)


def build_characters(rom: bytes, directory: Path) -> tuple[dict[str, Any], int]:
    """Write the 43 pose PNGs; return the index payload and byte count."""
    records = character_records(rom)
    extents = character_art_extents(rom, records)
    (directory / CHARACTER_PNG_DIRECTORY).mkdir(parents=True, exist_ok=True)

    entries: list[dict[str, Any]] = []
    total_bytes = 0
    total_poses = 0
    for record in records:
        labels = POSE_LABELS.get(record["symbol"], ())
        poses = []
        for pose in record["poses"]:
            patterns = extents[pose["art_offset"]]
            image, _ = character_pose_png(rom, pose, patterns)
            name = f"{_safe(record['symbol'])}_pose{pose['pose']}.png"
            path = f"{CHARACTER_PNG_DIRECTORY}/{name}"
            (directory / path).write_bytes(image)
            total_bytes += len(image)
            total_poses += 1
            words, consumed = enigma_decompress(
                rom, pose["mapping_offset"], max_words=2048
            )
            poses.append({
                # The index is the key: `loc_8244` reaches a pose with
                # `movea.l (a0,d0.w),a0`, d0 = 4 * pose. The label is a
                # researcher's name for it and nothing addresses it.
                "pose": pose["pose"],
                "label": labels[pose["pose"]] if pose["pose"] < len(labels) else None,
                "label_source": POSE_LABEL_SOURCE,
                "png": path,
                "png_sha256": hashlib.sha256(image).hexdigest(),
                "mapping_rom_offset": f"0x{pose['mapping_offset']:06X}",
                "mapping_consumed": consumed,
                "cell_count": len(words),
                "fits_slot": len(words) <= CHARACTER_SLOT_WORDS,
                "art_rom_offset": f"0x{pose['art_offset']:06X}",
                "patterns_used": max(word & 0x07FF for word in words) + 1,
            })
        art = []
        for offset in record["art_offsets"]:
            patterns = extents[offset]
            art.append({
                "rom_offset": f"0x{offset:06X}",
                "compression": None,
                "pattern_count": patterns,
                "size_bytes": patterns * TILE_SIZE,
                "rom_end_exclusive": f"0x{offset + patterns * TILE_SIZE:06X}",
            })
        entries.append({
            "id": record["id"],
            "symbol": record["symbol"],
            "plc_rom_offset": f"0x{record['plc_offset']:06X}",
            # A DMA allocation in bytes, which over-reserves; `art[].pattern_count`
            # is what the poses actually reach.
            "vram_bytes": record["vram_bytes"],
            "art": art,
            "pose_count": len(poses),
            "poses": poses,
        })

    payload = {
        "kind": "battle_character_art",
        "count": len(entries),
        "pose_count": total_poses,
        "plc_table": {
            "label": CHARACTER_PLC_TABLE["label"],
            "rom_offset": f"0x{CHARACTER_PLC_TABLE['rom_offset']:06X}",
            "entry_count": CHARACTER_COUNT,
        },
        "grid_cells": [CHARACTER_COLUMNS, CHARACTER_ROWS],
        "slot_words": CHARACTER_SLOT_WORDS,
        "base_word": f"0x{CHARACTER_BASE_WORD:04X}",
        "palette": {
            "cram_line": CHARACTER_CRAM_LINE,
            "table": {
                "label": CHARACTER_PALETTE_TABLE["label"],
                "rom_offset": f"0x{CHARACTER_PALETTE_TABLE['rom_offset']:06X}",
                "entry_size": CHARACTER_PALETTE_TABLE["entry_size"],
                "entry_count": CHARACTER_PALETTE_TABLE["entry_count"],
            },
            "first_index": CHARACTER_PALETTE_FIRST_INDEX,
            "color_count": CHARACTER_PALETTE_COLORS,
            "png_pixels_are": "CRAM indices within line 3, 0-15",
            "png_baked_variant": 0,
            "transparent_index": TRANSPARENT_INDEX,
            "variants": character_palettes(rom),
            "ui_colors": verify_ui_colors(rom),
            "note": (
                "All eleven characters share CRAM line 3, selected by "
                "Vehicle_Index. The PNGs bake variant 0 for convenience; the "
                "pixel indices are the authority and all four variants are here."
            ),
        },
        "pose_labels": {
            "source": POSE_LABEL_SOURCE,
            "advisory": True,
            "note": (
                "A pose is addressed by its index in the PLC block; the names "
                "come from the public disassembly and nothing keys on them."
            ),
        },
        "characters": entries,
    }
    return payload, total_bytes


# ---------------------------------------------------------------------------
# Backgrounds
# ---------------------------------------------------------------------------
def background_selection(rom: bytes) -> dict[str, Any]:
    """The three tables `Battle_SetupBackground` chooses a background from.

    Read from the routine's own operands and bracketed by what follows each
    table, so the offsets and the counts are both the cartridge's. A background
    nobody can select would be dead data, so the reachable set is computed here
    and the census reports it.
    """
    head = rom[BATTLE_SETUP_BACKGROUND:BATTLE_SETUP_BACKGROUND + len(BATTLE_SETUP_SIGNATURE)]
    if head != BATTLE_SETUP_SIGNATURE:
        raise BattleArtError(
            f"Battle_SetupBackground at 0x{BATTLE_SETUP_BACKGROUND:06X} does not "
            "start with its Event_Battle_Index test"
        )
    event = list(rom[EVENT_BG_INDEXES:EVENT_BG_INDEXES + EVENT_BG_COUNT])
    field = list(rom[FIELD_MAP_BG_INDEXES:FIELD_MAP_BG_INDEXES + FIELD_MAP_BG_COUNT])
    mota = list(rom[MOTA_BG_INDEXES:MOTA_BG_INDEXES + MOTA_BG_COUNT])
    return {
        "routine": "Battle_SetupBackground",
        "rom_offset": f"0x{BATTLE_SETUP_BACKGROUND:06X}",
        "order": ["event_battle", "field_map", "motavia_terrain"],
        "event_battle": {
            "table": "EventBattleBGIndexes",
            "rom_offset": f"0x{EVENT_BG_INDEXES:06X}",
            "count": EVENT_BG_COUNT,
            "indexed_by": "Event_Battle_Index, when it is not negative",
            "indexes": event,
        },
        "field_map": {
            "table": "Battle_BackgroundIndexes",
            "rom_offset": f"0x{FIELD_MAP_BG_INDEXES:06X}",
            "count": FIELD_MAP_BG_COUNT,
            "indexed_by": "Field_Map_Index, when it is not zero",
            "none": NO_BACKGROUND,
            "indexes": field,
        },
        "motavia_terrain": {
            "table": "MotaBattleBGIndexes",
            "rom_offset": f"0x{MOTA_BG_INDEXES:06X}",
            "count": MOTA_BG_COUNT,
            "indexed_by": (
                "the Motavia overworld terrain byte, when Field_Map_Index is 0"
            ),
            "stored_bias": MOTA_STORED_BIAS,
            "note": (
                "GetMotaBattleBGIndex stores index+1 and keeps 0 for index 0, so "
                "a background number is max(stored - 1, 0)."
            ),
            "indexes": mota,
        },
        "vehicle_mounted": {
            "selector": "Vehicle_Index != 0",
            "event_battle": "event_battle remains first when a scene supplies Event_Battle_Index",
            "mapped_planets": "Battle_BackgroundIndexes[Field_Map_Index]",
            "motavia": "MotaBattleBGIndexes[raw_chunk] when Field_Map_Index is 0",
            "debug_unmapped_fallback": (
                "use the Motavia chunk table only when a mounted debug map has "
                "no field-map background entry"
            ),
            "source": "Battle_SetupBackground and GetMotaBattleBGIndex",
        },
        "dark_force_2_swap": {
            "from": DARK_FORCE_2_SWAP[0],
            "to": DARK_FORCE_2_SWAP[1],
            "flag": DARK_FORCE_2_FLAG,
            "note": (
                "loc_6C24 swaps index 4 for 5 once the flag is set; it is the "
                "only path that reaches background 5."
            ),
        },
    }


def _reachable(selection: dict[str, Any]) -> dict[int, dict[str, int]]:
    """How many times each background index is selected, per path."""
    counts: dict[int, dict[str, int]] = {}

    def note(index: int, path: str) -> None:
        entry = counts.setdefault(index, {"event_battles": 0, "maps": 0, "motavia_terrain": 0})
        entry[path] += 1

    for index in selection["event_battle"]["indexes"]:
        note(index, "event_battles")
    for index in selection["field_map"]["indexes"]:
        if index != NO_BACKGROUND:
            note(index, "maps")
    for stored in selection["motavia_terrain"]["indexes"]:
        note(max(stored - MOTA_STORED_BIAS, 0), "motavia_terrain")
    return counts


def build_backgrounds(rom: bytes, directory: Path) -> tuple[dict[str, Any], int]:
    """Compose the 32 battle backgrounds; return the index payload and bytes.

    Composition is `psiv_tools.planes`' -- the art and its Enigma mapping
    bracket each other in the pointer table, which is what bounds both streams
    without anything outside the cartridge.
    """
    art_ptrs, map_ptrs, palette_ptrs = planes._battle_pointers(rom)
    landmarks = art_ptrs + map_ptrs + list(planes.BATTLE_BG_EXTRA_LANDMARKS)
    map_bounds = planes._gap_bounds(landmarks, len(rom), map_ptrs)
    art_bounds = planes._gap_bounds(landmarks, len(rom), art_ptrs)
    (directory / BACKGROUND_PNG_DIRECTORY).mkdir(parents=True, exist_ok=True)

    selection = background_selection(rom)
    reachable = _reachable(selection)
    columns, rows = planes.BATTLE_BG_COLUMNS, planes.BATTLE_BG_ROWS

    entries: list[dict[str, Any]] = []
    total_bytes = 0
    distinct_art: set[int] = set()
    for index in range(BATTLE_BG_TABLE["entry_count"]):
        art_ptr, map_ptr, palette_ptr = art_ptrs[index], map_ptrs[index], palette_ptrs[index]
        distinct_art.add(art_ptr)
        art_symbol = BATTLE_BG_ART_SYMBOLS[index]
        symbol = BATTLE_BG_PALETTE_SYMBOLS[index]
        decompressed, art_record = decompress_art(
            rom, art_ptr, f"ArtNem_{art_symbol}BattleBG",
            compressed_size=art_bounds[art_ptr],
        )
        words, mapping_record = planes.decode_mapping(
            rom, map_ptr, f"MapEni_{art_symbol}BattleBG",
            base_tile=planes.BATTLE_BG_BASE_TILE,
            compressed_size=map_bounds[map_ptr],
            expected_words=columns * rows,
        )
        raw = rom[palette_ptr:palette_ptr + BATTLE_BG_PALETTE_COLORS * 2]
        colors = decode_palette(raw)
        image = planes.render(
            planes.decode_cells(words), columns, decode_tiles(decompressed),
            planes.battle_cram(colors), planes.BATTLE_BG_ART_VRAM_TILE,
        )
        name = f"{index:02d}_{_safe(symbol)}.png"
        path = f"{BACKGROUND_PNG_DIRECTORY}/{name}"
        (directory / path).write_bytes(image)
        total_bytes += len(image)
        selected = reachable.get(index, {"event_battles": 0, "maps": 0, "motavia_terrain": 0})
        entries.append({
            "index": index,
            "symbol": symbol,
            "art_symbol": art_symbol,
            "png": path,
            "png_sha256": hashlib.sha256(image).hexdigest(),
            "width_cells": columns,
            "height_cells": rows,
            "width_pixels": columns * 8,
            "height_pixels": rows * 8,
            "art": {
                "rom_offset": art_record["rom_offset"],
                "compression": "nemesis",
                "compressed_size": art_record["compressed_size"],
                "pattern_count": art_record["tile_count"],
            },
            "mapping": {
                "rom_offset": mapping_record["rom_offset"],
                "compression": "enigma",
                "compressed_size": mapping_record["compressed_size"],
                "cell_count": columns * rows,
                "base_tile": planes.BATTLE_BG_BASE_TILE,
                "art_vram_tile": planes.BATTLE_BG_ART_VRAM_TILE,
            },
            "palette": {
                "label": f"Pal_{symbol}BattleBG",
                "rom_offset": f"0x{palette_ptr:06X}",
                "cram_line": 0,
                "first_index": BATTLE_BG_PALETTE_FIRST_INDEX,
                "color_count": BATTLE_BG_PALETTE_COLORS,
                "index_0_forced_black": True,
                "colors": [list(color) for color in palette_rgb(colors)],
            },
            "selected_by": selected,
            "selectable": sum(selected.values()) > 0
            or index == DARK_FORCE_2_SWAP[1],
        })

    payload = {
        "kind": "battle_backgrounds",
        "count": len(entries),
        "table": {
            "label": BATTLE_BG_TABLE["label"],
            "rom_offset": f"0x{BATTLE_BG_TABLE['rom_offset']:06X}",
            "entry_size": BATTLE_BG_TABLE["entry_size"],
            "entry_count": BATTLE_BG_TABLE["entry_count"],
            "columns": "art, plane mapping, palette",
        },
        "distinct_art_blobs": len(distinct_art),
        "plane_size_cells": [columns, rows],
        "palette_note": (
            "loc_6C3C clears colour 0 and copies 13 words to "
            "Palette_Table_Buffer+2, so a background palette is CRAM line 0 "
            "indices 1-13 with index 0 forced black. The PNGs bake it: unlike "
            "an enemy body, a background has exactly one palette."
        ),
        "selection": selection,
        "backgrounds": entries,
    }
    return payload, total_bytes


# ---------------------------------------------------------------------------
# Emission
# ---------------------------------------------------------------------------
def emit_battle_art(rom: bytes, out_dir: str | Path, version: int) -> dict[str, Any]:
    """Write `battle/art/` and return the `art` subtree of the battle manifest."""
    from .battle_animations import emit_enemy_attack_art

    directory = Path(out_dir)
    enemies, enemy_bytes = build_enemies(rom, directory)
    overlays, overlay_bytes, overlay_png_count = emit_enemy_overlays(rom, directory)
    attacks, attack_bytes, attack_png_count = emit_enemy_attack_art(rom, directory)
    characters, character_bytes = build_characters(rom, directory)
    backgrounds, background_bytes = build_backgrounds(rom, directory)
    enemy_sha = _write(directory, ENEMY_ART_NAME, enemies, version)
    overlay_sha = _write(directory, ENEMY_OVERLAY_ART_NAME, overlays, version)
    attack_sha = _write(directory, ENEMY_ATTACK_ART_NAME, attacks, version)
    character_sha = _write(directory, CHARACTER_ART_NAME, characters, version)
    background_sha = _write(directory, BACKGROUND_ART_NAME, backgrounds, version)

    complete = sum(1 for entry in enemies["enemies"] if entry["body_complete"])
    holed = len(enemies["enemies"]) - complete
    formats: dict[str, int] = {}
    for entry in enemies["enemies"]:
        kind = entry["mapping"]["format"]
        formats[kind] = formats.get(kind, 0) + 1

    return {
        "directory": ART_DIRECTORY,
        "files": {
            "enemies": {
                "file": ENEMY_ART_NAME,
                "sha256": enemy_sha,
                "count": enemies["count"],
                "png_directory": ENEMY_PNG_DIRECTORY,
                "png_count": enemies["count"],
                "png_bytes": enemy_bytes,
            },
            "characters": {
                "file": CHARACTER_ART_NAME,
                "sha256": character_sha,
                "count": characters["count"],
                "png_directory": CHARACTER_PNG_DIRECTORY,
                "png_count": characters["pose_count"],
                "png_bytes": character_bytes,
            },
            "backgrounds": {
                "file": BACKGROUND_ART_NAME,
                "sha256": background_sha,
                "count": backgrounds["count"],
                "png_directory": BACKGROUND_PNG_DIRECTORY,
                "png_count": backgrounds["count"],
                "png_bytes": background_bytes,
            },
            "enemy_overlays": {
                "file": ENEMY_OVERLAY_ART_NAME,
                "sha256": overlay_sha,
                "count": overlays["piece_count"],
                "png_directory": ENEMY_OVERLAY_PNG_DIRECTORY,
                "png_count": overlay_png_count,
                "png_bytes": overlay_bytes,
            },
            "enemy_attacks": {
                "file": ENEMY_ATTACK_ART_NAME,
                "sha256": attack_sha,
                "count": attacks["count"],
                "png_directory": ENEMY_ATTACK_ART_DIRECTORY,
                "png_count": attack_png_count,
                "png_bytes": attack_bytes,
            },
        },
        # What the emitted art actually contains, so a consumer sees the split
        # without opening 196 files. `body_holes` is the count of enemies with
        # at least one uncovered cell in the static body; the overlay census
        # beside it says which dynamic destination tiles cover those cells.
        "census": {
            "enemies": enemies["count"],
            "body_complete": complete,
            "body_holes": holed,
            "total_body_holes": sum(e["body_holes"] for e in enemies["enemies"]),
            "enemy_overlay_pieces": overlays["piece_count"],
            "enemy_overlay_enabled_pieces": overlays["enabled_piece_count"],
            "enemy_overlay_coverage": overlays["coverage"],
            "enemy_attack_sheets": attacks["census"],
            "enemy_mapping_formats": formats,
            "enemy_cram_lines": list(ENEMY_CRAM_LINES),
            "enemy_body_color_indices": (
                enemies["palette_layout"]["body_color_indices_used"]
            ),
            "characters": characters["count"],
            "poses": characters["pose_count"],
            "poses_per_character": {
                entry["symbol"]: entry["pose_count"] for entry in characters["characters"]
            },
            "character_palette_variants": CHARACTER_PALETTE_TABLE["entry_count"],
            "character_art_blobs": sum(
                len(entry["art"]) for entry in characters["characters"]
            ),
            "backgrounds": backgrounds["count"],
            "background_art_blobs": backgrounds["distinct_art_blobs"],
            # Every background is reachable from one of the three selection
            # paths, so none of the 32 is dead data. Index 5 is the one that
            # takes a flag to reach at all.
            "backgrounds_selectable": sum(
                1 for entry in backgrounds["backgrounds"] if entry["selectable"]
            ),
            "backgrounds_never_selectable": [
                entry["index"] for entry in backgrounds["backgrounds"]
                if not entry["selectable"]
            ],
            "background_maps_bound": sum(
                1 for value in backgrounds["selection"]["field_map"]["indexes"]
                if value != NO_BACKGROUND
            ),
            "background_maps_none": sum(
                1 for value in backgrounds["selection"]["field_map"]["indexes"]
                if value == NO_BACKGROUND
            ),
            "background_event_battles": len(
                backgrounds["selection"]["event_battle"]["indexes"]
            ),
        },
        "note": (
            "Body layer plus the additive animated sprite-piece overlay. "
            "The overlay census records the 87 static hole cells whose decoded "
            "destinations do not fully cover a body."
        ),
    }
