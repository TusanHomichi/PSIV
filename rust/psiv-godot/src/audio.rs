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
    music_paused: bool,
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
            music_paused: false,
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

    pub(crate) fn pause_music(&mut self) {
        self.music_paused = true;
    }

    pub(crate) fn resume_music(&mut self) {
        self.music_paused = false;
    }

    pub(crate) fn has_debug_override(&self) -> bool {
        self.debug_track.is_some() || self.debug_tone
    }

    /// Releases the player's playback before the engine tears down (#123).
    ///
    /// `stop` only marks the playback for deletion
    /// (`AudioServer::stop_playback_stream`, Godot 4.7.1
    /// servers/audio/audio_server.cpp): the audio thread's next mix takes it
    /// off the server's playback list, and the server frees what it took off
    /// when it is destroyed. The field stops it while the tree is torn down at
    /// quit, and the driver is finished soon after, so a quit that came
    /// before the next mix left the playback on the list, never freed: the
    /// `AudioStreamGeneratorPlayback` ObjectDB leak headless runs reported.
    /// So the exit path waits, bounded, for one mix that began after the stop.
    pub(crate) fn shutdown(&mut self) {
        let Some(mut player) = self.player.take() else {
            return;
        };
        let had_playback = player.has_stream_playback();
        player.stop();
        player.set_stream(Gd::<AudioStream>::null_arg());
        if had_playback && !wait_for_mix_after_stop() {
            godot_warn!(
                "audio: no mix within {} ms of the shutdown stop; Godot may report its playback leaked",
                SHUTDOWN_MIX_WAIT_US / 1_000
            );
        }
    }

    pub(crate) fn node(&self) -> &Gd<AudioStreamPlayer> {
        self.player.as_ref().expect("audio node already shut down")
    }

    pub(crate) fn fill(&mut self) {
        if !self.enabled || self.music_paused {
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

/// The longest the exit path waits for the audio thread's next mix. The
/// headless Dummy driver mixes every 1,024 frames at 44.1 kHz (about 23 ms).
const SHUTDOWN_MIX_WAIT_US: u64 = 500_000;

/// Waits until a mix that began after this call has finished; false when none
/// came within `SHUTDOWN_MIX_WAIT_US`.
///
/// The driver records each mix's start time and mixes under its lock, and
/// `get_time_since_last_mix` reads that time under the same lock, so a read
/// that shows a start after the stop also shows that mix finished.
fn wait_for_mix_after_stop() -> bool {
    let time = godot::classes::Time::singleton();
    let server = godot::classes::AudioServer::singleton();
    let stopped_at = time.get_ticks_usec();
    loop {
        let read_at = time.get_ticks_usec();
        let since = server.get_time_since_last_mix();
        if mixed_after(stopped_at, read_at, since) {
            return true;
        }
        if time.get_ticks_usec().saturating_sub(stopped_at) > SHUTDOWN_MIX_WAIT_US {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

/// Whether the last mix began after `stopped_at`, given the time since it
/// (seconds) read no earlier than `read_at` (both in engine ticks, µs). The
/// read can only come later than `read_at`, which makes the start computed
/// here no later than the true one: a mix judged after the stop was.
fn mixed_after(stopped_at: u64, read_at: u64, since_last_mix_s: f64) -> bool {
    let since_us = (since_last_mix_s * 1_000_000.0).ceil();
    if !since_us.is_finite() || since_us < 0.0 {
        return false;
    }
    (read_at as f64) - since_us > stopped_at as f64
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
        let mut machine = SoundMachine::with_register_trace();
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

    /// Renders `seconds` of one retail record as raw PCM bits.
    fn retail_pcm(machine: &mut SoundMachine, id: u8, seconds: usize) -> Vec<(u32, u32)> {
        machine.load_bank(piata_bank());
        machine.play(id);
        let pcm = machine
            .render_frames(SAMPLE_RATE as usize * seconds)
            .into_iter()
            .map(|frame| (frame.left.to_bits(), frame.right.to_bits()))
            .collect();
        assert!(machine.last_error().is_none(), "{:?}", machine.last_error());
        pcm
    }

    fn audible(pcm: &[(u32, u32)]) -> bool {
        pcm.iter()
            .any(|&(left, right)| f32::from_bits(left) != 0.0 || f32::from_bits(right) != 0.0)
    }

    /// #92: the live machine the bridge builds keeps no chip-write trace, and
    /// tracing never changes one PCM bit. `$84` drives the DAC (YM `$2A`, the
    /// write that grew the old always-on trace); `$F5` is a battle SFX.
    #[test]
    fn the_live_machine_keeps_no_trace_and_tracing_changes_no_pcm() {
        for (id, dac) in [(0x84, true), (0xf5, false)] {
            let mut traced = SoundMachine::with_register_trace();
            let mut live = SoundMachine::new();
            let traced_pcm = retail_pcm(&mut traced, id, 3);
            let live_pcm = retail_pcm(&mut live, id, 3);
            let log = traced.take_register_log();
            assert!(!log.is_empty(), "{id:#04x} wrote nothing");
            if dac {
                assert!(
                    log.iter()
                        .any(|write| write.chip == Chip::Ym2612 && write.register == 0x2a),
                    "{id:#04x} never wrote the DAC"
                );
            }
            assert!(audible(&traced_pcm), "{id:#04x} rendered silence");
            assert_eq!(live.register_log().writes(), None);
            assert_eq!(traced_pcm, live_pcm, "the trace changed {id:#04x}'s PCM");
        }
    }

    /// The exit path's wait ends only on a mix that began after the stop
    /// (#123). Times are hand-picked engine ticks (µs) and seconds.
    #[test]
    fn only_a_mix_begun_after_the_stop_ends_the_shutdown_wait() {
        use super::mixed_after;
        // Stopped at 1.000000 s; read at 1.030000 s.
        // 0.010 s since the last mix: it began at 1.020000 s, after the stop.
        assert!(mixed_after(1_000_000, 1_030_000, 0.010));
        // 0.040 s: it began at 0.990000 s, before the stop.
        assert!(!mixed_after(1_000_000, 1_030_000, 0.040));
        // 0.030 s: it began at the stop's own tick, not after it.
        assert!(!mixed_after(1_000_000, 1_030_000, 0.030));
        // Never mixed: the driver's last mix time is 0, so the time since is
        // the whole uptime.
        assert!(!mixed_after(1_000_000, 1_030_000, 1.030));
        assert!(!mixed_after(1_000_000, 1_030_000, f64::NAN));
        assert!(!mixed_after(1_000_000, 1_030_000, -0.5));
    }
}
