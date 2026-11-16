//! The elevator's DMA waits are independent of its palette ramps (#658).
use super::*;

pub(super) fn problems(rom: &[u8], ops: &[SceneOp]) -> Vec<String> {
    let mut errors = Vec::new();
    for (at, target) in [
        (0x06C392, DMA_HELPERS[0]), // arrival, before Pal_FadeIn
        (0x06C3DE, DMA_HELPERS[1]), // movement loop
        (0x06C44E, DMA_HELPERS[1]), // closing, foreground collision
        (0x06C45C, DMA_HELPERS[2]), // closing, background collision
    ] {
        if jsr_target(rom, at) != Some(target) {
            errors.push(format!("elevator: DMA at ${at:06X} changed"));
        }
    }
    // DBF d0 branches to itself: a CPU delay after the DMA's VBlank.
    if rom[0x06C466..0x06C46A] != [0x51, 0xC8, 0xFF, 0xFE] {
        errors.push("elevator: CPU-only DBF changed".into());
    }
    if ops.len() != 17
        || ops.get(1) != Some(&SceneOp::FadeOut)
        || ops.get(2) != Some(&SceneOp::TakeMapTransition)
        || ops.get(4) != Some(&SceneOp::WaitFrames { frames: 1 })
        || ops.get(5) != Some(&SceneOp::FadeIn)
        || !matches!(
            ops.get(6),
            Some(SceneOp::MoveActorOffset {
                actor: ActorRef::PartyMember(0),
                dx: 0,
                dy: 16,
                wait: true
            })
        )
        || ops.get(7) != Some(&SceneOp::OverlapCharacters)
    {
        errors.push("elevator: arrival DMA/movement must spend their own frames once".into());
    }
    for row in 0..4 {
        let at = 0x06C46E + row * 2;
        let op = 9 + row * 2;
        let chunk = u16::from(rom[at + 1]);
        // $01 is the CPU delay, not a DBF over DMA: still one frame.
        if rom[at] != 1
            || !matches!(ops.get(op), Some(SceneOp::WriteActorMapChunks {
                actor: ActorRef::PartyMember(0), chunks,
            }) if *chunks == [(0, -16, chunk)])
            || ops.get(op + 1) != Some(&SceneOp::WaitFrames { frames: 1 })
        {
            errors.push(format!(
                "elevator: closing row {row} must spend one DMA frame"
            ));
        }
    }
    if rom[0x06C476] != 0xFF {
        errors.push("elevator: door table terminator changed".into());
    }
    errors
}

#[test]
fn added_fade_waits_and_doubled_dma_frames_are_rejected() {
    let Some(rom) = crate::scenes::dialogue_callers_image::rom() else {
        return;
    };
    let ops = crate::scenes::bioplant::ELEVATOR_RIDE.ops;
    assert!(problems(rom, ops).is_empty());
    for frames in [1, 14, 16] {
        let mut doubled = ops.to_vec();
        doubled.insert(2, SceneOp::WaitFrames { frames });
        assert!(!problems(rom, &doubled).is_empty());
    }
    for index in [4, 10, 12, 14, 16] {
        let mut doubled = ops.to_vec();
        doubled[index] = SceneOp::WaitFrames { frames: 2 };
        assert!(!problems(rom, &doubled).is_empty(), "wait {index}");
    }
}
