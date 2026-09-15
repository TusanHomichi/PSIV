# Real Godot command input on an isolated native save; NATIVE_PLAYABILITY.md.
# Chaz EARTH -> enemy 6, Alys VORTEX -> enemy 7, Hahn VISION -> party.
extends SceneTree

var tick := 0
var held := ""
const PRESSES = {
    100: "ui_accept", 110: "ui_down", 120: "ui_down", 130: "ui_accept",
    140: "ui_accept", 150: "ui_accept",
    160: "ui_down", 170: "ui_down", 180: "ui_accept",
    190: "ui_accept", 200: "ui_down", 210: "ui_accept",
    220: "ui_down", 230: "ui_down", 240: "ui_accept", 250: "ui_accept",
    # Reopen Chaz's skill list after the round to inspect the remaining uses.
    1000: "ui_accept", 1010: "ui_down", 1020: "ui_down", 1030: "ui_accept",
}
const SHOTS = {135: "earth.png", 145: "target.png", 185: "vortex.png", 245: "vision.png"}

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
    if tick in SHOTS:
        var directory = OS.get_environment("PSIV_SKILL_SHOTS")
        if directory != "":
            root.get_texture().get_image().save_png(directory.path_join(SHOTS[tick]))
    return false
