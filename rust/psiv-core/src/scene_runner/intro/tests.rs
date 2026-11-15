use super::*;
use crate::{CollisionGrid, FieldMap, GameState, MapId, SceneInput, StepFrames};

fn map() -> FieldMap {
    FieldMap::new(
        MapId(0),
        CollisionGrid::filled(1, 1, 0).unwrap(),
        vec![],
        vec![],
    )
    .unwrap()
}

fn trace(ops: Vec<SceneOp>, phase: u16) -> Vec<Vec<SceneOp>> {
    let mut runner = SceneRunner::new(
        Box::leak(ops.into_boxed_slice()),
        vec![],
        StepFrames::default(),
    );
    let mut state = GameState::new();
    let mut trace = Vec::new();
    for tick in 0..3000 {
        runner.set_main_frame_count(phase.wrapping_add(tick));
        let effects = runner.tick(&map(), &mut state, SceneInput::EndingContinue);
        assert!(
            !runner.runs_map_updates(),
            "intro frame {tick} ran map updates"
        );
        trace.push(
            effects
                .into_iter()
                .filter_map(|e| match e {
                    SceneEffect::Presentation { op } => Some(op),
                    SceneEffect::Faulted(fault) => panic!("{fault:?}"),
                    _ => None,
                })
                .collect(),
        );
        if runner.is_finished() {
            return trace;
        }
    }
    panic!("intro exceeded frame budget");
}

#[test]
fn intro_helpers_spend_each_pass_and_the_floor_dma_on_its_own_frame() {
    for phase in [0, 1, 2, 3, 0xFFFE] {
        for (op, colour, frames, clears) in [
            (SceneOp::IntroTextFadeUp, 0x0666, 20, 0),
            (
                SceneOp::IntroTextFadeDown,
                0x0EEE,
                if phase & 3 == 0 { 22 } else { 21 },
                if phase & 3 == 0 { 2 } else { 1 },
            ),
            (
                SceneOp::IntroTextFadeDown,
                0x0666,
                if matches!(phase & 3, 1 | 2) { 26 } else { 27 },
                if matches!(phase & 3, 1 | 2) { 6 } else { 7 },
            ),
        ] {
            let got = trace(vec![SceneOp::SetTextColour { colour }, op], phase);
            assert_eq!(got.len(), frames + 1, "{op:?} phase {phase}");
            let mut word = colour;
            let mut passes = 0;
            let mut dma_count = 0;
            let mut after_dma = false;
            for (t, ops) in got[..frames].iter().enumerate() {
                let mut want = Vec::new();
                if t == 0 {
                    want.extend([SceneOp::SetTextColour { colour }, op]);
                }
                if after_dma {
                    after_dma = false;
                } else if phase.wrapping_add(t as u16) & 3 == 0 {
                    if op == SceneOp::IntroTextFadeDown && word == 0x0666 {
                        want.extend([
                            SceneOp::Presentation {
                                op: PresentationOp::ClearPlanes,
                            },
                            SceneOp::DmaPlanes,
                        ]);
                        after_dma = true;
                        dma_count += 1;
                    } else {
                        let next = if op == SceneOp::IntroTextFadeUp {
                            word + 0x222
                        } else {
                            word - 0x222
                        };
                        if next & 0x1000 == 0 {
                            word = next;
                            want.push(SceneOp::SetTextColour { colour: word });
                        }
                    }
                }
                if !after_dma {
                    want.push(SceneOp::WaitFrames { frames: 1 });
                    passes += 1;
                }
                assert_eq!(*ops, want, "{op:?}, phase {phase}, frame {t}");
            }
            assert_eq!(passes, 20);
            assert_eq!(dma_count, clears);
            assert!(got[frames].is_empty(), "release is after the last VInt");
        }
    }
}

fn pages() -> Vec<SceneOp> {
    let ops = crate::scenes::SCENES
        .iter()
        .find(|s| s.name == "Event_GameStart")
        .unwrap()
        .ops;
    let first = ops
        .iter()
        .position(|op| matches!(op, SceneOp::SetTextColour { .. }))
        .unwrap();
    let last = first
        + ops[first..]
            .iter()
            .position(|op| *op == SceneOp::FadeOut)
            .unwrap();
    ops[first..last].to_vec()
}

fn page_problem(ops: Vec<SceneOp>) -> bool {
    let got = trace(ops, 0);
    let draws: Vec<_> = got
        .iter()
        .enumerate()
        .flat_map(|(t, ops)| {
            ops.iter().filter_map(move |op| {
                if let SceneOp::DrawTextToPlane { entry, plane, vram } = *op {
                    Some((t, entry, plane, vram))
                } else {
                    None
                }
            })
        })
        .collect();
    // First page, phase zero: 3 bare waits + DMA + 20 up + 900 hold +
    // 22 down (two floor DMAs) + 90 pause. Four draws remain.
    // Second page differs only in the closing pause (120).
    let mut expected = Vec::new();
    for (base, entry) in [(0, 0x3A), (1036, 0x3E)] {
        for row in 0..4 {
            expected.push((
                base + row,
                entry + row as u16,
                0x840A + row as u32 * 0x180,
                0x4200 + row as u32 * 0x80,
            ));
        }
    }
    draws != expected
        || got.len() != 2103
        || [3, 1039]
            .iter()
            .any(|&t| !got[t].contains(&SceneOp::DmaPlanes))
        || [4, 1040]
            .iter()
            .any(|&t| !got[t].contains(&SceneOp::IntroTextFadeUp))
        || [24, 1060]
            .iter()
            .any(|&t| got[t] != [SceneOp::WaitFrames { frames: 900 }])
        || [924, 1960]
            .iter()
            .any(|&t| !got[t].contains(&SceneOp::IntroTextFadeDown))
        || got[946] != [SceneOp::WaitFrames { frames: 90 }]
        || got[1982] != [SceneOp::WaitFrames { frames: 120 }]
}

#[test]
fn both_prologue_pages_preserve_draws_holds_and_frame_boundaries() {
    assert!(!page_problem(pages()));
}

#[test]
fn prologue_guard_rejects_omitted_and_duplicated_dma_frames() {
    let mut omitted = pages();
    let dma = omitted
        .iter()
        .position(|op| *op == SceneOp::DmaPlanes)
        .unwrap();
    omitted.remove(dma);
    assert!(page_problem(omitted));
    let mut doubled = pages();
    doubled.insert(dma, SceneOp::DmaPlanes);
    assert!(page_problem(doubled));
}
