//! Scene outcomes for Zosa's item shop (#56): `$96`, the thief and the Pengu
//! Feed (`docs/scenes/161_PenguinFeedStolen.md`).

mod common;

use common::map_with;
use psiv_core::{
    ActorRef, Axis, Cell, Direction, EventIndex, GameState, ObjectOp, SceneEffect, SceneInput,
    SceneOp, SceneRunner, ScriptedActor, StepFrames, scene_for,
};

const PENGU_FEED: u8 = 0x92;
const THIEF: ActorRef = ActorRef::Npc(2);

/// A 48-cell strip: the thief's row (y 40) and the leader's above it.
fn map() -> psiv_core::FieldMap {
    let row = ".".repeat(48);
    let rows: Vec<&str> = (0..42).map(|_| row.as_str()).collect();
    map_with(&rows, vec![], vec![])
}

/// Runs `$96` with the leader at cell (30, 39) and two map objects, closing
/// each dialogue at once.
fn play(state: &mut GameState) -> (Vec<SceneEffect>, SceneRunner) {
    let scene = scene_for(EventIndex(0x96)).expect("$96 registered");
    let cast = vec![
        ScriptedActor::new(ActorRef::PartyMember(0), Cell::new(30, 39), Direction::Up),
        ScriptedActor::new(ActorRef::Npc(0), Cell::new(28, 25), Direction::Down),
        ScriptedActor::new(ActorRef::Npc(1), Cell::new(30, 30), Direction::Right),
    ];
    let mut runner = SceneRunner::new(scene.ops, cast, StepFrames::new(8).unwrap());
    let map = map();
    let mut log = Vec::new();
    let mut input = SceneInput::None;
    for _ in 0..5_000 {
        let effects = runner.tick(&map, state, input);
        input = SceneInput::None;
        for effect in &effects {
            match effect {
                SceneEffect::DialogueOpen(_) => input = SceneInput::DialogueClosed,
                SceneEffect::Faulted(fault) => panic!("$96: {fault:?}"),
                _ => {}
            }
        }
        log.extend(effects);
        if runner.is_finished() {
            return (log, runner);
        }
    }
    panic!("$96 did not finish");
}

/// `$07360E..$073639`: the spawn fills slot 2, which the cast did not hold,
/// at `($150,$270)`, and the thief is not solid (`bclr #3, $2(a4)`).
#[test]
fn the_thief_joins_the_cast_in_slot_two() {
    let mut state = GameState::new();
    let (log, runner) = play(&mut state);
    assert_eq!(
        log.iter()
            .find(|e| matches!(e, SceneEffect::ObjectSpawned { .. })),
        Some(&SceneEffect::ObjectSpawned {
            slot: 2,
            routine: 0x368,
            cell: Cell::new(0x150 / 16, 0x270 / 16 + 1),
            solid: false,
        })
    );
    assert!(runner.actor(THIEF).is_some(), "the cast holds it");
}

/// `$073650`: the first run ends under the leader's X; `$0736A8` the second at
/// `$2D0`; `$0736AE` clears the slot after it.
#[test]
fn the_thief_runs_under_the_leader_then_off_to_the_right() {
    let mut state = GameState::new();
    let (log, runner) = play(&mut state);
    let arrivals: Vec<Cell> = log
        .iter()
        .filter_map(|e| match e {
            SceneEffect::ActorArrived { actor, at } if *actor == THIEF => Some(*at),
            _ => None,
        })
        .collect();
    let row = 0x270 / 16 + 1;
    assert_eq!(arrivals.first(), Some(&Cell::new(30, row)));
    assert_eq!(arrivals.last(), Some(&Cell::new(0x2D0 / 16, row)));
    assert_eq!(
        runner.actor(THIEF).unwrap().cell,
        Cell::new(0x2D0 / 16, row)
    );
    let gone = log
        .iter()
        .position(|e| {
            matches!(
                e,
                SceneEffect::NpcDespawned {
                    npc_index: 2,
                    count: 1
                }
            )
        })
        .unwrap();
    let line = log
        .iter()
        .position(|e| matches!(e, SceneEffect::DialogueOpen(id) if id.0 == 0x10))
        .unwrap();
    assert!(gone < line, "the slot is clear before entry $10");
}

/// `loc_7365E`: twenty beats of down, up, right, left, then down.
#[test]
fn the_thief_spins_five_times() {
    let mut state = GameState::new();
    let (log, _) = play(&mut state);
    let facings: Vec<Direction> = log
        .iter()
        .filter_map(|e| match e {
            SceneEffect::ActorFaced { actor, facing } if *actor == THIEF => Some(*facing),
            _ => None,
        })
        .collect();
    use Direction::{Down, Left, Right, Up};
    let mut expected: Vec<Direction> = [Down, Up, Right, Left].repeat(5);
    expected.push(Down);
    assert_eq!(facings, expected);
}

/// `Event_UpdateObjFacing` ends in a `VInt_Prepare` of its own (`$05A972`, US
/// build), so each of `loc_7365E`'s twenty beats is that frame plus the two
/// after it, and the final turn to facing 0 is that frame before
/// `DoMainUpdatesLoop($1D)`. Ticks here are frames: one per `tick` call.
#[test]
fn each_turn_spends_the_helpers_frame() {
    let scene = scene_for(EventIndex(0x96)).expect("$96 registered");
    let cast = vec![
        ScriptedActor::new(ActorRef::PartyMember(0), Cell::new(30, 39), Direction::Up),
        ScriptedActor::new(ActorRef::Npc(0), Cell::new(28, 25), Direction::Down),
        ScriptedActor::new(ActorRef::Npc(1), Cell::new(30, 30), Direction::Right),
    ];
    let mut runner = SceneRunner::new(scene.ops, cast, StepFrames::new(8).unwrap());
    let (map, mut state) = (map(), GameState::new());
    let mut input = SceneInput::None;
    let mut turns = Vec::new();
    let mut end = None;
    for frame in 0..5_000_u32 {
        let effects = runner.tick(&map, &mut state, input);
        input = SceneInput::None;
        for effect in &effects {
            match effect {
                SceneEffect::ActorFaced { actor, .. } if *actor == THIEF => turns.push(frame),
                SceneEffect::ActorArrived { actor, at } if *actor == THIEF => {
                    if at.x == 0x2D0 / 16 {
                        end = Some(frame);
                    }
                }
                SceneEffect::DialogueOpen(_) => input = SceneInput::DialogueClosed,
                _ => {}
            }
        }
        if runner.is_finished() {
            break;
        }
    }
    assert_eq!(turns.len(), 21);
    // Twenty beats: turn, the helper's frame, two `VInt_Prepare`.
    for pair in turns[..20].windows(2) {
        assert_eq!(
            pair[1] - pair[0],
            3,
            "turns {pair:?} are not three frames apart"
        );
    }
    // Last beat to the final turn, then the helper's frame, then
    // `DoMainUpdatesLoop($1D)`: 30 ticks (the port's `Wait`, unchanged) and the
    // run to `$2D0` after it.
    assert_eq!(turns[20] - turns[19], 3);
    let end = end.expect("the second run arrives");
    assert!(
        end - turns[20] > 30,
        "final turn at {}, run ends at {end}",
        turns[20]
    );
}

/// `loc_736D2`: every Pengu Feed `GetItem` can find goes, and the bag closes
/// up each time.
#[test]
fn every_findable_pengu_feed_is_taken() {
    let mut state = GameState::new();
    for item in [PENGU_FEED, 0x01, PENGU_FEED, 0x02, PENGU_FEED] {
        state.inventory_mut().add(item).unwrap();
    }
    play(&mut state);
    assert_eq!(state.inventory().slots()[..3], [0x01, 0x02, 0x00]);
}

/// A hole hides the last item from `GetItem`, so a feed behind one stays, as
/// it would on the cartridge.
#[test]
fn a_pengu_feed_behind_a_hole_stays() {
    let mut state = GameState::new();
    for item in [0x01, 0x02, PENGU_FEED] {
        state.inventory_mut().add(item).unwrap();
    }
    state.inventory_mut().remove(0).unwrap();
    play(&mut state);
    assert_eq!(state.inventory().slots()[..3], [0x00, 0x02, PENGU_FEED]);
}

/// The spawn op on its own: a slot the cast already holds is moved back and
/// reset rather than added twice (a second `move.w #routine, (a4)` rewrites
/// the same object).
#[test]
fn a_second_spawn_reuses_the_slot() {
    static OPS: [SceneOp; 2] = [
        SceneOp::Object(ObjectOp::Spawn {
            slot: 1,
            routine: 0x368,
            art_tile: 0,
            x: 0x40,
            y: 0x40,
            solid: true,
        }),
        SceneOp::MoveActorToActorAxis {
            actor: ActorRef::Npc(1),
            target: ActorRef::PartyMember(0),
            axis: Axis::X,
            wait: true,
        },
    ];
    let cast = vec![
        ScriptedActor::new(ActorRef::PartyMember(0), Cell::new(10, 5), Direction::Up),
        ScriptedActor::new(ActorRef::Npc(1), Cell::new(20, 20), Direction::Left),
    ];
    let mut runner = SceneRunner::new(&OPS, cast, StepFrames::new(8).unwrap());
    let mut state = GameState::new();
    let map = map();
    let first = runner.tick(&map, &mut state, SceneInput::None);
    assert!(first.contains(&SceneEffect::ObjectSpawned {
        slot: 1,
        routine: 0x368,
        cell: Cell::new(4, 5),
        solid: true,
    }));
    assert_eq!(
        runner
            .actors()
            .iter()
            .filter(|a| a.actor == ActorRef::Npc(1))
            .count(),
        1
    );
    for _ in 0..200 {
        runner.tick(&map, &mut state, SceneInput::None);
    }
    assert!(runner.is_finished());
    assert_eq!(
        runner.actor(ActorRef::Npc(1)).unwrap().cell,
        Cell::new(10, 5)
    );
}
