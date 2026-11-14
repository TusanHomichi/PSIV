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
    runner.set_pack(std::sync::Arc::new(
        DialogueSet::load(&dir).expect("the dialogue pack loads"),
    ));
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
    // hard-coded: `loc_686BC` widens the 34-cell box by step_cells (2) a
    // pass and waits for a VInt on every other pass, the first included.
    // Tape 43 shows 16, 48, 80 ... 272 pixels on frames 8203-8211.
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
    assert_eq!(widths, [2, 6, 10, 14, 18, 22, 26, 30, 34]);
}

fn portrait(id: u8) -> Segment {
    Segment::Control(Ctrl::Portrait {
        code: 0xF4,
        operands: vec![id],
        id,
        position: None,
    })
}

/// What one frame shows: the portrait, the glyphs, and whether the next
/// frame is a lag frame.
fn shown(runner: &DialogueRunner) -> (Option<u8>, usize, bool) {
    let view = runner.view().expect("a window");
    (view.portrait, view.revealed, runner.lag_frame())
}

#[test]
fn a_new_windows_portrait_comes_with_its_first_glyph_after_two_lag_frames() {
    // Tape 43, `Cutscene_Rykros`'s second `Event_RunDialogue3` (`$F4 6`,
    // "Hey!"): the box opens on frames 8203-8211, 8212-8215 hold
    // `Main_Frame_Count`, and 8216 draws the portrait with the first glyph,
    // 8219 the second. The runner folds the routine's own two frames of
    // setup into the typewriter's three; the portrait adds its two lag frames.
    let Some(mut runner) = packed_runner() else {
        return;
    };
    let mut game = GameState::new();
    assert!(runner.open_resolved(&entry(vec![portrait(6), text("Hey!")]), &game));
    let mut frames = Vec::new();
    for _ in 0..17 {
        frame(&mut runner, &mut game, Pad::NEUTRAL);
        frames.push(shown(&runner));
    }
    // Frames 1-8 (8203-8210) open the box.
    let mut expected = vec![(None, 0, false); 8];
    expected.extend([
        (None, 0, true),     // 9 (8211): open; the next two frames lag
        (None, 0, true),     // 10 (8212): lag
        (None, 0, false),    // 11 (8213): lag
        (None, 0, false),    // 12-13 (8214-8215): the typewriter's wait
        (None, 0, false),    //
        (Some(6), 1, false), // 14 (8216): the portrait and "H"
        (Some(6), 1, false),
        (Some(6), 1, false),
        (Some(6), 2, false), // 17 (8219): "e"
    ]);
    assert_eq!(frames, expected);
}

#[test]
fn a_portrait_change_destroys_the_old_one_then_lags_two_frames() {
    // Tape 43 frame 7400 clears the page (`$FD`, then `$F4 8`); 7401 lags;
    // 7402 shows the window without the old portrait; 7403-7404 lag; 7405
    // draws the new portrait with the first glyph. A page with no `$F4`
    // clears on 7576 and draws its first glyph on 7578. So the change adds
    // three frames to the page, one destroy and two lag. The control types
    // its first glyph on the press frame (Speak held, tape 16); with the
    // change, that step waits behind the three frames.
    let mut runner = DialogueRunner::new();
    let mut game = GameState::new();
    let page = |id: Option<u8>| {
        let mut segments = vec![portrait(1), text("Are"), wait()];
        segments.extend(id.map(portrait));
        segments.push(text("We"));
        entry(segments)
    };
    for (next, expected) in [
        (
            Some(8),
            vec![
                (None, 0, true),     // the press frame: the destroy
                (None, 0, true),     // lag
                (None, 0, false),    // lag
                (Some(8), 1, false), // the step the press reached
            ],
        ),
        (
            None,
            vec![
                (Some(1), 1, false),
                (Some(1), 1, false),
                (Some(1), 1, false),
            ],
        ),
    ] {
        assert!(runner.open_resolved(&page(next), &game));
        let mut signals = Vec::new();
        for _ in 0..40 {
            if runner.view().is_some_and(|view| view.dismissable) {
                break;
            }
            frame(&mut runner, &mut game, Pad::NEUTRAL);
        }
        assert_eq!(shown(&runner), (Some(1), 3, false), "the first page is up");
        signals.extend(frame(&mut runner, &mut game, Pad::new(Button::Speak)));
        let mut frames = vec![shown(&runner)];
        for _ in 1..expected.len() {
            frame(&mut runner, &mut game, Pad::NEUTRAL);
            frames.push(shown(&runner));
        }
        assert_eq!(frames, expected, "next portrait {next:?}");
        assert!(faults(&signals).is_empty());
        runner.close_window();
    }
}

#[test]
fn hiding_the_portrait_takes_no_frame_and_shows_with_the_next_glyph() {
    // `$F4 0` is `loc_6A29C` alone: the plane is restored with no VInt, so
    // the old portrait goes with the next glyph and no frame is added. Here
    // it sits after two glyphs of the page: "B" on the press frame (Speak
    // held), "C" three frames on, then "D" with the portrait gone.
    let mut runner = DialogueRunner::new();
    let mut game = GameState::new();
    let segments = vec![
        portrait(1),
        text("A"),
        wait(),
        text("BC"),
        portrait(0),
        text("D"),
    ];
    assert!(runner.open_resolved(&entry(segments), &game));
    for _ in 0..40 {
        if runner.view().is_some_and(|view| view.dismissable) {
            break;
        }
        frame(&mut runner, &mut game, Pad::NEUTRAL);
    }
    frame(&mut runner, &mut game, Pad::new(Button::Speak));
    let mut frames = vec![shown(&runner)];
    for _ in 0..6 {
        frame(&mut runner, &mut game, Pad::NEUTRAL);
        frames.push(shown(&runner));
    }
    let up = |glyphs| (Some(1), glyphs, false);
    assert_eq!(
        frames,
        [up(1), up(1), up(1), up(2), up(2), up(2), (None, 3, false)]
    );
}

/// One frame, as [`frame`], and whether it was a lag frame: the shell asks
/// after the input half, which can start one.
fn lag_step(
    runner: &mut DialogueRunner,
    game: &mut GameState,
    pad: Pad,
) -> (bool, Vec<DialogueSignal>) {
    runner.input(game, pad, false);
    let lag = runner.lag_frame();
    runner.window(game);
    (lag, runner.drain_signals())
}

/// A frame: whether it was a lag frame, the portrait and the glyphs it showed.
type Shown = (bool, Option<u8>, usize);

/// One frame per pad: whether it was a lag frame, then what it showed.
fn trace(
    runner: &mut DialogueRunner,
    game: &mut GameState,
    pads: impl IntoIterator<Item = Pad>,
) -> Vec<Shown> {
    pads.into_iter()
        .map(|pad| {
            let (lag, _) = lag_step(runner, game, pad);
            let view = runner.view().expect("a window");
            (lag, view.portrait, view.revealed)
        })
        .collect()
}

/// Neutral frames until the typewriter has drawn `glyphs`.
fn type_to(runner: &mut DialogueRunner, game: &mut GameState, glyphs: usize) {
    for _ in 0..40 {
        if runner.view().expect("a window").revealed >= glyphs {
            return;
        }
        frame(runner, game, Pad::NEUTRAL);
    }
    panic!("the typewriter never drew {glyphs} glyphs");
}

fn lag_count(frames: &[Shown]) -> usize {
    frames.iter().filter(|frame| frame.0).count()
}

#[test]
fn a_press_after_the_last_glyph_waits_for_a_portrait_that_follows_it() {
    // `TextCtrlCode_Portrait` runs before the routine reads the page end, so
    // a `$F4` after the page's last glyph spends its frames before a press
    // can leave the page: through a `$FD` to the next page, or through the
    // terminator to the close. Pressed on the frame after the last glyph, the
    // press is the portrait's first lag frame and is lost; the page stays.
    for (end, next) in [(Some(wait()), Some("Yo")), (None, None)] {
        let mut runner = DialogueRunner::new();
        let mut game = GameState::new();
        let mut segments = vec![text("Hi"), portrait(1)];
        segments.extend(end);
        segments.extend(next.map(text));
        assert!(runner.open_resolved(&entry(segments), &game));
        type_to(&mut runner, &mut game, 2);
        assert!(!runner.view().expect("a window").dismissable, "{next:?}");
        let (lag, signals) = lag_step(&mut runner, &mut game, Pad::new(Button::Speak));
        assert!(faults(&signals).is_empty());
        let view = runner.view().expect("the press did not close the window");
        assert_eq!(view.lines[0], "Hi", "{next:?}: the press left the page");
        let mut frames = vec![(lag, view.portrait, view.revealed)];
        while !runner.view().expect("a window").dismissable {
            assert!(frames.len() < 10, "{next:?}: {frames:?}");
            frames.extend(trace(&mut runner, &mut game, [Pad::NEUTRAL]));
        }
        assert_eq!(lag_count(&frames), 2, "{next:?}: {frames:?}");
        assert_eq!(runner.view().expect("a window").portrait, Some(1));
        let signals = frame(&mut runner, &mut game, Pad::new(Button::Speak));
        match next {
            Some(next) => {
                // The next page keeps the portrait and spends no frame on it.
                let frames = trace(&mut runner, &mut game, [Pad::NEUTRAL; 12]);
                assert_eq!(runner.view().expect("a window").lines[0], next);
                assert_eq!(lag_count(&frames), 0, "{frames:?}");
                assert!(frames.iter().all(|frame| frame.1 == Some(1)), "{frames:?}");
            }
            None => assert!(
                signals
                    .iter()
                    .any(|signal| matches!(signal, DialogueSignal::Closed { .. })),
                "{signals:?}"
            ),
        }
    }
}

#[test]
fn a_choice_waits_for_the_portrait_before_it() {
    let mut runner = DialogueRunner::new();
    let mut game = GameState::new();
    let segments = vec![text("Ok"), portrait(1), yes_no(), text("Yes")];
    assert!(runner.open_resolved(&entry(segments), &game));
    type_to(&mut runner, &mut game, 2);
    let answered = |signals: &[DialogueSignal]| {
        signals
            .iter()
            .any(|signal| matches!(signal, DialogueSignal::ChoiceAnswered(_)))
    };
    let (lag, signals) = lag_step(&mut runner, &mut game, Pad::new(Button::Speak));
    assert!(
        !answered(&signals),
        "answered before the portrait: {signals:?}"
    );
    let mut frames = vec![(lag, None, 2)];
    while !runner
        .view()
        .and_then(|view| view.choice)
        .is_some_and(|choice| choice.ready)
    {
        assert!(frames.len() < 10, "{frames:?}");
        frames.extend(trace(&mut runner, &mut game, [Pad::NEUTRAL]));
    }
    assert_eq!(lag_count(&frames), 2, "{frames:?}");
    assert_eq!(runner.view().expect("a window").portrait, Some(1));
    let signals = frame(&mut runner, &mut game, Pad::new(Button::Speak));
    assert!(answered(&signals), "{signals:?}");
}

#[test]
fn a_board_page_closes_only_once_its_portrait_is_up() {
    // The guild board's page closes itself at its terminator; a `$F4` before
    // the terminator runs first, and the page it leaves shows the portrait.
    let mut runner = DialogueRunner::new();
    runner.set_board_text(true);
    let mut game = GameState::new();
    assert!(runner.open_resolved(&entry(vec![text("Hi"), portrait(1)]), &game));
    let mut lags = 0;
    for _ in 0..20 {
        let (lag, signals) = lag_step(&mut runner, &mut game, Pad::NEUTRAL);
        lags += usize::from(lag);
        if signals
            .iter()
            .any(|signal| matches!(signal, DialogueSignal::Closed { .. }))
        {
            break;
        }
    }
    assert_eq!(lags, 2);
    let page = runner.view().expect("the board page stays");
    assert_eq!((page.portrait, page.revealed), (Some(1), 2));
}

#[test]
fn each_portrait_change_runs_at_its_own_glyph_in_stream_order() {
    // Every `$F4` is `TextCtrlCode_Portrait` in its own right: a load is two
    // lag frames, and replacing a portrait that is up (the same id included)
    // first spends a counted destroy frame showing none. `$F4 0` costs
    // nothing and the old portrait goes with the next glyph. Each new
    // portrait shows with the glyph after its `$F4`.
    let lag = |portrait| (true, portrait, 0);
    let up = |portrait, glyphs| (false, portrait, glyphs);
    let cases: [(&str, Vec<Segment>, Vec<Shown>); 4] = [
        (
            "adjacent",
            vec![portrait(1), portrait(2), text("A")],
            // Load 1: lag, lag. Load 2: destroy, lag, lag. Then "A".
            vec![
                lag(None),
                lag(None),
                up(None, 0),
                lag(None),
                lag(None),
                up(None, 0),
                up(None, 0),
                up(Some(2), 1),
            ],
        ),
        (
            "apart",
            vec![text("A"), portrait(1), text("B"), portrait(2), text("C")],
            vec![
                up(None, 0),
                up(None, 0),
                up(None, 1),
                (true, None, 1),
                (true, None, 1),
                up(None, 1),
                up(None, 1),
                up(Some(1), 2),
                (false, None, 2), // destroy
                (true, None, 2),
                (true, None, 2),
                up(None, 2),
                up(None, 2),
                up(Some(2), 3),
            ],
        ),
        (
            "the same id again",
            vec![portrait(1), text("A"), portrait(1), text("B")],
            vec![
                lag(None),
                lag(None),
                up(None, 0),
                up(None, 0),
                up(Some(1), 1),
                (false, None, 1), // destroy
                (true, None, 1),
                (true, None, 1),
                up(None, 1),
                up(None, 1),
                up(Some(1), 2),
            ],
        ),
        (
            "load, hide, load",
            vec![
                portrait(1),
                text("A"),
                portrait(0),
                text("B"),
                portrait(2),
                text("C"),
            ],
            vec![
                lag(None),
                lag(None),
                up(None, 0),
                up(None, 0),
                up(Some(1), 1),
                up(Some(1), 1),
                up(Some(1), 1),
                up(None, 2), // the hide goes with "B"
                (true, None, 2),
                (true, None, 2),
                up(None, 2),
                up(None, 2),
                up(Some(2), 3),
            ],
        ),
    ];
    for (name, segments, expected) in cases {
        let mut runner = DialogueRunner::new();
        let mut game = GameState::new();
        assert!(runner.open_resolved(&entry(segments), &game));
        let frames = trace(&mut runner, &mut game, vec![Pad::NEUTRAL; expected.len()]);
        assert_eq!(frames, expected, "{name}");
    }
}

#[test]
fn a_portrait_the_walk_reaches_after_a_delay_or_an_action_runs_before_its_glyph() {
    // A `$F9` that runs out, or a `$F2` the window released, continues the
    // walk into the `$F4`: its frames still come before the glyph after it,
    // with Speak held (one glyph a frame) as without.
    let cases: [(&str, Vec<Segment>, usize); 4] = [
        (
            "delay, load",
            vec![text("A"), super::tests::delay(4), portrait(1), text("B")],
            2,
        ),
        (
            "delay, swap",
            vec![
                portrait(2),
                text("A"),
                super::tests::delay(4),
                portrait(1),
                text("B"),
            ],
            4,
        ),
        (
            "action, load",
            vec![text("A"), set_flag(5), portrait(1), text("B")],
            2,
        ),
        (
            "action, swap",
            vec![portrait(2), text("A"), set_flag(5), portrait(1), text("B")],
            4,
        ),
    ];
    for (name, segments, lags) in cases {
        for pad in [Pad::new(Button::Speak), Pad::NEUTRAL] {
            let mut runner = DialogueRunner::new();
            let mut game = GameState::new();
            assert!(runner.open_resolved(&entry(segments.clone()), &game));
            let frames = trace(&mut runner, &mut game, vec![pad; 24]);
            let first_b = frames
                .iter()
                .position(|frame| frame.2 == 2)
                .unwrap_or_else(|| panic!("{name}: \"B\" never drawn: {frames:?}"));
            assert_eq!(frames[first_b].1, Some(1), "{name} {pad:?}: {frames:?}");
            assert!(
                frames[first_b..]
                    .iter()
                    .all(|frame| !frame.0 && frame.1 == Some(1)),
                "{name} {pad:?}: the portrait ran after its glyph: {frames:?}"
            );
            assert_eq!(lag_count(&frames), lags, "{name} {pad:?}: {frames:?}");
        }
    }
}
