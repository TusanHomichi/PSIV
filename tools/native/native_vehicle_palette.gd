# Isolated rendering fixture: Anger Tower's existing $99 session, with the
# debug vehicle selector mounting a Land Rover on a map it cannot normally enter.
# No save is loaded or written. No input or palette mutation is performed here.
# Required: PSIV_DEBUG_EVENT=0x99, PSIV_DEBUG_VEHICLE_INDEX=1,
# PSIV_DEBUG_AUTOCLOSE_SCENE=1, PSIV_DEBUG_RETAIL_PACE=1, PSIV_DEBUG_ROUTE=1,
# PSIV_SAVE_DIR (empty directory), PSIV_VEHICLE_OUTPUT (existing directory).
# Run through tools/godot.py run under the godot heavy-lock class.
extends SceneTree
var game
var vehicle
var tick := 0
var initial := PackedByteArray()
var changes := 0
var output := ""
var observing := false
var before_state: Dictionary = {}

func _initialize():
    output = OS.get_environment("PSIV_VEHICLE_OUTPUT")
    if output.is_empty() or not DirAccess.dir_exists_absolute(output):
        push_error("PSIV_VEHICLE_OUTPUT must name an existing directory")
        quit(1)
        return
    call_deferred("start_game")

func start_game():
    game = load("res://field.tscn").instantiate()
    root.add_child(game)
    current_scene = game
    var sprites := []
    for child in game.get_children():
        if child is Sprite2D:
            sprites.append(child)
    if sprites.size() < 4:
        push_error("expected map, overlay, party and vehicle sprites")
        quit(1)
        return
    vehicle = sprites[3]

func _physics_process(_delta):
    if game == null or observing: return false
    tick += 1
    if tick > 900:
        push_error("vehicle palette did not change")
        quit(1)
        return false
    if tick < 60 or not vehicle.visible or vehicle.texture == null: return false
    var image: Image = vehicle.texture.get_image()
    var pixels := image.get_data()
    if initial.is_empty():
        initial = pixels.duplicate()
        capture("before", image)
    elif pixels != initial:
        changes += 1
        if changes == 12:
            capture("during", image)
    return false

func capture(label: String, image: Image):
    observing = true
    game.set_physics_process(false)
    image.save_png(output.path_join(label + "-sheet.png"))
    var rect: Rect2 = vehicle.region_rect
    image.get_region(Rect2i(rect)).save_png(output.path_join(label + "-vehicle.png"))
    var raw := str(game.debug_play_state())
    if raw.is_empty():
        push_error("PSIV_DEBUG_ROUTE=1 is required for the CRAM observation")
        quit(1)
        return
    var state = JSON.parse_string(raw)
    if not state.cram.live or state.cram.words.size() != 64:
        push_error("fixture has no live 64-word CRAM")
        quit(1)
        return
    if label == "before":
        before_state = state
    else:
        if before_state.cram.words.slice(32, 48) != state.cram.words.slice(32, 48):
            push_error("held line 2 changed")
            quit(1)
            return
        if before_state.cram.words.slice(48, 64) == state.cram.words.slice(48, 64):
            push_error("vehicle line 3 did not change")
            quit(1)
            return
        var after := image.get_data()
        for offset in range(3, initial.size(), 4):
            if initial[offset] != after[offset]:
                push_error("vehicle sheet transparency changed")
                quit(1)
                return
        if before_state.cell != state.cell:
            push_error("fixture moved during the palette change")
            quit(1)
            return
    var record := {"tick": tick, "region": [rect.position.x, rect.position.y, rect.size.x, rect.size.y],
        "position": [vehicle.position.x, vehicle.position.y], "visible": vehicle.visible,
        "state": state}
    FileAccess.open(output.path_join(label + ".json"), FileAccess.WRITE).store_string(JSON.stringify(record))
    await RenderingServer.frame_post_draw
    root.get_texture().get_image().save_png(output.path_join(label + "-viewport.png"))
    print("VEHICLE_CAPTURE ", label, " ", JSON.stringify(record))
    if label == "during":
        quit(0)
    else:
        game.set_physics_process(true)
        observing = false
