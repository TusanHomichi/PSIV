//! `Cutscene_Landale` (`$8010`) on the live Dezolis map: the spaceport the
//! scene raises must be there when it ends, with no battle and no reload.
//!
//! `Event_DezoSpaceportAppearing` writes BG chunk (6,36) <- `$2C` at pass 308 of
//! its 369 (`ps4.asm:145530-145541`); that chunk's four collision cells read
//! map-change, which is what makes Dezolis warp 9 (the spaceport's door) fire.
//! Before the write was transcribed the live map kept those cells open ground,
//! so the warp fired only after a reload (campaign run C6, `kuran-arrival`).
use super::*;
use psiv_core::CollisionType;

/// Dezolis warp 9's footprint, the four cells of chunk (6,36).
const DOOR: [Cell; 4] = [
    Cell::new(12, 72),
    Cell::new(13, 72),
    Cell::new(12, 73),
    Cell::new(13, 73),
];

/// A tick and the chunk writes `effects.chunk_patches` held when the map refreshed.
type ChunkWrite = (usize, Vec<(u32, u32, u16)>);

/// Plays the scene out: every dialogue it opens is closed on the next tick.
fn play(rt: &mut Runtime) -> Vec<ChunkWrite> {
    let mut writes = Vec::new();
    for tick in 0..6000 {
        for event in checked_tick(rt, Input::Neutral) {
            if event == RuntimeEvent::MapRefreshed && !rt.effects.chunk_patches.is_empty() {
                writes.push((tick, rt.effects.chunk_patches.clone()));
            }
            if matches!(
                event,
                RuntimeEvent::SceneDialogue { .. } | RuntimeEvent::SceneDialogueResume
            ) {
                rt.dialogue_closed();
            }
        }
        if !rt.scene_active() {
            return writes;
        }
    }
    panic!("Cutscene_Landale did not finish in 6000 ticks");
}

fn landale_runtime() -> Runtime {
    // The trigger's own gates: Tyler grave `$84` set, Dezo spaceport `$82` clear.
    let mut rt = fixture(0x15F, Cell::new(26, 83));
    rt.game.set(Flag::event(0x84)).unwrap();
    rt
}

#[test]
fn the_spaceport_door_is_on_the_live_map_when_landale_ends() {
    if !pack_dir().join("manifest.json").is_file() {
        eprintln!("pack absent; skipping");
        return;
    }
    let mut rt = landale_runtime();
    assert!(rt.start_event(0x8010), "Cutscene_Landale starts");
    let writes = play(&mut rt);

    assert_eq!(rt.map_id(), MapId(1), "the scene loaded Dezolis");
    assert!(
        rt.game.is_set(Flag::event(0x82)),
        "the spaceport flag is set"
    );
    assert_eq!(
        writes.len(),
        1,
        "one chunk write in the whole scene: {writes:?}"
    );
    assert_eq!(writes[0].1, vec![(6, 36, 0x2C)], "the spaceport chunk");
    for cell in DOOR {
        assert_eq!(
            rt.map.collision_at(cell),
            Some(CollisionType::MapChange),
            "the door cell {cell:?} is live the moment the scene ends"
        );
    }
    // The live map is what the pack's own load hook for `$82` resolves to: the
    // flag-130 entry of `overworld_patches` carries chunk (6,36) with four
    // map-change cells, and a reload builds those cells from it.
    let record = rt.data.map(psiv_data::MapId(1)).expect("Dezolis");
    let hook = record
        .overworld_patches
        .as_ref()
        .and_then(|patches| patches.iter().find(|patch| patch.event_flag == 130))
        .and_then(|patch| {
            patch
                .tiles
                .iter()
                .find(|t| (t.chunk_x, t.chunk_y) == (6, 36))
        })
        .expect("the $82 hook writes chunk (6,36)");
    assert_eq!(hook.collision, [1, 1, 1, 1]);
    assert_eq!(hook.collision_chunk_id, 0x2C);
}

/// The negative control: the map with the flag clear has no door, so the cells
/// above are the scene's doing and not the map's default.
#[test]
fn without_the_scene_dezolis_has_no_spaceport_door() {
    if !pack_dir().join("manifest.json").is_file() {
        eprintln!("pack absent; skipping");
        return;
    }
    let rt = fixture(0x01, Cell::new(9, 74));
    for cell in DOOR {
        assert_ne!(
            rt.map.collision_at(cell),
            Some(CollisionType::MapChange),
            "shut before the scene at {cell:?}"
        );
    }
}

/// Walks `rt` down one cell and returns the events of the landing tick.
fn step_down(rt: &mut Runtime) -> Vec<RuntimeEvent> {
    for _ in 0..64 {
        let events = checked_tick(rt, Input::Direction(Direction::Down));
        if events.iter().any(|event| {
            matches!(
                event,
                RuntimeEvent::StepCompleted { .. } | RuntimeEvent::MapChanged { .. }
            )
        }) {
            return events;
        }
    }
    panic!("the step never completed");
}

/// `RunEvents` runs before `RunMapTransitions` on foot (`ps4.asm:116768-116773`),
/// so the Landale trigger on the Hangar's warp-0 row (x `$1A0..$1B0`, y `$520`
/// is cell row 83, the warp's first row) starts the scene and the warp does not
/// fire; the same step with the trigger's gate shut takes the warp. The
/// campaign's `step_onto` objective rests on this ordering.
#[test]
fn a_trigger_on_a_warp_cell_wins_over_the_warp() {
    if !pack_dir().join("manifest.json").is_file() {
        eprintln!("pack absent; skipping");
        return;
    }
    let mut rt = fixture(0x15F, Cell::new(26, 82));
    rt.game.set(Flag::event(0x84)).unwrap();
    let events = step_down(&mut rt);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, RuntimeEvent::SceneStarted { trigger: 43 })),
        "RunEvent_FindingLandale ($2B) starts the scene: {events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, RuntimeEvent::MapChanged { .. })),
        "the warp does not fire on the frame a scene starts: {events:?}"
    );
    assert_eq!(rt.map_id(), MapId(0x15F), "still in the Hangar");
    assert!(rt.scene_active());

    // Negative control: Dezo spaceport `$82` set shuts the trigger's gate, and
    // the very step takes warp 0 to Dezolis.
    let mut gated = fixture(0x15F, Cell::new(26, 82));
    gated.game.set(Flag::event(0x84)).unwrap();
    gated.game.set(Flag::event(0x82)).unwrap();
    let events = step_down(&mut gated);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, RuntimeEvent::MapChanged { map, .. } if *map == MapId(1))),
        "the warp fires: {events:?}"
    );
    assert!(!gated.scene_active());
}
