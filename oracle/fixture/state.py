"""End-of-round state observed independently of transient hit/damage scratch."""
from .observations import HP_COLUMNS, victory_declared


def round_end_frame(log, first, last):
    """Observe completed combat before the separate victory reward sequence.

    loc_66B8 selects BattleRoutines $18 after death objects finish
    (ps4.asm:9760-9772). Item drops belong to Battle_CheckItemDrop
    (4831-4850), not the commands this round executed. Do not sample their
    inventory writes or later field/stat restoration as round effects.
    """
    return next((frame for frame in range(first, last + 1)
                 if frame in log.by_frame and victory_declared(log, frame)), last)


def round_state(log, frame, occupied):
    """Each fighter's stat cells at `frame`.

    An enemy slot whose `Obj_Fighters` word (`menu_object_N`) reads zero holds
    no fighter: its stats struct is whatever the last occupant left. That is a
    defeated enemy's, or - after Fusion's or COMBINE's reload (`loc_14D46`,
    `ps4.asm:29735`) cleared the slot - a fighter the battle no longer has, so
    it is not reported as state.
    """
    if not log.has("menu_party_0"):
        return None
    values = []
    names = {1: "alys", 2: "chaz", 3: "hahn"}
    for fighter in sorted(occupied):
        column = f"menu_object_{fighter}"
        if fighter > 5 and log.has(column) and log.num(frame, column) == 0:
            continue
        prefix = names[fighter] if fighter <= 5 else f"e{fighter - 5}"
        value = {"id": fighter, "hp": max(0, log.signed(frame, HP_COLUMNS[fighter])),
                 "status": log.num(frame, prefix + "_status") & 0x7F}
        if fighter <= 5:
            value["tp"] = log.num(frame, prefix + "_tp")
            if log.has(prefix + "_player_atk_bat"):
                for name in ("str_bat", "men_bat", "dex_bat", "atk_bat", "dfs_bat", "mdfs_bat"):
                    value[name] = log.num(frame, prefix + "_player_" + name)
                value["agi_bat"] = log.num(frame, prefix + "_agi_bat")
                for name, count in (("elements", 14), ("shadows", 14), ("uses", 8)):
                    column = {"elements": "element", "shadows": "shadow", "uses": "use"}[name]
                    value[name] = [log.num(frame, f"{prefix}_player_{column}_{i}") for i in range(count)]
        else:
            value["enemy_id"] = log.num(frame, prefix + "_id")
            value["mental_defence"] = log.num(frame, prefix + "_mdfs_bat")
            if log.has(prefix + "_atk_bat"):
                for name in ("str_bat", "men_bat", "agi_bat", "dex_bat", "atk_bat", "dfs_bat"):
                    value[name] = log.num(frame, prefix + "_" + name)
        values.append(value)
    return values
