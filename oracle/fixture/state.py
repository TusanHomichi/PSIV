"""End-of-round state observed independently of transient hit/damage scratch."""
from .observations import HP_COLUMNS


def round_state(log, frame, occupied):
    if not log.has("menu_party_0"):
        return None
    values = []
    names = {1: "alys", 2: "chaz", 3: "hahn"}
    for fighter in sorted(occupied):
        prefix = names[fighter] if fighter <= 5 else f"e{fighter - 5}"
        value = {"id": fighter, "hp": max(0, log.signed(frame, HP_COLUMNS[fighter])),
                 "status": log.num(frame, prefix + "_status") & 0x7F}
        if fighter <= 5:
            value["tp"] = log.num(frame, prefix + "_tp")
        else:
            value["enemy_id"] = log.num(frame, prefix + "_id")
            value["mental_defence"] = log.num(frame, prefix + "_mdfs_bat")
        values.append(value)
    return values
