//! Tyler grave timing from the US retail loop chain, without dialogue time.
use super::*;
use psiv_core::{ActorRef, PixelPos, PresentationOp, SceneOp};

#[test]
fn grave_camera_blocks_and_slab_sequences_follow_every_object_update() {
    if !pack_present() {
        return;
    }
    let mut rt = fixture(0x120, Cell::new(20, 26));
    // A return visit skips the two dialogue legs, but repeats both shakes and slide.
    rt.game.set(Flag::event(0x84)).unwrap();
    let leader = PixelPos::from_cell(rt.state().cell());
    let home = (leader.x - psiv_core::HOME_X, leader.y - psiv_core::HOME_Y);
    rt.set_camera(home.0, home.1);
    assert!(rt.install_scene(EventIndex(0x44)));
    let mut pans = Vec::new();
    let mut drift = Vec::new();
    let mut frames = 0;
    let mut writes = Vec::new();
    let mut cameras = Vec::new();
    while rt.scene_active() {
        let events = checked_tick(&mut rt, Input::Neutral);
        for event in events {
            match event {
                RuntimeEvent::ScenePresentation {
                    op: SceneOp::PanCamera { .. },
                    ..
                } => pans.push(frames),
                RuntimeEvent::ScenePresentation {
                    op:
                        SceneOp::Presentation {
                            op: PresentationOp::NpcPixelPosition { npc: 2, .. },
                        },
                    ..
                } => {
                    let animation = rt
                        .scene
                        .as_ref()
                        .unwrap()
                        .actor(ActorRef::Npc(2))
                        .unwrap()
                        .animation;
                    let right = rt
                        .scene
                        .as_ref()
                        .unwrap()
                        .actor(ActorRef::Npc(3))
                        .unwrap()
                        .animation
                        .unwrap();
                    let left = animation.unwrap();
                    assert_eq!(
                        (right.word, right.index, right.duration),
                        (0x14, left.index, left.duration)
                    );
                    drift.push((frames, animation));
                }
                RuntimeEvent::MapRefreshed => writes.push(frames),
                _ => {}
            }
        }
        let (x, y) = rt.camera().raw();
        cameras.push((x >> 16, y >> 16));
        assert_eq!(y >> 16, home.1);
        assert!((home.0..=home.0 + 4).contains(&(x >> 16)));
        frames += 1;
        assert!(frames < 200);
    }
    assert_eq!(drift.len(), 64);
    for (i, &(_, animation)) in drift.iter().enumerate() {
        let animation = animation.unwrap();
        let (word, index, duration) = (animation.word, animation.index, animation.duration);
        let update = i + 1;
        let (expected_index, expected_duration) = match update {
            1..=4 => (0, 4 - update),
            5..=9 => (1, 9 - update),
            10..=14 => (2, 14 - update),
            _ => (3, 0x34 - (update - 15)),
        };
        assert_eq!(
            (word, usize::from(index), usize::from(duration)),
            (0x10, expected_index, expected_duration),
            "update {update}"
        );
    }
    eprintln!(
        "frames={frames} pans={pans:?} writes={writes:?} drift={}..{}",
        drift[0].0, drift[63].0
    );
    assert_eq!(pans.len(), 4);
    for (index, &frame) in pans.iter().enumerate() {
        assert_eq!(
            cameras[frame + 1],
            (home.0 + if index % 2 == 0 { 4 } else { 0 }, home.1),
            "camera pan {index} arrives on the following frame"
        );
    }
    assert_eq!(pans[1] - pans[0], 1, "outward 4px at speed4 blocks a frame");
    assert_eq!(
        drift[0].0 - pans[1],
        2,
        "return pan completes, then drift is installed before its first update"
    );
    assert_eq!(pans[3] - pans[2], 1);
    assert_eq!(
        pans[2],
        drift[63].0 + 1,
        "facing writes and second shake follow final drift frame"
    );
    assert_eq!(rt.map().npcs()[2].facing, Direction::Right);
    assert_eq!(rt.map().npcs()[3].facing, Direction::Left);
    assert_eq!(drift[63].0 - drift[0].0, 63);
    assert_eq!(
        frames - pans[3],
        62,
        "return pan completion, wait setup and 60 map-update frames"
    );
}
