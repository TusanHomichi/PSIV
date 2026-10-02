//! The session: the cartridge's main loop, one 60 Hz frame at a time.
//!
//! [`Session`] owns the [`Runtime`] and the frame's own state — the dismiss
//! latch, the mode, and the battle loop — and its whole input surface is one
//! joypad byte per frame: [`Session::frame`]. Everything the field, a scene,
//! the message box and a battle do to a frame is decided here, in the
//! cartridge's order:
//!
//! ```text
//! 1. the dialogue window's input half   the pad's presses, the answer, the close
//! 2. a pending `$F6`                    the scene the message fired
//! 3. the field, or the scene            starved of input while a window is up,
//!                                       or while the story owns the party
//! 4. the battle                         once an encounter or a scene starts one:
//!                                       the menu, the beats, the epilogue
//! ```
//!
//! The shell that draws the game sends the pad and presents what comes back: it
//! decides nothing about game state. The modes that are still the shell's —
//! title, game over, shop and camp — are checked in front of this frame and
//! named in `psiv-godot/src/lib.rs`; `docs/campaign/CAMPAIGN_RUNNER.md`'s nodes
//! S4 and S5 move them in.
//!
//! # One frame, in two calls
//!
//! ```text
//! let frame = session.frame(pad);      // 1-4 above, and the frame's events
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
//! # The battle
//!
//! A battle is a mode, not a script the shell plays: `EncounterRolled` or a
//! scene's `SceneBattleStarted` starts the runtime's own battle inside this
//! frame, and from the next frame on [`Session::frame`] runs the battle loop
//! (`session/battle/`) instead of the field until the presentation closes.
//! [`Frame::battle`] carries that loop's view, its retail sound cues, and the
//! two edges the shell needs: the battle that began, and the battle that is
//! over.
//!
//! # What is not here
//!
//! The press edge — this frame's buttons minus the previous frame's — is the
//! runtime's, not the session's. [`DialogueRunner`] latches the pad it is handed
//! (`dialogue/mod.rs`, `input`), the field's confirm button is level-driven
//! with the press spent from `Field_Input_Buffer` when the party comes to rest
//! (`psiv-core/src/field.rs`, `FieldState::tick`), and the battle menu and its
//! beats latch their own (`session/battle/`). A second latch in here would be a
//! second answer to a question all three already answer.
//!
//! [`DialogueRunner`]: crate::dialogue::DialogueRunner

mod battle;

pub use battle::{
    BATTLE_DWELL_FRAMES, BattleBeat, BattleFrame, BattleStart, BattleView, BeatView,
    CommandMenuView, DamageView, EnemyStatus, MenuPage, MenuRow, MenuView, MessageKind,
    PartyStatus, SkillEntry, SkillSlotView, TargetKind, TechniqueEntry, battle_dwell_frames,
};

use psiv_core::Input;

use crate::Runtime;
use crate::dialogue::DialogueSignal;
use crate::events::{BattleTimeline, RuntimeEvent};
use crate::pad::Pad;
use battle::BattleMode;

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
    /// route, a map to reload, a scene panel, an encounter to fight, and the
    /// map refresh a finished battle returns through.
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
    /// walk of this frame's. A battle frame has none either — the field did not
    /// run at all.
    pub field_input: Option<Input>,
    /// The battle's own frame while a battle owns the session, including the
    /// frame one began on.
    pub battle: Option<BattleFrame>,
}

/// Which loop owns the frame.
enum Mode {
    /// The field, its scenes and the message box.
    Field,
    /// The battle loop, with its own menu, beats and epilogue.
    Battle(Box<BattleMode>),
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
    /// Which loop owns the frame. A battle is entered by the frame that starts
    /// it and left by the frame its presentation closes on.
    mode: Mode,
}

impl Session {
    /// A session over `runtime`, ready for its first frame.
    #[must_use]
    pub fn new(runtime: Runtime) -> Session {
        Session {
            runtime,
            accept_blocked: false,
            mode: Mode::Field,
        }
    }

    /// Advances one frame with this frame's joypad byte.
    ///
    /// The caller applies [`Frame::events`] — NPC routing, panels, transitions,
    /// battle entry — and then runs [`Session::window_tick`] for the window's
    /// own half of the same frame. See the module documentation for why those
    /// two are separate calls.
    pub fn frame(&mut self, pad: Pad) -> Frame {
        match std::mem::replace(&mut self.mode, Mode::Field) {
            Mode::Battle(mut mode) => {
                let battle = mode.frame(&mut self.runtime, pad);
                let close_ready = battle.view.as_ref().is_some_and(|view| view.close_ready);
                if close_ready {
                    // The battle is over: the field takes the next frame back.
                    // A finished battle returns through the map refresh the
                    // cartridge runs before revealing the field; a defeat
                    // leaves it pending exactly as the shell's own close did,
                    // because the game-over fade owns every frame from here.
                    let events = if self.runtime.game_over() {
                        Vec::new()
                    } else {
                        self.runtime.return_to_field()
                    };
                    return Frame {
                        events,
                        battle: Some(battle),
                        ..Frame::default()
                    };
                }
                self.mode = Mode::Battle(mode);
                Frame {
                    battle: Some(battle),
                    ..Frame::default()
                }
            }
            Mode::Field => {
                // Not `self.mode = Mode::Field` again: `field_frame` is where a
                // battle starts, and it owns the switch.
                self.field_frame(pad)
            }
        }
    }

    /// The field's frame: the window's input half, a pending `$F6`, the field
    /// or the scene, and the battle a frame's events may have started.
    fn field_frame(&mut self, pad: Pad) -> Frame {
        let window_was_open = self.runtime.dialogue_open();
        let signals = self.runtime.dialogue_frame(pad);
        // A `$F6` the dialogue fired becomes a running scene. The window half
        // of the frame it was fired in already shut the box, so the field sees
        // it on this frame and spends one tick entering the scene.
        let scene_started = self.runtime.take_dialogue_event().map(|event| SceneStart {
            event,
            started: self.runtime.start_event(event),
        });

        let (events, field_input) = if window_was_open {
            // An open window owns the input: the runtime already took the
            // press, and the engine gets Neutral (the cartridge swaps
            // Game_Mode_Routine to FieldRoutine_Interaction; we model it by
            // starving the field of input, per the engine's documented
            // non-modal contract).
            self.accept_blocked = true;
            (self.runtime.tick(Input::Neutral), None)
        } else {
            // While a scene runs the field gets Neutral: the story owns the
            // party.
            let mut field_input = if self.runtime.scene_active() {
                Input::Neutral
            } else {
                pad.field_input()
            };
            // The press that dismissed a window stays swallowed until released
            // — otherwise the engine (whose press latch reset during the
            // Neutral starvation) reads the still-held key as fresh and
            // reopens the NPC.
            if self.accept_blocked {
                if matches!(field_input, Input::Action) {
                    field_input = Input::Neutral;
                } else {
                    self.accept_blocked = false;
                }
            }
            self.runtime.set_field_suspended(false);
            (self.runtime.tick(field_input), Some(field_input))
        };
        let battle = self.begin_battle(&events);
        Frame {
            events,
            signals,
            scene_started,
            field_input,
            battle,
        }
    }

    /// Starts the battle this frame's events asked for, if they asked for one.
    ///
    /// An `EncounterRolled` is the runtime's own battle to start; a
    /// `SceneBattleStarted` is one the scene runner already started, and the
    /// event carries its opening timeline. A battle that cannot start leaves
    /// the field in control and reports the reason as a fault for the shell to
    /// log — the same "no battle this frame" the shell's own error paths
    /// produced.
    fn begin_battle(&mut self, events: &[RuntimeEvent]) -> Option<BattleFrame> {
        let start = events.iter().find_map(|event| match event {
            RuntimeEvent::EncounterRolled { formation } => Some(BattleStart::Encounter(*formation)),
            RuntimeEvent::SceneBattleStarted { index, .. } => {
                Some(BattleStart::EventBattle(*index))
            }
            _ => None,
        })?;
        let timeline = match start {
            BattleStart::Encounter(formation) => match self.start_encounter(formation) {
                Ok(timeline) => timeline,
                Err(fault) => {
                    return Some(BattleFrame {
                        view: None,
                        started: None,
                        fault: Some(fault),
                    });
                }
            },
            BattleStart::EventBattle(_) => {
                let Some(RuntimeEvent::SceneBattleStarted {
                    events,
                    sounds,
                    animations,
                    ..
                }) = events
                    .iter()
                    .find(|event| matches!(event, RuntimeEvent::SceneBattleStarted { .. }))
                else {
                    return None;
                };
                BattleTimeline {
                    events: events.clone(),
                    sounds: sounds.clone(),
                    animations: animations.clone(),
                }
            }
        };
        let mut mode = BattleMode::begin(&self.runtime, timeline, start);
        let frame = mode.start_frame(&self.runtime);
        self.mode = Mode::Battle(Box::new(mode));
        Some(frame)
    }

    /// Starts an encounter battle, or says why it cannot.
    ///
    /// The empty-party refusal is the shell's own (`encounter rolled formation
    /// {formation:#05x} with an empty party`): a battle with nobody in it can
    /// never end, so it must not start.
    fn start_encounter(&mut self, formation: u16) -> Result<BattleTimeline, String> {
        let party = self.runtime.battle_party();
        if party.is_empty() {
            return Err(format!(
                "encounter rolled formation {formation:#05x} with an empty party"
            ));
        }
        self.runtime
            .start_battle_timeline(formation, party)
            .map_err(|error| format!("could not start battle {formation:#05x}: {error}"))
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

    /// Whether the battle loop owns this session's frames.
    #[must_use]
    pub fn battle_active(&self) -> bool {
        matches!(self.mode, Mode::Battle(_))
    }

    /// Ends the current battle as escaped and gives the field the next frame.
    ///
    /// The presentation's own failure seam: a shell that cannot build a battle's
    /// art must not leave the fight running unseen, and the shell's error paths
    /// answered exactly this way (`finish_battle_for_outcome(Escaped, 0)`).
    /// Everything else about a battle's end is the battle mode's own decision.
    pub fn abort_battle(&mut self) {
        if self.battle_active() {
            let _ = self
                .runtime
                .finish_battle_for_outcome(psiv_core::battle::Outcome::Escaped, 0);
            self.mode = Mode::Field;
        }
    }

    /// Debug-selector family: starts `formation` as an ordinary encounter
    /// battle from outside the field frame, which is what
    /// `PSIV_DEBUG_BATTLE=<formation>` and `PSIV_DEBUG_VEHICLE_BATTLE` do.
    ///
    /// A real battle starts, so the capture that follows plays the same rounds
    /// a player would; nothing about it is a fixture.
    pub fn debug_battle(&mut self, formation: u16) -> BattleFrame {
        match self.start_encounter(formation) {
            Ok(timeline) => {
                let mut mode =
                    BattleMode::begin(&self.runtime, timeline, BattleStart::Encounter(formation));
                let frame = mode.start_frame(&self.runtime);
                self.mode = Mode::Battle(Box::new(mode));
                frame
            }
            Err(error) => BattleFrame {
                view: None,
                started: None,
                fault: Some(format!("debug battle {formation:#05x} refused: {error}")),
            },
        }
    }

    /// Debug-selector family: the newly-exact probe of `PSIV_DEBUG_BATTLE=0x89`.
    ///
    /// A real `formation` battle starts and its opening timeline is replaced by
    /// `timeline`, so a decoded enemy attack can be watched on the live screen
    /// without a command menu. The battle is real, which means the shared RNG
    /// advances here where the old fixture's did not; no certified capture
    /// covers this selector.
    pub fn debug_battle_probe(&mut self, formation: u16, timeline: BattleTimeline) -> BattleFrame {
        let party = self.runtime.battle_party();
        if party.is_empty() {
            return BattleFrame {
                view: None,
                started: None,
                fault: Some(format!(
                    "debug probe {formation:#05x} refused: the battle party is empty"
                )),
            };
        }
        match self.runtime.start_battle(formation, party) {
            Ok(_) => {
                let mut mode =
                    BattleMode::begin(&self.runtime, timeline, BattleStart::Encounter(formation));
                let frame = mode.start_frame(&self.runtime);
                self.mode = Mode::Battle(Box::new(mode));
                frame
            }
            Err(error) => BattleFrame {
                view: None,
                started: None,
                fault: Some(format!("debug probe {formation:#05x} refused: {error}")),
            },
        }
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
    /// game over, shop and camp. Each of them drives its own runtime calls
    /// today (a menu's tick with `Neutral`, a slot load), and the S4 and S5
    /// nodes move them into this session, at which point this accessor and its
    /// callers go away.
    pub fn runtime_mut(&mut self) -> &mut Runtime {
        &mut self.runtime
    }
}
