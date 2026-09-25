# Connected equipped-Zema save to Rika, via ordinary doors, elevator input,
# random encounters and dialogue. The probes are read-only.
extends "res://../tools/native/native_motavia.gd"

const ELEVATORS = {0xA4:[[32,17]],0xA6:[[24,43],[40,43]],0xA7:[[70,11]],0xAA:[[28,27]]}
var left_entrance := false
var door_phase := 0
var shots := {}
var pages := []
var command_actor := -1
var command_spell := -1
var command_skill := -1
var command_target := -1
var planned_heals := []
var planned_support := []
var battle_observation := ""
var saved_maps := []
var checkpoint_closing := false
var campaign_party := []
var poison_target := -1
var poison_caster := -1
var poison_before
var poison_closing := false
var poison_cures := 0
var ordinary_dialogue_accepts := 0

func configure_route():
    route = [
        [0,99,82,0x24], [0x24,31,11,0x2B], [0x2B,33,15,0x2C],
        [0x2C,23,15,0xA2], [0xA2,32,21,0xA3], [0xA3,32,17,0xA4],
        [0xA4,32,17,0xA6], [0xA6,40,43,0xA7], [0xA7,70,11,0xA9],
        [0xA9,32,50,0xAA], [0xAA,28,27,0xAB], [0xAB,24,23,0xAC],
        [0xAC,31,27,0,0x35],
    ]
    start_map = 0
    start_flag = 0x37
    start_label = "CONTINUE-Zema-equipped"
    end_label = "BioPlant-Rika-escaped"
    boss_check_leg = -1
    healing = false
    if OS.has_environment("PSIV_BIOPLANT_RESUME_MAP"):
        start_map = int(OS.get_environment("PSIV_BIOPLANT_RESUME_MAP"))
        leg = route.find(route.filter(func(step): return step[0] == start_map)[0])
        start_label = "CONTINUE-BioPlant-%03X" % start_map
        left_entrance = true

func before_route(state):
    state["ordinary_dialogue_accepts"] = ordinary_dialogue_accepts
    if campaign_party.is_empty():
        campaign_party = state.party_status.map(func(p): return int(p.id))
    if leg == 0 and int(state.map) == 0 and not left_entrance:
        if state.cell[1] < 84:
            press("ui_down")
            return true
        left_entrance = true
    if healing and field_poison_input(state): return true
    if healing and field_healing_input(state): return true
    if checkpoint_closing:
        if state.camp != null:
            press("ui_cancel")
            cooldown = 6
            return true
        checkpoint_closing = false
    if int(state.map) in [0xA7,0xAA,0xAC] and not int(state.map) in saved_maps:
        save_checkpoint_input(state)
        return true
    if leg >= route.size():
        if not 0x34 in state.flags or not 0x35 in state.flags or state.party_status.map(func(p): return int(p.id)) != campaign_party + [5]:
            fail("Rika escape did not retain the campaign party and story flags",state)
            return true
        return false
    var step = route[leg]
    var target = [step[1],step[2]]
    var door = int(state.map) == 0x2C and target == [23,15] and not state.temp_flags.any(func(flag): return int(flag) == 0)
    var elevator = int(state.map) in ELEVATORS and target in ELEVATORS[int(state.map)]
    if elevator:
        var chunk = [int(target[0]/2),int((target[1]-1)/2),0x53]
        elevator = not state.chunk_patches.any(func(c): return int(c[0]) == chunk[0] and int(c[1]) == chunk[1] and int(c[2]) == chunk[2])
    if not door and not elevator:
        door_phase = 0
        return false
    var approach = Vector2i(target[0],target[1]+1)
    if Vector2i(state.cell[0],state.cell[1]) != approach:
        var direction = first_step(Vector2i(state.cell[0],state.cell[1]),approach)
        if direction < 0: fail("no path to closed door",state)
        else: press(ACTIONS[direction])
    elif door_phase == 0:
        scene_shot("door-%03X-%02d-closed" % [int(state.map),leg],state)
        press("ui_up")
        door_phase = 1
    elif door_phase == 1:
        press("ui_accept")
        door_phase = 2
        cooldown = 10
    else: fail("door interaction did not open its original chunks",state)
    return true

func field_poison_input(state):
    var camp = state.camp
    if poison_closing:
        if camp != null:
            press("ui_cancel")
            cooldown = 6
        else: poison_closing = false
        return true
    if poison_target < 0:
        var target := -1
        var caster := -1
        for i in range(state.party_status.size()):
            var member = state.party_status[i]
            if member.hp > 0 and int(member.status) & 1 and not int(member.status) & 0x44:
                target = i
                break
        if target < 0: return false
        for i in range(state.party_resources.size()):
            var member = state.party_status[i]
            var resource = state.party_resources[i]
            if member.hp > 0 and not int(member.status) & 0x46 and resource.tp >= 2 and resource.techniques.any(func(id): return int(id) == 34):
                caster = i
                break
        if caster < 0: return false # Leave ordinary recovery to report the shortage.
        poison_target = target
        poison_caster = caster
        poison_before = state.duplicate(true)
    if camp == null: press("ui_cancel")
    else:
        match camp.mode:
            "Root": choose(camp.root,1)
            "AbilityCharacters": choose(camp.caster,poison_caster)
            "AbilityList":
                if int(camp.caster) != poison_caster: press("ui_cancel")
                else:
                    var index = camp.abilities.map(func(a): return int(a.id)).find(34)
                    if index < 0: fail("poison recovery caster has no ANTI",state)
                    else: choose(camp.ability,index)
            "AbilityTarget": choose(camp.target,poison_target)
            "AbilityResult":
                for i in range(state.party_status.size()):
                    var expected = poison_before.party_status[i].duplicate(true)
                    # debug_play_state dictionaries contain floating-point numbers.
                    if i == poison_target: expected.status = float(int(expected.status) & ~1)
                    if state.party_status[i] != expected:
                        fail("ANTI changed HP or failed to clear only the target's poison",state)
                        return true
                    var resource = poison_before.party_resources[i].duplicate(true)
                    if i == poison_caster: resource.tp -= 2
                    if state.party_resources[i] != resource:
                        fail("ANTI did not charge exactly 2 TP to its caster",state)
                        return true
                poison_cures += 1
                checkpoint("poison-cured-%02d" % poison_cures,state)
                capture.call_deferred("poison-cured-%02d.png" % poison_cures)
                poison_target = -1
                poison_caster = -1
                poison_closing = true
                press("ui_cancel")
            _: fail("unexpected poison recovery menu",state)
    cooldown = 6
    return true

func save_checkpoint_input(state):
    var camp = state.camp
    if camp == null: press("ui_cancel")
    else:
        match camp.mode:
            "Root": choose(camp.root,4)
            "State": choose(camp.state,2)
            "SaveSlots": choose(camp.save,0)
            "SaveResult":
                if camp.message != "FILE SAVED":
                    fail("intermediate save failed",state)
                    return
                var label = "saved-map-%03X" % int(state.map)
                var save = OS.get_environment("PSIV_SAVE_DIR").path_join("slot_1.sram")
                if DirAccess.copy_absolute(save,directory.path_join(label+".sram")) != OK:
                    fail("could not preserve intermediate save",state)
                    return
                checkpoint(label,state)
                FileAccess.open(directory.path_join(label+".json"),FileAccess.WRITE).store_string(JSON.stringify({"checkpoints":[{"name":label,"state":state}],"complete":true},"  "))
                capture.call_deferred(label+".png")
                saved_maps.append(int(state.map))
                checkpoint_closing = true
                press("ui_cancel")
            _: fail("unexpected intermediate save page",state)
    cooldown = 6

func needs_field_healing(member):
    return member.hp * 10 < member.max_hp * 9

func after_battle():
    planned_support.clear()
    super.after_battle()

func battle_input(battle):
    # An opening ambush discards the party's queued actions. Count support
    # only when its actual cast appears, so the next turn can retry it.
    for support in [[31,"Alys: SANER"],[20,"Hahn: GELUN"]]:
        if battle.message == support[1] and not support[0] in planned_support:
            planned_support.append(support[0])
    var observation = JSON.stringify([battle.message,battle.menu])
    if observation != battle_observation:
        battle_observation = observation
        print("NATIVE BIOPLANT battle-state ",observation)
    if battle.menu == null:
        command_actor = -1
        planned_heals.clear()
    if not battle.ready or battle.menu == null or battle.menu.actor == null:
        super.battle_input(battle)
        return
    var menu = battle.menu
    if int(menu.actor) != command_actor:
        command_actor = int(menu.actor)
        command_spell = -1
        command_skill = -1
        var injured = menu.party.filter(func(p): return p.hp > 0 and p.hp * 10 < p.max_hp * 7 and not int(p.id) in planned_heals)
        injured.sort_custom(func(a,b): return a.hp / a.max_hp < b.hp / b.max_hp)
        var available = menu.techniques.filter(func(t): return t.available).map(func(t): return int(t.id))
        var skills = menu.skills.filter(func(t): return t.available).map(func(t): return int(t.id))
        var targets = menu.enemies.duplicate()
        targets.sort_custom(func(a,b): return a.hp < b.hp)
        command_target = int(targets[0].id) if not targets.is_empty() else -1
        var strong = not targets.is_empty() and targets[-1].hp >= 40
        var caster = menu.party.filter(func(p): return int(p.id) == command_actor)[0]
        if not injured.is_empty() and 24 in available:
            command_spell = 24
            command_target = int(injured[0].id)
            planned_heals.append(command_target)
        elif strong and targets.size() >= 3 and 31 in available and not 31 in planned_support:
            command_spell = 31 # SANER lets the party heal before the next volley.
        elif strong and targets.size() >= 2 and 20 in available and not 20 in planned_support:
            command_spell = 20 # GELUN for dangerous groups; preserve TP on weak bugs.
        elif strong and targets.size() >= 3 and 17 in available:
            command_spell = 17 # Gryz's BROSE attempts the whole group for 16 TP.
        elif strong and 34 in skills:
            command_skill = 34 # Gryz's CRASH, paid from the real skill-use pool.
            command_target = int(targets[-1].id)
        elif strong and 1 in skills:
            command_skill = 1 # CROSSCUT, earned by Chaz at level six.
            command_target = int(targets[-1].id)
        elif targets.size() == 1 and targets[0].hp >= 90 and 6 in skills:
            command_skill = 6 # VORTEX for a durable single foe.
        elif strong and 4 in available and caster.tp >= 24:
            command_spell = 4 # WAT, while retaining at least six RES casts.
        # Otherwise ATTACK: Alys's two slashers hit the group without TP.
        print("NATIVE BIOPLANT command character=",menu.character," tech=",command_spell," skill=",command_skill," target=",command_target)
    if menu.page == "Actions": choose(menu.cursor,2 if command_skill >= 0 else (1 if command_spell >= 0 else 0))
    elif menu.page == "Techniques":
        var wanted = menu.techniques.map(func(t): return int(t.id)).find(command_spell)
        if wanted < 0:
            fail("planned spell disappeared",menu)
            return
        choose(menu.cursor,wanted)
    elif menu.page == "Skills":
        var wanted = menu.skills.map(func(t): return int(t.id)).find(command_skill)
        if wanted < 0:
            fail("planned skill disappeared",menu)
            return
        choose(menu.cursor,wanted)
    elif menu.page.begins_with("Targets"):
        choose(menu.cursor,max(0,menu.targets.map(func(id): return int(id)).find(command_target)))
    else: fail("unexpected battle command page",menu)
    cooldown = 4

func _physics_process(delta):
    # The shared route driver deliberately waits while dialogue owns input.
    # Snapshot readiness before it releases a held key or consumes cooldown,
    # so a release tick cannot accidentally submit a second accept.
    var held_before := held != ""
    var cooldown_before := cooldown
    var dialogue_ready_before := false
    if game != null and not finished:
        var before_state = JSON.parse_string(game.debug_play_state())
        if before_state != null and before_state.get("dialogue", false) and before_state.get("dialogue_choice") == null:
            var page = before_state.get("dialogue_page")
            dialogue_ready_before = page != null and page.get("ready", false)
    var result = super._physics_process(delta)
    if game == null or finished: return result
    var state = JSON.parse_string(game.debug_play_state())
    if state == null: return result
    if state.game_over:
        fail("campaign party perished",state)
        return result
    if state.battle != null and "CROSSCUT" in state.battle.message:
        scene_shot("crosscut-%02d" % battle_count,state)
    if state.red_palette != null:
        scene_shot("alarm-%s-%d" % ["in" if state.red_palette[0] else "out",int(state.red_palette[1])],state)
    if state.scene and not state.chunk_patches.is_empty():
        var ids = state.chunk_patches.map(func(c): return "%02X" % int(c[2]))
        scene_shot("chunks-%03X-%s" % [int(state.map),"-".join(ids)],state)
    if int(state.map) in [0xA3,0xAC,0xAD,0x24] and state.dialogue_page != null and state.dialogue_page.ready:
        var key = JSON.stringify([state.dialogue_page.tree,state.dialogue_page.lines])
        if not key in pages:
            pages.append(key)
            scene_shot("dialogue-%03d" % pages.size(),state)
    if dialogue_ready_before and not held_before and cooldown_before <= 0 and held == "":
        press("ui_accept")
        cooldown = 4
        ordinary_dialogue_accepts += 1
        print("NATIVE BIOPLANT ordinary-dialogue-accept ",ordinary_dialogue_accepts)
    return result

func scene_shot(label,state):
    if label in shots: return
    shots[label] = true
    checkpoint(label,state)
    capture.call_deferred(label+".png")
