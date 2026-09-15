# Isolated original Gryz fixture. CRASH/BROSE commands and SAVE use normal input.
extends "res://../tools/native_camp_abilities.gd"
var submitted := 0
var menu_open := false
var selected_skill := -1
var selected_tech := -1
var seen_crash := false
var seen_brose := false
var saw_battle := false
var initial_money := -1
var crash_commands := 0

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
    if initial_money < 0: initial_money = int(state.money)
    if tick > 16000:
        fail("death ability fixture timed out",state)
        return false
    if cooldown > 0:
        cooldown -= 1
        return false
    if state.battle != null:
        saw_battle = true
        var battle = state.battle
        if battle.finishing or battle.message == "Victory!" or battle.message.ends_with("LV increased!"):
            if menu_open:
                submitted += 1
                if selected_skill == 34: crash_commands += 1
                menu_open = false
            press("ui_accept")
            return false
        if battle.message.ends_with(": CRASH") and not seen_crash:
            seen_crash = true
            record("crash",state)
        if battle.message.ends_with(": BROSE") and not seen_brose:
            seen_brose = true
            record("brose",state)
        if battle.menu == null:
            if menu_open:
                submitted += 1
                if selected_skill == 34: crash_commands += 1
                menu_open = false
            if battle.ready: press("ui_accept")
            return false
        if not battle.ready: return false
        var menu = battle.menu
        if not menu_open:
            menu_open = true
            selected_skill = -1
            selected_tech = -1
            if submitted == 0: selected_skill = 34
            elif submitted == 1: selected_tech = 17
            elif menu.skills.any(func(s): return int(s.id) == 34 and s.available): selected_skill = 34
            record("round-%02d" % (submitted+1),state)
        if menu.page == "Actions": choose(menu.cursor,2 if selected_skill >= 0 else (1 if selected_tech >= 0 else 0))
        elif menu.page == "Skills":
            var wanted = menu.skills.map(func(s): return int(s.id)).find(selected_skill)
            if wanted < 0 or not menu.skills[wanted].available:
                fail("CRASH unavailable",state)
                return false
            choose(menu.cursor,wanted)
        elif menu.page == "Techniques":
            var wanted = menu.techniques.map(func(s): return int(s.id)).find(selected_tech)
            if wanted < 0 or not menu.techniques[wanted].available:
                fail("BROSE unavailable",state)
                return false
            choose(menu.cursor,wanted)
        elif menu.page.begins_with("Targets"): press("ui_accept")
        else: fail("unexpected combat menu",state)
        return false
    if menu_open:
        submitted += 1
        if selected_skill == 34: crash_commands += 1
        menu_open = false
    if not saw_battle or state.scene or state.dialogue or state.transition: return false
    if state.title or state.game_over:
        fail("fixture was defeated",state)
        return false
    var camp = state.camp
    if camp == null:
        press("ui_cancel")
        return false
    if camp.party.size() != 1 or int(camp.party[0].id) != 4 or int(camp.party[0].tp) != 4 or int(state.money) != initial_money + 6:
        fail("Gryz resources or victory reward wrong",state)
        return false
    match phase:
        0:
            var gryz = state.party_resources[0]
            if int(gryz.skills[0]) != 34 or int(gryz.skill_uses[0]) != 7 - crash_commands:
                fail("CRASH uses not preserved",state)
                return false
            if not seen_crash or not seen_brose:
                fail("missing ability presentation",state)
                return false
            record("spent-uses",state)
            phase = 2
            cooldown = 20
        2:
            match camp.mode:
                "Root": choose(camp.root,4)
                "State": choose(camp.state,2)
                "SaveSlots": choose(camp.save,1)
                "SaveResult":
                    if camp.message != "FILE SAVED":
                        fail("save failed",state)
                        return false
                    record("saved",state)
                    phase = 3
                    cooldown = 30
        3:
            var file = FileAccess.open(directory.path_join("receipt.json"),FileAccess.WRITE)
            file.store_string(JSON.stringify({"complete":true,"fixture":true,"submitted_rounds":submitted,"observations":observations},"  "))
            quit(0)
    return false
