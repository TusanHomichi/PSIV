"""Retail HINAS/RYUKA destinations, FieldRoutine_PlaceName entry records and
the teleport windows' strings.

Coordinates are the original Map_Start values (eight-pixel units), not cells.
The Motavia loop reads 16 entries although only 13 precede the Dezolis table;
retain its three aliases for modified town flags instead of inventing towns.
"""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
import struct

from .text import decode_name, split_terminated

TOWN_TABLES = ((0, 0x6115A, 16), (1, 0x611C2, 9), (2, 0x6120A, 1))
DUNGEON_TABLE = 0x61212
DUNGEON_COUNT = 34
ENTRY_TABLE = 0x664BC
ENTRY_COUNT = 62
# The teleport windows' strings, in the window charset: the town list's label
# (`Win_TelepipePlaceList`, `ps4.asm:124040`, and RYUKA's copy at `:129261`)
# and the three lines `Win_ItemTeleportMsg` and `Win_TechTeleportMsg` pick
# from (`loc_5CB66`, `loc_60B26`). `$FC` is a line break.
MESSAGES = (
    ('towns_label', 0x2AA892),
    ('teleported', 0x2AA896),
    ('cannot_be_used', 0x2AA8BC),
    ('not_enough_tp', 0x2AA8D2),
)
# `loc_2AA8D2` spells TP with the window font's two-tile pair `$7A $79`, which
# the window charset leaves out.
TP_TILES = {0x7A: 'T', 0x79: 'P'}
MESSAGE_LIMIT = 64


def decode_message(rom: bytes, offset: int) -> str:
    """One `$FE`-terminated window string; `$FC` becomes a newline."""
    end = rom.find(0xFE, offset, offset + MESSAGE_LIMIT)
    if end < 0:
        raise ValueError(f'no window string terminator within {MESSAGE_LIMIT} bytes of {offset:#x}')
    text = []
    for byte in rom[offset:end]:
        if byte == 0xFC:
            text.append('\n')
        elif byte in TP_TILES:
            text.append(TP_TILES[byte])
        else:
            text.append(decode_name(bytes([byte])))
    return ''.join(text)


def extract_travel(rom: bytes) -> dict:
    for offset, expected in [
        (0x6115A, '00000010005c011e'),
        (0x611C2, '1800014c004800be'),
        (0x6120A, '2d0000f100800082'),
        (0x61322, '08f80000ec9b6600'),
        (ENTRY_TABLE, '001000000000001d00000101'),
        (ENTRY_TABLE + ENTRY_COUNT * 6, 'ffff'),
    ]:
        wanted = bytes.fromhex(expected)
        if rom[offset:offset + len(wanted)] != wanted:
            raise ValueError(f'travel table guard failed at {offset:#x}')
    names = [decode_name(raw) for _, raw in split_terminated(rom[0x2AB8A2:0x2ABA70], 0xFE)]
    towns = []
    for world, base, count in TOWN_TABLES:
        for index in range(count):
            offset = base + index * 8
            place, _, previous_map, x, y = struct.unpack_from('>BBHHH', rom, offset)
            towns.append({
                'world': world, 'index': index,
                'town_flag': index if world == 0 else (16 + index if world == 1 else 25),
                'place_id': place, 'name': names[place], 'previous_map': previous_map,
                'x': x, 'y': y, 'rom_offset': offset,
            })
    dungeons = []
    for index in range(DUNGEON_COUNT):
        offset = DUNGEON_TABLE + index * 8
        map_id, x, y, facing, alignment = struct.unpack_from('>HHHBB', rom, offset)
        dungeons.append({'index': index, 'map': map_id, 'x': x, 'y': y,
                         'facing': facing, 'alignment': alignment, 'rom_offset': offset})
    entries = []
    for index in range(ENTRY_COUNT):
        offset = ENTRY_TABLE + index * 6
        map_id, previous_map, place, selector = struct.unpack_from('>HHBB', rom, offset)
        entries.append({'map': map_id, 'previous_map': previous_map,
                        'place_id': place, 'name': names[place],
                        'selector': selector, 'rom_offset': offset})
    messages = {key: decode_message(rom, offset) for key, offset in MESSAGES}
    return {'format_version': 2, 'kind': 'field_travel',
            'rom_sha256': hashlib.sha256(rom).hexdigest(),
            'towns': towns, 'dungeons': dungeons, 'entries': entries,
            'messages': messages}


def emit_travel(rom: bytes, directory: Path) -> dict:
    directory.mkdir(parents=True, exist_ok=True)
    payload = (json.dumps(extract_travel(rom), indent=2, sort_keys=True) + '\n').encode()
    (directory / 'travel.json').write_bytes(payload)
    return {'path': 'travel.json', 'sha256': hashlib.sha256(payload).hexdigest()}
