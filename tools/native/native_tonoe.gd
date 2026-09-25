# Actual CONTINUE from the native Holt save, paid rest, Rune, the mountain
# pass and Dorin. All changes to the game come through ordinary input.
extends "res://../tools/native/native_motavia.gd"

var inn_phase := 0
var inn_money := 0
var rested := false
var scene_shots := {}

func configure_route():
    route = [
        [0, 61, 100, 0x1D],
        [0x1D, 38, 43, 0x23],
        [0x23, 21, 34, 0x23],
        [0x23, 21, 46, 0x1D],
        [0x1D, 20, 50, 0],
        [0, 82, 158, 0x40],
        [0x40, 30, 22, 0x40, 17, "talk"],
        [0x40, 30, 56, 0],
        [0, 128, 121, 0xD8],
        [0xD8, 31, 31, 0x9B],
        [0x9B, 16, 11, 0x9D],
        [0x9D, 16, 15, 0x9F],
        [0x9F, 18, 15, 0xA1],
        [0xA1, 32, 15, 0xD9],
        [0xD9, 31, 36, 0],
        [0, 150, 88, 0x41],
        [0x41, 45, 21, 0x43],
        [0x43, 30, 32, 0x43, 0x30, "talk"],
    ]
    start_map = 0
    start_flag = 16
    start_label = "CONTINUE-Holt"
    end_label = "Tonoe-Gryz-joined"
    boss_check_leg = -1

func choice_input(choice):
    var prompt = " ".join(choice.lines)
    if "refining Titanium" in prompt or "blocked by a rock" in prompt:
        choose(choice.cursor, 1)
    elif "Warrior" in prompt:
        choose(choice.cursor, 0)
    else:
        fail("unrecognized Dorin choice", choice)
    cooldown = 8

func _physics_process(delta):
    var result = super._physics_process(delta)
    if game != null and not finished:
        var state = JSON.parse_string(game.debug_play_state())
        if state != null and state.battle != null and state.battle.message == "RES" and not "combat-RES" in scene_shots:
            scene_capture("combat-RES", state)
        if state != null and state.map == 0xD8 and state.scene:
            if state.dialogue and not "rune-dialogue" in scene_shots:
                scene_capture("rune-dialogue", state)
            for object in state.temporary_objects:
                if int(object.id) == 0x214 and int(object.elapsed) >= 20 and not "rune-casting" in scene_shots:
                    scene_capture("rune-casting", state)
                if int(object.id) == 0x218 and int(object.elapsed) >= 20 and not "rune-flame" in scene_shots:
                    scene_capture("rune-flame", state)
            if state.map_patches == 0 and not "rock-open" in scene_shots:
                scene_capture("rock-open", state)
    return result

func scene_capture(label, state):
    scene_shots[label] = true
    checkpoint(label, state)
    capture.call_deferred(label + ".png")

func before_route(state):
    if leg == 2 and state.cell == [21, 34] and not rested and state.camp == null:
        if inn_phase == 0:
            inn_money = int(state.money)
            press("ui_up")
            inn_phase = 1
        elif inn_phase == 1:
            press("ui_accept")
            inn_phase = 2
        elif state.shop == null:
            if inn_phase == 4:
                rested = true
                healing = true
                healing_done = false
                checkpoint("Mile-paid-rest", state)
            else:
                fail("Mile inn did not open", state)
        elif state.shop.mode == "InnGreeting":
            press("ui_cancel" if inn_phase == 4 else "ui_accept")
        elif state.shop.mode == "InnConfirm":
            choose(state.shop.confirm, 0)
        elif state.shop.mode == "Message":
            if int(state.money) != inn_money - 30:
                fail("Mile inn charged the wrong amount", state)
            inn_phase = 4
            scene_capture("Mile-inn-receipt", state)
            press("ui_accept")
        else:
            fail("unexpected inn mode", state)
        cooldown = 10
        return true
    return super.before_route(state)

var command_actor := -1
var command_spell := -1
var command_target := -1
var planned_heals := []

func battle_input(battle):
    if battle.menu == null:
        command_actor = -1
        planned_heals.clear()
    if not battle.ready or battle.menu == null or battle.menu.actor == null:
        super.battle_input(battle)
        return
    var menu = battle.menu
    if int(menu.actor) != command_actor:
        command_actor = int(menu.actor)
        command_spell = -1
        command_target = -1
        var injured = menu.party.filter(func(p): return p.hp > 0 and p.hp * 10 < p.max_hp * 7 and not int(p.id) in planned_heals)
        injured.sort_custom(func(a, b): return a.hp / a.max_hp < b.hp / b.max_hp)
        var available = menu.techniques.filter(func(t): return t.available).map(func(t): return int(t.id))
        if not injured.is_empty() and 24 in available:
            command_spell = 24
            command_target = int(injured[0].id)
            planned_heals.append(command_target)
        else:
            var enemies = menu.enemies.duplicate()
            enemies.sort_custom(func(a, b): return a.hp < b.hp)
            if not enemies.is_empty():
                command_target = int(enemies[0].id)
            if int(menu.character) == 3:
                if enemies.size() > 1 and 13 in available:
                    command_spell = 13
                elif 1 in available:
                    command_spell = 1
            elif int(menu.character) == 1 and enemies.size() == 1 and 1 in available:
                command_spell = 1
        print("NATIVE TONOE planned command actor=", menu.character, " spell=", command_spell, " target=", command_target)
    if menu.page == "Actions":
        choose(menu.cursor, 1 if command_spell >= 0 else 0)
    elif menu.page == "Techniques":
        var index := -1
        for i in range(menu.techniques.size()):
            if int(menu.techniques[i].id) == command_spell:
                index = i
        if index < 0:
            fail("planned spell is absent from the actual menu", menu)
            return
        choose(menu.cursor, index)
    elif menu.page.begins_with("Targets"):
        var ids = menu.targets.map(func(id): return int(id))
        var index = ids.find(command_target)
        choose(menu.cursor, max(0, index))
    else:
        fail("unexpected battle command page", menu)
    cooldown = 4
