//! Portrait changes in the runner: `TextCtrlCode_Portrait`'s destroy frame
//! and decompression lag, each `$F4` at its own glyph in stream order, and
//! what waits behind a change whose frames have not run.
//!
//! Tape 43's `Cutscene_Rykros` frames are the reference
//! (`docs/scenes/SCENE_DIALOGUE.md`, "Portrait loads are lag frames"). The
//! fixtures are the runner tests' own.

use super::*;
use crate::dialogue::tests::delay;

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
            vec![text("A"), delay(4), portrait(1), text("B")],
            2,
        ),
        (
            "delay, swap",
            vec![portrait(2), text("A"), delay(4), portrait(1), text("B")],
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

#[test]
fn controls_after_a_portrait_wait_for_its_frames() {
    // `RunText` reads one byte at a time: a `$F2` or a `$F6` after a `$F4`
    // runs once the portrait's frames are spent, in stream order.
    let mut runner = DialogueRunner::new();
    let mut game = GameState::new();
    let segments = vec![set_flag(1), portrait(1), set_flag(2), text("A")];
    assert!(runner.open_resolved(&entry(segments), &game));
    let flag = |game: &GameState, n: u16| game.is_set(Flag::event(n));
    let mut frames = Vec::new();
    for _ in 0..8 {
        let (lag, _) = lag_step(&mut runner, &mut game, Pad::NEUTRAL);
        frames.push((lag, flag(&game, 1), flag(&game, 2)));
    }
    let last_lag = frames
        .iter()
        .rposition(|frame| frame.0)
        .expect("lag frames");
    assert_eq!(
        lag_count(&frames.iter().map(|f| (f.0, None, 0)).collect::<Vec<_>>()),
        2
    );
    assert!(frames.iter().all(|frame| frame.1), "{frames:?}");
    assert!(
        frames[..=last_lag].iter().all(|frame| !frame.2),
        "the second flag ran before the portrait's frames: {frames:?}"
    );
    assert!(
        frames[last_lag + 1..].iter().any(|frame| frame.2),
        "{frames:?}"
    );

    // An event after it closes the window only once the frames are spent.
    let mut runner = DialogueRunner::new();
    let segments = vec![text("A"), portrait(1), event(5)];
    assert!(runner.open_resolved(&entry(segments), &game));
    let mut lags = 0;
    for _ in 0..20 {
        let (lag, _) = lag_step(&mut runner, &mut game, Pad::NEUTRAL);
        lags += usize::from(lag);
        if runner.view().is_none() {
            break;
        }
    }
    assert!(runner.view().is_none(), "the event never closed the window");
    assert_eq!(lags, 2, "the event discarded the portrait's frames");
    assert_eq!(runner.take_event(), Some(5));
}

#[test]
fn a_hide_costs_no_press_and_a_load_under_a_prompt_starts_as_on_a_page() {
    // `$F4 0` before the terminator: one press, on the frame after the last
    // glyph, closes the window.
    let mut runner = DialogueRunner::new();
    let mut game = GameState::new();
    assert!(runner.open_resolved(&entry(vec![text("A"), portrait(0)]), &game));
    type_to(&mut runner, &mut game, 1);
    let signals = frame(&mut runner, &mut game, Pad::new(Button::Speak));
    assert!(
        signals
            .iter()
            .any(|signal| matches!(signal, DialogueSignal::Closed { .. })),
        "the press was spent on the hide: {signals:?}"
    );

    // The board's text closes itself on the same frame as without the hide.
    let close_frame = |segments: Vec<Segment>| {
        let mut runner = DialogueRunner::new();
        runner.set_board_text(true);
        let mut game = GameState::new();
        assert!(runner.open_resolved(&entry(segments), &game));
        (0..20)
            .position(|_| {
                frame(&mut runner, &mut game, Pad::NEUTRAL)
                    .iter()
                    .any(|signal| matches!(signal, DialogueSignal::Closed { .. }))
            })
            .expect("the board page closes")
    };
    assert_eq!(
        close_frame(vec![text("Hi"), portrait(0)]),
        close_frame(vec![text("Hi")])
    );

    // A load before a prompt lags on the same frames as before a `$FD`.
    let timeline = |end: Segment| {
        let mut runner = DialogueRunner::new();
        let mut game = GameState::new();
        let segments = vec![text("Ok"), portrait(1), end, text("Yes")];
        assert!(runner.open_resolved(&entry(segments), &game));
        trace(&mut runner, &mut game, [Pad::NEUTRAL; 12])
    };
    assert_eq!(timeline(yes_no()), timeline(wait()));
}

#[test]
fn a_step_due_at_a_portrait_after_a_delay_is_kept() {
    // Speak held: a `$F9` running out into `$F4 0` draws the next glyph on
    // the same frame as with no `$F4` at all.
    let timeline = |segments: Vec<Segment>| {
        let mut runner = DialogueRunner::new();
        let mut game = GameState::new();
        assert!(runner.open_resolved(&entry(segments), &game));
        trace(&mut runner, &mut game, [Pad::new(Button::Speak); 12])
            .into_iter()
            .map(|frame| frame.2)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        timeline(vec![text("A"), delay(4), portrait(0), text("B")]),
        timeline(vec![text("A"), delay(4), text("B")]),
    );

    // A load or swap the delay runs into: the step was due with Speak held;
    // released during the lag, it still draws first thing after the frames.
    for (name, segments) in [
        ("load", vec![text("A"), delay(4), portrait(1), text("B")]),
        (
            "swap",
            vec![portrait(2), text("A"), delay(4), portrait(1), text("B")],
        ),
    ] {
        let mut runner = DialogueRunner::new();
        let mut game = GameState::new();
        assert!(runner.open_resolved(&entry(segments), &game));
        type_to(&mut runner, &mut game, 1);
        let mut pad = Pad::new(Button::Speak);
        let mut seen = Vec::new();
        for _ in 0..20 {
            let (lag, _) = lag_step(&mut runner, &mut game, pad);
            if lag {
                pad = Pad::NEUTRAL;
            }
            seen.push((lag, runner.view().expect("a window").revealed));
        }
        let last_lag = seen.iter().rposition(|frame| frame.0).expect("lag frames");
        let first_b = seen.iter().position(|frame| frame.1 == 2).expect("\"B\"");
        assert_eq!(first_b, last_lag + 1, "{name}: {seen:?}");
        // The frames spent at the portrait: its own, after the frame the
        // delay ran out on.
        let at = seen.iter().position(|frame| frame.0).unwrap();
        assert_eq!(last_lag + 1 - at, 2, "{name}: {seen:?}");
    }
}

#[test]
fn the_board_keeps_a_portrait_first_shown_as_its_page_completes() {
    for first in [vec![text("Hi"), portrait(1)], vec![portrait(1), text("A")]] {
        let mut runner = DialogueRunner::new();
        runner.set_board_text(true);
        let mut game = GameState::new();
        assert!(runner.open_resolved(&entry(first.clone()), &game));
        for _ in 0..20 {
            if frame(&mut runner, &mut game, Pad::NEUTRAL)
                .iter()
                .any(|signal| matches!(signal, DialogueSignal::Closed { .. }))
            {
                break;
            }
        }
        assert!(runner.open_resolved(&entry(vec![text("Yo")]), &game));
        frame(&mut runner, &mut game, Pad::NEUTRAL);
        assert_eq!(
            runner.view().expect("the next text").portrait,
            Some(1),
            "{first:?}"
        );
        // A later explicit change replaces it.
        idle(&mut runner, &mut game, 10);
        assert!(runner.open_resolved(&entry(vec![portrait(2), text("C")]), &game));
        idle(&mut runner, &mut game, 10);
        assert_eq!(runner.view().expect("a window").portrait, Some(2));
    }
}

#[test]
fn a_hide_keeps_what_the_screen_shows_until_the_next_glyph() {
    // `$F4 1, $F4 0`: portrait 1 never reaches the screen.
    let mut runner = DialogueRunner::new();
    let mut game = GameState::new();
    assert!(runner.open_resolved(&entry(vec![portrait(1), portrait(0), text("A")]), &game));
    let frames = trace(&mut runner, &mut game, [Pad::NEUTRAL; 8]);
    assert!(frames.iter().all(|frame| frame.1.is_none()), "{frames:?}");
    assert_eq!(frames.last().map(|frame| frame.2), Some(1), "{frames:?}");

    // Portrait 5 is up: two hides, adjacent or apart by a `$F2`, leave it on
    // screen until "A" is drawn.
    for between in [None, Some(set_flag(3))] {
        let mut runner = DialogueRunner::new();
        let mut game = GameState::new();
        let mut segments = vec![portrait(5), text("X"), wait(), portrait(0)];
        segments.extend(between.clone());
        segments.extend([portrait(0), text("A")]);
        assert!(runner.open_resolved(&entry(segments), &game));
        for _ in 0..40 {
            if runner.view().is_some_and(|view| view.dismissable) {
                break;
            }
            frame(&mut runner, &mut game, Pad::NEUTRAL);
        }
        let mut frames = trace(&mut runner, &mut game, [Pad::new(Button::Speak)]);
        frames.extend(trace(&mut runner, &mut game, [Pad::NEUTRAL; 4]));
        let first_a = frames.iter().position(|frame| frame.2 == 1).expect("\"A\"");
        assert!(
            frames[..first_a].iter().all(|frame| frame.1 == Some(5)),
            "{between:?}: {frames:?}"
        );
        assert_eq!(frames[first_a].1, None, "{between:?}: {frames:?}");
    }
}
