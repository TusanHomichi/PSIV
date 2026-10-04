"""Character_Command_Data, as written by the cartridge's own menus.

Command bytes: ATTACK/TECH/SKILL/ITEM/DEFEND = 1..5
(Battle_PickedCommandsOffs and loc_16C6, ps4.asm:2225-2232, 2312-2321).
The current word is copied only after the actor passes its status check
(loc_57C4, ps4.asm:8045-8055). It also identifies actions that draw no RNG.
"""
from .errors import FixtureError

KINDS = {1: "attack", 2: "technique", 3: "skill", 4: "item", 5: "defend"}


def command_entry(log, fighter, frame):
    prefix = f"cmd{fighter - 1}"
    entry = {"id": fighter, "command": "attack"}
    if log.has(prefix + "_index"):
        index = log.num(frame, prefix + "_index")
        if index not in KINDS:
            raise FixtureError(f"fighter {fighter} has unknown command index {index} at f{frame}")
        entry["command"] = KINDS[index]
        if index in (2, 3, 4):
            entry["ability"] = log.num(frame, prefix + "_id")
    if log.has(prefix + "_target"):
        value = log.num(frame, prefix + "_target")
        entry["target"] = value - 0x10000 if value > 0x7FFF else value
    if entry["command"] == "item":
        column = f"menu_item_source_{fighter - 1}"
        if not log.has(column):
            raise FixtureError("item command needs the script's source-slot observation")
        character = log.num(frame, f"menu_party_{fighter - 1}")
        equipment = [log.num(frame, f"menu_equipment_{character}_{i}") for i in range(4)]
        selected = log.num(frame, f"menu_item_cursor_{fighter - 1}")
        # Battle_FillItemList compacts nonzero equipment then inventory cells
        # (ps4.asm:1796-1816). Macro_Item_Slots reserves the displayed ordinal,
        # not a raw cell when inventory has holes (loc_21E4, 3359-3385).
        sources = [{"equipment": i} for i, item in enumerate(equipment) if item]
        sources += [{"inventory": i} for i, item in enumerate(inventory_at(log, frame)) if item]
        if selected >= len(sources):
            raise FixtureError(f"item cursor {selected} is outside the live item list")
        entry["source"] = sources[selected]
    return entry


def action_command(log, actor, frame):
    if actor > 5 or not log.has("current_command"):
        return None
    word = log.num(frame, "current_command")
    return {"command": KINDS.get(word >> 8), "ability": word & 255}


def inventory_at(log, frame):
    if not log.has("menu_inventory_0"):
        return None
    return [log.num(frame, f"menu_inventory_{slot}") for slot in range(40)]
