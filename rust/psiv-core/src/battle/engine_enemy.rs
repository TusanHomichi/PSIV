//! `Enemy_Attack`'s ability roll and the dispatch behind it.
//!
//! Split from `engine.rs` (the 1,000-line rule): the engine keeps the round's
//! shape and the turn loop, this file owns what an enemy's turn does once the
//! roll has picked an ability.

use super::super::ai::choose_ability;
use super::super::event::{BattleEvent, Outcome};
use super::super::fighters::{FighterId, Side};
use super::super::records::{BattleData, BattleDataError};
use super::super::rng::Rolls;
use super::super::zio::{self, Step, ZioTurn};
use super::Battle;

impl Battle {
    /// `Enemy_Attack`'s opening — the ability roll, always taken.
    ///
    /// Fission and the traced damage-skill routes
    /// ([`super::super::enemy_damage::resolve_damage_skill`]) dispatch after the
    /// ordinary ability roll and empty-space condition, and so do FloatMine-carrier
    /// rolls of `$07` and `$17`, which spend the turn without an effect. Unsupported abilities
    /// retain the fallback. When the roll
    /// lands on a real ability the enemy still swings physically, and an
    /// [`BattleEvent::UnsupportedAbility`] says so rather than letting a wrong
    /// number pass for a right one. 53 of the cartridge's 153 enemies —
    /// including everything in both oracle tapes — carry an all-zero list and
    /// never reach that branch.
    pub(super) fn roll_enemy_ability(
        &mut self,
        actor: FighterId,
        intended: Option<FighterId>,
        data: &BattleData,
        rolls: &mut impl Rolls,
        events: &mut Vec<BattleEvent>,
    ) -> Result<bool, BattleDataError> {
        let enemy_id = self.roster.get(actor).map_or(0, |f| f.stats.enemy_id);
        let record = data.enemy(enemy_id)?;
        let (_, mut ability) = choose_ability(record, &mut self.last_ability_index, rolls);
        // The instruction block runs *after* the roll and before the routine
        // dispatch, so it can replace the ability the roll picked
        // (`ps4.asm:19157-19168`).
        let outcome =
            super::super::enemy_ai::instruction_block(&mut self.roster, actor, record, rolls)?;
        let mut replacement = None;
        match outcome {
            super::super::enemy_ai::AiOutcome::Rolled => {}
            super::super::enemy_ai::AiOutcome::Replaced { slot, neighbour } => {
                ability = record.conditional_abilities[slot];
                replacement = neighbour;
            }
            super::super::enemy_ai::AiOutcome::Unsupported { .. } => {
                // The port cannot tell whether the arm held, so it must not run
                // the roll: reporting the ability that arm would have written
                // and falling back to the swing is the only honest answer
                // (`docs/battle/ENEMY_ABILITIES.md`, Port gaps).
                let ability = outcome.unreported_ability(record).expect("unsupported arm");
                if let Some(fighter) = self.roster.get_mut(actor) {
                    fighter.ability = ability;
                }
                events.push(BattleEvent::UnsupportedAbility { actor, ability });
                return Ok(false);
            }
        }
        if let Some(fighter) = self.roster.get_mut(actor) {
            fighter.ability = ability;
        }
        if self.scripted_latch && super::super::scripted_flag::reads_first_action(enemy_id) {
            // `EnemyAttack_DarkForce1`'s opening test (`ps4.asm:20031-20035`):
            // the latch is up, so the rolled ability is dropped, the latch
            // lowered and the fixed object `$818` loaded (the roll above was
            // drawn either way). `loc_32344` (`ps4.asm:64872-65000`) and its
            // children animate only: no damage request, no `UpdateRNGSeed2`.
            self.scripted_latch = false;
            if let Some(fighter) = self.roster.get_mut(actor) {
                fighter.ability = 0;
            }
            events.push(BattleEvent::FirstZioAction {
                actor,
                action: super::super::FirstZioAction::DarkForceCharge,
                target: None,
            });
            return Ok(true);
        }
        if let Some(arm) = zio::step(enemy_id, self.enemy_phase) {
            self.run_zio_arm(actor, intended, arm, events);
            return Ok(true);
        }
        if let Some(target) = replacement
            && super::super::enemy_skill::resolve_fission(
                &mut self.roster,
                actor,
                ability,
                target,
                data,
                events,
            )?
        {
            return Ok(true);
        }
        if super::super::enemy_fusion::resolve_fusion(
            &mut self.roster,
            actor,
            ability,
            data,
            events,
        )? {
            return Ok(true);
        }
        if super::super::enemy_damage::resolve_damage_skill(
            &mut self.roster,
            actor,
            ability,
            intended,
            data,
            rolls,
            events,
        ) {
            return Ok(true);
        }
        match super::super::enemy_effect::resolve_effect_skill(
            &mut self.roster,
            actor,
            ability,
            intended,
            data,
            rolls,
            events,
        ) {
            super::super::enemy_effect::EffectTurn::Resolved => return Ok(true),
            // The arm's guard sent the turn to the ordinary attack objects.
            super::super::enemy_effect::EffectTurn::Swing => return Ok(false),
            super::super::enemy_effect::EffectTurn::NotMine => {}
        }
        if super::super::enemy_skill::resolve_res(
            &mut self.roster,
            actor,
            ability,
            data,
            rolls,
            events,
        ) {
            return Ok(true);
        }
        // `EnemyAttack_FloatMine`'s fall-through (`loc_10406`): the roll has
        // nothing to load, so the turn is spent rather than turned into a
        // physical attack. Last, because it is the absence of an arm.
        if super::super::enemy_skill::resolve_no_effect_turn(
            &mut self.roster,
            actor,
            ability,
            data,
            events,
        ) {
            return Ok(true);
        }
        if ability != 0 {
            events.push(BattleEvent::UnsupportedAbility { actor, ability });
        }
        Ok(false)
    }

    /// One scripted arm of a Zio routine ([`zio::step`]): the routine has
    /// overwritten `$24(a4)` with the arm's ability and loaded its object.
    ///
    /// None of the objects draws from the shared stream (no `UpdateRNGSeed2`
    /// call sits in `ps4.asm:66438-67560`, and the helpers they call are the
    /// animation and request tails the damage routes already read), so the
    /// arms cost nothing beyond the roll the turn already took.
    fn run_zio_arm(
        &mut self,
        actor: FighterId,
        intended: Option<FighterId>,
        arm: Step,
        events: &mut Vec<BattleEvent>,
    ) {
        use super::super::FirstZioAction;
        self.roster.get_mut(actor).expect("acting enemy").ability = arm.ability;
        if arm.advances {
            self.enemy_phase = self.enemy_phase.saturating_add(1);
        }
        let action = match arm.turn {
            ZioTurn::Barrier => FirstZioAction::MagicBarrier,
            ZioTurn::Invocation => FirstZioAction::Invocation,
            ZioTurn::Pause => FirstZioAction::Pause,
            ZioTurn::Nightmare => FirstZioAction::Nightmare,
            ZioTurn::BlackWaveExit | ZioTurn::BlackWaveKill => FirstZioAction::BlackWave,
        };
        // Black Wave1 (`loc_D2F4`, `ps4.asm:19445-19460`) picks Alys by
        // character identity (`cmpi.w #1, $12(a0)`) and falls back to the
        // first fighter; Black Wave3 keeps the target `Enemy_Attack` drew.
        let target = match arm.turn {
            ZioTurn::BlackWaveExit => self
                .roster
                .side(Side::Party)
                .find(|f| f.character == Some(1))
                .or_else(|| self.roster.side(Side::Party).next())
                .map(|f| f.id),
            ZioTurn::BlackWaveKill => intended,
            _ => None,
        };
        events.push(BattleEvent::FirstZioAction {
            actor,
            action,
            target,
        });
        match arm.turn {
            ZioTurn::BlackWaveExit => self.outcome = Some(Outcome::ScriptedExit),
            ZioTurn::BlackWaveKill => {
                // `loc_25048` (`ps4.asm:48916`): fighter routine 7
                // (`Character_Dead`), HP cleared, the death bit set (the
                // android bit for a profession-5 member). No chance roll, no
                // damage request, and no check that the target still stands:
                // `Enemy_Attack` only draws a living one.
                if let Some(target) = target
                    && let Some(fighter) = self.roster.get_mut(target)
                    && fighter.is_alive()
                {
                    fighter.mark_defeated();
                    events.push(BattleEvent::Died { fighter: target });
                }
            }
            _ => {}
        }
    }
}
