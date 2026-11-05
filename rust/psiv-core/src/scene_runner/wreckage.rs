//! `Event_WreckageEngine`'s party reads ([`WreckageOp`]): who leads, and
//! whose portrait the resumed text shows. The scene is `scenes/wreckage.rs`.

use super::SceneRunner;
use crate::scene::{SceneEffect, SceneFault, WreckageOp};
use crate::state::{CharId, GameState};

/// `CharID_Chaz` and `CharID_Rika`, the two bytes `loc_6DCBE` passes over.
const PASSED_OVER: [CharId; 2] = [CharId(0), CharId(5)];

/// The `$F4` operand `loc_6DCBE` writes: the first party byte that is
/// neither Rika nor Chaz, plus one. An empty slot is `$FF`, so it writes 0.
#[must_use]
pub fn resume_portrait(party: &[Option<CharId>]) -> u8 {
    for slot in party {
        match slot {
            Some(who) if PASSED_OVER.contains(who) => {}
            Some(who) => return who.0.wrapping_add(1),
            None => return 0,
        }
    }
    0
}

impl SceneRunner {
    pub(super) fn wreckage_op(
        &mut self,
        op: WreckageOp,
        state: &GameState,
        effects: &mut Vec<SceneEffect>,
    ) -> Option<SceneFault> {
        match op {
            WreckageOp::BranchIfLeads {
                who,
                if_leads,
                if_follows,
            } => {
                let follows = state
                    .party()
                    .iter()
                    .position(|slot| *slot == Some(who))
                    .is_some_and(|slot| slot > 0);
                let target = usize::from(if follows { if_follows } else { if_leads });
                if target > self.scene.len() {
                    return Some(SceneFault::BadJump { target });
                }
                self.pc = target;
            }
            WreckageOp::ResumePortraitFromParty => {
                effects.push(SceneEffect::ResumePortrait {
                    portrait: resume_portrait(&state.party()),
                });
                self.pc += 1;
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::resume_portrait;
    use crate::state::CharId;

    fn party(ids: &[u8]) -> [Option<CharId>; 5] {
        let mut slots = [None; 5];
        for (slot, id) in ids.iter().enumerate() {
            slots[slot] = Some(CharId(*id));
        }
        slots
    }

    #[test]
    fn the_first_member_past_chaz_and_rika_speaks() {
        assert_eq!(resume_portrait(&party(&[0, 5, 6, 4])), 7, "Demi");
        assert_eq!(
            resume_portrait(&party(&[5, 4, 0])),
            5,
            "Gryz, wherever Chaz is"
        );
        assert_eq!(resume_portrait(&party(&[3, 0, 5])), 4, "Rune leading");
    }

    /// Negative control: with only Chaz and Rika the scan meets an empty
    /// slot's `$FF` and writes 0, which hides the portrait.
    #[test]
    fn an_empty_slot_hides_the_portrait() {
        assert_eq!(resume_portrait(&party(&[0, 5])), 0);
    }
}
