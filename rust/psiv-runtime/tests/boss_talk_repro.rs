//! Live-QA repro: pressing at the non-interactable Igglanova on map $17 must
//! answer "nothing here", not open the principal's dialogue.

use std::path::Path;

use psiv_core::{Cell, Direction, Input, StepFrames};
use psiv_data::GameData;
use psiv_runtime::Runtime;

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

#[test]
fn the_igglanova_answers_nothing_here() {
    if !Path::new(PACK).join("manifest.json").is_file() {
        eprintln!("pack absent; skipping");
        return;
    }
    let data = GameData::load(Path::new(PACK)).expect("pack loads");
    let record = data.map(psiv_data::MapId(0x017)).expect("map");
    let boss = &record.npcs[0];
    let (bx, by) = (boss.x_cell as u16, boss.y_cell as u16);
    eprintln!("boss at ({bx},{by})");
    let mut rt = Runtime::new(
        data,
        0x017,
        Cell::new(bx, by + 1),
        Direction::Up,
        StepFrames::default(),
    )
    .expect("spawn below the boss");
    // Press up at the boss.
    let mut saw = Vec::new();
    for _ in 0..8 {
        for e in rt.tick(Input::Action) {
            saw.push(format!("{e:?}"));
        }
        for e in rt.tick(Input::Neutral) {
            saw.push(format!("{e:?}"));
        }
    }
    eprintln!("events: {saw:?}");
    assert!(
        saw.iter().any(|e| e.contains("InteractNothing")),
        "expected nothing-here, got {saw:?}"
    );
    assert!(
        !saw.iter().any(|e| e.contains("Interact {")),
        "the bit-clear boss must not be talkable: {saw:?}"
    );
}
