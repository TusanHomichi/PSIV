use super::*;
use psiv_core::battle::status;
use psiv_core::{Input, MapId};
use std::path::Path;

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

fn fixture() -> Option<Runtime> {
    if !Path::new(PACK).join("manifest.json").is_file() {
        return None;
    }
    let mut rt = Runtime::new_game(
        GameData::load(Path::new(PACK)).unwrap(),
        StepFrames::default(),
    )
    .unwrap();
    rt.enable_battles(&psiv_data::BattleFiles::load(Path::new(PACK)).unwrap())
        .unwrap();
    rt.battles = None;
    rt.game.set(Flag::event(0x0C)).unwrap();
    rt.game
        .set_party([Some(CharId(0)), Some(CharId(6)), None, None, None]);
    rt.resize_party();
    let chaz = rt.game.roster_mut().get_mut(CharId(0)).unwrap();
    chaz.curr_hp = 10;
    chaz.status = status::POISONED;
    let demi = rt.game.roster_mut().get_mut(CharId(6)).unwrap();
    demi.curr_hp = 0;
    demi.status = status::ANDROID_DEAD;
    rt.change_map(MapId(0), Cell::new(99, 84), Direction::Down)
        .unwrap();
    for cell in [Cell::new(99, 84), Cell::new(99, 85)] {
        assert_eq!(
            rt.map.collision_at(cell),
            Some(psiv_core::CollisionType::Normal)
        );
    }
    Some(rt)
}

fn walk(rt: &mut Runtime, direction: Direction) -> Vec<RuntimeEvent> {
    let mut events = Vec::new();
    for _ in 0..32 {
        events.extend(rt.tick(Input::Direction(direction)));
        if events
            .iter()
            .any(|e| matches!(e, RuntimeEvent::StepCompleted { .. }))
        {
            rt.tick(Input::Neutral);
            return events;
        }
    }
    panic!("no landing at {:?}: {events:?}", rt.state().cell());
}

#[test]
fn ordinary_steps_drive_poison_and_android_recovery_once() {
    let Some(mut rt) = fixture() else {
        return;
    };
    for index in 0..4 {
        let events = walk(
            &mut rt,
            if index % 2 == 0 {
                Direction::Down
            } else {
                Direction::Up
            },
        );
        assert_eq!(events.contains(&RuntimeEvent::FieldPoisonFlash), index == 3);
        assert_eq!(rt.game.roster().get(CharId(6)).unwrap().curr_hp, index + 1);
    }
    assert_eq!(rt.game.roster().get(CharId(0)).unwrap().curr_hp, 9);
    let clock = rt.field_status.clock;
    rt.set_field_suspended(true);
    for _ in 0..120 {
        rt.tick(Input::Neutral);
    }
    assert_eq!(rt.field_status.clock, clock);
    assert_eq!(rt.game.roster().get(CharId(6)).unwrap().curr_hp, 4);
    rt.set_field_suspended(false);
    for _ in 0..60 {
        rt.tick(Input::Neutral);
    }
    assert_eq!(rt.field_status.clock, clock);
}

#[test]
fn death_window_parks_the_field_and_acknowledgement_resumes_without_an_extra_step() {
    let Some(mut rt) = fixture() else {
        return;
    };
    rt.game.roster_mut().get_mut(CharId(0)).unwrap().curr_hp = 1;
    for index in 0..4 {
        walk(
            &mut rt,
            if index % 2 == 0 {
                Direction::Down
            } else {
                Direction::Up
            },
        );
    }
    assert_eq!(rt.field_notice(), Some(FieldNotice::Fallen(CharId(0))));
    let cell = rt.state().cell();
    let clock = rt.field_status.clock;
    for _ in 0..100 {
        assert!(rt.tick(Input::Direction(Direction::Down)).is_empty());
    }
    assert_eq!(rt.state().cell(), cell);
    assert_eq!(rt.field_status.clock, clock);
    rt.acknowledge_field_notice();
    assert_eq!(rt.field_notice(), None);
    assert!(
        !rt.game_over(),
        "a shut-down android is not human Dead in this field routine"
    );
    rt.tick(Input::Direction(Direction::Down));
    assert_eq!(rt.state().cell(), cell);
    assert_eq!(
        rt.field_status.clock, clock,
        "resume only completes deferred control checks"
    );
    assert_eq!(rt.game.roster().get(CharId(6)).unwrap().curr_hp, 4);
}

#[test]
fn perished_requires_its_own_acknowledgement_and_never_rewrites_the_save() {
    let Some(mut rt) = fixture() else {
        return;
    };
    rt.game.set_party([Some(CharId(0)), None, None, None, None]);
    rt.resize_party();
    rt.game.roster_mut().get_mut(CharId(0)).unwrap().curr_hp = 1;
    let dir = std::env::temp_dir().join(format!("psiv-perished-{}", std::process::id()));
    let saved = rt.save_slot(&dir, 0).unwrap();
    let before = std::fs::read(&saved).unwrap();
    for index in 0..4 {
        walk(
            &mut rt,
            if index % 2 == 0 {
                Direction::Down
            } else {
                Direction::Up
            },
        );
    }
    rt.acknowledge_field_notice();
    assert_eq!(rt.field_notice(), Some(FieldNotice::Perished));
    assert!(!rt.game_over());
    rt.acknowledge_field_notice();
    assert!(rt.game_over());
    assert_eq!(rt.game.roster().get(CharId(0)).unwrap().curr_hp, 0);
    let at = rt.state().cell();
    for _ in 0..60 {
        assert!(rt.tick(Input::Direction(Direction::Down)).is_empty());
    }
    assert_eq!(rt.state().cell(), at);
    assert_eq!(std::fs::read(&saved).unwrap(), before);
    let loaded = Runtime::load_slot(rt.data.clone(), &dir, 0, StepFrames::default()).unwrap();
    assert!(!loaded.game_over());
    assert_eq!(loaded.game.roster().get(CharId(0)).unwrap().curr_hp, 1);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn doorway_and_battle_return_reset_both_status_counters() {
    let Some(mut rt) = fixture() else {
        return;
    };
    walk(&mut rt, Direction::Down);
    assert_ne!(
        rt.field_status.clock,
        psiv_core::FieldStatusClock::default()
    );
    rt.change_map(MapId(0x13), Cell::new(31, 14), Direction::Down)
        .unwrap();
    assert_eq!(
        rt.field_status.clock,
        psiv_core::FieldStatusClock::default()
    );
    rt.update_field_status(Cell::new(31, 14), &mut Vec::new());
    assert_ne!(
        rt.field_status.clock,
        psiv_core::FieldStatusClock::default()
    );
    rt.battle_field_refresh_pending = true;
    assert_eq!(rt.return_to_field(), [RuntimeEvent::MapRefreshed]);
    assert_eq!(
        rt.field_status.clock,
        psiv_core::FieldStatusClock::default()
    );
}

#[test]
fn transition_cells_skip_status_processing_and_rng() {
    let Some(mut rt) = fixture() else {
        return;
    };
    rt.change_map(MapId(0x13), Cell::new(31, 14), Direction::Down)
        .unwrap();
    let door = (0..rt.map.height())
        .flat_map(|y| (0..rt.map.width()).map(move |x| Cell::new(x, y)))
        .find(|cell| rt.map.collision_at(*cell) == Some(psiv_core::CollisionType::MapChange))
        .unwrap();
    let before = rt.game.snapshot();
    let seed = rt.rng.clone();
    rt.update_field_status(door, &mut Vec::new());
    assert_eq!(rt.game.snapshot(), before);
    assert_eq!(rt.rng, seed);
    assert_eq!(
        rt.field_status.clock,
        psiv_core::FieldStatusClock::default()
    );
}
