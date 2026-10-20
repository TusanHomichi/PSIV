//! The boss-margin probe (#154) against the real pack: trials start from one
//! save, wait a different number of frames each, and play the chapter through
//! the route's own loop. The win-rate arithmetic and the outcome classes are
//! unit-tested in `src/probe.rs`; the boss tables themselves are evidence in the
//! route's source notes, not test fixtures (the saves are local).

mod common;

use std::path::PathBuf;

use psiv_campaign::probe::{Outcome, ProbeConfig, probe};
use psiv_campaign::route::Route;
use psiv_campaign::runner::{RunConfig, run};

use common::{MAIN_ROUTE, pack, pack_dir};

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("probe")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Two trials of the Holt chapter from the academy save: both complete, the
/// second after its lead, and the two play different frames because the seed
/// word the lead stirred is another stream.
#[test]
fn two_trials_from_one_save_play_different_streams_through_the_route_loop() {
    if pack().is_none() {
        return;
    }
    let route = Route::parse(&std::fs::read_to_string(MAIN_ROUTE).unwrap()).unwrap();
    let academy_dir = scratch("academy");
    let academy = run(&RunConfig {
        route: route.clone(),
        pack: pack_dir(),
        from_chapter: None,
        until_chapter: Some("academy".to_owned()),
        save_dir: academy_dir,
        verbose: false,
    })
    .expect("the academy chapter sets up");
    assert!(academy.completed);
    let result = probe(&ProbeConfig {
        route,
        pack: pack_dir(),
        chapter: "holt".to_owned(),
        save: academy.chapters[0].save.clone(),
        trials: 2,
        spacing: 600,
        work_dir: scratch("holt"),
    })
    .expect("the probe sets up");
    assert_eq!(result.trials.len(), 2);
    assert_eq!(result.trials[0].lead, 0);
    assert_eq!(result.trials[1].lead, 600);
    for trial in &result.trials {
        assert_eq!(trial.outcome, Outcome::Won, "{trial:?}");
    }
    assert_ne!(
        result.trials[0].frames, result.trials[1].frames,
        "the lead moved the stream, so the chapter's battles differ"
    );
    assert_eq!(result.wins(), 2);
    assert_eq!(result.losses(), 0);
}

/// A chapter the route does not have, or a save that is not there, is a setup
/// error, not a trial.
#[test]
fn a_missing_chapter_or_save_is_a_setup_error() {
    let route = Route::parse(&std::fs::read_to_string(MAIN_ROUTE).unwrap()).unwrap();
    let config = |chapter: &str, save: &str| ProbeConfig {
        route: route.clone(),
        pack: pack_dir(),
        chapter: chapter.to_owned(),
        save: PathBuf::from(save),
        trials: 1,
        spacing: 0,
        work_dir: scratch("setup"),
    };
    assert!(probe(&config("no-such-chapter", "/nonexistent")).is_err());
    assert!(probe(&config("holt", "/nonexistent/slot_1.sram")).is_err());
}
