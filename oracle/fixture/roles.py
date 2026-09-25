"""What each of an action's RNG calls was for.

The trace says how many calls a frame held, not what they were for, so the
roles come from the cartridge's own structure plus what the RAM log then shows:

* a frame of exactly sixteen calls - or of several runs of sixteen - is
  `Battle_CalculateDamage` (`ps4.asm:17381`: `moveq #$F, d7` with `dbf`), one
  run per target, and the runs follow the acting side's fighters in slot order;
* a party action's other calls are `loc_B6A2`'s hit pass (`ps4.asm:17492`), one
  `Battle_CalculateChances` per target the swing covers, and
  `AlysKyraAttack_Init` (`ps4.asm:13975`) runs that pass a second time for the
  swing it animates. The attacker set is the *character*, not the weapon:
  `Character_AttackActionOffs` (`ps4.asm:13056-13068`) sends `Character_Stats`
  indices 1 and 9 - Alys and Kyra - to `CharAttack_AlysKyra` (`ps4.asm:13958`),
  and no other entry's routine calls `loc_B6A2`; the fixture's party list is
  what says which slot the character occupies, and a vehicle's party-side
  fighter is neither of them (`CHARACTER_SECOND_HIT_PASS`);
* an enemy's calls are `Enemy_Attack`'s ability roll (`ps4.asm:19146`),
  repeated while it equals `$FFFFEEA8`, and then - **only** for a basic attack
  - the hit pass. An ability's arm loads its own battle object and its effect
  dispatcher damages the target without a chances roll, and the arm that does
  not exist spends the turn outright (`loc_10406`, `ps4.asm:22781`), so an
  ability turn's frames hold the roll, its re-rolls and then the damage runs of
  `abilities in oracle/fixture/enemies.py`.

A count that matches none of those shapes is labelled `action` with no
target, so a reader can see that nothing was claimed about it.
"""
from .rolls import DAMAGE_RUN, HIT_NOT_TARGETED

#: The names whose attack route runs the close-range animation's `loc_B6A2`
#: pass as well as `Character_Attack`'s own (`CharAttack_AlysKyra`,
#: `ps4.asm:13958`; the mapping table's Alys/Kyra entries, `ps4.asm:13057-
#: 13067`), by the name the fixture's party list carries. It is what says the
#: two passes of one swing arrive in the *same* frame; every other fighter's
#: passes arrive one per frame, which `hit_pass_labels` reads from the frames
#: themselves.
ANIMATION_HIT_PASS_NAMES = {"ALYS", "KYRA"}

#: The party-side fighter ids: ids 1..=5 are the party (`cmpi.w #5, d0 / bgt`).
PARTY_SIDE_MAX = 5


def animation_hit_pass_actors(party):
    """The one-based slot ids whose swings take `loc_B6A2`'s second pass.

    A vehicle battle's party list is empty and its single party-side fighter is
    the vehicle, which is neither Alys nor Kyra, so it never takes the
    *animation's* second pass - the passes its own swing draws are its attack
    object's (`rust/psiv-core/src/battle/vehicle_attack.rs`'s `hit_passes`, two
    for the Ice Digger and three for the others). The port's own rule keys on
    the roster's character index the same way
    (`rust/psiv-core/src/battle/action.rs`'s `takes_second_hit_pass`).
    """
    return {entry["id"] for entry in party
            if entry["name"] in ANIMATION_HIT_PASS_NAMES}


def resolved_targets(record):
    """The slots the log shows this action resolving something on, in order.

    A slot whose **damage word moved** is one the action damaged, and that is
    the direct evidence of where its damage run belongs. The hit byte is the
    fallback for an action that damaged nobody (a swing that missed every slot
    it covered), because it is not always readable: `loc_B6A2` presets all nine
    flags before each pass, and between two actions the array carries what the
    last reader left there - formation `$03`'s second Crawler (sweep capture
    2026-09-24) reads `00`, `00` and `03` in slots 1-3 on the frame its swing
    opens, one of which is not even a verdict byte. Labelling that swing's
    damage run from the bytes gave the first slot while the damage landed on
    the second, which `account_for_every_roll` refused.
    """
    damaged = [target for target in record["targets"] if target["damage"]]
    if damaged:
        return damaged
    return [target for target in record["targets"]
            if target["hit"] != HIT_NOT_TARGETED]


def hit_pass_labels(count, known, first_pass):
    """The hit-pass calls one frame of a party swing holds.

    `known` is what the log shows the action resolving; `first_pass` is the
    pass number the frame's first call belongs to, which the action's earlier
    frames own (`record["passes_done"]`). One frame holds a whole number of
    passes over the slots a swing covers - `loc_B6A2` rolls once per covered
    slot per pass - so the calls are read as `count / covered` passes over
    them, and the pass numbers follow in frame order.

    The covered set is what the log resolved, and it is often smaller than the
    slots a pass actually touched: `loc_B6A2` blanks every flag to `$FF` before
    each pass, so a slot the swing missed that already read `$FF` leaves
    nothing behind, and a swing that resolved nothing at all has no covered set
    the log can name. Those calls are labelled `hit` with a `null` slot - one
    pass over `count` unknown slots - rather than given a target the log does
    not show. A count that is not a whole number of passes over the resolved
    set claims nothing at all.
    """
    if count <= 0:
        return None
    if known:
        per_pass = len(known)
        if count % per_pass:
            return None
        covered = [target["id"] for target in known]
    else:
        per_pass = count
        covered = [None] * count
    return [("hit", covered[index % per_pass], first_pass + index // per_pass)
            for index in range(count)]


def label_calls(record, frame, count):
    """`[(role, target, pass)]` for the `count` calls one frame held.

    `record` carries the actor, what the log shows it resolving, the action's
    own shape (`kind`, from `oracle/fixture/enemies.py`) and the actors that
    take the second hit pass.
    """
    if count and count % DAMAGE_RUN == 0:
        # The damage runs follow the acting side's fighters in slot order, so
        # the k-th run belongs to the k-th target the action resolved. One frame
        # can hold several: a two-enemy swing runs the routine once per target,
        # in the same frame.
        targets = [target["id"] for target in resolved_targets(record)]
        seen = sum(other[1] for other in record["rolls"]
                   if other[0] < frame) // DAMAGE_RUN
        labels = []
        for index in range(count // DAMAGE_RUN):
            position = seen + index
            labels += [("damage",
                        targets[position] if position < len(targets) else None,
                        0)] * DAMAGE_RUN
        return labels
    if record["actor"] <= PARTY_SIDE_MAX:
        labels = hit_pass_labels(count, resolved_targets(record),
                                 record["passes_done"] + 1)
        if labels is not None:
            return labels
        return [("action", None, 0)] * count
    # Enemy_Attack: the ability roll, repeated while it equals the word the
    # routine compares against, and then the hit pass - but only when the roll
    # landed on the basic attack. An ability's own arm never reaches
    # `loc_B6A2`.
    if count <= 0:
        return []
    if record["kind"] == "attack":
        labels = [("ability", None, 0)]
        labels += [("ability_reroll", None, 0)] * (count - 2)
        labels += [("hit", resolved_targets(record)[0]["id"]
                    if resolved_targets(record) else None, 1)]
        return labels[:count]
    return [("ability", None, 0)] + [("ability_reroll", None, 0)] * (count - 1)
