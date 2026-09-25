# Start the post-Igglanova scene from the isolated dialogue_resume save.
# PSIV_DEBUG_RETAIL_PACE dismisses only fully rendered pages; this script
# captures the result and supplies no scene acknowledgements.
extends SceneTree
var tick := 0
const SHOTS = [210, 450, 620, 1000, 1500, 2500, 3600]
func _initialize():
    call_deferred("start_game")
func start_game():
    var game = load("res://field.tscn").instantiate()
    root.add_child(game)
    current_scene = game
func _physics_process(_delta):
    tick += 1
    if tick in SHOTS:
        var directory = OS.get_environment("PSIV_DIALOGUE_SHOTS")
        if directory != "":
            capture.call_deferred(directory.path_join("page-%04d.png" % tick))
    return false

func capture(path):
    await RenderingServer.frame_post_draw
    root.get_texture().get_image().save_png(path)
