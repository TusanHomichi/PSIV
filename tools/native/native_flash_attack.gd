# The flash-only enemy attack branch, natively (#92, #214). Started by
# tools/verify_native_flash_attack.py on a battle-window fixture battle
# (PSIV_DEBUG_BATTLE_WINDOW=top, PSIV_DEBUG_BATTLE_FORMATION): the party
# DEFENDs through COMD every round, so the enemies' ordinary turns play their
# attacks, until PSIV_FLASH_ATTACKS attacks (sheet or flash: the verifier
# requires each to be the flash-only branch of `EnemySprite::begin_attack`)
# have run and every body is at rest again. Each tick it records the enemy
# bodies' attack state (`EnemySprite::debug_attack`) and Godot's orphan-node
# count. The verdict is the verifier's, from the report written to
# PSIV_FLASH_REPORT; this script only drives and observes.
extends SceneTree

const DEFEND := 4
const SETTLE_TICKS := 30
const TIMEOUT_TICKS := 3000
# The selector starts its battle at tick 30.
const NO_BATTLE_TICKS := 300

var game
var tick := 0
var held := ""
var cooldown := 0
var wanted := 3
var report := {"complete": false, "reason": "", "attacks": [], "orphans_max": 0,
        "orphans_battle_start": -1, "orphans_end": -1, "rounds": 0}
var previous := {}
var attacks_seen := 0
var settle := -1
var in_battle := false
var stopped := false

func _initialize():
    wanted = int(OS.get_environment("PSIV_FLASH_ATTACKS"))
    call_deferred("start_game")

func start_game():
    game = load("res://field.tscn").instantiate()
    root.add_child(game)
    current_scene = game

func press(action):
    held = action
    Input.action_press(action)
    cooldown = 6

func orphans() -> int:
    return int(Performance.get_monitor(Performance.OBJECT_ORPHAN_NODE_COUNT))

# The orphan-node monitor must be alive for "no orphan" to mean anything: one
# unparented node must raise it by one (native_tape.gd's probe).
func orphan_monitor_moves() -> bool:
    var before := orphans()
    var node := Node.new()
    var moved := orphans() == before + 1
    node.free()
    return moved

func _physics_process(_delta):
    tick += 1
    if held != "":
        Input.action_release(held)
        held = ""
        return false
    if game == null or stopped:
        return false
    if tick > TIMEOUT_TICKS:
        stop("timed out")
        return false
    var raw = game.debug_play_state()
    if raw == "":
        return false
    var state = JSON.parse_string(raw)
    report["party"] = state.party_status
    var battle = state.battle
    if battle == null:
        if not in_battle and tick > NO_BATTLE_TICKS:
            stop("no battle started by tick %d" % NO_BATTLE_TICKS)
        elif in_battle:
            stop("the battle ended before %d attacks" % wanted)
        return false
    if not in_battle:
        in_battle = true
        report["orphans_battle_start"] = orphans()
        report["idle"] = battle.enemies
    report["orphans_max"] = max(report["orphans_max"], orphans())
    report["message"] = battle.message
    observe(battle.enemies)
    if settle >= 0:
        settle -= 1
        if settle < 0:
            report["orphans_end"] = orphans()
            report["final"] = battle.enemies
            report["complete"] = true
            stop("")
        return false
    if cooldown > 0:
        cooldown -= 1
        return false
    if attacks_seen >= wanted and battle.ready and all_clear(battle.enemies):
        settle = SETTLE_TICKS
        return false
    if battle.ready:
        if battle.menu == null:
            report["rounds"] += 1
            press("ui_accept")
        else:
            # The command strip steps with Left/Right and wraps over five icons
            # (native_enemy_attacks.gd).
            var cursor = int(battle.menu.cursor)
            press("ui_accept" if cursor == DEFEND else ("ui_right" if (DEFEND - cursor + 5) % 5 <= 2 else "ui_left"))
    return false

# Records every attack start, with the body's state and the orphan count on
# its first tick; the per-tick orphan maximum is `orphans_max`.
func observe(enemies):
    for enemy in enemies:
        var key = int(enemy.fighter)
        var kind = enemy.attack
        if kind != null and previous.get(key) != kind:
            attacks_seen += 1
            report["attacks"].append({"tick": tick, "fighter": key, "enemy_id": int(enemy.enemy_id),
                    "attack": kind, "tint": enemy.tint, "children": int(enemy.children),
                    "orphans": orphans()})
        previous[key] = kind

func all_clear(enemies) -> bool:
    for enemy in enemies:
        if enemy.attack != null:
            return false
    return true

func stop(reason):
    stopped = true
    report["reason"] = reason
    report["tick"] = tick
    report["orphan_monitor_alive"] = orphan_monitor_moves()
    var path := OS.get_environment("PSIV_FLASH_REPORT")
    FileAccess.open(path, FileAccess.WRITE).store_string(JSON.stringify(report, "  "))
    print("NATIVE FLASH ATTACK ", JSON.stringify(report))
    quit(0)
