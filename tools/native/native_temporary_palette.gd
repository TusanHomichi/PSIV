# Scene temporary object palette fixture: Cutscene_Alshline ($8005) started by
# the debug event selector. The scene loads Zema itself, copies
# PiataBasementB2_Palettes to line 3 and builds Igglanova ($188, art tile $3A5),
# whose sheet draws line 3. Zema's map update then cycles line 1 every eight
# frames while Igglanova stays on screen. No save is loaded or written; no
# input or palette mutation is performed here.
# Required: PSIV_DEBUG_EVENT=0x8005, PSIV_DEBUG_AUTOCLOSE_SCENE=1,
# PSIV_DEBUG_RETAIL_PACE=1, PSIV_DEBUG_ROUTE=1, PSIV_SAVE_DIR (empty
# directory), PSIV_TEMPORARY_OUTPUT (existing directory).
# Run through tools/godot.py run under the godot heavy-lock class.
extends SceneTree

const OBJECT_ID := 0x188
const SHEET_SIZE := Vector2(64, 32)
# The retail surface the camera centres (`retail_surface.rs`).
const SCREEN := Vector2(320, 224)
const LINE := 3
const RAMP_R := [0, 32, 65, 98, 139, 172, 205, 238]
const RAMP_G := [0, 32, 68, 101, 137, 170, 206, 238]
const RAMP_B := [0, 32, 65, 98, 139, 172, 205, 238]

var game
var tick := 0
var output := ""
var previous: Dictionary = {}
var built: Dictionary = {}
var built_pixels := PackedByteArray()
var before: Dictionary = {}
var observing := false

func _initialize():
    output = OS.get_environment("PSIV_TEMPORARY_OUTPUT")
    if output.is_empty() or not DirAccess.dir_exists_absolute(output):
        fail("PSIV_TEMPORARY_OUTPUT must name an existing directory")
        return
    call_deferred("start_game")

func start_game():
    game = load("res://field.tscn").instantiate()
    root.add_child(game)
    current_scene = game

func fail(message: String):
    push_error(message)
    quit(1)

func words_of(state: Dictionary, from: int, to: int) -> Array:
    return state.cram.words.slice(from, to)

func rgb(word: int) -> Array:
    return [RAMP_R[(word >> 1) & 7], RAMP_G[(word >> 5) & 7], RAMP_B[(word >> 9) & 7]]

# The object's sprite: the one visible node showing its 64x32 sheet.
func object_node():
    var found = null
    for child in game.get_children():
        if child is Sprite2D and child.visible and child.texture != null \
                and child.texture.get_size() == SHEET_SIZE:
            if found != null:
                fail("two visible nodes show a 64x32 sheet")
                return null
            found = child
    return found

func on_screen(node: Sprite2D) -> bool:
    var camera: Camera2D = game.get_viewport().get_camera_2d()
    if camera == null:
        return false
    var view := Rect2(camera.global_position - SCREEN / 2, SCREEN)
    return view.encloses(Rect2(node.global_position + node.offset, node.region_rect.size))

func _physics_process(_delta):
    if game == null or observing: return false
    tick += 1
    if tick > 6000:
        fail("Igglanova was not observed through a palette change")
        return false
    var raw := str(game.debug_play_state())
    if raw.is_empty():
        fail("PSIV_DEBUG_ROUTE=1 is required for the CRAM observation")
        return false
    var state = JSON.parse_string(raw)
    var listed := false
    for object in state.get("temporary_objects", []):
        listed = listed or int(object.id) == OBJECT_ID
    var node = object_node() if listed else null
    if not listed and not built.is_empty():
        fail("Igglanova left before the palette change was observed")
        return false
    if node == null:
        if built.is_empty():
            previous = state
        return false
    if not state.cram.live or state.cram.words.size() != 64:
        fail("no live 64-word CRAM")
        return false
    var image: Image = node.texture.get_image()
    if built.is_empty():
        check_copy(state, image)
    elif not on_screen(node):
        pass
    elif before.is_empty():
        check_unchanged(state, image, "line 3 changed before the capture")
        before = state
        capture("before", node, image, state)
    elif int(state.cram.revision) != int(before.cram.revision) \
            and words_of(state, 16, 32) != words_of(before, 16, 32):
        check_unchanged(state, image, "line 3 changed")
        var changed := 0
        for i in range(16, 32):
            if state.cram.words[i] != before.cram.words[i]:
                changed += 1
        print("TEMPORARY_DURING line1_changed_words=%d" % changed)
        capture("during", node, image, state)
    return false

# The copy and the build share a tick: the previous tick shows the map's
# line 3, this one the copied words, and every opaque pixel is a line-3 colour.
func check_copy(state: Dictionary, image: Image):
    if previous.is_empty():
        fail("no tick before the object")
        return
    if words_of(previous, 48, 62) == words_of(state, 48, 62):
        fail("line 3 did not change when Igglanova was built")
        return
    for span in [[0, 16], [32, 48], [62, 64]]:
        if words_of(previous, span[0], span[1]) != words_of(state, span[0], span[1]):
            fail("words %d-%d changed with the copy" % [span[0], span[1] - 1])
            return
    var live := {}
    var old := {}
    for i in range(1, 16):
        live[rgb(int(state.cram.words[LINE * 16 + i]))] = true
        old[rgb(int(previous.cram.words[LINE * 16 + i]))] = true
    if image.get_format() != Image.FORMAT_RGBA8:
        fail("the sheet is not the live RGBA texture")
        return
    var pixels := image.get_data()
    var opaque := 0
    var copied := 0
    for offset in range(0, pixels.size(), 4):
        if pixels[offset + 3] == 0:
            continue
        opaque += 1
        var colour := [pixels[offset], pixels[offset + 1], pixels[offset + 2]]
        if not live.has(colour):
            fail("pixel %d is not a line-3 colour" % (offset / 4))
            return
        if not old.has(colour):
            copied += 1
    if opaque == 0 or copied == 0:
        fail("the object shows no copied colour (%d opaque)" % opaque)
        return
    built = state
    built_pixels = pixels
    var record := {"tick": tick, "previous_cram": previous.cram, "cram": state.cram,
        "opaque": opaque, "copied_colour_pixels": copied}
    FileAccess.open(output.path_join("built.json"), FileAccess.WRITE).store_string(JSON.stringify(record))
    print("TEMPORARY_BUILT tick=%d opaque=%d copied_colour_pixels=%d" % [tick, opaque, copied])

# Line 3 and every pixel of the sheet are as they were when it was built.
func check_unchanged(state: Dictionary, image: Image, message: String):
    if words_of(state, 48, 64) != words_of(built, 48, 64):
        fail(message)
        return
    if image.get_data() != built_pixels:
        fail("the object changed with another line")

func capture(label: String, node: Sprite2D, image: Image, state: Dictionary):
    observing = true
    game.set_physics_process(false)
    image.save_png(output.path_join(label + "-sheet.png"))
    var rect: Rect2 = node.region_rect
    image.get_region(Rect2i(rect)).save_png(output.path_join(label + "-object.png"))
    var camera: Camera2D = game.get_viewport().get_camera_2d()
    var record := {"tick": tick, "region": [rect.position.x, rect.position.y, rect.size.x, rect.size.y],
        "position": [node.global_position.x, node.global_position.y],
        "camera": [camera.global_position.x, camera.global_position.y], "state": state}
    FileAccess.open(output.path_join(label + ".json"), FileAccess.WRITE).store_string(JSON.stringify(record))
    await RenderingServer.frame_post_draw
    root.get_texture().get_image().save_png(output.path_join(label + "-viewport.png"))
    print("TEMPORARY_CAPTURE ", label, " tick=", tick, " region=", record.region,
        " revision=", state.cram.revision)
    if label == "during":
        quit(0)
        return
    observing = false
    game.set_physics_process(true)
