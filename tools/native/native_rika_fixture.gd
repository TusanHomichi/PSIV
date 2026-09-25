# Visual inspection of the existing isolated PSIV_DEBUG_EVENT=8007 fixture.
# This observer provides no game input and never writes a campaign save.
extends SceneTree
var game
var directory := ""
var checkpoints := []
var pages := {}
var shots := {}
var tick := 0
var ending := false

func _initialize():
    directory = OS.get_environment("PSIV_ROUTE_OUTPUT")
    call_deferred("start_game")

func start_game():
    game = load("res://field.tscn").instantiate()
    root.add_child(game)
    current_scene = game

func _physics_process(_delta):
    tick += 1
    if game == null or ending: return false
    var state = JSON.parse_string(game.debug_play_state())
    if state == null: return false
    if state.dialogue_page != null and state.dialogue_page.ready:
        var key = JSON.stringify([state.dialogue_page.tree,state.dialogue_page.lines])
        if not key in pages:
            pages[key] = true
            record("dialogue-%03d" % pages.size(),state)
    for object in state.temporary_objects:
        if int(object.id) == 0x194 and int(object.elapsed) >= 20 and not "holt-exit" in shots:
            record("holt-exit",state)
    if not state.scene and not state.transition and not state.dialogue and state.flags.any(func(f): return int(f) == 0x35):
        record("fixture-complete",state)
        ending = true
        FileAccess.open(directory.path_join("fixture.json"),FileAccess.WRITE).store_string(JSON.stringify({"complete":true,"fixture":true,"checkpoints":checkpoints},"  "))
        finish.call_deferred()
    if tick > 30000:
        push_error("Rika fixture did not complete")
        quit(1)
    return false

func record(label,state):
    shots[label] = true
    checkpoints.append({"name":label,"state":state})
    print("RIKA FIXTURE ",label," ",JSON.stringify(state))
    capture.call_deferred(label+".png")

func capture(name):
    await RenderingServer.frame_post_draw
    root.get_texture().get_image().save_png(directory.path_join(name))

func finish():
    await RenderingServer.frame_post_draw
    quit(0)
