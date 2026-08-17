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

#[test]
fn eb_sequence_sound_dispatch_overlays_music_deterministically() {
    let music = SoundSequence::new(
        0x81,
        1,
        vec![crate::FmVoice::fixture()],
        Vec::new(),
        vec![SoundTrack::fm(
            0,
            vec![0xef, 0, 0x81, 0xeb, 0xb5, 0x20, 0x81, 0x20, 0xf2],
        )],
    );
    let sfx = SoundSequence::new(
        0xb5,
        1,
        vec![crate::FmVoice::fixture()],
        Vec::new(),
        vec![SoundTrack::fm(0, vec![0xef, 0, 0x20, 0x20, 0xf2])],
    );
    let bank = crate::SoundBank::new(vec![music, sfx], Vec::new());

    let mut first = crate::SoundMachine::new();
    first.load_bank(bank.clone());
    first.play(0x81);
    let _ = first.tick();
    let _ = first.tick();
    let _ = first.tick();
    let first_log = first.take_register_log();

    let mut second = crate::SoundMachine::new();
    second.load_bank(bank);
    second.play(0x81);
    let _ = second.tick();
    let _ = second.tick();
    let _ = second.tick();
    let second_log = second.take_register_log();

    assert_eq!(first_log, second_log);
    assert!(first_log.iter().any(|write| write.register == 0x28));
    assert!(first_log.iter().any(|write| write.register == 0x30));
    assert!(first.last_error().is_none());
}

#[test]
fn battle_action_sfx_dispatch_order_is_deterministic_over_theme() {
    let mut music_voice = crate::FmVoice::fixture();
    music_voice.algorithm = 1;
    let mut sword_voice = crate::FmVoice::fixture();
    sword_voice.algorithm = 2;
    let mut miss_voice = crate::FmVoice::fixture();
    miss_voice.algorithm = 3;
    let track = |voice| SoundTrack::fm(0, vec![0xef, 0, 0x81, 0x20, 0xf2]).voice(voice);
    let bank = crate::SoundBank::new(
        vec![
            SoundSequence::new(0x81, 1, vec![music_voice], Vec::new(), vec![track(0)]),
            SoundSequence::new(0xf5, 1, vec![sword_voice], Vec::new(), vec![track(0)]),
            SoundSequence::new(0xb8, 1, vec![miss_voice], Vec::new(), vec![track(0)]),
        ],
        Vec::new(),
    );

    let run = |bank| {
        let mut machine = crate::SoundMachine::new();
        machine.load_bank(bank);
        machine.play(0x81);
        let _ = machine.tick();
        machine.play(0xf5);
        let _ = machine.tick();
        machine.play(0xb8);
        let _ = machine.tick();
        (machine.take_register_log(), machine.last_error().is_none())
    };

    let (first_log, first_ok) = run(bank.clone());
    let (second_log, second_ok) = run(bank);
    assert_eq!(first_log, second_log);
    assert!(first_ok && second_ok);
    let voice_algorithms: Vec<_> = first_log
        .iter()
        .filter(|write| write.register == 0xb0)
        .map(|write| write.value)
        .collect();
    let compressed: Vec<_> =
        voice_algorithms
            .into_iter()
            .fold(Vec::new(), |mut values, algorithm| {
                if values.last().copied() != Some(algorithm) {
                    values.push(algorithm);
                }
                values
            });
    assert_eq!(compressed, vec![1, 2, 3]);
}

#[test]
fn distinct_enemy_attack_sfx_register_log_is_ordered_and_deterministic() {
    let mut zoran_voice = crate::FmVoice::fixture();
    zoran_voice.algorithm = 4;
    let mut gunner_voice = crate::FmVoice::fixture();
    gunner_voice.algorithm = 5;
    let track = |voice| SoundTrack::fm(0, vec![0xef, 0, 0x81, 0x20, 0xf2]).voice(voice);
    let bank = crate::SoundBank::new(
        vec![
            SoundSequence::new(0xD8, 1, vec![zoran_voice], Vec::new(), vec![track(0)]),
            SoundSequence::new(0xD6, 1, vec![gunner_voice], Vec::new(), vec![track(0)]),
        ],
        Vec::new(),
    );

    let run = |bank| {
        let mut machine = crate::SoundMachine::new();
        machine.load_bank(bank);
        machine.play(0xD8); // Zoran Bult: retail EnemyAttack4.
        let _ = machine.tick();
        machine.play(0xD6); // Gunner Bit: retail MechEnemyAlarm.
        let _ = machine.tick();
        (machine.take_register_log(), machine.last_error().is_none())
    };

    let (first_log, first_ok) = run(bank.clone());
    let (second_log, second_ok) = run(bank);
    assert_eq!(first_log, second_log);
    assert!(first_ok && second_ok);
    let algorithms: Vec<_> = first_log
        .iter()
        .filter(|write| write.register == 0xB0)
        .map(|write| write.value)
        .collect();
    let compressed = algorithms
        .into_iter()
        .fold(Vec::new(), |mut values, algorithm| {
            if values.last().copied() != Some(algorithm) {
                values.push(algorithm);
            }
            values
        });
    assert_eq!(compressed, vec![4, 5]);
}
