# Fresh process: title CONTINUE, one normal field step, then save slot 2.
extends SceneTree
var game
var held := ""
var tick := 0
var phase := 0
var cooldown := 0
var expected
var directory := ""
var observations := []
var source_slot := 0
var destination_slot := 1
var check_item_slot := -1
var save_slot_seen := false

func _initialize():
    if OS.has_environment("PSIV_CONTINUE_SLOT"):
        source_slot = int(OS.get_environment("PSIV_CONTINUE_SLOT")) - 1
    if OS.has_environment("PSIV_CONTINUE_SAVE_SLOT"):
        destination_slot = int(OS.get_environment("PSIV_CONTINUE_SAVE_SLOT")) - 1
    if OS.has_environment("PSIV_CONTINUE_ITEM_SLOT"):
        check_item_slot = int(OS.get_environment("PSIV_CONTINUE_ITEM_SLOT")) - 1
    directory = OS.get_environment("PSIV_CONTINUE_OUTPUT")
    var receipt = JSON.parse_string(FileAccess.get_file_as_string(OS.get_environment("PSIV_CONTINUE_ROUTE_RECEIPT")))
    expected = receipt.checkpoints[-1].state
    call_deferred("start_game")

func start_game():
    game = load("res://field.tscn").instantiate()
    root.add_child(game)
    current_scene = game

func press(action):
    held = action
    Input.action_press(action)
    cooldown = 8

func _physics_process(_delta):
    tick += 1
    if held != "":
        Input.action_release(held)
        held = ""
        return false
    if game == null:
        return false
    var raw = game.debug_play_state()
    if raw == "":
        return false
    var state = JSON.parse_string(raw)
    if tick > 8000:
        fail("continue flow timed out", state)
        return false
    if cooldown > 0:
        cooldown -= 1
        return false
    if state.title:
        if state.get("title_menu") != null and state.title_menu.phase == "Slots":
            choose(state.title_menu.cursor, source_slot)
        else:
            press("ui_accept")
        return false
    if state.scene or state.dialogue or state.transition or state.stepping:
        return false
    match phase:
        0:
            var keys = ["map", "cell", "money", "flags", "leader"]
            for optional_key in ["inventory", "chests", "party_status", "party_resources"]:
                if expected.has(optional_key): keys.append(optional_key)
            for key in keys:
                if state[key] != expected[key]:
                    fail("CONTINUE changed " + key, state)
                    return false
            record("continued", state)
            phase = 4 if check_item_slot >= 0 else 1
            cooldown = 20
        1:
            if state.cell == expected.cell:
                press("ui_down")
            else:
                if state.cell != [expected.cell[0], expected.cell[1] + 1]:
                    fail("field input did not produce one downward step", state)
                    return false
                record("moved", state)
                phase = 2
                cooldown = 20
        2:
            var camp = state.camp
            if camp == null:
                press("ui_cancel")
            else:
                match camp.mode:
                    "Root": choose(camp.root, 4)
                    "State": choose(camp.state, 2)
                    "SaveSlots":
                        if camp.save == destination_slot and not save_slot_seen:
                            record("save-slot-selected",state)
                            save_slot_seen = true
                            cooldown = 20
                        else: choose(camp.save, destination_slot)
                    "SaveResult":
                        if camp.message != "FILE SAVED":
                            fail("continued save failed", state)
                            return false
                        record("resaved", state)
                        phase = 3
                        cooldown = 30
        4:
            var camp = state.camp
            if camp == null: press("ui_cancel")
            elif camp.mode == "Root": choose(camp.root, 0)
            elif camp.mode == "ItemList":
                if camp.item != check_item_slot: choose(camp.item, check_item_slot)
                else:
                    record("last-inventory-slot", state)
                    press("ui_cancel")
                    phase = 5
                    cooldown = 20
        5:
            if state.camp != null: press("ui_cancel")
            else:
                phase = 1
                cooldown = 20
        3:
            var file = FileAccess.open(directory.path_join("receipt.json"), FileAccess.WRITE)
            file.store_string(JSON.stringify({"complete": true, "observations": observations}, "  "))
            quit(0)
    return false

func choose(current, desired):
    press("ui_down" if current < desired else ("ui_up" if current > desired else "ui_accept"))

func record(label, state):
    observations.append({"label": label, "tick": tick, "state": state})
    print("NATIVE CONTINUE ", label, " ", JSON.stringify(state))
    capture.call_deferred(label)

func capture(label):
    await RenderingServer.frame_post_draw
    root.get_texture().get_image().save_png(directory.path_join(label + ".png"))

func fail(message, state):
    push_error(message + ": " + JSON.stringify(state))
    quit(1)
