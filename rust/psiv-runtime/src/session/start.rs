//! Starting a session: every way a game legitimately begins.
//!
//! A [`Runtime`] is the engine's state; a [`Session`] is the game. The engine's
//! constructors are crate-private, so this module is the only way a caller
//! outside the crate turns a pack into a game, and every one of them hands back
//! a session whose whole input surface is [`Session::frame`]. That is the point
//! of the seam: a shell (or the campaign runner, or a test) may choose *where a
//! game starts*, and may not touch it afterwards.
//!
//! ```text
//! power_on()          retail's power-on: the pack's game start, at the title
//! field()             the same boot with no front door (the debug selectors)
//! new_game()          the title's START: the initializer, battles, the opening
//! continue_slot(n)    the title's CONTINUE: slot n through a SaveStore
//! from_slot_bytes(..) a slot file's bytes (the runner's --from-chapter)
//! from_save(..)       the same, decoded: the fixtures' and tests' hand-built
//!                     state
//! ```
//!
//! `psiv-runtime/src/session/debug.rs` holds the certification fixtures, which
//! are the other documented way a session starts: from a hand-built state with
//! the retail initializer's banks under it.
//!
//! [`Start`] carries the run's own configuration, and every constructor applies
//! all of it: the pack itself ([`GameData`], which holds the maps, the dialogue
//! and the sound), the battle files (without them no encounter rolls — a
//! pre-battle pack) and the run's [`SaveStore`], where saving applies. A
//! session built without a store refuses CONTINUE, ERASE DATA and the camp's
//! SAVE rather than guessing a directory (`saves.rs`). The two harness switches
//! ([`Start::with_title_autostart`], [`Start::with_scene_dialogue_autoclose`])
//! are part of that configuration rather than calls after construction, because
//! the title reads its switch as it installs: a `power_on` that started the
//! title first would ignore it.

use psiv_core::{Cell, Direction, RetailSave, StepFrames};
use psiv_data::{BattleFiles, GameData};

use crate::Runtime;
use crate::save::SaveStore;

use super::Session;

/// Where the field starts when the pack has no `game_start` record: a pack
/// that predates game-start extraction. The one placement the runtime ever
/// invents, and the shell's own fallback before this module owned it.
pub const FALLBACK_SPAWN: (u16, Cell, Direction) = (0x010, Cell { x: 31, y: 8 }, Direction::Up);

/// A session about to start, and what the run has to hand it.
///
/// Built by [`Session::start`]; each constructor below turns it into a playing
/// session. The setters are the run's own configuration, so a caller that has
/// no battle pack or no save directory simply does not set one.
pub struct Start {
    data: GameData,
    battles: Option<BattleFiles>,
    store: Option<SaveStore>,
    step_frames: StepFrames,
    scene_dialogue_autoclose: bool,
    title_autostart: bool,
}

/// A [`Start`] taken apart: the pack, the battle files, the field's timing and
/// the run's own configuration.
struct Parts {
    data: GameData,
    battles: Option<BattleFiles>,
    step_frames: StepFrames,
    run: RunConfig,
}

/// The run's configuration, after the pack has been taken out of [`Start`].
struct RunConfig {
    store: Option<SaveStore>,
    scene_dialogue_autoclose: bool,
    title_autostart: bool,
}

impl Session {
    /// What a session starts over: the loaded pack.
    ///
    /// The pack carries the dialogue set, the maps and the sound, so a session
    /// built from it can open a window the moment it exists.
    #[must_use]
    pub fn start(data: GameData) -> Start {
        Start {
            data,
            battles: None,
            store: None,
            step_frames: StepFrames::default(),
            scene_dialogue_autoclose: false,
            title_autostart: false,
        }
    }
}

impl Start {
    /// Arms encounters with the run's battle files.
    ///
    /// A session without them is a pre-battle pack: encounters never roll, and
    /// the camp's item and ability tables are empty. A START or CONTINUE
    /// completed later keeps them, because the session does.
    #[must_use]
    pub fn with_battles(mut self, files: BattleFiles) -> Start {
        self.battles = Some(files);
        self
    }

    /// The run's save directory, as a [`SaveStore`].
    #[must_use]
    pub fn with_saves(mut self, store: SaveStore) -> Start {
        self.store = Some(store);
        self
    }

    /// The field's step timing, for a harness that walks at a different pace
    /// than the cartridge's.
    #[must_use]
    pub fn with_step_frames(mut self, step_frames: StepFrames) -> Start {
        self.step_frames = step_frames;
        self
    }

    /// The switch [`Session::set_title_autostart`] carries: with it on, the
    /// title walks its own phases to START (`PSIV_DEBUG_TITLE_AUTOSTART`).
    ///
    /// It has to reach the session *before* [`Start::power_on`] installs the
    /// title, because the title reads it as it installs.
    #[must_use]
    pub fn with_title_autostart(mut self, on: bool) -> Start {
        self.title_autostart = on;
        self
    }

    /// The switch [`Session::set_scene_dialogue_autoclose`] carries: with it
    /// on, a scene's dialogue lines are acknowledged without opening a window
    /// (`PSIV_DEBUG_AUTOCLOSE_SCENE`).
    #[must_use]
    pub fn with_scene_dialogue_autoclose(mut self, on: bool) -> Start {
        self.scene_dialogue_autoclose = on;
        self
    }

    /// Retail's power-on (`MainGameProgram`, `ps4.asm:86190`): a game at the
    /// pack's own first control, with the title owning the frames from the
    /// first one.
    ///
    /// The runtime under the title is the pack's `game_start` placement; START
    /// and CONTINUE replace it inside the session, so a player who never
    /// presses anything never plays it. The title's slot rows and its autostart
    /// switch are read as it installs, which is why the run's store and
    /// switches are setters on [`Start`].
    ///
    /// # Errors
    ///
    /// Whatever [`Runtime::new`] rejects: a pack whose game-start map or facing
    /// does not convert.
    pub fn power_on(self) -> Result<Session, String> {
        let mut session = self.field()?;
        session.start_title();
        Ok(session)
    }

    /// The same boot without the front door: the field at the pack's own first
    /// control, ready for its first frame.
    ///
    /// This is what the shell's debug selectors ask for — a screenshot or a
    /// battle fixture must reach the surface it was written to inspect without
    /// paying the title's delay.
    ///
    /// # Errors
    ///
    /// Whatever [`Runtime::new`] rejects: a pack whose game-start map or facing
    /// does not convert.
    pub fn field(self) -> Result<Session, String> {
        let Parts {
            data,
            battles,
            step_frames,
            run,
        } = self.into_parts();
        let (map, cell, facing) = game_start_placement(&data);
        let mut runtime = Runtime::new(data, map, cell, facing, step_frames)
            .map_err(|error| error.to_string())?;
        arm(&mut runtime, &battles)?;
        Ok(session_over(runtime, battles, run))
    }

    /// The title's START: the retail initializer, the run's battles armed and
    /// the opening's own event already fired.
    ///
    /// `loc_64EAC` is the initializer the title's START runs
    /// (`Title_StartOption`, `ps4.asm:87728`), and Event_GameStart is the
    /// pack's own opening; the first frame's scene is that event's first ops.
    /// This is the game a player gets by pressing START, and what the campaign
    /// runner plays.
    ///
    /// # Errors
    ///
    /// A pack with no title initializer, or one whose opening scene is absent.
    pub fn new_game(self) -> Result<Session, String> {
        let Parts {
            data,
            battles,
            step_frames,
            run,
        } = self.into_parts();
        let event = data
            .new_game()
            .ok_or_else(|| "pack has no title initializer (game_start.json)".to_owned())?
            .event_index;
        let mut runtime =
            Runtime::new_game(data, step_frames).map_err(|error| error.to_string())?;
        arm(&mut runtime, &battles)?;
        if !runtime.start_event(event) {
            return Err(format!("the opening event {event:#x} has no scene"));
        }
        Ok(session_over(runtime, battles, run))
    }

    /// The title's CONTINUE: visible slot `slot` through the run's store.
    ///
    /// The load is the boot's own validated path, so a slot that loads here
    /// loads there, and a corrupt one is refused with the validation's message.
    ///
    /// # Errors
    ///
    /// The session has no store (a run that named no directory), the slot is
    /// out of range or unreadable, or the saved map cannot be rebuilt.
    pub fn continue_slot(self, slot: usize) -> Result<Session, String> {
        let Parts {
            data,
            battles,
            step_frames,
            run,
        } = self.into_parts();
        let runtime = run
            .as_store()?
            .load(data, slot, step_frames)
            .map_err(|error| error.to_string())?;
        over_runtime(runtime, battles, run)
    }

    /// A game from a slot file's bytes, without a directory: what the campaign
    /// runner's `--from-chapter` reads.
    ///
    /// The bytes are the retail-shaped slot a CONTINUE writes, so the decode is
    /// the same one, and `slot` is the file's own slot index.
    ///
    /// # Errors
    ///
    /// The bytes are not a valid slot, or the saved map cannot be rebuilt.
    pub fn from_slot_bytes(self, bytes: &[u8], slot: usize) -> Result<Session, String> {
        let Parts {
            data,
            battles,
            step_frames,
            run,
        } = self.into_parts();
        let runtime = crate::save::slot_runtime(&data, bytes, slot, step_frames)
            .map_err(|error| error.to_string())?;
        over_runtime(runtime, battles, run)
    }

    /// A game from an already decoded save: the in-memory form of
    /// [`Start::from_slot_bytes`].
    ///
    /// The debug fixtures and the runtime's own tests build their state this
    /// way — a snapshot and the placement it stands at — because a state a test
    /// hand-builds has no file behind it.
    ///
    /// # Errors
    ///
    /// Whatever [`Runtime::from_save`] rejects: a pixel position off the
    /// 16-pixel grid, or a saved map the pack cannot rebuild.
    pub fn from_save(self, save: RetailSave) -> Result<Session, String> {
        let Parts {
            data,
            battles,
            step_frames,
            run,
        } = self.into_parts();
        let runtime =
            Runtime::from_save(data, save, step_frames).map_err(|error| error.to_string())?;
        over_runtime(runtime, battles, run)
    }

    /// The pack, the battle files, the timing and the run's configuration.
    fn into_parts(self) -> Parts {
        let Start {
            data,
            battles,
            store,
            step_frames,
            scene_dialogue_autoclose,
            title_autostart,
        } = self;
        Parts {
            data,
            battles,
            step_frames,
            run: RunConfig {
                store,
                scene_dialogue_autoclose,
                title_autostart,
            },
        }
    }
}

impl RunConfig {
    /// The run's store, or the refusal that keeps a session off a directory it
    /// never named.
    fn as_store(&self) -> Result<&SaveStore, String> {
        self.store
            .as_ref()
            .ok_or_else(|| "the session has no save directory for this run".to_owned())
    }
}

/// Arms `runtime` with the run's battle files, when it has any.
fn arm(runtime: &mut Runtime, battles: &Option<BattleFiles>) -> Result<(), String> {
    match battles {
        Some(files) => runtime
            .enable_battles(files)
            .map_err(|error| error.to_string()),
        None => Ok(()),
    }
}

/// A session over a runtime whose battles still need arming.
fn over_runtime(
    mut runtime: Runtime,
    battles: Option<BattleFiles>,
    run: RunConfig,
) -> Result<Session, String> {
    arm(&mut runtime, &battles)?;
    Ok(session_over(runtime, battles, run))
}

/// The session itself: the runtime with the run's store beside it and the
/// harness switches the caller set. The battle pack rides along so a START or
/// CONTINUE the title completes later arms the same battles.
fn session_over(runtime: Runtime, battles: Option<BattleFiles>, run: RunConfig) -> Session {
    let RunConfig {
        store,
        scene_dialogue_autoclose,
        title_autostart,
    } = run;
    let mut session = Session::over(runtime, store);
    session.battle_files = battles;
    session.set_scene_dialogue_autoclose(scene_dialogue_autoclose);
    session.set_title_autostart(title_autostart);
    session
}

/// The pack's own first control, or [`FALLBACK_SPAWN`] for a pack that
/// predates game-start extraction.
fn game_start_placement(data: &GameData) -> (u16, Cell, Direction) {
    let Some(start) = data.manifest().game_start.as_ref() else {
        return FALLBACK_SPAWN;
    };
    let cell = Cell::new(start.x_cell as u16, start.y_cell as u16);
    let facing = match start.facing.id {
        0x4 => Direction::Up,
        0x8 => Direction::Right,
        0xC => Direction::Left,
        _ => Direction::Down,
    };
    (start.map.id, cell, facing)
}
