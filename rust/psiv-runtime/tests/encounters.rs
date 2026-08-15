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
