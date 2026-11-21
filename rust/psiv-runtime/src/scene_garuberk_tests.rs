//! The Garuberk Tower on real maps, through ordinary interaction and walking
//! (`docs/scenes/103_GaruberkTowerDoors.md`, `104_GaruberkTowerEyes.md`).
use super::*;

/// A pack built before the door atlas cannot animate a tower door
/// (`psiv_tools.map_patches.garuberk_door_chunks`); such a pack skips here, as
/// `tests/test_garuberk_door_atlas.py` fails on it.
fn tower(map: u16, cell: Cell) -> Option<Runtime> {
    let pack = pack_dir();
    if !pack.join("manifest.json").is_file() {
        eprintln!("runtime pack absent; skipping");
        return None;
    }
    let rt = fixture(map, cell);
    let carries = rt.map_record().is_some_and(|record| {
        record
            .patch_tiles
            .as_ref()
            .is_some_and(|atlas| atlas.tiles.iter().any(|tile| tile.chunk_id == Some(0x3C)))
    });
    if !carries {
        eprintln!("the pack predates the Garuberk door atlas; rebuild it to run this test");
        return None;
    }
    Some(rt)
}

/// Check both directions without copying the cartridge's chunk-pair tables
/// into a fixture. Timing, write locations and traversal still run without the
/// optional image; the core DMA census also checks chunk values against it.
fn assert_opening(writes: &[ChunkFrame], table: usize, (x, y): (u32, u32)) {
    let rom = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Phantasy Star IV (USA).md"),
    )
    .ok();
    if rom.is_none() {
        eprintln!("ROM absent; checking door timing/locations and traversal only");
    }
    assert_eq!(writes.len(), 9);
    for (index, (tick, patches)) in writes.iter().enumerate() {
        assert_eq!(*tick - writes[0].0, 4 * index, "row {index}");
        for (column, position) in [(1, (x, y - 1)), (2, (x, y))] {
            let patch = patches.iter().find(|&&(px, py, _)| (px, py) == position);
            let &(_, _, chunk) = patch.expect("both door chunks must be written");
            if let Some(rom) = &rom {
                assert_eq!(chunk, u16::from(rom[table + 3 * index + column]));
            }
        }
    }
}

/// `Event_GaruberkTwDoorOpening1` (`$35`) steps `loc_6F4D4` at the leader:
/// nine rows four frames apart, the door chunk (17,9) and the one above it.
/// A second Speak at the open door is the guard's silent return
/// (`cmpi.b #$38, (a1)`). Walking into the door fires `$22`, whose `$37` takes
/// the tower's own transition to Part2, opens the arrival door around the
/// party, walks it one cell out and shuts it (`loc_6F714`).
#[test]
fn a_garuberk_door_opens_takes_the_party_through_and_shuts_behind_it() {
    let Some(mut rt) = tower(0x199, Cell::new(34, 20)) else {
        return;
    };
    assert!(!rt.map.is_walkable(Cell::new(34, 19)), "the door is shut");
    let writes = open(&mut rt);
    assert_opening(&writes, 0x6F4D4, (17, 9));
    assert!(
        rt.map.is_walkable(Cell::new(34, 19)),
        "the door stands open"
    );

    checked_tick(&mut rt, Input::Neutral);
    assert!(
        checked_tick(&mut rt, Input::Action).is_empty(),
        "Speak at an open door is the guard's silent return"
    );

    let mut started = false;
    for _ in 0..240 {
        let input = if started {
            Input::Neutral
        } else {
            Input::Direction(Direction::Up)
        };
        // The step onto the open door lands on a type-1 cell, and the scene
        // `RunEvent_EnterGrbkTwDoor` starts owns that frame: `RunEvents` runs
        // before `RunMapTransitions` (`ps4.asm:116768-116773`), so the
        // `WarpUnmapped` the probe reports beside the scene is no fault (the
        // runner's rule, RUNNER_LOG H24, commit 0faa91b^). Anything else stays one.
        let events = rt.tick(input);
        let door = events
            .iter()
            .any(|e| matches!(e, RuntimeEvent::SceneStarted { trigger: 0x22 }));
        assert!(
            !events.iter().any(|event| match event {
                RuntimeEvent::WarpUnmapped { .. } => !door,
                other => matches!(
                    other,
                    RuntimeEvent::SceneFaulted { .. }
                        | RuntimeEvent::SceneMissing { .. }
                        | RuntimeEvent::MapRefreshFailed { .. }
                        | RuntimeEvent::UnpackedTarget { .. }
                ),
            }),
            "unexpected event: {events:?}"
        );
        started |= door;
        if started && !rt.scene_active() {
            break;
        }
    }
    assert!(started, "RunEvent_EnterGrbkTwDoor fired");
    assert_eq!(rt.map_id(), MapId(0x19A));
    assert_eq!(
        rt.state().cell(),
        Cell::new(36, 46),
        "one cell below the arrival door"
    );
    assert_eq!(
        rt.effects.chunk_patches[rt.effects.chunk_patches.len() - 2..],
        [(18, 21, 0x35), (18, 22, 0x38)],
        "the arrival door shut again"
    );
    assert!(!rt.map.is_walkable(Cell::new(36, 45)));
    assert!(
        rt.party
            .members()
            .iter()
            .all(|m| m.cell == rt.state().cell())
    );
}

/// The reverse path exercises opening `$36` and entry `$38`, including the
/// final closing row's collision. Core's DMA census checks every row's wait
/// and update policy; this checks those writes still permit ordinary traversal.
#[test]
fn the_other_garuberk_door_returns_through_the_same_warp() {
    let Some(mut rt) = tower(0x19A, Cell::new(36, 46)) else {
        return;
    };
    let writes = open(&mut rt);
    assert_opening(&writes, 0x6F588, (18, 22));
    assert!(rt.map.is_walkable(Cell::new(36, 45)));
    let mut started = false;
    for _ in 0..240 {
        let events = rt.tick(if started {
            Input::Neutral
        } else {
            Input::Direction(Direction::Up)
        });
        let door = events
            .iter()
            .any(|e| matches!(e, RuntimeEvent::SceneStarted { trigger: 0x22 }));
        assert!(
            !events.iter().any(|event| match event {
                RuntimeEvent::WarpUnmapped { .. } => !door,
                other => matches!(
                    other,
                    RuntimeEvent::SceneFaulted { .. }
                        | RuntimeEvent::SceneMissing { .. }
                        | RuntimeEvent::MapRefreshFailed { .. }
                        | RuntimeEvent::UnpackedTarget { .. }
                ),
            }),
            "{events:?}"
        );
        started |= door;
        if started && !rt.scene_active() {
            break;
        }
    }
    assert!(started);
    assert!(!rt.scene_active());
    assert_eq!(rt.map_id(), MapId(0x199));
    assert_eq!(rt.state().cell(), Cell::new(34, 20));
    assert_eq!(
        rt.effects.chunk_patches[rt.effects.chunk_patches.len() - 2..],
        [(17, 8, 0x30), (17, 9, 0x38)]
    );
    assert!(!rt.map.is_walkable(Cell::new(34, 19)));
    assert!(
        rt.party
            .members()
            .iter()
            .all(|m| m.cell == rt.state().cell())
    );
}

/// `Event_GaruberkTwEyeAction1` (`$39`) sets temp `$14` and, sixty frames on,
/// decompresses Part2's other layout over the live map: the variant the map's
/// own load picks while `$14` is set, so its two rooms meet at once.
#[test]
fn the_first_eye_swaps_part2_onto_its_other_layout() {
    let Some(mut rt) = tower(0x19A, Cell::new(20, 32)) else {
        return;
    };
    assert!(!rt.map.is_walkable(Cell::new(43, 20)));
    checked_tick(&mut rt, Input::Neutral);
    checked_tick(&mut rt, Input::Direction(Direction::Up));
    let events = checked_tick(&mut rt, Input::Action);
    assert!(
        events.iter().any(|e| matches!(
            e,
            RuntimeEvent::SceneStartedFromInteraction { event: 0x39, .. }
        )),
        "{events:?}"
    );
    let mut seen_frames = Vec::new();
    for _ in 0..200 {
        checked_tick(&mut rt, Input::Neutral);
        let frame = rt.tile_art_frame();
        if seen_frames.last().copied() != Some(frame) {
            seen_frames.push(frame);
        }
        if !rt.scene_active() {
            break;
        }
    }
    assert!(!rt.scene_active());
    // Sixty frames = 30 even frames = 5 complete 0->1->2->1->0 ping-pong cycles.
    assert!(
        seen_frames.contains(&1) && seen_frames.contains(&2),
        "eye art cycle must visit frames 1 and 2: {seen_frames:?}"
    );
    assert_eq!(rt.tile_art_frame(), 0, "60 frames end back on frame 0");
    assert!(rt.game.is_set(Flag::temp(0x14)));
    assert_eq!(rt.effects.variant, Some(0));
    assert!(
        rt.map.is_walkable(Cell::new(43, 20)),
        "the variant's passage"
    );
    assert_eq!(rt.state().cell(), Cell::new(20, 32), "the party stays put");
}
