//! #658: actual CRAM clocks and persistent outcomes, on isolated pack fixtures.
//! The guardians' unmodeled tone loops remain #574; no retail-duration claim
//! is made for their current placeholder wait/object animation.
use super::*;
use psiv_core::{EventIndex, Scene, scene_for};

fn has(frame: &Frame, wanted: SceneOp) -> bool {
    frame.events.iter().any(|event| {
        matches!(event,
        RuntimeEvent::ScenePresentation { op, .. } if *op == wanted)
    })
}

fn at(frames: &[Frame], from: usize, wanted: SceneOp) -> usize {
    from + frames[from..]
        .iter()
        .position(|frame| has(frame, wanted))
        .expect("op emitted")
}

/// Fixed routines step the raw four-bit component (not the visible 3-bit
/// level) once a frame; black still takes one Out pass. In adds one display
/// frame and a final unchanged pass. This expectation does not call the model.
fn largest(words: &[u16; 64]) -> usize {
    words
        .iter()
        .flat_map(|word| [word & 15, (word >> 4) & 15, (word >> 8) & 15])
        .max()
        .unwrap() as usize
}

#[test]
fn lashiec_fades_at_the_explosion_planet_and_map_boundaries() {
    if !pack_present() {
        return;
    }
    let mut runtime = runtime_on(
        0x174,
        [Some(CharId(0)), None, None, None, None],
        &[Flag::event(0x9B), Flag::event(0xA6)],
    );
    let before = runtime.game().snapshot();
    let (base, frames) = trace(&mut runtime, 0x8016);
    let events: Vec<_> = frames
        .iter()
        .flat_map(|frame| frame.events.clone())
        .collect();
    use SceneOp::{FadeIn as In, FadeOut as Out, InitVramAndCram as Init};
    super::super::fixed_fades::assert_fades(
        &events,
        &[Init, In, Out, Init, In, Out, Init, In, Out, In],
    );
    let raw_outs: Vec<_> = frames
        .iter()
        .enumerate()
        .filter_map(|(tick, frame)| has(frame, Out).then_some(tick))
        .collect();
    for &start in &raw_outs {
        let source = if start == 0 {
            base
        } else {
            frames[start - 1].words
        };
        let count = largest(&source).max(1);
        assert_eq!(
            frames[start + count - 1].words,
            [0; 64],
            "Out at {start} ends black"
        );
        if count > 1 {
            assert_ne!(
                frames[start + count - 2].words,
                [0; 64],
                "Out at {start} spent every pass"
            );
        }
    }
    let explosion = at(&frames, 0, SceneOp::PanelCreate { id: 0x10A });
    let in_at = at(&frames, explosion, In);
    assert_eq!(in_at - explosion, 1, "panel DMA precedes explosion In");
    let source = frames[in_at - 1].words;
    let count = largest(&source) + 2;
    let resume = frames
        .iter()
        .position(|frame| frame.events.contains(&RuntimeEvent::SceneDialogueResume))
        .unwrap();
    assert_eq!(
        resume - in_at,
        count + 240,
        "fade frames plus the independent 240-frame hold"
    );
    let out = at(&frames, resume, Out);
    let third_init = at(&frames, out, Init);
    let planet = at(&frames, third_init, SceneOp::FlightPlanet);
    assert!(resume < out && out < third_init && third_init < planet);
    let planet_in = at(&frames, planet, In);
    let planet_out = at(&frames, planet_in, Out);
    let target = frames[planet_in - 1].words;
    assert_eq!(planet_out - planet_in, largest(&target) + 2 + 300);
    let load = frames
        .iter()
        .position(|frame| {
            frame
                .events
                .iter()
                .any(|event| matches!(event, RuntimeEvent::MapChanged { .. }))
        })
        .unwrap();
    assert_eq!(
        load - planet_out,
        largest(&frames[planet_out - 1].words).max(1)
    );
    let final_in = at(&frames, load, In);
    assert!(planet_out < load && load < final_in);
    assert!(runtime.cram().display);
    assert_eq!(runtime.map_id().0, 0x162);
    assert_eq!(runtime.world_index(), 1);
    assert!(runtime.game().inventory().slots().contains(&0x8E));
    let after = runtime.game().snapshot();
    assert_eq!(before.event_flags, after.event_flags);
    assert_eq!(before.party, after.party);
    eprintln!(
        "Lashiec: {} frames; explosion In {in_at}, Run5 {resume}, Out {out}, Init {third_init}, planet {planet_in}..{planet_out}, map {load}, final In {final_in}",
        frames.len()
    );
}

fn guardian(
    event: u16,
    map: u16,
    defeated: u16,
    ops: Option<&'static [SceneOp]>,
) -> (Vec<Frame>, Runtime) {
    let mut runtime = runtime_on(map, [Some(CharId(0)), None, None, None, None], &[]);
    let mut expected = GameState::from_snapshot(&runtime.game().snapshot());
    expected.set(Flag::event(defeated)).unwrap();
    // The optional candidate is an explicit negative-control scene fixture.
    if let Some(ops) = ops {
        let original = scene_for(EventIndex(event)).unwrap();
        let candidate = Box::leak(Box::new(Scene { ops, ..*original }));
        assert!(runtime.install_runner(candidate, candidate.event));
    }
    let (_, frames) = if ops.is_none() {
        trace(&mut runtime, event)
    } else {
        trace_running(&mut runtime)
    };
    assert_eq!(runtime.map_id().0, map);
    assert_eq!(runtime.game().snapshot(), expected.snapshot());
    (frames, runtime)
}

#[test]
fn guardians_keep_the_field_palette_and_only_set_their_defeat_flags() {
    if !pack_present() {
        return;
    }
    for (event, map, flag) in [(0x64, 0xF6, 0xE5), (0x65, 0xFB, 0xE6)] {
        let (frames, runtime) = guardian(event, map, flag, None);
        // Current placeholder budget, not the complete retail tone-loop time
        // (#574): 40 wait frames plus dialogue/runner handoffs.
        assert_eq!(frames.len(), 43);
        assert!(
            frames
                .iter()
                .all(|frame| frame.words == runtime.cram().words)
        );
        assert!(!frames.iter().any(|frame| has(frame, SceneOp::FadeOut)));
        eprintln!(
            "guardian {event:#x}: {} modeled frames, no fixed fade",
            frames.len()
        );
        let scene = psiv_core::SCENES
            .iter()
            .find(|scene| scene.event.0 == event)
            .unwrap();
        let mut altered = scene.ops.to_vec();
        altered.insert(1, SceneOp::FadeOut);
        let (wrong, _) = guardian(
            event,
            map,
            flag,
            Some(Box::leak(altered.into_boxed_slice())),
        );
        assert!(wrong.iter().any(|frame| frame.words == [0; 64]));
        assert_eq!(
            wrong.len() - frames.len(),
            largest(&frames[0].words).max(1),
            "restored fade adds exactly its palette frames"
        );
    }
}
