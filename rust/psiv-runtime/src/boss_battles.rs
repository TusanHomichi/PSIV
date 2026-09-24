//! Runtime support for scene-owned event battles.
//!
//! Boss formations are a separate pack table, but they use the same engine
//! record conversion and the same presentation timeline as random encounters.
//! This module keeps the scene-specific lifecycle out of the already crowded
//! field/runtime shell.

use std::collections::BTreeMap;

use psiv_core::SceneInput;
use psiv_core::battle::{Battle, BattleEvent, FormationRecord, Outcome, Rng2};
use psiv_data::BattleFiles;

use crate::encounters::formation_record;
use crate::{BattleTimeline, BridgeError, Runtime};

/// Converts the pack's boss table using the normal formation-record path.
pub(crate) fn boss_formation_records(
    files: &BattleFiles,
) -> Result<BTreeMap<u16, FormationRecord>, BridgeError> {
    let mut records = BTreeMap::new();
    for formation in &files.formations.boss_formations {
        let index = formation.event_battle_index.ok_or_else(|| {
            BridgeError::Rejected("boss formation without event battle index".into())
        })?;
        let record = formation_record(formation)?;
        if records.insert(index, record).is_some() {
            return Err(BridgeError::Rejected(format!(
                "duplicate boss event battle index {index}"
            )));
        }
    }
    Ok(records)
}

impl Runtime {
    /// Starts a boss formation requested by a running scene.
    pub(crate) fn start_boss_battle(&mut self, index: u16) -> Result<BattleTimeline, BridgeError> {
        let set = self
            .battles
            .as_ref()
            .ok_or_else(|| BridgeError::Rejected("battles not enabled".into()))?;
        let record = set.boss_formations.get(&index).ok_or_else(|| {
            BridgeError::Rejected(format!("unknown boss event battle index {index}"))
        })?;
        let party = self.battle_party();
        if party.is_empty() {
            return Err(BridgeError::Rejected(
                "boss battle requested with an empty party".into(),
            ));
        }
        let mut rng2 = Rng2::with_surrogate(&mut self.rng, self.frames);
        // The same `GameMode_LoadBattle` page wipe a random encounter goes
        // through (`ps4.asm:9992-9994`) - `RunEventBattle` sets mode $10 as
        // well - so the ability-index word starts at zero here too.
        self.last_ability_index = 0;
        let (battle, events) = if self.vehicle.is_some() {
            // `RunEventBattle` falls into the same battle setup as a random
            // encounter. `Vehicle_Index` is the discriminator: it replaces
            // the party with the saved vehicle fighter even when the scene
            // supplied Event_Battle_Index for its formation/background.
            Battle::start_vehicle(record, party, &set.data, self.last_ability_index, &mut rng2)
        } else {
            Battle::start(
                record,
                party,
                &set.data,
                true,
                self.last_ability_index,
                &mut rng2,
            )
        }
        .map_err(|e| BridgeError::Rejected(e.to_string()))?;
        self.battle = Some(battle);
        self.scene_battle = Some(index);
        Ok(BattleTimeline {
            events,
            sounds: Vec::new(),
            animations: Vec::new(),
        })
    }

    /// Finishes either kind of battle and, for a scene battle, returns the
    /// scene's completion edge to the interpreter.
    pub fn finish_battle_for_outcome(
        &mut self,
        outcome: Outcome,
        reward_each: u16,
    ) -> Vec<BattleEvent> {
        let levels = self.finish_battle_absorbing(reward_each);
        if outcome == Outcome::Defeat {
            // Battle_DefeatedMsg returns to Game_Mode_Index zero. Keep the
            // defeated roster until the title creates/loads a fresh runtime;
            // no revive, reward, map reload or post-boss scene can run here.
            self.end_game();
        } else if self.scene_battle.take().is_some() {
            self.scene_input = SceneInput::BattleFinished { outcome };
        }
        levels
    }
}
