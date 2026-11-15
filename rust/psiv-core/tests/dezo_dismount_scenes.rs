//! The mounted transitions of `Event_CarnivorousTrees`, `Event_SavingKyra`
//! and `Event_EclipseTorchUsed`, frame by frame (#592): each frame's sounds,
//! dismount, dialogue and battle, and how the frame ends: a frame of map
//! updates (`DoMapUpdateLoop`), or a bare `VInt_Prepare` frame with none.
//!
//! The US bytes, `ps4.asm:149539-149577` and `:149930-150019`:
//!
//! - Carnivorous Trees: saved music `$99`; on foot `beq.w loc_70826`
//!   (`$07077E`) to the party move; mounted, SFX stop, one map-update frame
//!   (`$07078C`), Red Alert, `Field_BuildSprites` and a bare frame
//!   (`$0707A0`), the dismount, `Field_UpdateObjects` and a frame (`$07080C`),
//!   dialogue `$34`.
//! - Saving Kyra: on foot `beq.w loc_70908` to the Red Alert; mounted, saved
//!   music `$99`, SFX stop, a map-update frame (`$07086E`), music stop `$FB`,
//!   `Field_BuildSprites` and a bare frame (`$070882`), the dismount, objects
//!   and a frame (`$0708EE`), dialogue `$34`.
//! - Eclipse Torch: on foot `beq.w loc_70226` to the panel; mounted, saved
//!   music `$99`, SFX stop, a bare frame (`$0701A0`), music `$99`
//!   (`$0701A6`), `Field_BuildSprites` and a bare frame (`$0701B4`), the
//!   dismount, objects and a frame (`$070220`).
//!
//! No native scene frame runs `Field_UpdateObjects` (#470): the frames after
//! the dismounts are one bare frame each, a deviation recorded in
//! `docs/source-notes/README.md`.

mod common;

use common::map_with;
use psiv_core::{
    ActorRef, Cell, CharId, Direction, GameState, PresentationOp, SceneEffect, SceneInput, SceneOp,
    SceneRunner, ScriptedActor, StepFrames,
};

fn scene(name: &str) -> &'static [SceneOp] {
    psiv_core::SCENES
        .iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("{name} is registered"))
        .ops
}

/// What a run leaves: one line per frame, the vehicle selector, and the
/// event flags the scene set.
#[derive(Debug)]
struct Run {
    frames: Vec<String>,
    vehicle: u16,
    flags: Vec<u16>,
}

/// One frame's record: its effects in order, then how the frame ends:
/// `map` for a frame of map updates, `frame` for one with none, `end` when
/// the scene finished on it.
fn frame_line(effects: &[SceneEffect], runner: &SceneRunner) -> String {
    let mut parts = Vec::new();
    for effect in effects {
        let part = match effect {
            SceneEffect::Presentation { op } => match op {
                SceneOp::PlaySound { id } => format!("sound {id:02X}"),
                SceneOp::SetSavedMusic { id } => format!("saved {id:02X}"),
                SceneOp::Presentation {
                    op: PresentationOp::RebuildSprites,
                } => "sprites".to_owned(),
                SceneOp::PanelCreate { id } => format!("panel {id:X}"),
                _ => continue,
            },
            SceneEffect::VehicleChanged { index } => format!("vehicle {index}"),
            SceneEffect::DialogueOpen(id) => format!("dialogue {:02X}", id.0),
            SceneEffect::BattleRequested { index } => format!("battle {index:02X}"),
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

/// Runs `ops` from a two-member party on cell (10, 20), `mounted` on the Ice
/// Digger or on foot, closing every dialogue on the next frame, for at most
/// `limit` frames, to the scene's end or its battle request.
fn run(ops: &'static [SceneOp], mounted: bool, limit: usize) -> Run {
    let map = map_with(&["....", "....", "...."], vec![], vec![]);
    let mut state = GameState::new();
    state.set_party([Some(CharId(0)), Some(CharId(1)), None, None, None]);
    state.set_vehicle_index(if mounted { 2 } else { 0 });
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
    let mut frames = Vec::new();
    let mut flags = Vec::new();
    while frames.len() < limit && !runner.is_finished() {
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
        frames.push(frame_line(&effects, &runner));
        // The battle owns the frames after its request.
        if effects
            .iter()
            .any(|e| matches!(e, SceneEffect::BattleRequested { .. }))
        {
            break;
        }
    }
    Run {
        frames,
        vehicle: state.vehicle_index(),
        flags,
    }
}

/// The frames that differ, or `Ok` when `ops` runs `expected` exactly.
fn check(ops: &'static [SceneOp], mounted: bool, expected: &[&str]) -> Result<Run, String> {
    let run = run(ops, mounted, expected.len() + 1);
    if run.frames == expected {
        return Ok(run);
    }
    let first = run
        .frames
        .iter()
        .zip(expected)
        .position(|(got, want)| got != want)
        .unwrap_or(run.frames.len().min(expected.len()));
    Err(format!(
        "frame {first}: {:?}, expected {:?}",
        run.frames.get(first),
        expected.get(first)
    ))
}

const TREES_MOUNTED: &[&str] = &[
    // `$07078C`: `DoMapUpdateLoop(0)` after the SFX stop.
    "saved 99, sound FD, map",
    // `$0707A0`: the bare frame after `Field_BuildSprites`.
    "sound A9, sprites, frame",
    // `$07080C`: the frame after the dismount, no map updates.
    "vehicle 0, frame",
    "dialogue 34, frame",
    "battle 0A, frame",
];
/// `beq.w loc_70826`: no dismount, no dialogue, no frame.
const TREES_ON_FOOT: &[&str] = &["saved 99, battle 0A, frame"];

const KYRA_MOUNTED: &[&str] = &[
    "saved 99, sound FD, map",
    "sound FB, sprites, frame",
    "vehicle 0, frame",
    "dialogue 34, frame",
    // `loc_70908`.
    "sound A9, map",
    // `loc_70908`: the panel's `DMAPlanes_VInt` precedes dialogue `$26`.
    "panel 98, frame",
    "dialogue 26, frame",
    "flag 95, battle 0A, frame",
];
/// `beq.w loc_70908`: straight to the Red Alert.
const KYRA_ON_FOOT: &[&str] = &[
    "sound A9, map",
    // `loc_70908`: the panel's `DMAPlanes_VInt` precedes dialogue `$26`.
    "panel 98, frame",
    "dialogue 26, frame",
    "flag 95, battle 0A, frame",
];

/// The mounted transition, to the panel at `loc_70226`.
const TORCH_MOUNTED: &[&str] = &[
    // `$0701A0`.
    "saved 99, sound FD, frame",
    // `$0701A6`, then `$0701B4`.
    "sound 99, sprites, frame",
    // `$070220`.
    "vehicle 0, frame",
];

#[test]
fn carnivorous_trees_dismounts_frame_by_frame_and_skips_it_all_on_foot() {
    let ops = scene("Event_CarnivorousTrees");
    let mounted = check(ops, true, TREES_MOUNTED).unwrap();
    assert_eq!(mounted.vehicle, 0, "the dismount");
    assert!(mounted.flags.is_empty(), "{:x?}", mounted.flags);
    let on_foot = check(ops, false, TREES_ON_FOOT).unwrap();
    assert_eq!(on_foot.vehicle, 0);
    assert!(on_foot.flags.is_empty(), "{:x?}", on_foot.flags);
}

#[test]
fn saving_kyra_dismounts_frame_by_frame_and_keeps_its_on_foot_branch() {
    let ops = scene("Event_SavingKyra");
    let mounted = check(ops, true, KYRA_MOUNTED).unwrap();
    assert_eq!(mounted.vehicle, 0);
    assert_eq!(mounted.flags, [0x95]);
    let on_foot = check(ops, false, KYRA_ON_FOOT).unwrap();
    assert_eq!(on_foot.vehicle, 0);
    assert_eq!(on_foot.flags, [0x95]);
}

/// Negative control for the panel DMA: omitting its `VInt_Prepare` frame
/// opens dialogue `$26` on the panel's frame, before retail does.
#[test]
fn saving_kyra_rejects_a_panel_dma_that_spends_no_frame() {
    let mut ops = scene("Event_SavingKyra").to_vec();
    let dma = ops
        .iter()
        .position(|op| matches!(op, SceneOp::DmaPlanes))
        .unwrap();
    ops[dma] = SceneOp::Wait { ticks: 0 };
    let ops = Box::leak(ops.into_boxed_slice());
    assert_eq!(
        check(ops, false, KYRA_ON_FOOT).unwrap_err(),
        r#"frame 1: Some("panel 98, dialogue 26, frame"), expected Some("panel 98, frame")"#
    );
}

/// Mounted, the three dismount frames come first; after them the scene is
/// the on-foot run, frame for frame, to the `$9C` flag.
#[test]
fn the_eclipse_torch_dismount_plays_the_field_music_then_joins_the_on_foot_run() {
    let ops = scene("Event_EclipseTorchUsed");
    let mounted = run(ops, true, 2_000);
    let on_foot = run(ops, false, 2_000);
    assert_eq!(mounted.frames[..3], *TORCH_MOUNTED);
    assert!(
        mounted.frames[3].starts_with("panel 10D,"),
        "{:?}",
        mounted.frames[3]
    );
    assert_eq!(mounted.frames[3..], on_foot.frames[..]);
    assert_eq!(
        on_foot.frames.last().map(String::as_str),
        Some("flag 9C, end")
    );
    assert_eq!((mounted.vehicle, on_foot.vehicle), (0, 0));
    assert_eq!(mounted.flags, [0x9C]);
    assert_eq!(on_foot.flags, [0x9C]);
}

/// The ops these scenes had before #592, against the same frames: the
/// branch that showed dialogue `$34` on foot, the missing frames, Saving
/// Kyra's mounted entry past its saved music, and Eclipse Torch's missing
/// frames and music; then each Eclipse Torch repair undone alone.
#[test]
fn control_the_old_transitions_fail_their_frame_checks() {
    let trees = scene("Event_CarnivorousTrees");
    // Main's ops before #592: on foot to op 7, the jump both branches took
    // into the tree load and dialogue `$34`; mounted, no bare frames.
    let old_trees: Vec<SceneOp> = [
        trees[0],
        SceneOp::BranchIfVehicle {
            if_mounted: 2,
            if_on_foot: 7,
        },
        trees[2],
        trees[3],
        trees[4],
        trees[5],
        trees[7],
        SceneOp::Jump { to: 8 },
    ]
    .into_iter()
    .chain(trees[9..].iter().copied())
    .collect();
    assert!(matches!(old_trees[7], SceneOp::Jump { to: 8 }));
    assert!(matches!(old_trees[8], SceneOp::SetDialogueTree { .. }));
    assert!(matches!(old_trees[10], SceneOp::PlacePartyY { dy: 0x20 }));
    let old_trees: &'static [SceneOp] = Box::leak(old_trees.into_boxed_slice());
    let on_foot = check(old_trees, false, TREES_ON_FOOT).unwrap_err();
    assert_eq!(
        on_foot,
        r#"frame 0: Some("saved 99, dialogue 34, frame"), expected Some("saved 99, battle 0A, frame")"#
    );
    assert_eq!(
        check(old_trees, true, TREES_MOUNTED).unwrap_err(),
        r#"frame 1: Some("sound A9, sprites, vehicle 0, dialogue 34, frame"), expected Some("sound A9, sprites, frame")"#
    );

    // Saving Kyra's old mounted entry, op 2: no saved music.
    let mut kyra = scene("Event_SavingKyra").to_vec();
    kyra[0] = SceneOp::BranchIfVehicle {
        if_mounted: 2,
        if_on_foot: 11,
    };
    let kyra: &'static [SceneOp] = Box::leak(kyra.into_boxed_slice());
    assert_eq!(
        check(kyra, true, KYRA_MOUNTED).unwrap_err(),
        r#"frame 0: Some("sound FD, map"), expected Some("saved 99, sound FD, map")"#
    );

    // Eclipse Torch's old mounted entry, before the panel at op 9.
    let torch = scene("Event_EclipseTorchUsed");
    let mut old_torch = vec![
        SceneOp::BranchIfVehicle {
            if_mounted: 1,
            if_on_foot: 7,
        },
        SceneOp::SetSavedMusic { id: 0x99 },
        SceneOp::PlaySound { id: 0xFD },
        SceneOp::Presentation {
            op: PresentationOp::RebuildSprites,
        },
        SceneOp::SetVehicleIndex { index: 0 },
        SceneOp::Wait { ticks: 1 },
        SceneOp::Jump { to: 7 },
    ];
    old_torch.extend_from_slice(&torch[9..]);
    let old_torch: &'static [SceneOp] = Box::leak(old_torch.into_boxed_slice());
    let old = run(old_torch, true, 3);
    assert_eq!(
        old.frames,
        [
            "saved 99, sound FD, sprites, vehicle 0, map",
            "panel 10D, frame",
            "map"
        ]
    );
    assert_ne!(old.frames[..], *TORCH_MOUNTED);
    // One repair undone at a time: no `$99` at `$0701A6`, map updates on
    // the object frame at `$070220`, no frame at `$0701B4`.
    let mut no_music = torch.to_vec();
    no_music[4] = SceneOp::Wait { ticks: 0 };
    let no_music: &'static [SceneOp] = Box::leak(no_music.into_boxed_slice());
    assert_eq!(run(no_music, true, 3).frames[1], "sprites, frame");
    let mut map_frame = torch.to_vec();
    map_frame[8] = SceneOp::Wait { ticks: 1 };
    let map_frame: &'static [SceneOp] = Box::leak(map_frame.into_boxed_slice());
    assert_eq!(run(map_frame, true, 3).frames[2], "vehicle 0, map");
    let mut no_frame = torch.to_vec();
    no_frame[6] = SceneOp::Wait { ticks: 0 };
    let no_frame: &'static [SceneOp] = Box::leak(no_frame.into_boxed_slice());
    assert_eq!(
        run(no_frame, true, 3).frames[1],
        "sound 99, sprites, vehicle 0, frame"
    );
}
