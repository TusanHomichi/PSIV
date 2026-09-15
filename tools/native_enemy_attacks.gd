# Isolated durability fixture with retail formation 150. Select DEFEND through
# COMD until Acid Breath executes, then win through ordinary attack menus.
extends SceneTree
var game
var tick := 0
var held := ""
var cooldown := 0
var saw_acid := false
var verified := false
var was_battle := false
var directory := ""
var observations := []

func _initialize():
    directory = OS.get_environment("PSIV_ENEMY_ATTACK_OUTPUT")
    call_deferred("start_game")

func start_game():
    game = load("res://field.tscn").instantiate()
    root.add_child(game)
    current_scene = game

func press(action):
    held = action
    Input.action_press(action)
    cooldown = 6

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
    if tick % 300 == 0:
        print("NATIVE ENEMY ATTACK observation ", JSON.stringify(state))
    if tick == 300:
        capture.call_deferred("observed")
    if tick > 12000:
        push_error("enemy attack fixture timed out")
        quit(1)
        return false
    if cooldown > 0:
        cooldown -= 1
        return false
    var battle = state.battle
    if battle == null:
        if was_battle and verified and not state.transition:
            record("field-return", state)
            FileAccess.open(directory.path_join("receipt.json"), FileAccess.WRITE).store_string(JSON.stringify({"complete":true, "fixture":true, "observations": observations}, "  "))
            finish.call_deferred()
        return false
    was_battle = true
    if battle.message == "ACIDBREATH" and not saw_acid:
        saw_acid = true
        record("acid-breath", state)
    if battle.ready:
        if saw_acid and not verified:
            verified = true
            record("after-acid-breath", state)
        if battle.menu == null:
            # COMD is the first root entry; the middle entry is MACR.
            press("ui_accept")
        else:
            var desired = 0 if verified else 4
            press("ui_down" if battle.menu.cursor < desired else ("ui_up" if battle.menu.cursor > desired else "ui_accept"))
    elif battle.finishing or battle.message == "Victory!" or battle.message.ends_with("LV increased!"):
        press("ui_accept")
    return false

func record(label, state):
    observations.append({"label":label, "tick":tick, "state":state})
    print("NATIVE ENEMY ATTACK ", label, " ", JSON.stringify(state))
    capture.call_deferred(label)

func capture(label):
    await RenderingServer.frame_post_draw
    root.get_texture().get_image().save_png(directory.path_join(label + ".png"))

func finish():
    await RenderingServer.frame_post_draw
    quit(0)
