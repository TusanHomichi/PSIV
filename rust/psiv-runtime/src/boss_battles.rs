//! Runtime support for scene-owned event battles.
//!
//! Boss formations are a separate pack table, but they use the same engine
//! record conversion and the same presentation timeline as random encounters.
//! This module keeps the scene-specific lifecycle out of the already crowded
//! field/runtime shell.

use std::collections::BTreeMap;

use psiv_core::SceneInput;
use psiv_core::battle::{BattleEvent, FormationRecord, Outcome, Rng2};
use psiv_data::BattleFiles;

use crate::encounters::formation_record;
use crate::{BattleTimeline, BridgeError, Runtime, RuntimeEvent};

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
        let prepared = self.take_battle_load(
            crate::session::BattleStart::EventBattle(index),
            self.battle_party(),
        )?;
        let mut rng2 = Rng2::with_surrogate(&mut self.rng, self.frames);
        let (battle, events) = prepared.roll(&mut rng2);
        // `GameMode_LoadBattle`'s `VDP_EnableDisplay` (`ps4.asm:10062`).
        self.effects.cram.enable_display();
        self.battle = Some(battle);
        self.scene_battle = Some(index);
        Ok(BattleTimeline {
            events,
            sounds: Vec::new(),
            animations: Vec::new(),
        })
    }

    /// Starts the event battle a scene asked for, once its entry has run: the
    /// battle and its timeline, or the failure, after which the scene resumes
    /// as if the party escaped so a malformed pack cannot deadlock it.
    pub(crate) fn start_requested_battle(&mut self, index: u16) -> RuntimeEvent {
        match self.start_boss_battle(index) {
            Ok(initial) => RuntimeEvent::SceneBattleStarted {
                index,
                events: initial.events,
                sounds: initial.sounds,
                animations: initial.animations,
            },
            Err(error) => {
                self.scene_input = SceneInput::BattleFinished {
                    outcome: Outcome::Escaped,
                };
                RuntimeEvent::SceneBattleFailed {
                    index,
                    error: error.to_string(),
                }
            }
        }
    }

    /// Finishes either kind of battle and, for a scene battle, returns the
    /// scene's completion edge to the interpreter.
    pub(crate) fn finish_battle_for_outcome(
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
