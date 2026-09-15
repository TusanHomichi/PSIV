# Real native item commands from the isolated combat_items save fixture.
# Chaz uses Dynamite on enemy 7; Alys heals Hahn; Hahn cures his poison.
extends SceneTree

var tick := 0
var held := ""
const PRESSES = {
    100: "ui_accept", 110: "ui_down", 120: "ui_down", 130: "ui_down",
    140: "ui_accept", 150: "ui_down", 160: "ui_down", 170: "ui_down",
    180: "ui_down", 190: "ui_down", 200: "ui_down", 210: "ui_accept",
    220: "ui_down", 230: "ui_accept",
    240: "ui_down", 250: "ui_down", 260: "ui_down", 270: "ui_accept",
    280: "ui_down", 290: "ui_down", 300: "ui_down", 310: "ui_accept",
    320: "ui_down", 330: "ui_down", 340: "ui_accept",
    350: "ui_down", 360: "ui_down", 370: "ui_down", 380: "ui_accept",
    390: "ui_down", 400: "ui_down", 410: "ui_down", 420: "ui_down",
    430: "ui_down", 440: "ui_accept", 450: "ui_down", 460: "ui_down",
    470: "ui_accept",
    # Reopen inventory after the round: only unused Moon-Dew/Repair-Kit remain.
    1200: "ui_accept", 1210: "ui_down", 1220: "ui_down", 1230: "ui_down",
    1240: "ui_accept", 1250: "ui_down", 1260: "ui_down", 1270: "ui_down",
    1280: "ui_down",
}
const SHOTS = {135: "actions.png", 205: "dynamite.png", 225: "enemy-target.png", 305: "monomate.png", 335: "ally-target.png", 435: "reserved.png"}

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
        var directory = OS.get_environment("PSIV_ITEM_SHOTS")
        if directory != "":
            root.get_texture().get_image().save_png(directory.path_join(SHOTS[tick]))
    return false
