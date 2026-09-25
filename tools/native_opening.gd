# Run with the opening smoke-test command in docs/campaign/NATIVE_PLAYABILITY.md.
# The real Field node owns the game. This script only supplies one Up press
# after the auto-acknowledged opening has returned control.
extends SceneTree

var tick := 0

func _initialize():
    call_deferred("start_game")

func start_game():
    var game = load("res://field.tscn").instantiate()
    root.add_child(game)
    current_scene = game

func _physics_process(_delta):
    tick += 1
    if tick == 3300:
        Input.action_press("ui_up")
    if tick == 3308:
        Input.action_release("ui_up")
    return false
