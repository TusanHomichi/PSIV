# Bounded connected proof: one encounter, normal recovery, then ordinary SAVE.
extends "res://../tools/native/native_bioplant.gd"

func configure_route():
    super.configure_route()
    end_label = "BioPlant-recovery-checkpoint"

func before_route(state):
    if battle_count > 0 and not healing:
        if battle_count != 1 or poison_cures != 2 or state.party_status.any(func(p): return p.hp <= 0 or int(p.status) & 0x45):
            fail("bounded recovery did not finish one battle and two poison cures",state)
            return true
        route = []
        leg = 0
        return false
    return super.before_route(state)
