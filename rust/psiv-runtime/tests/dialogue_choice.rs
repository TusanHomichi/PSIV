//! The player's answer, including NO, controls Chaz's house recovery.
use psiv_core::{
    Cell, CharId, Direction, GameState, Input, RetailLocation, RetailSave, StepFrames,
};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::{Runtime, RuntimeEvent};
use std::path::Path;
const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

fn fixture() -> Option<Runtime> {
    let pack = Path::new(PACK);
    if !pack.join("manifest.json").is_file() {
        return None;
    }
    let files = BattleFiles::load(pack).unwrap();
    let mut initial = Runtime::new(
        GameData::load(pack).unwrap(),
        0x13,
        Cell::new(48, 19),
        Direction::Down,
        StepFrames::default(),
    )
    .unwrap();
    initial.enable_battles(&files).unwrap();
    let mut game = GameState::from_snapshot(&initial.game().snapshot());
    game.set_party([Some(CharId(1)), Some(CharId(0)), None, None, None]);
    for who in [CharId(0), CharId(1)] {
        let stats = game.roster_mut().get_mut(who).unwrap();
        stats.curr_hp = 1;
        stats.curr_tp = 1;
        stats.curr_skill_uses = [0; 8];
    }
    for vehicle in game.vehicles_mut() {
        vehicle.current_hp = 3;
        vehicle.current_skill_uses = [0; 8];
    }
    let mut rt = Runtime::from_save(
        GameData::load(pack).unwrap(),
        RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index_2: 0,
                map_index: 0x5E,
                char_x: 368,
                char_y: 544,
            },
        },
        StepFrames::default(),
    )
    .unwrap();
    rt.enable_battles(&files).unwrap();
    Some(rt)
}

fn run_house(yes: bool, delayed: bool) {
    let Some(mut rt) = fixture() else {
        return;
    };
    if yes
        && !delayed
        && let Some(dir) = std::env::var_os("PSIV_CHOICE_SMOKE_SAVE_DIR")
    {
        rt.save_slot(Path::new(&dir), 0).unwrap();
    }
    let before = rt.game().snapshot();
    assert!(rt.start_event(0x3B));
    let mut prompted = false;
    let mut waited = false;
    for _ in 0..5000 {
        for event in rt.tick(Input::Neutral) {
            match event {
                RuntimeEvent::SceneDialogue { entry: 49 } => {
                    prompted = true;
                    if !delayed {
                        rt.dialogue_choice(yes);
                    }
                    rt.dialogue_closed();
                }
                RuntimeEvent::SceneChoiceRequested => {
                    assert!(delayed, "the dialogue's answer should already be available");
                    for _ in 0..60 {
                        assert!(
                            rt.tick(Input::Neutral)
                                .iter()
                                .all(|e| !matches!(e, RuntimeEvent::SceneEnded))
                        );
                        assert_eq!(
                            rt.game().snapshot(),
                            before,
                            "waiting cannot heal or advance"
                        );
                    }
                    waited = true;
                    rt.dialogue_choice(yes);
                    // Godot closes the standalone choice window on this tick.
                    rt.dialogue_closed();
                }
                RuntimeEvent::SceneFaulted { .. } => panic!("{event:?}"),
                _ => {}
            }
        }
        if !rt.scene_active() {
            break;
        }
    }
    assert!(prompted);
    assert_eq!(waited, delayed);
    assert!(!rt.scene_active());
    assert_eq!(
        rt.game().roster().get(CharId(2)),
        before.characters[2].as_ref(),
        "off-party characters are untouched"
    );
    for vehicle in rt.game().vehicles() {
        assert_eq!(
            vehicle.current_hp, 3,
            "retail recovery does not write saved vehicle HP"
        );
        assert_eq!(
            vehicle.current_skill_uses,
            if yes { vehicle.max_skill_uses } else { [0; 8] }
        );
    }
    for who in [CharId(0), CharId(1)] {
        let stats = rt.game().roster().get(who).unwrap();
        assert_eq!(stats.curr_hp, if yes { stats.max_hp } else { 1 });
        assert_eq!(stats.curr_tp, if yes { stats.max_tp } else { 1 });
        assert_eq!(
            stats.curr_skill_uses,
            if yes { stats.max_skill_uses } else { [0; 8] }
        );
    }
    if !yes {
        let mut expected = before;
        // Both answers latch the house trigger until the party leaves.
        expected.temp_flags[3] |= 0x80;
        assert_eq!(rt.game().snapshot(), expected);
    }
}

#[test]
fn house_yes_recovers_the_party() {
    run_house(true, false);
}
#[test]
fn house_no_leaves_resources_untouched() {
    run_house(false, false);
}
#[test]
fn unanswered_scene_waits_and_later_no_survives_window_close() {
    run_house(false, true);
}

#[test]
fn paid_inn_recovers_the_same_character_and_vehicle_use_banks() {
    let Some(mut rt) = fixture() else {
        return;
    };
    let before = rt.game().snapshot();
    assert!(matches!(
        rt.shop_stay(5, 0),
        psiv_runtime::InnResult::Stayed {
            cost: 10,
            party_slots: 2
        }
    ));
    assert_eq!(rt.game().money(), before.money - 10);
    for who in [CharId(0), CharId(1)] {
        let stats = rt.game().roster().get(who).unwrap();
        assert_eq!(stats.curr_hp, stats.max_hp);
        assert_eq!(stats.curr_tp, stats.max_tp);
        assert_eq!(stats.curr_skill_uses, stats.max_skill_uses);
    }
    for vehicle in rt.game().vehicles() {
        assert_eq!(vehicle.current_skill_uses, vehicle.max_skill_uses);
        assert_eq!(vehicle.current_hp, 3);
    }
}
