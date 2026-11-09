"""Declared party fixtures and independent stock-capture validation."""
from __future__ import annotations

import hashlib
import pathlib

from oracle.force.script import party_state

ROOT = pathlib.Path(__file__).resolve().parents[2]
# Character_Stats fields, ps4.constants.asm:34-73. These are RAM layout,
# not ability records; all ability ids come from the case's explicit commands.
WORDS = {"profession": 6, "level": 8, "hp": 14, "max_hp": 16,
         "tp": 18, "max_tp": 20, "attack": 36, "defence": 40,
         "mental_defence": 44}
BYTES = {"status": 22, "strength": 24, "mental": 27,
         "agility": 30, "dexterity": 33}


def patches(case: dict, raw: bytes, frame: int) -> list[str]:
    result = []
    if "inventory" in case:
        inventory = case["inventory"]
        if (not isinstance(inventory, list) or len(inventory) > 40
                or any(type(item) is not int or not 0 <= item <= 255
                       for item in inventory)):
            raise ValueError("inventory fixture needs at most 40 item bytes")
        # Inventory, ps4.constants.asm:2381 ($FFFFF410), 40 byte slots.
        contents = bytes(inventory + [0] * (40 - len(inventory)))
        result.append(f"{frame}:FFFFF410:{contents.hex()}")
    if "characters" in case:
        characters = case["characters"]
        if (len(characters) != 3 or len(set(characters)) != 3
                or any(type(c) is not int or not 0 <= c <= 10 for c in characters)):
            raise ValueError("capture needs three distinct character ids 0..10")
        state = bytearray(raw)
        state[0xF40A:0xF40F] = bytes(characters + [255, 255])
        raw = bytes(state)
        result.append(f"{frame}:FFFFF40A:{raw[0xF40A:0xF40F].hex()}")
    party = party_state(raw)
    needed = {slot: {"technique": [], "skill": []} for slot in party}
    for commands in case["rounds"]:
        if set(map(int, commands)) != set(party):
            raise ValueError("case commands must name every occupied party slot")
        for slot, command in commands.items():
            kind = command["command"]
            if kind in ("technique", "skill"):
                ids = needed[int(slot)][kind]
                if command["id"] not in ids:
                    ids.append(command["id"])
    for slot, member in party.items():
        start = 0xF500 + member["character"] * 0x80
        record = bytearray(raw[start:start + 0x80])
        settings = dict(case.get("defaults", {}))
        settings.update(case.get("party", {}).get(str(slot), {}))
        for key, value in settings.items():
            if key not in WORDS and key not in BYTES:
                raise ValueError(f"unknown party field {key}")
            width, offset = (2, WORDS[key]) if key in WORDS else (1, BYTES[key])
            record[offset:offset + width] = value.to_bytes(width, "big")
            if key in ("strength", "mental", "agility", "dexterity"):
                # Each byte stat has base, modified and battle copies. Keep
                # explicit synthetic stat settings together; FillBattleStats
                # copies modified into battle (ps4.asm:11272-11290).
                record[offset:offset + 3] = bytes((value, value, value))
        for kind, offset, count in (("technique", 0x52, 16), ("skill", 0x62, 8)):
            ids = needed[slot][kind]
            if len(ids) > count:
                raise ValueError(f"fighter {slot} needs too many {kind}s: {ids}")
            record[offset:offset + count] = bytes(ids + [0] * (count - len(ids)))
        for index in range(8):
            # Generous explicit resources permit a bounded sequence. The
            # observed start record and round state prove actual payment.
            record[0x6A + 2 * index:0x6C + 2 * index] = bytes((9, 9))
        # The stock host bounds each patch to 64 bytes.
        for offset in (0, 64):
            result.append(f"{frame}:FFFF{start + offset:04X}:{record[offset:offset + 64].hex()}")
    return result


def executed(fixture: dict) -> set[tuple[str, int]]:
    return {(a["kind"], a["ability"]) for r in fixture["rounds"]
            for a in r["actions"] if a["actor"] <= 5
            and a.get("kind") in ("technique", "skill")}


def require_executed(case: dict, fixture: dict) -> None:
    wanted = {(c["command"], c["id"]) for r in case["rounds"] for c in r.values()
              if c["command"] in ("technique", "skill")}
    missing = wanted - executed(fixture)
    if missing:
        raise ValueError(f"selected abilities did not execute: {sorted(missing)}")


def verify_report(report: dict) -> None:
    """A resumed extraction must still have both independent stock runs."""
    for key in ("trace", "log", "rerun_log", "start_ram", "party_script"):
        path = pathlib.Path(report[key])
        if hashlib.sha256(path.read_bytes()).hexdigest() != report[key + "_sha256"]:
            raise ValueError(f"capture {key} hash changed: {path}")
    if report["log_sha256"] != report["rerun_log_sha256"]:
        raise ValueError("stock capture and verification logs differ")
