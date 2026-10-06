# STATE/ORDER through ordinary input, including undo, cancel and SAVE.
# Starts from the connected BioPlant save (map $A7, flag $37) through the
# title's CONTINUE. The tape replay's `bioplant-order` chapter proves the
# committed ORDER and its SAVE byte for byte (docs/campaign/NATIVE_DRIVERS.md);
# this fixture adds the pick, undo and cancel paths and the captures that
# tools/native/verify_native_order.py compares with the original game.
extends SceneTree

const WANTED = [4, 1, 0, 2]
const START_MAP := 0xA7
const START_FLAG := 0x37
const START_LABEL := "CONTINUE-BioPlant-order"
const END_LABEL := "BioPlant-Gryz-leading"

var game
var tick := 0
var held := ""
var cooldown := 0
var directory := ""
var checkpoints := []
var started := false
var finished := false
var reached_end := false
var order_phase := 0
var before_order

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

func choose(current, desired):
    press("ui_down" if current < desired else ("ui_up" if current > desired else "ui_accept"))

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
        print("NATIVE ORDER observation ", JSON.stringify(state))
    if tick > 100000:
        fail("ORDER fixture exceeded 100000 ticks", state)
        return false
    if cooldown > 0:
        cooldown -= 1
        return false
    if state.title:
        # A save exists, so the title's default row is CONTINUE.
        press("ui_accept")
        cooldown = 15
        return false
    if state.battle != null:
        fail("the ORDER fixture does not fight", state)
        return false
    if state.scene or state.dialogue or state.transition or state.stepping:
        return false
    if not started:
        if state.map != START_MAP or not START_FLAG in state.flags:
            return false
        started = true
        checkpoint(START_LABEL, state, true)
    if order_input(state):
        return false
    if not reached_end:
        reached_end = true
        checkpoint(END_LABEL, state, true)
    save_input(state)
    return false

func order_input(state):
    if before_order == null:
        before_order = state.duplicate(true)
    if order_phase >= 6:
        return false
    var camp = state.camp
    if order_phase == 5:
        if camp == null:
            order_phase = 6
        else:
            press("ui_cancel")
    elif camp == null:
        press("psiv_camp")
    elif camp.mode == "Root":
        choose(camp.root, 4)
    elif camp.mode == "State":
        if order_phase == 3:
            unchanged(state)
            checkpoint("order-cancelled", state)
            order_phase = 4
        choose(camp.state, 1)
    elif camp.mode == "Order":
        if order_phase == 0:
            checkpoint("order-open", state)
            capture.call_deferred("order-open.png")
            order_phase = 1
        if order_phase == 1:
            if camp.order_chosen.is_empty():
                choose(camp.order_cursor, camp.order_remaining.map(func(id): return int(id)).find(4))
            else:
                unchanged(state)
                checkpoint("order-picked", state)
                capture.call_deferred("order-picked.png")
                press("ui_cancel")
                order_phase = 2
        elif order_phase == 2:
            if not camp.order_chosen.is_empty() or camp.order_remaining.map(func(id): return int(id)) != [1, 0, 2, 4]:
                fail("ORDER undo failed", state)
                return true
            unchanged(state)
            checkpoint("order-undone", state)
            capture.call_deferred("order-undone.png")
            press("ui_cancel")
            order_phase = 3
        elif order_phase == 4:
            choose(camp.order_cursor, camp.order_remaining.map(func(id): return int(id)).find(WANTED[camp.order_chosen.size()]))
    elif camp.mode == "OrderDone":
        if state.party_status.map(func(p): return int(p.id)) != WANTED or int(state.leader) != 4:
            fail("ORDER did not commit the requested lineup", state)
            return true
        var a = before_order.party_resources.duplicate(true)
        var b = state.party_resources.duplicate(true)
        a.sort_custom(func(x, y): return x.id < y.id)
        b.sort_custom(func(x, y): return x.id < y.id)
        if a != b:
            fail("ORDER changed character resources", state)
        checkpoint("order-committed", state)
        capture.call_deferred("order-committed.png")
        press("ui_accept")
        order_phase = 5
    else:
        fail("unexpected ORDER menu page", state)
    cooldown = 10
    return true

func unchanged(state):
    for key in ["map", "cell", "money", "inventory", "flags", "leader", "party_status", "party_resources"]:
        if state[key] != before_order[key]:
            fail("ORDER cancellation changed " + key, state)

func save_input(state):
    if state.camp == null:
        press("psiv_camp")
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
                FileAccess.open(directory.path_join("route.json"), FileAccess.WRITE).store_string(JSON.stringify({"checkpoints": checkpoints, "battles": 0, "complete": true}, "  "))
                finish.call_deferred()
            _:
                fail("unexpected camp state while saving", state)
    cooldown = 8

func checkpoint(label, state, shoot := false):
    print("NATIVE ORDER ", label, " ", JSON.stringify(state))
    checkpoints.append({"name": label, "state": state})
    if shoot:
        capture.call_deferred(label + ".png")

func capture(name):
    await RenderingServer.frame_post_draw
    root.get_texture().get_image().save_png(directory.path_join(name))

func fail(message, state):
    push_error(message + ": " + JSON.stringify(state))
    finished = true
    FileAccess.open(directory.path_join("failure.json"), FileAccess.WRITE).store_string(JSON.stringify({
        "complete": false, "error": message, "state": state,
        "checkpoints": checkpoints, "battles": 0,
    }, "  "))
    finish_failure.call_deferred()

func finish_failure():
    await RenderingServer.frame_post_draw
    root.get_texture().get_image().save_png(directory.path_join("failure.png"))
    quit(1)

func finish():
    await RenderingServer.frame_post_draw
    quit(0)
