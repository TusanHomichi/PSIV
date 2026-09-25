# One START, ordinary movement, conversations, and battle-menu input.
# Read-only probes observe the live Field. No scene injection, relocation,
# stat changes, or direct battle commands are available to this driver.
extends SceneTree

var game
var tick := 0
var held := ""
var leg := 0
var cooldown := 0
var talk_phase := 0
var route_started := false
var finished := false
var directory := ""
var checkpoints := []
var battle_count := 0
var was_battle := false
var reached_world := false
const DIRECTIONS = [Vector2i(0, -1), Vector2i(0, 1), Vector2i(-1, 0), Vector2i(1, 0)]
const ACTIONS = ["ui_up", "ui_down", "ui_left", "ui_right"]
# source map, destination cell, expected destination map, optional talk/flag
const ACADEMY_ROUTE = [
    [0x13, 38, 17, 0x13, 8],
    [0x13, 31, 10, 0x14],
    [0x14, 15, 16, 0x14, 9, "talk"],
    [0x14, 15, 21, 0x13],
    [0x13, 16, 19, 0x11],
    [0x11, 30, 8, 0x12],
    [0x12, 13, 18, 0x12, 10, "talk"],
    [0x12, 18, 13, 0x15],
    [0x15, 32, 9, 0x16],
    [0x16, 14, 19, 0x17],
    [0x17, 15, 11, 0x17, 15, "talk"],
    [0x17, 16, 19, 0x16],
    [0x16, 48, 9, 0x15],
    [0x15, 48, 9, 0x12],
    [0x12, 14, 22, 0x11],
    [0x11, 16, 19, 0x13],
    [0x13, 31, 10, 0x14],
    [0x14, 15, 16, 0x14, 12, "talk"],
    [0x14, 15, 21, 0x13],
    [0x13, 16, 19, 0x11],
    [0x11, 31, 20, 0x10],
    [0x10, 31, 48, 0],
]
var route = ACADEMY_ROUTE
var start_map := 0x13
var start_flag := 7
var start_label := "START"
var end_label := "Motavia"
var boss_check_leg := 10
var talk_idle := 0

func _initialize():
    directory = OS.get_environment("PSIV_ROUTE_OUTPUT")
    call_deferred("start_game")

func start_game():
    game = load("res://field.tscn").instantiate()
    root.add_child(game)
    current_scene = game

func press(action):
    held = action
    Input.action_press(action)

func _physics_process(_delta):
    tick += 1
    if held != "":
        Input.action_release(held)
        held = ""
        return false
    if game == null or finished:
        return false
    var raw = game.debug_play_state()
    if raw == "":
        return false
    var state = JSON.parse_string(raw)
    state.flags = state.flags.map(func(flag): return int(flag))
    state.cell = state.cell.map(func(value): return int(value))
    if tick % 600 == 0:
        print("NATIVE ROUTE observation ", JSON.stringify(state))
    if tick > 100000:
        fail("route exceeded 100000 ticks", state)
        return false
    if cooldown > 0:
        cooldown -= 1
        return false
    if state.title:
        # Fresh output directory: START is the first menu item.
        press("ui_accept")
        cooldown = 15
        return false
    if state.battle != null:
        if not was_battle:
            battle_count += 1
            was_battle = true
            print("NATIVE ROUTE battle ", battle_count, " at leg ", leg)
            capture.call_deferred("battle-%02d.png" % battle_count)
        battle_input(state.battle)
        return false
    if was_battle:
        after_battle()
    was_battle = false
    if state.get("dialogue_choice") != null:
        if state.dialogue_choice.ready:
            choice_input(state.dialogue_choice)
        return false
    if state.scene or state.dialogue or state.transition or state.stepping:
        return false
    if not route_started:
        if state.map != start_map or not start_flag in state.flags:
            return false
        route_started = true
        checkpoint(start_label, state)
    if before_route(state):
        return false
    if leg >= route.size():
        if not reached_world:
            checkpoint(end_label, state)
            reached_world = true
        save_input(state)
        return false
    var step = route[leg]
    var arrived = state.map == step[3] if step[0] != step[3] else state.cell == [step[1], step[2]]
    # Staged dialogue can move the party after arrival; the flag finishes it.
    if step.size() > 4 and step[4] in state.flags:
        arrived = true
    if arrived:
        if leg == boss_check_leg and 15 in state.flags and state.active_npcs.any(func(i): return int(i) < 3):
            fail("defeated boss objects survived field reload", state)
            return false
        if step.size() > 4 and not step[4] in state.flags:
            if step.size() > 5 and step[5] == "talk":
                if talk_phase == 0:
                    press("ui_up")
                    talk_phase = 1
                elif talk_phase == 1:
                    press("ui_accept")
                    talk_phase = 2
                    cooldown = 30
                else:
                    # RunEvents owns the following frame after an event returns.
                    talk_idle += 1
                    if talk_idle > 3:
                        fail("interaction did not set its story flag", state)
            return false
        checkpoint("leg-%02d" % leg, state)
        leg += 1
        talk_phase = 0
        talk_idle = 0
        cooldown = 4
        return false
    if state.map != step[0]:
        fail("unexpected map for leg %d" % leg, state)
        return false
    var direction = first_step(Vector2i(state.cell[0], state.cell[1]), Vector2i(step[1], step[2]))
    if direction < 0:
        fail("no walking path for leg %d" % leg, state)
    else:
        press(ACTIONS[direction])
    return false

func before_route(_state):
    return false

func after_battle():
    pass

func first_step(start, target):
    var map = JSON.parse_string(game.debug_walk_map())
    var queue = [start]
    var seen = {start: -1}
    var head := 0
    while head < queue.size():
        var at = queue[head]
        head += 1
        for i in range(4):
            var next = at + DIRECTIONS[i]
            if next.x < 0 or next.y < 0 or next.x >= map.width or next.y >= map.height or next in seen:
                continue
            if not map.walkable[next.y][next.x]:
                continue
            var doorway := false
            if next != target:
                for rect in map.warps:
                    var area = Rect2i(rect[0], rect[1], rect[2], rect[3])
                    # Arrival can occupy a two-cell stair trigger. Leaving
                    # it may cross another cell of that same trigger; retail
                    # does not re-enter while both cells are map-change tiles.
                    if area.has_point(next) and not area.has_point(target) and not area.has_point(start):
                        doorway = true
                        break
            if doorway:
                continue
            var first = i if seen[at] == -1 else seen[at]
            if next == target:
                return first
            seen[next] = first
            queue.append(next)
    return -1

func choice_input(choice):
    fail("route has no answer for this dialogue choice", choice)

func battle_input(battle):
    if battle.ready and battle.menu != null:
        # Attack the parent; Alys's slash still reaches the whole group.
        var menu = battle.menu
        var desired := 0
        for i in range(menu.rows.size()):
            if menu.rows[i][0].begins_with("IGGLANOVA"):
                desired = i
        if menu.cursor < desired:
            press("ui_down")
        elif menu.cursor > desired:
            press("ui_up")
        else:
            press("ui_accept")
    elif battle.ready or battle.finishing or battle.message == "Victory!" or battle.message.ends_with("LV increased!") or " learned " in battle.message:
        press("ui_accept")
    cooldown = 4

func save_input(state):
    if state.camp == null:
        press("ui_cancel")
    else:
        var camp = state.camp
        match camp.mode:
            "Root":
                press("ui_down" if camp.root < 4 else "ui_accept")
            "State":
                press("ui_down" if camp.state < 2 else "ui_accept")
            "SaveSlots":
                press("ui_accept")
            "SaveResult":
                if not FileAccess.file_exists(OS.get_environment("PSIV_SAVE_DIR").path_join("slot_1.sram")):
                    fail("save menu did not produce a slot", state)
                    return
                checkpoint("Saved", state)
                capture.call_deferred("saved.png")
                finished = true
                FileAccess.open(directory.path_join("route.json"), FileAccess.WRITE).store_string(JSON.stringify({"checkpoints": checkpoints, "battles": battle_count, "complete": true}, "  "))
                finish.call_deferred()
            _:
                fail("unexpected camp state while saving", state)
    cooldown = 8

func checkpoint(name, state):
    print("NATIVE ROUTE ", name, " ", JSON.stringify(state))
    checkpoints.append({"name": name, "state": state})
    if name in ["START", "leg-00", "leg-07", "leg-10", "leg-17", "Motavia"]:
        capture.call_deferred(name + ".png")

func capture(name):
    await RenderingServer.frame_post_draw
    root.get_texture().get_image().save_png(directory.path_join(name))

func fail(message, state):
    push_error(message + ": " + JSON.stringify(state))
    finished = true
    FileAccess.open(directory.path_join("failure.json"), FileAccess.WRITE).store_string(JSON.stringify({
        "complete": false, "error": message, "state": state,
        "checkpoints": checkpoints, "battles": battle_count,
    }, "  "))
    finish_failure.call_deferred()

func finish_failure():
    await RenderingServer.frame_post_draw
    root.get_texture().get_image().save_png(directory.path_join("failure.png"))
    quit(1)

func finish():
    await RenderingServer.frame_post_draw
    quit(0)
