//! Every oracle tape has a number of its own (#431).
//!
//! Prose names tapes by number ("tape 37", "tape-37 frame 7384"), so two
//! files in `oracle/tapes/` that share a number prefix make that prose name
//! two different tapes. A new tape takes the next free number.

use std::collections::BTreeMap;
use std::path::Path;

/// The numbers more than one of `names` starts with (`NN_name.tape`), each
/// with the names that share it; names without a number prefix are listed
/// under `None`.
fn shared_numbers(names: &[String]) -> BTreeMap<Option<u32>, Vec<String>> {
    let mut by_number: BTreeMap<Option<u32>, Vec<String>> = BTreeMap::new();
    for name in names {
        let number = name
            .split_once('_')
            .and_then(|(prefix, _)| prefix.parse::<u32>().ok());
        by_number.entry(number).or_default().push(name.clone());
    }
    by_number.retain(|number, names| number.is_none() || names.len() > 1);
    by_number
}

#[test]
fn shared_numbers_finds_a_repeat_and_an_unnumbered_tape() {
    let names = [
        "36_teleport_windows.tape",
        "37_anger_tower_barrier.tape",
        "37_battle_window_closing.tape",
        "38_ending_elsydeon.tape",
        "probe.tape",
    ]
    .map(String::from);
    let shared = shared_numbers(&names);
    assert_eq!(
        shared.get(&Some(37)).map(Vec::len),
        Some(2),
        "the two 37s: {shared:?}"
    );
    assert_eq!(shared.get(&None).map(Vec::len), Some(1), "{shared:?}");
    assert_eq!(shared.len(), 2, "{shared:?}");
}

#[test]
fn every_oracle_tape_has_a_number_of_its_own() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/tapes");
    let names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
        .map(|entry| entry.expect("a tape entry").file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".tape"))
        .collect();
    assert!(
        names.len() > 30,
        "only {} tapes in {}",
        names.len(),
        dir.display()
    );
    let shared = shared_numbers(&names);
    assert!(
        shared.is_empty(),
        "oracle tapes that share a number or have none (give each the next free number): {shared:?}"
    );
}
