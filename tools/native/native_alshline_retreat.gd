# The connected Alshline quest using ordinary RUN for random encounters.
# A retrieval mission can retreat while retaining normal encounter costs.
extends "res://../tools/native/native_alshline.gd"

func battle_input(battle):
    if not should_retreat(battle):
        super.battle_input(battle)
        return
    if battle.message == "THREAD" and not "thread" in scene_shots:
        scene_capture("thread", JSON.parse_string(game.debug_play_state()))
    if battle.finishing:
        press("ui_accept")
        cooldown = 6
    elif battle.ready:
        if battle.menu != null:
            press("ui_cancel")
        else:
            choose(int(battle.cursor), 2)
        cooldown = 6

func should_retreat(_battle):
    return true
