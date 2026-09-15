# Isolated fixture. Use actual CONTINUE, walking/menu input, defeat, and CONTINUE again.
extends SceneTree
var game
var tick := 0
var held := ""
var cooldown := 0
var phase := 0
var steps := 0
var previous := [99, 84]
var observations := []
var captured := {}
var directory := ""
var mode := ""
var pending_notice := ""
var notice_captured := false

func _initialize():
    directory = OS.get_environment("PSIV_STATUS_OUTPUT")
    mode = OS.get_environment("PSIV_STATUS_MODE")
    call_deferred("start_game")

func start_game():
    game = load("res://field.tscn").instantiate()
    root.add_child(game)
    current_scene = game

func press(action):
    held = action
    Input.action_press(action)
    cooldown = 8

func _process(_delta):
    if game != null:
        var state = JSON.parse_string(game.debug_play_state())
        if state != null and state.poison_flash and not "poison-flash" in captured:
            record("poison-flash", state)
    return false

func _physics_process(_delta):
    tick += 1
    if held != "":
        Input.action_release(held)
        held = ""
        return false
    if game == null: return false
    var state = JSON.parse_string(game.debug_play_state())
    if state == null: return false
    state.cell = state.cell.map(func(value): return int(value))
    if tick > 12000:
        push_error("status fixture timed out: " + JSON.stringify(state))
        quit(1)
        return false
    if tick % 600 == 0: print("STATUS observation ", JSON.stringify(state))
    if state.title and state.game_over and phase < 2:
        record("game-over-title", state)
        phase = 2
        cooldown = 20
    if cooldown > 0:
        cooldown -= 1
        return false
    if state.title:
        press("ui_accept")
        return false
    if phase == 2 and not state.transition:
        if state.cell != [99, 84] or state.party_status[0].hp != (2 if mode == "poison" else 1) or state.game_over:
            push_error("CONTINUE did not restore the saved fixture: " + JSON.stringify(state))
            quit(1)
            return false
        if mode == "poison" and not "poison-flash" in captured:
            push_error("poison flash was never rendered")
            quit(1)
            return false
        record("continued-after-defeat", state)
        FileAccess.open(directory.path_join("receipt.json"), FileAccess.WRITE).store_string(JSON.stringify({"complete":true,"fixture":true,"mode":mode,"steps":steps,"observations":observations},"  "))
        finish.call_deferred()
        phase = 3
        return false
    if phase == 3: return false
    if phase == 1 and mode == "poison" and not state.stepping and not state.transition and state.cell != previous:
        steps += 1
        previous = state.cell
    if state.battle != null:
        var b = state.battle
        if b.finishing and b.message.ends_with("defeated...!") and not "defeat-message" in captured: record("defeat-message", state)
        if b.ready:
            if b.menu == null: press("ui_accept")
            elif b.menu.page == "Actions":
                press("ui_down" if b.menu.cursor < 4 else "ui_accept")
            else:
                push_error("unexpected defeat command menu")
                quit(1)
        return false
    if state.field_notice != null:
        if pending_notice != state.field_notice:
            pending_notice = state.field_notice
            notice_captured = false
            cooldown = 15
        elif not notice_captured:
            record("notice-" + str(observations.size()), state)
            notice_captured = true
            cooldown = 4
        elif state.dialogue:
            press("ui_cancel")
        return false
    pending_notice = ""
    if state.game_over or state.transition or state.stepping or state.dialogue: return false
    if phase == 0:
        phase = 1
        record("fixture-start", state)
    if mode == "poison":
        if steps < 8: press("ui_down" if steps % 2 == 0 else "ui_up")
    return false

func record(label, state):
    captured[label] = true
    observations.append({"label":label,"tick":tick,"state":state})
    print("STATUS ", label, " ", JSON.stringify(state))
    capture.call_deferred(label)

func capture(label):
    await RenderingServer.frame_post_draw
    root.get_texture().get_image().save_png(directory.path_join(label + ".png"))

func finish():
    await RenderingServer.frame_post_draw
    quit(0)
