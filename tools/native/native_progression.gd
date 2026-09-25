# Isolated pre-level fixture; normal attacks, learned-ability messages and SAVE.
extends "res://../tools/native/native_camp_abilities.gd"
var saw_battle := false
var learned := []

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
    if tick > 18000:
        fail("progression fixture timed out",state)
        return false
    if cooldown > 0:
        cooldown -= 1
        return false
    if state.battle != null:
        saw_battle = true
        var battle = state.battle
        if " learned " in battle.message and not battle.message in learned:
            learned.append(battle.message)
            record("learned-%02d" % learned.size(),state)
            cooldown = 20
            return false
        if battle.finishing or battle.message == "Victory!" or battle.message.ends_with("LV increased!") or " learned " in battle.message:
            press("ui_accept")
        elif battle.ready:
            if battle.menu != null and battle.menu.page == "Actions": choose(battle.menu.cursor,0)
            else: press("ui_accept")
        return false
    if not saw_battle or state.scene or state.dialogue or state.transition: return false
    if state.title or state.game_over:
        fail("progression fixture was defeated",state)
        return false
    var camp = state.camp
    if camp == null:
        press("ui_cancel")
        return false
    match phase:
        0:
            if not "Chaz learned TSU!" in learned or not "Hahn learned WAT!" in learned:
                fail("learned-ability messages missing",state)
                return false
            for expected in [[0,4,7],[2,3,4]]:
                var member = state.party_resources.filter(func(p): return int(p.id) == expected[0])[0]
                if int(member.level) != expected[1] or not expected[2] in member.techniques.map(func(id): return int(id)) or int(member.skill_uses[0]) != 1:
                    fail("learned slots, level or spent uses wrong",state)
                    return false
            record("earned-abilities",state)
            phase = 5
        5:
            match camp.mode:
                "Root": choose(camp.root,4)
                "State": choose(camp.state,2)
                "SaveSlots": choose(camp.save,1)
                "SaveResult":
                    if camp.message != "FILE SAVED":
                        fail("progression save failed",state)
                        return false
                    record("saved",state)
                    phase = 6
                    cooldown = 30
        6:
            var file = FileAccess.open(directory.path_join("receipt.json"),FileAccess.WRITE)
            file.store_string(JSON.stringify({"complete":true,"fixture":true,"learned":learned,"observations":observations},"  "))
            quit(0)
    return false
