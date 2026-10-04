//! The ship: the `board` controller.
//!
//! A boarding row is a map trigger, not a menu: stepping onto it starts
//! `Cutscene_InsideSpaceship` (`RunEvent_EnterSpaceship`, `ps4.asm:115743`), whose
//! destination menu (`loc_63BC4`, `ps4.asm:133499`) is the session's
//! destination mode. The controller holds the step until that window is up,
//! walks its cursor onto the wanted world with Up and Down, presses Speak and
//! waits the flight out. Which rows exist, the confirm message, the wait and the
//! legs of the flight are the runtime's; the controller reads
//! [`psiv_runtime::DestinationView`] and presses buttons.

use psiv_core::{Direction, WORLD_COUNT};
use psiv_data::GameData;
use psiv_runtime::{Button, DestinationPhase, Pad};

use crate::driver::{Driver, dir_pad};
use crate::halt::{Halt, HaltKind, Res};
use crate::route::NameOrId;

/// Frames the held step may take to start the scene and open the menu: the
/// scene's own warm-up and its first ops, nowhere near a walk.
const MENU_FRAMES: u32 = 600;

/// The world a route's `to` names: a `World_Index`, or a name from the pack's
/// `loc_2AAA7A` table compared without case.
///
/// # Errors
///
/// A reason when the id is not a world, or a name needs a pack that has no
/// destination screen, or names nothing.
pub fn world_of(to: &NameOrId, data: &GameData) -> Result<u8, String> {
    match to {
        NameOrId::Id(id) => u8::try_from(*id)
            .ok()
            .filter(|world| *world < WORLD_COUNT)
            .ok_or_else(|| format!("{id} is not a World_Index (0..{WORLD_COUNT})")),
        NameOrId::Name(name) => {
            let menu = data.ship_menu().ok_or_else(|| {
                format!(
                    "the pack has no destination screen to resolve {name:?}; \
                     rebuild it or name the world by number"
                )
            })?;
            (0..WORLD_COUNT)
                .find(|world| {
                    menu.name(*world)
                        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(name))
                })
                .ok_or_else(|| format!("no world is called {name:?}"))
        }
    }
}

impl Driver {
    /// Steps `step` onto the boarding row, picks `world` in the destination
    /// menu and flies.
    ///
    /// # Errors
    ///
    /// [`HaltKind::UnexpectedState`] when the step opens no menu,
    /// [`HaltKind::MenuEntryMissing`] when the menu does not list the world, or
    /// a halt from a frame.
    pub fn board(&mut self, step: Direction, world: u8) -> Res {
        self.settle(false)?;
        let mut opened = false;
        for _ in 0..MENU_FRAMES {
            if self.session().destination_view().is_some() {
                opened = true;
                break;
            }
            self.tick(dir_pad(step))?;
        }
        if !opened {
            return Err(Halt::new(
                HaltKind::UnexpectedState,
                format!("stepping {step:?} opened no destination menu"),
            ));
        }
        // Let go of the step so the next press is a fresh edge, then wait out
        // the window's opening: the cartridge discards every pad edge while
        // Window_Draw runs (`DestinationPhase::Opening`), as a player's early
        // presses would be.
        self.tick(Pad::NEUTRAL)?;
        for _ in 0..MENU_FRAMES {
            match self.session().destination_view().map(|view| view.phase) {
                Some(DestinationPhase::Choosing) => break,
                Some(_) => self.tick(Pad::NEUTRAL)?,
                None => {
                    return Err(Halt::new(
                        HaltKind::UnexpectedState,
                        "the destination menu closed before it took input",
                    ));
                }
            }
        }
        let (row, rows) = {
            let view = self.session().destination_view().ok_or_else(|| {
                Halt::new(HaltKind::UnexpectedState, "the destination menu closed")
            })?;
            let row = view.rows.iter().position(|listed| *listed == world);
            (row, view.rows.clone())
        };
        let Some(row) = row else {
            return Err(Halt::new(
                HaltKind::MenuEntryMissing,
                format!(
                    "the destination menu does not list World_Index {world}; it lists {rows:?}"
                ),
            ));
        };
        self.cursor_to(
            "destination",
            |d| d.session().destination_view().map(|view| view.cursor),
            row,
            Some(rows.len()),
        )?;
        self.tap(Button::Speak)?;
        // The message types, the wait runs and the windows close; then the
        // scene flies on its own and the ordinary settle runs it out.
        while self.session().destination_view().is_some() {
            self.tick(Pad::NEUTRAL)?;
        }
        self.settle(false)?;
        Ok(())
    }
}
