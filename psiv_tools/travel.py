"""Retail HINAS/RYUKA destinations and FieldRoutine_PlaceName entry records.

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
    return {'format_version': 1, 'kind': 'field_travel',
            'rom_sha256': hashlib.sha256(rom).hexdigest(),
            'towns': towns, 'dungeons': dungeons, 'entries': entries}


def emit_travel(rom: bytes, directory: Path) -> dict:
    directory.mkdir(parents=True, exist_ok=True)
    payload = (json.dumps(extract_travel(rom), indent=2, sort_keys=True) + '\n').encode()
    (directory / 'travel.json').write_bytes(payload)
    return {'path': 'travel.json', 'sha256': hashlib.sha256(payload).hexdigest()}
