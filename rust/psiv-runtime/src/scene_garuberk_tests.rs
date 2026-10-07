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
    let rows: [(u16, u16); 9] = [
        (0x30, 0x38),
        (0x31, 0x39),
        (0x32, 0x3A),
        (0x31, 0x39),
        (0x30, 0x38),
        (0x31, 0x39),
        (0x32, 0x3A),
        (0x33, 0x3B),
        (0x34, 0x3C),
    ];
    assert_eq!(writes.len(), rows.len());
    for ((tick, patches), (index, &(above, under))) in writes.iter().zip(rows.iter().enumerate()) {
        assert_eq!(*tick - writes[0].0, 4 * index, "row {index}");
        assert!(
            patches.contains(&(17, 8, above)),
            "row {index}: {patches:?}"
        );
        assert!(
            patches.contains(&(17, 9, under)),
            "row {index}: {patches:?}"
        );
    }
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
        // runner's rule, RUNNER_LOG H24). Anything else stays one.
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
    for _ in 0..200 {
        checked_tick(&mut rt, Input::Neutral);
        if !rt.scene_active() {
            break;
        }
    }
    assert!(!rt.scene_active());
    assert!(rt.game.is_set(Flag::temp(0x14)));
    assert_eq!(rt.effects.variant, Some(0));
    assert!(
        rt.map.is_walkable(Cell::new(43, 20)),
        "the variant's passage"
    );
    assert_eq!(rt.state().cell(), Cell::new(20, 32), "the party stays put");
}
