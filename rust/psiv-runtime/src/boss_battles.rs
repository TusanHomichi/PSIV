//! Runtime support for scene-owned event battles.
//!
//! Boss formations are a separate pack table, but they use the same engine
//! record conversion and the same presentation timeline as random encounters.
//! This module keeps the scene-specific lifecycle out of the already crowded
//! field/runtime shell.

use std::collections::BTreeMap;

use psiv_core::battle::{Battle, BattleEvent, FormationRecord, Outcome, Rng2};
use psiv_core::{EventIndex, Flag, SceneInput, StepFrames, runner_for, scene_for};
use psiv_data::BattleFiles;

use crate::encounters::formation_record;
use crate::{BattleTimeline, BridgeError, Runtime};

/// The opening-act scene that owns event battle index zero.
const IGGLANOVA_EVENT: u16 = 0x6B;
const IGGLANOVA_BATTLE_INDEX: u16 = 0;

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
        let (battle, events) = Battle::start(record, party, &set.data, true, &mut rng2)
            .map_err(|e| BridgeError::Rejected(e.to_string()))?;
        self.battle = Some(battle);
        self.scene_battle = Some(index);
        Ok(BattleTimeline {
            events,
            sounds: Vec::new(),
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
        if let Some(index) = self.scene_battle.take() {
            if outcome == Outcome::Defeat {
                self.revive_interim();
                if index == IGGLANOVA_BATTLE_INDEX {
                    // Retail sets EventFlag_Igglanova before entering the
                    // battle and then goes through ordinary defeat/Game Over.
                    // The opening-act interim policy is retryable instead:
                    // clear that guard and reinstall Event_IgglanovaBattle
                    // after the old runner consumes BattleFinished.
                    let _ = self.game.clear(Flag::event(0x0B));
                    self.scene_retry = Some(IGGLANOVA_EVENT);
                }
            }
            self.scene_input = SceneInput::BattleFinished { outcome };
        } else if outcome == Outcome::Defeat {
            // Preserve the existing interim policy for random encounters.
            self.revive_interim();
        }
        levels
    }

    /// Replaces a completed Igglanova runner with its retry scene, if a boss
    /// defeat requested one. Called only after `scene_tick` has observed that
    /// the old runner consumed the BattleFinished input.
    pub(crate) fn retry_scene(&mut self) -> bool {
        let Some(event) = self.scene_retry.take() else {
            return false;
        };
        let Some(scene) = scene_for(EventIndex(event)) else {
            return false;
        };
        let cast = self.build_cast();
        let Ok(runner) = runner_for(scene, cast, StepFrames::default()) else {
            return false;
        };
        self.scene = Some(runner);
        self.scene_input = SceneInput::None;
        self.scene_warmup = true;
        true
    }
}
