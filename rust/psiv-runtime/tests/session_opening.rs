//! The opening, headless, through `Session::frame` with pads only.
//!
//! The native opening smoke plays START, the opening chain (which dispatches
//! trigger 124 and Chaz's `$6D` line), and an eight-frame `Up`, and finds the
//! cartridge's first control: map `$13`, standing cell `(48,18)`, Chaz alone,
//! no active scene, 500 meseta and town flags `80008040`
//! (`docs/campaign/PLAYABILITY_FOUNDATIONS.md`, "Evidence"). This case reaches
//! the same state the same way a player does: one [`Pad`] per frame, Speak
//! presses to dismiss the message boxes, then `Up`.
//!
//! It is also the negative control's twin: with the pads left neutral the
//! opening's box never closes, so the scene never hands control back.

use std::path::Path;

use psiv_core::{Cell, CharId, Flag, StepFrames};
use psiv_data::GameData;
use psiv_runtime::{
    Button, NpcDialogueOpen, Pad, Routed, Runtime, RuntimeEvent, SceneDialogueOpen, Session,
};

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

/// Frames between the pads' dialogue presses: a fresh press every fourth frame,
/// with the released frames in between — the shape the oracle tapes use (a held
/// button never advances a finished page).
const PRESS_PERIOD: u64 = 4;

/// The frames the native smoke holds `Up` for: one 8-frame cell step.
const UP_FRAMES: u32 = 8;

/// A session driven the way the shell drives it: a pad in, the frame's faults
/// checked. The session opens the windows itself — a talk, a scene's line, a
/// choice — so nothing here routes events.
struct Player {
    session: Session,
    ticks: u64,
}

impl Player {
    /// One frame: [`Session::frame`] and a look at what it opened and what went
    /// wrong.
    fn tick(&mut self, pad: Pad) {
        self.ticks += 1;
        assert!(self.ticks < 20_000, "the opening exceeded its frame budget");
        let frame = self.session.frame(pad);
        for routed in &frame.routed {
            match routed {
                Routed::SceneDialogue {
                    outcome: SceneDialogueOpen::UnknownTree,
                    ..
                } => panic!("scene dialogue tree is absent from the loaded pack"),
                Routed::Talk { outcome, .. } => {
                    assert_eq!(
                        *outcome,
                        NpcDialogueOpen::Opened,
                        "opening talk opened nothing"
                    );
                }
                _ => {}
            }
        }
        for event in &frame.events {
            match event {
                RuntimeEvent::SceneFaulted { .. }
                | RuntimeEvent::SceneMissing { .. }
                | RuntimeEvent::SceneBattleFailed { .. }
                | RuntimeEvent::MapRefreshFailed { .. }
                | RuntimeEvent::UnpackedTarget { .. }
                | RuntimeEvent::WarpUnmapped { .. } => panic!("opening faulted: {event:?}"),
                // Battles are never armed here: the academy rolls no
                // encounters, so a rolled one would be a pack change.
                RuntimeEvent::EncounterRolled { formation } => {
                    panic!("the academy rolled formation {formation:#06x}")
                }
                _ => {}
            }
        }
    }

    /// This frame's pad: a Speak press while the window is ready for one — a
    /// dismissable page, or a choice the cursor can answer YES — and nothing
    /// otherwise.
    fn auto_pad(&self) -> Pad {
        let ready =
            self.session.runtime().dialogue_view().is_some_and(|view| {
                view.dismissable || view.choice.is_some_and(|choice| choice.ready)
            });
        if ready && self.ticks.is_multiple_of(PRESS_PERIOD) {
            Pad::new(Button::Speak)
        } else {
            Pad::NEUTRAL
        }
    }

    fn runtime(&self) -> &Runtime {
        self.session.runtime()
    }
}

/// A new game armed the way the title arms it: the pack's initializer, the
/// dialogue pack, and the title's own opening event.
fn new_game() -> Player {
    let pack = Path::new(PACK);
    let data = GameData::load(pack).expect("pack loads");
    let event = data.new_game().expect("title initializer").event_index;
    // No second step: the loaded data carries the dialogue pack, so the
    // runtime can open a window the moment it exists.
    let mut runtime = Runtime::new_game(data, StepFrames::default()).expect("START");
    assert!(runtime.start_event(event), "the opening event starts");
    Player {
        session: Session::new(runtime),
        ticks: 0,
    }
}

fn pack_present() -> bool {
    Path::new(PACK).join("manifest.json").is_file()
}

/// The defect this follow-up fixes, pinned from the runtime's own side: a
/// runtime built by the title's START has to be able to open the opening's
/// first scene dialogue with no other setup. It could not before — the pack
/// arrived through a separate `load_dialogue` step that START and CONTINUE
/// never called, so `open_scene_dialogue` answered `UnknownTree` and the scene
/// waited forever for a window that never opened (the black screen).
///
/// The negative control lives beside the runner
/// (`src/dialogue/glue.rs`, `stripping_the_pack_leaves_the_scene_dialogue_unresolved`),
/// where the pack can be taken away from a runtime to show the same call then
/// fails.
#[test]
fn a_new_game_runtime_opens_the_openings_first_scene_dialogue() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let pack = Path::new(PACK);
    let data = GameData::load(pack).expect("pack loads");
    let event = data.new_game().expect("title initializer").event_index;
    // The title's own construction, and nothing else: no dialogue step.
    let mut runtime = Runtime::new_game(data, StepFrames::default()).expect("START");
    assert!(runtime.start_event(event), "the opening event starts");

    // The first scene dialogue the opening asks for, found the way the shell
    // finds it: tick until the scene raises one.
    let entry = loop {
        if let Some(RuntimeEvent::SceneDialogue { entry }) = runtime
            .tick(psiv_core::Input::Neutral)
            .into_iter()
            .find(|event| matches!(event, RuntimeEvent::SceneDialogue { .. }))
        {
            break entry;
        }
    };
    assert_eq!(
        runtime.open_scene_dialogue(entry, false),
        SceneDialogueOpen::Opened,
        "the loaded pack opens the opening's entry {entry:#04x}"
    );
    assert!(runtime.dialogue_open(), "a window is up");
    let view = runtime.dialogue_view().expect("the window has a view");
    assert!(
        view.lines.iter().any(|line| !line.is_empty()),
        "the box has text: {:?}",
        view.lines
    );
}

#[test]
fn the_opening_reaches_first_control_through_pads() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let expected = {
        let data = GameData::load(Path::new(PACK)).expect("pack loads");
        data.manifest().game_start.clone().expect("first control")
    };
    let mut player = new_game();

    // The opening is a chain of two scenes with a gap between them
    // (`game_start.json`'s `scene_chain`: `0x9F` then `0xA0`), so "no scene is
    // running" is not the end of it — the second scene's own flag is.
    while player.runtime().scene_active() || !player.runtime().game().is_set(Flag::event(21)) {
        let pad = player.auto_pad();
        player.tick(pad);
    }
    assert_eq!(
        player.runtime().map_id().0,
        expected.map.id,
        "first control is on the pack's map"
    );
    assert_eq!(
        player.runtime().state().cell(),
        Cell::new(expected.x_cell as u16, expected.y_cell as u16),
        "first control is on the pack's placement — one row south of the \
         cartridge's standing cell, which the pack's own note explains"
    );
    assert_eq!(
        player.runtime().game().party_members(),
        vec![CharId(0)],
        "Chaz stands alone"
    );

    // The native smoke's eight-frame Up: one cell north, to the standing cell
    // the retail oracle recorded (`docs/oracle/RESULTS.md`: 768,288 pixels =
    // cell 48,18).
    for _ in 0..UP_FRAMES {
        player.tick(Pad::new(Button::Up));
    }
    assert_eq!(
        player.runtime().state().cell(),
        Cell::new(48, 18),
        "the first step north is the recorded first-control cell"
    );
    assert!(!player.runtime().scene_active(), "no scene owns the party");
    assert_eq!(
        player.runtime().game().party_members(),
        vec![CharId(0)],
        "still Chaz alone"
    );
    assert_eq!(
        player.runtime().game().money(),
        500,
        "the initializer's meseta"
    );
    assert!(
        player.runtime().game().is_set(Flag::event(21)),
        "Piata control"
    );
    let snapshot = player.runtime().game().snapshot();
    assert_eq!(
        &snapshot.town_flags[..4],
        &[0x80, 0x00, 0x80, 0x40],
        "the town flags the opening set"
    );
}

/// The negative control: the same opening, the same frames, no Speak press.
/// The message box never closes, so the scene never releases the party.
#[test]
fn without_a_speak_press_the_opening_never_releases_control() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let mut player = new_game();
    for _ in 0..2_000 {
        player.tick(Pad::NEUTRAL);
    }
    assert!(
        player.runtime().scene_active(),
        "the scene still waits for its dialogue to be dismissed"
    );
    assert!(
        player.runtime().dialogue_open(),
        "the box is still up after 2000 frames of neutral pads"
    );
    assert_eq!(
        player.runtime().game().party_members(),
        vec![CharId(0), CharId(1)],
        "the opening's own cast, not first control"
    );
}
