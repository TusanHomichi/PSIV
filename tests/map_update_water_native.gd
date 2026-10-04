# Input-only title CONTINUE from an explicit Aiedo placement fixture.
# Observe the renderer's palette uniform and a real water pixel after drawing.
extends SceneTree

var game
var ticks := 0
var held := false
var busy := false
var last_color := ""
var records := []
var output := ""

func _initialize():
    output = OS.get_environment("PSIV_MAP_UPDATES_NATIVE_OUTPUT")
    if output == "":
        push_error("PSIV_MAP_UPDATES_NATIVE_OUTPUT is required")
        quit(2)
    call_deferred("start_game")

func start_game():
    game = load("res://field.tscn").instantiate()
    root.add_child(game)
    current_scene = game

func _physics_process(_delta):
    ticks += 1
    if held:
        Input.action_release("ui_accept")
        held = false
        return false
    if ticks > 1000:
        push_error("Aiedo title/palette probe timed out")
        quit(1)
        return false
    if game == null:
        return false
    var raw = game.debug_play_state()
    if raw == "":
        return false
    var state = JSON.parse_string(raw)
    if state.title:
        if ticks % 8 == 0:
            Input.action_press("ui_accept")
            held = true
        return false
    if state.map != 84:
        push_error("CONTINUE did not load Aiedo")
        quit(1)
        return false
    if not busy:
        busy = true
        capture.call_deferred()
    return false

func capture():
    await RenderingServer.frame_post_draw
    var map_sprite
    for child in game.get_children():
        if child is Sprite2D and child.z_index == 0 and child.texture != null:
            if child.texture.get_width() == 1536 and child.material is ShaderMaterial:
                map_sprite = child
                break
    if map_sprite == null:
        push_error("indexed map material missing")
        quit(1)
        return
    var palette = map_sprite.material.get_shader_parameter("palette")
    var color = palette[26]
    var key = str([color.r8, color.g8, color.b8])
    if key == last_color:
        busy = false
        return
    last_color = key
    var world_point = Vector2(556.5, 484.5)
    var screen = map_sprite.get_global_transform_with_canvas() * world_point
    var picture = root.get_texture().get_image()
    var point = Vector2i(screen)
    if point.x < 0 or point.y < 0 or point.x >= picture.get_width() or point.y >= picture.get_height():
        push_error("water point is outside the viewport")
        quit(1)
        return
    var actual = picture.get_pixelv(point)
    var rgb = [actual.r8, actual.g8, actual.b8]
    if rgb != [color.r8, color.g8, color.b8]:
        push_error("rendered water pixel disagrees with runtime-owned palette: " + str(rgb) + " vs " + key)
        quit(1)
        return
    var values := []
    for entry in palette:
        values.append([entry.r8, entry.g8, entry.b8])
    var label = "water-%02d" % (records.size() + 1)
    picture.save_png(output.path_join(label + ".png"))
    records.append({"label":label, "tick":JSON.parse_string(game.debug_play_state()).tick,
        "pixel": [point.x, point.y], "rgb":rgb, "palette":values})
    print("AIEDO WATER ", JSON.stringify(records[-1]))
    if records.size() == 8:
        var file = FileAccess.open(output.path_join("receipt.json"), FileAccess.WRITE)
        file.store_string(JSON.stringify({"complete":true, "fixture":true, "records":records}, "  "))
        quit(0)
    busy = false
