# Continue the connected post-Rika save through Zema's inn and the opened
# northern Motavia crossing using ordinary field, menu, battle, and save input.
extends "res://../tools/native_bioplant.gd"

const EXPECTED_PARTY = [4, 1, 0, 2, 5]
const SOURCE_KEYS = ["map", "cell", "money", "flags", "party_status",
    "party_resources", "inventory", "leader"]

var source_snapshot = {}
var source_verified := false
var footprint_left := false
var inn_phase := 0
var inn_money := 0
var inn_yes_count := 0
var rest_complete := false
var crossing_seen := false

func configure_route():
    route = [
        [0,99,82,0x24], [0x24,36,31,0x27], [0x27,33,34,0x27],
        [0x27,33,40,0x24], [0x24,31,50,0], [0,84,68,0],
        [0,84,67,0], [0,84,65,0], [0,84,64,0],
    ]
    start_map = 0
    start_flag = 0x35
    start_label = "CONTINUE-post-Rika"
    end_label = "Motavia-north-bank"
    boss_check_leg = -1
    healing = false

func before_route(state):
    if not source_verified:
        source_snapshot = state.duplicate(true)
        if not verify_source(state): return true
        source_verified = true
        checkpoint("post-Rika-source", state)

    # Match BioPlant's ordinary post-battle recovery order without its
    # dungeon-specific doors, elevators, and intermediate saves.
    if healing and field_poison_input(state): return true
    if healing and field_healing_input(state): return true

    if leg == 0 and int(state.map) == 0 and not footprint_left:
        if state.cell != [99,84]:
            if state.cell != [99,83]:
                fail("unexpected cell before leaving Zema's entrance footprint", state)
            else:
                press("ui_down")
            return true
        footprint_left = true

    if leg == 2 and not rest_complete and state.cell == [33,34]:
        rest_input(state)
        return true

    if rest_complete and not crossing_seen and int(state.map) == 0 and state.cell == [84,67]:
        crossing_seen = true
        checkpoint("northern-crossing", state)
        capture.call_deferred("northern-crossing.png")

    if leg >= route.size():
        if not validate_finish(state): return true
    return false

func verify_source(state):
    if not OS.has_environment("PSIV_POST_RIKA_SOURCE_RECEIPT"):
        fail("PSIV_POST_RIKA_SOURCE_RECEIPT is required", state)
        return false
    var receipt_path = OS.get_environment("PSIV_POST_RIKA_SOURCE_RECEIPT")
    if receipt_path.is_empty() or not FileAccess.file_exists(receipt_path):
        fail("post-Rika source receipt is missing", state)
        return false
    var receipt = JSON.parse_string(FileAccess.get_file_as_string(receipt_path))
    if typeof(receipt) != TYPE_DICTIONARY or not receipt.get("complete", false):
        fail("post-Rika source receipt is incomplete or malformed", state)
        return false
    var checkpoints = receipt.get("checkpoints", [])
    if checkpoints.is_empty() or typeof(checkpoints[-1]) != TYPE_DICTIONARY:
        fail("post-Rika source receipt has no final checkpoint", state)
        return false
    var expected = checkpoints[-1].get("state", null)
    if typeof(expected) != TYPE_DICTIONARY:
        fail("post-Rika source receipt has no final state", state)
        return false
    # native_route normalizes these integer arrays before calling this hook;
    # JSON.parse_string leaves the saved receipt's numeric values as floats.
    for key in ["cell", "flags"]:
        if expected.has(key):
            expected[key] = expected[key].map(func(value): return int(value))
    for key in SOURCE_KEYS:
        if not expected.has(key) or not state.has(key) or state[key] != expected[key]:
            fail("post-Rika source differs from its receipt at " + key, state)
            return false
    var party_ids = state.party_status.map(func(p): return int(p.id))
    var resource_ids = state.party_resources.map(func(p): return int(p.id))
    if int(state.map) != 0 or state.cell != [99,83] or int(state.money) != 1203:
        fail("unexpected post-Rika starting map, cell, or meseta", state)
        return false
    if not 0x35 in state.flags or party_ids != EXPECTED_PARTY or resource_ids != EXPECTED_PARTY:
        fail("post-Rika source lacks the opened crossing or expected party order", state)
        return false
    return true

func rest_input(state):
    var shop = state.shop
    if inn_phase == 0:
        inn_money = int(state.money)
        press("ui_up")
        inn_phase = 1
    elif inn_phase == 1:
        press("ui_accept")
        inn_phase = 2
    elif shop == null:
        if inn_phase != 5:
            fail("Zema inn closed before its paid-rest receipt", state)
            return
        rest_complete = true
        inn_phase = 6
        checkpoint("Zema-paid-rest", state)
    elif shop.mode == "InnGreeting":
        if inn_phase == 2:
            press("ui_accept")
            inn_phase = 3
        elif inn_phase == 5:
            press("ui_cancel")
        else:
            fail("unexpected Zema inn greeting", state)
    elif shop.mode == "InnConfirm" and inn_phase == 3:
        if shop.confirm == 0:
            inn_yes_count += 1
            inn_phase = 4
        choose(shop.confirm, 0)
    elif shop.mode == "Message" and inn_phase == 4:
        if inn_yes_count != 1 or int(state.money) != inn_money - 100:
            fail("Zema inn did not charge exactly 100 meseta once", state)
            return
        if state.inventory != source_snapshot.inventory or state.flags != source_snapshot.flags:
            fail("Zema inn changed source inventory or event flags", state)
            return
        if state.party_status.map(func(p): return int(p.id)) != EXPECTED_PARTY:
            fail("Zema inn changed party order", state)
            return
        if state.party_status.any(func(p): return p.hp != p.max_hp or int(p.status) != 0):
            fail("Zema inn did not fully restore HP and status", state)
            return
        if state.party_resources.any(func(p): return p.tp != p.max_tp or p.skill_uses != p.max_skill_uses):
            fail("Zema inn did not fully restore TP and skill uses", state)
            return
        inn_phase = 5
        checkpoint("Zema-inn-receipt", state)
        capture.call_deferred("Zema-inn-receipt.png")
        press("ui_accept")
    else:
        fail("unexpected Zema inn menu", state)
    cooldown = 8

func validate_finish(state):
    if not rest_complete or inn_yes_count != 1:
        fail("final route reached without exactly one completed Zema rest", state)
        return false
    if int(state.map) != 0 or state.cell != [84,64] or not crossing_seen:
        fail("route did not finish across the northern bridge at (84,64)", state)
        return false
    if state.party_status.map(func(p): return int(p.id)) != EXPECTED_PARTY:
        fail("final party order differs from the connected source", state)
        return false
    if state.party_resources.map(func(p): return int(p.id)) != EXPECTED_PARTY:
        fail("final resource order differs from the connected source", state)
        return false
    if state.party_status.any(func(p): return p.hp <= 0 or int(p.status) != 0):
        fail("final party is not alive and free of persistent status", state)
        return false
    if state.inventory != source_snapshot.inventory or state.flags != source_snapshot.flags:
        fail("final inventory or event flags differ from the connected source", state)
        return false
    if int(state.money) < 1103:
        fail("final meseta fell below the post-inn balance", state)
        return false
    return true

func save_input(state):
    if state.camp != null and state.camp.mode == "SaveResult":
        if state.camp.message != "FILE SAVED":
            fail("final SAVE did not report FILE SAVED", state)
            return
        var save_dir = OS.get_environment("PSIV_SAVE_DIR")
        if save_dir.is_empty() or not FileAccess.file_exists(save_dir.path_join("slot_1.sram")):
            fail("final SAVE did not produce slot_1.sram", state)
            return
    super.save_input(state)
