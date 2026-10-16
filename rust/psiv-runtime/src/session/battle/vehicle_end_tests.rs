//! A vehicle battle's end, against the real pack: what the battle writes back
//! to the saved vehicle record.
//!
//! Retail's battle end copies only the skill uses back (`Vehicle_Stats+$6A` to
//! `Saved_Vehicle_Stats+6`, `ps4.asm:6372-6389`) and every battle starts the
//! vehicle at its static HP (`loc_77AE`, `ps4.asm:11309-11310`), so the saved
//! HP never changes in a battle.

use std::path::Path;

use psiv_core::battle::{BattleEvent, Command, RoundOrders};
use psiv_core::{Cell, Direction, Flag, StepFrames};
use psiv_data::{BattleFiles, GameData};

use crate::Runtime;

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

/// The Ice Digger with a saved record at 101 of 960 HP fights encounter 132
/// (the Dezolis formation the r8 trace met) at full static HP, spends one use
/// of vehicle skill 5 (mask `$50`: skills 5 and 7), and the battle's end
/// leaves the saved HP as it was while the spent use comes back to the
/// record. Negatives: writing the battle's HP back changes the saved record
/// to the battle's 960; not writing the uses back leaves the record at 3.
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
        record.skill_mask = 0x50;
        record.current_skill_uses = [0, 0, 0, 0, 3, 0, 2, 0];
        record.max_skill_uses = [0, 0, 0, 0, 8, 0, 4, 0];
    }
    let party = rt.battle_party();
    assert_eq!(party.len(), 1, "the vehicle is the one fighter");
    assert_eq!(
        party[0].stats.curr_hp, 0x03C0,
        "static HP at the battle's start"
    );
    rt.start_battle(132, party).unwrap();
    let ended = |events: &[BattleEvent]| {
        events.iter().find_map(|e| match e {
            BattleEvent::Ended { outcome } => Some(*outcome),
            _ => None,
        })
    };
    let first = rt
        .battle_round(&RoundOrders::Commands(vec![Command::VehicleSkill(5)]))
        .unwrap();
    assert!(
        first.iter().any(|e| matches!(
            e,
            BattleEvent::VehicleSkillUsed {
                skill: 5,
                remaining: 2,
                ..
            }
        )),
        "one use of skill 5 spent: {first:?}"
    );
    let mut outcome = ended(&first);
    for _ in 0..40 {
        if outcome.is_some() {
            break;
        }
        outcome = ended(&rt.battle_round(&RoundOrders::Run).unwrap());
    }
    let outcome = outcome.expect("the battle ended within 40 rounds");
    rt.finish_battle_for_outcome(outcome, 0);
    let record = rt.game.vehicles()[1];
    assert_eq!(record.current_hp, 101, "the saved HP is never written");
    assert_eq!(record.current_skill_uses[4], 2, "the spent use comes back");
    assert_eq!(
        record.current_skill_uses[6], 2,
        "an unspent skill keeps its uses"
    );
}
