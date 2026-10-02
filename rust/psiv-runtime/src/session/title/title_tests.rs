//! The title flow's own tests: the phases, the rows and the slot rules.
//!
//! Fixtures are the mode alone: no pack and no runtime, because every rule
//! asserted here is the flow's own. The pad-driven path from power-on to the
//! opening's first line lives in `tests/session_title.rs`.

use psiv_core::Direction;

use crate::Pad;
use crate::pad::Button;

use super::*;

fn title(slots: [bool; 3]) -> TitleMode {
    TitleMode::new(slots, false)
}

/// The press-start hold is the retail `#$233` and it opens the option
/// window: 563 frames of neutral, then one more frame with the window up.
#[test]
fn the_press_start_hold_opens_the_option_window_on_the_retail_frame() {
    let mut title = title([false; 3]);
    // The Sega hold and the reveal transfer first.
    for _ in 0..(SEGA_HOLD_TICKS + 1) {
        assert_eq!(title.frame(Pad::NEUTRAL), TitleAction::None);
    }
    assert_eq!(title.view().phase, TitlePhase::Reveal);
    for _ in 0..(TITLE_REVEAL_TICKS + 1) {
        assert_eq!(title.frame(Pad::NEUTRAL), TitleAction::None);
    }
    assert_eq!(title.view().phase, TitlePhase::PressStart);
    for _ in 1..PRESS_START_HOLD_TICKS {
        assert_eq!(title.frame(Pad::NEUTRAL), TitleAction::None);
    }
    assert_eq!(
        title.view().phase,
        TitlePhase::PressStart,
        "the hold lasts the whole count"
    );
    assert_eq!(title.frame(Pad::NEUTRAL), TitleAction::None);
    assert_eq!(title.view().phase, TitlePhase::Menu);
    assert_eq!(title.view().window, TitleWindow::NoSave);
}

/// Negative control: a save-less card's option window is the single START
/// row (`Title_NoSavedData`, `ps4.asm:87542`).
#[test]
fn the_save_less_option_window_accepts_its_one_row() {
    let mut title = title([false; 3]);
    title.phase = TitlePhase::Menu;
    assert_eq!(title.view().window, TitleWindow::NoSave);
    title.move_menu(Direction::Down);
    assert_eq!(title.view().cursor, 0, "a one-row window cannot move");
    assert_eq!(title.accept(), TitleAction::Start);
}

/// An empty slot is refused and nothing changes: the phase, the cursor and
/// the slot list all stay where they were.
#[test]
fn an_empty_slot_is_refused_with_no_state_change() {
    let mut title = title([true, false, true]);
    title.phase = TitlePhase::Slots;
    title.cursor = 1;
    assert_eq!(title.accept(), TitleAction::None);
    assert_eq!(title.view().phase, TitlePhase::Slots);
    assert_eq!(title.view().cursor, 1);
    assert_eq!(title.view().slots, [true, false, true]);
    // The populated row beside it still continues.
    title.cursor = 2;
    assert_eq!(title.accept(), TitleAction::Continue(2));
}

/// ERASE DATA's ARE YOU SURE? erases on YES and backs out on NO.
#[test]
fn erase_confirms_on_the_first_row_and_backs_out_on_the_second() {
    let mut title = title([true, false, false]);
    title.phase = TitlePhase::EraseSlots;
    title.cursor = 0;
    assert_eq!(title.accept(), TitleAction::None);
    assert_eq!(title.view().phase, TitlePhase::EraseConfirm);
    // NO first: the list comes back with the cursor on that slot.
    title.cursor = 1;
    assert_eq!(title.accept(), TitleAction::None);
    assert_eq!(title.view().phase, TitlePhase::EraseSlots);
    assert_eq!(title.view().cursor, 0);
    // YES: the row is asked about, and the finished erase clears it.
    title.cursor = 0;
    assert_eq!(title.accept(), TitleAction::None);
    assert_eq!(title.accept(), TitleAction::Erase(0));
    title.finish_erase(0, true);
    assert_eq!(title.view().phase, TitlePhase::Menu);
    assert_eq!(title.view().slots, [false; 3]);
    assert_eq!(title.view().window, TitleWindow::NoSave);
}

/// A failed erase returns to the slot list with the cursor on the slot the
/// player chose, and the row is still there.
#[test]
fn a_failed_erase_keeps_the_row() {
    let mut title = title([true, false, false]);
    title.phase = TitlePhase::EraseSlots;
    title.cursor = 0;
    title.accept();
    assert_eq!(title.accept(), TitleAction::Erase(0));
    title.finish_erase(0, false);
    assert_eq!(title.view().phase, TitlePhase::EraseSlots);
    assert_eq!(title.view().cursor, 0);
    assert_eq!(title.view().slots, [true, false, false]);
}

/// The three-row option window wraps: Up from the first row is the last.
#[test]
fn the_option_cursor_wraps_over_its_three_rows() {
    let mut title = title([true, false, false]);
    title.phase = TitlePhase::Menu;
    assert_eq!(title.view().window, TitleWindow::SaveOptions);
    title.move_menu(Direction::Up);
    assert_eq!(title.view().cursor, 2, "ERASE DATA");
    title.move_menu(Direction::Down);
    assert_eq!(title.view().cursor, 0, "CONTINUE");
    assert_eq!(title.accept(), TitleAction::None);
    assert_eq!(title.view().phase, TitlePhase::Slots);
    assert_eq!(title.view().cursor, 0, "the first populated row");
}

/// Every phase has the option window's three buttons: Camp confirms as
/// Speak and Start do (`TitleRoutine_PickOption`, `ps4.asm:87514`).
#[test]
fn the_option_rows_confirm_on_speak_camp_and_start() {
    for button in [Button::Speak, Button::Camp, Button::Start] {
        let mut title = title([false; 3]);
        title.phase = TitlePhase::Menu;
        assert_eq!(
            title.frame(Pad::new(button)),
            TitleAction::Start,
            "{} confirms START",
            button.name()
        );
        // A held key is one press, not a stream of rows.
        assert_eq!(title.frame(Pad::new(button)), TitleAction::None);
        assert_eq!(title.frame(Pad::NEUTRAL), TitleAction::None);
        assert_eq!(
            title.frame(Pad::new(button)),
            TitleAction::Start,
            "the next press is fresh"
        );
    }
}

/// A held accept button does not also step the cursor: the cartridge's
/// confirm branch runs before its cursor routine on the same press, so a
/// Camp+Down press answers the row under the cursor instead of moving first
/// and answering the row beside it.
#[test]
fn an_accept_button_does_not_step_the_cursor() {
    let mut title = title([true, false, false]);
    title.phase = TitlePhase::Menu;
    title.cursor = 2; // ERASE DATA
    let both = Pad::of(&[Button::Camp, Button::Down]);
    assert_eq!(title.frame(both), TitleAction::None);
    // A Down read first would wrap the cursor onto CONTINUE and open the slot
    // list; the confirm wins and opens ERASE DATA's own list.
    assert_eq!(title.view().phase, TitlePhase::EraseSlots);
}
