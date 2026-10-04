//! $076E00 runs on Raja Temple, with $40 meaning the next 64-chunk row.
use super::*;
use psiv_core::{CharId, CollisionType, GameState, PixelPos, RetailLocation, RetailSave};

const COORDS: [(u16, u16); 4] = [(47, 9), (48, 9), (47, 10), (48, 10)];
const FRAMES: [[u16; 4]; 5] = [
    [0x50, 0x51, 0x58, 0x59],
    [0x52, 0x53, 0x58, 0x59],
    [0x54, 0x55, 0x5A, 0x5B],
    [0x56, 0x57, 0x5C, 0x5D],
    [0x56, 0x57, 0x5E, 0x5F],
];

fn assert_frame(rt: &Runtime, frame: [u16; 4]) {
    assert_eq!(rt.map_id(), MapId(0x14C), "not the Dezolis overworld");
    for ((x, y), id) in COORDS.into_iter().zip(frame) {
        assert_eq!(
            rt.map_chunk_at(PixelPos {
                x: i32::from(x) * 32,
                y: i32::from(y) * 32
            }),
            Some(id),
            "live chunk ({x},{y})"
        );
        // From RajaTemple's $1ABDA8 chunk definitions, not Dezolis'
        // $114914: only $58, $5E, $5F impose solid ($8) in this sequence.
        let expected = if [0x58, 0x5E, 0x5F].contains(&id) {
            CollisionType::Solid
        } else {
            CollisionType::Normal
        };
        for dy in 0..2 {
            for dx in 0..2 {
                assert_eq!(
                    rt.map.collision_at(Cell::new(x * 2 + dx, y * 2 + dy)),
                    Some(expected)
                );
            }
        }
    }
}

#[test]
fn crash_landing_writes_five_live_bg_frames_before_flag_and_reload() {
    let data = GameData::load(&pack_dir()).expect("pack loads");
    let mut game = GameState::new();
    game.set_party([
        Some(CharId(0)),
        Some(CharId(5)),
        Some(CharId(3)),
        Some(CharId(7)),
        None,
    ]);
    let mut rt = Runtime::from_save(
        data,
        RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index_2: 0x18D,
                map_index: 0x18C,
                char_x: 16,
                char_y: 16,
            },
        },
        StepFrames::default(),
    )
    .unwrap();
    assert!(rt.start_event(0x800F));
    let mut frame_ticks = Vec::new();
    let mut loads = Vec::new();
    let mut adjacent = Vec::new();
    let neighbors = [Cell::new(92, 18), Cell::new(98, 20), Cell::new(94, 22)];
    for tick in 0..10_000 {
        for event in checked_tick(&mut rt, Input::Neutral) {
            if let RuntimeEvent::MapChanged { map, .. } = event {
                loads.push(map.0);
                if loads.len() == 2 {
                    assert!(
                        rt.game.is_clear(Flag::event(0x85)),
                        "first load must not preapply final frame"
                    );
                    assert_frame(&rt, FRAMES[0]);
                    adjacent = neighbors
                        .iter()
                        .map(|&cell| rt.map.collision_at(cell))
                        .collect();
                }
            }
            if event == RuntimeEvent::MapRefreshed {
                let index = frame_ticks.len();
                assert!(index < 5, "unexpected live frame");
                assert!(
                    rt.game.is_clear(Flag::event(0x85)),
                    "a flag/reload must not fake the live write"
                );
                assert_frame(&rt, FRAMES[index]);
                assert_eq!(rt.effects.chunk_patches.len(), 4);
                assert_eq!(rt.effects.patch_blits.len(), 4);
                assert_eq!(
                    neighbors
                        .iter()
                        .map(|&cell| rt.map.collision_at(cell))
                        .collect::<Vec<_>>(),
                    adjacent
                );
                frame_ticks.push(tick);
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
    assert!(!rt.scene_active(), "crash landing completes");
    assert_eq!(frame_ticks.len(), 5);
    assert!(
        frame_ticks.windows(2).all(|ticks| ticks[1] - ticks[0] == 6),
        "six updates per frame: {frame_ticks:?}"
    );
    assert_eq!(loads, [0x001, 0x14C, 0x14C]);
    assert!(rt.game.is_set(Flag::event(0x85)));
    assert!(rt.game.is_set(Flag::event(0x88)));
    assert_eq!(
        rt.game.party_members(),
        [CharId(0), CharId(5), CharId(3), CharId(7), CharId(8)]
    );
    assert_frame(&rt, FRAMES[4]);

    // Negative control: identical $85 flag alone on a loaded temple cannot
    // alter collision. MapDataManager runs only on load ($052AAE), while
    // the live animation must close the lower-right roof cell immediately.
    let mut no_write = fixture(0x14C, Cell::new(3, 24));
    no_write.game.set(Flag::event(0x85)).unwrap();
    assert_frame(&no_write, FRAMES[0]);
    assert!(no_write.map.is_walkable(Cell::new(96, 20)));
    assert!(!rt.map.is_walkable(Cell::new(96, 20)));

    // Fresh process-shaped load uses the flag hook to restore the last frame.
    let save = psiv_core::RetailSlot::from_bytes(&rt.slot_bytes(0).unwrap(), 0)
        .unwrap()
        .decode()
        .unwrap();
    let reloaded = Runtime::from_save(rt.data.clone(), save, StepFrames::default()).unwrap();
    assert_frame(&reloaded, FRAMES[4]);
}
