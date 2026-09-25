# Isolated Academy boss fixture. Defend twice to allow both births, then
# Alys uses Dynamite on the left creature and Igglanova replaces it.
extends SceneTree
var tick := 0
var held := ""
var presses := {}
const SHOTS = {90: "alone.png", 700: "first-birth.png", 1350: "both-born.png", 1760: "replacement-progress.png"}
func _initialize():
    for start in [100, 750]:
        presses[start] = "ui_accept"
        for actor in range(3):
            for down in range(4):
                presses[start + 10 + actor * 50 + down * 10] = "ui_down"
            presses[start + 50 + actor * 50] = "ui_accept"
    presses[1400] = "ui_accept"
    for at in [1410,1420,1430,1440,1460,1470,1480,1500,1510,1520,1550,1560,1570,1580]:
        presses[at] = "ui_down"
    for at in [1450,1490,1530,1540,1590]:
        presses[at] = "ui_accept"
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
    if tick in presses:
        held = presses[tick]
        Input.action_press(held)
    if tick in SHOTS:
        var directory = OS.get_environment("PSIV_FISSION_SHOTS")
        if directory != "":
            root.get_texture().get_image().save_png(directory.path_join(SHOTS[tick]))
    return false
