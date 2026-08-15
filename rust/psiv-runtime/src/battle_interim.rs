//! Small runtime seams that keep battle presentation out of `GameState`.

use crate::Runtime;

impl Runtime {
    /// Ends the battle and re-arms the encounter grace period, as the
    /// cartridge resets `$FFFFECE4` to 10 after every fight.
    pub fn finish_battle(&mut self) {
        self.battle = None;
        if let Some(set) = self.battles.as_mut() {
            set.clock.reset();
        }
    }

    /// Interim defeat policy pending the save system: put every current party
    /// member back on their feet at 1 HP so the field can continue. This is a
    /// runtime seam, not renderer access to `GameState`, and deliberately
    /// clears only the two dead bits; poison, paralysis and other statuses are
    /// still state that a later defeat/save design must adjudicate.
    pub fn revive_interim(&mut self) {
        let dead = psiv_core::DEAD_STATUS_MASK;
        for id in self.game.party_members() {
            if let Some(stats) = self.game.roster_mut().get_mut(id) {
                stats.curr_hp = 1;
                stats.status &= !dead;
            }
        }
    }
}
