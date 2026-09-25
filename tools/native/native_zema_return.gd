# Connected Alshline save: paid rest, return through the pass, rescue Zema,
# ordinary boss commands, post-battle conversation and save outside town.
extends "res://../tools/native/native_alshline_retreat.gd"

var zema_pages := []

func _physics_process(delta):
    var result = super._physics_process(delta)
    if game != null and not finished:
        var state = JSON.parse_string(game.debug_play_state())
        if state != null and int(state.map) == 0x24:
            for object in state.temporary_objects:
                if int(object.id) == 0x188 and object.get("position") != null:
                    var y = int(object.position[1])
                    var label = "igglanova-halfway" if y == 176 else ("igglanova-arrived" if y == 192 else "")
                    if label != "" and not label in scene_shots:
                        scene_capture(label,state)
            var page = state.get("dialogue_page")
            if page != null and page.ready:
                var key = JSON.stringify([page.tree,page.lines])
                if not key in zema_pages:
                    zema_pages.append(key)
                    scene_capture("zema-page-%02d" % zema_pages.size(),state)
    return result

func configure_route():
    route = [
        [0x41,19,42,0x41],
        [0x41,19,41,0x46],
        [0x46,20,32,0x46],
        [0x46,22,40,0x41],
        [0x41,22,56,0],
        [0,148,93,0xD9],
        [0xD9,31,31,0xA1],
        [0xA1,32,49,0x9F],
        [0x9F,30,49,0x9D],
        [0x9D,46,49,0x9B],
        [0x9B,32,53,0xD8],
        [0xD8,31,36,0],
        [0,99,82,0x24],
        [0x24,30,17,0x24,0x37],
        [0x24,31,50,0],
    ]
    start_map = 0x41
    start_flag = 0x32
    start_label = "CONTINUE-Alshline-in-Tonoe"
    end_label = "Zema-restored-after-Igglanova"
    boss_check_leg = -1
    healing = false

func should_retreat(_battle):
    var state = JSON.parse_string(game.debug_play_state())
    return not state.flags.any(func(f): return int(f) == 0x33)

func allow_instant_death():
    return false # The boss is immune; Gryz uses his actual equipped weapon.

func before_route(state):
    if int(state.map) == 0x24 and 0x33 in state.flags and not 0x37 in state.flags:
        return true # Let the automatic post-battle scene take the next tick.
    if int(state.map) == 0x24 and 0x37 in state.flags:
        if zema_pages.size() != 50:
            fail("Zema restoration skipped dialogue pages",state)
            return true
        if not "igglanova-halfway" in scene_shots or not "igglanova-arrived" in scene_shots:
            fail("Igglanova did not complete its visible field entrance",state)
            return true
        if state.active_npcs.size() != 7:
            fail("Zema restoration did not restore the seven townspeople",state)
            return true
    return super.before_route(state)
