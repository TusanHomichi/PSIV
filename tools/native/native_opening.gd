# Ordinary title START, dialogue dismissals, then eight frames of Up.
# PSIV_DEBUG_ROUTE=1 enables read-only observations; no state is injected.
extends SceneTree

var game
var tick := 0
var held := ""
var cooldown := 0
var moved_at := -1
var before := {}
var saw_title := false

func _initialize():
    call_deferred("start_game")

func start_game():
    game = load("res://field.tscn").instantiate()
    root.add_child(game)
    current_scene = game

func fail(message, state):
    push_error("opening: " + message + " " + JSON.stringify(state))
    quit(1)

func press(action):
    Input.action_press(action)
    held = action
    cooldown = 4

func _physics_process(_delta):
    tick += 1
    if game == null:
        return false
    var raw = game.debug_play_state()
    if raw == "":
        if tick > 60:
            fail("read-only probe requires PSIV_DEBUG_ROUTE=1", {})
        return false
    var state = JSON.parse_string(raw)
    if tick > 12000:
        fail("timed out", state)
        return false
    if moved_at >= 0:
        if tick == moved_at + 8:
            Input.action_release("ui_up")
        if tick == moved_at + 16:
            if state.scene or state.cell != [before.cell[0], before.cell[1] - 1]:
                fail("eight Up frames did not move one cell", state)
                return false
            var output = OS.get_environment("PSIV_OPENING_OUTPUT")
            var file = FileAccess.open(output + "/state.json", FileAccess.WRITE)
            file.store_string(JSON.stringify({"before": before, "after": state}, "  "))
            root.get_texture().get_image().save_png(output + "/field.png")
            print("opening: PASS ordinary START, dialogue and one Up step at t", tick)
            quit(0)
        return false
    if held != "":
        Input.action_release(held)
        held = ""
        return false
    if cooldown > 0:
        cooldown -= 1
        return false
    if state.title:
        saw_title = true
        if state.title_menu != null:
            press("psiv_speak")
        return false
    if state.dialogue:
        var page = state.get("dialogue_page")
        if page != null and page.ready:
            press("psiv_speak")
        return false
    if not state.scene and state.map == 0x13 and state.flags.has(21):
        if not saw_title or state.money != 500 or state.party_status.size() != 1 or state.party_status[0].id != 0:
            fail("first-control state differs", state)
            return false
        before = state
        moved_at = tick
        Input.action_press("ui_up")
    return false
