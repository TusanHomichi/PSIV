# Isolated chest fixtures. Read-only probes, ordinary field/menu/save input.
extends "res://../tools/native/native_camp_abilities.gd"
var scenario := OS.get_environment("PSIV_CHEST_SCENARIO")
var initial_inventory := []
var initial_money := 0
var waiting := 0

func _physics_process(_delta):
    tick += 1
    if held != "":
        Input.action_release(held)
        held = ""
        return false
    if game == null: return false
    var raw = game.debug_play_state()
    if raw == "": return false
    var state = JSON.parse_string(raw)
    if tick > 8000:
        fail("chest flow timed out",state)
        return false
    if cooldown > 0:
        cooldown -= 1
        return false
    if state.scene or state.dialogue or state.transition or state.title: return false
    var menu = state.camp
    match phase:
        0:
            initial_inventory = state.inventory.duplicate()
            initial_money = state.money
            record("closed",state)
            press("ui_up") # Face the adjacent solid chest; no movement needed.
            phase = 1
        1:
            press("ui_accept")
            phase = 2
        2:
            if menu == null or menu.mode != "LootMessage": return false
            record("opened",state)
            if scenario == "full":
                if state.inventory != initial_inventory or has_flag(state,264):
                    fail("full chest granted before a decision",state)
                phase = 10
            else:
                if scenario == "meseta":
                    if state.money != initial_money + 100 or state.inventory != initial_inventory:
                        fail("meseta chest grant differs",state)
                elif state.inventory[0] != (129 if scenario == "white" else 125):
                    fail("wrong chest item",state)
                phase = 3
            cooldown = 20
        3:
            if menu != null: press("ui_accept")
            else:
                press("ui_accept")
                phase = 4
        4:
            if menu == null: return false
            if menu.message != "It is already open.":
                fail("opened chest did not report already open",state)
                return false
            record("reopened",state)
            press("ui_accept")
            phase = 20
        10:
            if menu.mode == "LootMessage": press("ui_accept")
            elif menu.mode == "LootItems": choose(menu.item,0)
            elif menu.mode == "LootDiscardConfirm": choose(menu.target,0)
            elif menu.mode == "LootBlocked":
                if state.inventory != initial_inventory: fail("necessary item changed",state)
                record("necessary-item-protected",state)
                press("ui_cancel")
                phase = 11
        11:
            if menu.mode == "LootItems": press("ui_cancel")
            elif menu.mode == "LootReturnConfirm":
                if menu.target == 1: record("return-default-no",state)
                choose(menu.target,0)
                phase = 12 if menu.target == 0 else 11
        12:
            if menu != null: return false
            if state.inventory != initial_inventory or has_flag(state,264):
                fail("return changed inventory or chest flag",state)
                return false
            record("returned-closed",state)
            press("ui_accept")
            phase = 13
        13:
            if menu == null: return false
            if menu.mode == "LootMessage": press("ui_accept")
            elif menu.mode == "LootItems": choose(menu.item,7)
            elif menu.mode == "LootDiscardConfirm":
                choose(menu.target,0)
                if menu.target == 0: phase = 14
        14:
            if menu.mode != "LootMessage": return false
            if state.inventory[0] != 57 or state.inventory[1] != 126 or state.inventory[39] != 141 or not has_flag(state,264):
                fail("discard/compact/append/flag differed",state)
                return false
            record("alshline-procured",state)
            press("ui_accept")
            phase = 15
        15:
            if not has_flag(state,0x32): return false
            record("alshline-scene-completed",state)
            phase = 16
        16:
            if menu == null: press("ui_cancel")
            elif menu.mode == "Root": choose(menu.root,0)
            elif menu.mode == "ItemList":
                if menu.item != 39: choose(menu.item,39)
                else:
                    record("last-inventory-slot-visible",state)
                    press("ui_cancel")
                    phase = 20
        20:
            if menu == null: press("ui_cancel")
            elif menu.mode == "Root": choose(menu.root,4)
            elif menu.mode == "State": choose(menu.state,2)
            elif menu.mode == "SaveSlots": choose(menu.save,1)
            elif menu.mode == "SaveResult":
                if menu.message != "FILE SAVED":
                    fail("save failed",state)
                    return false
                record("saved",state)
                cooldown = 30
                phase = 21
        21:
            var file = FileAccess.open(directory.path_join("receipt.json"),FileAccess.WRITE)
            file.store_string(JSON.stringify({"complete":true,"fixture":true,"scenario":scenario,"observations":observations},"  "))
            quit(0)
    return false

func has_flag(state, id):
    return state.flags.any(func(flag): return int(flag) == id)
