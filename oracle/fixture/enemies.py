"""What an enemy's turn did: the ability it rolled, and what that left behind.

`Enemy_Attack` (`ps4.asm:19146`) runs the ability roll before anything else:
one `UpdateRNGSeed2` call masked to `$7` (`andi.w #7, d0`), **re-rolled while
the index it lands on equals the word at `$FFFFEEA8`** (`ps4.asm:19149-19151`),
which is why an ability turn's first frame can hold more calls than one index. The id that lands is written into the acting fighter object -
`move.b $58(a3,d0.w), ability+1(a4)` (`ps4.asm:19153`) - and that byte is what
`oracle/ram_map.json` logs as `eN_ability`:

    "the ability id enemy N executes on its turn ... $00 means no ability was
     written and the enemy is making its basic attack"

so the log decides *which* ability was used, and the trace decides which index
was drawn. The two are cross-checked by the Rust side: the index's own record
(`data.enemy(id).regular_abilities[index]`) has to be the id the log shows.

The *re-roll* is why the id byte alone is not enough to label a frame: the byte
is written once per turn, while the frame holds one call per draw. How many
calls an ability turn's frames hold is the model in `oracle/fixture/roles.py`,
and the class it belongs to is the one thing the log cannot state outright:

* `EnemyAttack_*` load an object and its effect dispatcher damages the target;
  those arms never reach `loc_B6A2`'s hit pass, so the ability's frame holds the
  ability roll and its re-rolls and nothing else, and the damage runs follow;
* an id whose arm does not exist - the FloatMine carriers' `$07` and `$17` -
  spends the turn with no effect at all (`loc_10406`, `ps4.asm:22781`), which
  the log shows as an ability id and no target movement;
* an id of `$00` is the basic attack, which does run the hit pass.

The extractor records the id, the frame the byte moved in, and whether anything
was left behind; `"kind"` is `"ability"`, `"wasted"` or `"attack"` accordingly,
and each is a claim the Rust comparator tests against the port's own event.
"""
from .errors import FixtureError

#: The four enemy slots the log names, as one-based fighter ids 6..=9.
ENEMY_SLOTS = (1, 2, 3, 4)

#: What an action was: a physical attack, a resolved ability, or an ability
#: that left the log with nothing to show.
KINDS = ("attack", "ability", "wasted")


def ability_column(slot):
    """The log's column for enemy `slot`'s executed ability id."""
    return f"e{slot}_ability"


def ability_used(log, slot, start, end):
    """The ability id enemy `slot` executed, and the frame the byte moved in.

    The byte is written once per ability roll and is *not* cleared by the next
    turn's roll: an enemy that rolls the same slot twice leaves it standing, so
    the reading is the value at the action's own start frame - the first frame
    the log shows the actor on - and `moved` says whether the byte changed
    inside the window (a change is what proves the write happened there).

    Raises [`FixtureError`] when the log carries no such column, because
    `oracle/ram_map.json` has it in the `enemy` group and a capture without
    that group cannot be read for abilities at all.
    """
    column = ability_column(slot)
    if not log.has(column):
        raise FixtureError(
            f"the log has no {column} column: rerun the oracle with the "
            f"enemy group to extract an enemy's ability")
    moved = [frame for frame in range(start, end + 1)
             if frame in log.by_frame and log.changed(frame, column)]
    return {
        "ability": log.num(start, column),
        "ability_frame": moved[0] if moved else None,
        "written": bool(moved),
    }


def kind_of(ability, targets, effects=None):
    """What the log's readings make of one enemy action.

    `ability` is the id the byte held when the turn opened - `Enemy_Attack`
    writes it (`move.b $58(a3,d0.w), ability+1(a4)`, `ps4.asm:19153`) before
    anything else, so the byte at the action's own first frame is this turn's
    roll and not the last one's - `targets` the slots the pass resolved, and
    `effects` what moved on any fighter (`observations.action_effects`).

    An id of zero is the basic attack. For a nonzero one the log decides
    whether the ability *did* anything - a resolved slot, a moved damage word,
    or any fighter's HP, status or battle stat cell moving inside the action -
    and says `wasted` when nothing did.

    That last word is the honest one and no more: the draws are no help, since
    `loc_10406` spends an id whose arm does not exist, and an arm that runs and
    finds nothing to do - a status the target already carries, which
    `AbilityEffect_Poison` returns on before its roll - draws no more than the
    roll itself. The two are the same log, so a fixture says `wasted` for both
    and the comparator takes either arm for it; what the comparator refuses is
    a turn that ran a *different* ability, resolved damage the log does not
    show, or never reached an arm at all
    (`rust/psiv-core/src/battle/replay/compare.rs`).
    """
    if not ability:
        return "attack"
    if targets or _moved(effects):
        return "ability"
    return "wasted"


def _moved(effects):
    """Whether any fighter's HP, status or battle stat cell moved."""
    if not effects:
        return False
    return any(effects.get(key) for key in ("hp", "status", "stats"))
