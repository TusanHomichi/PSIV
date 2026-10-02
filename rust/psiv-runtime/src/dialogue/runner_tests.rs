//! The runner's own rules: the clock, the presses, the answers and the live
//! flags.
//!
//! These drive [`DialogueRunner`] the way the shell does — one pad per frame,
//! the input half before the window half — so a rule that only holds when the
//! window is ticked by hand would fail here. Entries are built by hand where
//! the rule is synthetic, and taken from the pack where the *retail data* is
//! the point (the mid-message `$FA` chains, the open animation).

use super::{DialogueAction, DialogueRunner, DialogueSignal};
use crate::pad::{Button, Pad};
use psiv_core::{Flag, GameState};
use psiv_data::{ActionKind, Ctrl, DialogueEntry, DialogueSet, FlagScope, Segment};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn entry(segments: Vec<Segment>) -> DialogueEntry {
    DialogueEntry {
        id: 0,
        text: String::new(),
        segments,
        pages: Vec::new(),
    }
}

fn text(run: &str) -> Segment {
    Segment::Text(run.to_owned())
}

fn wait() -> Segment {
    Segment::Control(Ctrl::Wait {
        code: 0xFD,
        operands: Vec::new(),
    })
}

fn event(id: u16) -> Segment {
    Segment::Control(Ctrl::Event {
        code: 0xF6,
        operands: vec![0, id as u8],
        id,
    })
}

fn flag_check(flag: u8, then_entry: u16) -> Segment {
    Segment::Control(Ctrl::FlagCheck {
        code: 0xFA,
        operands: vec![flag, then_entry as u8],
        flag,
        scope: FlagScope::EventFlag,
        then_entry,
    })
}

/// `$F2` 11: set event flag `flag`.
fn set_flag(flag: u8) -> Segment {
    Segment::Control(Ctrl::Action {
        code: 0xF2,
        operands: vec![11, flag],
        action: ActionKind::SetEventFlag,
        action_id: 0,
        sound: None,
        panel: None,
        flag: Some(flag),
    })
}

/// Retail's yes/no at the end of a page, continuing inside the entry.
fn yes_no() -> Segment {
    Segment::Control(Ctrl::YesNo {
        code: 0xF5,
        operands: vec![0, 0],
        yes_entry: 0,
        no_entry: 0,
    })
}

fn pack_dir() -> PathBuf {
    match std::env::var_os("PSIV_RUNTIME_PACK") {
        Some(path) => PathBuf::from(path),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("runtime-pack"),
    }
}

/// A runner with the real pack: its window geometry animates the box, and its
/// trees resolve mid-message jumps. `None` without the local pack.
fn packed_runner() -> Option<DialogueRunner> {
    let dir = pack_dir();
    if !dir.join("dialogue").join("trees.json").is_file() {
        eprintln!("skipping: no dialogue pack at {}", dir.display());
        return None;
    }
    let mut runner = DialogueRunner::new();
    runner.set_pack(DialogueSet::load(&dir).expect("the dialogue pack loads"));
    Some(runner)
}

/// One frame, in the shell's order: the input half, then the window half.
fn frame(runner: &mut DialogueRunner, game: &mut GameState, pad: Pad) -> Vec<DialogueSignal> {
    runner.input(game, pad, false);
    runner.window(game);
    runner.drain_signals()
}

/// Frames with a neutral pad, up to `count`.
fn idle(runner: &mut DialogueRunner, game: &mut GameState, count: usize) {
    for _ in 0..count {
        frame(runner, game, Pad::NEUTRAL);
    }
}

/// Drives the window the way a player does: neutral frames until the page (or
/// the prompt) is ready, then one Speak press.
fn press_when_ready(runner: &mut DialogueRunner, game: &mut GameState) -> Vec<DialogueSignal> {
    for _ in 0..2_000 {
        let view = runner.view().expect("a window is up");
        let ready = view.dismissable || view.choice.is_some_and(|choice| choice.ready);
        if ready {
            break;
        }
        frame(runner, game, Pad::NEUTRAL);
    }
    frame(runner, game, Pad::new(Button::Speak))
}

fn faults(signals: &[DialogueSignal]) -> Vec<&str> {
    signals
        .iter()
        .filter_map(|signal| match signal {
            DialogueSignal::Fault(line) => Some(line.as_str()),
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The rules
// ---------------------------------------------------------------------------

#[test]
fn a_mid_message_flag_check_takes_the_branch_the_live_bank_holds() {
    // Retail data: tree 16 entry 56 asks about Raja, then runs three `$FA`
    // checks. Flag 148 is the last one; its taken branch is entry 57.
    let Some(mut runner) = packed_runner() else {
        return;
    };
    let mut game = GameState::new();
    assert!(runner.open_entry(16, 56, &game));
    press_when_ready(&mut runner, &mut game);
    assert_eq!(
        runner.view().expect("a window").lines,
        ["I've known him a long time.", "He's a weird one...but,"],
        "a clear flag runs on into the entry's own text"
    );
    assert!(faults(&runner.drain_signals()).is_empty());

    // The same entry with the flag set: the last check wins and the window
    // continues in the branch target, which is the retail follow-up.
    let mut game = GameState::new();
    game.set(Flag::event(148)).unwrap();
    let mut runner = packed_runner().expect("pack");
    assert!(runner.open_entry(16, 56, &game));
    // Both answers continue inside the entry (offsets 0), so the prompt's
    // answer does not matter to which check runs.
    press_when_ready(&mut runner, &mut game);
    assert_eq!(
        runner.view().expect("a window").lines,
        [
            "Raja has fallen? But he's the",
            "kind of guy who would return"
        ],
        "the set flag takes the branch into entry 57"
    );
}

#[test]
fn a_flag_set_after_the_window_opened_changes_a_later_branch() {
    // The live-read proof: the entry is open and on screen before the flag
    // changes, and the check that runs afterwards reads the new value. The
    // same window, driven with the flag left clear, runs on instead.
    let Some(mut runner) = packed_runner() else {
        return;
    };
    let mut game = GameState::new();
    assert!(runner.open_entry(16, 56, &game));
    idle(&mut runner, &mut game, 4);
    assert!(runner.is_open(), "the window is up before the write");
    game.set(Flag::event(148)).unwrap();
    press_when_ready(&mut runner, &mut game);
    assert_eq!(
        runner.view().expect("a window").lines,
        [
            "Raja has fallen? But he's the",
            "kind of guy who would return"
        ],
        "the window reads the bank at the branch, not at open time"
    );
}

#[test]
fn an_f2_action_sets_the_event_flag_in_the_game_state() {
    let mut runner = DialogueRunner::new();
    let mut game = GameState::new();
    assert!(!game.is_set(Flag::event(0x2a)));
    assert!(runner.open_resolved(&entry(vec![set_flag(0x2a), text("told")]), &game));
    assert!(!game.is_set(Flag::event(0x2a)), "the action waits its turn");
    let signals = frame(&mut runner, &mut game, Pad::NEUTRAL);
    assert!(
        signals.contains(&DialogueSignal::Action(DialogueAction::SetEventFlag(0x2a))),
        "the action is released to the shell too: {signals:?}"
    );
    assert!(
        game.is_set(Flag::event(0x2a)),
        "$F2 writes the live bank directly"
    );
    assert_eq!(
        runner.view().expect("a window").lines,
        ["told"],
        "the text after the action runs on"
    );
}

#[test]
fn the_typewriter_releases_one_glyph_per_three_frames_and_one_while_speak_is_held() {
    let mut runner = DialogueRunner::new();
    let mut game = GameState::new();
    assert!(runner.open_resolved(&entry(vec![text(&"A".repeat(12))]), &game));
    idle(&mut runner, &mut game, 3);
    assert_eq!(
        runner.view().unwrap().revealed,
        1,
        "released: one glyph in three frames"
    );
    idle(&mut runner, &mut game, 6);
    assert_eq!(runner.view().unwrap().revealed, 3);

    let mut runner = DialogueRunner::new();
    let mut game = GameState::new();
    assert!(runner.open_resolved(&entry(vec![text(&"A".repeat(12))]), &game));
    for frame_count in 1..=5 {
        frame(&mut runner, &mut game, Pad::new(Button::Speak));
        assert_eq!(
            runner.view().unwrap().revealed,
            frame_count,
            "held: one glyph per frame"
        );
    }
}

#[test]
fn a_press_during_the_open_animation_is_swallowed() {
    let Some(mut runner) = packed_runner() else {
        return;
    };
    // A field-status window loads its whole text at once, so its page is
    // complete from the first frame: the only thing holding the press back is
    // the open animation itself (a message page would be held by the
    // typewriter first, which is the other swallow).
    let mut game = GameState::new();
    let lines = vec!["Chaz is on the verge".to_owned(), "of Death.".to_owned()];
    assert!(runner.open_status(&lines, &game));
    let signals = frame(&mut runner, &mut game, Pad::new(Button::Speak));
    let view = runner.view().expect("a window");
    assert!(!view.dismissable, "the box is still opening");
    assert!(view.open_cells > 0, "it did start opening");
    assert_eq!(view.revealed, view.total, "the page is complete");
    assert!(runner.is_open(), "the press did not dismiss the window");
    assert!(faults(&signals).is_empty());
    assert!(runner.take_event().is_none(), "and it fired nothing either");

    // Once the box is open the same press does dismiss it.
    press_when_ready(&mut runner, &mut game);
    assert!(!runner.is_open(), "a settled page dismisses on a press");

    // A prompt whose page has no glyphs meanwhile — a scene's standalone
    // choice — is held back by the same animation gate, not by the
    // typewriter.
    let mut runner = packed_runner().expect("pack");
    let mut game = GameState::new();
    assert!(runner.open_standalone_choice(&game));
    let signals = frame(&mut runner, &mut game, Pad::new(Button::Speak));
    assert!(runner.is_open(), "the prompt waits for the box");
    assert!(
        !signals
            .iter()
            .any(|signal| matches!(signal, DialogueSignal::ChoiceAnswered(_))),
        "and is not answered while it opens: {signals:?}"
    );
    press_when_ready(&mut runner, &mut game);
    assert!(!runner.is_open());
}

#[test]
fn a_held_speak_does_not_advance_a_finished_page_and_a_fresh_press_does() {
    let mut runner = DialogueRunner::new();
    let mut game = GameState::new();
    assert!(runner.open_resolved(&entry(vec![text("one"), wait(), text("two")]), &game));
    // Press and hold: the press lands mid-page and is swallowed, and the hold
    // accelerates the typing without ever advancing the finished page.
    for _ in 0..40 {
        frame(&mut runner, &mut game, Pad::new(Button::Speak));
    }
    let view = runner.view().expect("a window");
    assert_eq!(view.revealed, view.total, "the page typed out");
    assert!(view.dismissable);
    assert_eq!(
        view.lines,
        ["one"],
        "a held button never advances a finished page"
    );

    // A fresh press on the next frame does advance it.
    frame(&mut runner, &mut game, Pad::NEUTRAL);
    let signals = frame(&mut runner, &mut game, Pad::new(Button::Speak));
    assert!(faults(&signals).is_empty());
    assert_eq!(runner.view().expect("a window").lines, ["two"]);
}

#[test]
fn choice_cancel_answers_no() {
    let mut runner = DialogueRunner::new();
    let mut game = GameState::new();
    assert!(runner.open_resolved(&entry(vec![text("well?"), yes_no()]), &game));
    // Neutral frames until the prompt is typed, then Cancel: NO from the YES
    // row, which is the cartridge's direct shortcut.
    for _ in 0..2_000 {
        if runner
            .view()
            .and_then(|view| view.choice)
            .is_some_and(|choice| choice.ready)
        {
            break;
        }
        frame(&mut runner, &mut game, Pad::NEUTRAL);
    }
    assert_eq!(runner.view().unwrap().choice.unwrap().cursor, 0);
    let signals = frame(&mut runner, &mut game, Pad::new(Button::Cancel));
    assert!(
        signals.contains(&DialogueSignal::ChoiceAnswered(false)),
        "cancel is NO: {signals:?}"
    );

    // Speak on the same prompt answers YES, and the cursor's own row with it.
    let mut runner = DialogueRunner::new();
    let mut game = GameState::new();
    assert!(runner.open_resolved(&entry(vec![text("well?"), yes_no()]), &game));
    let signals = press_when_ready(&mut runner, &mut game);
    assert!(
        signals.contains(&DialogueSignal::ChoiceAnswered(true)),
        "speak answers the cursor: {signals:?}"
    );

    // A scene's standalone branch prompt closes on the answer.
    let mut runner = DialogueRunner::new();
    let mut game = GameState::new();
    assert!(runner.open_standalone_choice(&game));
    let signals = press_when_ready(&mut runner, &mut game);
    assert!(signals.contains(&DialogueSignal::ChoiceAnswered(true)));
    assert!(
        !runner.is_open(),
        "the scene's own prompt closes on the answer"
    );
}

#[test]
fn an_npc_preamble_event_yields_the_event_signal() {
    // The interaction walk's preamble is `$FA* ( $F6 | text )`: a tree entry
    // whose checks are skipped can fire an event and show no window. No packed
    // entry uses that shape today, so it is built here.
    let mut runner = DialogueRunner::new();
    let game = GameState::new();
    assert!(!runner.open_resolved(&entry(vec![flag_check(3, 1), event(0x9f)]), &game));
    assert!(!runner.is_open(), "$F6 shows no window");
    assert_eq!(runner.take_event(), Some(0x9f));
    assert_eq!(runner.take_event(), None, "an event is reported once");

    // A mid-message `$F6` is the same signal, and the window shuts behind it.
    let mut runner = DialogueRunner::new();
    let mut game = GameState::new();
    assert!(runner.open_resolved(&entry(vec![text("bye"), wait(), event(0x25)]), &game));
    let signals = press_when_ready(&mut runner, &mut game);
    assert!(faults(&signals).is_empty());
    assert!(!runner.is_open(), "the event ends the window");
    assert_eq!(runner.take_event(), Some(0x25));
}

#[test]
fn the_close_signal_says_whether_the_cursor_was_saved() {
    // A scene's `$F7` page saves the text cursor for the scene
    // (`dialogue_closed`); an ordinary talk entry's last page does not
    // (`dialogue_ended`).
    let mut runner = DialogueRunner::new();
    let mut game = GameState::new();
    assert!(runner.open_resolved(&entry(vec![text("bye")]), &game));
    let signals = press_when_ready(&mut runner, &mut game);
    assert!(
        signals.contains(&DialogueSignal::Closed { suspended: false }),
        "{signals:?}"
    );
    assert!(!runner.is_open());
}

#[test]
fn the_open_animation_takes_nine_frames_from_the_packs_window_geometry() {
    // The oracle's 9-frame open, derived from `window.json` rather than
    // hard-coded: the pack's step_cells is per side and the box is 34 cells
    // wide, so it reaches full width on the ninth frame.
    let Some(mut runner) = packed_runner() else {
        return;
    };
    let mut game = GameState::new();
    assert!(runner.open_resolved(&entry(vec![text("hi")]), &game));
    let mut frames = 0;
    let mut widths = Vec::new();
    while runner.view().expect("a window").open_cells < 34 {
        frame(&mut runner, &mut game, Pad::NEUTRAL);
        frames += 1;
        widths.push(runner.view().unwrap().open_cells);
        assert!(frames <= 16, "the box never opened: {widths:?}");
    }
    assert_eq!(frames, 9);
    assert_eq!(widths, [4, 8, 12, 16, 20, 24, 28, 32, 34]);
}
