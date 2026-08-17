//! The timeline a resolved round emits.
//!
//! A round is resolved **atomically**: the engine runs the whole thing and
//! hands back an ordered list of what happened. Presentation replays that list
//! at whatever pace it likes, and the oracle compares end-of-round state rather
//! than per-frame RAM — which is the only comparison available anyway, because
//! on hardware the HP subtraction happens inside the animation state machine
//! (`FighterShowDamage_DecreaseHP` is reached from a fighter routine, not from
//! the turn engine).
//!
//! Events carry enough for a renderer to stage the round without re-deriving
//! anything: who acted, what each roll decided, how much damage landed, and
//! what the target had left afterwards.

use super::chances::Verdict;
use super::fighters::FighterId;
use super::order::Priority;
use super::vehicle_skill::VehicleSkillEffectKind;

/// How a battle finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Outcome {
    /// Every enemy is out.
    Victory,
    /// Every party member is out.
    Defeat,
    /// The party ran.
    Escaped,
}

/// Why a queued fighter did nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Skipped {
    /// It died earlier in the same round, after the queue was built.
    Dead,
    /// A status in the `$6E` mask landed on it after the queue was built.
    Incapacitated,
    /// It had no living target left.
    NoTarget,
    /// A character with no weapon in either hand has no Attack command
    /// (`Battle_AttackCommand`'s `loc_1682`).
    Unarmed,
}

/// One thing that happened, in resolution order.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum BattleEvent {
    /// The battle opened. Emitted once, before any round.
    Started {
        /// What the opening roll decided.
        priority: Priority,
        /// The enemies, in slot order.
        enemies: Vec<FighterId>,
    },
    /// A round began, with the queue the ordering pass produced.
    RoundBegan {
        /// One-based round number.
        round: u16,
        /// Fighters in the order they will act.
        order: Vec<FighterId>,
    },
    /// The party tried to run and got away.
    Escaped,
    /// The party tried to run and did not. The enemies get the round to
    /// themselves — `Battle_RunFailMsg` sets `Battle_Priority` to `$FF`.
    EscapeFailed,
    /// A queued fighter did nothing.
    TurnSkipped {
        /// Who.
        actor: FighterId,
        /// Why.
        reason: Skipped,
    },
    /// A fighter swung.
    Attacked {
        /// Who.
        actor: FighterId,
        /// Everyone in the swing's path, in id order. More than one means a
        /// type-2 or type-4 weapon.
        targets: Vec<FighterId>,
    },
    /// A fighter took the Defend command: physical resistance until the round
    /// ends.
    Defended {
        /// Who.
        actor: FighterId,
    },
    /// A mounted vehicle consumed one saved use from its selected skill slot.
    VehicleSkillUsed {
        /// Who selected the skill.
        actor: FighterId,
        /// One-based vehicle skill slot.
        skill: u8,
        /// Uses left in the battle copy after the decrement.
        remaining: u8,
    },
    /// A vehicle-skill command named a slot that was not available in the
    /// mounted member's battle copy. The command consumes no use and does not
    /// fall through to a physical attack.
    VehicleSkillRejected {
        /// Who selected the slot.
        actor: FighterId,
        /// One-based vehicle skill slot.
        skill: u8,
    },
    /// A valid vehicle skill named an effect that is outside the implemented
    /// retail table. This is deliberately a no-op marker: a vehicle skill must
    /// never masquerade as physical damage.
    VehicleSkillEffectUnavailable {
        /// Who selected the skill.
        actor: FighterId,
        /// One-based vehicle skill slot.
        skill: u8,
    },
    /// The proven `VehicleSkillData` dispatcher selected an effect and its
    /// affected targets. Damage skills use the ordinary resolved-damage path;
    /// N-Spher uses the separate death-effect chance and follows this event
    /// with `Died` for each target it kills.
    VehicleSkillEffect {
        /// Who selected the skill.
        actor: FighterId,
        /// One-based vehicle skill slot.
        skill: u8,
        /// The retail effect kind.
        effect: VehicleSkillEffectKind,
        /// Targets that accepted the selected effect.
        targets: Vec<FighterId>,
    },
    /// One attacker-target pair resolved.
    Resolved {
        /// Who swung.
        actor: FighterId,
        /// Who was aimed at.
        target: FighterId,
        /// What the hit roll decided. [`Verdict::Miss`] means no damage.
        verdict: Verdict,
        /// Damage dealt after the 1..=999 clamp, `None` on a miss.
        damage: Option<u16>,
        /// The target's HP afterwards, floored at zero. The cartridge lets the
        /// stored value go negative; this reports what a player would see.
        remaining_hp: u16,
    },
    /// An enemy's ability roll landed on something Tier 1 does not implement.
    ///
    /// The enemy falls back to a plain physical attack. Emitted so a wrong
    /// number can never pass silently for a right one; 53 of the 153 enemy
    /// records carry an all-zero ability list and never reach this.
    UnsupportedAbility {
        /// Who rolled it.
        actor: FighterId,
        /// The enemy-skill id.
        ability: u8,
    },
    /// A fighter's HP reached zero or below.
    Died {
        /// Who.
        fighter: FighterId,
    },
    /// The round finished and end-of-round restoration ran.
    RoundEnded {
        /// One-based round number.
        round: u16,
    },
    /// Experience and meseta were handed out.
    Rewarded {
        /// The pool both enemies contributed to, saturating at `$FFFF`.
        experience_total: u16,
        /// The per-member share: the pool divided by the living count.
        experience_each: u16,
        /// The meseta pool, added to the party's purse.
        meseta: u16,
        /// Who was alive to collect, in id order.
        recipients: Vec<FighterId>,
    },
    /// A character gained a level.
    LevelUp {
        /// Index into `Character_Stats`.
        character: u8,
        /// The level reached.
        level: u16,
        /// New maximum HP.
        max_hp: u16,
        /// New maximum TP.
        max_tp: u16,
    },
    /// The battle finished.
    Ended {
        /// How.
        outcome: Outcome,
    },
}
