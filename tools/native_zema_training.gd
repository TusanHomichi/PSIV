# Ordinary Birth Valley battles and paid Zema rests. No generated XP or stats.
extends "res://../tools/native_bioplant.gd"

var training_phase := "outbound"
var inn_phase := 0
var inn_money := 0
var rest_count := 0
var patrol_count := 0
var target_level := 6
var current_map := -1
var cached_map := -1
var cached_target := Vector2i(-1,-1)
var cached_steps := {}

func configure_route():
    if OS.has_environment("PSIV_TRAIN_LEVEL"):
        target_level = int(OS.get_environment("PSIV_TRAIN_LEVEL"))
    route = [[0,99,82,0x24],[0x24,31,11,0x2B],[0x2B,33,15,0x2C]]
    start_map = 0
    start_flag = 0x37
    start_label = "CONTINUE-Zema-equipped"
    end_label = "Zema-trained-and-rested"
    boss_check_leg = -1
    healing = false

func trained(state):
    return state.party_resources.all(func(p): return p.level >= target_level) and state.money >= 1180

func before_route(state):
    current_map = int(state.map)
    if not left_entrance and int(state.map) == 0:
        if state.cell[1] < 84:
            press("ui_down")
            return true
        left_entrance = true
    if training_phase == "train" and state.camp == null and (trained(state) or state.party_resources.filter(func(p): return int(p.id) in [0,2]).reduce(func(total,p): return total + p.tp,0) < 12):
        training_phase = "rest"
        route = [[0x2C,24,49,0x2B],[0x2B,17,53,0x24],[0x24,36,31,0x27],[0x27,33,34,0x27]]
        leg = 0
        healing = false
        checkpoint("training-return-%02d" % rest_count,state)
    if training_phase == "rest" and int(state.map) == 0x27 and state.cell == [33,34]:
        rest_input(state)
        return true
    if healing and field_healing_input(state): return true
    if training_phase != "done" and leg >= route.size():
        training_phase = "train"
        route = [[0x2C,23,17,0x2C],[0x2C,24,46,0x2C]]
        leg = 0
        patrol_count += 1
        checkpoint("training-patrol-%02d" % patrol_count,state)
    return false

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
        if inn_phase != 3:
            fail("training inn did not open",state)
            return
        rest_count += 1
        checkpoint("training-paid-rest-%02d" % rest_count,state)
        inn_phase = 0
        healing = false
        leg = 0
        if state.party_resources.all(func(p): return p.level >= target_level) and state.money >= 1100:
            training_phase = "done"
            route = [[0x27,33,40,0x24],[0x24,31,50,0]]
        else:
            training_phase = "outbound"
            route = [[0x27,33,40,0x24],[0x24,31,11,0x2B],[0x2B,33,15,0x2C]]
    elif shop.mode == "InnGreeting": press("ui_cancel" if inn_phase == 3 else "ui_accept")
    elif shop.mode == "InnConfirm": choose(shop.confirm,0)
    elif shop.mode == "Message":
        if int(state.money) != inn_money - 80 or state.party_status.any(func(p): return p.hp != p.max_hp or p.status != 0) or state.party_resources.any(func(p): return p.tp != p.max_tp):
            fail("training inn charge or recovery mismatch",state)
            return
        inn_phase = 3
        press("ui_accept")
    else: fail("unexpected training inn state",state)
    cooldown = 8

func after_battle():
    cached_steps.clear() # Re-read collision after the field's reload.
    super.after_battle()

func first_step(start,target):
    # Cache a walking path between patrol endpoints. Every step still uses
    # ordinary input; this avoids solving the same corridor every frame.
    if current_map == cached_map and target == cached_target and start in cached_steps:
        return cached_steps[start]
    var map = JSON.parse_string(game.debug_walk_map())
    cached_map = current_map
    cached_target = target
    cached_steps = {target:-1}
    var queue = [target]
    var head := 0
    while head < queue.size():
        var at = queue[head]
        head += 1
        for i in range(4):
            var next = at + DIRECTIONS[i]
            if next.x < 0 or next.y < 0 or next.x >= map.width or next.y >= map.height or next in cached_steps or not map.walkable[next.y][next.x]: continue
            var doorway := false
            for rect in map.warps:
                var area = Rect2i(rect[0],rect[1],rect[2],rect[3])
                if area.has_point(next) and not area.has_point(target) and not area.has_point(start):
                    doorway = true
                    break
            if doorway: continue
            cached_steps[next] = i ^ 1
            if next == start: return cached_steps[next]
            queue.append(next)
    return -1
