"""Read the cartridge's live list and cursor; expand a command to joypad presses.

Battle_CharCommand/Battle_PickedCommandsOffs (ps4.asm:2190-2225) are horizontal.
Battle_TechWindow/SkillWindow/ItemWindow (2593, 2933, 3325) use four-row pages:
Up/Down wrap inside a page (Battle_UpdateRedCursor2, 1572-1603), Left/Right
change page. Battle_PickTargetEnemy/Char (1216, 1337) use Left/Right over occupied
objects, not over record ids (Battle_UpdateCursor, 70646-70667).
"""
from __future__ import annotations

import pathlib
import json

from .errors import ForceError
from .script import Command
from .tape import PRESS_FRAMES, RELEASE_FRAMES, Step

COMMAND_INDEX = {"attack": 0, "technique": 1, "skill": 2, "item": 3, "defend": 4}
LIST_MENU = {8: ("technique", 0x412E, 16, 1, 0x41AE),
             0x11: ("skill", 0x4142, 8, 2, 0x41BA),
             0x17: ("item", 0x4152, 44, 1, 0x41C4)}
READY = {2, 5, *LIST_MENU, 0xC, 0xD, 0x1F, 0x22, 0x24, 0x26, 0x28, 0x2A, 0x2D, 0x2F}
CONFIRM = {0x1F, 0x20, 0x22, 0x24, 0x26, 0x28, 0x2A, 0x2D, 0x2F}


def confirmation_ready(routine: int) -> bool:
    """Confirm text/results in either initialized state, never a busy list.

    RunBattleRoutines2 masks bit 15 (ps4.asm:1123-1134); results sets it on
    initialization and continues through that state (5894-5908).
    """
    return routine & 0x7FFF in CONFIRM


def extra_fields() -> list[tuple[str, int, int]]:
    """Addresses are RAM layout, not ROM records (constants:2003-2034, 2099)."""
    fields = []
    for slot in range(5):
        fields.append((f"menu_command_{slot}", 0x41A2 + slot * 2, 2))
        fields.append((f"menu_item_source_{slot}", 0xEE6A + slot, 1))
        for kind, _, _, _, address in LIST_MENU.values():
            fields.append((f"menu_{kind}_cursor_{slot}", address + slot * 2, 2))
    for kind, address, count, stride, _ in LIST_MENU.values():
        fields.extend((f"menu_{kind}_{i}", address + i * stride, 1) for i in range(count))
    # Fighter objects are $40 bytes apart; stats_addr is +4 (constants:98-109).
    fields.extend((f"menu_object_{i + 1}", 0x4400 + i * 0x40, 2) for i in range(9))
    fields.extend((f"menu_object_x_{i + 1}", 0x440C + i * 0x40, 2) for i in range(9))
    fields.append(("menu_target_x", 0xD06C, 2))
    fields.extend((f"menu_party_{i}", 0xF40A + i, 1) for i in range(5))
    for character in range(11):
        fields.extend((f"menu_equipment_{character}_{i}", 0xF54C + character * 0x80 + i, 1)
                      for i in range(4))
    # The independent stock capture compares support effects and resource
    # payment, not only HP. RAM offsets: ps4.constants.asm:34-73.
    for prefix, character in (("alys", 1), ("chaz", 0), ("hahn", 2)):
        start = 0xF500 + character * 0x80
        for name, offset, width in (("str_bat", 0x1A, 1), ("men_bat", 0x1D, 1),
                                    ("dex_bat", 0x23, 1), ("atk_bat", 0x26, 2),
                                    ("dfs_bat", 0x2A, 2), ("mdfs_bat", 0x2E, 2)):
            fields.append((f"{prefix}_player_{name}", start + offset, width))
        for index in range(14):
            fields.append((f"{prefix}_player_element_{index}", start + 0x30 + 2 * index, 1))
            fields.append((f"{prefix}_player_shadow_{index}", start + 0x31 + 2 * index, 1))
        for index in range(8):
            fields.append((f"{prefix}_player_use_{index}", start + 0x6A + 2 * index, 1))
    for slot in range(4):
        start = 0x4200 + slot * 0x80
        for name, offset in (("atk_bat", 0x26), ("dfs_bat", 0x2A), ("mdfs_bat", 0x2E)):
            fields.append((f"e{slot + 1}_{name}", start + offset, 2))
    fields.extend((f"menu_inventory_{i}", 0xF410 + i, 1) for i in range(40))
    return fields


def party_address(name: str, address: int, party: dict | None) -> int:
    """Legacy CSV prefixes name fighter slots, not a substituted character.

    Keep the extractor's existing three-slot schema while observing the actual
    selected records. Equipment/menu fields retain their literal addresses.
    """
    if party is None:
        return address
    for prefix, slot, original in (("alys", 1, 1), ("chaz", 2, 0), ("hahn", 3, 2)):
        start = 0xF500 + original * 0x80
        if name.startswith(prefix + "_") and start <= address < start + 0x80:
            return address + (party[slot]["character"] - original) * 0x80
    return address


def write_map(base: pathlib.Path, out: pathlib.Path, party: dict | None = None) -> pathlib.Path:
    from .runs import GROUPS
    # The host counts disabled fields too. A battle script needs only the
    # capture's enabled groups; including the full field-object map exhausts
    # its bounded field table when support-state observations are added.
    groups = set(GROUPS.split(","))
    if party is not None and set(party) != {1, 2, 3}:
        raise ForceError("script capture's observed schema needs exactly three occupied slots")
    lines = []
    for line in base.read_text().splitlines():
        if not line or line.startswith("#"):
            lines.append(line)
            continue
        cells = line.split("\t")
        if cells[3] not in groups:
            continue
        address = party_address(cells[0], int(cells[1], 16) & 0xFFFF, party)
        cells[1] = f"FFFF{address:04X}"
        lines.append("\t".join(cells))
    out.write_text("\n".join(lines) + "\n" + "".join(
        f"{name}\tFFFF{party_address(name, address, party):04X}\t{size}\tmenu\t-\n"
        for name, address, size in extra_fields()))
    fields = [field for field in json.loads(base.with_suffix(".json").read_text())["fields"]
              if field["group"] in groups]
    fields += [{"name": name, "addr": f"FFFF{address:04X}", "size": size,
                "group": "menu", "hex": False}
               for name, address, size in extra_fields()]
    for field in fields:
        address = int(field["addr"], 16) & 0xFFFF
        field["addr"] = f"FFFF{party_address(field['name'], address, party):04X}"
    out.with_suffix(".json").write_text(json.dumps({"fields": fields}, indent=2) + "\n")
    return out


def presses(buttons: list[str]) -> list[Step]:
    return [step for button in buttons for step in
            (Step(PRESS_FRAMES, button), Step(RELEASE_FRAMES, "."))]


def horizontal(current: int, wanted: int, count: int) -> list[str]:
    right = (wanted - current) % count
    left = (current - wanted) % count
    return ["R"] * right if right <= left else ["L"] * left


def list_buttons(current: int, wanted: int) -> list[str]:
    """Change one page at a time: page transitions animate before accepting input."""
    if current // 4 != wanted // 4:
        return ["R" if current // 4 < wanted // 4 else "L"]
    down = (wanted - current) % 4
    up = (current - wanted) % 4
    return (["D"] * down if down <= up else ["U"] * up) + ["C"]


def command_buttons(row: dict, command: Command) -> list[str]:
    routine = int(row["battle_routine_2"], 16)
    slot = int(row["battle_total_comd"])
    if routine == 5:
        current = int(row[f"menu_command_{slot}"])
        return horizontal(current, COMMAND_INDEX[command.kind], 5) + ["C"]
    if routine in LIST_MENU:
        kind, _, count, _, _ = LIST_MENU[routine]
        if command.kind != kind:
            raise ForceError(f"entered {kind} window for {command.kind} command")
        values = [int(row[f"menu_{kind}_{i}"]) for i in range(count)]
        if command.id not in values:
            raise ForceError(f"fighter {slot + 1}'s live {kind} list rejects id {command.id}: {values}")
        current = int(row[f"menu_{kind}_cursor_{slot}"])
        return list_buttons(current, values.index(command.id))
    if routine in (0xC, 0xD):
        side = range(6, 10) if routine == 0xC else range(1, 6)
        targets = [i for i in side if int(row[f"menu_object_{i}"])]
        if command.target not in targets:
            raise ForceError(f"live target list {targets} rejects target {command.target}")
        if routine == 0xD:
            # Cursor and body anchors differ. Their left-to-right order is
            # shared, not their exact coordinates: Battle_AllyCursorInitPos
            # (ps4.asm:70625) versus fighter_x_pos (constants:148).
            # Read that order instead of embedding loc_D9A's ROM table.
            spatial = sorted(targets, key=lambda i: int(row[f"menu_object_x_{i}"]))
            return horizontal(int(row["battle_char_index"]),
                              spatial.index(command.target), len(spatial)) + ["C"]
        column = "battle_enemy_index" if routine == 0xC else "battle_char_index"
        return horizontal(int(row[column]), targets.index(command.target), len(targets)) + ["C"]
    raise ForceError(f"not a command menu: ${routine:04X}")


def verify_commands(raw: bytes, commands: dict[int, Command], records: dict,
                    fighters: list[int]) -> None:
    """Read back the selected commands at queue build; never patch a mismatch.

    Character_Command_Data is copied by loc_57C4 (ps4.asm:8045-8055).
    This catches an ignored press or a weapon whose range cannot express the
    requested target, even if the pilot reached the next round normally.
    """
    for fighter in fighters:
        if fighter > 5 or fighter == 0:
            continue
        command = commands[fighter]
        offset = 0x410A + (fighter - 1) * 4
        selected = raw[offset:offset + 4]
        if selected[0] != COMMAND_INDEX[command.kind] + 1:
            raise ForceError(f"fighter {fighter}'s menu selected another command")
        if command.id is not None and selected[1] != command.id:
            raise ForceError(f"fighter {fighter}'s menu selected another record id")
        single = command.kind == "attack"
        if command.kind in ("technique", "skill", "item"):
            record = records[command.kind][command.id]
            range_ = (record["targeting_or_parameter_3"] if command.kind == "item"
                      else record["targeting"]["raw"]) & 15
            single = range_ in (1, 4, 6, 8)
        if single and int.from_bytes(selected[2:], "big", signed=True) != command.target:
            raise ForceError(f"fighter {fighter}'s menu selected another target; "
                             "check the weapon/command range")
