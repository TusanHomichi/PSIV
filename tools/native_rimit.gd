# Isolated original five-person records. RIMIT, attacks and SAVE use input.
extends "res://../tools/native_camp_abilities.gd"
var rounds := 0
var menu_open := false
var saw_battle := false
var initial_tp := -1
var initial_money := -1
var seen := []
var saw_sleep := false
var plan := [[23,-1,"RIMIT"]]

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
        fail("RIMIT fixture timed out",state)
        return false
    if cooldown > 0:
        cooldown -= 1
        return false
    if state.battle != null:
        saw_battle = true
        var battle = state.battle
        if battle.message.ends_with(" asleep!") and not saw_sleep:
            saw_sleep = true
            record("enemy-asleep",state)
            cooldown = 15
            return false
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
                fail("RIMIT unavailable",state)
                return false
            choose(menu.cursor,wanted)
        elif menu.page.begins_with("Targets"):
            if casting:
                var wanted = menu.targets.map(func(id): return int(id)).find(plan[rounds][1])
                if wanted < 0:
                    fail("RIMIT recipient missing",state)
                    return false
                choose(menu.cursor,wanted)
            else: press("ui_accept")
        else: fail("unexpected RIMIT battle menu",state)
        return false
    if not saw_battle or state.scene or state.dialogue or state.transition: return false
    if state.title or state.game_over:
        fail("RIMIT fixture was defeated",state)
        return false
    var camp = state.camp
    if camp == null:
        press("ui_cancel")
        return false
    match phase:
        0:
            var raja = state.party_resources.filter(func(p): return int(p.id) == 8)[0]
            if not saw_sleep or seen != [23] or int(raja.tp) != initial_tp-10 or int(state.money) != initial_money+6:
                fail("RIMIT cast or paid resources wrong",state)
                return false
            record("after-RIMIT-victory",state)
            phase = 1
            cooldown = 20
        1:
            match camp.mode:
                "Root": choose(camp.root,4)
                "State": choose(camp.state,2)
                "SaveSlots": choose(camp.save,1)
                "SaveResult":
                    if camp.message != "FILE SAVED":
                        fail("RIMIT save failed",state)
                        return false
                    record("saved",state)
                    phase = 2
                    cooldown = 30
        2:
            var file = FileAccess.open(directory.path_join("receipt.json"),FileAccess.WRITE)
            file.store_string(JSON.stringify({"complete":true,"fixture":true,"casts":seen,"observations":observations},"  "))
            quit(0)
    return false
