//! A vehicle battle's end, against the real pack: what the battle writes back
//! to the saved vehicle record.
//!
//! Retail's battle end copies only the skill uses back (`Vehicle_Stats+$6A` to
//! `Saved_Vehicle_Stats+6`, `ps4.asm:6372-6389`) and every battle starts the
//! vehicle at its static HP (`loc_77AE`, `ps4.asm:11295-11300`), so the saved
//! HP never changes in a battle.

use std::path::Path;

use psiv_core::battle::{Outcome, RoundOrders};
use psiv_core::{Cell, Direction, Flag, StepFrames};
use psiv_data::{BattleFiles, GameData};

use crate::Runtime;

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

/// The Ice Digger with a saved record at 101 of 960 HP fights encounter 132
/// (the Dezolis formation the r8 trace met) at full static HP, and the battle
/// leaves the saved HP as it was while the uses come back. Negative: writing
/// the battle's HP back changes the saved record to the battle's 960.
#[test]
fn a_vehicle_battle_writes_back_uses_and_never_hp() {
    let pack = Path::new(PACK);
    if !pack.join("manifest.json").is_file() {
        eprintln!("runtime pack absent; skipping");
        return;
    }
    let files = BattleFiles::load(pack).unwrap();
    let mut rt = Runtime::new(
        GameData::load(pack).unwrap(),
        1,
        Cell::new(146, 50),
        Direction::Up,
        StepFrames::default(),
    )
    .unwrap();
    rt.enable_battles(&files).unwrap();
    rt.game.set(Flag::event(0x80)).unwrap();
    rt.set_vehicle_index(2).unwrap();
    {
        let record = &mut rt.game.vehicles_mut()[1];
        record.current_hp = 101;
        record.current_skill_uses = [3, 2, 0, 0, 0, 0, 0, 0];
    }
    let party = rt.battle_party();
    assert_eq!(party.len(), 1, "the vehicle is the one fighter");
    assert_eq!(
        party[0].stats.curr_hp, 0x03C0,
        "static HP at the battle's start"
    );
    rt.start_battle(132, party).unwrap();
    let mut ended = false;
    for _ in 0..40 {
        if rt
            .battle_round(&RoundOrders::Run)
            .unwrap()
            .iter()
            .any(|e| matches!(e, psiv_core::battle::BattleEvent::Ended { .. }))
        {
            ended = true;
            break;
        }
    }
    assert!(
        ended,
        "the vehicle ran or the battle ended within 40 rounds"
    );
    rt.finish_battle_for_outcome(Outcome::Escaped, 0);
    let record = rt.game.vehicles()[1];
    assert_eq!(record.current_hp, 101, "the saved HP is never written");
    assert_eq!(record.current_skill_uses[..2], [3, 2], "the uses come back");
}
