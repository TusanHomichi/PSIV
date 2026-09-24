//! The vehicle's own swing — command 6, fighter routine `$12` (`loc_AF9C`).
//!
//! A vehicle's Attack never runs `Character_Attack`. `loc_5B8E`
//! (`ps4.asm:8410-8416`) turns `Current_Command`'s command index into a fighter
//! routine through its own seven-entry table (`loc_5BD4`, `ps4.asm:8430`):
//! `$06, $0A, $0B, $10, $0F, $12, $13` for command indices 1..7. Index 1 —
//! the ordinary Attack — is `$06` = `Character_Attack`, and index 6, which is
//! the one the vehicle menu writes, is `$12` = `loc_AF9C`
//! (`BattleCharacterRoutinePtrs`, `ps4.asm:1051`). `loc_AF9C` is a routine of
//! its own, listed in the same table as `loc_B1D4` (`ps4.asm:1052`), which
//! command 7 — the vehicle menu's second option — selects.
//!
//! That is why the vehicle's weapon check never happens. `Character_Attack`'s
//! `right_hand` / `left_hand` test (`ps4.asm:13002-13019`) belongs to command 1,
//! and its dispatch table `Character_AttackActionOffs` (`ps4.asm:13056`) is
//! indexed by `fighter_id - 1`, i.e. by `Vehicle_Index + $B - 1` = `$0B` for
//! the Land Rover (`loc_78EE`, `ps4.asm:11408-11414` seats `fighter_id =
//! Vehicle_Index + $B`) — one past that table's eleven characters. Nothing
//! reaches it: a vehicle's fighter routine is `$12` for its whole turn.
//!
//! # The swing: three hit passes, the last one deciding
//!
//! `loc_AF9C` (`ps4.asm:16810-16814`) stores the command's **low byte** —
//! `loc_684A` (`ps4.asm:9852-9866`) writes `6` and then `($FFFFF43D).w`, the
//! vehicle index — in the actor's `ability` (`$24`), and drives the ten-state
//! machine `loc_AFE4` (`ps4.asm:16831-16841`). Three of its frames call
//! `loc_B6A2`:
//!
//! 1. state 4, `loc_B14C` → `loc_B166` (`ps4.asm:16957-16979`): the vehicle
//!    attack object `$644`/`$648`/`$64C` is created and the routine **jumps**
//!    straight into `loc_B6A2`;
//! 2. state 5, `loc_9848` (`ps4.asm:14964-14965`): `jsr loc_B6A2`, then the
//!    targets are put into their damage animation;
//! 3. that object's own first state hands the vehicle's `action_routine` back
//!    to 5 — `move.w #5, $32(a0)` — once its wind-up has run (12 frames for
//!    the Land Rover and the Hydrofoil, none for the Ice Digger), so `loc_9848`
//!    runs a third time.
//!
//! `loc_B6A2` blanks all nine `Fighters_Hit_Flags` before every pass
//! (`ps4.asm:17493-17496`), so each pass overwrites the last one's verdicts;
//! the **third** pass is what `Fighter_TakeDamage` reads. The object's second
//! state then sets the vehicle's `action_routine` to 7 (`move.w #7, $32(a0)`,
//! `ps4.asm:82986` and its two siblings), and state 7 `loc_B1A6`
//! (`ps4.asm:16999-17008`) puts the target into routine `$C` — the frame whose
//! `loc_266C` runs `Battle_CalculateDamage`. The attack command's own check is
//! what keeps the rolls: `loc_B6D4` routes command 6 to the rolling path before
//! the `tst.w $24(a4)` that would have turned the nonzero ability into a
//! free hit (`ps4.asm:17513-17516`).
//!
//! # The damage: the target's own energy property
//!
//! The ability stored in `$24(a4)` is nonzero, so `Figher_DamageCheckActor`
//! (`ps4.asm:3737-3749`) reaches `Character_DamageEnemy` (`ps4.asm:3910`) and
//! its `bne.w loc_27D4` arm (`ps4.asm:3917-3918`) — the ability path, not the
//! weapon one. `loc_27D4` (`ps4.asm:3988-3998`) takes the command index,
//! subtracts two, and indexes the table at `loc_27FC` (`ps4.asm:4004-4010`);
//! command 6 selects the fifth entry, `loc_280A` (`ps4.asm:4016-4018`), whose
//! whole body is
//!
//! ```text
//! loc_280A:
//!     moveq   #0, d3
//!     move.b  $32(a1), d3     ; the target's element-2 property byte
//!     bra.s   loc_27A4
//! ```
//!
//! `a1` is the **target's** stats, and `$32` is the second element word of
//! `element_props` (`$30`), i.e. `Stats::element_factor(2)` — energy. So a
//! vehicle's cannon is energy-elemental and carries no weapon element at all:
//! `loc_280A` never reads the actor's hands, which is also why the record built
//! by `loc_77AE` (`$4C`/`$4D` are equipment; `ps4.asm:11295-11404` writes none)
//! cannot change it. `loc_27A4` (`ps4.asm:3963-3969`) then supplies
//! `atk_pow_battle` of the actor, `dfs_pow_battle` of the target and the
//! critical bonus `atk >> 2` when the hit flag is `$01`, and jumps to `loc_266C`
//! — `Battle_CalculateDamage` (`ps4.asm:17374`) and the 1..=999 clamp, the same
//! sixteen draws every other damage path spends.
//!
//! # Why one target, and why no critical demotion
//!
//! `loc_B6A2` reads `Current_Target_Index` and takes the four-enemy window only
//! when it is negative (`smi ($FFFFEE49).w`, `ps4.asm:17502-17511`), and the
//! vehicle's command always carries a target index: `loc_1152`
//! (`ps4.asm:1824-1844`) writes the first present enemy slot's index as the
//! command resolves, and `Battle_PickTargetEnemy` writes the cursor's when the
//! player chose one. So one roll per pass, one target, and `$FFFFEE49` stays
//! zero — a critical stays a critical, unlike a multi-target swing's.

use super::action::{Reach, candidate_targets, critical_bonus, roll_hits};
use super::chances::Verdict;
use super::damage::{calculate_damage, clamp_damage};
use super::event::{BattleEvent, Skipped};
use super::fighters::{Fighter, FighterId, Roster};
use super::rng::Rolls;

#[cfg(test)]
#[path = "vehicle_attack_tests.rs"]
mod tests;

/// The fighter ids `loc_78EE` seats a vehicle under: `Vehicle_Index + $B`
/// (`ps4.asm:11410-11412`), for the Land Rover, the Ice Digger and the
/// Hydrofoil.
pub const VEHICLE_FIGHTER_IDS: [u8; 3] = [0x0C, 0x0D, 0x0E];

/// How many `loc_B6A2` passes one vehicle swing draws.
///
/// Three for every vehicle — state 4, state 5, and state 5 again after the
/// attack object's wind-up — and the third pass's flags are the ones the
/// damage reads.
pub const VEHICLE_HIT_PASSES: usize = 3;

/// The element `loc_280A` reads off the target's `element_props` (`$32(a1)`).
///
/// One-based, as [`Stats::element_factor`](super::stats::Stats::element_factor)
/// takes it: energy.
pub const VEHICLE_ATTACK_ELEMENT: u8 = 2;

/// Whether this fighter is one of `loc_78EE`'s vehicle fighters.
///
/// The port's [`crate::vehicle::battle_member`] carries the cartridge's own
/// identity for it: `0x0B + Vehicle_Index`. It is what makes its Attack command
/// 6 rather than command 1.
#[must_use]
pub fn is_vehicle_fighter(fighter: &Fighter) -> bool {
    fighter
        .character
        .is_some_and(|character| VEHICLE_FIGHTER_IDS.contains(&character))
}

/// Resolves a vehicle's Attack end to end, mutating the roster.
///
/// Appends to `events` and returns the fighters that died, in resolution order,
/// so the engine awards their rewards exactly as it does for a physical attack.
pub fn resolve_vehicle_attack(
    roster: &mut Roster,
    actor: FighterId,
    intended: Option<FighterId>,
    rolls: &mut impl Rolls,
    events: &mut Vec<BattleEvent>,
) -> Vec<FighterId> {
    // `loc_1152` / the target cursor: one slot, never a window.
    let targets = candidate_targets(roster, actor, intended, Reach::Single);
    if targets.is_empty() {
        events.push(BattleEvent::TurnSkipped {
            actor,
            reason: Skipped::NoTarget,
        });
        return Vec::new();
    }
    events.push(BattleEvent::Attacked {
        actor,
        targets: targets.clone(),
    });

    // Three passes, one roll per living target each. The first two are drawn
    // for their rolls alone: `loc_B6A2` blanks the flags before every pass, so
    // only the third pass's verdicts survive to the damage. `multi_target` is
    // false — `$FFFFEE49` is zero for a command with a target index, so a
    // critical is not demoted.
    let mut pass = roll_hits(roster, actor, &targets, false, rolls);
    for _ in 1..VEHICLE_HIT_PASSES {
        pass = roll_hits(roster, actor, &targets, false, rolls);
    }

    let attack = roster
        .get(actor)
        .map_or(0, |fighter| fighter.stats.attack.battle);
    let mut died = Vec::new();
    for (target, verdict) in pass.verdicts {
        if verdict == Verdict::Miss {
            let remaining = roster
                .get(target)
                .map_or(0, |fighter| fighter.stats.curr_hp);
            events.push(BattleEvent::Resolved {
                actor,
                target,
                verdict,
                damage: None,
                remaining_hp: remaining,
            });
            continue;
        }

        // `loc_280A`: the target's own energy property. No weapon element, no
        // shield, no item lookup — the actor's hands are never read.
        let (defence, element) = roster.get(target).map_or((0, 0), |fighter| {
            (
                fighter.stats.defence.battle,
                u16::from(
                    fighter
                        .stats
                        .element_factor(VEHICLE_ATTACK_ELEMENT)
                        .unwrap_or(0),
                ),
            )
        });
        let bonus = if verdict == Verdict::Critical {
            critical_bonus(attack)
        } else {
            0
        };
        let damage = clamp_damage(calculate_damage(attack, defence, element, bonus, rolls));

        let fighter = roster.get_mut(target).expect("vehicle target present");
        let signed = i32::from(fighter.stats.curr_hp) - i32::from(damage);
        fighter.stats.curr_hp = signed.max(0) as u16;
        let remaining = fighter.stats.curr_hp;
        let killed = signed <= 0 && !fighter.stats.is_out();
        if killed {
            fighter.mark_defeated();
        }

        events.push(BattleEvent::Resolved {
            actor,
            target,
            verdict,
            damage: Some(damage),
            remaining_hp: remaining,
        });
        if killed {
            events.push(BattleEvent::Died { fighter: target });
            died.push(target);
        }
    }
    died
}
