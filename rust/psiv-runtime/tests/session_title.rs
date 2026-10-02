//! The front door and the defeat that returns to it, headless, pads only.
//!
//! Power-on is retail's title, and everything the player does there — walking
//! the phases, opening a row, continuing a slot, erasing one — is a pad press
//! this file makes. The session does the rest: no test here calls a runtime
//! mutator, and every slot on disk is written by the session's own store.
//!
//! ```text
//! power-on to START      the phases run on their own, 563 frames of them
//! CONTINUE               a slot this file saved through the camp menu
//! ERASE DATA             the slot's payload zeroed, its row gone
//! a defeat               the perished notice, fourteen fade frames, the title
//! ```
//!
//! The negative controls sit beside the cases they guard: CONTINUE and ERASE
//! DATA on an empty row change nothing at all, and the certified title capture's
//! own state (clone tick 480) is pinned here so a phase-length change cannot
//! pass unnoticed.

use std::path::{Path, PathBuf};

use psiv_core::battle::status;
use psiv_core::{CharId, Flag, GameState, RetailLocation, RetailSave, StepFrames};
use psiv_data::GameData;
use psiv_runtime::{
    Button, CampPage, FrameMode, GAME_OVER_FADE_FRAMES, GameOverFrame, Pad, Runtime, RuntimeEvent,
    SaveStore, Session, TitleEntry, TitleFrame, TitlePhase, TitleView, TitleWindow,
};

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

/// Frames between a driver's presses: a fresh press every fourth frame, with
/// the released frames in between — the shape the oracle tapes use.
const PRESS_PERIOD: u64 = 4;

/// The certified title capture's clone tick (`tools/certify.py`, pair
/// `title`): oracle tape 25 frame 450, the settled logo and subtitle inside the
/// Press Start hold.
const CERTIFIED_TITLE_TICK: u32 = 480;

/// The frames the native opening smoke holds `Up` for: one cell step.
const UP_FRAMES: u32 = 8;

fn pack_present() -> bool {
    Path::new(PACK).join("manifest.json").is_file()
}

fn pack() -> &'static Path {
    Path::new(PACK)
}

/// A run directory this test owns. Every save goes here — never the checkout's
/// `saves/` — and the test removes it again.
fn run_dir(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("psiv-title-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).expect("the run directory is ours");
    directory
}

/// The retail initializer's state, the one START builds: the new-game flag
/// banks, 500 meseta, and the roster the battle pack seats (`enable_battles`
/// is what fills the character records; a bare initializer has party slots and
/// nothing in them yet).
fn new_game_state() -> GameState {
    let data = GameData::load(pack()).expect("pack loads");
    let mut initial = Runtime::new_game(data, StepFrames::default()).expect("START");
    initial
        .enable_battles(&psiv_data::BattleFiles::load(pack()).expect("battle files load"))
        .expect("battles enable");
    GameState::from_snapshot(&initial.game().snapshot())
}

/// A session standing at `(map, x, y)` with `game`'s party, saving through
/// `directory`.
fn session_at(place: (u16, u16, u16), game: &GameState, directory: &Path) -> Session {
    let data = GameData::load(pack()).expect("pack loads");
    let (map, x, y) = place;
    let runtime = Runtime::from_save(
        data,
        RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index_2: 0,
                map_index: map,
                char_x: x * 16,
                char_y: y * 16,
            },
        },
        StepFrames::default(),
    )
    .expect("runtime builds");
    Session::with_saves(runtime, SaveStore::new(directory))
}

/// The power-on session the shell builds before the title: a runtime at the
/// pack's own spawn, with the title owning its frames.
fn power_on(directory: &Path) -> Session {
    let data = GameData::load(pack()).expect("pack loads");
    let start = data.manifest().game_start.clone().expect("first control");
    let runtime = Runtime::new(
        data,
        start.map.id,
        psiv_core::Cell::new(start.x_cell as u16, start.y_cell as u16),
        psiv_core::Direction::Down,
        StepFrames::default(),
    )
    .expect("the spawn runtime builds");
    let mut session = Session::with_saves(runtime, SaveStore::new(directory));
    session.start_title();
    session
}

/// One frame of the title, as the shell presents it.
fn title_tick(session: &mut Session, pad: Pad) -> TitleFrame {
    let frame = session.frame(pad);
    assert_eq!(frame.mode, FrameMode::Title, "the title owns the frame");
    frame.title.expect("a title frame carries its view")
}

/// A fresh press on the title: down for one frame, released for the next. The
/// frame that leaves the front door is the last title frame, so the release is
/// only spent while the title still owns the session.
fn title_press(session: &mut Session, button: Button) -> TitleFrame {
    let frame = title_tick(session, Pad::new(button));
    if session.title().is_some() {
        title_tick(session, Pad::NEUTRAL);
    }
    frame
}

/// Runs the title's own phases with neutral pads until its option window
/// opens by itself (the retail `#$233` hold), and returns that view.
fn settle_to_option_window(session: &mut Session) -> TitleView {
    let mut view = session.title().expect("the title is up");
    for _ in 0..(CERTIFIED_TITLE_TICK + 563 + 64) {
        view = title_tick(session, Pad::NEUTRAL).view.expect("a view");
        if view.phase == TitlePhase::Menu {
            return view;
        }
    }
    panic!("the title's option window never opened: {view:?}");
}

/// Moves the title's cursor onto `target` with Down presses; the windows wrap,
/// so it always arrives.
fn title_cursor_to(session: &mut Session, target: usize) {
    for _ in 0..16 {
        let view = title_tick(session, Pad::NEUTRAL).view.expect("a view");
        if view.cursor == target {
            return;
        }
        title_press(session, Button::Down);
    }
    panic!("the title's cursor never reached row {target}");
}

/// The opening, driven the way the shell drives it: a Speak press while a
/// window is ready for one, nothing otherwise.
fn settle_dialogue(session: &mut Session, ticks: &mut u64) {
    for _ in 0..600 {
        *ticks += 1;
        let ready = session
            .runtime()
            .dialogue_view()
            .is_some_and(|view| view.dismissable || view.choice.is_some_and(|choice| choice.ready));
        let pad = if ready && ticks.is_multiple_of(PRESS_PERIOD) {
            Pad::new(Button::Speak)
        } else {
            Pad::NEUTRAL
        };
        let frame = session.frame(pad);
        for event in &frame.events {
            match event {
                RuntimeEvent::SceneFaulted { .. }
                | RuntimeEvent::SceneMissing { .. }
                | RuntimeEvent::SceneBattleFailed { .. }
                | RuntimeEvent::MapRefreshFailed { .. }
                | RuntimeEvent::UnpackedTarget { .. }
                | RuntimeEvent::WarpUnmapped { .. } => panic!("opening faulted: {event:?}"),
                _ => {}
            }
        }
        if session.runtime().dialogue_open() {
            return;
        }
    }
    panic!("the opening's first dialogue never opened");
}

/// Power-on reaches the certified title state, and START hands the session to
/// the opening: its first dialogue opens with no other setup.
#[test]
fn power_on_reaches_the_title_and_start_opens_the_opening() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let directory = run_dir("power-on");
    let mut session = power_on(&directory);

    // The certified capture: clone tick 480 is inside the reveal's settled art
    // (the reveal's own 300 frames), 563 frames before the window opens by
    // itself. No window is up yet and the phase clock has run 279 frames.
    let mut view = session.title().expect("the title is up");
    for _ in 0..CERTIFIED_TITLE_TICK {
        view = title_tick(&mut session, Pad::NEUTRAL).view.expect("a view");
    }
    assert_eq!(view.elapsed, CERTIFIED_TITLE_TICK);
    assert_eq!(view.phase, TitlePhase::Reveal);
    assert_eq!(view.ticks, 279, "the reveal's own clock at the capture");
    assert_eq!(view.window, TitleWindow::None);
    assert_eq!(
        view.slots, [false; 3],
        "an empty run directory shows three empty rows"
    );

    let view = settle_to_option_window(&mut session);
    assert_eq!(
        view.window,
        TitleWindow::NoSave,
        "a save-less card opens the single START row"
    );
    assert_eq!(view.phase, TitlePhase::Menu);

    let started = title_press(&mut session, Button::Speak);
    assert_eq!(
        started.entered,
        Some(TitleEntry::Started {
            event_started: true
        }),
        "START fires Event_GameStart"
    );
    assert!(
        started.view.is_none(),
        "the front door is over: the field owns the next frame"
    );
    assert!(session.title().is_none());

    // The opening's first dialogue, reached with pads only.
    let mut ticks: u64 = 0;
    settle_dialogue(&mut session, &mut ticks);
    let dialogue = session.runtime().dialogue_view().expect("the box is up");
    assert!(
        dialogue.lines.iter().any(|line| !line.is_empty()),
        "the opening's first line has text: {:?}",
        dialogue.lines
    );
    std::fs::remove_dir_all(&directory).expect("the run directory is ours");
}

/// CONTINUE, from a slot a camp SAVE wrote in the same test, restores the same
/// map, cell, party, purse and flags.
#[test]
fn continue_loads_the_slot_a_camp_save_wrote() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let directory = run_dir("continue");
    // Motavia (99,84) with (99,85) south of it: the pair the field-status
    // fixture walks, so the step this test saves is a verified one.
    let mut game = new_game_state();
    game.set_party([Some(CharId(0)), None, None, None, None]);
    game.set_money(1234);
    game.set(Flag::event(0x0C)).expect("a story flag");
    let mut session = session_at((0x00, 99, 84), &game, &directory);

    // One ordinary step south, so the saved cell is not the spawn cell.
    for _ in 0..UP_FRAMES {
        session.frame(Pad::new(Button::Down));
    }
    session.frame(Pad::NEUTRAL);
    let saved_cell = session.runtime().state().cell();
    assert_eq!(saved_cell.y, 85, "one cell south of the spawn");
    let saved_state = session.runtime().game().snapshot();
    let saved_map = session.runtime().map_id();

    // The camp's STATE > SAVE, through the session's own store.
    session.frame(Pad::new(Button::Camp));
    session.frame(Pad::NEUTRAL);
    assert_eq!(
        session.camp_view().expect("the camp opened").page,
        CampPage::Root
    );
    for _ in 0..8 {
        if session.camp_view().expect("the camp is up").root_selection == 4 {
            break;
        }
        session.frame(Pad::new(Button::Down));
        session.frame(Pad::NEUTRAL);
    }
    session.frame(Pad::new(Button::Speak));
    session.frame(Pad::NEUTRAL);
    for _ in 0..8 {
        if session.camp_view().expect("the camp is up").state_selection == 2 {
            break;
        }
        session.frame(Pad::new(Button::Down));
        session.frame(Pad::NEUTRAL);
    }
    session.frame(Pad::new(Button::Speak));
    session.frame(Pad::NEUTRAL);
    assert_eq!(
        session.camp_view().expect("the camp is up").page,
        CampPage::SaveSlots
    );
    let frame = session.frame(Pad::new(Button::Speak));
    assert_eq!(frame.mode, FrameMode::Camp);
    assert_eq!(
        session.camp_view().expect("the camp is up").message,
        "FILE SAVED",
        "the session wrote the slot itself"
    );
    assert!(
        directory.join("slot_1.sram").is_file(),
        "the camp's SAVE wrote slot 1 through the session's store"
    );

    // A fresh process's session over the same directory: power-on, CONTINUE.
    let mut resumed = power_on(&directory);
    let view = settle_to_option_window(&mut resumed);
    assert_eq!(
        view.slots,
        [true, false, false],
        "the written slot is visible"
    );
    assert_eq!(view.window, TitleWindow::SaveOptions);
    let slots = title_press(&mut resumed, Button::Speak);
    assert_eq!(
        slots.view.expect("the slot list is up").phase,
        TitlePhase::Slots
    );
    assert_eq!(slots.entered, None, "the list is not a load yet");
    let continued = title_press(&mut resumed, Button::Speak);
    assert_eq!(continued.entered, Some(TitleEntry::Continued { slot: 0 }));
    assert_eq!(resumed.runtime().map_id(), saved_map, "same map");
    assert_eq!(resumed.runtime().state().cell(), saved_cell, "same cell");
    assert_eq!(
        resumed.runtime().game().snapshot(),
        saved_state,
        "same party, purse and flags"
    );
    assert_eq!(resumed.runtime().game().money(), 1234);
    assert!(resumed.runtime().game().is_set(Flag::event(0x0C)));
    std::fs::remove_dir_all(&directory).expect("the run directory is ours");
}

/// ERASE DATA zeroes the picked slot's payload, drops its row from the title
/// and leaves the other slots alone.
#[test]
fn erase_data_clears_the_picked_slot() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let directory = run_dir("erase");
    // Two slots on disk: the eraser takes the first and leaves the second.
    let mut game = new_game_state();
    game.set_party([Some(CharId(0)), None, None, None, None]);
    for slot in 0..2 {
        let session = session_at((0x00, 99, 84), &game, &directory);
        let path = session.save_slot(slot).expect("the store writes the slot");
        assert!(path.is_file());
    }

    let mut session = power_on(&directory);
    let view = settle_to_option_window(&mut session);
    assert_eq!(view.slots, [true, true, false]);
    title_cursor_to(&mut session, 2); // ERASE DATA
    let slots = title_press(&mut session, Button::Speak);
    assert_eq!(
        slots.view.expect("the slot list is up").phase,
        TitlePhase::EraseSlots
    );
    title_cursor_to(&mut session, 0);
    let confirm = title_press(&mut session, Button::Speak);
    assert_eq!(
        confirm.view.expect("the confirmation is up").window,
        TitleWindow::EraseConfirm
    );
    let erased = title_press(&mut session, Button::Speak);
    let Some(psiv_runtime::TitleErase::Erased { slot, .. }) = erased.erased.as_ref() else {
        panic!("the erase did not run: {:?}", erased.erased);
    };
    assert_eq!(*slot, 0);
    let view = erased.view.expect("the option window came back");
    assert_eq!(view.phase, TitlePhase::Menu);
    assert_eq!(view.slots, [false, true, false], "the erased row is gone");

    // The erased slot no longer loads; the untouched one still does.
    assert!(
        SaveStore::new(&directory)
            .load(
                GameData::load(pack()).expect("pack loads"),
                0,
                StepFrames::default()
            )
            .is_err(),
        "an erased slot does not continue"
    );
    assert!(
        SaveStore::new(&directory)
            .load(
                GameData::load(pack()).expect("pack loads"),
                1,
                StepFrames::default()
            )
            .is_ok(),
        "the other slot is untouched"
    );
    std::fs::remove_dir_all(&directory).expect("the run directory is ours");
}

/// Negative control: CONTINUE on an empty row is refused with no state change
/// at all — the title stays on the slot list, on the same row, and the runtime
/// it would have replaced is the one still running.
#[test]
fn continue_on_an_empty_slot_is_refused_with_no_state_change() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let directory = run_dir("empty-continue");
    let mut game = new_game_state();
    game.set_party([Some(CharId(0)), None, None, None, None]);
    session_at((0x00, 99, 84), &game, &directory)
        .save_slot(0)
        .expect("slot 1 is written");

    let mut session = power_on(&directory);
    settle_to_option_window(&mut session);
    let list = title_press(&mut session, Button::Speak); // CONTINUE
    assert_eq!(
        list.view.expect("the slot list is up").phase,
        TitlePhase::Slots
    );
    let before = session.runtime().game().snapshot();
    let map = session.runtime().map_id();
    let cell = session.runtime().state().cell();
    title_cursor_to(&mut session, 1); // an empty row
    let refused = title_press(&mut session, Button::Speak);
    assert_eq!(refused.entered, None, "nothing was continued");
    assert_eq!(refused.failure, None, "and nothing failed");
    let view = refused.view.expect("the slot list is still up");
    assert_eq!(view.phase, TitlePhase::Slots);
    assert_eq!(view.cursor, 1, "the cursor did not move");
    assert_eq!(view.slots, [true, false, false]);
    assert_eq!(session.runtime().map_id(), map);
    assert_eq!(session.runtime().state().cell(), cell);
    assert_eq!(
        session.runtime().game().snapshot(),
        before,
        "no state change"
    );
    std::fs::remove_dir_all(&directory).expect("the run directory is ours");
}

/// Negative control: ERASE DATA on an empty row opens no confirmation and
/// writes nothing.
#[test]
fn erase_data_on_an_empty_slot_is_refused_with_no_state_change() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let directory = run_dir("empty-erase");
    let mut game = new_game_state();
    game.set_party([Some(CharId(0)), None, None, None, None]);
    let written = session_at((0x00, 99, 84), &game, &directory)
        .save_slot(0)
        .expect("slot 1 is written");
    let before = std::fs::read(&written).expect("the slot file is there");

    let mut session = power_on(&directory);
    settle_to_option_window(&mut session);
    title_cursor_to(&mut session, 2);
    title_press(&mut session, Button::Speak); // ERASE DATA's slot list
    title_cursor_to(&mut session, 1); // an empty row
    let refused = title_press(&mut session, Button::Speak);
    assert_eq!(refused.erased, None, "no erase ran");
    let view = refused.view.expect("the slot list is still up");
    assert_eq!(view.phase, TitlePhase::EraseSlots);
    assert_eq!(view.cursor, 1);
    assert_eq!(
        std::fs::read(&written).expect("the slot file is there"),
        before
    );
    assert_eq!(
        session.save_slots(),
        [true, false, false],
        "the populated row is still there"
    );
    std::fs::remove_dir_all(&directory).expect("the run directory is ours");
}

/// The field-status fixture's defeat: the perished notice, the fourteen fade
/// frames and the title they hand the game back to.
#[test]
fn a_defeat_reaches_game_over_and_returns_to_the_title() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let directory = run_dir("defeat");
    // The field-status fixture's poison state, with Chaz alone: the shape the
    // runtime's own perished test walks, on Motavia where the status clock
    // runs (map $00, the verified (99,84) pair).
    let mut game = new_game_state();
    game.set(Flag::event(0x0C)).expect("a story flag");
    game.set_party([Some(CharId(0)), None, None, None, None]);
    let chaz = game.roster_mut().get_mut(CharId(0)).expect("Chaz's record");
    chaz.curr_hp = 1;
    chaz.status = status::POISONED;
    let mut session = session_at((0x00, 99, 84), &game, &directory);
    let mut ticks: u64 = 0;
    // Four walks south/north: the poison kills Chaz, the notices queue.
    for leg in 0..4 {
        let direction = if leg % 2 == 0 {
            Button::Down
        } else {
            Button::Up
        };
        let start = session.runtime().state().cell();
        for _ in 0..64 {
            ticks += 1;
            let frame = session.frame(Pad::new(direction));
            if session.runtime().dialogue_open() {
                // The notice window: acknowledge it with a fresh press.
                session.frame(Pad::new(Button::Speak));
                session.frame(Pad::NEUTRAL);
                continue;
            }
            if frame
                .events
                .iter()
                .any(|event| matches!(event, RuntimeEvent::StepCompleted { .. }))
            {
                session.frame(Pad::NEUTRAL);
                break;
            }
        }
        if session.runtime().game_over() {
            break;
        }
        assert_ne!(
            session.runtime().state().cell(),
            start,
            "the walk landed a step"
        );
    }
    // The notices, the boundary they end on and the fade: one pass, because
    // the frame that acknowledges the perished message is the fade's first.
    let mut fade: Vec<GameOverFrame> = Vec::new();
    for _ in 0..900 {
        ticks += 1;
        let pad = if session.runtime().dialogue_open() && ticks.is_multiple_of(PRESS_PERIOD) {
            Pad::new(Button::Speak)
        } else {
            Pad::NEUTRAL
        };
        let frame = session.frame(pad);
        if let Some(game_over) = frame.game_over {
            assert_eq!(frame.mode, FrameMode::GameOver, "the fade owns the frame");
            fade.push(game_over);
        }
        if session.title().is_some() {
            break;
        }
    }
    assert!(
        session.runtime().game_over(),
        "the defeated party reached the game-over boundary"
    );
    assert_eq!(
        fade.len(),
        usize::from(GAME_OVER_FADE_FRAMES) + 1,
        "the fade's first frame through the frame the title returns on"
    );
    assert_eq!(fade[0].frames, 0);
    assert!(!fade[0].title_restored);
    let last = fade.last().expect("the fade ran");
    assert_eq!(last.frames, GAME_OVER_FADE_FRAMES);
    assert!(last.title_restored, "the title takes this frame back");
    let view = session.title().expect("the title is back over the defeat");
    assert_eq!(view.phase, TitlePhase::Sega, "a fresh front door");
    assert_eq!(view.slots, [false; 3], "the defeat wrote no save");
    std::fs::remove_dir_all(&directory).expect("the run directory is ours");
}
