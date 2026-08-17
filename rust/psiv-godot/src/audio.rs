//! Minimal Godot output bridge for the native PSIV sound machine.

use godot::classes::{
    AudioStream, AudioStreamGenerator, AudioStreamGeneratorPlayback, AudioStreamPlayer,
};
use godot::prelude::*;
use psiv_data::{SoundFiles, SoundTrackKind};
use psiv_sound::{
    DacSample, FmVoice, PsgEnvelope, SAMPLE_RATE, SoundBank, SoundMachine, SoundSequence,
    SoundTrack, TrackKind,
};

const MAX_PUSH_FRAMES: i32 = 2_048;

pub(crate) struct AudioOutput {
    player: Option<Gd<AudioStreamPlayer>>,
    machine: SoundMachine,
    enabled: bool,
    debug_track: Option<u8>,
    debug_tone: bool,
}

impl AudioOutput {
    pub(crate) fn new(bank: SoundBank) -> Self {
        let mut stream = AudioStreamGenerator::new_gd();
        stream.set_mix_rate(SAMPLE_RATE as f32);
        stream.set_buffer_length(0.25);

        let mut player = AudioStreamPlayer::new_alloc();
        player.set_stream(&stream);

        let mut machine = SoundMachine::new();
        machine.load_bank(bank);

        Self {
            player: Some(player),
            machine,
            enabled: false,
            debug_track: parse_sound_id("PSIV_DEBUG_TRACK"),
            debug_tone: std::env::var("PSIV_DEBUG_TONE").is_ok_and(|value| value == "1"),
        }
    }

    pub(crate) fn start(&mut self) {
        self.enabled = true;
        if let Some(id) = self.debug_track {
            self.machine.play(id);
        } else if self.debug_tone {
            self.machine.load(SoundSequence::fixture_tone());
            self.machine.start();
        }
        if let Some(player) = self.player.as_mut() {
            player.play();
        }
    }

    pub(crate) fn play(&mut self, id: u8) {
        self.enabled = true;
        self.machine.play(id);
        if let Some(player) = self.player.as_mut() {
            player.play();
        }
    }

    pub(crate) fn has_debug_override(&self) -> bool {
        self.debug_track.is_some() || self.debug_tone
    }

    pub(crate) fn shutdown(&mut self) {
        if let Some(mut player) = self.player.take() {
            player.stop();
            player.set_stream(Gd::<AudioStream>::null_arg());
        }
    }

    pub(crate) fn node(&self) -> &Gd<AudioStreamPlayer> {
        self.player.as_ref().expect("audio node already shut down")
    }

    pub(crate) fn fill(&mut self) {
        if !self.enabled {
            return;
        }
        let Some(mut playback) = self.player.as_mut().and_then(generator_playback) else {
            return;
        };
        let available = playback.get_frames_available().min(MAX_PUSH_FRAMES);
        if available <= 0 {
            return;
        }

        let frames = self.machine.render_frames(available as usize);
        let mut buffer = PackedVector2Array::new();
        for frame in frames {
            buffer.push(Vector2::new(frame.left, frame.right));
        }
        playback.push_buffer(&buffer);
    }
}

pub(crate) fn sound_bank_from_data(files: &SoundFiles) -> Result<SoundBank, String> {
    let envelopes = files
        .volume_envelopes()
        .iter()
        .cloned()
        .map(PsgEnvelope::from_bytes)
        .collect::<Vec<_>>();
    let mut sequences = Vec::new();
    for record in files.music_records().chain(files.sfx_records()) {
        let voices = record
            .voices
            .iter()
            .map(|bytes| FmVoice::from_bytes(bytes).map_err(|error| error.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        let tracks = record
            .tracks
            .iter()
            .map(|source| {
                let kind = match source.kind {
                    SoundTrackKind::Fm => TrackKind::Fm,
                    SoundTrackKind::Psg => TrackKind::Psg,
                };
                let mut track =
                    SoundTrack::with_channel(kind, Some(source.channel), source.bytes.clone())
                        .start_offset(source.start_offset)
                        .volume(source.initial_volume);
                track.initial_voice = source.initial_voice;
                track.initial_transpose = source.initial_transpose;
                track.psg_envelope = source.psg_envelope;
                track.tick_multiplier = source.tick_multiplier;
                track.dac = source.dac;
                track
            })
            .collect();
        sequences.push(SoundSequence::new(
            record.id,
            record.tempo,
            voices,
            envelopes.clone(),
            tracks,
        ));
    }
    let dac_samples = files
        .dac_samples()
        .map(|(id, bytes)| DacSample {
            id,
            bytes: bytes.to_vec(),
        })
        .collect();
    Ok(SoundBank::new(sequences, dac_samples))
}

fn parse_sound_id(variable: &str) -> Option<u8> {
    let value = std::env::var(variable).ok()?;
    let value = value.trim();
    let (radix, digits) = if let Some(value) = value.strip_prefix("0x") {
        (16, value)
    } else if let Some(value) = value.strip_prefix('$') {
        (16, value)
    } else {
        (10, value)
    };
    u8::from_str_radix(digits, radix).ok()
}

fn generator_playback(
    player: &mut Gd<AudioStreamPlayer>,
) -> Option<Gd<AudioStreamGeneratorPlayback>> {
    player
        .get_stream_playback()
        .and_then(|playback| playback.try_cast::<AudioStreamGeneratorPlayback>().ok())
}

#[cfg(test)]
mod tests {
    use super::sound_bank_from_data;
    use psiv_data::GameData;
    use psiv_sound::{Chip, DRIVER_TICK_HZ, SAMPLE_RATE, SoundMachine};
    use std::path::Path;

    fn piata_bank() -> psiv_sound::SoundBank {
        let pack = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack"));
        let data = GameData::load(pack).expect("runtime pack loads");
        sound_bank_from_data(data.sound()).expect("sound records resolve")
    }

    fn real_track_log() -> Vec<psiv_sound::RegisterWrite> {
        let mut machine = SoundMachine::new();
        machine.load_bank(piata_bank());
        machine.play(0x84);
        let frames = (SAMPLE_RATE / DRIVER_TICK_HZ) as usize * 3;
        let _ = machine.render_frames(frames);
        assert!(
            machine.last_error().is_none(),
            "real track failed: {:?}",
            machine.last_error()
        );
        machine.take_register_log()
    }

    #[test]
    fn real_piata_track_first_ticks_are_deterministic_and_include_dac() {
        let first = real_track_log();
        let second = real_track_log();
        assert_eq!(first, second);
        assert!(first.iter().any(|write| {
            write.chip == Chip::Ym2612 && write.register == 0x2b && write.value == 0x80
        }));
        assert!(
            first
                .iter()
                .any(|write| write.chip == Chip::Ym2612 && write.register == 0x2a)
        );
    }

    #[test]
    fn debug_track_bank_contains_the_pack_path() {
        let bank = piata_bank();
        assert!(bank.sequences.iter().any(|sequence| sequence.id == 0x84));
        assert!(bank.sequences.iter().any(|sequence| sequence.id == 0xf2));
    }

    #[test]
    fn every_extracted_record_can_start_three_driver_ticks() {
        let bank = piata_bank();
        let ids: Vec<u8> = bank.sequences.iter().map(|sequence| sequence.id).collect();
        let frames = (SAMPLE_RATE / DRIVER_TICK_HZ) as usize * 3;
        for id in ids {
            let mut machine = SoundMachine::new();
            machine.load_bank(bank.clone());
            machine.play(id);
            let _ = machine.render_frames(frames);
            assert!(
                machine.last_error().is_none(),
                "extracted record {id:#04x} failed: {:?}",
                machine.last_error()
            );
        }
    }
}
