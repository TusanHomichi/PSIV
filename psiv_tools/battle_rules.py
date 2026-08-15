"""What the cartridge's own code says an equipment record means.

`psiv_tools.core` decodes the character and inventory tables; nothing in those
bytes says which slot a type equips to, which types can attack, how equipment
reaches a derived stat, or what byte `$12` is for. Five routines do, and this
module decodes them out of retail rather than transcribing the disassembly.

============================  =========  ==================================
routine                       retail     what is taken from it
============================  =========  ==================================
`InitializeCharStats`         `$044652`  the table address, the record
                                         count, and the record layout --
                                         every field's width and the RAM
                                         slot it lands in, including the
                                         three mirrors (`curr_hp` to
                                         `max_hp`, `curr_tp` to `max_tp`,
                                         skill uses to their maxima)
`UpdateCharModStats`          `$05F754`  the seven derived-stat passes: for
                                         each, the base stat and which item
                                         offsets are summed over the four
                                         equipment slots
`UpdateEquipment`             `$05F674`  which slot each item type equips
                                         to, and which slot it clears
`UpdateCharElems`             `$05FD2A`  what byte `$12` means per slot: a
                                         weapon's attack element, or a
                                         resistance an armour grants
`Battle_AttackCommand`        `$0015FC`  which types can attack at all and
                                         which hit every enemy
============================  =========  ==================================

Every one is located by a signature that must occur exactly as often as retail
contains it -- once, except the equip-list filter and the shield element test,
which retail carries twice each. Nothing is hard-coded that the cartridge
states: the item stride, the `InventoryData` base, the equipment slot count,
the element count, the type bound and both jump-table lengths are read back out
of the instructions that use them, and disagreement with `psiv_tools.core`'s
tables raises.

`psiv_tools.battle_records` builds the pack's two record files on top of this.
"""

from __future__ import annotations

import re
import struct
from typing import Any

from .core import ITEM_TYPES, PROPERTY_NAMES, TABLES


#: How many equipment slots a character has, in the order the record stores
#: them. Confirmed against the `moveq #3` counters in both bonus routines.
EQUIPMENT_SLOTS = ("right_hand", "left_hand", "head", "body")

#: `Character_Stats` struct offsets, from `ps4.constants.asm`. Used to name the
#: fields the record-layout decoder finds; a slot the decoder reports that is
#: not in here raises, so a changed layout cannot be silently mislabelled.
RAM_FIELDS: dict[int, str] = {
    0x06: "profession", 0x08: "level", 0x0A: "experience",
    0x0E: "curr_hp", 0x10: "max_hp", 0x12: "curr_tp", 0x14: "max_tp",
    0x18: "strength", 0x1B: "mental", 0x1E: "agility", 0x21: "dexterity",
    0x4C: "right_hand", 0x4D: "left_hand", 0x4E: "head", 0x4F: "body",
    0x52: "techniques", 0x62: "skills",
    0x6A: "curr_skill_uses", 0x6B: "max_skill_uses",
    0x19: "strength_mod", 0x1C: "mental_mod", 0x1F: "agility_mod",
    0x22: "dexterity_mod", 0x24: "atk_pow", 0x28: "dfs_pow", 0x2C: "magic_dfs",
    0x50: "right_hand_element", 0x51: "left_hand_element",
}
#: The fourteen element properties are words at `$30`..`$4A`; the character
#: record fills their low bytes, one above each.
for _index, _name in enumerate(PROPERTY_NAMES):
    RAM_FIELDS[0x30 + _index * 2] = f"{_name}_prop"
    RAM_FIELDS[0x31 + _index * 2] = f"{_name}_prop+1"

#: Signatures. Each is checked for exactly the number of retail occurrences
#: named beside it, so a wrong build fails rather than decoding garbage.
SIGNATURES: dict[str, tuple[str, int]] = {
    "InitializeCharStats": ("41f8f50043f9", 1),
    "UpdateCharModStats": ("7e0670002f0061", 1),
    "AddItemBonusToCharStats": ("2f0b760345f9", 1),
    "AddItemBonusToCharStats2": ("2f0b780345f9", 1),
    "UpdateEquipment": ("1028000a5340d040d040", 1),
    "EquipListFilter": ("1431100a0c020007", 2),
    "Battle_AttackCommand": ("1029004c67164eb9", 1),
    "Battle_GetItemType": ("4a00670661061028000a4e75", 1),
    "UpdateCharElems": ("7000323c000d343c0030", 1),
    "CharElemsShieldTest": ("0c2a0005000a", 2),
    "CharElemsRightWeapon": ("116a00120050", 1),
    "CharElemsLeftWeapon": ("116a00120051", 1),
    "CharElemsResistance": ("11bc00010030", 1),
}


class BattleRecordError(ValueError):
    pass


# ---------------------------------------------------------------------------
# Reading retail
# ---------------------------------------------------------------------------
def _sites(rom: bytes, label: str) -> list[int]:
    """Every offset of `label`'s signature, checked against its retail count."""
    signature, expected = SIGNATURES[label]
    pattern = bytes.fromhex(signature)
    hits = [match.start() for match in re.finditer(re.escape(pattern), rom)]
    if len(hits) != expected:
        raise BattleRecordError(
            f"{label}: signature {signature} occurs {len(hits)} times, not the "
            f"{expected} retail has"
        )
    return hits


def _site(rom: bytes, label: str) -> int:
    return _sites(rom, label)[0]


def _w(rom: bytes, at: int) -> int:
    return struct.unpack_from(">H", rom, at)[0]


def _sw(rom: bytes, at: int) -> int:
    return struct.unpack_from(">h", rom, at)[0]


def _l(rom: bytes, at: int) -> int:
    return struct.unpack_from(">I", rom, at)[0]


def _expect(rom: bytes, at: int, opcode: int, what: str) -> None:
    if _w(rom, at) != opcode:
        raise BattleRecordError(
            f"0x{at:06X}: expected {what} (0x{opcode:04X}), found 0x{_w(rom, at):04X}"
        )


def _bra_target(rom: bytes, at: int) -> int:
    """The target of a `bra.w`/`jsr`-table entry at `at`."""
    _expect(rom, at, 0x6000, "bra.w")
    return at + 2 + _sw(rom, at + 2)


def _pc_relative_table(rom: bytes, at: int, opcode: int, what: str) -> int:
    """The table a `jmp`/`jsr (d8,PC,d0.w)` at `at` dispatches through."""
    _expect(rom, at, opcode, what)
    extension = _w(rom, at + 2)
    if extension & 0xFF00:
        raise BattleRecordError(
            f"0x{at:06X}: {what} extension 0x{extension:04X} is not the plain "
            "word-indexed form the dispatchers use"
        )
    return at + 2 + (extension & 0xFF)


def _self_bounded_entries(rom: bytes, table: int, what: str) -> int:
    """How many `bra.w` entries a table has, from its own lowest target.

    Every entry jumps forward past the end of the table, so the table ends
    where the nearest target begins. The same trick bounds `AbilityEffectsOffs`
    in `psiv_tools.battle_pack`; here it means neither jump table's length is
    carried as a constant.
    """
    lowest = None
    count = 0
    while lowest is None or table + count * 4 < lowest:
        target = _bra_target(rom, table + count * 4)
        if target <= table + count * 4:
            raise BattleRecordError(
                f"{what}: entry {count} jumps backwards to 0x{target:06X}"
            )
        lowest = target if lowest is None else min(lowest, target)
        count += 1
        if count > 64:
            raise BattleRecordError(f"{what}: no entry bounds the table")
    return count


# ---------------------------------------------------------------------------
# InitializeCharStats: the record layout
# ---------------------------------------------------------------------------
def read_char_init(rom: bytes) -> dict[str, Any]:
    """`InitializeCharStats`, decoded into the record layout it implies.

    The routine is straight-line: a run of `move.X (a1)+, d16(a0)` reads that
    consume the record in order, three `move (a0), (a0)` mirrors that consume
    nothing, and three `dbf` blocks that copy the technique, skill and
    skill-use arrays. Walking it gives the record's size and every field's
    place without transcribing either.
    """
    site = _site(rom, "InitializeCharStats")
    char_stats_ram = 0xFFFF0000 | _w(rom, site + 2)
    table = _l(rom, site + 6)
    if rom[site + 10] != 0x7E:
        raise BattleRecordError(
            f"0x{site + 10:06X}: InitializeCharStats does not set its loop "
            "counter with `moveq #imm, d7`"
        )
    count = rom[site + 11] + 1

    fields: list[dict[str, Any]] = []
    at = site + 12
    consumed = 0
    while True:
        opcode = _w(rom, at)
        if opcode == 0x48E7:  # movem.l: the reads are over
            break
        if opcode in (0x1159, 0x3159, 0x2159):  # move.X (a1)+, d16(a0)
            width = {0x1159: 1, 0x3159: 2, 0x2159: 4}[opcode]
            fields.append(_field(consumed, width, 1, _w(rom, at + 2), None))
            consumed += width
            at += 4
        elif opcode == 0x3168:  # move.w d16(a0), d16(a0): a mirror
            _mirror(fields, _w(rom, at + 2), _w(rom, at + 4))
            at += 6
        elif opcode & 0xFF00 == 0x7C00:  # moveq #imm, d6
            at, consumed = _copy_block(rom, at, fields, consumed)
        else:
            raise BattleRecordError(
                f"0x{at:06X}: unknown opcode 0x{opcode:04X} in InitializeCharStats"
            )

    _expect(rom, at + 4, 0x47E8, "lea equipment(a0), a3")
    _expect(rom, at + 8, 0x4EB9, "jsr UpdateCharModStats")
    mod_stats = _l(rom, at + 10)
    if mod_stats != _site(rom, "UpdateCharModStats"):
        raise BattleRecordError(
            f"InitializeCharStats calls 0x{mod_stats:06X}, not the "
            f"UpdateCharModStats at 0x{_site(rom, 'UpdateCharModStats'):06X}"
        )

    spec = TABLES["characters"]
    if (table, count, consumed) != (spec["offset"], spec["count"], spec["record_size"]):
        raise BattleRecordError(
            f"InitializeCharStats reads {count} records of {consumed} bytes from "
            f"0x{table:06X}; psiv_tools.core has {spec['count']} of "
            f"{spec['record_size']} from 0x{spec['offset']:06X}"
        )
    return {
        "routine": f"0x{site:06X}",
        "table": table,
        "count": count,
        "record_bytes": consumed,
        "character_stats_ram": f"0x{char_stats_ram:08X}",
        "struct_bytes": _struct_bytes(rom, at),
        "mod_stats_routine": f"0x{mod_stats:06X}",
        "fields": fields,
    }


def _field(offset: int, width: int, count: int, ram: int,
           mirrored_to: int | None) -> dict[str, Any]:
    if ram not in RAM_FIELDS:
        raise BattleRecordError(f"no name for Character_Stats offset 0x{ram:02X}")
    return {
        "record_offset": offset,
        "bytes": width * count,
        "width": width,
        "count": count,
        "field": RAM_FIELDS[ram],
        "ram_offset": f"0x{ram:02X}",
        "mirrored_to": None if mirrored_to is None else RAM_FIELDS[mirrored_to],
    }


def _mirror(fields: list[dict[str, Any]], source: int, destination: int) -> None:
    """Record that the field just read is copied straight into another slot."""
    name = RAM_FIELDS.get(source)
    for field in reversed(fields):
        if field["field"] == name:
            field["mirrored_to"] = RAM_FIELDS[destination]
            return
    raise BattleRecordError(
        f"InitializeCharStats mirrors 0x{source:02X}, which no read filled"
    )


def _copy_block(rom: bytes, at: int, fields: list[dict[str, Any]],
                consumed: int) -> tuple[int, int]:
    """One `moveq / lea / move.b (a1)+, (a2)+ / dbf` array copy."""
    iterations = rom[at + 1] + 1
    _expect(rom, at + 2, 0x45E8, "lea d16(a0), a2")
    ram = _w(rom, at + 4)
    _expect(rom, at + 6, 0x14D9, "move.b (a1)+, (a2)+")
    at += 8
    mirrored = None
    if _w(rom, at) == 0x14EA:  # move.b d16(a2), (a2)+: uses into their maxima
        if _sw(rom, at + 2) != -1:
            raise BattleRecordError(
                f"0x{at:06X}: the array mirror reads {_sw(rom, at + 2)}(a2), not -1(a2)"
            )
        mirrored = ram + 1
        at += 4
    _expect(rom, at, 0x51CE, "dbf d6")
    fields.append(_field(consumed, 1, iterations, ram, mirrored))
    return at + 4, consumed + iterations


def _struct_bytes(rom: bytes, at: int) -> int:
    """The `lea $80(a0), a0` that walks to the next character's struct."""
    for probe in range(at, at + 0x40, 2):
        if _w(rom, probe) == 0x41E8 and _w(rom, probe + 4) == 0x51CF:
            return _w(rom, probe + 2)
    raise BattleRecordError(
        f"0x{at:06X}: InitializeCharStats' loop does not end in `lea d16(a0), a0` + `dbf d7`"
    )


# ---------------------------------------------------------------------------
# UpdateCharModStats: the seven derived-stat passes
# ---------------------------------------------------------------------------
def read_mod_stats(rom: bytes) -> dict[str, Any]:
    """`UpdateCharModStats` and the seven handlers its jump table reaches.

    Each handler has the same five- or six-instruction shape, so the base stat
    and the item offsets summed into it are read out of the immediates rather
    than transcribed. Which of the two adder routines a handler tail-calls is
    what says whether the sum is a wrapping byte or a signed word.
    """
    site = _site(rom, "UpdateCharModStats")
    passes_expected = rom[site + 1] + 1
    table = _pc_relative_table(rom, site + 0x16, 0x4EFB, "jmp UpdateCharModStatsJmpTbl")
    count = _self_bounded_entries(rom, table, "UpdateCharModStatsJmpTbl")
    if count != passes_expected:
        raise BattleRecordError(
            f"UpdateCharModStats loops {passes_expected} times over a "
            f"{count}-entry jump table"
        )

    byte_adder = _site(rom, "AddItemBonusToCharStats")
    word_adder = _site(rom, "AddItemBonusToCharStats2")
    inventory, stride, slots = _read_adder(rom, byte_adder, "AddItemBonusToCharStats")
    inventory2, stride2, slots2 = _read_adder(rom, word_adder, "AddItemBonusToCharStats2")
    if (inventory, stride, slots) != (inventory2, stride2, slots2):
        raise BattleRecordError(
            "the two bonus adders disagree about InventoryData: "
            f"0x{inventory:06X}/{stride}/{slots} against 0x{inventory2:06X}/{stride2}/{slots2}"
        )

    passes = [
        _read_mod_pass(rom, _bra_target(rom, table + index * 4), byte_adder, word_adder)
        for index in range(count)
    ]
    spec = TABLES["items"]
    if (inventory, stride) != (spec["offset"], spec["record_size"]):
        raise BattleRecordError(
            f"the bonus adders read {stride}-byte records from 0x{inventory:06X}; "
            f"psiv_tools.core has {spec['record_size']} from 0x{spec['offset']:06X}"
        )
    if slots != len(EQUIPMENT_SLOTS):
        raise BattleRecordError(
            f"the bonus adders walk {slots} equipment slots, not {len(EQUIPMENT_SLOTS)}"
        )
    return {
        "routine": f"0x{site:06X}",
        "jump_table": f"0x{table:06X}",
        "inventory_data": f"0x{inventory:06X}",
        "record_bytes": stride,
        "equipment_slots": slots,
        "adders": {
            "byte": {"label": "AddItemBonusToCharStats", "rom_offset": f"0x{byte_adder:06X}"},
            "word": {"label": "AddItemBonusToCharStats2", "rom_offset": f"0x{word_adder:06X}"},
        },
        "passes": passes,
    }


def _read_adder(rom: bytes, at: int, label: str) -> tuple[int, int, int]:
    """`InventoryData`, the record stride and the slot count, out of one adder."""
    slots = rom[at + 3] + 1
    _expect(rom, at + 4, 0x45F9, f"{label}: lea (InventoryData).l, a2")
    inventory = _l(rom, at + 6)
    for probe in range(at, at + 0x20, 2):
        if _w(rom, probe) & 0xF1FF == 0xC0FC:  # mulu.w #stride, dN
            return inventory, _w(rom, probe + 2), slots
    raise BattleRecordError(f"{label}: no `mulu.w #stride` found")


def _read_mod_pass(rom: bytes, at: int, byte_adder: int,
                   word_adder: int) -> dict[str, Any]:
    """One `ModStatsUpdate_*` handler."""
    start = at
    _expect(rom, at, 0x7000, "moveq #0, d0")
    _expect(rom, at + 2, 0x1028, "move.b d16(a0), d0")
    base = _w(rom, at + 4)
    _expect(rom, at + 6, 0x43E8, "lea d16(a0), a1")
    destination = _w(rom, at + 8)
    _expect(rom, at + 10, 0x323C, "move.w #offset, d1")
    offsets = [_w(rom, at + 12)]
    at += 14
    if _w(rom, at) == 0x343C:  # move.w #offset, d2: a second bonus byte
        offsets.append(_w(rom, at + 2))
        at += 4
    target = _bra_target(rom, at)
    if target == byte_adder:
        arithmetic, width = "add.b", 1
    elif target == word_adder:
        arithmetic, width = "ext.w + add.w", 2
    else:
        raise BattleRecordError(
            f"0x{at:06X}: a derived-stat pass tail-calls 0x{target:06X}, neither adder"
        )
    if width == 1 and len(offsets) != 1 or width == 2 and len(offsets) != 2:
        raise BattleRecordError(
            f"0x{at:06X}: {len(offsets)} item offsets do not match a {arithmetic} pass"
        )
    if base not in RAM_FIELDS or destination not in RAM_FIELDS:
        raise BattleRecordError(
            f"0x{at:06X}: a derived-stat pass names an unknown stat slot"
        )
    return {
        "stat": RAM_FIELDS[destination],
        "base_stat": RAM_FIELDS[base],
        "item_offsets": [f"0x{offset:02X}" for offset in offsets],
        "bonuses": [BONUS_FIELDS[offset] for offset in offsets],
        "result_bytes": width,
        "arithmetic": arithmetic,
        "signed": width == 2,
        "rom_offset": f"0x{start:06X}",
    }


#: The bonus bytes, by record offset. `psiv_tools.core` decodes them under
#: these names and the derived-stat passes are what proves the mapping.
BONUS_FIELDS = {
    0x0B: "strength", 0x0C: "mental", 0x0D: "agility", 0x0E: "dexterity",
    0x0F: "attack", 0x10: "defense", 0x11: "magic_defense",
}


# ---------------------------------------------------------------------------
# UpdateEquipment: which slot a type equips to
# ---------------------------------------------------------------------------
def read_equip_rules(rom: bytes) -> dict[str, Any]:
    """`UpdateEquipment`'s jump table, and the filter that feeds it.

    The filter builds the list of items a character may equip: type at most
    seven, and the character's bit set in the `usable_by` word. The jump table
    then places the chosen item, and each handler says which slot it writes and
    which it clears.
    """
    filters = _sites(rom, "EquipListFilter")
    type_offset = _w(rom, filters[0] + 2) & 0xFF
    max_type = _w(rom, filters[0] + 6)
    usable_by = None
    for site in filters:
        # The signature covers the type read and the `cmpi.b`/`bhi.s` pair; the
        # `usable_by` read is the instruction after them.
        _expect(rom, site + 0x0A, 0x3431, "move.w d8(a1,d1.w), d2")
        offset = _w(rom, site + 0x0C) & 0xFF
        if usable_by is not None and offset != usable_by:
            raise BattleRecordError("the two equip-list filters read different words")
        usable_by = offset

    site = _site(rom, "UpdateEquipment")
    if _w(rom, site + 2) & 0xFF != type_offset:
        raise BattleRecordError(
            "UpdateEquipment and the equip-list filter read different type bytes"
        )
    table = _pc_relative_table(rom, site + 0x16, 0x4EBB, "jsr EquipItemTypeJmpTbl")
    count = _self_bounded_entries(rom, table, "EquipItemTypeJmpTbl")
    if count != max_type:
        raise BattleRecordError(
            f"EquipItemTypeJmpTbl has {count} entries but the equip-list filter "
            f"admits types up to {max_type}"
        )

    handlers = {
        item_type: _read_equip_handler(rom, _bra_target(rom, table + (item_type - 1) * 4))
        for item_type in range(1, count + 1)
    }
    return {
        "routine": f"0x{site:06X}",
        "jump_table": f"0x{table:06X}",
        "filters": [f"0x{offset:06X}" for offset in filters],
        "type_offset": f"0x{type_offset:02X}",
        "usable_by_offset": f"0x{usable_by:02X}",
        "max_equippable_type": max_type,
        "slots": list(EQUIPMENT_SLOTS),
        "handlers": handlers,
    }


def _read_equip_handler(rom: bytes, start: int) -> dict[str, Any]:
    """One `EquipItemType_*` handler, as the slots it writes and clears.

    Only the shield handler branches: it asks what the right hand already holds
    and takes a second arm when that is two-handed. Both arms are walked, so
    the slots the handler can clear include the ones only that arm reaches.
    """
    writes: list[int] = []
    clears: list[int] = []
    conditions: list[int] = []
    pending = [start]
    seen: set[int] = set()
    while pending:
        at = pending.pop()
        if at in seen:
            continue
        seen.add(at)
        while True:
            opcode = _w(rom, at)
            if opcode == 0x4E75:  # rts, and every arm ends in one
                break
            if opcode == 0x1778:  # move.b (xxx).w, d16(a3): place the chosen item
                writes.append(_w(rom, at + 4))
                at += 6
            elif opcode == 0x177C:  # move.b #imm, d16(a3): empty a slot
                if _w(rom, at + 2) != 0:
                    raise BattleRecordError(
                        f"0x{at:06X}: an equip handler writes a non-zero immediate"
                    )
                clears.append(_w(rom, at + 4))
                at += 6
            elif opcode == 0x11E9:  # move.b d16(a1), (xxx).w: remember what was there
                at += 6
            elif opcode == 0x0C42:  # cmpi.w #type, d2 / beq: the two-handed test
                conditions.append(_w(rom, at + 2))
                if rom[at + 4] != 0x67:
                    raise BattleRecordError(
                        f"0x{at + 4:06X}: an equip handler's type test is not a `beq.s`"
                    )
                pending.append(at + 6 + rom[at + 5])
                at += 6
            elif opcode in (0x7000, 0x7400, 0x5340):
                at += 2
            elif opcode in (0x102B, 0xC0FC, 0x1430):
                at += 4
            elif opcode == 0x41F9:
                at += 6
            else:
                raise BattleRecordError(
                    f"0x{at:06X}: unknown opcode 0x{opcode:04X} in an equip handler"
                )
            if at - start > 0x60:
                raise BattleRecordError(f"0x{start:06X}: equip handler does not end")
    if not writes:
        raise BattleRecordError(f"0x{start:06X}: equip handler writes no slot")

    for slot in writes + clears:
        if slot >= len(EQUIPMENT_SLOTS):
            raise BattleRecordError(f"0x{start:06X}: equip handler touches slot {slot}")
    return {
        "rom_offset": f"0x{start:06X}",
        "equips_to": sorted({EQUIPMENT_SLOTS[slot] for slot in writes}),
        "clears": sorted({EQUIPMENT_SLOTS[slot] for slot in clears}),
        "conditional_on_other_hand_type": sorted(set(conditions)),
    }


# ---------------------------------------------------------------------------
# Battle_AttackCommand: which types are weapons
# ---------------------------------------------------------------------------
def read_attack_rules(rom: bytes) -> dict[str, Any]:
    """Which item types can attack, and which hit every enemy at once.

    `Battle_AttackCommand` tests the right hand and then the left: two `cmpi.b`
    against the multi-target types, then a signed branch that separates the
    single-target weapons from everything heavier. Both immediates and the
    bound come from the instructions.
    """
    site = _site(rom, "Battle_AttackCommand")
    _expect(rom, site + 6, 0x4EB9, "jsr Battle_GetItemType")
    get_type = _l(rom, site + 8)
    if get_type != _site(rom, "Battle_GetItemType"):
        raise BattleRecordError(
            f"Battle_AttackCommand calls 0x{get_type:06X}, not Battle_GetItemType"
        )
    _expect(rom, site + 0x0E, 0x0C00, "cmpi.b #type, d0")
    _expect(rom, site + 0x14, 0x0C00, "cmpi.b #type, d0")
    multi = sorted({_w(rom, site + 0x10), _w(rom, site + 0x16)})
    bound = _w(rom, site + 0x16)
    if rom[site + 0x1A] != 0x6D:  # blt.s: below the bound is a single-target weapon
        raise BattleRecordError(
            f"0x{site + 22:06X}: Battle_AttackCommand does not separate weapons with `blt.s`"
        )
    weapons = sorted(set(range(1, bound + 1)))
    if not set(multi) <= set(weapons):
        raise BattleRecordError("a multi-target type is not a weapon type")
    return {
        "routine": f"0x{site:06X}",
        "item_type": {"label": "Battle_GetItemType", "rom_offset": f"0x{get_type:06X}"},
        "weapon_types": weapons,
        "multi_target_types": multi,
        "note": (
            "the right hand is tried first and the left only if the right holds "
            "nothing that can swing; a character holding only shields has no "
            "Attack command at all"
        ),
    }


# ---------------------------------------------------------------------------
# UpdateCharElems: what byte $12 means
# ---------------------------------------------------------------------------
def read_element_rules(rom: bytes) -> dict[str, Any]:
    """`UpdateCharElems`: element byte `$12` read two different ways.

    In a hand, a weapon's `$12` is the attack element and lands in `$50`/`$51`.
    On a shield or on either armour slot it is instead an element the wearer
    becomes resistant to, written as `1` into that element's property slot.
    Whatever the four slots leave at zero falls back to the character record's
    own value.
    """
    site = _site(rom, "UpdateCharElems")
    elements = _w(rom, site + 4) + 1
    props = _w(rom, site + 8)
    shield_type = _w(rom, _sites(rom, "CharElemsShieldTest")[0] + 2)
    type_offset = _w(rom, _sites(rom, "CharElemsShieldTest")[0] + 4)
    right = _site(rom, "CharElemsRightWeapon")
    left = _site(rom, "CharElemsLeftWeapon")
    element_offset = _w(rom, right + 2)
    if _w(rom, left + 2) != element_offset:
        raise BattleRecordError("the two weapon-element writes read different bytes")
    resistance = _site(rom, "CharElemsResistance")
    value = _w(rom, resistance + 2)
    if _w(rom, resistance + 4) != props:
        raise BattleRecordError(
            "the resistance write and the clearing loop use different property bases"
        )
    if elements != len(PROPERTY_NAMES):
        raise BattleRecordError(
            f"UpdateCharElems clears {elements} element properties, not {len(PROPERTY_NAMES)}"
        )
    return {
        "routine": f"0x{site:06X}",
        "element_offset": f"0x{element_offset:02X}",
        "type_offset": f"0x{type_offset:02X}",
        "shield_type": shield_type,
        "elements": elements,
        "property_base": f"0x{props:02X}",
        "property_stride": 2,
        "resistance_value": value,
        "weapon_element_slots": {
            "right_hand": f"0x{_w(rom, right + 4):02X}",
            "left_hand": f"0x{_w(rom, left + 4):02X}",
        },
        "note": (
            "a hand holding anything but a shield sets that hand's attack "
            "element; a shield or either armour slot instead sets the named "
            "element's property to the resistance value. Element slots no item "
            "touched fall back to the character record's own byte."
        ),
    }


def read_rules(rom: bytes) -> dict[str, Any]:
    """Every rule the two files depend on, decoded from retail in one pass."""
    return {
        "equip": read_equip_rules(rom),
        "attack": read_attack_rules(rom),
        "derived_stats": read_mod_stats(rom),
        "elements": read_element_rules(rom),
    }


# ---------------------------------------------------------------------------
# Item types, as the rules describe them
# ---------------------------------------------------------------------------
def type_rules(rules: dict[str, Any]) -> dict[int, dict[str, Any]]:
    """One entry per item type, saying what the cartridge will do with it."""
    equip = rules["equip"]
    attack = rules["attack"]
    out: dict[int, dict[str, Any]] = {}
    for item_type, name in ITEM_TYPES.items():
        if item_type == 0:
            continue
        handler = equip["handlers"].get(item_type)
        entry: dict[str, Any] = {
            "type": item_type,
            "name": name,
            "equippable": handler is not None,
            "is_weapon": item_type in attack["weapon_types"],
            "multi_target": item_type in attack["multi_target_types"],
        }
        if handler is None:
            entry.update({
                "slot": None, "clears": [], "two_handed": False,
                "clears_when_other_hand_holds": [], "handler": None,
            })
        else:
            slots = handler["equips_to"]
            if len(slots) != 1:
                raise BattleRecordError(
                    f"item type {item_type} equips to {slots}, not one slot"
                )
            conditional = handler["conditional_on_other_hand_type"]
            entry.update({
                "slot": slots[0],
                # A conditional clear is the shield displacing a two-handed
                # weapon, which is not the same as a two-handed weapon of its
                # own always taking the other hand.
                "clears": [] if conditional else handler["clears"],
                "two_handed": bool(handler["clears"]) and not conditional,
                "clears_when_other_hand_holds": conditional,
                "handler": handler["rom_offset"],
            })
            if conditional:
                entry["clears_conditionally"] = handler["clears"]
        entry["element_role"] = (
            "attack_element" if entry["is_weapon"]
            else "resistance_granted" if entry["equippable"]
            else None
        )
        out[item_type] = entry
    return out

