# Observe the real Ending fixture without advancing, skipping or rewriting its
# scene. Only rendering is disabled between captures. No save is loaded; the
# launcher supplies an empty PSIV_SAVE_DIR and PSIV_DEBUG_EVENT=0x8021.
extends SceneTree

var game
var text
var output := ""
var observing := false
var failed := false
var seen := false
var next := 0
var receipts := []
const LABELS := ["before", "fall", "dark", "rise", "after", "cleared"]
const WORDS := [0xEEE, 0x888, 0x222, 0xAAA, 0xEEE]

func _initialize():
    output = OS.get_environment("PSIV_STAFF_OUTPUT")
    if output.is_empty() or not DirAccess.dir_exists_absolute(output):
        fail("PSIV_STAFF_OUTPUT must name an existing directory")
        return
    call_deferred("start_game")

func start_game():
    game = load("res://field.tscn").instantiate()
    root.add_child(game)
    current_scene = game
    var layers = game.find_children("*", "TextPlaneLayer", true, false)
    if layers.size() != 1:
        fail("expected one shared TextPlaneLayer")
        return
    text = layers[0]
    RenderingServer.set_render_loop_enabled(false)

func fail(message: String):
    if failed: return
    failed = true
    push_error(message)
    quit(1)

func _physics_process(_delta):
    if game == null or text == null or observing or failed: return false
    var raw := str(game.debug_play_state())
    if raw.is_empty():
        fail("PSIV_DEBUG_ROUTE=1 is required")
        return false
    var state = JSON.parse_string(raw)
    if int(state.tick) > 45000:
        fail("staff palette sequence did not finish")
        return false
    if text.is_visible_in_tree():
        seen = true
        if next < WORDS.size() and int(state.cram.words[47]) == WORDS[next]:
            observing = true
            observe.call_deferred(state)
    elif seen and next == WORDS.size():
        observing = true
        observe.call_deferred(state)
    return false

func observe(state: Dictionary):
    game.set_physics_process(false)
    game.set_process(false)
    # The deferred observation runs after the field's physics callback.
    # Record the frozen game tick that actually supplies the pixels.
    state = JSON.parse_string(str(game.debug_play_state()))
    if next < WORDS.size() and int(state.cram.words[47]) != WORDS[next]:
        fail("palette changed before the capture could freeze")
        return
    RenderingServer.set_render_loop_enabled(true)
    for i in 3:
        await RenderingServer.frame_post_draw
    var label: String = LABELS[next]
    var path := output.path_join(label + ".png")
    var viewport: Image = root.get_texture().get_image()
    if viewport.save_png(path) != OK:
        fail("cannot save " + path)
        return
    receipts.append({"label": label, "tick": state.tick, "word47": state.cram.words[47],
        "visible": text.is_visible_in_tree(), "viewport": [viewport.get_width(), viewport.get_height()]})
    print("STAFF_CAPTURE ", JSON.stringify(receipts.back()))
    next += 1
    if next == LABELS.size():
        var receipt := FileAccess.open(output.path_join("captures.json"), FileAccess.WRITE)
        if receipt == null:
            fail("cannot write captures.json")
            return
        receipt.store_string(JSON.stringify(receipts, "  ") + "\n")
        quit(0)
        return
    RenderingServer.set_render_loop_enabled(false)
    game.set_physics_process(true)
    game.set_process(true)
    observing = false
