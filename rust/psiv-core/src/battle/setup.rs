//! Battle loading and the opening roll are separate retail phases.
use super::super::fighters::Side;
use super::super::order::roll_priority;
use super::{Battle, BattleEvent, FighterId, Priority, Rolls};

/// A seated battle whose `loc_B62A` has not run. Consuming it makes the
/// opening roll a one-time operation, after the stage fade and DMA sweep.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedBattle {
    pub(super) battle: Battle,
    pub(super) ambush_chance: u8,
    pub(super) boss: bool,
    pub(super) enemies: Vec<FighterId>,
}

impl PreparedBattle {
    /// The post-initialization enemy objects present before the opening roll.
    /// Dormant formation slots remain cached in the roster for later refills.
    #[must_use]
    pub fn present_enemies(&self) -> &[FighterId] {
        &self.enemies
    }

    /// `loc_B62A` (`ps4.asm:17436`), on its original frame, after setup.
    pub fn roll(mut self, rolls: &mut impl Rolls) -> (Battle, Vec<BattleEvent>) {
        let rolled = roll_priority(
            self.battle.roster().highest_party_agility(),
            self.ambush_chance,
            self.boss,
            rolls,
        );
        let priority = if self.battle.scripted_latch {
            Priority::Ambush
        } else {
            rolled
        };
        self.battle.pending_priority = priority;
        if priority == Priority::Ambush {
            // `loc_B62A`'s tail sets bit 3 on every occupied enemy slot.
            for fighter in self
                .battle
                .roster
                .iter_mut()
                .filter(|f| f.id.side() == Side::Enemy)
            {
                fighter.reaction_flags |= super::super::fighters::reaction::AMBUSH;
            }
        }
        (
            self.battle,
            vec![BattleEvent::Started {
                priority,
                enemies: self.enemies,
            }],
        )
    }
}
