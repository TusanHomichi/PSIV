# Isolated pipe fixture. Only ordinary ITEM, cancel, destination and SAVE input.
extends "res://../tools/native/native_camp_abilities.gd"
var initial_inventory := []

func inventory_is(state, ids):
    var expected = ids.duplicate()
    expected.resize(40)
    expected = expected.map(func(id): return 0 if id == null else int(id))
    return state.inventory.map(func(id): return int(id)) == expected

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
    if tick > 6000:
        fail("pipe flow timed out", state)
        return false
    if cooldown > 0:
        cooldown -= 1
        return false
    if state.scene or state.dialogue or state.transition or state.title: return false
    var camp = state.camp
    if camp == null:
        if phase == 4:
            if int(state.map) != 0x42 or int(state.previous_map) != 0x47 or state.cell.map(func(v): return int(v)) != [31,28]:
                fail("ESCAPIPE wrong destination",state)
                return false
            record("escapipe-arrived",state)
            phase = 5
        elif phase == 8:
            if int(state.map) != 0 or int(state.previous_map) != 0x1D or state.cell.map(func(v): return int(v)) != [61,102]:
                fail("TELEPIPE wrong destination",state)
                return false
            record("telepipe-arrived",state)
            phase = 9
        press("ui_cancel")
        return false
    if phase > 0 and camp.party != baseline:
        fail("pipes changed party HP, TP or status",state)
        return false
    match phase:
        0:
            baseline = camp.party.duplicate(true)
            initial_inventory = state.inventory.duplicate()
            phase = 1
        1:
            match camp.mode:
                "Root": choose(camp.root,0)
                "ItemList": choose(camp.item,3)
                "ItemResult":
                    if camp.message != "CANNOT TELEPORT HERE" or state.inventory != initial_inventory:
                        fail("blocked TELEPIPE was consumed or misreported",state)
                        return false
                    record("telepipe-blocked",state)
                    phase = 2
                    cooldown = 20
        2:
            if camp.mode != "Root": press("ui_cancel")
            else: phase = 3
        3:
            match camp.mode:
                "Root": choose(camp.root,0)
                "ItemList": choose(camp.item,2)
                "TravelReady":
                    if not inventory_is(state,[125,132,132,126]) or int(state.map) != 0x47:
                        fail("ESCAPIPE consumption or message timing wrong",state)
                        return false
                    record("escapipe-ready",state)
                    phase = 4
                    cooldown = 20
        4:
            if camp.mode == "TravelReady": press("ui_cancel")
        5:
            match camp.mode:
                "Root": choose(camp.root,0)
                "ItemList": choose(camp.item,2)
                "TravelTowns":
                    if not inventory_is(state,[125,132,132,126]):
                        fail("TELEPIPE browsing consumed an item",state)
                        return false
                    record("telepipe-browse",state)
                    phase = 6
                    cooldown = 20
        6:
            if camp.mode == "TravelTowns": press("ui_cancel")
            elif camp.mode == "ItemList":
                if not inventory_is(state,[125,132,132,126]):
                    fail("TELEPIPE cancel consumed an item",state)
                    return false
                record("telepipe-cancelled",state)
                phase = 7
                cooldown = 20
        7:
            match camp.mode:
                "ItemList": choose(camp.item,2)
                "TravelTowns":
                    var wanted = camp.towns.map(func(t): return t.name).find("MILE")
                    if wanted < 0:
                        fail("MILE missing",state)
                        return false
                    choose(camp.town,wanted)
                "TravelReady":
                    if not inventory_is(state,[125,132,126]) or int(state.map) != 0x42:
                        fail("TELEPIPE removed the wrong duplicate or travelled before acknowledgement",state)
                        return false
                    record("telepipe-ready",state)
                    phase = 8
                    cooldown = 20
        8:
            if camp.mode == "TravelReady": press("ui_accept")
        9:
            match camp.mode:
                "Root": choose(camp.root,4)
                "State": choose(camp.state,2)
                "SaveSlots": choose(camp.save,1)
                "SaveResult":
                    if camp.message != "FILE SAVED":
                        fail("pipe save failed",state)
                        return false
                    record("saved",state)
                    phase = 10
                    cooldown = 30
        10:
            var file = FileAccess.open(directory.path_join("receipt.json"),FileAccess.WRITE)
            file.store_string(JSON.stringify({"complete":true,"fixture":true,"observations":observations},"  "))
            quit(0)
    return false
