//! PSIV's live sound path.
//!
//! The public input is deliberately close to the cartridge: a sequence owns
//! raw track bytes and local voices, and [`SoundMachine`] interprets those
//! bytes on the PSIV tick clock.  The extraction lane can deserialize its
//! `runtime-pack/sound/` records into these types without teaching this crate
//! about JSON or the ROM.
//!
//! Every chip write passes through [`RegisterLog`] before it reaches a core.
//! That is the fixture seam: current tests assert the deterministic write
//! stream, while future oracle captures can replace expected vectors without
//! changing the renderer or driver.

mod driver;
mod fm;
mod psg;
mod sequence;
mod trace;

pub use fm::Ym2612;
pub use psg::Sn76489;
pub use sequence::{FmVoice, PsgEnvelope, SequenceError, SoundSequence, SoundTrack, TrackKind};
pub use trace::{Chip, RegisterLog, RegisterWrite};

/// Genesis libretro and the scout's oracle path use 44.1 kHz PCM.
pub const SAMPLE_RATE: u32 = 44_100;
/// The NTSC Mega Drive master clock divided by seven, used as the YM clock.
pub const YM_CLOCK_HZ: u32 = 7_670_454;
/// The driver's ten-byte update cadence is exposed as a tick, not a guessed
/// wall-clock duration.  A sequence's tempo byte determines how often tracks
/// advance on that cadence.
pub const DRIVER_TICK_HZ: u32 = 60;

/// One interleaved stereo frame in the native mixer format.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StereoFrame {
    pub left: f32,
    pub right: f32,
}

/// A live YM2612 + SN76489 machine with the PSIV bytecode driver in front of
/// it.  It is intentionally single-threaded; the Godot bridge owns it on its
/// audio-output node and asks for PCM frames when the generator has room.
pub struct SoundMachine {
    driver: driver::Driver,
    ym: Ym2612,
    psg: Sn76489,
    log: RegisterLog,
}

impl SoundMachine {
    /// Creates an idle machine.  Use [`Self::load`] before rendering a song.
    pub fn new() -> Self {
        Self {
            driver: driver::Driver::new(),
            ym: Ym2612::new(SAMPLE_RATE),
            psg: Sn76489::new(SAMPLE_RATE),
            log: RegisterLog::new(),
        }
    }

    /// Loads the extraction-facing sequence and resets all chip state.
    pub fn load(&mut self, sequence: SoundSequence) {
        self.driver.load(sequence);
        self.ym.reset();
        self.psg.reset();
        self.log.clear();
    }

    /// Starts the sequence's first frame on the next [`Self::tick`].
    pub fn start(&mut self) {
        self.driver.start();
    }

    /// Queues a PSIV sound ID.  The driver applies the documented range
    /// dispatch and priority behavior at the next tick.
    pub fn queue_sound(&mut self, id: u8) {
        self.driver.queue_sound(id);
    }

    /// Runs one 60 Hz driver update and returns the first mixed sample from
    /// that update.  The audio bridge normally uses [`Self::render_frames`].
    pub fn tick(&mut self) -> StereoFrame {
        self.driver.tick(&mut self.ym, &mut self.psg, &mut self.log);
        let frame = self.render_one();
        self.driver.samples_until_tick = self.driver.samples_until_tick.saturating_sub(1);
        frame
    }

    /// Renders native-rate stereo frames.  The driver tick is scheduled from
    /// the exact sample clock so Godot does not become the sequencer clock.
    pub fn render_frames(&mut self, count: usize) -> Vec<StereoFrame> {
        let mut frames = Vec::with_capacity(count);
        for _ in 0..count {
            if self.driver.samples_until_tick == 0 {
                self.driver.tick(&mut self.ym, &mut self.psg, &mut self.log);
            }
            frames.push(self.render_one());
            self.driver.samples_until_tick = self.driver.samples_until_tick.saturating_sub(1);
        }
        frames
    }

    /// Returns and clears the write trace since the previous call.
    pub fn take_register_log(&mut self) -> Vec<RegisterWrite> {
        self.log.take()
    }

    /// Returns the current driver tick for diagnostics and future oracle
    /// alignment.
    pub fn tick_count(&self) -> u64 {
        self.driver.tick_count()
    }

    /// Returns the first malformed-sequence error observed by the driver.
    pub fn last_error(&self) -> Option<&SequenceError> {
        self.driver.last_error()
    }

    fn render_one(&mut self) -> StereoFrame {
        let fm = self.ym.render_sample();
        let psg = self.psg.render_sample();
        StereoFrame {
            left: (fm[0] + psg[0]).clamp(-1.0, 1.0),
            right: (fm[1] + psg[1]).clamp(-1.0, 1.0),
        }
    }
}

impl Default for SoundMachine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{SoundMachine, SoundSequence, StereoFrame};

    #[test]
    fn fixture_renders_non_silent_frames() {
        let mut machine = SoundMachine::new();
        machine.load(SoundSequence::fixture_tone());
        machine.start();
        let frames = machine.render_frames(2_205);
        assert!(frames.iter().any(|frame| {
            frame != &StereoFrame::default() && (frame.left.abs() + frame.right.abs()) > 0.001
        }));
    }
}
