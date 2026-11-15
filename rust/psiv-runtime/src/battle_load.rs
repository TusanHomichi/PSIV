//! `loc_6B20` / `loc_6BA4`: the battle stage before its opening roll.
use crate::session::BattleStart;
use crate::{BridgeError, Runtime};
use psiv_core::battle::{Battle, PartyMember, PreparedBattle};

/// The loader's submitted plane and palette state. The shell draws this view;
/// it owns no scroll, DMA or palette clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BattleLoadView {
    /// The formation whose prepared stage is being revealed.
    pub start: BattleStart,
    /// Plane A vertical scroll, in pixels, in the VDP convention.
    pub foreground_y: i16,
    /// Plane B vertical scroll, in pixels, in the VDP convention.
    pub background_y: i16,
    /// Uploaded 16-pixel rows of plane A, beginning at row zero.
    pub foreground_rows: u8,
    /// First uploaded 16-pixel row of plane B (uploads proceed upward).
    pub background_first_row: u8,
    /// `Battle_UpdateEnemySprites` runs once before `loc_6B20`
    /// (`ps4.asm:10028`); its first timer underflow selects frame one.
    pub enemy_animation_phase_ticks: usize,
    fade_steps: u8,
}

impl BattleLoadView {
    pub(crate) fn new(start: BattleStart, steps: u16) -> Self {
        // `$4104` starts at 14 and is decremented before each queue: fourteen
        // uploads, then the fifteenth VInt checks the unchanged palette.
        let uploads = steps.min(14) as u8;
        Self {
            start,
            foreground_y: -224 + i16::from(uploads) * 16,
            background_y: 224 - i16::from(uploads) * 16,
            foreground_rows: uploads,
            background_first_row: 14 - uploads,
            enemy_animation_phase_ticks: 1,
            fade_steps: steps.min(14) as u8,
        }
    }

    /// `loc_6B56`: lower each nibble by one toward the target, not by an
    /// alpha blend or by subtracting the same amount from the final colour.
    #[must_use]
    pub fn palette_word(self, target: u16) -> u16 {
        let level = 14_u16.saturating_sub(u16::from(self.fade_steps));
        [0, 4, 8].into_iter().fold(0, |out, shift| {
            out | (((target >> shift) & 0xF).max(level) << shift)
        })
    }
}

impl Runtime {
    pub(crate) fn prepare_battle_load(
        &self,
        start: BattleStart,
        party: Vec<PartyMember>,
    ) -> Result<PreparedBattle, BridgeError> {
        let set = self
            .battles
            .as_ref()
            .ok_or_else(|| BridgeError::Rejected("battles not enabled".into()))?;
        let record = match start {
            BattleStart::Encounter(id) => set.table.formation(id),
            BattleStart::EventBattle(index) => set.boss_formations.get(&index),
        }
        .ok_or_else(|| BridgeError::Rejected(format!("unknown battle {start:?}")))?;
        if party.is_empty() {
            return Err(BridgeError::Rejected(
                "battle requested with an empty party".into(),
            ));
        }
        Battle::prepare(
            record,
            party,
            &set.data,
            matches!(start, BattleStart::EventBattle(_)) && self.vehicle.is_none(),
            self.vehicle.is_some(),
        )
        .map_err(|error| BridgeError::Rejected(error.to_string()))
    }

    pub(crate) fn take_battle_load(
        &mut self,
        start: BattleStart,
        party: Vec<PartyMember>,
    ) -> Result<PreparedBattle, BridgeError> {
        match self.prepared_battle.take() {
            Some((loaded, battle)) if loaded == start => Ok(battle),
            _ => self.prepare_battle_load(start, party),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn planes_upload_in_opposite_directions_and_palette_stops_at_target() {
        let start = BattleStart::EventBattle(7);
        for step in 0..=18 {
            let view = BattleLoadView::new(start, step);
            let n = step.min(14) as u8;
            assert_eq!(view.enemy_animation_phase_ticks, 1);
            assert_eq!(
                (view.foreground_y, view.background_y),
                (-224 + i16::from(n) * 16, 224 - i16::from(n) * 16)
            );
            assert_eq!(
                (view.foreground_rows, view.background_first_row),
                (n, 14 - n)
            );
            for target in [0, 0x0246, 0x0ACE, 0x0EEE] {
                let word = view.palette_word(target);
                for shift in [0, 4, 8] {
                    assert_eq!(
                        (word >> shift) & 15,
                        ((target >> shift) & 15).max(14 - u16::from(n))
                    );
                }
            }
        }
        // A dark target stays much lighter than its final word mid-fade;
        // adding a tint to final pixels or only fading the backdrop fails.
        assert_eq!(BattleLoadView::new(start, 7).palette_word(0x024A), 0x077A);
    }
}
