# Isolated battle-entry fixtures, launched through tools/godot.py run with
# PSIV_DEBUG_EVENT or PSIV_DEBUG_ENCOUNTER_ENTRY. Checks the native stage's
# initialized presence and its framing on every load frame and at the roll.
# PSIV_LOAD_ENEMIES is the expected visible count; PSIV_LOAD_REQUIRE_BARS=1
# additionally requires scene bars and a preceding field camera away from
# the battle origin. PSIV_LOAD_REPORT names the JSON receipt.
extends SceneTree

var game: Node
var tick := 0
var load_frames := 0
var saw_field_camera := false
var field_camera := Vector2.ZERO
var checked_bars := 0

func _initialize():
    call_deferred("start_game")

func start_game():
    game = load("res://field.tscn").instantiate()
    root.add_child(game)
    current_scene = game

func stop(reason: String):
    var report = {"pass": reason.is_empty(), "reason": reason, "tick": tick,
        "load_frames": load_frames, "checked_bars": checked_bars,
        "preceding_field_camera_away_from_battle": saw_field_camera,
        "preceding_field_camera": [field_camera.x, field_camera.y]}
    FileAccess.open(OS.get_environment("PSIV_LOAD_REPORT"), FileAccess.WRITE).store_string(JSON.stringify(report, "  "))
    print("NATIVE BATTLE LOAD ", JSON.stringify(report))
    quit(0 if reason.is_empty() else 1)

func _physics_process(_delta):
    tick += 1
    if tick > 1200:
        stop("battle entry timed out")
        return false
    if game == null:
        return false
    var camera: Camera2D
    var stage: Node2D
    var bars := []
    for child in game.get_children():
        if child is Camera2D:
            camera = child
        if child.get_class() == "BattleScreen":
            stage = child
        if child is ColorRect and child.z_index == 500 and child.visible:
            bars.append(child)
    if camera == null or stage == null or not stage.visible:
        if camera != null:
            field_camera = camera.position
            saw_field_camera = not field_camera.is_equal_approx(Vector2(160, 112))
        return false
    var loading := false
    var enemies := 0
    for child in stage.get_children():
        if child is Sprite2D:
            if child.z_index == 20:
                loading = true
            if child.z_index == -12 and child.visible:
                enemies += 1
    var expected := int(OS.get_environment("PSIV_LOAD_ENEMIES"))
    if enemies != expected:
        stop("visible enemies: expected %d, got %d" % [expected, enemies])
        return false
    if OS.get_environment("PSIV_LOAD_REQUIRE_BARS") == "1" and bars.size() != 4:
        stop("scene's four letterbox bars are missing")
        return false
    if bars.size() == 4:
        var center := camera.position
        var edges = [bars[0].position.y + bars[0].size.y, bars[1].position.y,
            bars[2].position.x + bars[2].size.x, bars[3].position.x]
        var wanted = [center.y - 112, center.y + 112, center.x - 160, center.x + 160]
        for index in range(4):
            if not is_equal_approx(edges[index], wanted[index]):
                stop("letterbox edge %d: expected %f, got %f" % [index, wanted[index], edges[index]])
                return false
        checked_bars += 1
    if loading:
        load_frames += 1
    elif load_frames > 0:
        if load_frames != 20:
            stop("expected 20 loading frames, got %d" % load_frames)
        elif OS.get_environment("PSIV_LOAD_REQUIRE_BARS") == "1" and not saw_field_camera:
            stop("fixture did not exercise a preceding field camera away from battle")
        else:
            stop("")
    return false
