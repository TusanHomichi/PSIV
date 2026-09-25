# Isolated injured original-record party. All casts, attacks and SAVE use input.
extends "res://../tools/native/native_camp_abilities.gd"
var rounds := 0
var menu_open := false
var saw_battle := false
var initial_tp := -1
var initial_money := -1
var seen := []
var plan := [[34,2,"ANTI"],[35,2,"RIMPA"],[36,3,"REVER"],[37,3,"REGEN"]]

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
    if initial_tp < 0:
        var raja = state.party_resources.filter(func(p): return int(p.id) == 8)
        if raja.is_empty(): return false
        initial_tp = int(raja[0].tp)
        initial_money = int(state.money)
    if tick > 18000:
        fail("battle recovery fixture timed out",state)
        return false
    if cooldown > 0:
        cooldown -= 1
        return false
    if state.battle != null:
        saw_battle = true
        var battle = state.battle
        for ability in plan:
            if battle.message.ends_with(": " + ability[2]) and not ability[0] in seen:
                seen.append(ability[0])
                record(ability[2].to_lower(),state)
                cooldown = 15
                return false
        if battle.menu == null and menu_open:
            rounds += 1
            menu_open = false
        if battle.finishing or battle.message == "Victory!" or battle.message.ends_with("LV increased!") or " learned " in battle.message:
            press("ui_accept")
            return false
        if not battle.ready: return false
        var menu = battle.menu
        if menu == null:
            press("ui_accept")
            return false
        if not menu_open:
            menu_open = true
            record("round-%02d" % (rounds+1),state)
        var casting = rounds < plan.size() and int(menu.character) == 8
        if menu.page == "Actions": choose(menu.cursor,1 if casting else (4 if rounds < plan.size() else 0))
        elif menu.page == "Techniques":
            var wanted = menu.techniques.map(func(t): return int(t.id)).find(plan[rounds][0])
            if wanted < 0 or not menu.techniques[wanted].available:
                fail("recovery technique unavailable",state)
                return false
            choose(menu.cursor,wanted)
        elif menu.page.begins_with("Targets"):
            if casting:
                var wanted = menu.targets.map(func(id): return int(id)).find(plan[rounds][1])
                if wanted < 0:
                    fail("recovery recipient missing",state)
                    return false
                choose(menu.cursor,wanted)
            else: press("ui_accept")
        else: fail("unexpected recovery battle menu",state)
        return false
    if not saw_battle or state.scene or state.dialogue or state.transition: return false
    if state.title or state.game_over:
        fail("recovery fixture was defeated",state)
        return false
    var camp = state.camp
    if camp == null:
        press("ui_cancel")
        return false
    match phase:
        0:
            var raja = state.party_resources.filter(func(p): return int(p.id) == 8)[0]
            var chaz = camp.party.filter(func(p): return int(p.id) == 0)[0]
            var hahn = camp.party.filter(func(p): return int(p.id) == 2)[0]
            if seen.size() != 4 or int(raja.tp) != initial_tp-55 or int(chaz.status) != 0 or int(hahn.status) != 16 or hahn.hp <= 0 or int(state.money) != initial_money+6:
                fail("recovery effects or paid resources wrong",state)
                return false
            record("recovered-party",state)
            phase = 1
            cooldown = 20
        1:
            match camp.mode:
                "Root": choose(camp.root,4)
                "State": choose(camp.state,2)
                "SaveSlots": choose(camp.save,1)
                "SaveResult":
                    if camp.message != "FILE SAVED":
                        fail("recovery save failed",state)
                        return false
                    record("saved",state)
                    phase = 2
                    cooldown = 30
        2:
            var file = FileAccess.open(directory.path_join("receipt.json"),FileAccess.WRITE)
            file.store_string(JSON.stringify({"complete":true,"fixture":true,"casts":seen,"observations":observations},"  "))
            quit(0)
    return false
