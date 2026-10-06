"""Battle formations, decoded from the Kosinski-compressed retail blobs.

Layout is documented in the public disassembly immediately above the
`Battle_FormationData1` label. Each record is variable length:

    byte 0   agility rating for the surprise-attack check
    byte 1   agility rating for the run check ($F0 or more: cannot run)
    byte 2   item drop rate
    byte 3   dropped item id (0 = nothing)
    byte 4   number of enemies
    byte 5   group 1 membership bitmask (bit N = enemy slot N+1)
    byte 6   group 2 membership bitmask (0 if group 2 is empty)
    byte 7+  (enemy id, position) pairs for 1-4 enemies
    $FF      terminator

Records are concatenated with no index; `Battle_SetupEnemyData` locates
record N by counting $FF terminators from the start of the decompressed
block, which is exactly what `parse_formation_block` does.

Formation ids are global across four blocks of at most $80 records each
(`Battle_SetupEnemyData` subtracts $80 and advances a block at a time). Boss
formations live in a separate id space indexed by the event battle index.
"""

from __future__ import annotations

import hashlib
from typing import Any

from .kosinski import decompress
from .symbols import ENEMY_SYMBOLS, ITEM_SYMBOLS

HEADER_SIZE = 7
TERMINATOR = 0xFF
CANNOT_RUN_THRESHOLD = 0xF0
INDEX_GROUP_SIZE = 0x40
INDEX_ENTRIES_PER_GROUP = 32

# Compressed blobs, end-exclusive, proven against this retail build by
# requiring the decompressor to consume exactly (end - start) bytes.
#
# The disassembly's inline range annotations break `Battle_FormationData4`
# into many small chunks; the first of them ends at 0x284C3D, which is *not*
# the end of the stream. The blob actually runs to 0x284F7C, where
# Battle_BossFormationData begins.
FORMATION_BLOCKS: list[dict[str, Any]] = [
    {"name": "block_1", "label": "Battle_FormationData1", "start": 0x283E6C, "end": 0x2842AE, "id_base": 0x000},
    {"name": "block_2", "label": "Battle_FormationData2", "start": 0x2842BC, "end": 0x284713, "id_base": 0x080},
    {"name": "block_3", "label": "Battle_FormationData3", "start": 0x28471C, "end": 0x284B7E, "id_base": 0x100},
    {"name": "block_4", "label": "Battle_FormationData4", "start": 0x284B8C, "end": 0x284F7C, "id_base": 0x180},
]
BOSS_FORMATION_BLOCK: dict[str, Any] = {
    "name": "boss", "label": "Battle_BossFormationData", "start": 0x284F7C, "end": 0x285012, "id_base": None,
}
FORMATION_INDEX_BLOCK: dict[str, Any] = {
    "name": "formation_indexes", "label": "Battle_FormationIndexes", "start": 0x2836EC, "end": 0x283E67,
}


#: Formation records stored inline in battle-object code, uncompressed, which
#: an object copies over `Enemy_Formation_Data` (`$FFFF41F0`) before it calls
#: `loc_14D46` to rebuild the enemy side mid-battle. The copy takes the whole
#: header too, so `Enemy_Run_Chance` and the drop bytes follow the new record.
#: Each is located by the one `lea (<offset>).l, a0` that loads it.
INLINE_FORMATIONS: list[dict[str, Any]] = [
    # BattleObj_Fusion, frame $14: `lea (loc_1A2F4).l, a0` (ps4.asm:35832),
    # the record at ps4.asm:35846-35847. Ability $12 FUSION (34 ZolSlug).
    {"label": "loc_1A2F4", "offset": 0x1A2F4, "loaded_by": "BattleObj_Fusion"},
    # Object $354 (loc_23C84), first frame: `lea (loc_23D00).l, a0`
    # (ps4.asm:47491), the record at 47505. Abilities $3A/$3B COMBINE
    # (84 BladeRight, 86 HakenLeft).
    {"label": "loc_23D00", "offset": 0x23D00, "loaded_by": "loc_23C84"},
]
#: `lea (abs).l, a0`: opcode 41F9 and a 32-bit address.
LEA_ABS_A0 = b"\x41\xF9"
#: The longest inline record the scan accepts: the header and four enemies.
INLINE_MAX = HEADER_SIZE + 4 * 2 + 1


class FormationError(ValueError):
    pass


def _enemy_symbol(value: int) -> dict[str, Any]:
    """Enemy ids are 0-based: `EnemyID_Helex = 0`, `EnemyID_MonsterFly = 1`."""
    return {
        "id": value,
        "symbol": ENEMY_SYMBOLS[value] if 0 <= value < len(ENEMY_SYMBOLS) else None,
    }


def _item_symbol(value: int) -> dict[str, Any] | None:
    """Item ids are 1-based: `ItemID_Dagger = 1`. 0 means no dropped item."""
    if value == 0:
        return None
    idx = value - 1
    return {
        "id": value,
        "symbol": ITEM_SYMBOLS[idx] if 0 <= idx < len(ITEM_SYMBOLS) else None,
    }


def _mask_slots(mask: int) -> list[int]:
    return [bit + 1 for bit in range(4) if mask & (1 << bit)]


def decompress_block(data: bytes, spec: dict[str, Any]) -> tuple[bytes, dict[str, Any]]:
    """Decompress one annotated blob and verify its length against the ROM map."""
    start, end = spec["start"], spec["end"]
    decompressed, consumed = decompress(data, start)
    if consumed != end - start:
        raise FormationError(
            f"{spec['label']} at 0x{start:06X}: decompressor consumed {consumed} bytes, "
            f"but the documented range is {end - start} bytes"
        )
    compressed = data[start:end]
    source = {
        "label": spec["label"],
        "rom_offset": f"0x{start:06X}",
        "rom_end_exclusive": f"0x{end:06X}",
        "compression": "kosinski",
        "compressed_size": len(compressed),
        "compressed_sha256": hashlib.sha256(compressed).hexdigest(),
        "decompressed_size": len(decompressed),
    }
    return decompressed, source


def parse_formation_record(record: bytes) -> dict[str, Any]:
    """Decode one $FF-terminated formation record."""
    if len(record) < HEADER_SIZE + 3 or record[-1] != TERMINATOR:
        raise FormationError(f"Malformed formation record: {record.hex()}")

    body = record[HEADER_SIZE:-1]
    if len(body) % 2:
        raise FormationError(f"Formation record has a dangling enemy byte: {record.hex()}")

    group_1_mask, group_2_mask = record[5], record[6]
    enemies = []
    for slot, i in enumerate(range(0, len(body), 2), start=1):
        bit = 1 << (slot - 1)
        groups = [n for n, mask in ((1, group_1_mask), (2, group_2_mask)) if mask & bit]
        enemies.append({
            "slot": slot,
            "enemy": _enemy_symbol(body[i]),
            "position": body[i + 1],
            "groups": groups,
        })

    declared_count = record[4]
    return {
        "surprise_agility": record[0],
        "run_agility": record[1],
        "can_run": record[1] < CANNOT_RUN_THRESHOLD,
        "item_drop_rate": record[2],
        "dropped_item": _item_symbol(record[3]),
        "enemy_count": declared_count,
        # Block 3 record $77 declares four enemies but lists three pairs. The
        # disassembly's uncompressed source has the same bytes, so this is a
        # ROM inconsistency, not a decode failure; surface it instead of
        # trusting either number silently.
        "enemy_count_matches_entries": declared_count == len(enemies),
        "group_1_mask": f"0x{group_1_mask:02X}",
        "group_2_mask": f"0x{group_2_mask:02X}",
        "group_1_slots": _mask_slots(group_1_mask),
        "group_2_slots": _mask_slots(group_2_mask),
        "enemies": enemies,
        "raw_hex": record.hex(),
    }


def split_formation_records(block: bytes) -> list[tuple[int, bytes]]:
    """Split a decompressed block into `(offset, record)` pairs on $FF.

    The scan starts past the 7-byte header and steps two bytes at a time, so
    it only ever lands on enemy-id bytes. A $FF used as a position byte would
    therefore not be mistaken for a terminator. `parse_formation_block` checks
    this structural split against the game's own byte-by-byte $FF count.
    """
    records: list[tuple[int, bytes]] = []
    start = 0
    while start < len(block):
        cursor = start + HEADER_SIZE
        while cursor < len(block) and block[cursor] != TERMINATOR:
            cursor += 2
        if cursor >= len(block):
            raise FormationError(
                f"Unterminated formation record at offset 0x{start:X} of a "
                f"{len(block)}-byte block"
            )
        records.append((start, block[start:cursor + 1]))
        start = cursor + 1
    return records


def parse_formation_block(block: bytes, source: dict[str, Any], spec: dict[str, Any]) -> list[dict[str, Any]]:
    records = split_formation_records(block)
    # The game counts $FF bytes one at a time to find record N. If a $FF ever
    # appeared as a header or position byte, that scan and the structural
    # split above would disagree about where records begin.
    terminators = block.count(TERMINATOR)
    if terminators != len(records):
        raise FormationError(
            f"{spec['label']}: structural split found {len(records)} records but "
            f"the block contains {terminators} $FF bytes; the game's terminator "
            "scan would not agree with this parse"
        )

    parsed = []
    for index, (offset, record) in enumerate(records):
        entry: dict[str, Any] = {}
        if spec["id_base"] is not None:
            entry["id"] = spec["id_base"] + index
        else:
            entry["event_battle_index"] = index
        entry.update({
            "block": spec["name"],
            "index_in_block": index,
            "block_offset": f"0x{offset:04X}",
            **parse_formation_record(record),
            "source": source,
        })
        parsed.append(entry)
    return parsed


def extract_inline_formations(data: bytes) -> list[dict[str, Any]]:
    """The formation records battle objects keep in their own code.

    Each record is read from its offset up to the `$FF` terminator with the
    same structural rule as a block record (header, then whole pairs), and is
    accepted only when the ROM holds exactly one `lea (<offset>).l, a0` - the
    load that makes those bytes a formation and not something else.
    """
    parsed = []
    for spec in INLINE_FORMATIONS:
        offset = spec["offset"]
        reference = LEA_ABS_A0 + offset.to_bytes(4, "big")
        loads = data.count(reference)
        if loads != 1:
            raise FormationError(
                f"{spec['label']} at 0x{offset:06X}: expected one `lea` that loads it, "
                f"found {loads}")
        window = data[offset:offset + INLINE_MAX]
        (start, record), = split_formation_records(
            window[:split_end(window, spec)])
        parsed.append({
            "label": spec["label"],
            "rom_offset": f"0x{offset:06X}",
            "loaded_by": spec["loaded_by"],
            **parse_formation_record(record),
        })
    return parsed


def split_end(window: bytes, spec: dict[str, Any]) -> int:
    """One record's length: the header, whole pairs, then the `$FF`."""
    cursor = HEADER_SIZE
    while cursor < len(window) and window[cursor] != TERMINATOR:
        cursor += 2
    if cursor >= len(window):
        raise FormationError(f"{spec['label']}: no terminator within {INLINE_MAX} bytes")
    return cursor + 1


def extract_formations(data: bytes) -> dict[str, Any]:
    blocks = []
    formations = []
    for spec in FORMATION_BLOCKS:
        decompressed, source = decompress_block(data, spec)
        records = parse_formation_block(decompressed, source, spec)
        if len(records) > 0x80:
            raise FormationError(
                f"{spec['label']} holds {len(records)} records; the game's id "
                "dispatch allows at most 0x80 per block"
            )
        formations.extend(records)
        blocks.append({
            "name": spec["name"],
            "id_base": f"0x{spec['id_base']:03X}",
            "record_count": len(records),
            **source,
        })

    boss_block, boss_source = decompress_block(data, BOSS_FORMATION_BLOCK)
    boss_formations = parse_formation_block(boss_block, boss_source, BOSS_FORMATION_BLOCK)

    return {
        "blocks": blocks,
        "boss_block": {
            "name": BOSS_FORMATION_BLOCK["name"],
            "record_count": len(boss_formations),
            **boss_source,
        },
        "total_formations": len(formations),
        "total_boss_formations": len(boss_formations),
        "formations": formations,
        "boss_formations": boss_formations,
        "inline_formations": extract_inline_formations(data),
    }


def extract_formation_indexes(data: bytes, known_ids: set[int] | None = None) -> dict[str, Any]:
    """Decode the encounter tables: 68 groups of 32 big-endian formation ids.

    `Battle_SetupEnemyData` scales the group index by $40 and then picks one of
    32 words at random, so each group is one 64-byte table of candidate
    formations.
    """
    decompressed, source = decompress_block(data, FORMATION_INDEX_BLOCK)
    if len(decompressed) % INDEX_GROUP_SIZE:
        raise FormationError(
            f"Battle_FormationIndexes decompressed to {len(decompressed)} bytes, "
            f"which is not a multiple of the {INDEX_GROUP_SIZE}-byte group size"
        )

    groups = []
    unresolved = set()
    for group_index in range(len(decompressed) // INDEX_GROUP_SIZE):
        offset = group_index * INDEX_GROUP_SIZE
        chunk = decompressed[offset:offset + INDEX_GROUP_SIZE]
        ids = [
            int.from_bytes(chunk[i:i + 2], "big")
            for i in range(0, INDEX_GROUP_SIZE, 2)
        ]
        if known_ids is not None:
            unresolved.update(i for i in ids if i not in known_ids)
        groups.append({
            "group": group_index,
            "block_offset": f"0x{offset:04X}",
            "formation_ids": ids,
            "raw_hex": chunk.hex(),
        })

    if unresolved:
        raise FormationError(
            "Battle_FormationIndexes references formation ids that do not exist: "
            + ", ".join(f"0x{i:03X}" for i in sorted(unresolved))
        )

    return {
        "source": source,
        "group_size_bytes": INDEX_GROUP_SIZE,
        "entries_per_group": INDEX_ENTRIES_PER_GROUP,
        "group_count": len(groups),
        "groups": groups,
    }
