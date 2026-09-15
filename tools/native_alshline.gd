# Continue the input-verified Tonoe campaign: paid inn, Gryz's door,
# basement traversal, Alshline chest, and an ordinary save back in town.
extends "res://../tools/native_tonoe.gd"

func configure_route():
    route = [
        [0x43,30,40,0x41],
        [0x41,19,41,0x46],
        [0x46,20,32,0x46],
        [0x46,22,40,0x41],
        [0x41,31,11,0x42],
        [0x42,30,28,0x42,0x31,"talk"],
        [0x42,30,27,0x47],
        [0x47,32,15,0x48],
        [0x48,30,49,0x49],
        [0x49,48,53,0x4A],
        [0x4A,40,33,0x4A,0x32,"talk"],
        [0x4A,50,48,0x49],
        [0x49,28,52,0x48],
        [0x48,30,12,0x47],
        [0x47,32,46,0x42],
        [0x42,30,40,0x41],
    ]
    start_map = 0x43
    start_flag = 0x30
    start_label = "CONTINUE-Tonoe-Gryz"
    end_label = "Tonoe-Alshline-return"
    boss_check_leg = -1
    healing = false # Rest before spending the campaign's remaining TP.

func before_route(state):
    if state.camp != null and state.camp.mode.begins_with("Loot"):
        if state.camp.mode != "LootMessage":
            fail("campaign chest unexpectedly needs a full-pack decision",state)
            return true
        checkpoint("Alshline-chest",state)
        press("ui_accept")
        cooldown = 8
        return true
    if leg == 2 and state.cell == [20,32] and not rested and state.camp == null:
        if inn_phase == 0:
            inn_money = int(state.money)
            press("ui_up")
            inn_phase = 1
        elif inn_phase == 1:
            press("ui_accept")
            inn_phase = 2
        elif state.shop == null:
            if inn_phase == 4:
                rested = true
                healing = true
                healing_done = false
                checkpoint("Tonoe-paid-rest",state)
            else: fail("Tonoe inn did not open",state)
        elif state.shop.mode == "InnGreeting":
            press("ui_cancel" if inn_phase == 4 else "ui_accept")
        elif state.shop.mode == "InnConfirm": choose(state.shop.confirm,0)
        elif state.shop.mode == "Message":
            if int(state.money) != inn_money - 60:
                fail("Tonoe inn did not charge 15 per party member",state)
                return true
            inn_phase = 4
            scene_capture("Tonoe-inn-receipt",state)
            press("ui_accept")
        else: fail("unexpected Tonoe inn mode",state)
        cooldown = 10
        return true
    return super.before_route(state)

# Normal commands only. Use the party's actual learned support/kill skills,
# spend their real resources, and focus damage so injured foes stop attacking.
var planned_support := []
var command_skill := -1

func allow_instant_death():
    return true

func after_battle():
    planned_support.clear()
    super.after_battle()

func battle_input(battle):
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
        command_target = -1
        var injured = menu.party.filter(func(p): return p.hp > 0 and p.hp * 10 < p.max_hp * 7 and not int(p.id) in planned_heals)
        injured.sort_custom(func(a,b): return a.hp / a.max_hp < b.hp / b.max_hp)
        var available = menu.techniques.filter(func(t): return t.available).map(func(t): return int(t.id))
        var skills = menu.skills.filter(func(t): return t.available).map(func(t): return int(t.id))
        var targets = menu.enemies.duplicate()
        targets.sort_custom(func(a,b): return a.hp < b.hp)
        if not targets.is_empty(): command_target = int(targets[0].id)
        if 31 in available and not 31 in planned_support and targets.size() >= 3:
            command_spell = 31 # SANER once for initiative from round two onward.
            planned_support.append(31)
        elif not injured.is_empty() and 24 in available:
            command_spell = 24
            command_target = int(injured[0].id)
            planned_heals.append(command_target)
        elif 20 in available and not 20 in planned_support and targets.size() >= 3:
            command_spell = 20 # GELUN once: lower the opponents' attack power.
            planned_support.append(20)
        elif allow_instant_death() and 17 in available and targets.size() >= 3:
            command_spell = 17 # Gryz's BROSE uses its original 16 TP.
        elif allow_instant_death() and 34 in skills and not targets.is_empty() and targets[-1].hp >= 35:
            command_skill = 34
            command_target = int(targets[-1].id)
        elif 1 in available:
            command_spell = 1
        elif 7 in available:
            command_spell = 7 # Chaz's TSU, earned at level four.
        elif 4 in available:
            command_spell = 4 # Hahn's WAT, earned at level three.
        elif 31 in skills and targets.size() >= 3:
            command_skill = 31
            command_target = int(targets[-1].id)
        print("NATIVE ALSHLINE command character=",menu.character," tech=",command_spell," skill=",command_skill," target=",command_target)
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
        var wanted = menu.targets.map(func(id): return int(id)).find(command_target)
        choose(menu.cursor,max(0,wanted))
    else: fail("unexpected battle command page",menu)
    cooldown = 4
