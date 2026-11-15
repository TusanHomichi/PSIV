//! #645: `Cutscene_Ending`'s windows run `RunText3` (`dialogue/text3.rs`)
//! through the session, as the shell plays the certify fixture
//! (`PSIV_DEBUG_EVENT=0x8021`): they page and close with no press, and no
//! press changes a frame of them.

use std::path::Path;

use psiv_core::{PresentationOp, SceneOp, StepFrames};
use psiv_data::GameData;

use crate::dialogue::DialogueSignal;
use crate::pad::{Button, Pad};
use crate::{RuntimeEvent, scene_fixture};

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

/// The windows the run waits for: entry `$09` (`EndingIntro`), then `$0A`
/// and `$0B` (`Event_GetAndRunDialogue4`) and the first two
/// `Event_RunDialogue4` resumes (`$079146`, `$07916A`).
const WINDOWS: usize = 5;

/// What one frame showed: the window's page and glyphs, and whether the
/// frame closed a window or started a tone loop.
type Shown = (Option<(Vec<String>, usize)>, bool, bool);

/// Plays `$8021` from its certify fixture with `pad(frame)` until
/// [`WINDOWS`] windows have closed. `None` without the pack.
fn play(pad: impl Fn(usize) -> Pad) -> Option<Vec<Shown>> {
    let dir = Path::new(PACK);
    if !dir.join("manifest.json").is_file() {
        eprintln!("runtime pack not present; skipping");
        return None;
    }
    let data = GameData::load(dir).expect("pack loads");
    let mut session = scene_fixture(data, 0x8021, StepFrames::default())
        .expect("$8021 has a fixture")
        .expect("the fixture builds");
    assert!(session.debug_start_event(0x8021), "the ending starts");
    let mut shown = Vec::new();
    let mut closed = 0;
    for index in 0..30_000 {
        let frame = session.frame(pad(index));
        let view = session.runtime().dialogue_view();
        if let Some(view) = view.as_ref() {
            assert!(
                !view.waiting && !view.dismissable,
                "frame {index}: an ending window drew an arrow or took a press: {view:?}"
            );
        }
        let closes = frame
            .signals
            .iter()
            .chain(&frame.window_signals)
            .any(|signal| matches!(signal, DialogueSignal::Closed { .. }));
        let tone = frame.events.iter().any(|event| {
            matches!(
                event,
                RuntimeEvent::ScenePresentation {
                    op: SceneOp::Presentation {
                        op: PresentationOp::PaletteIncreaseTone { .. }
                    },
                    ..
                }
            )
        });
        closed += usize::from(closes);
        shown.push((view.map(|view| (view.lines, view.revealed)), closes, tone));
        if closed == WINDOWS {
            return Some(shown);
        }
        assert!(
            session.runtime().scene_active(),
            "frame {index}: the ending ended after {closed} windows"
        );
    }
    panic!("only {closed} of the ending's first {WINDOWS} windows closed with no press");
}

#[test]
fn the_endings_windows_page_and_close_with_no_press() {
    let Some(neutral) = play(|_| Pad::NEUTRAL) else {
        return;
    };
    // The first window closes on its own, and the scene's first tone loop
    // (`$078F9C`) runs after it.
    let first_close = neutral.iter().position(|&(_, closes, _)| closes);
    let first_tone = neutral.iter().position(|&(_, _, tone)| tone);
    assert!(
        first_close.is_some_and(|close| first_tone.is_some_and(|tone| close <= tone)),
        "close {first_close:?}, tone {first_tone:?}"
    );
}

#[test]
fn a_press_changes_no_frame_of_the_endings_windows() {
    // The negative control for the shell's inputs: Speak held throughout, and
    // tapped every other frame (each tap an edge), play frame for frame as
    // no press does.
    let Some(neutral) = play(|_| Pad::NEUTRAL) else {
        return;
    };
    let held = play(|_| Pad::new(Button::Speak)).expect("the pack is present");
    assert_eq!(held, neutral);
    let tapped = play(|index| {
        if index % 2 == 0 {
            Pad::new(Button::Speak)
        } else {
            Pad::NEUTRAL
        }
    })
    .expect("the pack is present");
    assert_eq!(tapped, neutral);
}
