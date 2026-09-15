# Isolated reordered five-person party, normal combat and camp/status input.
extends "res://../tools/native_camp_abilities.gd"
var battle_recorded := false
var portraits_recorded := 0
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
    if tick > 16000:
        fail("status fixture timed out",state)
        return false
    if cooldown > 0:
        cooldown -= 1
        return false
    if state.battle != null:
        if state.transition: return false
        var battle = state.battle
        if battle.menu != null and battle.ready and not battle_recorded:
            if battle.menu.party.size() != 5:
                fail("five battle members missing",state)
                return false
            battle_recorded = true
            record("five-party-battle",state)
            cooldown = 30
            return false
        if battle.finishing or battle.message == "Victory!" or battle.message.ends_with("LV increased!") or " learned " in battle.message:
            press("ui_accept")
        elif battle.ready:
            if battle.menu != null and battle.menu.page == "Actions": choose(battle.menu.cursor,0)
            else: press("ui_accept")
        return false
    if not battle_recorded or state.scene or state.dialogue or state.transition: return false
    if state.title or state.game_over:
        fail("status fixture was defeated",state)
        return false
    var camp = state.camp
    if camp == null:
        press("ui_cancel")
        return false
    match phase:
        0:
            record("three-digit-summary",state)
            phase = 1
            cooldown = 30
        1:
            match camp.mode:
                "Root": choose(camp.root,4)
                "State": choose(camp.state,0)
                "Status":
                    if portraits_recorded < camp.party.size():
                        if int(camp.status_selection) != portraits_recorded:
                            press("ui_down")
                        else:
                            var member = camp.party[portraits_recorded]
                            if member.profession == "UNKNOWN":
                                fail("STATUS profession missing",state)
                                return false
                            record("status-"+member.name,state)
                            portraits_recorded += 1
                            cooldown = 30
                    else: phase = 2
        2:
            if camp.mode != "Root": press("ui_cancel")
            else: phase = 3
        3:
            match camp.mode:
                "Root": choose(camp.root,4)
                "State": choose(camp.state,2)
                "SaveSlots": choose(camp.save,1)
                "SaveResult":
                    if camp.message != "FILE SAVED":
                        fail("status save failed",state)
                        return false
                    record("saved",state)
                    phase = 4
                    cooldown = 30
        4:
            var file = FileAccess.open(directory.path_join("receipt.json"),FileAccess.WRITE)
            file.store_string(JSON.stringify({"complete":true,"fixture":true,"observations":observations},"  "))
            quit(0)
    return false
