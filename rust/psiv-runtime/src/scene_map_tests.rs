//! Real map chunks, ordinary interaction and both directions of a ride.
use super::*;
use std::path::Path;

fn fixture(map: u16, cell: Cell) -> Runtime {
    let data = GameData::load(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../runtime-pack"
    )))
    .unwrap();
    Runtime::new(data, map, cell, Direction::Up, StepFrames::default()).unwrap()
}

fn checked_tick(rt: &mut Runtime, input: Input) -> Vec<RuntimeEvent> {
    let events = rt.tick(input);
    assert!(
        !events.iter().any(|event| matches!(
            event,
            RuntimeEvent::SceneFaulted { .. }
                | RuntimeEvent::SceneMissing { .. }
                | RuntimeEvent::TriggerUnsupported { .. }
                | RuntimeEvent::WarpUnmapped { .. }
                | RuntimeEvent::MapRefreshFailed { .. }
                | RuntimeEvent::UnpackedTarget { .. }
        )),
        "unexpected event: {events:?}"
    );
    events
}

type ChunkFrame = (usize, Vec<(u32, u32, u16)>);

fn open(rt: &mut Runtime) -> Vec<ChunkFrame> {
    checked_tick(rt, Input::Neutral); // entry trigger scan
    checked_tick(rt, Input::Direction(Direction::Up)); // face the closed door after arrival
    let events = checked_tick(rt, Input::Action);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, RuntimeEvent::SceneStartedFromInteraction { .. })),
        "door at {:?} {:?}: {events:?}",
        rt.map_id(),
        rt.state().cell()
    );
    let mut writes = Vec::new();
    for tick in 0..100 {
        let events = checked_tick(rt, Input::Neutral);
        if events.contains(&RuntimeEvent::MapRefreshed) {
            writes.push((tick, rt.effects.chunk_patches.clone()));
        }
        if !rt.scene_active() {
            return writes;
        }
    }
    panic!("door never finished");
}

#[test]
fn birth_valley_door_animates_collision_then_warps_and_survives_reload() {
    let mut rt = fixture(0x2C, Cell::new(23, 16));
    rt.game.set(Flag::event(16)).unwrap(); // Holt is already complete.
    assert!(!rt.map.is_walkable(Cell::new(23, 15)));
    let writes = open(&mut rt);
    assert_eq!(writes.len(), 3);
    assert_eq!(writes[1].0 - writes[0].0, 17);
    assert_eq!(writes[2].0 - writes[1].0, 17);
    for (entry, ids) in writes
        .iter()
        .zip([[0x42, 0x44], [0x43, 0x45], [0x29, 0x2A]])
    {
        assert_eq!(entry.1, vec![(11, 7, ids[0]), (11, 8, ids[1])]);
    }
    assert!(rt.game.is_set(Flag::temp(0)));
    assert!(rt.map.is_walkable(Cell::new(23, 15)));
    let save = psiv_core::RetailSave {
        snapshot: rt.game.snapshot(),
        location: psiv_core::RetailLocation {
            world_index: 0,
            map_index_2: 0,
            map_index: 0x2C,
            char_x: 23 * 16,
            char_y: 16 * 16,
        },
    };
    let restored = Runtime::from_save(rt.data.clone(), save, StepFrames::default()).unwrap();
    assert!(restored.map.is_walkable(Cell::new(23, 15)));
    for _ in 0..20 {
        checked_tick(&mut rt, Input::Direction(Direction::Up));
        if rt.map_id() == MapId(0xA2) {
            break;
        }
    }
    assert_eq!(rt.map_id(), MapId(0xA2));
    assert_eq!(rt.state().cell(), Cell::new(32, 29));
}

#[test]
fn elevators_open_ride_step_out_close_and_return_using_the_real_tables() {
    let mut rt = fixture(0xA4, Cell::new(32, 18));
    for (target, arrival, door_y) in [(0xA6, 12, 5), (0xA4, 18, 8)] {
        assert!(!rt.map.is_walkable(Cell::new(32, rt.state().cell().y - 1)));
        let writes = open(&mut rt);
        assert_eq!(writes.len(), 4);
        for (index, entry) in writes.iter().enumerate() {
            assert_eq!(entry.0 - writes[0].0, index);
            assert_eq!(entry.1.last().unwrap().2, 0x50 + index as u16);
        }
        // Another confirm on an already-open door is a silent no-op.
        checked_tick(&mut rt, Input::Neutral);
        assert!(checked_tick(&mut rt, Input::Action).is_empty());
        let mut started = false;
        let mut closing = Vec::new();
        for _ in 0..140 {
            let events = checked_tick(
                &mut rt,
                if started {
                    Input::Neutral
                } else {
                    Input::Direction(Direction::Up)
                },
            );
            if events
                .iter()
                .any(|e| matches!(e, RuntimeEvent::SceneStarted { trigger: 0x0D }))
            {
                started = true;
            }
            if rt.map_id() == MapId(target) && events.contains(&RuntimeEvent::MapRefreshed) {
                closing.push(rt.effects.chunk_patches.last().unwrap().2);
            }
            if started && !rt.scene_active() {
                break;
            }
        }
        assert!(started);
        assert_eq!(rt.map_id(), MapId(target));
        assert_eq!(rt.state().cell(), Cell::new(32, arrival));
        assert_eq!(closing, vec![0x53, 0x52, 0x51, 0x50, 0x4F]);
        assert_eq!(rt.effects.chunk_patches, vec![(16, door_y, 0x4F)]);
        assert!(
            rt.party
                .members()
                .iter()
                .all(|m| m.cell == rt.state().cell())
        );
        assert!(
            checked_tick(&mut rt, Input::Neutral).is_empty(),
            "the closed arrival must not retrigger"
        );
    }
}

#[test]
fn alarm_blocks_for_eight_stages_in_each_direction_before_dialogue() {
    let mut rt = fixture(0xA3, Cell::new(28, 31));
    assert!(rt.start_event(0x12));
    let mut fade_ticks = Vec::new();
    let mut dialogue_tick = None;
    for tick in 0..100 {
        for event in checked_tick(&mut rt, Input::Neutral) {
            match event {
                RuntimeEvent::ScenePresentation {
                    op:
                        psiv_core::SceneOp::Presentation {
                            op:
                                psiv_core::PresentationOp::FadeToRed { .. }
                                | psiv_core::PresentationOp::FadeFromRed { .. },
                        },
                } => fade_ticks.push(tick),
                RuntimeEvent::SceneDialogue { .. } => {
                    dialogue_tick = Some(tick);
                }
                _ => {}
            }
        }
        if dialogue_tick.is_some() {
            break;
        }
    }
    assert_eq!(fade_ticks.len(), 2);
    assert_eq!(fade_ticks[1] - fade_ticks[0], 32);
    assert_eq!(dialogue_tick.unwrap() - fade_ticks[1], 32);
}
