use super::*;
use crate::{
    CollisionGrid, FieldMap, GameState, MapId, SceneEffect, SceneInput, SceneRunner, StepFrames,
};

fn scene(name: &str) -> &'static [SceneOp] {
    super::super::SCENES
        .iter()
        .find(|s| s.name == name)
        .unwrap()
        .ops
}

fn map() -> FieldMap {
    FieldMap::new(
        MapId(0),
        CollisionGrid::filled(8, 8, 0).unwrap(),
        vec![],
        vec![],
    )
    .unwrap()
}

/// Run the consuming span, counting only held frames, not its completion
/// tick. Every VInt-only phase must leave objects and map updates stopped.
fn run(ops: &'static [SceneOp], frames: usize, map_updates: bool) -> Vec<(usize, SceneEffect)> {
    let map = map();
    let mut state = GameState::new();
    let mut runner = SceneRunner::new(ops, vec![], StepFrames::default());
    let mut events = Vec::new();
    for frame in 0..=frames {
        let effects = runner.tick(&map, &mut state, SceneInput::None);
        assert!(
            !effects.iter().any(|e| matches!(e, SceneEffect::Faulted(_))),
            "{effects:?}"
        );
        events.extend(effects.into_iter().map(|effect| (frame, effect)));
        assert_eq!(runner.is_finished(), frame == frames, "frame {frame}");
        if frame < frames {
            assert_eq!(runner.runs_map_updates(), map_updates, "frame {frame}");
        }
    }
    events
}

#[test]
fn existing_waits_spend_the_dma_frames_once() {
    let rune = scene("Event_RuneFlaeli");
    let events = run(&rune[21..32], 240, false);
    assert_eq!(
        events
            .iter()
            .filter_map(|(f, e)| matches!(e, SceneEffect::MapChunksRestored { .. }).then_some(*f))
            .collect::<Vec<_>>(),
        [106]
    );

    let machine = scene("Event_MachineCenterAppearing");
    let events = run(&machine[5..8], 369, true);
    assert_eq!(
        events
            .iter()
            .filter_map(|(f, e)| matches!(e, SceneEffect::MapChunksWritten { .. }).then_some(*f))
            .collect::<Vec<_>>(),
        [308]
    );

    run(&scene("Event_EclipseTorchUsed")[38..39], 1, false);
}

#[test]
fn wreckage_repeats_dma_plus_three_vints_for_each_of_eight_writes() {
    let events = run(&scene("Event_WreckageEngine")[23..39], 32, false);
    let writes: Vec<_> = events
        .iter()
        .filter_map(|(f, e)| match e {
            SceneEffect::MapChunksWritten { chunks } => Some((*f, chunks.len())),
            _ => None,
        })
        .collect();
    assert_eq!(writes, (0..8).map(|i| (i * 4, 4)).collect::<Vec<_>>());
}

#[test]
fn juza_spends_all_eight_dma_delays_before_setting_the_flag() {
    let events = run(scene("Event_JuzaDefeated"), 56, false);
    let writes: Vec<_> = events
        .iter()
        .filter_map(|(f, e)| match e {
            SceneEffect::MapChunksWritten { .. } => Some((*f, false)),
            SceneEffect::MapChunksRestored { .. } => Some((*f, true)),
            _ => None,
        })
        .collect();
    assert_eq!(
        writes,
        (0..8).map(|i| (i * 7, i % 2 == 1)).collect::<Vec<_>>()
    );
    assert!(events.iter().any(|(f, e)| *f == 56
        && *e
            == SceneEffect::FlagChanged {
                flag: crate::Flag::event(0x48),
                value: true,
            }));
}

#[test]
fn refresh_gaps_are_explicitly_546_owned_without_invented_cpu_costs() {
    for gap in REFRESH_GAPS {
        assert_eq!(gap.issue, 546);
        let ops = scene(gap.scene);
        // The current op is immediate. Its missing boundary is deliberately
        // unimplemented here, rather than claimed as an existing consumer.
        run(&ops[gap.op..gap.op + 1], 0, false);
    }
}

#[test]
fn census_rejects_missing_doubled_and_wrong_policy_consumers() {
    let Some(rom) = super::super::rom() else {
        return;
    };
    for (name, indices) in [
        ("Event_RuneFlaeli", &[23, 27, 29, 31][..]),
        ("Event_MachineCenterAppearing", &[5, 7][..]),
        ("Event_EclipseTorchUsed", &[38][..]),
        (
            "Event_WreckageEngine",
            &[24, 26, 28, 30, 32, 34, 36, 38][..],
        ),
        ("Event_JuzaDefeated", &[2, 4, 6, 8, 10, 12, 14, 16][..]),
    ] {
        let ops = scene(name);
        assert!(problems(rom, name, ops).is_empty());
        for &index in indices {
            for delta in [-1_i32, 1] {
                let mut changed = ops.to_vec();
                changed[index] = match changed[index] {
                    SceneOp::WaitFrames { frames } => SceneOp::WaitFrames {
                        frames: (i32::from(frames) + delta) as u16,
                    },
                    SceneOp::Wait { ticks } => SceneOp::Wait {
                        ticks: (i32::from(ticks) + delta) as u16,
                    },
                    _ => unreachable!(),
                };
                assert!(
                    !problems(rom, name, &changed).is_empty(),
                    "{name} op {index} delta {delta}"
                );
            }
            let mut changed = ops.to_vec();
            changed[index] = match changed[index] {
                SceneOp::WaitFrames { frames } => SceneOp::Wait { ticks: frames },
                SceneOp::Wait { ticks } => SceneOp::WaitFrames { frames: ticks },
                _ => unreachable!(),
            };
            assert!(
                !problems(rom, name, &changed).is_empty(),
                "{name} op {index} policy"
            );
            let mut changed = ops.to_vec();
            changed.insert(index, SceneOp::DmaPlanes);
            assert!(
                !problems(rom, name, &changed).is_empty(),
                "{name} op {index} duplicate DMA"
            );
        }
    }
    for gap in REFRESH_GAPS {
        let mut changed = scene(gap.scene).to_vec();
        changed.insert(gap.op + 1, SceneOp::WaitFrames { frames: 1 });
        assert!(!problems(rom, gap.scene, &changed).is_empty());
        let mut changed = rom.to_vec();
        changed[gap.dma + 2..gap.dma + 6].copy_from_slice(&VINT_PREPARE.to_be_bytes());
        assert!(!problems(&changed, gap.scene, scene(gap.scene)).is_empty());
    }
    // The original Juza collapse (one restore, no delays) is rejected.
    let juza = scene("Event_JuzaDefeated");
    assert!(
        !problems(
            rom,
            "Event_JuzaDefeated",
            &[juza[0].clone(), juza[15].clone(), juza[17].clone()],
        )
        .is_empty()
    );
    // A wrong DBF repeat count or a lost bare VInt invalidates Wreckage's
    // four-frame assignment, even if the native waits remain unchanged.
    for at in [0x06_DD07, 0x06_DD49] {
        let mut changed = rom.to_vec();
        changed[at] ^= 1;
        assert!(
            !problems(
                &changed,
                "Event_WreckageEngine",
                scene("Event_WreckageEngine")
            )
            .is_empty()
        );
    }
}
