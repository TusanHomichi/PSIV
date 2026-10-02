//! Input and save-slot boot helpers for the field node.
//!
//! The pad is the shell's whole output as an input device; the title's own
//! reading of it is the runtime's (`psiv-runtime/src/session/title.rs`).

use godot::classes::Input;
use godot::prelude::*;
use psiv_runtime::{Button, Pad};

use crate::{Field, RETAIL_DISMISS_HOLD_FRAMES, retail_pace_enabled};

/// This frame's joypad byte, from the engine's actions.
///
/// Each cartridge button has its own action: `ui_accept` is Genesis C,
/// `ButtonSpeak`; `ui_cancel` is B, `ButtonCancel`; `psiv_camp` is A,
/// `ButtonCamp`, which opens the field's camp menu (`FieldControls_GetInput`,
/// `ps4.asm:114890`) and confirms in the menus; `psiv_start` is `ButtonStart`.
/// `synthetic_speak` is the debug retail pace's own press; it is a press in the
/// pad, not a private path into the dialogue.
///
/// This is the shell's whole output as an input device: the pad goes to
/// `Session::frame`, which resolves the field's direction and confirm
/// (`Pad::field_input`, in the cartridge's own order), reads the menus' presses
/// as edges and hands the byte to the dialogue runner, whose presses are its
/// own edges.
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
        ("psiv_camp", Button::Camp),
        ("psiv_start", Button::Start),
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

/// `PSIV_DEBUG_INPUT=1`: log the resolved field input on every change,
/// splitting keymap faults (nothing arrives) from field-control faults (input
/// arrives, nothing moves). Debug-selector family.
pub(crate) fn debug_trace(resolved: psiv_core::Input) {
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

impl Field {
    /// The pad the session's frame is driven with: this frame's buttons, plus
    /// the retail-pace debug harness's synthetic Speak press.
    ///
    /// Retail pacing holds a completed page for the oracle tapes' four frames
    /// and then dismisses it; that press is an ordinary Speak press in the pad,
    /// so the runtime's own edge and swallow rules decide it. The dialogue view
    /// is read here either way — for the pace's dismiss hold, and for the
    /// `PSIV_DEBUG_INPUT` page trace, which must see the page *before* the
    /// frame spends the press that dismisses it.
    pub(crate) fn frame_pad(&mut self) -> Pad {
        let dismissable = self.session.as_ref().is_some_and(|session| {
            session
                .runtime()
                .dialogue_view()
                .is_some_and(|view| view.dismissable)
        });
        let synthetic_speak = retail_pace_enabled()
            && dismissable
            && self.retail_dialogue_wait >= RETAIL_DISMISS_HOLD_FRAMES;
        if retail_pace_enabled() && dismissable {
            self.retail_dialogue_wait = self.retail_dialogue_wait.saturating_add(1);
        } else if !dismissable {
            self.retail_dialogue_wait = 0;
        }
        let pad = read_pad(synthetic_speak);
        if std::env::var("PSIV_DEBUG_INPUT").is_ok_and(|value| value == "1")
            && pad
                .pressed(self.dialogue_pad)
                .held(psiv_runtime::Button::Speak)
            && let Some(view) = self
                .session
                .as_ref()
                .and_then(|session| session.runtime().dialogue_view())
            && view.page_end.is_some()
        {
            godot_print!("dialogue page {:?}: {:?}", view.page_end, view.lines);
        }
        self.dialogue_pad = pad;
        if synthetic_speak {
            self.retail_dialogue_wait = 0;
        }
        pad
    }
}
