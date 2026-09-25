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
//! # The swing: two hit passes, or three, and the last one deciding
//!
//! `loc_AF9C` (`ps4.asm:16810-16815`) stores the command data's **low byte** —
//! `loc_684A` (`ps4.asm:9873-9874`) writes `#6` and then `($FFFFF43D).w`, the
//! vehicle index, into `Battle_Command_Data`, and `andi.w #$FF, d0` reads that
//! low byte back out — in the actor's `ability` (`$24`), and drives the
//! ten-state machine `loc_AFE4` (`ps4.asm:16831-16841`). Two of its frames
//! call `loc_B6A2`, and a third only when the attack object hands state 5 back:
//!
//! 1. state 4, `loc_B14C` → `loc_B166` (`ps4.asm:16957-16988`): the vehicle
//!    attack object `$644`/`$648`/`$64C` is created with the vehicle as its
//!    `parent` (`move.l a4, $3C(a1)`, `ps4.asm:16979`) and the routine
//!    **jumps** straight into `loc_B6A2`. The jump is why state 4's own
//!    `addq.w #1, $32(a4)` (`ps4.asm:16961`) is the *only* thing that moves
//!    the vehicle to 5: `trap #2` is a `jsr` through the table
//!    (`Trap02Exception`, `ps4.asm:186-190`), so a branch that returns reaches
//!    that `addq`, and `loc_B6A2`'s `rts` returns through the `rte`.
//! 2. state 5, `loc_9848` (`ps4.asm:14964-14978`): `jsr loc_B6A2`, then the
//!    targets are put into their damage animation and `action_routine` is
//!    advanced to 6 — `addq.w #1, action_routine(a4)`, `ps4.asm:14978`. State
//!    6 is `loc_987A`'s bare `rts` (`ps4.asm:14984-14985`): a wait.
//! 3. state 5 **again**, only when the attack object writes `move.w #5,
//!    $32(a0)` back onto the vehicle after it has left 5.
//!
//! That hand-back is the object's own first state, on a timer of its own, and
//! the timer is what splits the vehicles ([`hit_passes`] and
//! [`VEHICLE_ATTACK_OBJECTS`] carry each object's citations):
//!
//! * `BattleObj_LandRoverAtk` and `BattleObj_HydrofoilAtk` are created with
//!   `move.w #$C, $1C(a4)` (`ps4.asm:82945`, `83071`), so their countdown
//!   expires thirteen frames later (`subq.w #1` then `bge`, `ps4.asm:82970-82978`,
//!   `83104-83112`) and the hand-back lands while the vehicle sits in 6 — the
//!   write re-enters 5, and the swing draws **three** passes. The Land Rover's
//!   capture has them at f25059 (state 4), f25060 (state 5) and f25072.
//! * `BattleObj_IceDiggerAtk` is created with `clr.w $1C(a4)`
//!   (`ps4.asm:82996`), so its countdown is already expired in the frame it is
//!   created (`ps4.asm:83043-83050`) and it hands back **while state 4 is still
//!   running** — `GameMode_Battle` runs every fighter before any battle object
//!   (`Battle_UpdateFighters`, `ps4.asm:953`, then `Battle_RunObjects`,
//!   `ps4.asm:956`/`981`), so the object's write lands after `loc_B14C`'s own
//!   `addq`, on the value `$32` is about to hold anyway: state 5 runs once, and
//!   the swing draws **two** passes. The Ice Digger's capture has them at
//!   f25058 and f25059, and the third pass the port used to draw is what
//!   `docs/oracle/BATTLE_ORACLE_FORCED.md` §5.1 recorded as a divergence.
//!
//! `loc_B6A2` blanks all nine `Fighters_Hit_Flags` before every pass
//! (`ps4.asm:17493-17496`), so each pass overwrites the last one's verdicts;
//! the **last** pass is what `Fighter_TakeDamage` reads, whichever count the
//! vehicle draws. The object's second state then sets the vehicle's
//! `action_routine` to 7 (`move.w #7, $32(a0)`, `ps4.asm:82986`, `83059` — the
//! Hydrofoil's state table points at the Land Rover's `loc_3FFC8`,
//! `ps4.asm:83102`), and state 7 `loc_B1A6` (`ps4.asm:16999-17008`) puts the
//! target into routine `$C` — the frame whose `loc_266C` runs
//! `Battle_CalculateDamage`. The attack command's own check is what keeps the
//! rolls: `loc_B6D4` routes command 6 to the rolling path before the
//! `tst.w $24(a4)` that would have turned the nonzero ability into a
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
//! critical bonus `(atk & $FF) >> 2` (`critical_bonus`) when the hit flag is `$01`, and jumps to `loc_266C`
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

#[cfg(test)]
#[path = "vehicle_attack_passes_tests.rs"]
mod passes_tests;

/// The fighter ids `loc_78EE` seats a vehicle under: `Vehicle_Index + $B`
/// (`ps4.asm:11409-11412`), for the Land Rover, the Ice Digger and the
/// Hydrofoil.
pub const VEHICLE_FIGHTER_IDS: [u8; 3] = [0x0C, 0x0D, 0x0E];

/// The `Vehicle_Index` that id base adds to, so `0x0B + index` is the fighter.
const VEHICLE_FIGHTER_ID_BASE: u8 = 0x0B;

/// The `loc_B6A2` passes a vehicle swing draws before any hand-back: state 4's
/// own pass and state 5's (`loc_9848`).
const HIT_PASSES_BASE: usize = 2;

/// The pass a hand-back adds when it lands after the vehicle has left state 5
/// — state 5 again.
const HIT_PASSES_HAND_BACK: usize = 1;

/// The longest swing any vehicle draws, and what [`hit_passes`] answers for a
/// selector `VehicleData` does not cover.
const HIT_PASSES_MAX: usize = HIT_PASSES_BASE + HIT_PASSES_HAND_BACK;

/// One vehicle's attack object: what `loc_AF9C`'s state 4 creates, and the
/// `$1C` wind-up it is created with — the pair that decides the pass count.
///
/// `loc_B14C` (`ps4.asm:16957-16960`) dispatches on the actor's `ability`,
/// which for a vehicle is the vehicle index (`loc_684A` writes `#6` and then
/// `($FFFFF43D).w` into the command data, `ps4.asm:9873-9874`), through
/// `loc_B15C` (`ps4.asm:16964-16968`): entry 1 → `loc_B166` (`$644`), 2 →
/// `loc_B17E` (`$648`), 3 → `loc_B184` (`$64C`). The three ids name the
/// routines in the battle object table (`ps4.asm:74660-74662`), and each
/// entry's own creator writes the timer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VehicleAttackObject {
    /// The `Vehicle_Index` whose state 4 creates it, and the table entry.
    pub vehicle: u16,
    /// The object id the loader is handed (`move.w #$644, d0`, …).
    pub object_id: u16,
    /// The routine that id points at in `ps4.asm:74660-74662`.
    pub routine: &'static str,
    /// The `$1C` frame timer the object is created with.
    pub wind_up: u16,
    /// Where the creator writes that timer.
    pub wind_up_line: &'static str,
    /// Where the object's first state hands `move.w #5, $32(a0)` back.
    pub hand_back_line: &'static str,
}

/// Each `VehicleData` vehicle's attack object, in `Vehicle_Index` order.
///
/// `VehicleData` (`ps4.asm:321152-321179`) holds the records in this order
/// (Land Rover, Ice Digger, Hydrofoil), and `loc_77AE` (`ps4.asm:11295-11311`)
/// indexes them by `Vehicle_Index - 1`; the ability table above is indexed by
/// the same value, which is what pairs each record with its object.
pub const VEHICLE_ATTACK_OBJECTS: [VehicleAttackObject; 3] = [
    VehicleAttackObject {
        vehicle: 1,
        object_id: 0x644,
        routine: "BattleObj_LandRoverAtk",
        wind_up: 12,
        wind_up_line: "ps4.asm:82945",
        hand_back_line: "ps4.asm:82975",
    },
    VehicleAttackObject {
        vehicle: 2,
        object_id: 0x648,
        routine: "BattleObj_IceDiggerAtk",
        wind_up: 0,
        wind_up_line: "ps4.asm:82996",
        hand_back_line: "ps4.asm:83048",
    },
    VehicleAttackObject {
        vehicle: 3,
        object_id: 0x64C,
        routine: "BattleObj_HydrofoilAtk",
        wind_up: 12,
        wind_up_line: "ps4.asm:83071",
        hand_back_line: "ps4.asm:83109",
    },
];

/// The attack object `vehicle`'s swing creates, or `None` for a selector
/// `VehicleData` does not cover.
#[must_use]
pub fn attack_object(vehicle: u16) -> Option<&'static VehicleAttackObject> {
    VEHICLE_ATTACK_OBJECTS
        .iter()
        .find(|object| object.vehicle == vehicle)
}

/// How many `loc_B6A2` passes `vehicle`'s Attack draws — two, or three when the
/// attack object's wind-up hands state 5 back.
///
/// The hand-back is one write of `move.w #5, $32(a0)` on the vehicle, and
/// whether it re-enters state 5 depends on **when** it lands against state 4's
/// own `addq.w #1, $32(a4)` (`ps4.asm:16961`) and state 5's
/// `addq.w #1, action_routine(a4)` (`ps4.asm:14978`):
///
/// * a **nonzero** wind-up expires on creation frame + `wind_up`, thirteen
///   frames for the `#$C` the Land Rover and the Hydrofoil are created with
///   (`subq.w #1` with `bge`, so the countdown is `wind_up + 1` calls). The
///   vehicle left state 5 on creation frame + 2, so the write re-enters it and
///   the swing draws three passes.
/// * a **zero** wind-up expires in the creation frame itself, and the fighters
///   run before the battle objects within a frame (`Battle_UpdateFighters`,
///   `ps4.asm:953`, then `Battle_RunObjects`, `ps4.asm:956`), so the write lands
///   after state 4's own advance and on the value `$32` is about to hold
///   anyway. State 5 runs once and the swing draws two passes.
///
/// A `vehicle` outside `VehicleData` — a caller reaching
/// [`resolve_vehicle_attack`] without `resolve_attack`'s dispatch on
/// [`is_vehicle_fighter`] — is answered with the longest swing any vehicle
/// draws, three.
#[must_use]
pub fn hit_passes(vehicle: u16) -> usize {
    match attack_object(vehicle) {
        Some(object) if object.wind_up == 0 => HIT_PASSES_BASE,
        _ => HIT_PASSES_MAX,
    }
}

/// The `Vehicle_Index` a vehicle fighter's `character` id carries, or `None`
/// for anything else.
///
/// `loc_78EE` (`ps4.asm:11409-11412`) seats the vehicle as fighter id
/// `Vehicle_Index + $B`; it is what makes its Attack command 6 rather than
/// command 1, and what selects its attack object and pass count.
#[must_use]
pub fn vehicle_index(fighter: &Fighter) -> Option<u16> {
    fighter.character.and_then(|character| {
        VEHICLE_FIGHTER_IDS
            .contains(&character)
            .then(|| u16::from(character) - u16::from(VEHICLE_FIGHTER_ID_BASE))
    })
}

/// The element `loc_280A` reads off the target's `element_props` (`$32(a1)`).
///
/// One-based, as [`Stats::element_factor`](super::stats::Stats::element_factor)
/// takes it: energy.
pub const VEHICLE_ATTACK_ELEMENT: u8 = 2;

/// Whether this fighter is one of `loc_78EE`'s vehicle fighters.
///
/// The port's [`crate::vehicle::battle_member`] carries the cartridge's own
/// identity for it: `0x0B + Vehicle_Index` (see [`vehicle_index`]). It is what
/// makes its Attack command 6 rather than command 1.
#[must_use]
pub fn is_vehicle_fighter(fighter: &Fighter) -> bool {
    vehicle_index(fighter).is_some()
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
    // `loc_1152` / the target cursor: one slot, never a window. The aim comes
    // from the same command window a party swing's does, so the retarget scan
    // applies here too (`action::candidate_targets`).
    let targets = candidate_targets(roster, actor, intended, Reach::Single, rolls);
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

    // The pass count is the vehicle's own: its attack object's wind-up decides
    // whether the swing re-enters state 5 (`hit_passes`). Every pass is drawn
    // for its rolls alone — `loc_B6A2` blanks the flags before each, so only
    // the last pass's verdicts survive to the damage. `multi_target` is false:
    // `$FFFFEE49` is zero for a command with a target index, so a critical is
    // not demoted.
    let passes = roster
        .get(actor)
        .and_then(vehicle_index)
        .map_or(HIT_PASSES_MAX, hit_passes);
    let mut pass = roll_hits(roster, actor, &targets, false, rolls);
    for _ in 1..passes {
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
