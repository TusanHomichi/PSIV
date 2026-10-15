//! Tracing is opt-in, bounded when off, and never changes the sound (#92).

use crate::{DacSample, SAMPLE_RATE, SoundBank, SoundMachine, SoundSequence, SoundTrack};

/// A music record whose only track turns DAC looping on (`$E4 01`) and starts
/// sample `$81` (`$EE 81`): the DAC then writes YM `$2A` at its pitch rate
/// for as long as the machine renders, the densest trace the driver makes.
fn looping_dac_bank() -> SoundBank {
    let music = SoundSequence::new(
        0x81,
        1,
        vec![crate::FmVoice::fixture()],
        Vec::new(),
        vec![SoundTrack::fm(0, vec![0xe4, 0x01, 0xee, 0x81, 0xf2])],
    );
    // A battle SFX over it: an FM hit that also retriggers the DAC.
    let sfx = SoundSequence::new(
        0xf5,
        1,
        vec![crate::FmVoice::fixture()],
        Vec::new(),
        vec![SoundTrack::fm(
            0,
            vec![0xef, 0, 0xee, 0x82, 0xa0, 0x08, 0xf2],
        )],
    );
    let ramp = |seed: u8| {
        (0..=255u8)
            .map(|i| i.wrapping_mul(seed))
            .collect::<Vec<_>>()
    };
    SoundBank::new(
        vec![music, sfx],
        vec![
            DacSample {
                id: 0x81,
                bytes: ramp(7),
            },
            DacSample {
                id: 0x82,
                bytes: ramp(13),
            },
        ],
    )
}

fn machine(traced: bool) -> SoundMachine {
    let mut machine = if traced {
        SoundMachine::with_register_trace()
    } else {
        SoundMachine::new()
    };
    machine.load_bank(looping_dac_bank());
    machine
}

/// Renders one driver tick of the looping DAC music, so the music is
/// installed before anything else is queued, then `seconds` more with the
/// SFX struck once a second. Returns the PCM as raw bits.
fn render(machine: &mut SoundMachine, seconds: usize) -> Vec<(u32, u32)> {
    let bits = |frames: Vec<crate::StereoFrame>| {
        frames
            .into_iter()
            .map(|frame| (frame.left.to_bits(), frame.right.to_bits()))
            .collect::<Vec<_>>()
    };
    machine.play(0x81);
    let mut pcm = bits(machine.render_frames((SAMPLE_RATE / crate::DRIVER_TICK_HZ) as usize));
    for _ in 0..seconds {
        machine.play(0xf5);
        pcm.extend(bits(machine.render_frames(SAMPLE_RATE as usize)));
    }
    pcm
}

#[test]
fn the_fixture_is_dac_heavy_when_traced() {
    let mut traced = machine(true);
    let _ = render(&mut traced, 1);
    let dac_writes = traced
        .take_register_log()
        .iter()
        .filter(|write| write.register == 0x2a)
        .count();
    // ~21 kHz at the default pitch: the rate that grew the live trace.
    assert!(
        dac_writes > 15_000,
        "only {dac_writes} DAC writes in one second"
    );
    assert!(traced.last_error().is_none());
}

#[test]
fn an_untraced_machine_records_nothing_over_minutes_of_dac() {
    let mut live = machine(false);
    let _ = render(&mut live, 300);
    assert!(live.last_error().is_none());
    assert!(!live.register_log().is_traced());
    assert_eq!(live.register_log().writes(), None);
}

#[test]
fn tracing_is_pure_observation() {
    let mut traced = machine(true);
    let mut live = machine(false);
    let traced_pcm = render(&mut traced, 3);
    let live_pcm = render(&mut live, 3);
    assert!(
        traced_pcm
            .iter()
            .any(|&(left, _)| f32::from_bits(left) != 0.0)
    );
    assert!(!traced.take_register_log().is_empty());
    assert_eq!(traced_pcm, live_pcm, "the trace changed the PCM");
}

#[test]
#[should_panic(expected = "SoundMachine::with_register_trace()")]
fn an_untraced_machine_refuses_to_hand_out_a_trace() {
    let mut live = machine(false);
    let _ = render(&mut live, 1);
    let _ = live.take_register_log();
}

/// Negative control: the determinism tests compare two logs. Built without
/// the opt-in, that comparison must fail loudly, not pass on two empty logs.
#[test]
fn a_determinism_check_without_the_opt_in_cannot_pass_vacuously() {
    let outcome = std::panic::catch_unwind(|| {
        let mut first = SoundMachine::new();
        first.load(SoundSequence::fixture_tone());
        first.start();
        let _ = first.tick();
        let mut second = SoundMachine::new();
        second.load(SoundSequence::fixture_tone());
        second.start();
        let _ = second.tick();
        assert_eq!(first.take_register_log(), second.take_register_log());
    });
    assert!(outcome.is_err(), "two untraced logs compared equal");
}
