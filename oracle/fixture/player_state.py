"""Attach independently captured party RAM, never reconstruct it from recipes.

The core's existing retail save codec decodes these bytes during replay. This
module verifies the identity, widths and every initial cell the CSV observes.
"""
import hashlib
from psiv_tools.text import decode_name

from .errors import FixtureError


def attach_start_ram(fixture, path, expected):
    raw = path.read_bytes()
    digest = hashlib.sha256(raw).hexdigest()
    if not expected or digest != expected:
        raise FixtureError("stock-host start RAM hash mismatch or missing expected hash")
    if len(raw) != 65536:
        raise FixtureError("stock-host start RAM must contain 65536 bytes")
    # Character_Stats $F500, $80-byte records; current party $F40A.
    cells = {"level": (8, 2), "hp": (14, 2), "max_hp": (16, 2),
             "tp": (18, 2), "max_tp": (20, 2), "status": (22, 1),
             "strength": (26, 1), "mental": (29, 1), "agility": (32, 1),
             "dexterity": (35, 1), "attack": (36, 2), "defence": (40, 2)}
    for member in fixture["party"]:
        character = raw[0xF40A + member["id"] - 1]
        if character > 10:
            raise FixtureError(f"no character in fighter slot {member['id']}")
        start = 0xF500 + character * 0x80
        record = raw[start:start + 0x80]
        for name, (offset, width) in cells.items():
            observed = int.from_bytes(record[offset:offset + width], "big")
            if observed != member[name]:
                raise FixtureError(f"start RAM fighter {member['id']} {name}: {observed} != CSV {member[name]}")
        name = decode_name(record[:6].split(b"\xfe")[0]).upper()
        member.update(character_id=character, name=name, record=list(record))
    fixture["provenance"]["start_ram_sha256"] = digest
    fixture["provenance"]["start_ram_frame"] = fixture["provenance"]["start_frame"]
