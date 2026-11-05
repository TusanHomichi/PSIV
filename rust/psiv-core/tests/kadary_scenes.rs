//! Scene outcomes for Kadary (#81): `$24`, the fainting priest, and `$31`, the
//! Zio believer (`docs/scenes/164_KadaryPriestAndBeliever.md`).

mod common;

use common::map_with;
use psiv_core::{
    ActorRef, Cell, Direction, EventIndex, GameState, ObjectOp, PresentationOp, SceneEffect,
    SceneInput, SceneOp, SceneRunner, ScriptedActor, StepFrames, scene_for,
};

/// An open 40-cell room.
fn room() -> psiv_core::FieldMap {
    let row = ".".repeat(40);
    let rows: Vec<&str> = (0..40).map(|_| row.as_str()).collect();
    map_with(&rows, vec![], vec![])
}

/// Runs `ops` with the leader at (31, 23) and objects 0 to 9, object `n` at
/// cell (n + 20, 21) except object 9 at `nine`, answering each window at once.
fn play(ops: &'static [SceneOp], nine: Cell) -> (Vec<(usize, SceneEffect)>, SceneRunner) {
    let mut cast = vec![ScriptedActor::new(
        ActorRef::PartyMember(0),
        Cell::new(31, 23),
        Direction::Up,
    )];
    for n in 0..10 {
        let cell = if n == 9 {
            nine
        } else {
            Cell::new(n as u16 + 20, 21)
        };
        cast.push(ScriptedActor::new(ActorRef::Npc(n), cell, Direction::Down));
    }
    let mut runner = SceneRunner::new(ops, cast, StepFrames::new(8).unwrap());
    let map = room();
    let mut state = GameState::new();
    let mut log = Vec::new();
    let mut input = SceneInput::None;
    for tick in 0..2_000 {
        let effects = runner.tick(&map, &mut state, input);
        input = SceneInput::None;
        for effect in &effects {
            match effect {
                SceneEffect::DialogueOpen(_) | SceneEffect::DialogueResume => {
                    input = SceneInput::DialogueClosed;
                }
                SceneEffect::Faulted(fault) => panic!("{fault:?}"),
                _ => {}
            }
        }
        log.extend(effects.into_iter().map(|e| (tick, e)));
        if runner.is_finished() {
            return (log, runner);
        }
    }
    panic!("the scene did not finish");
}

fn ops(event: u16) -> &'static [SceneOp] {
    scene_for(EventIndex(event)).expect("registered").ops
}

fn tick_of(log: &[(usize, SceneEffect)], wanted: impl Fn(&SceneEffect) -> bool) -> usize {
    log.iter()
        .find(|(_, e)| wanted(e))
        .map(|(t, _)| *t)
        .expect("effect present")
}

/// `$06D616..$06D6C7`: entry `$45`, the faint for 46 frames, the block in
/// slot 10 at `($200,$140)`, the resume, and `$38` for both objects.
#[test]
fn the_priest_faints_and_a_block_fills_his_second_cell() {
    let (log, _) = play(ops(0x24), Cell::new(31, 21));
    let routine = tick_of(&log, |e| {
        *e == SceneEffect::ObjectRoutineSet {
            npc_index: 9,
            routine: 0x180,
        }
    });
    let block = tick_of(&log, |e| matches!(e, SceneEffect::ObjectSpawned { .. }));
    assert_eq!(block - routine, 46, "the faint's frames");
    assert!(log.iter().any(|(_, e)| *e
        == SceneEffect::ObjectSpawned {
            slot: 10,
            routine: 0,
            cell: Cell::new(0x200 / 16, 0x140 / 16 + 1),
            solid: true,
        }));
    for npc in [10, 9] {
        assert!(
            log.iter().any(|(_, e)| *e
                == SceneEffect::NpcDialogueSet {
                    npc_index: npc,
                    dialogue_id: 0x38,
                }),
            "object {npc} answers $38"
        );
    }
    assert!(log.iter().any(|(_, e)| *e == SceneEffect::DialogueResume));
}

/// `$06EE5E..$06EEB5`: object 9 takes object 4's cell, object 4 is cleared,
/// and 30 frames pass before entry `$1F`.
#[test]
fn the_believer_is_replaced_where_he_stands() {
    let (log, runner) = play(ops(0x31), Cell::new(5, 5));
    let spawned = log
        .iter()
        .find_map(|(t, e)| match e {
            SceneEffect::ObjectSpawned {
                slot: 9,
                routine: 0x148,
                cell,
                solid: true,
            } => Some((*t, *cell)),
            _ => None,
        })
        .expect("slot 9 spawned");
    assert_eq!(spawned.1, Cell::new(24, 21), "object 4's cell");
    assert_eq!(
        runner.actor(ActorRef::Npc(9)).unwrap().cell,
        Cell::new(24, 21)
    );
    assert!(log.iter().any(|(_, e)| *e
        == SceneEffect::NpcDespawned {
            npc_index: 4,
            count: 1
        }));
    let last_line = tick_of(
        &log,
        |e| matches!(e, SceneEffect::DialogueOpen(id) if id.0 == 0x1F),
    );
    assert_eq!(last_line - spawned.0, 30, "the writhing frames");
}

/// The first pass of `$06EE90`'s loop draws the sprites: object 9 is on screen
/// at object 4's cell, and object 4 gone, for all 30 frames and the line
/// after, not only once the scene ends. The rebuild comes after the spawn and
/// the clear, on the spawn's tick, before any frame of the wait.
#[test]
fn the_writhing_believer_is_drawn_before_he_writhes() {
    let (log, _) = play(ops(0x31), Cell::new(5, 5));
    let position = |wanted: &dyn Fn(&SceneEffect) -> bool| {
        log.iter()
            .position(|(_, e)| wanted(e))
            .expect("effect present")
    };
    let spawned = position(&|e| matches!(e, SceneEffect::ObjectSpawned { slot: 9, .. }));
    let cleared = position(&|e| {
        *e == SceneEffect::NpcDespawned {
            npc_index: 4,
            count: 1,
        }
    });
    let rebuilt = position(&|e| {
        *e == SceneEffect::Presentation {
            op: SceneOp::Presentation {
                op: PresentationOp::RebuildSprites,
            },
        }
    });
    assert!(spawned < cleared && cleared < rebuilt, "{log:?}");
    assert_eq!(log[rebuilt].0, log[spawned].0, "drawn on the spawn's tick");
    let last_line = tick_of(
        &log,
        |e| matches!(e, SceneEffect::DialogueOpen(id) if id.0 == 0x1F),
    );
    assert_eq!(
        last_line - log[rebuilt].0,
        30,
        "drawn through the 30 frames"
    );
}

/// `ObjectOp::SpawnAt` on its own: the slot takes the named actor's live cell,
/// and an actor missing from the cast faults rather than guessing a place.
#[test]
fn spawn_at_takes_the_named_actors_cell() {
    static OPS: [SceneOp; 1] = [SceneOp::Object(ObjectOp::SpawnAt {
        slot: 3,
        routine: 0x148,
        at: ActorRef::PartyMember(0),
        solid: false,
    })];
    let cast = vec![ScriptedActor::new(
        ActorRef::PartyMember(0),
        Cell::new(7, 9),
        Direction::Up,
    )];
    let mut runner = SceneRunner::new(&OPS, cast, StepFrames::new(8).unwrap());
    let effects = runner.tick(&room(), &mut GameState::new(), SceneInput::None);
    assert!(effects.contains(&SceneEffect::ObjectSpawned {
        slot: 3,
        routine: 0x148,
        cell: Cell::new(7, 9),
        solid: false,
    }));
    assert_eq!(
        runner.actor(ActorRef::Npc(3)).unwrap().cell,
        Cell::new(7, 9)
    );

    static MISSING: [SceneOp; 1] = [SceneOp::Object(ObjectOp::SpawnAt {
        slot: 3,
        routine: 0x148,
        at: ActorRef::Npc(8),
        solid: false,
    })];
    let mut runner = SceneRunner::new(&MISSING, Vec::new(), StepFrames::new(8).unwrap());
    let effects = runner.tick(&room(), &mut GameState::new(), SceneInput::None);
    assert!(
        effects.iter().any(|e| matches!(e, SceneEffect::Faulted(_))),
        "{effects:?}"
    );
}
