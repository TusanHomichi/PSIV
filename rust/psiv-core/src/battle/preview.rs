//! Local copies for command valuation, through the same support dispatcher
//! and carrier-specific damage resolver as a real turn.

use super::{BattleData, BattleDataError, BattleEvent, FighterId, Rolls, Roster};

impl Roster {
    /// A copy with a successful support effect applied to one fighter.
    /// The effect's real derived-stat and integer-width rules are preserved.
    /// Chance/target eligibility is the command menu's responsibility.
    #[must_use]
    pub fn preview_support(&self, target: FighterId, effect: u8, power: u16) -> Option<Self> {
        let mut roster = self.clone();
        let stats = &mut roster.get_mut(target)?.stats;
        super::player_effect::support(
            stats,
            target,
            target,
            &super::player_effect::Effect {
                effect,
                power,
                threshold: 0,
                resistance: 0,
                element: 0,
            },
            &mut Vec::new(),
        );
        Some(roster)
    }

    /// Preview one chosen enemy ability on a copy, with caller-owned rolls.
    /// `None` means its damage object is not supported by the proven carrier
    /// routes (including non-damage abilities). A supported but sealed damage
    /// object instead returns `Some` without damage events.
    /// No ability selection, AI scan or turn ordering is predicted.
    ///
    /// # Errors
    /// The same data errors as the physical attack resolver.
    pub fn preview_enemy_damage(
        &self,
        actor: FighterId,
        ability: u8,
        intended: FighterId,
        data: &BattleData,
        rolls: &mut impl Rolls,
    ) -> Result<Option<Vec<BattleEvent>>, BattleDataError> {
        let mut roster = self.clone();
        let mut events = Vec::new();
        if ability == 0 {
            super::action::resolve_attack(
                &mut roster,
                actor,
                Some(intended),
                data,
                rolls,
                &mut events,
            )?;
        } else if !super::enemy_damage::resolve_damage_skill(
            &mut roster,
            actor,
            ability,
            Some(intended),
            data,
            rolls,
            &mut events,
        ) {
            return Ok(None);
        }
        Ok(Some(events))
    }
}

#[cfg(test)]
#[path = "preview_tests.rs"]
mod tests;
