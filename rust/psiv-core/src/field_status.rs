//! `DoCharStatsUpdate`: ailments and android recovery on completed foot steps.
//! These two byte counters belong to the loaded map, not to an ailment or save.

use crate::battle::{Rolls, status};
use crate::{CharId, GameState};

/// Volatile `Poison_Frame_Counter` and `Paralyze_Frame_Counter`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FieldStatusClock {
    poison: u8,
    paralysis: u8,
}

/// Visible consequences of one `DoCharStatsUpdate` pass.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct FieldStatusResult {
    /// Characters whose poison reduced HP to zero, in party-slot order.
    pub fallen: Vec<CharId>,
    /// At least one poisoned character took damage and survived.
    pub flash_red: bool,
    /// Every occupied party slot carries the human Dead bit.
    pub perished: bool,
}

impl FieldStatusClock {
    /// A complete map load (including battle return) resets both counters.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Processes one completed step outside vehicles and transition tiles.
    /// The caller gates movement, windows and scenes. RNG is the shared field
    /// UpdateRNGSeed stream, with one draw per eligible paralyzed character.
    pub fn step(
        &mut self,
        game: &mut GameState,
        poison_map: bool,
        rolls: &mut impl Rolls,
    ) -> FieldStatusResult {
        self.poison = self.poison.wrapping_add(1) & 3;
        self.paralysis = self.paralysis.wrapping_add(1);
        let mut result = FieldStatusResult::default();
        let mut count = 0;
        let mut dead = 0;
        for who in game
            .party()
            .into_iter()
            .take_while(Option::is_some)
            .flatten()
        {
            let Some(stats) = game.roster_mut().get_mut(who) else {
                continue;
            };
            count += 1;
            if poison_map && self.poison == 0 && stats.status & status::POISONED != 0 {
                // The 68000 subtracts a word without a zero-HP guard.
                stats.curr_hp = stats.curr_hp.wrapping_sub(1);
                if stats.curr_hp == 0 {
                    stats.status &= !(status::POISONED | status::PARALYZED);
                    stats.status |= status::DEAD;
                    result.fallen.push(who);
                } else {
                    result.flash_red = true;
                }
            }
            if stats.status & status::PARALYZED != 0
                && self.paralysis > 5
                && (self.paralysis >= 25 || rolls.next_roll() & 7 == 0)
            {
                // No stat restoration here: retail clears only the bit.
                stats.status &= !status::PARALYZED;
            }
            if stats.status & status::DEAD != 0 {
                dead += 1;
            }
            // Literal character-record addresses, not profession. Android
            // shutdown does not inhibit this heal or get cleared by it.
            if matches!(who.0, 6 | 7) && stats.curr_hp < stats.max_hp {
                stats.curr_hp += 1;
            }
        }
        result.perished = count != 0 && dead == count;
        result
    }
}

#[cfg(test)]
#[path = "field_status_tests.rs"]
mod tests;
