//! Input and save-slot boot helpers for the field node.

use godot::classes::Input;
use godot::prelude::*;
use psiv_core::Direction;
use psiv_runtime::{Button, Pad};

/// This frame's joypad byte, from the engine's actions.
///
/// `ui_accept` is Genesis C — `ButtonSpeak`, the talk and confirm button —
/// and `ui_cancel` is B. Camp and Start have no binding of their own yet:
/// the field's camp opens on cancel and the title reads confirm, so nothing
/// here invents one. `synthetic_speak` is the debug retail pace's own press;
/// it is a press in the pad, not a private path into the dialogue.
pub(crate) fn read_pad(synthetic_speak: bool) -> Pad {
    let input = Input::singleton();
    let mut pad = Pad::NEUTRAL;
    for (action, button) in [
        ("ui_up", Button::Up),
        ("ui_down", Button::Down),
        ("ui_left", Button::Left),
        ("ui_right", Button::Right),
        ("ui_cancel", Button::Cancel),
        ("ui_accept", Button::Speak),
    ] {
        if input.is_action_pressed(action) {
            pad = pad.with(button);
        }
    }
    if synthetic_speak {
        pad = pad.with(Button::Speak);
    }
    pad
}

/// One input per tick. Confirm wins over movement; the cartridge reads them
/// on separate paths and the talk takes the frame.
pub(crate) fn read_input() -> psiv_core::Input {
    let input = Input::singleton();
    if input.is_action_pressed("ui_accept") {
        debug_trace(psiv_core::Input::Action);
        return psiv_core::Input::Action;
    }
    let held = [
        ("ui_up", Direction::Up),
        ("ui_down", Direction::Down),
        ("ui_left", Direction::Left),
        ("ui_right", Direction::Right),
    ]
    .into_iter()
    .find(|(action, _)| input.is_action_pressed(*action));
    let resolved = match held {
        Some((_, dir)) => psiv_core::Input::Direction(dir),
        None => psiv_core::Input::Neutral,
    };
    debug_trace(resolved);
    resolved
}

/// `PSIV_DEBUG_INPUT=1`: log the resolved [`psiv_core::Input`] on every
/// change, splitting keymap faults (nothing arrives) from field-control
/// faults (input arrives, nothing moves). Debug-selector family.
fn debug_trace(resolved: psiv_core::Input) {
    use std::sync::Mutex;
    static LAST: Mutex<Option<psiv_core::Input>> = Mutex::new(None);
    if !std::env::var("PSIV_DEBUG_INPUT").is_ok_and(|value| value == "1") {
        return;
    }
    let mut last = LAST.lock().expect("input trace lock");
    if *last != Some(resolved) {
        godot_print!("input: {resolved:?}");
        *last = Some(resolved);
    }
}

pub(crate) fn requested_save_slot() -> Option<usize> {
    let value = std::env::var("PSIV_LOAD_SLOT").ok().or_else(|| {
        std::env::args().find_map(|argument| {
            argument
                .strip_prefix("--psiv-load-slot=")
                .map(str::to_owned)
        })
    })?;
    match value.parse::<usize>() {
        Ok(slot @ 1..=3) => Some(slot - 1),
        _ => {
            godot_error!("PSIV_LOAD_SLOT must be a visible slot number 1, 2, or 3");
            None
        }
    }
}
