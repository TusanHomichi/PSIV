//! `RunText3` in the runner (`text3.rs`): the ending's windows type a glyph a
//! frame, read no pad, draw no arrow, and turn each page `9n + 1` frames
//! after its last glyph's.
//!
//! Tape 38 (`Cutscene_Ending` by fixture) is the reference: its RAM log of
//! `$FFFFED5B` (the page's glyph count, cleared at the turn) and
//! `Main_Frame_Count` gives each page of entry `$09`'s last glyph and turn.
//! The fixture is entry `$09`'s shape (its controls, in order, and each
//! run's glyph count), not its text.

use super::*;
use crate::dialogue::tests::{close, delay, newline, portrait};

/// `count` glyphs of page `page`'s letter: a page's runs share one, so a
/// page that grows (a `$F2` released mid-page) is told from the next.
fn glyphs(page: char, count: usize) -> Segment {
    text(&page.to_string().repeat(count))
}

fn action(action: ActionKind, action_id: u8, operands: Vec<u8>) -> Segment {
    let sound = (action == ActionKind::LoadSound).then(|| operands[1]);
    let panel = (action == ActionKind::LoadPanel)
        .then(|| u16::from(operands[1]) << 8 | u16::from(operands[2]));
    Segment::Control(Ctrl::Action {
        code: 0xF2,
        operands,
        action,
        action_id,
        sound,
        panel,
        flag: None,
    })
}

fn load_sound(id: u8) -> Segment {
    action(ActionKind::LoadSound, 3, vec![3, id])
}

fn load_panel(id: u16) -> Segment {
    let [high, low] = id.to_be_bytes();
    action(ActionKind::LoadPanel, 0, vec![0, high, low])
}

fn elsydeon_broken() -> Segment {
    action(ActionKind::ElsydeonBroken, 12, vec![12])
}

/// Entry `$09` of tree 42, `Cutscene_Ending`'s first window, by shape.
fn ending_intro() -> DialogueEntry {
    entry(vec![
        load_panel(0x141),
        portrait(1),
        glyphs('a', 4),
        load_sound(0xAD),
        glyphs('a', 17),
        wait(),
        load_sound(0xAD),
        portrait(4),
        glyphs('b', 17),
        newline(),
        load_sound(0xAD),
        glyphs('b', 26),
        wait(),
        glyphs('c', 8),
        wait(),
        portrait(8),
        load_sound(0xAD),
        glyphs('d', 20),
        wait(),
        glyphs('e', 20),
        newline(),
        load_sound(0xAD),
        glyphs('e', 15),
        wait(),
        load_sound(0xAD),
        portrait(6),
        glyphs('f', 28),
        wait(),
        load_panel(0x142),
        portrait(1),
        glyphs('g', 11),
        delay(99),
        elsydeon_broken(),
        load_sound(0xE6),
        load_panel(0x143),
        delay(5),
        load_panel(0x144),
        wait(),
        load_panel(0x145),
        glyphs('h', 15),
        wait(),
        portrait(4),
        glyphs('i', 21),
        wait(),
        load_panel(0x146),
        portrait(6),
        glyphs('j', 17),
        delay(29),
        wait(),
        portrait(1),
        glyphs('k', 16),
    ])
}

/// One page as the runner showed it: its glyphs, and the counted frames from
/// the one that drew its last glyph to the one that turned it (or closed the
/// window).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Turn {
    glyphs: usize,
    frames: u32,
}

fn turn(glyphs: usize, frames: u32) -> Turn {
    Turn { glyphs, frames }
}

/// Opens `entry` as the ending opens its windows: a scene window running
/// `RunText3`. The scene paths select both from the window kind
/// (`Runtime::open_scene_dialogue`); this fixture has no scene.
fn open_text3(runner: &mut DialogueRunner, entry: &DialogueEntry, game: &GameState) {
    assert!(runner.open_resolved(entry, game));
    runner.scene_dialogue = true;
    runner.set_text3(true);
}

/// Runs a `RunText3` window to its close with `pad` every frame, counting
/// only frames that are not lag frames (`Main_Frame_Count`'s count).
fn run_text3(entry: &DialogueEntry, pad: Pad) -> (Vec<Turn>, Vec<DialogueSignal>) {
    let mut runner = DialogueRunner::new();
    let mut game = GameState::new();
    open_text3(&mut runner, entry, &game);
    let mut turns = Vec::new();
    let mut signals = Vec::new();
    let mut counted = 0_u32;
    let mut page: Option<(Vec<String>, usize)> = None;
    let mut last_glyph: Option<u32> = None;
    for _ in 0..10_000 {
        if !runner.lag_frame() {
            counted += 1;
        }
        signals.extend(frame(&mut runner, &mut game, pad));
        let view = runner.view();
        if let Some(view) = view.as_ref() {
            assert!(!view.waiting, "RunText3 draws no arrow: {view:?}");
            assert!(!view.dismissable, "RunText3 reads no press: {view:?}");
        }
        // The page turned when what is up no longer extends it: the
        // window closed, or the next page's letter replaced its own.
        let turned = |lines: &Vec<String>| {
            view.as_ref().is_none_or(|view| {
                view.lines.len() < lines.len()
                    || view
                        .lines
                        .iter()
                        .zip(lines)
                        .any(|(now, was)| !now.starts_with(was.as_str()))
            })
        };
        if let (Some((lines, total)), Some(last)) = (page.as_ref(), last_glyph)
            && turned(lines)
        {
            turns.push(Turn {
                glyphs: *total,
                frames: counted - last,
            });
            last_glyph = None;
        }
        let Some(view) = view else {
            return (turns, signals);
        };
        // A `$F2` released mid-page lets the page grow: its last glyph is
        // the one that leaves nothing more to type.
        if view.revealed < view.total {
            last_glyph = None;
        } else if last_glyph.is_none() && view.total > 0 {
            last_glyph = Some(counted);
        }
        page = Some((view.lines, view.total));
    }
    panic!("the window never closed: {turns:?}");
}

/// Tape 38, entry `$09`: each page's glyphs, and `Main_Frame_Count` from the
/// frame that drew its last glyph to the one whose `$FFFFED5B` reads zero.
/// (Frames 7280 -> 7471, 7519 -> 7908, 7916 -> 7990, 8014 -> 8196,
/// 8232 -> 8549, 8581 -> 8835, 8864 -> 9127, 9157 -> 9294, 9318 -> 9509,
/// 9544 -> 9728 and 9747 -> 9893.)
const TAPE_38: [(usize, u32); 11] = [
    (21, 191),
    (43, 389),
    (8, 74),
    (20, 182),
    (35, 317),
    (28, 254),
    (11, 248),
    (15, 137),
    (21, 191),
    (17, 184),
    (16, 146),
];

/// Frames the cartridge spends inside the seventh page that the runner does
/// not: after the Elsydeon break, `TextAction_LoadSound`'s `VInt_Prepare` and
/// `TextAction_LoadPanel`'s `DMAPlanes_VInt`, before its `$F9 5`. A `$F2`
/// costs no frame natively, in `RunText` and `RunText3` alike (#667). (The
/// second `TextAction_LoadPanel`, after the `$F9`, costs none here either:
/// the runner releases an action the frame after a `$F9` runs out, which is
/// where the cartridge's VInt falls.)
const ACTION_FRAMES: [u32; 11] = [0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0];

#[test]
fn every_page_of_the_endings_first_window_turns_on_tape_38s_frame() {
    let (turns, signals) = run_text3(&ending_intro(), Pad::NEUTRAL);
    let expected: Vec<Turn> = TAPE_38
        .iter()
        .zip(ACTION_FRAMES)
        .map(|(&(glyphs, frames), unspent)| turn(glyphs, frames - unspent))
        .collect();
    assert_eq!(turns, expected);
    // The terminator returns to the scene as `$F7` does, its cursor saved.
    assert!(
        signals.contains(&DialogueSignal::Closed { suspended: true }),
        "the entry's end closes the window by itself: {signals:?}"
    );
}

#[test]
fn a_runtext3_page_waits_nine_frames_a_glyph_plus_one() {
    // `9n + 1` frames of wait after the last glyph's own frame: the page
    // turns on the `9n + 2`nd. `$F7` ends the window the same way, its cursor
    // saved for the scene.
    let (turns, signals) = run_text3(
        &entry(vec![glyphs('a', 3), wait(), glyphs('b', 1), close()]),
        Pad::NEUTRAL,
    );
    assert_eq!(turns, [turn(3, 29), turn(1, 11)]);
    assert!(signals.contains(&DialogueSignal::Closed { suspended: true }));
}

#[test]
fn a_page_its_last_glyph_filled_waits_from_that_glyphs_frame() {
    // The 64th glyph branches to `Text3CtrlCode_Interrupt` before its
    // `.wait`, so the page turns one frame sooner than one ended by `$FD`.
    let (turns, _) = run_text3(&entry(vec![glyphs('a', 64), glyphs('b', 2)]), Pad::NEUTRAL);
    assert_eq!(turns, [turn(64, 9 * 64 + 1), turn(2, 20)]);
}

#[test]
fn speak_neither_speeds_nor_turns_a_runtext3_page() {
    // The negative control: the same window under a held Speak, and under a
    // press every other frame, runs frame for frame as it does untouched.
    let neutral = run_text3(&ending_intro(), Pad::NEUTRAL).0;
    assert_eq!(
        run_text3(&ending_intro(), Pad::new(Button::Speak)).0,
        neutral
    );
    let mut runner = DialogueRunner::new();
    let mut game = GameState::new();
    open_text3(&mut runner, &ending_intro(), &game);
    let mut tapped = DialogueRunner::new();
    let mut tapped_game = GameState::new();
    open_text3(&mut tapped, &ending_intro(), &tapped_game);
    for index in 0..4_000 {
        frame(&mut runner, &mut game, Pad::NEUTRAL);
        let pad = if index % 2 == 0 {
            Pad::new(Button::Speak)
        } else {
            Pad::NEUTRAL
        };
        frame(&mut tapped, &mut tapped_game, pad);
        assert_eq!(tapped.view(), runner.view(), "frame {index}");
    }
}

#[test]
fn a_runtext_page_still_waits_for_speak() {
    // The other negative control: the same entry through `RunText` stops at
    // its first page with the arrow up until a press.
    let mut runner = DialogueRunner::new();
    let mut game = GameState::new();
    assert!(runner.open_resolved(&ending_intro(), &game));
    idle(&mut runner, &mut game, 2_000);
    let view = runner.view().expect("the window waits");
    assert_eq!((view.total, view.revealed), (21, 21));
    assert!(view.waiting && view.dismissable);
}
