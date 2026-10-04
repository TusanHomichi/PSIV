"""Retail RunMapUpdates programs, decoded into the ignored runtime pack.

The dispatcher at $5493A names its own 64-entry bra.w table. Code selects
clocks, CRAM destinations, gates and ROM tables; no palette table is embedded
in this module. See docs/field/MAP_UPDATES.md for the complete source census.
"""

from __future__ import annotations

import hashlib
import re
from typing import Any

from .m68k import DecodeError, bra_target, self_bounded_bra_table, w, l, sw


class MapUpdatesError(DecodeError):
    """The image does not contain the supported retail update mechanism."""


NAMES = (
    "MapUpdate_NoUpdate", "MapUpdate_MotaWaterPal", "MapUpdate_MotaTownsWaterPal",
    "MapUpdate_PiataBasementB2_MonstersPal", "MapUpdate_MotaQuicksandPal",
    "MapUpdate_ZioFortMoveUpdateRate", "MapUpdate_ZioFortTunnels",
    "MapUpdate_ZioFortMoveUpdateRate2", "MapUpdate_CaveWaterPal",
    "MapUpdate_SoldiersTempleWaterPal", "MapUpdate_ValleyMazeWaterPal",
    "MapUpdate_StripClubLightsPal", "MapUpdate_DezoSnowstormPal",
    "MapUpdate_BioPlantMoveUpdateRate", "MapUpdate_BioPlantPal",
    "MapUpdate_MachCntrMoveUpdateRate", "MapUpdate_PlateSysMoveUpdateRate",
    "MapUpdate_VahFortConveyorBelts", "MapUpdate_WpnPlntConveyorBelts",
    "MapUpdate_WreckagePal", "MapUpdate_WreckagePal2", "MapUpdate_MachCenterPal",
    "loc_55320", "MapUpdate_PlateSysF1Pal", "MapUpdate_PlateSysF1Pal2",
    "loc_55484", "MapUpdate_ClimCenterPal", "MapUpdate_ZelanF1Pal",
    "MapUpdate_NurvusB1Pal", "MapUpdate_NurvusB1Pal2", "MapUpdate_AirCastlePal",
    "MapUpdate_AirCastlePal2", "MapUpdate_WreckageEnginePal", "MapUpdate_KuranF2Pal",
    "MapUpdate_GaruberkTowerPart4", "MapUpdate_GaruberkTowerPal",
    "MapUpdate_GaruberkTowerPal2", "loc_55972", "MapUpdate_GaruberkTower",
    "MapUpdate_PlateSystemPal", "MapUpdate_NurvusPal", "MapUpdate_NurvusPal2",
    "MapUpdate_RykCrystalsPal", "MapUpdate_ZelanKuranSpacePal",
    "MapUpdate_LeRoofRoomPal", "MapUpdate_ZelanCanceller", "MapUpdate_TheEdgePalLine1",
    "MapUpdate_TheEdgePalLine2", "MapUpdate_TheEdgePart2_PalLine2", "loc_55D1E",
    "loc_55D78", "MapUpdate_StrengthTwChests", "MapUpdate_CourageTwChests",
    "MapUpdate_VahFortPal", "MapUpdate_VahFortPal2", "MapUpdate_LadeaTwAirCstlMusic",
    "MapUpdate_ClrChestFlag", "MapUpdate_RandomBattlesFlag", "MapUpdate_NoUpdate2",
    "MapUpdate_NoUpdate3", "MapUpdate_NoUpdate4", "MapUpdate_NoUpdate5",
    "MapUpdate_NoUpdate6", "MapUpdate_NoUpdate7",
)


def dispatch_table(rom: bytes) -> int:
    """Resolve the table from RunMapUpdates, not the fork's build addresses."""
    pattern = re.compile(rb"\x43\xf9(....)\x4e\xb1\x00\x00\x66.\x60\x00", re.S)
    hits = list(pattern.finditer(rom))
    if len(hits) != 1:
        raise MapUpdatesError(f"RunMapUpdates dispatcher occurs {len(hits)} times, not once")
    table = int.from_bytes(hits[0][1], "big")
    if self_bounded_bra_table(rom, table, "MapUpdateJmpTbl") != len(NAMES):
        raise MapUpdatesError("MapUpdateJmpTbl does not have 64 entries")
    return table


def _find(code: bytes, opcode: int) -> list[int]:
    return [at for at in range(0, len(code) - 1, 2) if w(code, at) == opcode]


def _one(code: bytes, opcode: int) -> int:
    hits = _find(code, opcode)
    if len(hits) != 1:
        raise MapUpdatesError(f"expected one opcode {opcode:#06x}, found {len(hits)}")
    return hits[0]


def _writes(rom: bytes, table: int, reads: list[tuple[int, int]], length: int,
            stride: int = 2) -> list[list[dict[str, int]]]:
    end = table + (length - 1) * stride + max(src for src, _ in reads) + 2
    if not 0 <= table < end <= len(rom):
        raise MapUpdatesError("palette reads run outside the ROM")
    return [[{"slot": dest, "word": w(rom, table + step * stride + src)}
             for src, dest in reads] for step in range(length)]


def _gates(rom: bytes, code: bytes, address: int) -> tuple[list[dict], int | None]:
    # Immediate flag id followed by jsr absolute.l or bsr.w. The next branch
    # encodes whether set or clear proceeds. $F120 is extended events/chests;
    # the fork's ChestFlags label does NOT mean its $F140 temporary door.
    gates, bypass = [], None
    for at in range(0, len(code) - 11, 2):
        if w(code, at) not in (0x303C, 0x103C):
            continue
        call = w(code, at + 4)
        if call == 0x4EB9:
            target, branch = l(code, at + 6), at + 10
        elif call == 0x6100:
            target, branch = address + at + 6 + sw(code, at + 6), at + 8
        else:
            continue
        if target not in (0x57624, 0x5762E, 0x57638):
            continue
        flag = w(code, at + 2)
        bank = {0x57624: "event", 0x5762E: "chest", 0x57638: "temp"}[target]
        if bank == "event" and at == 0 and code[branch] == 0x66 and flag == 0xDA:
            bypass = flag  # Reunion branches past the shutdown gates.
        else:
            gates.append({"bank": bank, "flag": flag, "set": code[branch] == 0x67})
    return gates, bypass


def _palette(rom: bytes, code: bytes, address: int, index: int) -> dict[str, Any]:
    bases = _find(code, 0x43F8)
    if not bases or w(code, bases[0] + 2) not in (0xFB00, 0xFB20):
        raise MapUpdatesError(f"update {index:#x} has no CRAM base")
    base = (w(code, bases[0] + 2) - 0xFB00) // 2
    masks = [(w(code, at + 2), at) for at in range(0, len(code) - 3, 2)
             if w(code, at) in (0x0240, 0x0241)]
    if not masks:
        raise MapUpdatesError("palette clock has no mask")
    frame_mask = masks[0][0]
    gates, bypass = _gates(rom, code, address)
    fixed = [{"slot": base + w(code, at + 4) // 2, "word": w(code, at + 2)}
             for at in _find(code, 0x337C)]
    cycles = []
    compares = _find(code, 0x0C38)
    if compares:
        for compare in compares:
            length, counter = w(code, compare + 2), w(code, compare + 4) - 0xECF2
            if counter not in (0, 1) or not 0 < length <= 32:
                raise MapUpdatesError("unknown palette counter")
            reg = 3 if index == 0x0E and counter == 1 else 2
            table = l(code, _one(code, 0x41F9 + reg * 0x200) + 2)
            reads = [(w(code, at + 2) & 0xFF, base + w(code, at + 4) // 2)
                     for at in _find(code, 0x3370 + reg)]
            stride = 2
            if index == 0x0B:
                reads = [(0, base + w(code, _one(code, 0x3352) + 2) // 2)]
            if index == 0x2C:
                reads = [(0, base + w(code, _one(code, 0x3341) + 2) // 2)]
                stride = 1
            if not reads:
                raise MapUpdatesError("palette cycle has no writes")
            frames = _writes(rom, table, reads, length, stride)
            if stride == 1:
                frames = [[{"slot": reads[0][1], "word": rom[table + step]}]
                          for step in range(length)]
            cycles.append({"frame_mask": frame_mask, "timer": index == 0x0B,
                           "phase": {"kind": "counter", "slot": counter}, "frames": frames})
    else:
        table = l(code, _one(code, 0x45F9) + 2)
        if len(masks) != 2:
            raise MapUpdatesError("frame-indexed palette has no phase mask")
        phase_mask, at = masks[1]
        shift_word = w(code, at + 4)
        if shift_word & 0xF1FF != 0xE048:
            raise MapUpdatesError("palette phase is not lsr.w immediate")
        shift = (shift_word >> 9) & 7
        reads = [(0, base + w(code, p + 2) // 2) for p in _find(code, 0x3352)]
        reads += [(w(code, p + 2), base + w(code, p + 4) // 2)
                  for p in _find(code, 0x336A)]
        reads += [(w(code, p + 2) & 0xFF, base + w(code, p + 4) // 2)
                  for p in _find(code, 0x3372)]
        # Air Castle $1F masks the already-zero d0; retail never reloads the
        # frame count. The fork's optional fix is deliberately NOT applied.
        length = 1 if index == 0x1F else (phase_mask >> shift) // 2 + 1
        cycles.append({"frame_mask": frame_mask, "timer": False,
                       "phase": {"kind": "frame", "mask": 0 if index == 0x1F else phase_mask,
                                 "shift": shift + 1},
                       "frames": _writes(rom, table, reads, length)})
    return {"kind": "palette", "gates": gates, "bypass": bypass,
            "cycles": cycles, "stopped": fixed}


def _program(rom: bytes, code: bytes, address: int, index: int) -> dict[str, Any]:
    if index == 0 or index >= 0x3A:
        if code != bytes.fromhex("70004e75"):
            raise MapUpdatesError("no-update routine is no longer a no-op")
        return {"kind": "noop"}
    if index in (0x11, 0x12, 0x22):
        return {"kind": "unsupported", "missing": (
            "live Map_Layout_FG byte buffer, conveyor chunk definitions, plane refresh/DMA"
            if index in (0x11, 0x12) else "shared RAM_Start pattern staging buffer and tile DMA"
        )}
    if index in (5, 7, 0x0D, 0x0F, 0x10, 0x25):
        shifts = [(w(code, at) >> 9) & 7 for at in range(0, len(code) - 1, 2)
                  if w(code, at) & 0xF1FF == 0xE080]
        if len(shifts) != 2:
            raise MapUpdatesError("camera rate routine has no two arithmetic shifts")
        return {"kind": "camera", "foreground": index == 0x25,
                "x_shift": shifts[0], "y_shift": shifts[1]}
    if index == 6:
        return {"kind": "tunnels"}
    if index in (0x26, 0x31, 0x32):
        call = next(at for at in _find(code, 0x4EB9) if l(code, at + 2) == 0x423B4)
        routine = l(code, call + 2)
        # move.b (d8,pc,d0.w),d0 at the positive sine door.
        if w(rom, routine + 4) != 0x103B:
            raise MapUpdatesError("sine helper has changed")
        table = routine + 6 + (w(rom, routine + 6) & 0xFF)
        positive = list(rom[table:table + 128])
        samples = positive + [(-positive[255 - i]) & 0xFF for i in range(128, 256)]
        return {"kind": "scroll", "mode": {0x26: "unsigned", 0x31: "signed", 0x32: "vertical"}[index],
                "samples": samples}
    if index == 0x2A:
        table = l(code, _one(code, 0x45F9) + 2)
        base_write = _find(code, 0x31FC)[0]
        return {"kind": "crystals", "colors": [w(rom, table + i * 2) for i in range(4)],
                "rest": w(code, base_write + 2)}
    if index in (0x2D, 0x33, 0x34, 0x38):
        ids = [w(code, at + 2) for at in _find(code, 0x103C)]
        return {"kind": "flags", "chests": ids[:-1] if index != 0x38 else [],
                "flag": ids[-1], "clear_chest": index == 0x38}
    if index == 0x2E:
        table = l(code, _one(code, 0x45F9) + 2)
        return {"kind": "edge", "colors": [w(rom, table + i * 2) for i in range(5)]}
    if index in (0x2F, 0x30):
        return {"kind": "edge_line", "sliding": index == 0x30, "frames": []}
    if index == 0x37:
        return {"kind": "music", "secondary_map": w(code, _one(code, 0x31FC) + 2),
                "sound": w(code, _one(code, 0x11FC) + 2)}
    if index == 0x39:
        return {"kind": "encounters", "y_max": w(code, _one(code, 0x0C40) + 2)}
    return _palette(rom, code, address, index)


def extract_map_updates(rom: bytes, records: list[dict]) -> dict[str, Any]:
    """Decode all 64 routines and bind ordered programs to every real map."""
    table = dispatch_table(rom)
    addresses = [bra_target(rom, table + i * 4) for i in range(len(NAMES))]
    if addresses != sorted(set(addresses)):
        raise MapUpdatesError("retail routine boundaries are not strictly increasing")
    catalog = []
    for index, address in enumerate(addresses):
        end = addresses[index + 1] if index < 63 else address + 4
        region = rom[address:end]
        returns = [at for at in range(0, len(region) - 3, 2)
                   if region[at:at + 4] == bytes.fromhex("70004e75")]
        if not returns:
            raise MapUpdatesError(f"update {index:#x} has no retail zero return")
        code = region[:returns[-1] + 4]
        program = _program(rom, code, address, index)
        catalog.append({"index": index, "routine": NAMES[index],
                        "address": f"0x{address:06X}", "program": program,
                        "code_sha256": hashlib.sha256(code).hexdigest()})
    per_map, uses = {}, {i: [] for i in range(64)}
    for record in records:
        if record["is_null"]:
            continue
        entries = []
        for index in record["map_updates"]["ids"]:
            if not 0 <= index < 64:
                raise MapUpdatesError(f"map {record['id']}: update {index} out of bounds")
            entry = {**catalog[index], "program": dict(catalog[index]["program"])}
            if index in (0x2F, 0x30):
                # Map_Palettes_Addr points to a long containing the blob address.
                table_at = int(record["palette"]["pointer"], 16) + 0x60
                length, stride = (14, 2) if index == 0x30 else (28, 28)
                entry["program"]["frames"] = [
                    [{"slot": 0, "word": w(rom, table_at + step * stride)}]
                    + [{"slot": 16 + color, "word": w(rom, table_at + step * stride + color * 2)}
                       for color in range(14)] for step in range(length)]
            entries.append(entry)
            uses[index].append(record["id"])
        per_map[record["id"]] = entries
    return {"per_map": per_map, "census": {"jump_table": f"0x{table:06X}",
            "routine_count": 64, "map_count": len(per_map),
            "routines": [{**entry, "maps": uses[entry["index"]]} for entry in catalog]}}
