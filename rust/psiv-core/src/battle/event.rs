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
    /// An enemy object returned directly to the field, without a victory,
    /// defeat or escape epilogue (the first Zio encounter).
    ScriptedExit,
}

/// `EnemyAttack_Zio3`'s five turns, controlled by `$FFFFEE98`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirstZioAction {
    /// Object $908, ability $6B.
    MagicBarrier,
    /// Object $90C, no ability dispatch or damage.
    Invocation,
    /// An empty turn between the two Nightmare objects.
    Pause,
    /// Object $910, ability $53; presentation only in this encounter.
    Nightmare,
    /// Object $914 returns to the field without invoking effect $2C.
    BlackWave,
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
    /// Revived or replenished this round: retail's transient status bit 7.
    JustRevived,
}

/// One thing that happened, in resolution order.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum BattleEvent {
    /// Psycho Wand's object reloads the enemy records after changing the
    /// first formation entry from invulnerable Zio to vulnerable Zio.
    EnemyStatsReloaded {
        /// Party member using the wand.
        actor: FighterId,
        /// Formation slot whose stats were reloaded.
        fighter: FighterId,
        /// Record now occupying that slot.
        enemy_id: u16,
        /// Reloaded display name.
        name: String,
        /// Reloaded HP. Object occupancy is unchanged by this operation.
        hp: u16,
    },
    /// One stage of the first Zio encounter's object-driven sequence.
    FirstZioAction {
        /// Acting enemy.
        actor: FighterId,
        /// Object-side stage.
        action: FirstZioAction,
        /// Black Wave selects Alys by character identity, then falls back
        /// to the first occupied party slot. Other stages clear the target.
        target: Option<FighterId>,
    },
    /// A successful enemy physical-attack effect changed a status bit.
    StatusInflicted {
        /// Attacker.
        actor: FighterId,
        /// Recipient.
        target: FighterId,
        /// Newly applied status bit (poison or paralysis).
        status: u8,
    },
    /// An enemy executes an implemented object-side ability.
    EnemySkillUsed {
        /// Acting enemy.
        actor: FighterId,
        /// One-based enemy skill id.
        skill: u8,
        /// Cartridge display name.
        name: String,
    },
    /// Fission replaces a defeated neighbor from its original formation data.
    EnemyReplenished {
        /// Enemy using Fission.
        actor: FighterId,
        /// Restored formation slot.
        fighter: FighterId,
        /// The restored enemy's record id.
        enemy_id: u16,
        /// Display name.
        name: String,
        /// Full restored HP.
        hp: u16,
    },
    /// An actor activated an item from equipment or shared inventory.
    ItemUsed {
        /// Acting fighter.
        actor: FighterId,
        /// Cartridge item id.
        item: u8,
        /// Display name.
        name: String,
        /// Whether a disposable inventory item was removed.
        consumed: bool,
    },
    /// An invalid item command spent nothing and produced no effect.
    ItemRejected {
        /// Acting fighter.
        actor: FighterId,
        /// Requested item id.
        item: u8,
        /// Rejection reason.
        reason: super::item::ItemRejection,
    },
    /// A valid item use produced no change for this recipient.
    ItemIneffective {
        /// Acting fighter.
        actor: FighterId,
        /// Selected recipient.
        target: FighterId,
    },
    /// A paid technique produced no change for this recipient.
    TechniqueIneffective {
        /// Acting fighter.
        actor: FighterId,
        /// Selected recipient.
        target: FighterId,
    },
    /// An effect removed one or more status flags.
    StatusRestored {
        /// Acting fighter.
        actor: FighterId,
        /// Recipient.
        target: FighterId,
        /// Flags removed by this effect.
        removed: u8,
    },
    /// A downed fighter returned to combat.
    Revived {
        /// Acting fighter.
        actor: FighterId,
        /// Recipient.
        target: FighterId,
        /// HP after revival.
        remaining_hp: u16,
    },
    /// An effect restored a recipient's unbuffed combat stats/resistances.
    StatsRestored {
        /// Acting fighter.
        actor: FighterId,
        /// Recipient.
        target: FighterId,
    },
    /// A character spent one use of a learned skill.
    SkillUsed {
        /// Actor.
        actor: FighterId,
        /// Cartridge skill id.
        skill: u8,
        /// Pack display name.
        name: String,
        /// Uses remaining in the matching learned slot.
        remaining: u8,
    },
    /// A skill could not execute; it never becomes a physical attack.
    SkillRejected {
        /// Actor.
        actor: FighterId,
        /// Requested skill id.
        skill: u8,
        /// Failure reason. A weapon missing at execution follows payment,
        /// just as the retail Character_DoSkill checks it after loc_9C2C.
        reason: super::skill::SkillRejection,
    },
    /// The target already has a status that excludes this skill's effect.
    SkillIneffective {
        /// Actor.
        actor: FighterId,
        /// Recipient.
        target: FighterId,
    },
    /// An effect put a target to sleep.
    FellAsleep {
        /// Actor.
        actor: FighterId,
        /// Recipient.
        target: FighterId,
    },
    /// End-of-round recovery cleared both sleep flags and restored agility.
    WokeUp {
        /// Fighter that woke.
        fighter: FighterId,
    },
    /// Enemy paralysis expires at round end, without a random draw.
    ParalysisCleared {
        /// Enemy that recovered.
        fighter: FighterId,
    },
    /// A validated technique consumed TP when its actor's turn arrived.
    TechniqueUsed {
        /// Caster.
        actor: FighterId,
        /// Cartridge technique id.
        technique: u8,
        /// Pack display name.
        name: String,
        /// TP after paying the cost.
        remaining_tp: u16,
    },
    /// A technique could not execute. Invalid orders spend nothing; sealing
    /// after selection is reported after `TechniqueUsed` has charged TP.
    TechniqueRejected {
        /// Caster.
        actor: FighterId,
        /// Requested id.
        technique: u8,
        /// Reason the command could not execute.
        reason: super::technique::TechniqueRejection,
    },
    /// A successful healing effect, capped by the target's maximum HP.
    Healed {
        /// Caster.
        actor: FighterId,
        /// Recipient.
        target: FighterId,
        /// HP actually restored.
        amount: u16,
        /// HP after healing.
        remaining_hp: u16,
    },
    /// A support technique changed a battle-only stat.
    StatChanged {
        /// Caster.
        actor: FighterId,
        /// Recipient.
        target: FighterId,
        /// Stat changed.
        stat: super::technique::TechniqueStat,
        /// New value.
        value: u16,
    },
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
    /// The rolled ability left the actor's attack routine with nothing to
    /// load, so the actor's turn is spent and nothing happens.
    ///
    /// `EnemyAttack_FloatMine` (`ps4.asm:22675`) has an arm for `$14`
    /// Warning, `$18` Explosion, `$19` Detonation and `$1A` CyanicBomb only.
    /// Every other id reaches the fall-through at `loc_10406`
    /// (`ps4.asm:22781`), which loads no battle object, clears
    /// `Current_Target_Index` and `$24(a4)` — the ability the effect dispatcher
    /// reads — sets `Battle_Routine` to `$16` and drops the actor's
    /// `fighter_routine` from `Enemy_Attack` to `Fighter_DoNothing`
    /// (`subq.w #2, $2(a4)`; the shared `loc_D200` tail does the same for a
    /// real attack).
    ///
    /// `loc_B6A2` then leaves every `Fighters_Hit_Flags` entry at `$FF` ("not
    /// being targeted or miss"), and `Battle_DoAttackEffect` (`ps4.asm:8553`)
    /// finds ability 0 with no hit flag set, so it never reaches
    /// `Ability_GetEffectAndRange` and hands over to `Battle_Routine` `$12`:
    /// no object, no palette or PLC upload, no sound, no message window, no
    /// damage and no status. The `$12`/`$1E` pair spends the usual wait and
    /// advances the turn order, so the actor has acted. Only the two ids the
    /// carriers of that routine can roll without an arm are reachable here:
    /// `$07` Fission2 (50 FloatMine2) and `$17` Waiting (44 FloatMine, 46
    /// VopalSphre, 50 FloatMine2).
    EnemyAbilityWasted {
        /// Who rolled it.
        actor: FighterId,
        /// The enemy-skill id, as rolled.
        ability: u8,
        /// Cartridge display name of the record the roll landed on.
        name: String,
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
    /// A newly learned technique or skill from the visible level-up sequence.
    LearnedAbility {
        /// Index into `Character_Stats`.
        character: u8,
        /// Display name from the runtime ability catalog.
        name: String,
    },
    /// The battle finished.
    Ended {
        /// How.
        outcome: Outcome,
    },
}
