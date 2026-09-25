# Isolated spell fixture. All casts, cancellations and saves use normal input.
extends "res://../tools/native/native_camp_abilities.gd"

func _physics_process(_delta):
    tick += 1
    if held != "":
        Input.action_release(held)
        held = ""
        return false
    if game == null:
        return false
    var raw = game.debug_play_state()
    if raw == "":
        return false
    var state = JSON.parse_string(raw)
    if tick > 6000:
        fail("travel flow timed out", state)
        return false
    if cooldown > 0:
        cooldown -= 1
        return false
    if state.scene or state.dialogue or state.transition or state.title:
        return false
    var camp = state.camp
    if camp == null:
        if phase == 2:
            if int(state.map) != 0x42 or int(state.previous_map) != 0x47 or int(state.cell[0]) != 31 or int(state.cell[1]) != 28:
                fail("HINAS wrong destination", state)
                return false
            record("hinas-arrived", state)
            phase = 3
        elif phase == 6:
            if int(state.map) != 0 or int(state.previous_map) != 0x1D or int(state.cell[0]) != 61 or int(state.cell[1]) != 102:
                fail("RYUKA wrong destination", state)
                return false
            record("ryuka-arrived", state)
            phase = 7
        press("ui_cancel")
        return false
    match phase:
        0:
            baseline = camp.party.duplicate(true)
            phase = 1
        1:
            match camp.mode:
                "Root": choose(camp.root, 1)
                "AbilityCharacters": choose(camp.caster, 1)
                "AbilityList": choose(camp.ability, ability_index(camp, 40))
                "TravelReady":
                    if camp.party[1].tp != 46 or int(state.map) != 0x47:
                        fail("HINAS charge or message timing wrong", state)
                        return false
                    record("hinas-ready", state)
                    phase = 2
                    cooldown = 20
        2:
            if camp.mode == "TravelReady": press("ui_cancel")
        3:
            match camp.mode:
                "Root": choose(camp.root, 1)
                "AbilityCharacters": choose(camp.caster, 1)
                "AbilityList": choose(camp.ability, ability_index(camp, 39))
                "TravelTowns":
                    if camp.party[1].tp != 46:
                        fail("RYUKA browsing spent TP", state)
                        return false
                    record("ryuka-browse", state)
                    phase = 4
                    cooldown = 20
        4:
            if camp.mode == "TravelTowns": press("ui_cancel")
            elif camp.mode == "AbilityList":
                if camp.party[1].tp != 46:
                    fail("RYUKA cancel spent TP", state)
                    return false
                record("ryuka-cancelled", state)
                phase = 5
                cooldown = 15
        5:
            match camp.mode:
                "AbilityList": choose(camp.ability, ability_index(camp, 39))
                "TravelTowns":
                    var target = -1
                    for i in range(camp.towns.size()):
                        if camp.towns[i].name == "MILE": target = i
                    if target < 0:
                        fail("MILE missing from visited list", state)
                        return false
                    choose(camp.town, target)
                "TravelReady":
                    if camp.party[1].tp != 38 or int(state.map) != 0x42:
                        fail("RYUKA charge or message timing wrong", state)
                        return false
                    record("ryuka-ready", state)
                    phase = 6
                    cooldown = 20
        6:
            if camp.mode == "TravelReady": press("ui_accept")
        7:
            match camp.mode:
                "Root": choose(camp.root, 4)
                "State": choose(camp.state, 2)
                "SaveSlots": choose(camp.save, 1)
                "SaveResult":
                    if camp.message != "FILE SAVED":
                        fail("travel save failed", state)
                        return false
                    record("saved", state)
                    phase = 8
                    cooldown = 30
        8:
            var file = FileAccess.open(directory.path_join("receipt.json"), FileAccess.WRITE)
            file.store_string(JSON.stringify({"complete": true, "fixture": true, "observations": observations}, "  "))
            quit(0)
    return false
