# Spend battle-earned meseta on carbon suits through the existing shop and
# equipment menus. The party already paid for its final training rest.
extends "res://../tools/native/native_zema_outfit.gd"

func configure_route():
    route = [[0,99,82,0x24],[0x24,26,31,0x26],[0x26,35,32,0x26],
        [0x26,31,40,0x24],[0x24,31,50,0]]
    start_map = 0
    start_flag = 0x37
    start_label = "CONTINUE-Zema-trained"
    end_label = "Zema-trained-and-armoured"
    boss_check_leg = -1
    healing = false

func equipment_plan():
    return [[1,3,"CRBN-SUIT"],[0,3,"CRBN-SUIT"]]

func before_route(state):
    if leg == 0 and not left_town_footprint:
        if state.cell[1] < 84:
            press("ui_down")
            return true
        left_town_footprint = true
    if leg == 2 and state.cell == [35,32] and not leg in completed_counters:
        buy_input(state,{"facing":"ui_right","items":[[11,550],[11,550]]})
        return true
    if leg == 3 and not outfit_complete:
        equip_input(state)
        return true
    return false
