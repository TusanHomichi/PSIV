//! Explicit repair for saves made before level-up learning was implemented.
use crate::{BridgeError, Runtime};
use psiv_core::{CHARACTER_COUNT, CharId, GameState};

/// Changes to one character's earned abilities and skill capacity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressionRepair {
    /// Persistent character ID.
    pub character: CharId,
    /// Missing techniques added in level-table order.
    pub techniques: Vec<u8>,
    /// Missing skills added in level-table order.
    pub skills: Vec<u8>,
    /// Previously saved skill maxima.
    pub previous_max_uses: [u8; 8],
    /// Maxima from the character's attained level.
    pub max_uses: [u8; 8],
}

impl Runtime {
    /// Explicitly repair an early native save whose level-ups dropped learned
    /// abilities and skill maxima. Ordinary load never rewrites a modded roster.
    /// HP, TP, EXP, level, existing spent uses and all world state are preserved.
    /// New skill slots receive their original learning-level uses. The operation
    /// is idempotent and atomic; it does not write any file.
    pub fn repair_legacy_progression(&mut self) -> Result<Vec<ProgressionRepair>, BridgeError> {
        if self.battle.is_some() || self.scene_active() {
            return Err(BridgeError::Rejected(
                "progression repair requires idle field".into(),
            ));
        }
        let set = self
            .battles
            .as_ref()
            .ok_or_else(|| BridgeError::Rejected("progression repair needs battle data".into()))?;
        let mut game = GameState::from_snapshot(&self.game.snapshot());
        let mut repairs = Vec::new();
        for character in 0..CHARACTER_COUNT {
            let id = CharId(character as u8);
            let Some(stats) = game.roster_mut().get_mut(id) else {
                continue;
            };
            let table = set
                .data
                .level_table(id.0)
                .map_err(|e| BridgeError::Rejected(e.to_string()))?;
            let mut repair = ProgressionRepair {
                character: id,
                techniques: Vec::new(),
                skills: Vec::new(),
                previous_max_uses: stats.max_skill_uses,
                max_uses: stats.max_skill_uses,
            };
            for record in table
                .levels
                .iter()
                .filter(|record| record.level <= stats.level)
            {
                if record.new_technique != 0
                    && set.data.technique(record.new_technique).is_some()
                    && !stats.techniques.contains(&record.new_technique)
                {
                    let slot =
                        stats
                            .techniques
                            .iter()
                            .position(|id| *id == 0)
                            .ok_or_else(|| {
                                BridgeError::Rejected(format!(
                                    "character {character} technique slots full"
                                ))
                            })?;
                    stats.techniques[slot] = record.new_technique;
                    repair.techniques.push(record.new_technique);
                }
                if record.new_skill != 0
                    && set.data.skill(record.new_skill).is_some()
                    && !stats.skills.contains(&record.new_skill)
                {
                    let slot = stats.skills.iter().position(|id| *id == 0).ok_or_else(|| {
                        BridgeError::Rejected(format!("character {character} skill slots full"))
                    })?;
                    stats.skills[slot] = record.new_skill;
                    stats.curr_skill_uses[slot] = record.skill_uses[slot];
                    repair.skills.push(record.new_skill);
                }
                stats.max_skill_uses = record.skill_uses;
            }
            repair.max_uses = stats.max_skill_uses;
            if !repair.techniques.is_empty()
                || !repair.skills.is_empty()
                || repair.previous_max_uses != repair.max_uses
            {
                repairs.push(repair);
            }
        }
        self.game = game;
        Ok(repairs)
    }
}
