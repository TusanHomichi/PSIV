//! Minimal Godot output bridge for the native PSIV sound machine.

use godot::classes::{
    AudioStream, AudioStreamGenerator, AudioStreamGeneratorPlayback, AudioStreamPlayer,
};
use godot::prelude::*;
use psiv_sound::{SAMPLE_RATE, SoundMachine, SoundSequence};

const MAX_PUSH_FRAMES: i32 = 2_048;

pub(crate) struct AudioOutput {
    player: Option<Gd<AudioStreamPlayer>>,
    playback: Option<Gd<AudioStreamGeneratorPlayback>>,
    machine: SoundMachine,
    enabled: bool,
}

impl AudioOutput {
    pub(crate) fn new() -> Self {
        let mut stream = AudioStreamGenerator::new_gd();
        stream.set_mix_rate(SAMPLE_RATE as f32);
        stream.set_buffer_length(0.25);

        let mut player = AudioStreamPlayer::new_alloc();
        player.set_stream(&stream);

        let mut machine = SoundMachine::new();
        machine.load(SoundSequence::fixture_tone());

        Self {
            player: Some(player),
            playback: None,
            machine,
            enabled: false,
        }
    }

    pub(crate) fn start(&mut self) {
        self.enabled = true;
        self.machine.start();
        if let Some(player) = self.player.as_mut() {
            player.play();
        }
    }

    pub(crate) fn shutdown(&mut self) {
        self.playback = None;
        if let Some(mut player) = self.player.take() {
            player.stop();
            player.set_stream(Gd::<AudioStream>::null_arg());
            player.queue_free();
        }
    }

    pub(crate) fn node(&self) -> &Gd<AudioStreamPlayer> {
        self.player.as_ref().expect("audio node already shut down")
    }

    pub(crate) fn fill(&mut self) {
        if !self.enabled {
            return;
        }
        if self.playback.is_none() {
            self.playback = self.player.as_mut().and_then(generator_playback);
        }
        let Some(playback) = self.playback.as_mut() else {
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

fn generator_playback(
    player: &mut Gd<AudioStreamPlayer>,
) -> Option<Gd<AudioStreamGeneratorPlayback>> {
    player
        .get_stream_playback()
        .and_then(|playback| playback.try_cast::<AudioStreamGeneratorPlayback>().ok())
}
