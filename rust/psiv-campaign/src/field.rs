//! Letting the game run: the controller that waits for a scene, a message box,
//! a chest or a battle to be over, and the one that turns the party.
//!
//! [`Driver::settle`] is what every other controller calls before it decides
//! anything, because a warp, a step or a talk can start something that owns the
//! frames for a while. It presses only what a player presses: Speak to turn a
//! finished page, nothing to wait out a scene, and whatever the battle driver
//! presses to fight.

use psiv_core::Direction;
use psiv_runtime::{Button, CampPage};

use crate::driver::{Driver, dir_pad};
use crate::halt::{Halt, HaltKind, Res};

/// Consecutive quiet frames that count as "the game has handed control back".
///
/// A trigger enters its scene a frame or two after the landing that fired it,
/// so one quiet frame proves nothing.
const QUIET_FRAMES: u32 = 3;

/// What stopped a [`Driver::settle`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Settled {
    /// The party is in the field, at rest, with nothing open.
    Idle,
    /// A yes/no prompt is up and ready for an answer.
    Choice,
}

impl Driver {
    /// Plays frames until the game hands control back.
    ///
    /// Battles are fought with the current policy; finished pages are turned
    /// with Speak; chest results are acknowledged. A yes/no prompt returns
    /// [`Settled::Choice`] when `stop_at_choice` and is a halt otherwise: no
    /// objective answered it.
    ///
    /// # Errors
    ///
    /// A halt from a frame, or [`HaltKind::UnexpectedState`] for a window the
    /// route did not ask for (a shop, the camp, a full pack, a field notice,
    /// an unanswered prompt).
    pub fn settle(&mut self, stop_at_choice: bool) -> Res<Settled> {
        let mut quiet = 0;
        loop {
            if self.session().battle_active() {
                self.fight()?;
                quiet = 0;
                continue;
            }
            if let Some(camp) = self.session().camp_view() {
                match camp.page {
                    CampPage::LootMessage => {
                        let message = camp.message.clone();
                        self.note(format!("chest: {}", message.replace('\n', " / ")));
                        self.tap(Button::Speak)?;
                    }
                    CampPage::LootItems => {
                        return Err(Halt::new(
                            HaltKind::UnexpectedState,
                            format!("a chest found a full item pack: {}", camp.message),
                        ));
                    }
                    other => {
                        return Err(Halt::new(
                            HaltKind::UnexpectedState,
                            format!("the camp window ({other:?}) is open and no objective asked"),
                        ));
                    }
                }
                quiet = 0;
                continue;
            }
            if self.session().shop_view().is_some() {
                return Err(Halt::new(
                    HaltKind::UnexpectedState,
                    "a shop window is open and no objective asked",
                ));
            }
            if self.session().destination_view().is_some() {
                return Err(Halt::new(
                    HaltKind::UnexpectedState,
                    "the ship's destination menu is open and no objective asked",
                ));
            }
            if let Some(notice) = self.runtime().field_notice() {
                return Err(Halt::new(
                    HaltKind::UnexpectedState,
                    format!(
                        "a field notice ({notice:?}) is up; acknowledging it is not on the \
                         session's pad surface"
                    ),
                ));
            }
            if let Some(view) = self.runtime().dialogue_view() {
                quiet = 0;
                if let Some(choice) = view.choice {
                    if !choice.ready {
                        self.tick(psiv_runtime::Pad::NEUTRAL)?;
                    } else if stop_at_choice {
                        return Ok(Settled::Choice);
                    } else {
                        return Err(Halt::new(
                            HaltKind::UnexpectedState,
                            "a yes/no prompt opened and the route does not answer it here",
                        ));
                    }
                } else if view.dismissable {
                    self.tap(Button::Speak)?;
                } else {
                    self.tick(psiv_runtime::Pad::NEUTRAL)?;
                }
                continue;
            }
            let busy = {
                let runtime = self.runtime();
                runtime.scene_active()
                    || crate::driver::is_stepping(runtime)
                    || runtime.loot_state().is_some()
            };
            if busy {
                quiet = 0;
                self.tick(psiv_runtime::Pad::NEUTRAL)?;
                continue;
            }
            quiet += 1;
            if quiet >= QUIET_FRAMES {
                // A battle just ended: the party is cured through the camp
                // before anything else walks it into the next one.
                if self.take_recovery_due() && self.recover()? {
                    quiet = 0;
                    continue;
                }
                return Ok(Settled::Idle);
            }
            self.tick(psiv_runtime::Pad::NEUTRAL)?;
        }
    }

    /// Turns the party to face `direction` without stepping.
    ///
    /// Pressing a direction at a walkable cell would walk, so a face toward
    /// open ground is refused rather than risked.
    ///
    /// # Errors
    ///
    /// [`HaltKind::UnexpectedState`] when the faced cell is walkable or the
    /// party did not turn; a halt from a frame.
    pub fn face(&mut self, direction: Direction) -> Res {
        self.settle(false)?;
        let runtime = self.runtime();
        if runtime.state().facing() == direction {
            return Ok(());
        }
        let cell = runtime.state().cell();
        let ahead = runtime.map().neighbor(cell, direction);
        if ahead.is_some_and(|c| runtime.map().is_walkable(c)) {
            return Err(Halt::new(
                HaltKind::UnexpectedState,
                format!(
                    "cannot turn {direction:?} at ({},{}) without stepping: the cell ahead is open",
                    cell.x, cell.y
                ),
            ));
        }
        self.tick(dir_pad(direction))?;
        self.neutral(2)?;
        let after = self.runtime().state();
        if after.facing() != direction || after.cell() != cell {
            return Err(Halt::new(
                HaltKind::Stuck,
                format!("pressing {direction:?} did not turn the party in place"),
            ));
        }
        Ok(())
    }
}
