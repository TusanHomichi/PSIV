//! The exit path's retirement wait (`src/audio_shutdown.rs`, #123) against a
//! model of Godot 4.7.1's driver and mix loop.
//!
//! The model is `AudioDriver::audio_server_process` (record the call's start
//! and frames, then run the server), `AudioServer::_driver_process` (a mix
//! step whenever the 512-frame buffer is used up) and the two reads the wait
//! polls under the driver's lock, `get_time_since_last_mix` and
//! `get_time_to_next_mix`, as servers/audio/audio_server.cpp computes them.
//! The wait may say a step ran only when one did, for every buffer phase at
//! the stop, fixed and varying call sizes (128-frame PulseAudio calls among
//! them), and polls that miss calls or come late in one.

#[path = "../src/audio_shutdown.rs"]
mod audio_shutdown;

use audio_shutdown::{DriverRead, RETIREMENT_WAIT_US, RetirementWait, SERVER_STEP_FRAMES};

/// `AudioServer::_driver_process`: frames left in the buffer, and the steps.
struct Server {
    to_mix: u64,
    steps: u64,
}

impl Server {
    fn process(&mut self, frames: u64) {
        let mut todo = frames;
        while todo > 0 {
            if self.to_mix == 0 {
                self.steps += 1;
                self.to_mix = 512;
            }
            let copied = self.to_mix.min(todo);
            todo -= copied;
            self.to_mix -= copied;
        }
    }
}

/// `AudioDriver`: the last call's start (engine ticks, µs) and frames.
struct Driver {
    rate: u32,
    last_time: u64,
    last_frames: u64,
    server: Server,
}

impl Driver {
    /// A server with `to_mix` frames left, after one call at `at`.
    fn new(rate: u32, to_mix: u64, at: u64) -> Self {
        Self {
            rate,
            last_time: at,
            last_frames: 1_024,
            server: Server { to_mix, steps: 0 },
        }
    }

    fn call(&mut self, at: u64, frames: u64) {
        self.last_time = at;
        self.last_frames = frames;
        self.server.process(frames);
    }

    fn since_last_mix(&self, now: u64) -> f64 {
        (now - self.last_time) as f64 / 1_000_000.0
    }

    fn to_next_mix(&self, now: u64) -> f64 {
        let total = (now - self.last_time) as f64 / 1_000_000.0;
        self.last_frames as f64 / f64::from(self.rate) - total
    }

    /// A poll under the lock (no call between its reads), the reads landing
    /// at `n1` and `n2`, between `read_at` and `returned_at`.
    fn read(&self, read_at: u64, n1: u64, n2: u64, returned_at: u64) -> DriverRead {
        DriverRead {
            read_at,
            since_last_mix_s: self.since_last_mix(n1),
            to_next_mix_s: self.to_next_mix(n2),
            mix_rate: f64::from(self.rate),
            returned_at,
        }
    }

    /// Microseconds a call of `frames` lasts, at least 8 so polls fit in.
    fn period_us(&self, frames: u64) -> u64 {
        (frames * 1_000_000 / u64::from(self.rate)).max(8)
    }
}

/// How the exit path's polls fall against the driver's calls.
#[derive(Clone, Copy, Debug)]
enum Polls {
    /// Once after each call.
    AfterEach,
    /// Once after every third call: the two between go uncounted.
    EveryThird,
    /// Late in each call, with time passing between the two reads.
    Late,
}

/// What a run of the wait saw.
#[derive(Debug)]
struct Run {
    /// Calls after the stop before the wait ended, if it did.
    done_after: Option<usize>,
    /// Steps the server ran after the stop by then.
    steps_by_then: u64,
}

const STOP: u64 = 1_000_000;

/// Stops with `to_mix` frames left in the server's buffer, then drives calls
/// of `sizes` (cycled) until the wait ends or `limit` calls have run. Fails
/// the test the moment the wait ends before a step ran after the stop.
fn run(rate: u32, to_mix: u64, sizes: &[u64], polls: Polls, limit: usize) -> Run {
    let mut driver = Driver::new(rate, to_mix, STOP - 23_000);
    let mut wait = RetirementWait::new(STOP);
    // A poll before any call after the stop sees only the one before it.
    assert!(!wait.observe(driver.read(STOP + 1, STOP + 1, STOP + 1, STOP + 2)));
    let mut now = STOP + 3;
    for call in 0..limit {
        let frames = sizes[call % sizes.len()];
        let at = now + 2;
        driver.call(at, frames);
        let ended = match polls {
            Polls::AfterEach => wait.observe(driver.read(at + 1, at + 1, at + 1, at + 2)),
            Polls::EveryThird => {
                call % 3 == 2 && wait.observe(driver.read(at + 1, at + 1, at + 1, at + 2))
            }
            Polls::Late => {
                let n1 = at + driver.period_us(frames) - 6;
                wait.observe(driver.read(n1 - 1, n1, n1 + 3, n1 + 4))
            }
        };
        if ended {
            assert!(
                driver.server.steps > 0,
                "the wait ended with no step after the stop: rate {rate}, {to_mix} frames left, \
                 calls {sizes:?}, polls {polls:?}, {} frames counted after {} calls",
                wait.frames(),
                call + 1
            );
            return Run {
                done_after: Some(call + 1),
                steps_by_then: driver.server.steps,
            };
        }
        now = at + driver.period_us(frames);
    }
    Run {
        done_after: None,
        steps_by_then: driver.server.steps,
    }
}

/// Deterministic call sizes from 1 to 700 frames (a 64-bit LCG).
fn varying_sizes(seed: u64, count: usize) -> Vec<u64> {
    let mut state = seed;
    (0..count)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            1 + (state >> 33) % 700
        })
        .collect()
}

/// The review's case (#123): 384 frames left at the stop and 128-frame calls.
/// Two calls take 256 frames, three 384, and none of them steps; a wait that
/// counted two calls would end there. This one waits for the fourth.
#[test]
fn two_short_calls_without_a_step_do_not_end_the_wait() {
    let mut driver = Driver::new(44_100, 384, STOP - 3_000);
    let mut wait = RetirementWait::new(STOP);
    let mut at = STOP + 10;
    for call in 1..=4 {
        driver.call(at, 128);
        let ended = wait.observe(driver.read(at + 1, at + 1, at + 1, at + 2));
        assert_eq!(driver.server.steps, u64::from(call == 4), "call {call}");
        assert_eq!(ended, call == 4, "call {call}: {} frames", wait.frames());
        at += 2_902;
    }
}

/// One headless Dummy call (1,024 frames) always steps, and ends the wait.
#[test]
fn one_dummy_call_ends_the_wait_from_every_phase() {
    for to_mix in 0..SERVER_STEP_FRAMES {
        let run = run(44_100, to_mix, &[1_024], Polls::AfterEach, 10);
        assert_eq!(run.done_after, Some(1), "{to_mix} frames left");
        assert!(run.steps_by_then >= 1);
    }
}

/// Every phase, fixed call sizes from one frame to past a step, two mix
/// rates, and each way polls fall: the wait ends only after a step, and
/// after exactly the calls its count needs. A poll after each call counts
/// its frames; one late in a call, 3 µs between its reads, counts one frame
/// fewer, so a one-frame call counts for nothing and the wait runs to its
/// bound.
#[test]
fn the_wait_ends_only_after_a_step_for_every_phase_and_period() {
    let sizes = [
        1, 2, 3, 64, 100, 127, 128, 255, 256, 257, 441, 480, 511, 512, 513, 1_024, 2_048,
    ];
    for rate in [44_100, 48_000] {
        for frames in sizes {
            for to_mix in 0..SERVER_STEP_FRAMES {
                for polls in [Polls::AfterEach, Polls::EveryThird, Polls::Late] {
                    let run = run(rate, to_mix, &[frames], polls, 4_000);
                    let expected = match polls {
                        Polls::AfterEach => Some(SERVER_STEP_FRAMES.div_ceil(frames)),
                        Polls::EveryThird => Some(3 * SERVER_STEP_FRAMES.div_ceil(frames)),
                        Polls::Late if frames == 1 => None,
                        Polls::Late => Some(SERVER_STEP_FRAMES.div_ceil(frames - 1)),
                    };
                    assert_eq!(
                        run.done_after.map(|calls| calls as u64),
                        expected,
                        "calls of {frames} at {rate} Hz, {to_mix} left, {polls:?}"
                    );
                }
            }
        }
    }
}

/// Calls whose sizes change from one to the next, from every phase.
#[test]
fn the_wait_ends_only_after_a_step_for_varying_call_sizes() {
    for seed in 0..24 {
        let sizes = varying_sizes(seed, 64);
        for to_mix in (0..SERVER_STEP_FRAMES).step_by(7) {
            for polls in [Polls::AfterEach, Polls::EveryThird, Polls::Late] {
                let run = run(44_100, to_mix, &sizes, polls, 4_000);
                assert!(
                    run.done_after.is_some(),
                    "seed {seed}, {to_mix} left, {polls:?}"
                );
            }
        }
    }
}

/// A call that began before the stop counts for nothing, and one call read
/// twice counts once.
#[test]
fn only_distinct_calls_begun_after_the_stop_count() {
    let mut driver = Driver::new(44_100, 0, STOP - 5_000);
    driver.last_frames = 2_048;
    let mut wait = RetirementWait::new(STOP);
    // The call before the stop, 2,048 frames, read after it.
    assert!(!wait.observe(driver.read(STOP + 5, STOP + 5, STOP + 5, STOP + 6)));
    assert_eq!(wait.frames(), 0);
    // A call that began at the stop's own tick is not after it.
    driver.call(STOP, 300);
    assert!(!wait.observe(driver.read(STOP + 10, STOP + 10, STOP + 10, STOP + 11)));
    assert_eq!(wait.frames(), 0);
    // A 300-frame call after the stop, read three times: counted once.
    driver.call(STOP + 100, 300);
    for read_at in [STOP + 101, STOP + 150, STOP + 200] {
        assert!(!wait.observe(driver.read(read_at, read_at, read_at, read_at + 1)));
        assert_eq!(wait.frames(), 300);
    }
    // The next 300-frame call ends it.
    driver.call(STOP + 7_000, 300);
    assert!(wait.observe(driver.read(STOP + 7_001, STOP + 7_001, STOP + 7_001, STOP + 7_002)));
    assert_eq!(wait.frames(), 600);
}

/// Time between the two reads rounds the count down: a 512-frame call read
/// over 1 µs counts 511 frames and does not end the wait alone.
#[test]
fn time_between_the_reads_rounds_the_count_down() {
    let mut driver = Driver::new(44_100, 0, STOP - 5_000);
    let mut wait = RetirementWait::new(STOP);
    driver.call(STOP + 100, 512);
    assert!(!wait.observe(driver.read(STOP + 200, STOP + 200, STOP + 201, STOP + 202)));
    assert_eq!(wait.frames(), 511);
}

/// Why the exit path takes the driver's lock for its two reads: across a
/// call, the first read's age of the old call adds to the new call's length,
/// and the poll counts more frames than any call ran.
#[test]
fn reads_across_a_call_would_count_more_than_it_ran() {
    let mut driver = Driver::new(44_100, 511, STOP - 5_000);
    let mut wait = RetirementWait::new(STOP);
    driver.call(STOP + 100, 2_048);
    // The first read, 1,900 µs into the 2,048-frame call; then a 100-frame
    // call; then the second read.
    let since = driver.since_last_mix(STOP + 2_000);
    driver.call(STOP + 2_100, 100);
    let read = DriverRead {
        read_at: STOP + 2_000,
        since_last_mix_s: since,
        to_next_mix_s: driver.to_next_mix(STOP + 2_101),
        mix_rate: 44_100.0,
        returned_at: STOP + 2_102,
    };
    wait.observe(read);
    assert!(wait.frames() > 100, "{} frames", wait.frames());
}

/// Reads that show no call: never mixed (the time since is the uptime), not
/// a number, negative, or no mix rate.
#[test]
fn reads_that_show_no_call_count_nothing() {
    let read = |since: f64, to_next: f64, rate: f64| DriverRead {
        read_at: STOP + 30_000,
        since_last_mix_s: since,
        to_next_mix_s: to_next,
        mix_rate: rate,
        returned_at: STOP + 30_001,
    };
    for poll in [
        read(1.030, -1.0, 44_100.0),
        read(f64::NAN, 0.02, 44_100.0),
        read(-0.5, 0.02, 44_100.0),
        read(0.001, f64::NAN, 44_100.0),
        read(0.001, -0.001, 44_100.0),
        read(0.001, 0.02, 0.0),
        read(0.001, 0.02, f64::NAN),
    ] {
        let mut wait = RetirementWait::new(STOP);
        assert!(!wait.observe(poll), "{poll:?}");
        assert_eq!(wait.frames(), 0, "{poll:?}");
    }
}

/// The bound: past 500 ms from the stop, not at it.
#[test]
fn the_wait_is_bounded_at_half_a_second() {
    let wait = RetirementWait::new(STOP);
    assert_eq!(RETIREMENT_WAIT_US, 500_000);
    assert!(!wait.timed_out(STOP));
    assert!(!wait.timed_out(STOP + RETIREMENT_WAIT_US));
    assert!(wait.timed_out(STOP + RETIREMENT_WAIT_US + 1));
    // A clock read before the stop is not past it.
    assert!(!wait.timed_out(STOP - 1));
}
