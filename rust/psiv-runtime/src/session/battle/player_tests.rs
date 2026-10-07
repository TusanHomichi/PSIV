//! Event-to-session coverage, separate from cartridge/core replay evidence.
use super::*;
use psiv_core::battle::{Command, status};
use psiv_core::{Cell, CharId, Direction, StepFrames};

fn runtime() -> Runtime {
    let pack = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack"));
    let mut runtime = Runtime::new(
        psiv_data::GameData::load(pack).expect("decoded runtime pack"),
        0x13,
        Cell { x: 48, y: 19 },
        Direction::Down,
        StepFrames::default(),
    )
    .unwrap();
    runtime
        .enable_battles(&psiv_data::BattleFiles::load(pack).unwrap())
        .unwrap();
    runtime.game.set_party([
        Some(CharId(1)),
        Some(CharId(0)),
        Some(CharId(2)),
        None,
        None,
    ]);
    runtime
}

fn mode(runtime: &mut Runtime) -> BattleMode {
    runtime.start_battle(0x8A, runtime.battle_party()).unwrap();
    BattleMode::begin(
        runtime,
        BattleTimeline {
            events: Vec::new(),
            sounds: Vec::new(),
            animations: Vec::new(),
        },
        BattleStart::Encounter(0x8A),
    )
}

#[test]
fn tp_event_updates_only_its_recipient_at_the_played_beat() {
    let mut runtime = runtime();
    let mut mode = mode(&mut runtime);
    let before = mode.party.clone();
    let target = FighterId::new(2).unwrap();
    mode.update_live_party(&BattleEvent::TpRestored {
        actor: FighterId::new(1).unwrap(),
        target,
        amount: 7,
        remaining_tp: 17,
    });
    for (old, new) in before.iter().zip(&mode.party) {
        assert_eq!(new.hp, old.hp);
        assert_eq!(new.status, old.status);
        assert_eq!(new.tp, if new.fighter == 2 { 17 } else { old.tp });
    }
}

#[test]
fn session_round_applies_arows_global_event_to_off_party_records() {
    let mut runtime = runtime();
    let mut off_party = runtime.game.roster().get(CharId(0)).unwrap().clone();
    off_party.status = status::ASLEEP | status::ASLEEP_2 | status::ANDROID_DEAD;
    off_party.agility.battle = 1;
    runtime
        .game
        .roster_mut()
        .seat(CharId(10), off_party)
        .unwrap();
    let actor = runtime.game.roster_mut().get_mut(CharId(1)).unwrap();
    actor.techniques[0] = 38;
    actor.curr_tp = 99;
    let mut mode = mode(&mut runtime);
    mode.run_round(
        &mut runtime,
        &RoundOrders::Commands(vec![
            Command::Technique {
                technique: 38,
                target: None,
            },
            Command::Defend,
            Command::Defend,
            Command::Defend,
            Command::Defend,
        ]),
    );
    assert!(mode.fault.is_none());
    let off_party = runtime.game.roster().get(CharId(10)).unwrap();
    assert_eq!(off_party.status, status::ASLEEP_2 | status::ANDROID_DEAD);
    assert_eq!(off_party.agility.battle, 1);
}
