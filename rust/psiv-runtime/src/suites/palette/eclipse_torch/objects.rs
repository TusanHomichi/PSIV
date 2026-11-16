//! Isolated object-pass proof. Scene main-update policy remains #470/#573.
use super::*;
use psiv_core::{
    EclipseTorchObject, EclipseTorchSequence, EclipseTorchSpawn, Scene, SceneRunner, StepFrames,
};

type Snapshot = Vec<(EclipseTorchSpawn, Option<u8>, u8)>;

fn snapshot(rt: &Runtime) -> Snapshot {
    rt.eclipse_torch_objects()
        .map(|o| (o.spawn(), o.mapping(), o.duration()))
        .collect()
}

fn spawned() -> Runtime {
    let mut rt = runtime_at(
        0x001,
        (0xBA0, 0xD0),
        [Some(CharId(0)), Some(CharId(1)), None, None, None],
        &[Flag::event(0x9B)],
    );
    assert!(rt.start_event(0x0047));
    for _ in 0..1000 {
        let events = rt.tick(Input::Neutral);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, RuntimeEvent::SceneFaulted { .. }))
        );
        if rt.eclipse_torch_objects().count() == 9 {
            // VInt-only fixture, independent of the missing scene-main policy.
            const PARK: Scene = Scene {
                name: "torch VInt isolation",
                event: psiv_core::EventIndex(0x47),
                ops: &[
                    SceneOp::WaitFrames { frames: 1000 },
                    SceneOp::Return { value: 1 },
                ],
            };
            rt.scene = Some(SceneRunner::new(
                PARK.ops,
                Vec::new(),
                StepFrames::default(),
            ));
            rt.scene_warmup = false;
            return rt;
        }
    }
    panic!("nine torch spawns not reached");
}

fn trace(rt: &mut Runtime) -> Vec<Snapshot> {
    let mut result = vec![snapshot(rt)];
    for _ in 1..=65 {
        rt.tick_field_objects();
        result.push(snapshot(rt));
    }
    result
}

/// Expected values independent of the routine: all passes, not just endpoints.
fn valid_trace(trace: &[Snapshot]) -> bool {
    if trace.len() != 66 {
        return false;
    }
    let positions = [
        (0xBA0, 0xC0),
        (0xB80, 0xA0),
        (0xBA0, 0xA0),
        (0xBC0, 0xA0),
        (0xB60, 0x80),
        (0xB80, 0x80),
        (0xBA0, 0x80),
        (0xBC0, 0x80),
        (0xBE0, 0x80),
    ];
    trace.iter().enumerate().all(|(pass, rows)| {
        let expected: Vec<_> = positions
            .iter()
            .enumerate()
            .filter_map(|(slot, &(x, y))| {
                let (sequence, period) = match slot {
                    0 => (EclipseTorchSequence::Fast, 8),
                    1..=3 => (EclipseTorchSequence::Medium, 12),
                    _ => (EclipseTorchSequence::Slow, 16),
                };
                (pass < period * 4).then_some((
                    EclipseTorchSpawn {
                        slot,
                        x,
                        y,
                        sequence,
                    },
                    (pass > 0).then_some((pass / period) as u8),
                    (period - 1 - pass % period) as u8,
                ))
            })
            .collect();
        *rows == expected
    })
}

#[test]
fn eclipse_torch_runtime_object_api_all_nine_every_pass() {
    if !pack_present() {
        return;
    }
    let mut rt = spawned();
    assert!(valid_trace(&trace(&mut rt)));
}

#[test]
fn eclipse_torch_controls_reject_uniform_delays_and_early_or_late_clear() {
    if !pack_present() {
        return;
    }
    let mut rt = spawned();
    let correct = trace(&mut rt);
    assert!(valid_trace(&correct));
    let mut early = correct.clone();
    early[31].retain(|(spawn, _, _)| spawn.slot != 0);
    assert!(!valid_trace(&early), "one pass early");
    let mut late = correct.clone();
    late[32].insert(0, correct[31][0]);
    assert!(!valid_trace(&late), "one pass late");
    let mut rt = spawned();
    for object in rt.eclipse_torch.values_mut() {
        let mut spawn = object.spawn();
        spawn.sequence = EclipseTorchSequence::Fast;
        *object = EclipseTorchObject::new(spawn);
    }
    let mut uniform = trace(&mut rt);
    // Keep the correct sequence labels: reject the timing, not just metadata.
    for rows in &mut uniform {
        for (spawn, _, _) in rows {
            spawn.sequence = correct[0][spawn.slot].0.sequence;
        }
    }
    assert!(!valid_trace(&uniform));
}

#[test]
fn eclipse_torch_vint_frames_do_not_advance_and_control_detects_an_object_pass() {
    if !pack_present() {
        return;
    }
    let mut rt = spawned();
    for pass in 0..65 {
        let before = snapshot(&rt);
        let frames = rt.frames;
        let maps = rt.map_updates().calls;
        for _ in 0..3 {
            rt.tick(Input::Neutral);
        }
        assert_eq!(rt.frames, frames + 3);
        assert_eq!(rt.map_updates().calls, maps, "VInt only");
        assert_eq!(snapshot(&rt), before, "no object pass at {pass}");
        rt.tick_field_objects();
        if pass < 64 {
            assert_ne!(snapshot(&rt), before, "control: a pass changes state");
        }
    }
}
