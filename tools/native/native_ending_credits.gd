# Observe the real Ending fixture through all three credits stages, the
# 14-step credits palette ramp and the third PaletteIncreaseTone loop, without
# advancing, skipping or rewriting the scene. Only rendering is disabled
# between captures. No save is loaded; the launcher supplies an empty
# PSIV_SAVE_DIR and PSIV_DEBUG_EVENT=0x8021.
extends SceneTree

var game
var output := ""
var observing := false
var failed := false
var next := 0
var receipts := []

const TARGETS := [
    {"label": "stage1_entry", "stage": 1, "tc": 0x0019, "fg": 0x0000, "bg": 0xFF39, "col": false,
     "w0": 0x0000, "w18": 0x0000, "w30": 0x0000, "w47": 0x0EEE, "w50": 0x08EE},
    {"label": "stage1_scroll", "stage": 1, "tc": 0x0156, "fg": 0x0000, "bg": 0x0076, "col": false,
     "w0": 0x0000, "w18": 0x0000, "w30": 0x0000, "w47": 0x0EEE, "w50": 0x08EE},
    {"label": "stage2_column_scroll", "stage": 2, "tc": 0x03BA, "fg": 0x003A, "bg": 0x02BD, "col": true,
     "w0": 0x0000, "w18": 0x0000, "w30": 0x0000, "w47": 0x0EEE, "w50": 0x08EE},
    {"label": "stage3_presented", "stage": 3, "tc": 0x0480, "fg": 0x0100, "bg": 0x0320, "col": true,
     "w0": 0x0000, "w18": 0x0000, "w30": 0x0000, "w47": 0x0000, "w50": 0x08EE},
    {"label": "ramp_mid", "stage": 3, "tc": 0x0480, "fg": 0x0100, "bg": 0x0320, "col": true,
     "w0": 0x0000, "w18": 0x0ECC, "w30": 0x0000, "w47": 0x0000, "w50": 0x08EE},
    {"label": "tone_baseline", "stage": 3, "tc": 0x0480, "fg": 0x0100, "bg": 0x0320, "col": true,
     "w0": 0x0000, "w18": 0x0EEE, "w30": 0x0ECC, "w47": 0x0000, "w50": 0x08EE},
    {"label": "tone_rise", "stage": 3, "tc": 0x0480, "fg": 0x0100, "bg": 0x0320, "col": true,
     "w0": 0x0888, "w18": 0x0EEE, "w30": 0x0EEE, "w47": 0x0888, "w50": 0x0EEE},
    {"label": "tone_white", "stage": 3, "tc": 0x0480, "fg": 0x0100, "bg": 0x0320, "col": true,
     "w0": 0x0EEE, "w18": 0x0EEE, "w30": 0x0EEE, "w47": 0x0EEE, "w50": 0x0EEE},
]

func _initialize():
    output = OS.get_environment("PSIV_CREDITS_OUTPUT")
    if output.is_empty() or not DirAccess.dir_exists_absolute(output):
        fail("PSIV_CREDITS_OUTPUT must name an existing directory")
        return
    call_deferred("start_game")

func start_game():
    game = load("res://field.tscn").instantiate()
    root.add_child(game)
    current_scene = game
    RenderingServer.set_render_loop_enabled(false)

func fail(message: String):
    if failed: return
    failed = true
    push_error(message)
    quit(1)

func matches_target(state: Dictionary, target: Dictionary) -> bool:
    if state.credits == null:
        return false
    var c: Dictionary = state.credits
    var words: Array = state.cram.words
    return (
        int(c.stage) == target.stage
        and int(c.text_counter) == target.tc
        and int(c.fg_y) == target.fg
        and int(c.bg_y) == target.bg
        and bool(c.column_scroll) == target.col
        and int(words[0]) == target.w0
        and int(words[18]) == target.w18
        and int(words[30]) == target.w30
        and int(words[47]) == target.w47
        and int(words[50]) == target.w50
    )

func _physics_process(_delta):
    if game == null or observing or failed: return false
    var raw := str(game.debug_play_state())
    if raw.is_empty():
        fail("PSIV_DEBUG_ROUTE=1 is required")
        return false
    var state = JSON.parse_string(raw)
    if int(state.tick) > 62000:
        fail("credits sequence did not reach all targets (next=%d)" % next)
        return false
    if next < TARGETS.size() and matches_target(state, TARGETS[next]):
        freeze_and_observe(state)
    return false

func freeze_and_observe(state: Dictionary):
    observing = true
    # SceneTree's physics callback precedes the field's. Freeze here so the
    # deferred render cannot accidentally capture the following game tick.
    game.set_physics_process(false)
    game.set_process(false)
    observe.call_deferred(state)

func observe(state: Dictionary):
    game.set_physics_process(false)
    game.set_process(false)
    state = JSON.parse_string(str(game.debug_play_state()))
    var target: Dictionary = TARGETS[next]
    if not matches_target(state, target):
        fail("credits state changed before capture could freeze for " + str(target.label))
        return
    RenderingServer.set_render_loop_enabled(true)
    for i in 3:
        await RenderingServer.frame_post_draw
    var label: String = target.label
    var path := output.path_join(label + ".png")
    var viewport: Image = root.get_texture().get_image()
    if viewport.save_png(path) != OK:
        fail("cannot save " + path)
        return
    var c: Dictionary = state.credits
    var words: Array = state.cram.words
    receipts.append({
        "label": label,
        "tick": state.tick,
        "stage": c.stage,
        "text_counter": c.text_counter,
        "fg_y": c.fg_y,
        "bg_y": c.bg_y,
        "column_scroll": c.column_scroll,
        "word0": words[0],
        "word18": words[18],
        "word30": words[30],
        "word47": words[47],
        "word50": words[50],
        "viewport": [viewport.get_width(), viewport.get_height()],
    })
    print("CREDITS_CAPTURE ", JSON.stringify(receipts.back()))
    next += 1
    if next == TARGETS.size():
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
