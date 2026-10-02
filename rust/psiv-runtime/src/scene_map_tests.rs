//! Real map chunks, ordinary interaction and both directions of a ride.
use super::*;
use psiv_core::{Input, MapId};

/// The pack this target builds against. `PSIV_RUNTIME_PACK` lets a lane test a
/// pack it built itself (the same escape hatch `dialogue::glue::tests` uses).
fn pack_dir() -> std::path::PathBuf {
    match std::env::var_os("PSIV_RUNTIME_PACK") {
        Some(path) => std::path::PathBuf::from(path),
        None => std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("runtime-pack"),
    }
}

fn fixture(map: u16, cell: Cell) -> Runtime {
    let data = GameData::load(&pack_dir()).unwrap();
    Runtime::new(data, map, cell, Direction::Up, StepFrames::default()).unwrap()
}

fn checked_tick(rt: &mut Runtime, input: Input) -> Vec<RuntimeEvent> {
    let events = rt.tick(input);
    assert!(
        !events.iter().any(|event| matches!(
            event,
            RuntimeEvent::SceneFaulted { .. }
                | RuntimeEvent::SceneMissing { .. }
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

/// H19: a scene's tile writes must reach the *live* map's collision.
///
/// `Event_JuzaDefeated` rewrites the stair chunks itself — `GetMapLayoutOffset`
/// with `RefreshPlaneBG`, at `$06FB74` — which is how the cartridge makes the
/// stairway walkable without a map reload: `MapDataManager` runs at map load
/// only (`docs/field/MAP_EFFECTS.md` §2), so a flag write alone would leave
/// the shut layout in place. The scene used to record those four tile groups
/// as presentation, and the party stood in the stairs' warp rectangle unable
/// to trigger it.
#[test]
fn juzas_stairs_open_on_the_live_map_when_the_event_writes_them() {
    if !pack_dir().join("manifest.json").is_file() {
        eprintln!("pack absent; skipping");
        return;
    }
    // Juza `$41` set and Juza Defeated `$48` clear: the state the party fights
    // him in, where the map effect writes the closed chunk and the blocker
    // object is still present.
    let mut rt = fixture(0x87, Cell::new(47, 57));
    rt.game.set(Flag::event(0x41)).unwrap();
    assert!(rt.game.is_clear(Flag::event(0x48)));
    let stairs = psiv_core::CellRect::new(24, 18, 2, 2);
    assert!(
        rt.map
            .warp_at(Cell::new(24, 18), psiv_core::WarpTrigger::MapChange)
            .is_some(),
        "the stairs' map-change warp covers (24,18)"
    );
    assert!(
        !rt.map.is_walkable(Cell::new(24, 18))
            || rt.map.collision_at(Cell::new(24, 18)) != Some(psiv_core::CollisionType::MapChange),
        "shut before the event: {:?}",
        rt.map.collision_at(Cell::new(24, 18))
    );

    assert!(rt.start_event(0x0041), "Juza defeated starts");
    let mut refreshes = Vec::new();
    for _ in 0..4000 {
        for event in checked_tick(&mut rt, Input::Neutral) {
            if event == RuntimeEvent::MapRefreshed {
                refreshes.push(rt.effects.chunk_patches.clone());
            }
        }
        if !rt.scene_active() {
            break;
        }
    }
    assert!(!rt.scene_active(), "the event finished");
    assert!(rt.game.is_set(Flag::event(0x48)));
    assert_eq!(refreshes.len(), 1, "the restore refreshes once");
    assert!(
        !refreshes[0].iter().any(|&(x, y, _)| (x, y) == (12, 9)),
        "the map effect's own closed-chunk write is gone: {:?}",
        refreshes[0]
    );
    assert_eq!(
        rt.map.collision_at(Cell::new(24, 18)),
        Some(psiv_core::CollisionType::MapChange),
        "the stairs are walkable the moment the scene ends"
    );
    assert_eq!(
        rt.map.collision_at(Cell::new(25, 19)),
        Some(psiv_core::CollisionType::MapChange)
    );

    // The map's own effect still agrees on a fresh build.
    let record = rt
        .data
        .map(psiv_data::MapId(0x87))
        .expect("Juza room")
        .clone();
    let fresh = crate::field_map_patched(&record, Some(&rt.effects)).expect("rebuilds");
    assert_eq!(
        fresh.collision_at(Cell::new(24, 18)),
        Some(psiv_core::CollisionType::MapChange)
    );

    // Negative control: the same map, same flags, no event — the shut layout
    // and the effect's own closed-chunk write are still there, so the open
    // reading above is the scene's write and not the map's default.
    let mut shut = fixture(0x87, Cell::new(47, 57));
    shut.game.set(Flag::event(0x41)).unwrap();
    assert!(shut.game.is_clear(Flag::event(0x48)));
    assert!(shut.effects.chunk_patches.contains(&(12, 9, 22)));
    assert_ne!(
        shut.map.collision_at(Cell::new(24, 18)),
        Some(psiv_core::CollisionType::MapChange),
    );
    assert!(stairs.contains(Cell::new(24, 18)));
}

/// H19's second instance: `Event_MachineCenterAppearing` writes BG chunk
/// (57,90) with `GetMapLayoutChunkBG($39,$5A)` mid-countdown (`$06B5CE`), so
/// the Machine Center's door is on the live overworld without a battle or a
/// map reload. The port must reach it through the scene, not through a
/// convenient encounter.
#[test]
fn the_machine_center_door_opens_on_the_live_overworld() {
    if !pack_dir().join("manifest.json").is_file() {
        eprintln!("pack absent; skipping");
        return;
    }
    // Zio `$42` set (the trigger's own gate), Machine Center `$43` clear.
    let mut rt = fixture(0x00, Cell::new(114, 179));
    rt.game.set(Flag::event(0x42)).unwrap();
    let door = [
        Cell::new(114, 180),
        Cell::new(115, 180),
        Cell::new(114, 181),
        Cell::new(115, 181),
    ];
    assert!(
        rt.map
            .warp_at(door[0], psiv_core::WarpTrigger::MapChange)
            .is_some(),
        "warp 21 covers the door cell"
    );
    for cell in door {
        assert_ne!(
            rt.map.collision_at(cell),
            Some(psiv_core::CollisionType::MapChange),
            "shut before the event at {cell:?}"
        );
    }

    assert!(rt.start_event(0x0006), "the machine center appears");
    let mut seen_write = None;
    for _ in 0..4000 {
        for event in checked_tick(&mut rt, Input::Neutral) {
            if event == RuntimeEvent::MapRefreshed {
                seen_write = Some(rt.effects.chunk_patches.clone());
            }
            if matches!(
                event,
                RuntimeEvent::SceneDialogue { .. } | RuntimeEvent::SceneDialogueResume
            ) {
                rt.dialogue_closed();
            }
        }
        if !rt.scene_active() {
            break;
        }
    }
    assert!(!rt.scene_active(), "the event finished");
    assert_eq!(
        seen_write,
        Some(vec![(57, 90, 0xD3)]),
        "the scene wrote the door chunk"
    );
    assert!(rt.game.is_set(Flag::event(0x43)));
    for cell in door {
        assert_eq!(
            rt.map.collision_at(cell),
            Some(psiv_core::CollisionType::MapChange),
            "open the moment the scene ends at {cell:?}"
        );
    }

    // Negative control: without the event, the same flag state has no door.
    let mut shut = fixture(0x00, Cell::new(114, 179));
    shut.game.set(Flag::event(0x42)).unwrap();
    assert_ne!(
        shut.map.collision_at(door[0]),
        Some(psiv_core::CollisionType::MapChange),
    );
}
