//! The exit path's wait for Godot's audio server to retire the stopped
//! playback (#123). Godot-free, so `tests/audio_shutdown.rs` compiles it on
//! its own and checks it against a model of the server's mix loop.
//!
//! In Godot 4.7.1 (servers/audio/audio_server.cpp) `stop_playback_stream`
//! only marks a playback `FADE_OUT_TO_DELETION`. The server takes it off its
//! playback list in the next `_mix_step` that sees that mark, and frees only
//! what it took off when it is destroyed. A driver call
//! (`AudioDriver::audio_server_process`, which records the call's start and
//! frame count, `update_mix_time`, then runs `AudioServer::_driver_process`)
//! runs a step only when the server's 512-frame buffer is used up, and
//! between calls up to 511 of those frames are left. So one call proves
//! nothing on its own: a 128-frame call (PulseAudio at
//! `--audio-output-latency 3`) can end without a step, and so can two, or
//! three. What forces a step is frames: once calls begun after the stop have
//! taken 512 frames between them, more than the 511 that can be left, one of
//! them ran a step, and that step began after the stop, so it retired the
//! playback. [`RetirementWait`] counts those frames, from below.

/// Frames the server mixes per step (`buffer_size`, `AudioServer::init`):
/// calls begun after the stop that take this many frames between them
/// include a step.
pub(crate) const SERVER_STEP_FRAMES: u64 = 512;

/// The longest the exit path waits (engine ticks, µs). The headless Dummy
/// driver's 1,024-frame calls come every 23 ms at 44.1 kHz.
pub(crate) const RETIREMENT_WAIT_US: u64 = 500_000;

/// One poll of the driver, taken under `AudioServer.lock()` (the driver's
/// lock, which each Godot 4.7.1 driver holds through a call):
/// `AudioServer.get_time_since_last_mix()`, then `get_time_to_next_mix()`,
/// between two reads of the engine clock (`Time.get_ticks_usec()`, the clock
/// the driver stamps calls with). Under the lock both reads see the same call;
/// across a call they could add one call's age to the next one's length.
#[derive(Clone, Copy, Debug)]
pub(crate) struct DriverRead {
    /// The clock before the first read.
    pub(crate) read_at: u64,
    /// Seconds since the last call began.
    pub(crate) since_last_mix_s: f64,
    /// The last call's length in seconds less the time since it began.
    pub(crate) to_next_mix_s: f64,
    /// The driver's mix rate, frames per second.
    pub(crate) mix_rate: f64,
    /// The clock after the second read.
    pub(crate) returned_at: u64,
}

/// Counts driver frames, from calls begun after the stop, until they force a
/// mix step.
///
/// Each poll sees one call at most, and only through two reads, so the count
/// is a lower bound: a call between two polls goes uncounted, and a call
/// counted twice never happens. That only makes the wait longer, never
/// shorter.
#[derive(Clone, Debug)]
pub(crate) struct RetirementWait {
    stopped_at: u64,
    /// Every call still to be counted began after this.
    reference: u64,
    frames: u64,
}

impl RetirementWait {
    /// A wait for a playback stopped before `stopped_at` (engine ticks, µs).
    pub(crate) fn new(stopped_at: u64) -> Self {
        Self {
            stopped_at,
            reference: stopped_at,
            frames: 0,
        }
    }

    /// Takes one poll; true once the frames counted force a step.
    ///
    /// Both reads see one call, begun at `A` and `F` frames long, the first
    /// read made at `n1` and the second at `n2`. Their sum is
    /// `(n1 - A) + (F / rate - (n2 - A))`, that is `F / rate - (n2 - n1)`:
    /// never more than the call's length. `A` is judged from below,
    /// `read_at - since`, so a call counted began after the reference, and it
    /// began before `returned_at`, which becomes the reference: the next call
    /// counted is a later one.
    pub(crate) fn observe(&mut self, read: DriverRead) -> bool {
        if !self.done()
            && began_after(self.reference, read.read_at, read.since_last_mix_s)
            && let Some(frames) = call_frames(read)
        {
            self.frames += frames;
            self.reference = read.returned_at;
        }
        self.done()
    }

    /// Whether the frames counted force a step.
    pub(crate) fn done(&self) -> bool {
        self.frames >= SERVER_STEP_FRAMES
    }

    /// Driver frames counted so far.
    pub(crate) fn frames(&self) -> u64 {
        self.frames
    }

    /// Whether `now` (engine ticks, µs) is past the wait's bound.
    pub(crate) fn timed_out(&self, now: u64) -> bool {
        now.saturating_sub(self.stopped_at) > RETIREMENT_WAIT_US
    }
}

/// Whether the call the first read saw began after `reference`. The read can
/// come only later than `read_at`, so `read_at - since` is no later than the
/// call's true start: a call judged after the reference was.
fn began_after(reference: u64, read_at: u64, since_last_mix_s: f64) -> bool {
    let since_us = (since_last_mix_s * 1_000_000.0).ceil();
    if !since_us.is_finite() || since_us < 0.0 {
        return false;
    }
    (read_at as f64) - since_us > reference as f64
}

/// The frames a poll shows, rounded down: never more than the call's length
/// (see `observe`). The sum is that length exactly, or short of it by at
/// least one clock tick (1 µs, `rate / 1e6` frames); `ROUNDING` only absorbs
/// the floating-point error of the exact case, far below a tick. None for a
/// poll that shows none.
fn call_frames(read: DriverRead) -> Option<u64> {
    const ROUNDING: f64 = 1e-6;
    let seconds = read.since_last_mix_s + read.to_next_mix_s;
    let frames = (seconds * read.mix_rate + ROUNDING).floor();
    if !frames.is_finite() || frames < 1.0 || !read.mix_rate.is_finite() || read.mix_rate <= 0.0 {
        return None;
    }
    Some(frames as u64)
}
