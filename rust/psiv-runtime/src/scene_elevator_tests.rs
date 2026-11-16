//! #658: ordinary input into a ride, actual fade clocks and DMA boundaries.
use super::*;
use psiv_core::{EventIndex, Scene, SceneOp, scene_for};

#[derive(Debug)]
pub(super) struct Ride {
    fades: Vec<(usize, SceneOp)>,
    writes: Vec<(usize, u16)>,
    load: usize,
    end: usize,
    out_frames: usize,
    in_frames: usize,
}

impl Ride {
    pub(super) fn chunks(&self) -> Vec<u16> {
        self.writes.iter().map(|&(_, chunk)| chunk).collect()
    }

    pub(super) fn clock_matches(&self) -> bool {
        let [(out, SceneOp::FadeOut), (fade_in, SceneOp::FadeIn)] = self.fades[..] else {
            return false;
        };
        self.writes.len() == 5
            && self.load - out == self.out_frames
            // One map handshake, then the caller's one arrival DMA frame.
            && self.writes[0].0 == self.load + 1
            && fade_in == self.writes[0].0 + 1
            && self.writes[1].0 - fade_in == self.in_frames + usize::from(StepFrames::DEFAULT.get())
            && self.writes[1..].windows(2).all(|pair| pair[1].0 == pair[0].0 + 1)
            && self.end == self.writes[4].0 + 1
    }
}

fn largest(words: &[u16; 64]) -> usize {
    words
        .iter()
        .flat_map(|word| [word & 15, (word >> 4) & 15, (word >> 8) & 15])
        .max()
        .unwrap() as usize
}

/// The optional scene is an explicit mutation fixture. Movement into the
/// trigger and all map/palette work still use the production input/runtime.
pub(super) fn ride(rt: &mut Runtime, candidate: Option<&'static [SceneOp]>) -> Ride {
    let before = rt.game.snapshot();
    let mut started = false;
    let source = rt.map_id();
    let mut result = Ride {
        fades: vec![],
        writes: vec![],
        load: 0,
        end: 0,
        out_frames: 0,
        in_frames: 0,
    };
    for tick in 0..250 {
        let words = rt.cram().words;
        let events = checked_tick(
            rt,
            if started {
                Input::Neutral
            } else {
                Input::Direction(Direction::Up)
            },
        );
        if events
            .iter()
            .any(|event| matches!(event, RuntimeEvent::SceneStarted { trigger: 0x0D }))
        {
            started = true;
            if let Some(ops) = candidate {
                let original = scene_for(EventIndex(0x14)).unwrap();
                let candidate = Box::leak(Box::new(Scene { ops, ..*original }));
                assert!(rt.install_runner(candidate, candidate.event));
            }
        }
        for event in &events {
            if let RuntimeEvent::ScenePresentation {
                op: op @ (SceneOp::FadeOut | SceneOp::FadeIn),
                ..
            } = event
            {
                result.fades.push((tick, *op));
                if *op == SceneOp::FadeOut {
                    result.out_frames = largest(&words).max(1);
                } else {
                    result.in_frames = largest(&words) + 2;
                }
            }
        }
        if events
            .iter()
            .any(|event| matches!(event, RuntimeEvent::MapChanged { .. }))
        {
            result.load = tick;
        }
        if rt.map_id() != source && events.contains(&RuntimeEvent::MapRefreshed) {
            result
                .writes
                .push((tick, rt.effects.chunk_patches.last().unwrap().2));
        }
        if started && !rt.scene_active() {
            result.end = tick;
            assert_eq!(
                rt.game.snapshot(),
                before,
                "ride changes no persistent state"
            );
            assert!(rt.cram().display);
            assert_ne!(rt.cram().words, [0; 64]);
            assert!(rt.cram().missing.is_empty());
            eprintln!("elevator: {result:?}");
            return result;
        }
    }
    panic!("elevator never finished: {result:?}");
}

#[test]
fn an_added_fade_length_wait_double_counts_frames_even_with_the_same_map_outcome() {
    if !pack_present() {
        return;
    }
    let mut rt = fixture(0xA4, Cell::new(32, 18));
    open(&mut rt);
    let scene = psiv_core::SCENES
        .iter()
        .find(|scene| scene.event.0 == 0x14)
        .unwrap();
    let mut altered = scene.ops.to_vec();
    altered.insert(2, SceneOp::WaitFrames { frames: 14 });
    let result = ride(&mut rt, Some(Box::leak(altered.into_boxed_slice())));
    assert!(
        !result.clock_matches(),
        "double-counted palette frames must fail the clock"
    );
    let out = result.fades[0].0;
    assert_eq!(result.load - out, result.out_frames + 14);
    assert_eq!(rt.map_id(), MapId(0xA6));
    assert_eq!(rt.state().cell(), Cell::new(32, 12));
}
