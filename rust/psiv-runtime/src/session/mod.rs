//! The session: the cartridge's main loop, one 60 Hz frame at a time.
//!
//! [`Session`] owns the [`Runtime`] and the frame's own state — the dismiss
//! latch, the previous pad and the menu modes — and its whole input surface is
//! one joypad byte per frame: [`Session::frame`]. Everything the field, a scene,
//! the message box, a shop and the camp do to a frame is decided here, in the
//! cartridge's order. A frame belongs to one mode:
//!
//! ```text
//! shop or inn window   the counter's pages own the pad            (shop.rs)
//! camp menu, a chest   the camp's pages own the pad               (camp/)
//! a battle             the menu, the beats, the epilogue          (battle/)
//! the field            1. the dialogue window's input half
//!                      2. a pending `$F6`
//!                      3. the field, or the scene — starved of input while a
//!                         window is up, or while the story owns the party
//!                      4. the events that open windows: a talk, a counter, a
//!                         scene's line, a choice, "nothing here"
//!                      5. the window's own half
//! ```
//!
//! A window that is up owns the frame before either menu may open, and a chest
//! the field opened pre-empts the camp, as the shell's old dispatcher ordered
//! them. The shell that draws the game sends the pad and presents what comes
//! back: it decides nothing about game state. The modes that are still the
//! shell's — title and game over — are checked in front of this frame and named
//! in `psiv-godot/src/lib.rs`; `docs/campaign/CAMPAIGN_RUNNER.md`'s node S5
//! moves them in.
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
//! # One frame, one call
//!
//! The window's own half runs last because the cartridge's node order has it
//! so: the field node ran before the window node, and an event *opens* the
//! window — an `Interact` that routes to a talk, a scene's
//! [`RuntimeEvent::SceneDialogue`], an empty hand's "nothing here". The box
//! advances its open animation on the frame it opens, so running the window's
//! half before those events were applied would move the box one frame later — a
//! timing change the presentation certification would see. The session applies
//! the opening events itself (`route.rs`) for that reason, and reports
//! what it opened in [`Frame::routed`] for the shell's diagnostics.
//!
//! # What is not here
//!
//! The press edge — this frame's buttons minus the previous frame's — is the
//! menus', read from `prev_pad`. For the dialogue and the field it is the
//! runtime's, not the session's: [`DialogueRunner`] latches the pad it is
//! handed (`dialogue/mod.rs`, `input`), and the field's confirm button is
//! level-driven with the press spent from `Field_Input_Buffer` when the party
//! comes to rest (`psiv-core/src/field.rs`, `FieldState::tick`). A second latch
//! for those two would be a second answer to a question both already answer.
//!
//! [`DialogueRunner`]: crate::dialogue::DialogueRunner

mod battle;
mod camp;
mod route;
mod shop;

pub use battle::{
    BATTLE_DWELL_FRAMES, BattleBeat, BattleFrame, BattleStart, BattleView, BeatView,
    CommandMenuView, DamageView, EnemyStatus, MenuPage, MenuRow, MenuView, MessageKind,
    PartyStatus, SkillEntry, SkillSlotView, TargetKind, TechniqueEntry, battle_dwell_frames,
};

use psiv_core::{Cell, Input};

use crate::dialogue::DialogueSignal;
use crate::events::BattleTimeline;
use crate::pad::{Button, Pad};
use crate::{NpcDialogueOpen, Runtime, RuntimeEvent, SceneDialogueOpen};

pub use camp::{CampPage, CampView, OrderDraft, ROOT_OPTIONS};
pub use shop::{ShopCounterView, ShopOwnedItem, ShopPage, ShopStock, ShopView};

use battle::BattleMode;
use camp::MenuInput;

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

/// Which mode owned a frame.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum FrameMode {
    /// The field, a scene and the message box.
    #[default]
    Field,
    /// The shop or inn window, including the frame that closed it.
    Shop,
    /// The camp menu or a chest window, including the frame that closed it.
    Camp,
    /// The battle loop, including the frame it began and the frame it closed.
    Battle,
}

/// Which loop owns the frame.
enum Mode {
    /// The field, its scenes, the message box and the menus over them.
    Field,
    /// The battle loop, with its own menu, beats and epilogue.
    Battle(Box<BattleMode>),
}

/// A window the session opened this frame, for the shell's diagnostics and the
/// sprite sequence a talk turns an object to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Routed {
    /// A talk opened `npc_index`'s dialogue (or found none to open).
    Talk {
        /// The object talked to.
        npc_index: usize,
        /// The probe cell that hit it.
        cell: Cell,
        /// What opening the window did.
        outcome: NpcDialogueOpen,
    },
    /// A talk across a counter found no shop row at the object, and fell back
    /// to dialogue (a [`Routed::Talk`] follows).
    CounterWithoutShop {
        /// The probe cell that hit the object.
        cell: Cell,
    },
    /// A counter opened the shop or inn window.
    ShopOpened {
        /// The counter row's id.
        counter: usize,
    },
    /// An empty hand: the leader's "nothing here" line.
    NothingHere,
    /// A scene's line opened.
    SceneDialogue {
        /// The entry.
        entry: u16,
        /// What opening it did.
        outcome: SceneDialogueOpen,
    },
    /// The debug harness's auto-close skipped a scene's line without opening
    /// the window.
    SceneDialogueSkipped {
        /// The entry.
        entry: u16,
    },
    /// A scene reopened its saved text cursor.
    SceneDialogueResume {
        /// Whether a window opened (otherwise the scene was released).
        opened: bool,
    },
    /// The debug harness's auto-close skipped a resumed line.
    SceneDialogueResumeSkipped,
    /// A scene's yes/no branch opened with no text in front of it.
    SceneChoice,
}

/// What one frame produced, in the order the frame produced it.
#[derive(Debug, Default)]
pub struct Frame {
    /// Which mode owned the frame.
    pub mode: FrameMode,
    /// The field's or the scene's events, for the shell to apply: a map to
    /// reload, a scene panel, an encounter to fight. The events the session
    /// routes itself are not here; see [`Frame::routed`].
    pub events: Vec<RuntimeEvent>,
    /// The windows the session opened from the frame's events.
    pub routed: Vec<Routed>,
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
    /// What the window's own half did, after the events were applied.
    pub window_signals: Vec<DialogueSignal>,
    /// The events a camp command produced — a teleport's map change, a new
    /// party order — which the shell applies before the camp's [`Frame::sound`]
    /// and the tick's own events.
    pub menu_events: Vec<RuntimeEvent>,
    /// The sound effect a menu asked for this frame.
    pub sound: Option<u8>,
    /// The save slot the camp's SAVE picked this frame. Writing a file is the
    /// shell's (it owns the save directory and its policy); it answers with
    /// [`Session::finish_camp_save`], which sets the result line the page shows.
    pub save_request: Option<usize>,
    /// The battle's own frame while a battle owns the session, including the
    /// frame one began on.
    pub battle: Option<BattleFrame>,
}

/// The cartridge's main loop: the runtime, the latches, the modes and one frame.
pub struct Session {
    runtime: Runtime,
    /// Set while a dialogue is open and until the confirm press is released
    /// after it closes.
    ///
    /// The press that dismissed a window must not immediately re-open the
    /// object: the engine's own press latch resets while the field is starved
    /// with `Neutral`, so the still-held key would read as a fresh press.
    accept_blocked: bool,
    /// Last frame's pad: the menus' presses are the edges against it.
    prev_pad: Pad,
    shop: Option<ShopView>,
    camp: Option<CampView>,
    /// The debug harness's switch: scene lines are acknowledged unseen.
    scene_dialogue_autoclose: bool,
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
            prev_pad: Pad::NEUTRAL,
            shop: None,
            camp: None,
            scene_dialogue_autoclose: false,
            mode: Mode::Field,
        }
    }

    /// Answers a [`Frame::save_request`]: the camp's SAVE page shows the
    /// result line.
    pub fn finish_camp_save(&mut self, result: Result<(), String>) {
        if let Some(camp) = self.camp.as_mut() {
            camp.finish_save(result);
        }
    }

    /// The debug harness's switch: with it on, a scene's dialogue lines are
    /// acknowledged without opening a window.
    pub fn set_scene_dialogue_autoclose(&mut self, on: bool) {
        self.scene_dialogue_autoclose = on;
    }

    /// Advances one frame with this frame's joypad byte.
    pub fn frame(&mut self, pad: Pad) -> Frame {
        let pressed = pad.pressed(self.prev_pad);
        self.prev_pad = pad;
        if let Mode::Battle(_) = self.mode {
            return self.battle_frame(pad);
        }
        // A window that is already up owns the frame, so neither menu runs
        // under one — the old dispatcher reached them only after the window
        // branch had returned.
        if !self.runtime.dialogue_open() {
            if self.shop.is_some() {
                return self.shop_frame(pad, pressed);
            }
            if let Some(frame) = self.camp_frame(pressed) {
                return frame;
            }
        }
        self.field_frame(pad)
    }

    /// One frame of the battle loop.
    fn battle_frame(&mut self, pad: Pad) -> Frame {
        let Mode::Battle(mut mode) = std::mem::replace(&mut self.mode, Mode::Field) else {
            unreachable!("battle_frame runs only in battle mode");
        };
        let battle = mode.frame(&mut self.runtime, pad);
        let close_ready = battle.view.as_ref().is_some_and(|view| view.close_ready);
        if !close_ready {
            self.mode = Mode::Battle(mode);
            return Frame {
                mode: FrameMode::Battle,
                battle: Some(battle),
                ..Frame::default()
            };
        }
        // The battle is over: the field takes the next frame back. A finished
        // battle returns through the map refresh the cartridge runs before
        // revealing the field; a defeat leaves it pending, because the
        // game-over fade owns every frame from here.
        let events = if self.runtime.game_over() {
            Vec::new()
        } else {
            self.runtime.return_to_field()
        };
        let (events, routed) = self.route(events);
        Frame {
            mode: FrameMode::Battle,
            events,
            routed,
            battle: Some(battle),
            ..Frame::default()
        }
    }

    /// The field, a scene and the message box.
    fn field_frame(&mut self, pad: Pad) -> Frame {
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

        let field_input = if window_was_open {
            // An open window owns the input: the runtime already took the
            // press, and the engine gets Neutral (the cartridge swaps
            // Game_Mode_Routine to FieldRoutine_Interaction; we model it by
            // starving the field of input, per the engine's documented
            // non-modal contract).
            self.accept_blocked = true;
            Input::Neutral
        } else {
            // While a scene runs the field gets Neutral: the story owns the
            // party.
            let mut input = if self.runtime.scene_active() {
                Input::Neutral
            } else {
                pad.field_input()
            };
            // The press that dismissed a window stays swallowed until released
            // — otherwise the engine (whose press latch reset during the
            // Neutral starvation) reads the still-held key as fresh and
            // reopens the NPC.
            if self.accept_blocked {
                if matches!(input, Input::Action) {
                    input = Input::Neutral;
                } else {
                    self.accept_blocked = false;
                }
            }
            self.runtime.set_field_suspended(false);
            input
        };
        let events = self.runtime.tick(field_input);
        let (events, routed) = self.route(events);
        // An encounter or a scene battle this frame's events asked for starts
        // inside the frame; the battle loop owns the frames after it.
        let battle = self.begin_battle(&events);
        // The window's own half of the same frame, after the events above may
        // have opened it.
        let window_signals = self.runtime.dialogue_tick();
        Frame {
            mode: if battle.is_some() {
                FrameMode::Battle
            } else {
                FrameMode::Field
            },
            events,
            routed,
            signals,
            scene_started,
            field_input: (!window_was_open).then_some(field_input),
            window_signals,
            battle,
            ..Frame::default()
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

    /// One frame of the shop or inn window.
    fn shop_frame(&mut self, pad: Pad, pressed: Pad) -> Frame {
        let shop = self.shop.as_mut().expect("a shop frame has a shop");
        if shop.frame(&mut self.runtime, pad, pressed) == shop::ShopOutcome::Closed {
            self.shop = None;
            self.runtime.set_field_suspended(false);
            return Frame {
                mode: FrameMode::Shop,
                ..Frame::default()
            };
        }
        self.runtime.set_field_suspended(true);
        let events = self.runtime.tick(Input::Neutral);
        let (events, routed) = self.route(events);
        Frame {
            mode: FrameMode::Shop,
            events,
            routed,
            ..Frame::default()
        }
    }

    /// One frame of the camp menu or a chest window; `None` when neither is up
    /// or opens, and the field has the frame.
    fn camp_frame(&mut self, pressed: Pad) -> Option<Frame> {
        match self.camp.as_mut() {
            None => {
                self.camp = Some(if self.runtime.loot_state().is_some() {
                    CampView::open_loot(&self.runtime)
                } else if self.runtime.scene_active() || !pressed.held(Button::Camp) {
                    return None;
                } else {
                    CampView::open(&self.runtime)
                });
            }
            Some(camp) => {
                camp.frame(&mut self.runtime, MenuInput::of(pressed));
            }
        }
        let camp = self.camp.as_mut().expect("the camp is up");
        let save_request = camp.take_save_request();
        let drained = camp.finish_frame();
        if drained.closed {
            self.camp = None;
            self.runtime.set_field_suspended(false);
            // The press that closed the menu must not read as a talk.
            self.accept_blocked = true;
            return Some(Frame {
                mode: FrameMode::Camp,
                menu_events: drained.events,
                sound: drained.sound,
                save_request,
                ..Frame::default()
            });
        }
        self.runtime.set_field_suspended(true);
        let events = self.runtime.tick(Input::Neutral);
        let (events, routed) = self.route(events);
        if let Some(camp) = self.camp.as_mut() {
            camp.sync(&self.runtime);
        }
        Some(Frame {
            mode: FrameMode::Camp,
            events,
            routed,
            menu_events: drained.events,
            sound: drained.sound,
            save_request,
            ..Frame::default()
        })
    }

    /// Opens the camp menu as the debug hook does, with no button. Refused
    /// while a scene runs, as the Camp button is.
    pub fn open_camp_menu(&mut self) -> bool {
        if self.runtime.scene_active() {
            return false;
        }
        self.camp = Some(CampView::open(&self.runtime));
        true
    }

    /// Opens the shop or inn at counter row `index`, as the debug selector
    /// does, with no talk.
    pub fn open_shop_counter(&mut self, index: usize) -> bool {
        let Some(shops) = self.runtime.data().shops() else {
            return false;
        };
        let Some(counter) = shops.counter_index(index) else {
            return false;
        };
        self.shop = Some(ShopView::open(counter, shops, &self.runtime));
        true
    }

    /// The shop or inn window, while it is up.
    #[must_use]
    pub fn shop_view(&self) -> Option<&ShopView> {
        self.shop.as_ref()
    }

    /// The camp menu or chest window, while it is up.
    #[must_use]
    pub fn camp_view(&self) -> Option<&CampView> {
        self.camp.as_ref()
    }

    /// Blocks the confirm press until the pad releases it.
    ///
    /// The shell's own modes hand the field back this way: the press that left
    /// the game-over fade must not read as a talk on the next field frame.
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

    /// The runtime, mutably, for the modes that are still the shell's — title
    /// and game over. Each drives its own runtime calls today (a slot load, the
    /// fade's reset), and node S5 moves them into this session, at which point
    /// this accessor and its callers go away.
    pub fn runtime_mut(&mut self) -> &mut Runtime {
        &mut self.runtime
    }
}
