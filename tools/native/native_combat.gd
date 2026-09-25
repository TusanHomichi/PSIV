# A real battle from an isolated native save. See docs/campaign/NATIVE_PLAYABILITY.md.
# Chaz -> RES -> Chaz; Alys -> FOI -> second enemy; Hahn -> GELUN.
extends SceneTree

var tick := 0
var held := ""
const PRESSES = {
    100: "ui_accept", 110: "ui_down", 120: "ui_accept",
    130: "ui_accept", 140: "ui_accept", 150: "ui_down",
    160: "ui_accept", 170: "ui_down", 180: "ui_down",
    190: "ui_accept", 200: "ui_down", 210: "ui_accept",
    220: "ui_down", 230: "ui_accept", 240: "ui_accept",
}

func _initialize():
    call_deferred("start_game")

func start_game():
    var game = load("res://field.tscn").instantiate()
    root.add_child(game)
    current_scene = game

func _physics_process(_delta):
    tick += 1
    if held != "":
        Input.action_release(held)
        held = ""
    if tick in PRESSES:
        held = PRESSES[tick]
        Input.action_press(held)
    if tick in [185, 205]:
        var directory = OS.get_environment("PSIV_COMBAT_SHOTS")
        if directory != "":
            root.get_texture().get_image().save_png(directory.path_join("techniques.png" if tick == 185 else "target.png"))
    return false
