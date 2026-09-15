# Actual title CONTINUE from the input-verified Academy save. Camp recovery,
# movement, battles and Holt's conversation all use ordinary Godot input.
extends "res://../tools/native_route.gd"

var healing := true
var healing_done := false
var heal_target := -1
var heal_caster := -1
var heal_count := 0
var heal_before := 0
var saw_acid := false

func _initialize():
    configure_route()
    super._initialize()

func configure_route():
    route = [
        [0, 61, 100, 0x1D],
        [0x1D, 20, 50, 0],
        [0, 99, 82, 0x24],
        [0x24, 31, 11, 0x2B],
        [0x2B, 33, 15, 0x2C],
        [0x2C, 23, 17, 0x2C, 16, "talk"],
        [0x24, 31, 50, 0],
    ]
    start_map = 0
    start_flag = 12
    start_label = "CONTINUE-Academy"
    end_label = "Motavia-after-Holt"
    boss_check_leg = -1

func after_battle():
    healing = true
    healing_done = false

func before_route(state):
    return field_healing_input(state)

func needs_field_healing(member):
    return member.hp * 4 < member.max_hp * 3

func field_healing_input(state):
    if not healing:
        return false
    var camp = state.camp
    if healing_done:
        if camp != null:
            press("ui_cancel")
            cooldown = 6
        else:
            healing = false
            healing_done = false
        return true
    if camp == null:
        press("ui_cancel")
        cooldown = 8
        return true
    if camp.mode == "AbilityResult":
        if camp.party[heal_caster].hp <= 0 or camp.party[heal_target].hp <= heal_before:
            fail("healing failed", state)
            return true
        heal_count += 1
        if heal_count > 50:
            fail("too many healing casts", state)
            return true
        checkpoint("healed-%02d" % heal_count, state)
        heal_target = -1
        heal_caster = -1
        press("ui_cancel")
        cooldown = 8
        return true
    if heal_target < 0:
        for member in camp.party:
            if member.hp <= 0 or int(member.status) & 0x44:
                fail("route needs revival", state)
                return true
        for i in range(camp.party.size()):
            var member = camp.party[i]
            if member.hp <= 0 or int(member.status) & 0x44:
                fail("route needs revival", state)
                return true
            if needs_field_healing(member):
                heal_target = i
                heal_before = int(member.hp)
                break
        if heal_target < 0:
            healing_done = true
            return true
        for i in range(camp.party.size()):
            var caster = camp.party[i]
            if int(caster.id) in [0, 2] and caster.hp > 0 and int(caster.status) & 0x46 == 0 and caster.tp >= 3:
                heal_caster = i
                break
        if heal_caster < 0:
            fail("route needs an inn", state)
            return true
    match camp.mode:
        "Root": choose(camp.root, 1)
        "AbilityCharacters": choose(camp.caster, heal_caster)
        "AbilityList":
            if int(camp.caster) != heal_caster:
                press("ui_cancel")
            else:
                var index := -1
                for i in range(camp.abilities.size()):
                    if int(camp.abilities[i].id) == 24:
                        index = i
                if index < 0:
                    fail("caster has no RES", state)
                else:
                    choose(camp.ability, index)
        "AbilityTarget": choose(camp.target, heal_target)
        _: fail("unexpected camp mode", state)
    cooldown = 8
    return true

func choose(current, desired):
    press("ui_down" if current < desired else ("ui_up" if current > desired else "ui_accept"))

func battle_input(battle):
    if battle.message == "ACIDBREATH" and not saw_acid:
        saw_acid = true
        capture.call_deferred("acid-breath.png")
    super.battle_input(battle)

func checkpoint(label, state):
    super.checkpoint(label, state)
    if label.begins_with("leg-") or label in [start_label, end_label, "healed-01"]:
        capture.call_deferred(label + ".png")
