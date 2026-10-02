//! The battle driver: pad presses that carry a [`Policy`]'s choices through the
//! session's own battle menus.
//!
//! Every press is an edge (the cartridge's menus read `Joypad_Pressed`), so a
//! press is always followed by a released frame. The menu is read from the
//! last [`psiv_runtime::BattleView`] the session produced; nothing here knows a
//! battle rule. The windows move one row per press and the main options wrap
//! (`session/battle/menu/mod.rs` documents which window does what).

use psiv_runtime::{Button, CommandMenuView, MenuPage, MenuView, Pad, Runtime, TargetKind};

use crate::driver::Driver;
use crate::halt::Res;
use crate::policy::{Intent, Policy};

/// Fights the battle the session is in until the field takes the frame back.
///
/// # Errors
///
/// A [`crate::halt::Halt`] from the driver: a fault, an unsupported ability, a
/// defeat, or the budget.
pub fn fight(driver: &mut Driver, policy: &mut dyn Policy) -> Res {
    let mut released = true;
    let mut idle_frames = 0_u32;
    while driver.session().battle_active() {
        let button = if released {
            wanted_button(driver, policy)
        } else {
            None
        };
        match button {
            Some(button) => {
                driver.tick(Pad::new(button))?;
                released = false;
                idle_frames = 0;
            }
            None => {
                driver.tick(Pad::NEUTRAL)?;
                released = true;
                idle_frames += 1;
            }
        }
        // A beat that waits for no press and a window that is not open: keep
        // the frames flowing, but never sit on a state nothing can change.
        if idle_frames > 2_000 {
            return Err(crate::halt::Halt::new(
                crate::halt::HaltKind::Stuck,
                "the battle made no progress for 2000 frames",
            ));
        }
    }
    policy.end_round();
    Ok(())
}

/// The button this frame's battle state asks for, or `None` to release.
fn wanted_button(driver: &Driver, policy: &mut dyn Policy) -> Option<Button> {
    let view = driver.battle_view()?;
    if !view.ready {
        policy.end_round();
        // A results page waits for a confirm; a beat times out by itself.
        return view
            .current
            .is_some_and(|beat| beat.waits_for_confirm)
            .then_some(Button::Speak);
    }
    match view.menu.as_ref()? {
        // The main options: row 0 is COMD (ATTAC when mounted).
        MenuView::Top { cursor } => {
            let boss = driver.battles().last().is_some_and(|b| b.kind == "event");
            let row = if policy.wants_run(boss) { 2 } else { 0 };
            Some(toward_wrapping(*cursor, row, 3))
        }
        MenuView::VehicleSkills { slots, cursor } => {
            let wanted = slots.iter().position(|slot| slot.current > 0)?;
            Some(toward(*cursor, wanted))
        }
        MenuView::Commands(menu) => command_button(menu, driver.runtime(), policy),
    }
}

fn toward(current: usize, wanted: usize) -> Button {
    match current.cmp(&wanted) {
        std::cmp::Ordering::Less => Button::Down,
        std::cmp::Ordering::Greater => Button::Up,
        std::cmp::Ordering::Equal => Button::Speak,
    }
}

/// Like [`toward`] on a window that wraps: the shorter way round.
fn toward_wrapping(current: usize, wanted: usize, rows: usize) -> Button {
    if current == wanted {
        return Button::Speak;
    }
    let down = (wanted + rows - current) % rows;
    if down <= rows - down {
        Button::Down
    } else {
        Button::Up
    }
}

/// The per-character window.
fn command_button(
    menu: &CommandMenuView,
    runtime: &Runtime,
    policy: &mut dyn Policy,
) -> Option<Button> {
    if menu.actor.is_none() {
        // Nobody can act: the order is the default, and an accept submits it.
        return Some(Button::Speak);
    }
    let intent = policy.choose(menu, runtime);
    match menu.page {
        MenuPage::Actions => {
            let row = match intent {
                Intent::Attack => 0,
                Intent::Technique { .. } => 1,
                Intent::Item { .. } => 3,
                Intent::Defend => 4,
            };
            Some(toward(menu.cursor, row))
        }
        MenuPage::Techniques => {
            let Intent::Technique { id, .. } = intent else {
                return Some(Button::Cancel);
            };
            match menu.techniques.iter().position(|entry| entry.id == id) {
                Some(row) if menu.rows.get(row).is_some_and(|r| r.enabled) => {
                    Some(toward(menu.cursor, row))
                }
                _ => {
                    policy.refuse();
                    Some(Button::Cancel)
                }
            }
        }
        MenuPage::Items => {
            let Intent::Item { name, .. } = intent else {
                return Some(Button::Cancel);
            };
            let row = menu
                .rows
                .iter()
                .position(|row| row.enabled && row.label == name);
            if let Some(row) = row {
                Some(toward(menu.cursor, row))
            } else {
                policy.refuse();
                Some(Button::Cancel)
            }
        }
        MenuPage::Skills => Some(Button::Cancel),
        MenuPage::Targets(kind) => {
            let wanted = match (kind, &intent) {
                (TargetKind::Attack, _) => menu.targets.first().copied(),
                (_, Intent::Technique { target, .. } | Intent::Item { target, .. }) => *target,
                _ => None,
            };
            let row = wanted
                .and_then(|id| menu.targets.iter().position(|t| *t == id))
                .unwrap_or(0);
            Some(toward(menu.cursor, row))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wrapping_main_options_take_the_short_way() {
        assert_eq!(toward_wrapping(0, 0, 3), Button::Speak);
        assert_eq!(toward_wrapping(2, 0, 3), Button::Down);
        assert_eq!(toward_wrapping(1, 0, 3), Button::Up);
        assert_eq!(toward_wrapping(0, 2, 3), Button::Up);
    }

    #[test]
    fn a_list_cursor_steps_one_row_toward_its_target() {
        assert_eq!(toward(0, 3), Button::Down);
        assert_eq!(toward(3, 1), Button::Up);
        assert_eq!(toward(2, 2), Button::Speak);
    }
}
