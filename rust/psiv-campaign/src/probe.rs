//! The boss-margin probe (#154): how often the route's own policy wins a
//! fight, measured instead of guessed.
//!
//! A training target chosen from one route run is the luck of that run's
//! random streams: a change upstream that moves RNG consumption can flip the
//! boss. The probe plays one chapter (the fight and whatever readies it) from
//! the save the chapter before it ended on, `trials` times, each in a fresh
//! session, and reports how many it won with a Wilson lower bound.
//!
//! **The streams.** A loaded save starts the cartridge's seed word again
//! (`Lcg41::default`), and the cartridge stirs that word every frame in every
//! mode (the VInt handler's `UpdateRNGSeed`, `ps4.asm:617`; the runtime's
//! `Runtime::tick`), so a player who stands still for `k` frames before the
//! chapter meets the boss on another stream. Trial `i` waits `i * spacing`
//! neutral frames first: no seam into the runtime, only pads. The streams are
//! windows of one LCG sequence, `2 * spacing` draws apart (the field tick
//! stirs it once more on each idle frame), so a `spacing` larger than half the
//! draws one attempt consumes keeps the windows disjoint: an attempt of `n`
//! frames draws at most `2n` plus its battles' own rolls.
//!
//! Every trial plays through [`crate::runner`]'s own chapter loop: the same
//! controllers, policy and recovery as the route, nothing duplicated.

use std::path::PathBuf;

use psiv_runtime::Pad;

use crate::driver::Driver;
use crate::exec::Memory;
use crate::halt::HaltKind;
use crate::route::Route;
use crate::runner::{Failure, RunConfig, play_chapter};
use crate::start::{SetupError, StartPoint, open_session};

/// Everything a probe needs.
#[derive(Debug, Clone)]
pub struct ProbeConfig {
    /// The route that holds the chapter.
    pub route: Route,
    /// The runtime pack directory.
    pub pack: PathBuf,
    /// The chapter each trial plays.
    pub chapter: String,
    /// The save the chapter starts from (the previous chapter's boundary save).
    pub save: PathBuf,
    /// How many trials.
    pub trials: u32,
    /// Neutral frames between consecutive trials' starts.
    pub spacing: u64,
    /// Where each trial writes its chapter save (`trial-NNN/`).
    pub work_dir: PathBuf,
}

/// How a trial ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The chapter completed.
    Won,
    /// The party was defeated.
    Lost,
    /// The chapter halted for any other reason: a route or port fault, not a
    /// measure of the fight. The report names it.
    Halted(String),
}

/// One trial.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trial {
    /// Neutral frames waited before the chapter.
    pub lead: u64,
    /// How it ended.
    pub outcome: Outcome,
    /// Frames the chapter played, the lead excluded.
    pub frames: u64,
}

/// A probe's trials.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeResult {
    /// In trial order.
    pub trials: Vec<Trial>,
}

impl ProbeResult {
    /// Trials won.
    #[must_use]
    pub fn wins(&self) -> u32 {
        self.count(|o| *o == Outcome::Won)
    }

    /// Trials lost.
    #[must_use]
    pub fn losses(&self) -> u32 {
        self.count(|o| *o == Outcome::Lost)
    }

    /// Trials that halted for another reason.
    #[must_use]
    pub fn halted(&self) -> u32 {
        self.count(|o| matches!(o, Outcome::Halted(_)))
    }

    fn count(&self, test: impl Fn(&Outcome) -> bool) -> u32 {
        self.trials.iter().filter(|t| test(&t.outcome)).count() as u32
    }

    /// The Wilson score interval's 95% lower bound on the win rate over the
    /// trials that were fought (won or lost), in parts per million; `0` with
    /// no fought trial.
    #[must_use]
    pub fn wilson_lower_ppm(&self) -> u32 {
        wilson_lower_ppm(self.wins(), self.wins() + self.losses())
    }
}

/// `z^2` for the 95% interval, `1.96^2 = 3.8416`, over [`Z2_SCALE`].
const Z2: u128 = 38_416;
/// The scale of [`Z2`].
const Z2_SCALE: u128 = 10_000;
/// Parts per million.
const PPM: u128 = 1_000_000;

/// The Wilson score 95% lower bound for `wins` of `n`, in parts per million:
/// `(w + z²/2 - sqrt(z² (w(n-w)/n + z²/4))) / (n + z²)`, the textbook form
/// multiplied through by `n`. Integer arithmetic throughout: the crate keeps
/// floating point out (`#![deny(clippy::float_arithmetic)]`), and the bound is
/// exact to the part per million (the square root rounds down).
#[must_use]
pub fn wilson_lower_ppm(wins: u32, n: u32) -> u32 {
    if n == 0 {
        return 0;
    }
    let (w, n) = (u128::from(wins.min(n)), u128::from(n));
    let centre = w * PPM + Z2 * PPM / (2 * Z2_SCALE);
    // z² (w(n-w)/n + z²/4), scaled by PPM²: Z2 (4 S w(n-w) + Z2 n) PPM² / (4 S² n).
    let inside =
        Z2 * (4 * Z2_SCALE * w * (n - w) + Z2 * n) * PPM * PPM / (4 * Z2_SCALE * Z2_SCALE * n);
    let margin = isqrt(inside);
    let denominator = n * PPM + Z2 * PPM / Z2_SCALE;
    let lower = centre.saturating_sub(margin) * PPM / denominator;
    u32::try_from(lower).unwrap_or(u32::MAX)
}

/// The integer square root, rounded down (Newton's method).
fn isqrt(value: u128) -> u128 {
    if value < 2 {
        return value;
    }
    let mut x = value;
    let mut y = x.div_ceil(2);
    while y < x {
        x = y;
        y = (x + value / x) / 2;
    }
    x
}

/// How a trial's chapter ended: a defeat is the measure, any other halt is a
/// fault the probe reports apart from it.
fn outcome_of(result: Result<(), Failure>) -> Outcome {
    match result {
        Ok(()) => Outcome::Won,
        Err(failure) if failure.halt.kind == HaltKind::LostBattle => Outcome::Lost,
        Err(failure) => Outcome::Halted(format!(
            "objective {} ({}): {}",
            failure.index, failure.kind, failure.halt
        )),
    }
}

/// Runs the probe.
///
/// # Errors
///
/// [`SetupError`] for an unknown chapter, a missing save or a pack that will
/// not load. A trial's halt is a result, not an error.
pub fn probe(config: &ProbeConfig) -> Result<ProbeResult, SetupError> {
    let index = config
        .route
        .chapters
        .iter()
        .position(|c| c.id == config.chapter)
        .ok_or_else(|| SetupError(format!("the route has no chapter {:?}", config.chapter)))?;
    let chapter = &config.route.chapters[index];
    if !config.save.is_file() {
        return Err(SetupError(format!(
            "the probe starts from {} and it does not exist",
            config.save.display()
        )));
    }
    let mut trials = Vec::with_capacity(config.trials as usize);
    for trial in 0..config.trials {
        let lead = u64::from(trial) * config.spacing;
        let (session, _) = open_session(&config.pack, &StartPoint::Save(config.save.clone()))?;
        let save_dir = config.work_dir.join(format!("trial-{trial:03}"));
        let mut driver = Driver::new(session, Some(save_dir.clone()));
        let mut outcome = None;
        for _ in 0..lead {
            if let Err(halt) = driver.tick(Pad::NEUTRAL) {
                outcome = Some(Outcome::Halted(format!("{halt}")));
                break;
            }
        }
        let started = driver.frames();
        let outcome = outcome.unwrap_or_else(|| {
            let run = RunConfig {
                route: config.route.clone(),
                pack: config.pack.clone(),
                from_chapter: Some(config.chapter.clone()),
                until_chapter: Some(config.chapter.clone()),
                save_dir,
                verbose: false,
            };
            outcome_of(
                play_chapter(&mut driver, &mut Memory::default(), &run, index, chapter).map(|_| ()),
            )
        });
        trials.push(Trial {
            lead,
            outcome,
            frames: driver.frames() - started,
        });
    }
    Ok(ProbeResult { trials })
}

#[cfg(test)]
mod tests {
    use super::{Outcome, outcome_of, wilson_lower_ppm};
    use crate::halt::{Halt, HaltKind};
    use crate::runner::Failure;

    fn failure(kind: HaltKind) -> Failure {
        Failure {
            index: 1,
            kind: "step_onto".to_owned(),
            objective: serde_json::Value::Null,
            halt: Halt::new(kind, "detail"),
        }
    }

    /// A completed chapter is a win, a defeat a loss, and every other halt is
    /// kept out of the rate: a budget or a scene fault measures the route,
    /// not the fight.
    #[test]
    fn a_defeat_is_a_loss_and_any_other_halt_is_not_counted() {
        assert_eq!(outcome_of(Ok(())), Outcome::Won);
        assert_eq!(
            outcome_of(Err(failure(HaltKind::LostBattle))),
            Outcome::Lost
        );
        for kind in [
            HaltKind::BudgetExhausted,
            HaltKind::SceneFault,
            HaltKind::ExpectFailed,
        ] {
            assert!(
                matches!(outcome_of(Err(failure(kind))), Outcome::Halted(_)),
                "{kind:?}"
            );
        }
    }

    /// Reference values computed from the Wilson formula to 40 digits
    /// (`(p + z^2/2n - z sqrt(p(1-p)/n + z^2/4n^2)) / (1 + z^2/n)`, `z^2` =
    /// 3.8416) outside the code under test: 20 of 20 is 838,869.87 ppm, 10 of
    /// 20 is 299,294.91, 97 of 100 is 915,479.22 and 15 of 40 is 242,227.74.
    /// The integer bound rounds down, so it may sit one or two parts per
    /// million below, never above.
    #[test]
    fn the_wilson_bound_matches_reference_values() {
        for (wins, n, reference) in [
            (20, 20, 838_869),
            (10, 20, 299_294),
            (97, 100, 915_479),
            (15, 40, 242_227),
        ] {
            let got = wilson_lower_ppm(wins, n);
            assert!(
                got <= reference && reference - got <= 2,
                "{wins}/{n}: {got}"
            );
        }
        assert_eq!(wilson_lower_ppm(0, 20), 0);
        assert_eq!(wilson_lower_ppm(0, 0), 0);
    }
}
