use super::{SoundSequence, SoundTrack};
use crate::{Chip, SoundMachine};

#[test]
fn fixture_fm_register_trace_is_deterministic() {
    let mut first = SoundMachine::new();
    first.load(SoundSequence::fixture_tone());
    first.start();
    let _ = first.tick();
    let first_log = first.take_register_log();

    let mut second = SoundMachine::new();
    second.load(SoundSequence::fixture_tone());
    second.start();
    let _ = second.tick();
    assert_eq!(first_log, second.take_register_log());
    let writes: Vec<_> = first_log
        .iter()
        .map(|write| (write.port, write.register, write.value))
        .collect();
    assert_eq!(
        writes,
        vec![
            (0, 0xb0, 0x07),
            (0, 0x30, 0x01),
            (0, 0x38, 0x01),
            (0, 0x34, 0x01),
            (0, 0x3c, 0x01),
            (0, 0x50, 0x1f),
            (0, 0x58, 0x1f),
            (0, 0x54, 0x1f),
            (0, 0x5c, 0x1f),
            (0, 0x60, 0x00),
            (0, 0x68, 0x00),
            (0, 0x64, 0x00),
            (0, 0x6c, 0x00),
            (0, 0x70, 0x00),
            (0, 0x78, 0x00),
            (0, 0x74, 0x00),
            (0, 0x7c, 0x00),
            (0, 0x80, 0x0f),
            (0, 0x88, 0x0f),
            (0, 0x84, 0x0f),
            (0, 0x8c, 0x0f),
            (0, 0x40, 0x10),
            (0, 0x48, 0x10),
            (0, 0x44, 0x10),
            (0, 0x4c, 0x10),
            (0, 0xb4, 0xc0),
            (0, 0xa4, 0x0a),
            (0, 0xa0, 0x5e),
            (0, 0x28, 0xf0),
        ]
    );
    assert!(first_log.iter().all(|write| write.chip == Chip::Ym2612));
}

#[test]
fn fixture_psg_trace_contains_latch_and_data_writes() {
    let sequence = SoundSequence::new(
        0x81,
        1,
        Vec::new(),
        Vec::new(),
        vec![SoundTrack::psg(0, vec![0x81, 0x04, 0x08, 0xf2])],
    );
    let mut machine = SoundMachine::new();
    machine.load(sequence);
    machine.start();
    let _ = machine.tick();
    let log = machine.take_register_log();
    assert_eq!(
        log.iter().map(|write| write.value).collect::<Vec<_>>(),
        vec![0x86, 0x35, 0x90]
    );
    assert!(log.iter().all(|write| write.chip == Chip::Sn76489));
}

#[test]
fn command_loop_and_goto_are_executed() {
    let sequence = SoundSequence::new(
        0x81,
        1,
        vec![crate::FmVoice::fixture()],
        Vec::new(),
        vec![SoundTrack::fm(
            0,
            vec![0xef, 0, 0x81, 1, 0xf7, 0, 2, 0xff, 0xf9, 0xf2],
        )],
    );
    let mut machine = SoundMachine::new();
    machine.load(sequence);
    machine.start();
    let _ = machine.tick();
    assert!(machine.last_error().is_none());
}

#[test]
fn disabled_pan_animation_consumes_only_its_count_byte() {
    let sequence = SoundSequence::new(
        0x81,
        1,
        Vec::new(),
        Vec::new(),
        vec![SoundTrack::fm(0, vec![0xff, 0x00, 0x00, 0xf2])],
    );
    let mut machine = SoundMachine::new();
    machine.load(sequence);
    machine.start();
    let _ = machine.tick();
    assert!(machine.last_error().is_none());
}
