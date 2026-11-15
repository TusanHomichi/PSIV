//! `Event_EclipseTorchUsed` after its dismount, frame by frame (#593): the
//! panel's DMA frame and 120 map-update frames, two white flashes, the nine `$338`
//! creations and the 80 main-update frames, then dialogue `$2A`, the `$9C`
//! flag and the return.
//!
//! The US bytes, `$070226..$070481` (`ps4.asm:149579-149717`, `revision=1`):
//!
//! - `$070238`: `DoMapUpdateLoop($77)`, 120 frames of map updates.
//! - Each flash: Deban (`$07023E`, `$070280`), `Palette_Table_Buffer` saved
//!   to `Palette_Table_Buffer_2` (`move.w #$3F, d7` / `trap #1`), the live
//!   buffer filled with `$0EEE` (`loc_7025A`, `loc_7029C`), then
//!   `DoMapUpdateLoop(1)` (`$070266`, `$0702A8`), two frames of map updates,
//!   and forty passes of `loc_58666` and `VInt_Prepare` (`$070270`,
//!   `$0702B2`), forty frames with no object or map update.
//! - `$0702C2`: `Panel_Destroy` and `DMAPlanes_VInt`; the art at `$1D712C`.
//! - `$0702E4..$07041D`: nine `$338` objects written to their slots, with no
//!   frame between them; `DMAPlane_B_VInt`'s frame at `$07043C`.
//! - `$070442`: `SFXID_EnemyKilled`; `$07044C`: `DoMainUpdatesLoop($4F)`,
//!   80 frames of object and map updates.
//! - `$070452..$07045E`: `DialogueTree37`, dialogue `$2A`, then `$9C`.
//!
//! A native `Wait` runs map updates and no field-object pass (#470): the
//! main-update frames' object pass is a deviation recorded in
//! `docs/source-notes/README.md` ("Eclipse Torch").

mod common;

use common::map_with;
use psiv_core::{
    ActorRef, Cell, CharId, Direction, GameState, PaletteBuffer, PaletteSource, PresentationOp,
    SceneEffect, SceneInput, SceneOp, SceneRunner, ScriptedActor, StepFrames,
};

fn torch() -> &'static [SceneOp] {
    psiv_core::SCENES
        .iter()
        .find(|s| s.name == "Event_EclipseTorchUsed")
        .expect("registered")
        .ops
}

/// One frame's record: its effects in order, then `map` for a frame of map
/// updates, `frame` for one with none, `end` when the scene finished on it.
fn frame_line(effects: &[SceneEffect], runner: &SceneRunner) -> String {
    let mut parts = Vec::new();
    for effect in effects {
        let part = match effect {
            SceneEffect::Presentation { op } => match op {
                SceneOp::PlaySound { id } => format!("sound {id:02X}"),
                SceneOp::PanelCreate { id } => format!("panel {id:X}"),
                SceneOp::PanelDestroyLast => "unpanel".to_owned(),
                SceneOp::CreateFieldObject {
                    slot,
                    object_id,
                    art_tile,
                    x,
                    y,
                } => format!("{object_id:X}/{art_tile:X} {slot}@{x:X},{y:X}"),
                SceneOp::ObjectAnimation {
                    slot,
                    object_id,
                    frames,
                    ..
                } => format!("{object_id:X} {slot} for {frames}"),
                SceneOp::Presentation { op } => match op {
                    PresentationOp::PaletteCopy {
                        source: PaletteSource::Buffer(PaletteBuffer::Live),
                        destination: PaletteBuffer::Saved,
                        start: 0,
                        words: 64,
                    } => "save".to_owned(),
                    PresentationOp::PaletteCopy {
                        source: PaletteSource::Fill(0x0EEE),
                        destination: PaletteBuffer::Live,
                        start: 0,
                        words: 64,
                    } => "white".to_owned(),
                    PresentationOp::PaletteRestoreTone { frames } => format!("restore {frames}"),
                    other => format!("{other:?}"),
                },
                _ => continue,
            },
            SceneEffect::DialogueOpen(id) => format!("dialogue {:02X}", id.0),
            SceneEffect::FlagChanged { flag, .. } => format!("flag {:02X}", flag.id),
            SceneEffect::Faulted(fault) => format!("fault {fault:?}"),
            _ => continue,
        };
        parts.push(part);
    }
    let end = if runner.is_finished() {
        "end"
    } else if runner.runs_map_updates() {
        "map"
    } else {
        "frame"
    };
    parts.push(end.to_owned());
    parts.join(", ")
}

/// Runs `ops` on foot from a two-member party, closing each dialogue on the
/// next frame, to the end; the frames as runs of equal lines, and the event
/// flags set.
fn run(ops: &'static [SceneOp]) -> (Vec<(String, usize)>, Vec<u16>) {
    let map = map_with(&["....", "....", "...."], vec![], vec![]);
    let mut state = GameState::new();
    state.set_party([Some(CharId(0)), Some(CharId(1)), None, None, None]);
    let cast = (0..2)
        .map(|slot| {
            ScriptedActor::new(
                ActorRef::PartyMember(slot),
                Cell::new(10, 20),
                Direction::Up,
            )
        })
        .collect();
    let mut runner = SceneRunner::new(ops, cast, StepFrames::new(8).unwrap());
    let mut input = SceneInput::None;
    let mut runs: Vec<(String, usize)> = Vec::new();
    let mut flags = Vec::new();
    for _ in 0..2_000 {
        if runner.is_finished() {
            break;
        }
        let effects = runner.tick(&map, &mut state, input);
        for effect in &effects {
            if let SceneEffect::FlagChanged { flag, value: true } = effect {
                flags.push(flag.id);
            }
        }
        input = if effects
            .iter()
            .any(|e| matches!(e, SceneEffect::DialogueOpen(_)))
        {
            SceneInput::DialogueClosed
        } else {
            SceneInput::None
        };
        let line = frame_line(&effects, &runner);
        match runs.last_mut() {
            Some((last, count)) if *last == line => *count += 1,
            _ => runs.push((line, 1)),
        }
    }
    assert!(runner.is_finished(), "the scene ends");
    (runs, flags)
}

/// The frames from the panel on, as runs of equal lines.
fn expected() -> Vec<(String, usize)> {
    let flash = |sound: &str| {
        [
            (format!("{sound}, save, white, map"), 1),
            ("map".to_owned(), 1),
            ("restore 40, frame".to_owned(), 1),
            ("frame".to_owned(), 39),
        ]
    };
    let mut lines = vec![
        // The panel's `DMAPlanes_VInt`, then `$070238`'s 120 map updates.
        ("panel 10D, frame".to_owned(), 1),
        ("map".to_owned(), 120),
    ];
    lines.extend(flash("sound DC"));
    lines.extend(flash("sound DC"));
    let objects = [
        (0, 0xBA0, 0xC0),
        (1, 0xB80, 0xA0),
        (2, 0xBA0, 0xA0),
        (3, 0xBC0, 0xA0),
        (4, 0xB60, 0x80),
        (5, 0xB80, 0x80),
        (6, 0xBA0, 0x80),
        (7, 0xBC0, 0x80),
        (8, 0xBE0, 0x80),
    ]
    .map(|(slot, x, y)| format!("338/3AF {slot}@{x:X},{y:X}"))
    .join(", ");
    // `$0702C2`: the panel destroy's DMA, then the nine creations on
    // `DMAPlane_B_VInt`'s frame; the sound with the first of eighty.
    lines.push(("unpanel, frame".to_owned(), 1));
    lines.push((format!("{objects}, frame"), 1));
    lines.push(("sound B9, map".to_owned(), 1));
    lines.push(("map".to_owned(), 79));
    lines.push(("dialogue 2A, frame".to_owned(), 1));
    lines.push(("flag 9C, end".to_owned(), 1));
    lines
}

/// The frames that differ, or `Ok`.
fn check(ops: &'static [SceneOp]) -> Result<Vec<u16>, String> {
    let (runs, flags) = run(ops);
    let want = expected();
    if runs == want {
        return Ok(flags);
    }
    let first = runs
        .iter()
        .zip(&want)
        .position(|(got, want)| got != want)
        .unwrap_or(runs.len().min(want.len()));
    Err(format!(
        "run {first}: {:?}, expected {:?}",
        runs.get(first),
        want.get(first)
    ))
}

#[test]
fn the_eclipse_torch_flashes_twice_then_runs_eighty_main_update_frames() {
    let flags = check(&torch()[9..]).unwrap();
    assert_eq!(flags, [0x9C]);
    // Two panel DMA frames, 120 + 2 * (2 + 40), the object DMA frame,
    // then 80 map updates, the dialogue and the end.
    let (runs, _) = run(&torch()[9..]);
    let frames: usize = runs.iter().map(|(_, count)| count).sum();
    assert_eq!(frames, 2 + 120 + 2 * 42 + 1 + 80 + 2);
}

/// The scene's ops before #593, and each repair undone alone, fail the
/// frame check: the missing second flash, the restore as 40 map-update
/// ticks, the creations' recorded frame each, and the missing main loop.
#[test]
fn control_the_old_flash_and_spawn_timing_fail_the_frame_check() {
    let torch = &torch()[9..];
    let at = |pick: &dyn Fn(&SceneOp) -> bool| torch.iter().position(pick).unwrap();
    let main_loop = at(&|op| matches!(op, SceneOp::Wait { ticks: 80 }));
    let objects = at(&|op| matches!(op, SceneOp::CreateFieldObject { .. }));
    let leak = |ops: Vec<SceneOp>| -> &'static [SceneOp] { Box::leak(ops.into_boxed_slice()) };

    // Main's ops before #593: the panel, 120 ticks, Deban, 40 ticks, the
    // panel gone, the art, nine one-frame `ObjectAnimation`s, no main loop.
    let mut old = torch[..3].to_vec();
    old.push(SceneOp::PlaySound { id: 0xDC });
    old.push(SceneOp::Wait { ticks: 40 });
    old.extend_from_slice(&torch[objects - 3..objects]);
    old.extend((0..9).map(|slot| SceneOp::ObjectAnimation {
        slot,
        object_id: 0x338,
        art_tile: 0x3AF,
        frames: 1,
    }));
    old.extend_from_slice(&torch[main_loop + 1..]);
    assert_eq!(
        check(leak(old)).unwrap_err(),
        r#"run 2: Some(("sound DC, map", 1)), expected Some(("sound DC, save, white, map", 1))"#
    );

    // No second flash: its six ops gone.
    let second = at(&|op| matches!(op, SceneOp::PlaySound { id: 0xDC })) + 6;
    assert!(matches!(torch[second], SceneOp::PlaySound { id: 0xDC }));
    let mut one_flash = torch.to_vec();
    one_flash.drain(second..second + 6);
    let err = check(leak(one_flash)).unwrap_err();
    assert!(err.starts_with("run 6: Some((\"unpanel, "), "{err}");

    // The first restore's frames as map-update ticks.
    let mut map_restore = torch.to_vec();
    map_restore[second - 1] = SceneOp::Wait { ticks: 40 };
    assert_eq!(
        check(leak(map_restore)).unwrap_err(),
        r#"run 4: Some(("restore 40, map", 1)), expected Some(("restore 40, frame", 1))"#
    );

    // A frame after each creation.
    let mut spawn_frames = Vec::new();
    for (index, op) in torch.iter().enumerate() {
        spawn_frames.push(*op);
        if (objects..objects + 9).contains(&index) {
            spawn_frames.push(SceneOp::WaitFrames { frames: 1 });
        }
    }
    let err = check(leak(spawn_frames)).unwrap_err();
    assert!(
        err.starts_with("run 11: Some((\"338/3AF 0@BA0,C0, frame\""),
        "{err}"
    );

    // The main loop missing, or as many frames without map updates.
    let mut no_loop = torch.to_vec();
    no_loop.remove(main_loop);
    let err = check(leak(no_loop)).unwrap_err();
    assert!(
        err.starts_with("run 12: ") && err.contains("sound B9, dialogue 2A, frame\", 1"),
        "{err}"
    );
    let mut bare_loop = torch.to_vec();
    bare_loop[main_loop] = SceneOp::WaitFrames { frames: 80 };
    let err = check(leak(bare_loop)).unwrap_err();
    assert!(
        err.starts_with("run 12: ") && err.contains("sound B9, frame"),
        "{err}"
    );
}
