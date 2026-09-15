# STATE/ORDER through ordinary input, including undo, cancel and SAVE.
extends "res://../tools/native_motavia.gd"
var order_phase := 0
var before_order
const WANTED = [4,1,0,2]

func configure_route():
    route = []
    start_map = 0xA7
    start_flag = 0x37
    start_label = "CONTINUE-BioPlant-order"
    end_label = "BioPlant-Gryz-leading"
    boss_check_leg = -1
    healing = false

func before_route(state):
    if before_order == null: before_order = state.duplicate(true)
    if order_phase >= 6: return false
    var camp = state.camp
    if order_phase == 5:
        if camp == null: order_phase = 6
        else: press("ui_cancel")
    elif camp == null: press("ui_cancel")
    elif camp.mode == "Root": choose(camp.root,4)
    elif camp.mode == "State":
        if order_phase == 3:
            unchanged(state)
            checkpoint("order-cancelled",state)
            order_phase = 4
        choose(camp.state,1)
    elif camp.mode == "Order":
        if order_phase == 0:
            checkpoint("order-open",state)
            capture.call_deferred("order-open.png")
            order_phase = 1
        if order_phase == 1:
            if camp.order_chosen.is_empty(): choose(camp.order_cursor,camp.order_remaining.map(func(id): return int(id)).find(4))
            else:
                unchanged(state)
                checkpoint("order-picked",state)
                capture.call_deferred("order-picked.png")
                press("ui_cancel")
                order_phase = 2
        elif order_phase == 2:
            if not camp.order_chosen.is_empty() or camp.order_remaining.map(func(id): return int(id)) != [1,0,2,4]:
                fail("ORDER undo failed",state)
                return true
            unchanged(state)
            checkpoint("order-undone",state)
            capture.call_deferred("order-undone.png")
            press("ui_cancel")
            order_phase = 3
        elif order_phase == 4:
            choose(camp.order_cursor,camp.order_remaining.map(func(id): return int(id)).find(WANTED[camp.order_chosen.size()]))
    elif camp.mode == "OrderDone":
        if state.party_status.map(func(p): return int(p.id)) != WANTED or int(state.leader) != 4:
            fail("ORDER did not commit the requested lineup",state)
            return true
        var a = before_order.party_resources.duplicate(true)
        var b = state.party_resources.duplicate(true)
        a.sort_custom(func(x,y): return x.id < y.id)
        b.sort_custom(func(x,y): return x.id < y.id)
        if a != b: fail("ORDER changed character resources",state)
        checkpoint("order-committed",state)
        capture.call_deferred("order-committed.png")
        press("ui_accept")
        order_phase = 5
    else: fail("unexpected ORDER menu page",state)
    cooldown = 10
    return true

func unchanged(state):
    for key in ["map","cell","money","inventory","flags","leader","party_status","party_resources"]:
        if state[key] != before_order[key]:
            fail("ORDER cancellation changed "+key,state)
