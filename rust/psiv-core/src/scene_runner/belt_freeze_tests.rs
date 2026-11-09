//! The conveyor belts' walk freeze (`FieldObj_UpdateStatus`'s dead bit),
//! split from `mechanics_tests.rs` to keep both under the size limit.

use super::mechanics_tests::{CHAZ, RUNE, belt_layout, big_map, state_with};
use super::*;
use crate::{Direction, EventIndex, scene_for};

#[test]
fn a_belt_freezes_the_leader_then_each_follower_once_it_stands_on_the_belt() {
    // `Event_ConveyorBeltDown` sets status bit 2 on the leader before the
    // first frame (`$06CEA8`) and on a follower once the chunk under it is a
    // belt chunk (`$06CEEA`); the bits stay until the carry ends.
    let map = big_map();
    let layout = belt_layout(3..=3, 10..=14, 0xA8);
    let scene = scene_for(EventIndex(0x1D)).expect("registered");
    let cast = vec![
        ScriptedActor::new(ActorRef::PartyMember(0), Cell::new(6, 20), Direction::Down),
        ScriptedActor::new(ActorRef::PartyMember(1), Cell::new(6, 18), Direction::Down),
    ];
    let mut runner = SceneRunner::new(scene.ops, cast, StepFrames::default());
    let mut state = state_with(&[CHAZ, RUNE]);
    let mut leader_frozen_from = None;
    let mut follower_frozen_from = None;
    let mut ticks = 0;
    while !runner.is_finished() {
        ticks += 1;
        assert!(ticks < 2_000, "the belt did not finish");
        let _ = runner.tick_with(&map, &mut state, SceneInput::None, Some(&layout));
        if runner.belt_freezes(0) {
            leader_frozen_from.get_or_insert(ticks);
        } else if leader_frozen_from.is_some() {
            break;
        }
        let follower = runner.belt_freezes(1);
        if follower {
            follower_frozen_from.get_or_insert(ticks);
        }
        // Once set, a bit stays set while the carry lasts.
        if follower_frozen_from.is_some() && runner.belt_freezes(0) {
            assert!(follower, "tick {ticks}: the follower thawed mid-carry");
        }
        // Frozen only while standing on (or once having reached) the belt.
        if follower_frozen_from == Some(ticks) {
            let cell = runner.actor(ActorRef::PartyMember(1)).unwrap().cell;
            assert!(cell.y >= 20, "frozen off the belt at {cell:?}");
        }
    }
    assert_eq!(
        leader_frozen_from,
        Some(1),
        "the leader from the first frame"
    );
    assert!(
        follower_frozen_from.is_some_and(|tick| tick > 1),
        "the follower freezes once it reaches the belt, not at once: {follower_frozen_from:?}"
    );
    // The carry is over: every byte restored.
    assert!(!runner.belt_freezes(0) && !runner.belt_freezes(1));
}
