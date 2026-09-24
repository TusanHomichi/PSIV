//! `$FFFFEEA8`: where the ability-index word starts, and how long it lives.
//!
//! The word is written only by `Enemy_Attack` (`ps4.asm:19151`) and read only
//! by its own re-roll loop (`ps4.asm:19149`), so its lifetime is what the two
//! clears around it leave: the boot RAM clear (`ps4.asm:379-402`) at power-on,
//! and `GameMode_LoadBattle`'s page wipe (`ps4.asm:9992-9994`) at every battle
//! load. The oracle measured both on the cartridge - tape 10's third battle is
//! the one that shows a non-zero value surviving into the next load and being
//! wiped by it (`docs/BATTLE_ORACLE_REPLAY.md`) - and these tests hold the
//! port's session state to the same two edges.
use psiv_core::battle::{BattleEvent, Outcome, RoundOrders};
use psiv_core::{RetailLocation, RetailSave, StepFrames};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::Runtime;
use std::path::Path;

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

/// The formation both battles here are fought against: two ZoranBults, whose
/// eight ability slots are all ability 0 - so the index the word carries is
/// the draw, not a choice between effects.
const FORMATION: u16 = 0x8A;

/// A runtime at the Academy Basement with battles armed, on `seed`.
fn fixture(seed: u32) -> Option<Runtime> {
    let pack = Path::new(PACK);
    if !pack.join("manifest.json").exists() {
        return None;
    }
    let files = BattleFiles::load(pack).unwrap();
    let mut initial =
        Runtime::new_game(GameData::load(pack).unwrap(), StepFrames::default()).unwrap();
    initial.enable_battles(&files).unwrap();
    let mut snapshot = initial.game().snapshot();
    snapshot.party = [1, 0, 2, 0xFF, 0xFF];
    let mut rt = Runtime::from_save(
        GameData::load(pack).unwrap(),
        RetailSave {
            snapshot,
            location: RetailLocation {
                world_index: 0,
                map_index_2: 0,
                map_index: 0x15,
                char_x: 768,
                char_y: 160,
            },
        },
        StepFrames::default(),
    )
    .unwrap();
    rt.enable_battles(&files).unwrap();
    rt.set_rng_seed(seed);
    Some(rt)
}

/// Attacks until the field is clear, returning the per-member award.
fn win(rt: &mut Runtime) -> u16 {
    for _ in 0..100 {
        for event in rt.battle_round(&RoundOrders::attack_all()).unwrap() {
            if let BattleEvent::Rewarded {
                experience_each, ..
            } = event
            {
                return experience_each;
            }
        }
    }
    panic!("the opening party failed to defeat two ZoranBults");
}

#[test]
fn a_battle_load_clears_the_word_and_the_battle_leaves_its_last_index() {
    // This seed's battles both leave the word at 3, which is what makes the
    // clear below visible; the assertion that it is non-zero is what would fail
    // if an engine change ever flattened the roll stream.
    let Some(mut rt) = fixture(0x0101_5678) else {
        return;
    };

    assert_eq!(
        rt.last_ability_index(),
        0,
        "a new session's word is the boot clear's zero (ps4.asm:379-402)"
    );

    rt.start_battle_timeline(FORMATION, rt.battle_party())
        .unwrap();
    assert_eq!(
        rt.last_ability_index(),
        0,
        "and a battle load clears it again (ps4.asm:9992-9994)"
    );

    // The battle's own copy is the live one until the epilogue reads it back,
    // so the session word is read after finishing, as the cartridge's single
    // cell would be read between battles.
    let experience = win(&mut rt);
    rt.finish_battle_for_outcome(Outcome::Victory, experience);
    let left = rt.last_ability_index();
    assert_ne!(
        left, 0,
        "the battle has to leave the word non-zero for the next load's clear \
         to be observable"
    );
    assert!(
        left <= 7,
        "the word holds an index, not the raw draw: {left}"
    );

    // The second load is the edge the oracle measured: GameMode_LoadBattle
    // wipes $FFFFEE00-$FFFFEEFF, so whatever the last battle left is gone
    // before the first ability roll compares against it.
    rt.start_battle_timeline(FORMATION, rt.battle_party())
        .unwrap();
    assert_eq!(
        rt.last_ability_index(),
        0,
        "the next battle load clears the word the last battle left"
    );

    let experience = win(&mut rt);
    rt.finish_battle_for_outcome(Outcome::Victory, experience);
    let second = rt.last_ability_index();
    assert_ne!(second, 0, "and the second battle sets it in its turn");
    assert!(second <= 7);
}
