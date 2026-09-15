# Actual YES, NO, and Cancel input at Chaz's house. Retail-paced text remains
# on the choice until this script supplies the requested answer.
extends SceneTree
var tick := 0
var held := ""
const SHOTS = {400: "prompt.png", 480: "selected.png", 620: "answer.png"}
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
    var answer = OS.get_environment("PSIV_CHOICE_ANSWER")
    if tick == 450 and answer == "no":
        held = "ui_down"
    if tick == 500:
        held = "ui_cancel" if answer == "cancel" else "ui_accept"
    if held != "":
        Input.action_press(held)
    if tick in SHOTS:
        var directory = OS.get_environment("PSIV_CHOICE_SHOTS")
        if directory != "":
            capture.call_deferred(directory.path_join(SHOTS[tick]))
    return false

func capture(path):
    await RenderingServer.frame_post_draw
    root.get_texture().get_image().save_png(path)
    if OS.get_environment("PSIV_CAPTURE_X11") == "1":
        OS.execute("import", ["-window", "root", path.replace(".png", "-x11.png")])
