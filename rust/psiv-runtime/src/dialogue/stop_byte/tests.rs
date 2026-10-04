//! `stop_byte`: the byte under `Saved_Dialogue_Addr` when a dialogue stops.
//!
//! Retail data throughout, so each case skips with a message when the local
//! pack is absent.

use super::{segment_byte, stop_byte_at};
use crate::dialogue::{DialogueRunner, DialogueSignal};
use crate::pad::{Button, Pad};
use psiv_core::GameState;
use psiv_data::DialogueSet;
use std::path::PathBuf;
use std::sync::Arc;

fn pack_dir() -> PathBuf {
    match std::env::var_os("PSIV_RUNTIME_PACK") {
        Some(path) => PathBuf::from(path),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("runtime-pack"),
    }
}

fn pack() -> Option<Arc<DialogueSet>> {
    let dir = pack_dir();
    if !dir.join("dialogue").join("trees.json").is_file() {
        eprintln!("skipping: no dialogue pack at {}", dir.display());
        return None;
    }
    Some(Arc::new(DialogueSet::load(&dir).expect("the pack loads")))
}

/// Each entry's `raw_hex` from the pack file: `(tree, entry id, bytes)`.
fn raw_entries() -> Vec<(u8, u16, Vec<u8>)> {
    let text = std::fs::read_to_string(pack_dir().join("dialogue").join("trees.json")).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    let mut found = Vec::new();
    for tree in value["trees"].as_array().unwrap() {
        let number = tree["tree"].as_u64().unwrap() as u8;
        for entry in tree["entries"].as_array().unwrap() {
            let hex = entry["raw_hex"].as_str().unwrap();
            let bytes = (0..hex.len())
                .step_by(2)
                .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).unwrap())
                .collect();
            found.push((number, entry["id"].as_u64().unwrap() as u16, bytes));
        }
    }
    found
}

/// The first byte the stop cursor reads at the start of entry `id`.
fn raw_first(entries: &[(u8, u16, Vec<u8>)], tree: u8, id: u16) -> u8 {
    entries
        .iter()
        .find(|(t, i, _)| *t == tree && *i == id)
        .and_then(|(_, _, bytes)| bytes.first().copied())
        .unwrap_or(0xFF)
}

#[test]
fn every_entry_opens_on_the_byte_its_segments_give() {
    let Some(set) = pack() else {
        return;
    };
    let mut wrong = Vec::new();
    for (tree, id, bytes) in raw_entries() {
        let entry = set.entry(tree, id).expect("the pack has the entry");
        let rebuilt = entry
            .segments
            .first()
            .map_or(Some(0xFF), |segment| segment_byte(&set, segment));
        let actual = bytes.first().copied().unwrap_or(0xFF);
        if rebuilt != Some(actual) {
            wrong.push(format!(
                "tree {tree} entry {id}: rebuilt {rebuilt:?}, raw {actual:#X}"
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn the_space_ship_answer_stops_on_the_period_that_opens_entry_61() {
    // `Event_Gyuna`'s `cmpi.b #$35, (a0)`: entry 60 ends on a terminator and
    // entry 61 ("...What's the use now?") opens with `$35`.
    let Some(set) = pack() else {
        return;
    };
    let raw = raw_entries();
    assert_eq!(raw_first(&raw, 16, 61), 0x35);
    let end = set.entry(16, 60).unwrap().segments.len();
    assert_eq!(stop_byte_at(&set, 16, 60, end, false), Some(0x35));
    // Off by one entry either way is not `$35`.
    for id in [59, 61] {
        let end = set.entry(16, id).unwrap().segments.len();
        let got = stop_byte_at(&set, 16, id, end, false);
        assert_eq!(got, Some(raw_first(&raw, 16, id + 1)), "entry {id}");
        assert_ne!(got, Some(0x35), "entry {id}");
    }
    // Entry 62 is the tree's last: nothing readable follows it.
    let end = set.entry(16, 62).unwrap().segments.len();
    assert_eq!(stop_byte_at(&set, 16, 62, end, false), None);
}

#[test]
fn an_f7_stop_reads_the_next_byte_of_the_same_entry() {
    // Tree 14 entry 30 (Tyler's grave) yields once at `$F7`; the byte after it
    // is the first of the text that resumes.
    let Some(set) = pack() else {
        return;
    };
    let raw = raw_entries();
    let bytes = &raw.iter().find(|(t, i, _)| *t == 14 && *i == 30).unwrap().2;
    let at = bytes.iter().position(|&b| b == 0xF7).expect("one $F7");
    let entry = set.entry(14, 30).unwrap();
    // The segment index that follows the `$F7` control.
    let index = entry
        .segments
        .iter()
        .position(|s| matches!(s, psiv_data::Segment::Control(c) if c.code() == 0xF7))
        .unwrap()
        + 1;
    assert_eq!(stop_byte_at(&set, 14, 30, index, true), Some(bytes[at + 1]));
    // One segment earlier would read the `$F7` itself.
    assert_eq!(stop_byte_at(&set, 14, 30, index - 1, true), Some(0xF7));
}

/// Drives the window as a player does until it closes, answering each yes/no
/// with the next of `answers`.
fn play(runner: &mut DialogueRunner, game: &mut GameState, answers: &[bool]) {
    let mut answers = answers.iter();
    for _ in 0..20_000 {
        if !runner.is_open() {
            return;
        }
        let view = runner.view().expect("a window is up");
        let pad = if let Some(choice) = view.choice {
            if choice.ready {
                let yes = *answers.next().expect("an answer for every prompt");
                Pad::new(if yes { Button::Speak } else { Button::Cancel })
            } else {
                Pad::NEUTRAL
            }
        } else if view.dismissable {
            Pad::new(Button::Speak)
        } else {
            Pad::NEUTRAL
        };
        runner.input(game, pad, false);
        runner.window(game);
        let signals = runner.drain_signals();
        assert!(
            !signals
                .iter()
                .any(|s| matches!(s, DialogueSignal::Fault(_))),
            "{signals:?}"
        );
    }
    panic!("the dialogue never closed");
}

fn gyuna(answers: &[bool]) -> Option<Option<u8>> {
    let set = pack()?;
    let mut runner = DialogueRunner::with_pack(set);
    let mut game = GameState::new();
    assert!(runner.open_scene_entry(16, 0x31, false, &game));
    play(&mut runner, &mut game, answers);
    Some(runner.stop_byte())
}

#[test]
fn the_window_reports_the_period_when_the_conversation_ends_in_entry_60() {
    // Entry 49 asks about the storm: NO jumps four entries on to 53 (the tower),
    // NO again to 56 (Raja), NO to 60 (the space ship), YES runs entry 60 out.
    let Some(stop) = gyuna(&[false, false, false, true]) else {
        return;
    };
    assert_eq!(stop, Some(0x35));
}

#[test]
fn declining_the_space_ship_ends_somewhere_else() {
    // The same road with a final NO jumps to entry 62 ("Thank you."), the
    // tree's last entry: nothing readable follows it, so not `$35`.
    let Some(stop) = gyuna(&[false, false, false, false]) else {
        return;
    };
    assert_eq!(stop, None);
}

#[test]
fn the_storm_answer_ends_elsewhere_too() {
    // YES at the first prompt: entry 51 ends the conversation.
    let Some(stop) = gyuna(&[true]) else {
        return;
    };
    assert_ne!(stop, Some(0x35));
}
