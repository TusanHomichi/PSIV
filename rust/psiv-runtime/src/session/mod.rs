//! The session: the cartridge's main loop, one 60 Hz frame at a time.
//!
//! [`Session`] owns the [`Runtime`] and the frame's own state — the dismiss
//! latch, the previous pad, the save store and the modes — and its whole input
//! surface is one joypad byte per frame: [`Session::frame`]. Everything the
//! field, a scene, the message box, a shop, the camp, the title and a defeat
//! do to a frame is decided here, in the cartridge's order. A frame belongs to
//! one mode:
//!
//! ```text
//! the title            the front door's phases and its rows       (title.rs)
//! the defeat fade      fourteen frames that gate the title's return
//! shop or inn window   the counter's pages own the pad            (shop.rs)
//! camp menu, a chest   the camp's pages own the pad               (camp/)
//! a battle             the menu, the beats, the epilogue          (battle/)
//! the field            1. the field-status windows a defeat queues
//!                      2. the dialogue window's input half
//!                      3. a pending `$F6`
//!                      4. the field, or the scene — starved of input while a
//!                         window is up, or while the story owns the party
//!                      5. the events that open windows: a talk, a counter, a
//!                         scene's line, a choice, "nothing here"
//!                      6. the window's own half
//! ```
//!
//! A window that is up owns the frame before either menu may open, and a chest
//! the field opened pre-empts the camp, as the shell's old dispatcher ordered
//! them. The shell that draws the game sends the pad and presents what comes
//! back: it decides nothing about game state, and it has no way to reach one —
//! [`Session::runtime`] hands out a `&Runtime` and the runtime's mutators are
//! crate-private.
//!
//! # The front door
//!
//! Retail powers on into the title (`MainGameProgram`, `ps4.asm:86190`), so a
//! session that starts at the front door is put there by the shell
//! ([`Session::start_title`]), and START, CONTINUE and ERASE DATA complete
//! here: the title's own module owns the rows, the slot rules and the erase
//! confirmation, and the session builds the runtime each choice asks for
//! through [`Session::new_game`] and [`Session::continue_save`]. A defeat ends
//! the same way: [`Runtime::game_over`] hands the session to the fade and the
//! fade hands it back to the title.
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
//! what it opened in [`Frame::routed`] for the shell's diagnostics. The
//! field-status windows a defeat queues open at the *top* of the frame for the
//! same reason: the shell's old dispatcher serviced them before everything
//! else, and the box's first input half lands on the frame it opened.
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
//! # The boundary
//!
//! `psiv-godot` presents and sends input; it decides nothing about game state.
//! Every path a shell once had into the runtime's mutators goes through this
//! session or is gone: the session's own save store replaces the shell's slot
//! writes, the debug fixtures build their sessions here (`session/debug.rs`),
//! and the runtime's mutators are crate-private. A shell that tries to mutate
//! a frame of game fails to build:
//!
//! ```compile_fail
//! # fn shell(session: &mut psiv_runtime::Session) {
//! session.runtime().tick(psiv_core::Input::Neutral);
//! # }
//! ```
//!
//! The same reference cannot reach any other state-changing method either:
//!
//! ```compile_fail
//! # fn shell(session: &mut psiv_runtime::Session) {
//! session.runtime().close_dialogue();
//! # }
//! ```
//!
//! [`DialogueRunner`]: crate::dialogue::DialogueRunner

mod battle;
mod camp;
mod debug;
#[cfg(test)]
mod debug_tests;
mod game_over;
mod notices;
mod route;
mod saves;
mod shop;
mod title;

pub use battle::{
    BATTLE_DWELL_FRAMES, BattleBeat, BattleFrame, BattleStart, BattleView, BeatView,
    CommandMenuView, DamageView, EnemyStatus, MenuPage, MenuRow, MenuView, MessageKind,
    PartyStatus, SkillEntry, SkillSlotView, TargetKind, TechniqueEntry, battle_dwell_frames,
};
pub use debug::{camp_fixture, scene_fixture};
pub use game_over::{GAME_OVER_FADE_FRAMES, GameOverFrame};
pub use notices::FieldNoticeOpened;
pub use title::{
    TitleEntry, TitleErase, TitleFailure, TitleFrame, TitlePhase, TitleView, TitleWindow,
};

use psiv_core::{Cell, Input};
use psiv_data::BattleFiles;

use crate::dialogue::DialogueSignal;
use crate::pad::Pad;
use crate::save::SaveStore;
use crate::{NpcDialogueOpen, Runtime, RuntimeEvent, SceneDialogueOpen};

pub use camp::{CampPage, CampView, OrderDraft, ROOT_OPTIONS};
pub use shop::{ShopCounterView, ShopOwnedItem, ShopPage, ShopStock, ShopView};

use battle::BattleMode;
use game_over::GameOverMode;
use title::TitleMode;

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
    /// The title, including the frame a defeat's fade restored it on.
    Title,
    /// The defeat fade, before the title comes back.
    GameOver,
}

/// Which loop owns the frame.
enum Mode {
    /// The field, its scenes, the message box and the menus over them.
    Field,
    /// The battle loop, with its own menu, beats and epilogue.
    Battle(Box<BattleMode>),
    /// Retail's front door: the phases, the option rows and the slot lists.
    Title(Box<TitleMode>),
    /// The defeat fade that gates the title's return.
    GameOver(Box<GameOverMode>),
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

/// The camp's SAVE failure, on the frame the write failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CampSaveFailure {
    /// The visible slot the SAVE page picked, zero-based.
    pub slot: usize,
    /// Why the write failed.
    pub error: String,
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
    /// The field-status window the session opened this frame, which the shell
    /// logs.
    pub notice: Option<FieldNoticeOpened>,
    /// The camp's SAVE when its write failed this frame. The session owns the
    /// store, so the write is no longer a request the shell answers; this is
    /// its only report, and it carries what the shell's log line said.
    pub camp_save_error: Option<CampSaveFailure>,
    /// The battle's own frame while a battle owns the session, including the
    /// frame one began on.
    pub battle: Option<BattleFrame>,
    /// The title's own frame while the title owns the session.
    pub title: Option<TitleFrame>,
    /// The defeat fade's frame, with the title on the frame it restores it.
    pub game_over: Option<GameOverFrame>,
}

/// The cartridge's main loop: the runtime, the latches, the modes and one frame.
pub struct Session {
    runtime: Runtime,
    /// The run's save store, when the shell resolved a directory for it. A
    /// session without one refuses every slot operation (`saves.rs`).
    save_store: Option<SaveStore>,
    /// The battle pack, once the shell enabled it: a START or CONTINUE that
    /// builds a fresh runtime enables it on the new one, so a session never
    /// silently loses battles across a front-door choice.
    battle_files: Option<BattleFiles>,
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
    /// The debug harness's switch: the title walks itself to START.
    title_autostart: bool,
    /// The field-status window that has been shown and is waiting to be read.
    notice_open: bool,
    /// Which loop owns the frame. A battle is entered by the frame that starts
    /// it and left by the frame its presentation closes on; the title is
    /// entered by the shell or by the fade that follows a defeat.
    mode: Mode,
}

impl Session {
    /// A session over `runtime`, ready for its first frame.
    #[must_use]
    pub fn new(runtime: Runtime) -> Session {
        Session::over(runtime, None)
    }

    /// A session whose save I/O goes through `store`: the run directory the
    /// shell resolved (`rust/psiv-godot/src/save_dir.rs`).
    ///
    /// The store is not a convenience: a session without one refuses CONTINUE,
    /// ERASE DATA and the camp's SAVE rather than falling back to a directory
    /// the run never named.
    #[must_use]
    pub fn with_saves(runtime: Runtime, store: SaveStore) -> Session {
        Session::over(runtime, Some(store))
    }

    fn over(runtime: Runtime, save_store: Option<SaveStore>) -> Session {
        Session {
            runtime,
            save_store,
            battle_files: None,
            accept_blocked: false,
            prev_pad: Pad::NEUTRAL,
            shop: None,
            camp: None,
            scene_dialogue_autoclose: false,
            title_autostart: false,
            notice_open: false,
            mode: Mode::Field,
        }
    }

    /// The debug harness's switch: with it on, a scene's dialogue lines are
    /// acknowledged without opening a window.
    pub fn set_scene_dialogue_autoclose(&mut self, on: bool) {
        self.scene_dialogue_autoclose = on;
    }

    /// The debug harness's switch: with it on, the title walks its phases with
    /// synthetic accepts and selects START, so the new-game handoff is
    /// testable without an input device (`PSIV_DEBUG_TITLE_AUTOSTART`).
    pub fn set_title_autostart(&mut self, on: bool) {
        self.title_autostart = on;
    }

    /// Advances one frame with this frame's joypad byte.
    pub fn frame(&mut self, pad: Pad) -> Frame {
        let pressed = pad.pressed(self.prev_pad);
        self.prev_pad = pad;
        // The title owns every frame it is up: the shell's own title driver
        // ran in front of the field-status windows and the battle stage too.
        if let Mode::Title(_) = self.mode {
            return self.title_frame(pad);
        }
        // The field-status windows a defeat queues are serviced before
        // everything else, the point the shell's own service ran at: the box
        // that opens here takes its first input half on this same frame.
        let notice = self.serve_field_notice();
        let mut frame = if let Mode::GameOver(_) = self.mode {
            self.game_over_frame()
        } else if let Mode::Battle(_) = self.mode {
            self.battle_frame(pad)
        } else if self.runtime.game_over() {
            // A defeat's boundary. The runtime cleared the scene and the
            // notices when it ended, so nothing else runs until the title is
            // back — not the field, not the shared seed.
            self.mode = Mode::GameOver(Box::new(GameOverMode::new()));
            self.game_over_frame()
        } else if !self.runtime.dialogue_open() && self.shop.is_some() {
            self.shop_frame(pad, pressed)
        } else if !self.runtime.dialogue_open()
            && let Some(frame) = self.camp_frame(pressed)
        {
            frame
        } else {
            self.field_frame(pad)
        };
        frame.notice = notice;
        frame
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
        // game-over fade owns every frame from here — it begins on this frame,
        // the frame the defeated message's stage closes on, exactly as the
        // shell's own battle driver began it.
        if self.runtime.game_over() {
            self.mode = Mode::GameOver(Box::new(GameOverMode::new()));
            return Frame {
                mode: FrameMode::Battle,
                battle: Some(battle),
                game_over: Some(self.game_over_frame_first()),
                ..Frame::default()
            };
        }
        let events = self.runtime.return_to_field();
        let (events, routed) = self.route(events);
        Frame {
            mode: FrameMode::Battle,
            events,
            routed,
            battle: Some(battle),
            ..Frame::default()
        }
    }

    /// The fade's first frame, taken without running the mode: the frame the
    /// battle's defeat ended on carries it beside the battle's own frame.
    fn game_over_frame_first(&mut self) -> GameOverFrame {
        match &mut self.mode {
            Mode::GameOver(mode) => mode.frame(),
            _ => unreachable!("the fade was just installed"),
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

    /// The camp menu or chest window, while it is up.
    #[must_use]
    pub fn camp_view(&self) -> Option<&CampView> {
        self.camp.as_ref()
    }

    /// Blocks the confirm press until the pad releases it.
    ///
    /// The session's own modes hand the field back this way: the press that
    /// closed a camp menu, left the game-over fade or opened the title must not
    /// read as a talk on the next field frame. A driver that answers a window
    /// outside the session's own flow — a replay harness handing the field back
    /// after a fixture — uses it the same way.
    pub fn block_accept(&mut self) {
        self.accept_blocked = true;
    }

    /// The runtime, for presentation views.
    ///
    /// This is the read-only half of the session: what to draw, what to play,
    /// where the party is. A frame of game goes through [`Session::frame`], and
    /// the runtime's mutators are crate-private, so a shell cannot reach one
    /// through this reference (see the module's boundary doctests).
    #[must_use]
    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }
}
