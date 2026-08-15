//! Encounters end to end against the real pack: walk an encounter-enabled
//! map until the 1-in-32 roll fires, and check the formation came from that
//! map's own group.

use std::path::Path;

use psiv_core::{Cell, CollisionType, Direction, Input, StepFrames};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::{EncounterClock, Runtime, RuntimeEvent};

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

#[test]
fn walking_an_encounter_map_rolls_a_formation_from_its_group() {
    if !Path::new(PACK).join("battle").is_dir() {
        eprintln!("pack battle section not present; skipping");
        return;
    }
    let files = BattleFiles::load(Path::new(PACK)).expect("battle files load");
    let data = GameData::load(Path::new(PACK)).expect("pack loads");

    // Pick a group-bound map and a spot where two vertically adjacent cells
    // are walkable, unsuppressed, and inside the map — derived from the pack
    // rather than hard-coded so a re-extraction cannot silently break this.
    let mut chosen = None;
    'maps: for binding in &files.formations.map_bindings {
        if binding.mode != "group" {
            continue;
        }
        let Some(record) = data.map(psiv_data::MapId(binding.map_id)) else {
            continue;
        };
        let grid = &record.collision.grid;
        for y in 1..grid.height().saturating_sub(2) {
            for x in 1..grid.width().saturating_sub(1) {
                let a = Cell::new(x as u16, y as u16);
                let b = Cell::new(x as u16, (y + 1) as u16);
                let walkable = |cell: Cell| {
                    let index = cell.y as usize * grid.width() as usize + cell.x as usize;
                    grid.cells()
                        .get(index)
                        .is_some_and(|c| CollisionType::from_raw(c.code()) == CollisionType::Normal)
                };
                if walkable(a) && walkable(b) {
                    // Reject spots the cartridge suppresses; the runtime
                    // exposes the same predicate through EncounterClock.
                    let map = psiv_runtime::field_map(record).expect("map converts");
                    if !EncounterClock::suppressed(&map, a)
                        && !EncounterClock::suppressed(&map, b)
                        && map.npc_at(a).is_none()
                        && map.npc_at(b).is_none()
                    {
                        chosen = Some((binding.map_id, binding.group.unwrap(), a));
                        break 'maps;
                    }
                }
            }
        }
    }
    let (map_id, group, spawn) = chosen.expect("some group-bound map has open floor");

    let mut rt =
        Runtime::new(data, map_id, spawn, Direction::Down, StepFrames::default()).expect("spawn");
    rt.enable_battles(&files).expect("battle data converts");

    let group_formations: Vec<u16> = files
        .formations
        .encounter_groups
        .as_ref()
        .expect("groups present")
        .groups
        .iter()
        .find(|g| g.group == group)
        .expect("the binding's group exists")
        .formation_ids
        .clone();

    // Pace up and down; each landing decrements the grace counter, and from
    // the tenth step on every landing rolls 1-in-32. 4000 steps of 8 frames
    // make a missing encounter astronomically unlikely (p < 1e-50).
    let mut rolled = None;
    let mut dir = Direction::Down;
    'walk: for _ in 0..32_000 {
        for event in rt.tick(Input::Direction(dir)) {
            match event {
                RuntimeEvent::EncounterRolled { formation } => {
                    rolled = Some(formation);
                    break 'walk;
                }
                RuntimeEvent::StepCompleted { .. } => {
                    dir = match dir {
                        Direction::Down => Direction::Up,
                        _ => Direction::Down,
                    };
                }
                RuntimeEvent::SceneStarted { .. } => {
                    panic!("chosen spot fired a story trigger; pick logic is wrong")
                }
                _ => {}
            }
        }
    }
    let formation = rolled.expect("an encounter fired within 4000 steps");
    assert!(
        group_formations.contains(&formation),
        "formation {formation} is not in map {map_id:#05x}'s group {group}"
    );
}

#[test]
fn a_battle_round_trips_through_the_roster() {
    if !Path::new(PACK).join("battle").is_dir() {
        eprintln!("pack battle section not present; skipping");
        return;
    }
    let files = BattleFiles::load(Path::new(PACK)).expect("battle files load");
    let data = GameData::load(Path::new(PACK)).expect("pack loads");

    // Spawn at game start (Chaz alone), enable battles: the roster seats all
    // eleven from the pack through the one seating path.
    let start = data.manifest().game_start.clone().expect("game start");
    let mut rt = Runtime::new(
        data,
        start.map.id,
        Cell::new(start.x_cell as u16, start.y_cell as u16),
        Direction::Down,
        psiv_core::StepFrames::default(),
    )
    .expect("spawn");
    rt.enable_battles(&files).expect("battles enable");

    let party = rt.battle_party();
    assert_eq!(party.len(), 1, "Chaz alone at first control");
    assert_eq!(party[0].name, "Chaz");
    let hp_before = party[0].stats.curr_hp;
    let exp_before = party[0].stats.experience;

    // The scout's worked example formation: 2x ZoranBult is anything from
    // the basement group; use the first formation the basement group lists.
    let group = files
        .formations
        .encounter_groups
        .as_ref()
        .unwrap()
        .groups
        .iter()
        .find(|g| !g.formation_ids.is_empty())
        .unwrap();
    let formation = group.formation_ids[0];

    let events = rt.start_battle(formation, party).expect("battle starts");
    assert!(!events.is_empty(), "the opening emits a timeline");
    // Mash attack until it ends, bounded.
    let mut ended = None;
    for _ in 0..200 {
        let round = rt
            .battle_round(&psiv_core::battle::RoundOrders::attack_all())
            .expect("round resolves");
        for event in &round {
            if let psiv_core::battle::BattleEvent::Ended { outcome } = event {
                ended = Some(*outcome);
            }
        }
        if ended.is_some() {
            break;
        }
    }
    let outcome = ended.expect("the battle ends within 200 rounds");

    // Absorb + award: the roster's record IS the battle's record afterward.
    rt.finish_battle_absorbing(5);
    assert!(!rt.battle_active());
    let after = rt.battle_party();
    assert_eq!(after.len(), 1);
    match outcome {
        psiv_core::battle::Outcome::Victory => {
            assert!(
                after[0].stats.experience > exp_before,
                "victory pays experience through the roster"
            );
        }
        _ => eprintln!("non-victory outcome {outcome:?}; persistence still checked"),
    }
    // Damage taken in battle walks out to the field: hp must be <= before,
    // and whatever it is, it is the ROSTER's copy that says so.
    assert!(after[0].stats.curr_hp <= hp_before);
}
