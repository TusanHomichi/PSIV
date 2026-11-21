//! Zio's fixed fades in execution order, among the visible scene boundaries.
//! The image census pins call sites; this trace also rejects a correct fade
//! sequence placed on the wrong side of a map, dialogue or party change.

use super::post_rika_cutscenes::ZIO_DEFEATED;
use crate::collision::CollisionGrid;
use crate::geom::{Cell, Direction};
use crate::map::{FieldMap, MapId};
use crate::scene::{ActorRef, SceneEffect, SceneInput, SceneOp, ScriptedActor};
use crate::scene_runner::SceneRunner;
use crate::state::{CharId, GameState};
use crate::{DialogueWindow, PresentationOp};

fn trace(ops: &'static [SceneOp]) -> Vec<String> {
    let map = FieldMap::new(
        MapId(0xD3),
        CollisionGrid::filled(64, 64, 0).unwrap(),
        vec![],
        vec![],
    )
    .unwrap();
    let mut state = GameState::new();
    state.set_party([
        Some(CharId(0)),
        Some(CharId(4)),
        Some(CharId(6)),
        Some(CharId(5)),
        Some(CharId(3)),
    ]);
    let actors = (0..5)
        .map(|slot| {
            ScriptedActor::new(
                ActorRef::PartyMember(slot),
                Cell::new(30, 29),
                Direction::Down,
            )
        })
        .collect();
    let mut runner = SceneRunner::new(ops, actors, crate::StepFrames::default());
    let mut input = SceneInput::None;
    let mut steps = Vec::new();
    for _ in 0..5_000 {
        if runner.awaits_palette() {
            runner.release_palette(1);
        }
        let effects = runner.tick(&map, &mut state, input);
        input = SceneInput::None;
        for effect in effects {
            match effect {
                SceneEffect::DialogueOpen(_) | SceneEffect::DialogueResume => {
                    steps.push("dialogue".into());
                    input = SceneInput::DialogueClosed;
                }
                SceneEffect::MapRequested {
                    op:
                        SceneOp::LoadMap {
                            start_x, start_y, ..
                        },
                } => {
                    steps.push(format!("map {start_x:X},{start_y:X}"));
                    input = SceneInput::MapLoaded;
                }
                SceneEffect::PartyChanged => steps.push("remove party".into()),
                SceneEffect::MapChunksWritten { chunks } => {
                    if chunks == [(26, 45, 0x3F)] {
                        steps.push("spaceport".into());
                    } else {
                        assert_eq!(chunks.len(), 3);
                        assert!(chunks.iter().all(|&(x, y, id)| {
                            (14..=16).contains(&x) && y == 12 && (0xAD..=0xC0).contains(&id)
                        }));
                    }
                }
                SceneEffect::Presentation { op } => match op {
                    SceneOp::PanCamera { .. } => input = SceneInput::CameraArrived,
                    SceneOp::FadeOut => steps.push("out".into()),
                    SceneOp::FadeIn => steps.push("in".into()),
                    SceneOp::InitVramAndCram => {
                        // Init owns its internal fade-out, not another scene call.
                        assert_eq!(steps.pop().as_deref(), Some("out"));
                        steps.push("init".into());
                    }
                    SceneOp::PanelCreate { id } => steps.push(format!("panel {id:X}")),
                    SceneOp::Presentation {
                        op: PresentationOp::ReloadMapChunks,
                    } => steps.push("chunks".into()),
                    SceneOp::SetDialogueTree { rom_addr: 0x1F9020 } => steps.push("tree 36".into()),
                    _ => {}
                },
                SceneEffect::Faulted(fault) => panic!("Zio faulted: {fault:?}"),
                _ => {}
            }
        }
        if runner.is_finished() {
            assert_eq!(state.party_members(), [CharId(0), CharId(5), CharId(3)]);
            return steps;
        }
    }
    panic!("Zio did not finish: {steps:?}");
}

fn expected() -> Vec<String> {
    [
        "init",
        "in",
        "panel 6D",
        "dialogue",
        "chunks",
        "out",
        "map 3D,38",
        "in",
        "dialogue",
        "dialogue",
        "dialogue",
        "out",
        "init",
        "in",
        "panel 70",
        "dialogue",
        "map 70,BC",
        "in",
        "spaceport",
        "out",
        "init",
        "in",
        "tree 36",
        "init",
        "in",
        "panel 73",
        "panel 74",
        "panel 75",
        "dialogue",
        "remove party",
        "remove party",
        "map 6C,B8",
        "in",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

#[test]
fn zio_fixed_fades_run_around_dialogue_maps_and_party_removals() {
    assert_eq!(trace(ZIO_DEFEATED.ops), expected());
    let ops = ZIO_DEFEATED.ops;
    assert!(bioplant_boundary(ops));
    // $075D84's tree load separates the two init/in pairs, before panel $73.
    let tree = ops
        .iter()
        .position(|op| *op == SceneOp::SetDialogueTree { rom_addr: 0x1F9020 })
        .unwrap();
    assert_eq!(
        &ops[tree - 3..tree + 4],
        &[
            SceneOp::FadeOut,
            SceneOp::InitVramAndCram,
            SceneOp::FadeIn,
            SceneOp::SetDialogueTree { rom_addr: 0x1F9020 },
            SceneOp::InitVramAndCram,
            SceneOp::FadeIn,
            SceneOp::PanelCreate { id: 0x73 },
        ]
    );
    // Run5 owns its closing fade at $075DF8, before the removals; no scene Out.
    let text = ops
        .iter()
        .rposition(|op| {
            *op == SceneOp::RunDialogueResumeWithWindow {
                window: DialogueWindow::Cutscene5,
            }
        })
        .unwrap();
    assert_eq!(ops[text + 1], SceneOp::RemovePartyMember { who: CharId(4) });
}

#[test]
fn missing_or_misplaced_repaired_fades_change_the_zio_trace() {
    let ops = ZIO_DEFEATED.ops;
    let tree = ops
        .iter()
        .position(|op| matches!(op, SceneOp::SetDialogueTree { .. }))
        .unwrap();
    // The five calls restored at $075D72..$075D96.
    for index in [tree - 3, tree - 2, tree - 1, tree + 1, tree + 2] {
        let mut missing = ops.to_vec();
        missing.remove(index);
        assert_ne!(trace(Box::leak(missing.into_boxed_slice())), expected());
        // Move the same call past the next panel, preserving the fade count.
        let mut moved = ops.to_vec();
        let fade = moved.remove(index);
        let panel = moved
            .iter()
            .position(|op| *op == SceneOp::PanelCreate { id: 0x73 })
            .unwrap();
        moved.insert(panel + 1, fade);
        assert_ne!(trace(Box::leak(moved.into_boxed_slice())), expected());
    }
}

/// $075B10 follows the chunk reload and the full $3B+1 map updates, then
/// refreshes 0xD3 and stages Demi before fading in.
fn bioplant_boundary(ops: &[SceneOp]) -> bool {
    ops.windows(8).any(|span| {
        matches!(
            span,
            [
                SceneOp::Presentation {
                    op: PresentationOp::ReloadMapChunks,
                },
                SceneOp::Wait { ticks: 60 },
                SceneOp::FadeOut,
                SceneOp::DespawnNpc {
                    npc_index: 0,
                    count: 2
                },
                SceneOp::SetMapLoadFlags {
                    set: 0x08,
                    clear: 0
                },
                SceneOp::LoadMap { map: 0xD3, .. },
                SceneOp::PlaceActor { .. },
                SceneOp::FadeIn,
            ]
        )
    })
}

#[test]
fn moving_bioplant_fade_before_the_wait_breaks_its_boundary() {
    let mut ops = ZIO_DEFEATED.ops.to_vec();
    let out = ops.iter().position(|op| *op == SceneOp::FadeOut).unwrap();
    ops.swap(out - 1, out);
    assert!(!bioplant_boundary(&ops));
    // The list of fade kinds is unchanged, so the image order alone misses it.
    assert_eq!(trace(Box::leak(ops.into_boxed_slice())), expected());
}

#[test]
fn moving_the_second_init_in_before_the_tree_changes_the_trace() {
    let mut ops = ZIO_DEFEATED.ops.to_vec();
    let tree = ops
        .iter()
        .position(|op| matches!(op, SceneOp::SetDialogueTree { .. }))
        .unwrap();
    // Move the pair across only the tree load: all fourteen fade kinds
    // remain in the same order, but their retail boundary is lost.
    ops[tree..tree + 3].rotate_left(1);
    assert_ne!(trace(Box::leak(ops.into_boxed_slice())), expected());
}
