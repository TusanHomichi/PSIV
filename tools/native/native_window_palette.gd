# Native window palette fixture (#533): one window family is opened from a
# debug selector, the game is paused, and line 2 of the windows' palette is
# changed under control through `Field.debug_window_line` (the windows'
# material only: the runtime's CRAM, the map and the sprites keep their
# words). Four viewport captures: `before`, `hidden` (the family's node
# hidden, so the difference is the window's footprint), `during` (line 2's
# every word with its colour components inverted, `word ^ $0EEE`) and
# `after` (the override cleared).
#
# It fails unless: the node carries a material; every footprint pixel that
# was a line-2 colour 1-15 turned into that index's new colour; no other
# pixel changed; and `after` equals `before` exactly. With
# PSIV_WINDOW_UNBIND=1 it removes the node's material before `during`, the
# wrong-binding negative control, which must fail.
#
# PSIV_WINDOW_FAMILY selects the window and how it is reached:
#   camp    PSIV_DEBUG_CAMP=1; STATE, then STATUS (frames, text, portrait)
#   shop    PSIV_DEBUG_SHOP=<counter>; the counter's opening page
#   choice  a scene whose dialogue asks YES/NO (PSIV_DEBUG_EVENT=...); pages
#           are acknowledged until the choice is ready
# Required: PSIV_DEBUG_ROUTE=1, PSIV_SAVE_DIR (empty directory),
# PSIV_WINDOW_OUTPUT (existing directory). No save is loaded or written.
# Run through tools/godot.py run under the godot heavy-lock class.
extends SceneTree

const RAMP_R := [0, 32, 65, 98, 139, 172, 205, 238]
const RAMP_G := [0, 32, 68, 101, 137, 170, 206, 238]
const RAMP_B := [0, 32, 65, 98, 139, 172, 205, 238]
const NODE_CLASS := {"camp": "CampMenu", "shop": "ShopWindow", "choice": "DialogueWindow"}
const SETTLE := 30

var game
var family := ""
var output := ""
var tick := 0
var held := ""
var steady := 0
var last_key := ""
var camp_presses: Array = []
var capturing := false
var failed := false

func _initialize():
    family = OS.get_environment("PSIV_WINDOW_FAMILY")
    output = OS.get_environment("PSIV_WINDOW_OUTPUT")
    if not NODE_CLASS.has(family):
        fail("PSIV_WINDOW_FAMILY must be camp, shop or choice")
        return
    if output.is_empty() or not DirAccess.dir_exists_absolute(output):
        fail("PSIV_WINDOW_OUTPUT must name an existing directory")
        return
    # Root to STATE (four downs), then STATE's STATUS row.
    camp_presses = ["ui_down", "ui_down", "ui_down", "ui_down", "ui_accept", "ui_accept"]
    call_deferred("start_game")

func start_game():
    game = load("res://field.tscn").instantiate()
    root.add_child(game)
    current_scene = game

func fail(message: String):
    if failed:
        return
    failed = true
    push_error("WINDOW_PALETTE " + message)
    quit(1)

func rgb(word: int) -> Array:
    return [RAMP_R[(word >> 1) & 7], RAMP_G[(word >> 5) & 7], RAMP_B[(word >> 9) & 7]]

func window_node():
    for child in game.get_children():
        if child.get_class() == NODE_CLASS[family]:
            return child
    return null

func press(action: String):
    held = action
    Input.action_press(action)

# The page the family must reach, as a key that stays the same while it is up.
func target_key(state: Dictionary) -> String:
    match family:
        "camp":
            var camp = state.get("camp")
            if camp == null:
                return ""
            if str(camp.mode) == "Status":
                return "Status"
            if tick % 20 == 0 and not camp_presses.is_empty():
                var mode := str(camp.mode)
                if mode == "Root" or mode == "State":
                    press(camp_presses.pop_front())
            return ""
        "shop":
            var shop = state.get("shop")
            return "" if shop == null else str(shop.mode) + str(shop.message)
        "choice":
            var choice = state.get("dialogue_choice")
            if choice != null and choice.ready:
                return "choice" + str(choice.lines)
            var page = state.get("dialogue_page")
            if page != null and page.ready and tick % 20 == 0:
                press("ui_accept")
            return ""
    return ""

func _physics_process(_delta):
    if game == null or capturing or failed:
        return false
    if held != "":
        Input.action_release(held)
        held = ""
    tick += 1
    if tick > 4000:
        fail("the %s window was not reached" % family)
        return false
    var raw := str(game.debug_play_state())
    if raw.is_empty():
        fail("PSIV_DEBUG_ROUTE=1 is required")
        return false
    var state = JSON.parse_string(raw)
    var key := target_key(state)
    if key.is_empty() or key != last_key:
        last_key = key
        steady = 0
        return false
    steady += 1
    if steady == SETTLE:
        capturing = true
        observe.call_deferred(state)
    return false

func frame() -> Image:
    for i in 3:
        await RenderingServer.frame_post_draw
    return root.get_texture().get_image()

func observe(state: Dictionary):
    game.set_physics_process(false)
    game.set_process(false)
    var node = window_node()
    if node == null or not node.visible:
        fail("no visible %s" % NODE_CLASS[family])
        return
    if not (node.material is ShaderMaterial):
        fail("%s carries no window material" % NODE_CLASS[family])
        return
    if not state.cram.live:
        fail("no live palette")
        return
    var words: Array = state.cram.words.slice(32, 48)
    var live := PackedInt32Array()
    var changed := {}
    for i in 16:
        var word := int(words[i]) ^ 0x0EEE
        live.append(word)
        # Index 0 is never looked up; the first index with a colour wins.
        var from := rgb(int(words[i]))
        if i > 0 and not changed.has(from):
            changed[from] = rgb(word)

    var before: Image = await frame()
    node.visible = false
    var hidden: Image = await frame()
    node.visible = true
    if OS.get_environment("PSIV_WINDOW_UNBIND") == "1":
        node.material = null
    if not game.debug_window_line(live):
        fail("debug_window_line refused")
        return
    var during: Image = await frame()
    game.debug_window_line(PackedInt32Array())
    var after: Image = await frame()
    for pair in [["before", before], ["hidden", hidden], ["during", during], ["after", after]]:
        pair[1].save_png(output.path_join(pair[0] + ".png"))

    var width := before.get_width()
    var height := before.get_height()
    var footprint := 0
    var reached := 0
    var box := Rect2i()
    var first := ""
    for y in height:
        for x in width:
            var b := before.get_pixel(x, y)
            var was := [b.r8, b.g8, b.b8]
            var d := during.get_pixel(x, y)
            var now := [d.r8, d.g8, d.b8]
            var inside := hidden.get_pixel(x, y) != b
            if inside:
                footprint += 1
                box = Rect2i(x, y, 1, 1) if box.size == Vector2i.ZERO else box.expand(Vector2i(x, y))
            var expected = was
            if inside and changed.has(was):
                expected = changed[was]
            if now != expected and first.is_empty():
                first = "pixel (%d,%d) %s became %s, expected %s (%s)" % [
                    x, y, was, now, expected, "window" if inside else "outside the window"]
            if inside and changed.has(was) and now == expected:
                reached += 1
    box = box.grow_individual(0, 0, 1, 1)
    var report := {"family": family, "node": NODE_CLASS[family], "tick": tick,
        "viewport": [width, height], "footprint_pixels": footprint,
        "footprint_box": [box.position.x, box.position.y, box.size.x, box.size.y],
        "reached_pixels": reached, "live_line": Array(live), "cram": state.cram,
        "camp": state.get("camp"), "shop": state.get("shop"),
        "dialogue_choice": state.get("dialogue_choice"),
        "unbound": OS.get_environment("PSIV_WINDOW_UNBIND") == "1"}
    FileAccess.open(output.path_join("report.json"), FileAccess.WRITE).store_string(JSON.stringify(report))
    print("WINDOW_PALETTE family=%s footprint=%d box=%s reached=%d tick=%d revision=%d" % [
        family, footprint, report.footprint_box, reached, tick, int(state.cram.revision)])
    if footprint == 0 or reached == 0:
        fail("the line-2 change reached no window pixel (footprint %d)" % footprint)
        return
    if not first.is_empty():
        fail(first)
        return
    if after.get_data() != before.get_data():
        fail("clearing the override did not restore the frame")
        return
    quit(0)
