# Isolated damaged-party fixture; every menu action is ordinary Godot input.
extends SceneTree
var game
var held := ""
var tick := 0
var phase := 0
var cooldown := 0
var baseline := []
var directory := ""
var observations := []

func _initialize():
    directory = OS.get_environment("PSIV_CAMP_ABILITY_OUTPUT")
    call_deferred("start_game")

func start_game():
    game = load("res://field.tscn").instantiate()
    root.add_child(game)
    current_scene = game

func press(action):
    held = action
    Input.action_press(action)
    cooldown = 8

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
    if tick > 8000:
        fail("camp flow timed out", state)
        return false
    if cooldown > 0:
        cooldown -= 1
        return false
    if state.scene or state.dialogue or state.transition or state.title:
        return false
    var camp = state.camp
    if camp == null:
        press("ui_cancel")
        return false
    match phase:
        0:
            baseline = camp.party.duplicate(true)
            phase = 1
        1:
            match camp.mode:
                "Root": choose(camp.root, 1)
                "AbilityCharacters": choose(camp.caster, 0)
                "AbilityList": choose(camp.ability, ability_index(camp, 24))
                "AbilityTarget":
                    record("target-before-cancel", state)
                    phase = 2
                    cooldown = 15
        2:
            if camp.mode == "AbilityTarget":
                press("ui_cancel")
            elif camp.mode == "AbilityList":
                if camp.party != baseline:
                    fail("cancel changed HP or TP", state)
                    return false
                record("cancelled", state)
                phase = 3
                cooldown = 15
        3:
            match camp.mode:
                "AbilityList": choose(camp.ability, ability_index(camp, 24))
                "AbilityTarget": choose(camp.target, 0)
                "AbilityResult":
                    if camp.party[0].tp != baseline[0].tp - 3 or camp.party[0].hp <= baseline[0].hp:
                        fail("RES did not heal and charge exactly 3 TP", state)
                        return false
                    record("res-used", state)
                    phase = 4
                    cooldown = 15
        4:
            if camp.mode != "Root": press("ui_cancel")
            else: phase = 5
        5:
            match camp.mode:
                "Root": choose(camp.root, 2)
                "AbilityCharacters": choose(camp.caster, 2)
                "AbilityList": choose(camp.ability, ability_index(camp, 42))
                "AbilityResult":
                    if camp.party[2].hp <= 0 or int(camp.party[2].status) & 0x40 or camp.abilities[0].remaining != 2:
                        fail("RECOVER did not repair Demi and charge one use", state)
                        return false
                    record("recover-used", state)
                    phase = 6
                    cooldown = 15
        6:
            if camp.mode != "Root": press("ui_cancel")
            else: phase = 7
        7:
            match camp.mode:
                "Root": choose(camp.root, 4)
                "State": choose(camp.state, 2)
                "SaveSlots": press("ui_accept")
                "SaveResult":
                    if camp.message != "FILE SAVED":
                        fail("camp save failed", state)
                        return false
                    record("saved", state)
                    phase = 8
                    cooldown = 30
        8:
            var file = FileAccess.open(directory.path_join("receipt.json"), FileAccess.WRITE)
            file.store_string(JSON.stringify({"complete": true, "fixture": true, "observations": observations}, "  "))
            quit(0)
    return false

func choose(current, desired):
    press("ui_down" if current < desired else ("ui_up" if current > desired else "ui_accept"))

func ability_index(camp, id):
    for i in range(camp.abilities.size()):
        if camp.abilities[i].id == id:
            return i
    fail("expected learned ability missing", camp)
    return 0

func record(label, state):
    observations.append({"label": label, "tick": tick, "state": state})
    print("NATIVE CAMP ", label, " ", JSON.stringify(state))
    capture.call_deferred(label)

func capture(label):
    await RenderingServer.frame_post_draw
    root.get_texture().get_image().save_png(directory.path_join(label + ".png"))

func fail(message, state):
    push_error(message + ": " + JSON.stringify(state))
    quit(1)
