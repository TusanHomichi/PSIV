//! The session: the cartridge's main loop, one 60 Hz frame at a time.
//!
//! [`Session`] owns the [`Runtime`] and the frame's own state — the dismiss
//! latch and nothing else — and its whole input surface is one joypad byte per
//! frame: [`Session::frame`]. Everything the field, a scene and the message box
//! do to a frame is decided here, in the cartridge's order:
//!
//! ```text
//! 1. the dialogue window's input half   the pad's presses, the answer, the close
//! 2. a pending `$F6`                    the scene the message fired
//! 3. the field, or the scene            starved of input while a window is up,
//!                                       or while the story owns the party
//! ```
//!
//! The shell that draws the game sends the pad and presents what comes back: it
//! decides nothing about game state. The modes that are still the shell's —
//! title, game over, battle, shop and camp — are checked in front of this frame
//! and named in `psiv-godot/src/lib.rs`; `docs/campaign/CAMPAIGN_RUNNER.md`'s
//! nodes S3 to S5 move them in.
//!
//! # One frame, in two calls
//!
//! ```text
//! let frame = session.frame(pad);      // 1-3 above, and the frame's events
//! ...                                  // the shell applies `frame.events`
//! let signals = session.window_tick(); // the window's own half
//! ```
//!
//! The split is the cartridge's node order, not a convenience. The field node
//! ran before the window node, and the shell's own handling of a frame's events
//! sits between them because an event *opens* the window: an `Interact` the
//! shell routes to a talk, a scene's [`RuntimeEvent::SceneDialogue`], an empty
//! hand's "nothing here". The box advances its open animation on the frame it
//! opens, so running the window's half before the shell has applied the events
//! would move the box one frame later — a timing change the presentation
//! certification would see.
//!
//! # What is not here
//!
//! The press edge — this frame's buttons minus the previous frame's — is the
//! runtime's, not the session's. [`DialogueRunner`] latches the pad it is handed
//! (`dialogue/mod.rs`, `input`), and the field's confirm button is level-driven
//! with the press spent from `Field_Input_Buffer` when the party comes to rest
//! (`psiv-core/src/field.rs`, `FieldState::tick`). A second latch in here would
//! be a second answer to a question both of those already answer.
//!
//! [`DialogueRunner`]: crate::dialogue::DialogueRunner

use psiv_core::Input;

use crate::dialogue::DialogueSignal;
use crate::pad::Pad;
use crate::{Runtime, RuntimeEvent};

/// A `$F6` the dialogue fired, and whether its scene began.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneStart {
    /// The event id. A high bit marks a panel cutscene, which the shell runs a
    /// scene transition for (`event & 0x8000`).
    pub event: u16,
    /// Whether a transcribed scene was there to start. `false` leaves the field
    /// in control — the pack has no scene for that event, and the shell logs it.
    pub started: bool,
}

/// What one frame produced, in the order the frame produced it.
#[derive(Debug, Default)]
pub struct Frame {
    /// The field's or the scene's events, for the shell to apply: a talk to
    /// route, a map to reload, a scene panel, an encounter to fight.
    pub events: Vec<RuntimeEvent>,
    /// What the window's input half did — the answer the player gave, the page
    /// they closed, the `$F2` action a scene released.
    pub signals: Vec<DialogueSignal>,
    /// The `$F6` this frame handed to the scene runner, if the dialogue fired
    /// one.
    pub scene_started: Option<SceneStart>,
    /// The input the field was ticked with, for the walk-animation bridge.
    ///
    /// `None` on a frame a window owned: the field was starved with
    /// [`Input::Neutral`], and the shell must not read a step in progress as a
    /// walk of this frame's.
    pub field_input: Option<Input>,
}

/// The cartridge's main loop: the runtime, the dismiss latch, and one frame.
pub struct Session {
    runtime: Runtime,
    /// Set while a dialogue is open and until the confirm press is released
    /// after it closes.
    ///
    /// The press that dismissed a window must not immediately re-open the
    /// object: the engine's own press latch resets while the field is starved
    /// with `Neutral`, so the still-held key would read as a fresh press.
    accept_blocked: bool,
}

impl Session {
    /// A session over `runtime`, ready for its first frame.
    #[must_use]
    pub fn new(runtime: Runtime) -> Session {
        Session {
            runtime,
            accept_blocked: false,
        }
    }

    /// Advances one frame with this frame's joypad byte.
    ///
    /// The caller applies [`Frame::events`] — NPC routing, panels, transitions,
    /// battle entry — and then runs [`Session::window_tick`] for the window's
    /// own half of the same frame. See the module documentation for why those
    /// two are separate calls.
    pub fn frame(&mut self, pad: Pad) -> Frame {
        // The window's input half. The pad reaches the runtime every frame —
        // that latch is what makes a press read as fresh exactly once — and
        // with a window up the accept/choice press and the close are decided
        // here, before the field tick below.
        let window_was_open = self.runtime.dialogue_open();
        let signals = self.runtime.dialogue_frame(pad);
        // A `$F6` the dialogue fired becomes a running scene. The window half
        // of the frame it was fired in already shut the box, so the field sees
        // it on this frame and spends one tick entering the scene.
        let scene_started = self.runtime.take_dialogue_event().map(|event| SceneStart {
            event,
            started: self.runtime.start_event(event),
        });

        if window_was_open {
            // An open window owns the input: the runtime already took the
            // press, and the engine gets Neutral (the cartridge swaps
            // Game_Mode_Routine to FieldRoutine_Interaction; we model it by
            // starving the field of input, per the engine's documented
            // non-modal contract).
            self.accept_blocked = true;
            let events = self.runtime.tick(Input::Neutral);
            return Frame {
                events,
                signals,
                scene_started,
                field_input: None,
            };
        }

        // While a scene runs the field gets Neutral: the story owns the party.
        let mut field_input = if self.runtime.scene_active() {
            Input::Neutral
        } else {
            pad.field_input()
        };
        // The press that dismissed a window stays swallowed until released —
        // otherwise the engine (whose press latch reset during the Neutral
        // starvation) reads the still-held key as fresh and reopens the NPC.
        if self.accept_blocked {
            if matches!(field_input, Input::Action) {
                field_input = Input::Neutral;
            } else {
                self.accept_blocked = false;
            }
        }
        self.runtime.set_field_suspended(false);
        let events = self.runtime.tick(field_input);
        Frame {
            events,
            signals,
            scene_started,
            field_input: Some(field_input),
        }
    }

    /// The window's own half of a frame: the retail `$F2` actions released at
    /// their byte positions, the open animation, the typewriter and the flow's
    /// signals.
    ///
    /// Run it after [`Frame::events`] have been applied, and present the
    /// [`DialogueView`](crate::DialogueView) it leaves behind: the runtime owns
    /// the window's state, the shell owns its pixels.
    pub fn window_tick(&mut self) -> Vec<DialogueSignal> {
        self.runtime.dialogue_tick()
    }

    /// Blocks the confirm press until the pad releases it.
    ///
    /// The shell's own modes hand the field back this way: the press that
    /// closed a camp menu or left the game-over fade must not read as a talk on
    /// the next field frame.
    pub fn block_accept(&mut self) {
        self.accept_blocked = true;
    }

    /// The runtime, for presentation views.
    ///
    /// This is the read-only half of the session: what to draw, what to play,
    /// where the party is. A frame of game goes through [`Session::frame`].
    #[must_use]
    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    /// The runtime, mutably, for the modes that are still the shell's — title,
    /// game over, battle, shop and camp. Each of them drives its own runtime
    /// calls today (a menu's tick with `Neutral`, a battle round, a slot load),
    /// and the S3 to S5 nodes move them into this session, at which point this
    /// accessor and its callers go away.
    pub fn runtime_mut(&mut self) -> &mut Runtime {
        &mut self.runtime
    }
}
