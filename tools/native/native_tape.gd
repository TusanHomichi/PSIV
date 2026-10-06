# Generic campaign tape through the shipped Field node and ordinary actions.
# tools/verify_native_tape.py owns the isolated directories and this process.
extends SceneTree

const ACTIONS := [
    "ui_up", "ui_down", "ui_left", "ui_right",
    "ui_cancel", "ui_accept", "psiv_camp", "psiv_start",
]

var feed
var pads: PackedByteArray
var game: Node
var field_scene: PackedScene
var expected_count := 0
var consumed := 0
var started := false
var title_down := false
var frozen_checks := 0
var failed := false
var report := {}
# Chapter checkpoints: [{frame, label, expect_save}], ascending by frame. The
# slot snapshot is compared with each saved chapter the moment the Field has
# consumed that many gameplay Session frames, inside one continuous replay.
var checkpoints: Array = []
var next_checkpoint := 0
var checkpoint_results: Array = []
var replay_started_ms := 0

func _initialize() -> void:
    var tape_path := OS.get_environment("PSIV_TAPE_FILE")
    report = {"tape": tape_path, "result": "running", "source_copied": false}
    if tape_path.is_empty():
        _fail("PSIV_TAPE_FILE is required")
        return
    call_deferred("_prepare_tape")

func _prepare_tape() -> void:
    var tape_path := str(report["tape"])
    # The entry point has already prepared the local extension index. Load
    # the Field resource without constructing a game before source validation.
    field_scene = load("res://field.tscn")
    if field_scene == null:
        _fail("Field scene is unavailable")
        return
    feed = ClassDB.instantiate("TapeFeed")
    if feed == null:
        _fail("TapeFeed GDExtension class is unavailable")
        return
    if not feed.open_tape(tape_path):
        _fail(str(feed.last_error()))
        return
    pads = feed.pad_bytes()
    expected_count = int(str(feed.frame_count_decimal()))
    report["start_kind"] = str(feed.start_kind())
    report["frames_declared"] = str(feed.frame_count_decimal())
    report["source_fnv_hex"] = str(feed.save_hash_hex())
    if pads.size() != expected_count:
        _fail("typed feed frame count differs from its pad bytes")
        return
    # A prefix replay: stop after exactly this many gameplay frames.
    var stop_at := OS.get_environment("PSIV_TAPE_STOP_AT")
    if not stop_at.is_empty():
        if not stop_at.is_valid_int() or int(stop_at) < 1 or int(stop_at) > expected_count:
            _fail("PSIV_TAPE_STOP_AT must be a frame count within the tape")
            return
        expected_count = int(stop_at)
        report["frames_limit"] = expected_count
    if not _load_checkpoints():
        return
    var source_path := OS.get_environment("PSIV_TAPE_SOURCE_SAVE")
    if report["start_kind"] == "save":
        if source_path.is_empty() or not FileAccess.file_exists(source_path):
            _fail("save-start tape needs an existing source save")
            return
        var source_bytes := FileAccess.get_file_as_bytes(source_path)
        if not feed.matches_source_save(source_bytes):
            _fail(str(feed.last_error()))
            return
        report["source_hash_verified_before_copy"] = true
        var target := OS.get_environment("PSIV_SAVE_DIR").path_join("slot_1.sram")
        if FileAccess.file_exists(target):
            _fail("isolated slot already exists: " + target)
            return
        var output := FileAccess.open(target, FileAccess.WRITE)
        if output == null:
            _fail("cannot create isolated slot: " + target)
            return
        output.store_buffer(source_bytes)
        output.close()
        if FileAccess.get_file_as_bytes(target) != source_bytes:
            _fail("isolated slot differs from verified source bytes")
            return
        report["source_copied"] = true
        report["source_bytes"] = source_bytes.size()
    elif report["start_kind"] != "new-game" or not source_path.is_empty():
        _fail("tape start and source-save argument disagree")
        return
    call_deferred("_start_game")

func _load_checkpoints() -> bool:
    var path := OS.get_environment("PSIV_TAPE_CHECKPOINTS")
    if path.is_empty():
        return true
    if not FileAccess.file_exists(path):
        _fail("checkpoint list is missing: " + path)
        return false
    var parsed = JSON.parse_string(FileAccess.get_file_as_string(path))
    if typeof(parsed) != TYPE_ARRAY or parsed.is_empty():
        _fail("checkpoint list is not a non-empty JSON array")
        return false
    var previous := 0
    for entry in parsed:
        if typeof(entry) != TYPE_DICTIONARY or not entry.has("frame") \
                or not entry.has("label") or not entry.has("expect_save"):
            _fail("checkpoint entry needs frame, label and expect_save")
            return false
        var frame := int(entry["frame"])
        if frame <= previous or frame > expected_count:
            _fail("checkpoint %s frame %d is not ascending within the tape" % [entry["label"], frame])
            return false
        if not FileAccess.file_exists(str(entry["expect_save"])):
            _fail("checkpoint %s save is missing: %s" % [entry["label"], entry["expect_save"]])
            return false
        previous = frame
        checkpoints.append({"frame": frame, "label": str(entry["label"]),
                "expect_save": str(entry["expect_save"])})
    report["checkpoints_declared"] = checkpoints.size()
    return true

func _start_game() -> void:
    if failed:
        return
    game = field_scene.instantiate()
    root.add_child(game)
    current_scene = game

func _physics_process(_delta: float) -> bool:
    if failed or game == null:
        return false
    var boundary: PackedInt64Array = game.debug_tape_boundary()
    if boundary.size() != 6:
        _fail("native tape boundary probe is unavailable")
        return false
    var count := int(boundary[0])
    var phase := int(boundary[2])
    if phase == -2: # Field has not installed its session yet.
        return false
    if not started:
        if phase >= 0:
            _drive_title(phase, int(boundary[3]), int(boundary[4]))
            return false
        if count != 0:
            _fail("gameplay advanced before the first tape pad")
            return false
        report["title_handoff_shell_tick"] = int(boundary[5])
        replay_started_ms = Time.get_ticks_msec()
        started = true
        title_down = false
        if expected_count == 0:
            _freeze_at_end()
        else:
            _apply_game_pad(0)
        return false
    if phase >= 0:
        _fail("title reappeared during tape replay")
        return false
    if frozen_checks > 0:
        if count != expected_count:
            _fail("gameplay advanced after exact tape stop")
            return false
        frozen_checks += 1
        if frozen_checks >= 3:
            _finish()
        return false
    if count == consumed:
        return false # The Field callback has not consumed this byte yet.
    if count != consumed + 1 or count > expected_count:
        _fail("gameplay boundary jumped or ran past the tape")
        return false
    var actual := int(boundary[1])
    var wanted := int(pads[consumed])
    if actual != wanted:
        _fail("pad boundary mismatch at byte %d: expected %02x, got %02x" % [consumed, wanted, actual])
        return false
    consumed = count
    if consumed % 5000 == 0:
        print("native-tape: consumed %d/%d gameplay frames" % [consumed, expected_count])
    if not _check_checkpoint():
        return false
    if consumed == 1:
        if int(boundary[5]) != int(report["title_handoff_shell_tick"]) + 1:
            _fail("first gameplay Session frame did not follow title handoff")
            return false
        report["first_gameplay_shell_tick"] = int(boundary[5])
        report["first_gameplay_session_frame"] = count
        report["first_pad"] = actual
    if consumed == expected_count:
        report["last_pad"] = actual
        report["stop_shell_tick"] = int(boundary[5])
        _freeze_at_end()
    else:
        _apply_game_pad(consumed)
    return false

func _check_checkpoint() -> bool:
    if next_checkpoint >= checkpoints.size() or consumed != int(checkpoints[next_checkpoint]["frame"]):
        return true
    var point: Dictionary = checkpoints[next_checkpoint]
    var snapshot: PackedByteArray = game.debug_slot_bytes(0)
    var expected := FileAccess.get_file_as_bytes(str(point["expect_save"]))
    var result := {"label": point["label"], "frame": consumed,
            "elapsed_ms": Time.get_ticks_msec() - replay_started_ms,
            "snapshot_bytes": snapshot.size(), "expected_bytes": expected.size(),
            "snapshot_sha256": _sha256_hex(snapshot), "match": snapshot == expected}
    checkpoint_results.append(result)
    report["checkpoints"] = checkpoint_results
    next_checkpoint += 1
    if snapshot != expected:
        var first := 0
        while first < min(snapshot.size(), expected.size()) and snapshot[first] == expected[first]:
            first += 1
        result["first_differing_byte"] = first
        var dump := OS.get_environment("PSIV_TAPE_SNAPSHOT_OUT") + ".divergence-" + str(point["label"])
        var output := FileAccess.open(dump, FileAccess.WRITE)
        if output != null:
            output.store_buffer(snapshot)
            output.close()
            result["divergent_snapshot"] = dump
        _fail("chapter %s: native slot bytes differ from the chapter save at frame %d (first differing byte %d)" % [point["label"], consumed, first])
        return false
    print("native-tape: chapter %s matches at frame %d (%d ms)" % [point["label"], consumed, result["elapsed_ms"]])
    _write_report()
    return true

func _sha256_hex(bytes: PackedByteArray) -> String:
    var context := HashingContext.new()
    context.start(HashingContext.HASH_SHA256)
    context.update(bytes)
    return context.finish().hex_encode()

func _drive_title(phase: int, window: int, cursor: int) -> void:
    if title_down:
        _apply_pad(0)
        title_down = false
        return
    if phase <= 2: # Sega, reveal, Press Start: ordinary Start presses.
        _apply_pad(0x80)
    elif phase == 3:
        if report["start_kind"] == "new-game" and window == 2 and cursor == 0:
            _apply_pad(0x02) # START is row 1 when a save exists.
        elif report["start_kind"] == "save" and window != 2:
            _fail("verified save is not visible in the CONTINUE title window")
            return
        else:
            _apply_pad(0x80)
    elif phase == 4 and report["start_kind"] == "save":
        if cursor != 0:
            _fail("CONTINUE did not select isolated slot 1")
            return
        _apply_pad(0x80)
    else:
        _fail("unexpected title phase %d for %s" % [phase, report["start_kind"]])
        return
    title_down = true
    report["title_input_edges"] = int(report.get("title_input_edges", 0)) + 1

func _apply_game_pad(index: int) -> void:
    var value := int(pads[index])
    if index == int(OS.get_environment("PSIV_TAPE_DROP_AT")) and OS.has_environment("PSIV_TAPE_DROP_AT"):
        report["dropped_pad_at"] = index
        value = 0
    _apply_pad(value)

func _apply_pad(value: int) -> void:
    for bit in range(ACTIONS.size()):
        if value & (1 << bit):
            Input.action_press(ACTIONS[bit])
        else:
            Input.action_release(ACTIONS[bit])

func _freeze_at_end() -> void:
    game.set_physics_process(false)
    _apply_pad(0)
    frozen_checks = 1
    report["frames_consumed"] = consumed

func _finish() -> void:
    var boundary: PackedInt64Array = game.debug_tape_boundary()
    if int(boundary[0]) != expected_count:
        _fail("frozen boundary differs from declared frame count")
        return
    if next_checkpoint != checkpoints.size():
        _fail("replay ended before checkpoint %s" % checkpoints[next_checkpoint]["label"])
        return
    report["post_stop_callbacks_checked"] = frozen_checks - 1
    report["post_stop_session_frames"] = int(boundary[0])
    var save_acks: PackedInt64Array = game.debug_tape_save_acks()
    if save_acks.size() != 3:
        _fail("native camp SAVE acknowledgement probe is unavailable")
        return
    report["camp_save_acks"] = [int(save_acks[0]), int(save_acks[1]), int(save_acks[2])]
    var required_slot := OS.get_environment("PSIV_TAPE_REQUIRE_SAVE_SLOT")
    if not required_slot.is_empty():
        var slot := int(required_slot)
        if slot < 0 or slot >= save_acks.size() or int(save_acks[slot]) < 1:
            _fail("ordinary camp SAVE acknowledgement absent for slot %d" % [slot + 1])
            return
    var snapshot: PackedByteArray = game.debug_slot_bytes(0)
    if snapshot.is_empty():
        _fail("read-only slot snapshot is unavailable")
        return
    var snapshot_path := OS.get_environment("PSIV_TAPE_SNAPSHOT_OUT")
    var output := FileAccess.open(snapshot_path, FileAccess.WRITE)
    if output == null:
        _fail("cannot write isolated snapshot: " + snapshot_path)
        return
    output.store_buffer(snapshot)
    output.close()
    report["snapshot_bytes"] = snapshot.size()
    var expected_path := OS.get_environment("PSIV_TAPE_EXPECT_SAVE")
    if not expected_path.is_empty():
        if not FileAccess.file_exists(expected_path):
            _fail("expected chapter save is missing")
            return
        if snapshot != FileAccess.get_file_as_bytes(expected_path):
            _fail("native slot bytes differ from expected chapter save")
            return
        report["snapshot_matches_chapter"] = true
    var state = JSON.parse_string(str(game.debug_play_state()))
    if typeof(state) != TYPE_DICTIONARY:
        _fail("endpoint read-only state probe is unavailable")
        return
    report["endpoint"] = {"map": state["map"], "cell": state["cell"], "title": state["title"], "scene": state["scene"]}
    var expected_map := OS.get_environment("PSIV_TAPE_EXPECT_MAP")
    if not expected_map.is_empty() and int(expected_map) != int(state["map"]):
        _fail("endpoint map mismatch: expected %s, got %s" % [expected_map, state["map"]])
        return
    var expected_cell := OS.get_environment("PSIV_TAPE_EXPECT_CELL")
    if not expected_cell.is_empty():
        var parts := expected_cell.split(",")
        var actual: Array = state["cell"]
        if parts.size() != 2 or actual.size() != 2 or \
                int(parts[0]) != int(actual[0]) or int(parts[1]) != int(actual[1]):
            _fail("endpoint cell mismatch: expected %s, got %s" % [expected_cell, actual])
            return
    var capture := OS.get_environment("PSIV_TAPE_CAPTURE")
    if not capture.is_empty():
        var image := game.get_viewport().get_texture().get_image()
        if image == null or image.save_png(capture) != OK:
            _fail("selected rendered capture failed")
            return
        report["capture"] = capture
    report["result"] = "pass"
    _write_report()
    print("native-tape: PASS %d pads; map %s cell %s" % [consumed, state["map"], state["cell"]])
    quit(0)

func _fail(message: String) -> void:
    if failed:
        return
    failed = true
    if game != null:
        game.set_physics_process(false)
    _apply_pad(0)
    report["result"] = "fail"
    report["error"] = message
    report["frames_consumed"] = consumed
    _write_report()
    push_error("native-tape: " + message)
    quit(1)

func _write_report() -> void:
    var path := OS.get_environment("PSIV_TAPE_REPORT")
    if path.is_empty():
        return
    var output := FileAccess.open(path, FileAccess.WRITE)
    if output != null:
        output.store_string(JSON.stringify(report, "  ") + "\n")
        output.close()
