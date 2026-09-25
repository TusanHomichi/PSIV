# Continue the rescued-Zema campaign, buy equipment with earned meseta,
# equip it through the camp menus, rest, and save outside town.
extends "res://../tools/native/native_motavia.gd"

const PURCHASES = {
    5: {"facing":"ui_left", "items":[[9,160],[9,160],[8,280],[22,1000]]},
    6: {"facing":"ui_right", "items":[[11,550],[12,220],[15,100]]},
}
# Character id, menu slot (head/right/left/body), expected item name.
const EQUIPMENT = [[1,1,"SLASHER"],[1,2,"SLASHER"],[0,1,"STEL-SWORD"],
    [4,1,"BROAD-AXE"],[2,3,"CRBN-SUIT"],[2,2,"CRBNSHIELD"],[2,0,"CIRCLET"]]
const RAW_SLOTS = [2,0,1,3]
var interaction_phase := 0
var purchase := 0
var money_before := 0
var inventory_before := []
var completed_counters := []
var equipped := 0
var outfit_complete := false
var rest_complete := false
var checked_resident_colours := false
var left_town_footprint := false
var hand_cancel_checked := false
var hand_cancel_before := {}

func configure_route():
    route = [[0,99,82,0x24], [0x24,36,31,0x27], [0x27,33,34,0x27],
        [0x27,33,40,0x24], [0x24,26,31,0x26], [0x26,28,32,0x26],
        [0x26,35,32,0x26], [0x26,31,40,0x24], [0x24,31,50,0]]
    start_map = 0
    start_flag = 0x37
    start_label = "CONTINUE-Zema-restored"
    end_label = "Zema-equipped-and-rested"
    boss_check_leg = -1
    healing = false

func before_route(state):
    # Zema's overworld exit lands inside its 2x2 entrance footprint. Retail
    # suppresses a warp between adjacent type-1 cells; leave it before returning.
    if leg == 0 and int(state.map) == 0 and not left_town_footprint:
        if state.cell[1] < 84:
            press("ui_down")
            return true
        left_town_footprint = true
    if int(state.map) == 0x24 and not checked_resident_colours:
        var expected = ["NPCType2_cb8a59c5","NPCType2_8522bac4","NPCType2_95d55f48",
            "NPCType1_0e29dfdf","NPCType2_8522bac4","NPCType2_cb8a59c5","NPCType8_1ad3a6ff"]
        var residents = state.npc_sheets.filter(func(n): return int(n.index) < 7)
        if residents.size() != 7 or residents.any(func(n): return n.sheet != expected[int(n.index)]):
            fail("rescued residents still use their petrified palette",state)
            return true
        checked_resident_colours = true
        checkpoint("resident-colours-restored",state)
        capture.call_deferred("resident-colours-restored.png")
    if leg == 2 and state.cell == [33,34] and not rest_complete:
        rest_input(state)
        return true
    if leg in PURCHASES and not leg in completed_counters and state.cell == [route[leg][1],route[leg][2]]:
        buy_input(state,PURCHASES[leg])
        return true
    if leg == 7 and not outfit_complete:
        equip_input(state)
        return true
    return false

func rest_input(state):
    var shop = state.shop
    if interaction_phase == 0:
        money_before = int(state.money)
        press("ui_up")
        interaction_phase = 1
    elif interaction_phase == 1:
        press("ui_accept")
        interaction_phase = 2
    elif shop == null:
        if interaction_phase != 3:
            fail("Zema inn did not open",state)
            return
        rest_complete = true
        interaction_phase = 0
        checkpoint("Zema-paid-rest",state)
    elif shop.mode == "InnGreeting": press("ui_cancel" if interaction_phase == 3 else "ui_accept")
    elif shop.mode == "InnConfirm": choose(shop.confirm,0)
    elif shop.mode == "Message":
        if int(state.money) != money_before - 80 or state.party_status.any(func(c): return c.hp != c.max_hp or c.status != 0) or state.party_resources.any(func(c): return c.tp != c.max_tp):
            fail("Zema inn did not charge and restore the party correctly",state)
            return
        interaction_phase = 3
        checkpoint("Zema-inn-receipt",state)
        capture.call_deferred("Zema-inn-receipt.png")
        press("ui_accept")
    else: fail("unexpected inn menu",state)
    cooldown = 8

func buy_input(state,plan):
    var shop = state.shop
    if interaction_phase == 0:
        press(plan.facing)
        interaction_phase = 1
    elif interaction_phase == 1:
        press("ui_accept")
        interaction_phase = 2
    elif shop == null:
        if purchase != plan.items.size():
            fail("Zema counter did not complete purchases",state)
            return
        completed_counters.append(leg)
        purchase = 0
        interaction_phase = 0
    elif purchase == plan.items.size(): press("ui_cancel")
    elif shop.mode == "Greeting": press("ui_accept")
    elif shop.mode == "Root": choose(shop.root,0)
    elif shop.mode == "BuyList":
        var item = plan.items[purchase]
        var index = shop.stock.map(func(s): return int(s.id)).find(item[0])
        if index < 0 or int(shop.stock[index].price) != item[1]:
            fail("Zema shop stock differs from its cartridge record",state)
            return
        money_before = int(state.money)
        inventory_before = state.inventory.duplicate()
        choose(shop.item,index)
    elif shop.mode == "BuyConfirm": choose(shop.confirm,0)
    elif shop.mode == "Message":
        var item = plan.items[purchase]
        var expected = inventory_before.duplicate()
        var free_slot = expected.find(0.0)
        if free_slot < 0:
            fail("outfit route ran out of inventory space",state)
            return
        expected[free_slot] = float(item[0])
        if int(state.money) != money_before - item[1] or state.inventory != expected:
            fail("purchase did not deduct the exact price and add one item",state)
            return
        var label = "purchase-%02d-%02d" % [leg,purchase]
        checkpoint(label,state)
        capture.call_deferred(label + ".png")
        purchase += 1
        press("ui_accept")
    else: fail("unexpected shop menu",state)
    cooldown = 8

func equipment_plan():
    return EQUIPMENT

func equip_input(state):
    var camp = state.camp
    var plans = equipment_plan()
    if equipped == plans.size():
        if camp != null: press("ui_cancel")
        else:
            outfit_complete = true
            checkpoint("outfit-complete",state)
        cooldown = 8
        return
    if camp == null:
        press("ui_cancel")
        cooldown = 8
        return
    var plan = plans[equipped]
    var member = camp.party.map(func(c): return int(c.id)).find(plan[0])
    if member < 0:
        fail("equipment character missing",state)
        return
    match camp.mode:
        "Root": choose(camp.root,3)
        "EquipCharacters": choose(camp.equipment_character,member)
        "EquipStats":
            if int(camp.equipment_character) != member: press("ui_cancel")
            elif camp.party[member].equipment[RAW_SLOTS[plan[1]]] == plan[2]:
                var label = "equipped-%02d" % equipped
                checkpoint(label,state)
                capture.call_deferred(label + ".png")
                equipped += 1
            else: choose(camp.equipment_slot,plan[1])
        "EquipItems":
            if not hand_cancel_before.is_empty():
                if state.inventory != hand_cancel_before.inventory or camp.party != hand_cancel_before.party:
                    fail("cancelling the hand selector mutated equipment or inventory",state)
                    return
                hand_cancel_before.clear()
                checkpoint("hand-selection-cancelled",state)
            var index = camp.equipment_options.map(func(i): return i.name).find(plan[2])
            if index < 0:
                fail("purchased equipment not available to its character",state)
                return
            choose(camp.equipment_item,index)
        "EquipResult":
            if not camp.message.begins_with("EQUIPPED ") and not camp.message.begins_with("REMOVED "):
                fail("equipment operation failed",state)
                return
            press("ui_accept")
        "EquipHands":
            if not hand_cancel_checked:
                hand_cancel_before = {"inventory":state.inventory.duplicate(true),"party":camp.party.duplicate(true)}
                hand_cancel_checked = true
                capture.call_deferred("hand-selection.png")
                press("ui_cancel")
            else: choose(camp.equipment_hand,1 if plan[1] == 2 else 0)
        _: fail("unexpected equipment menu",state)
    cooldown = 8
