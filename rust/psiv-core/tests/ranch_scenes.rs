//! Scene outcomes for the ranch owner (#81): `$70` and `$72`. Every case cites
//! the retail bytes (`docs/scenes/142_RanchOwner.md`).

mod common;

use common::map_with;
use psiv_core::{
    ActorRef, Cell, Direction, EventIndex, Flag, GameState, SceneEffect, SceneInput, SceneRunner,
    ScriptedActor, StepFrames, scene_for,
};

const OWNER: ActorRef = ActorRef::Npc(0);

fn play(event: u16, state: &mut GameState) -> Vec<SceneEffect> {
    play_from(event, state, Cell::new(19, 45)).0
}

/// Plays `event` with the owner at `owner`, two rows above the leader at
/// (20,47). Returns the log and the tick (from 0) each effect came on.
fn play_from(event: u16, state: &mut GameState, owner: Cell) -> (Vec<SceneEffect>, Vec<usize>) {
    let row = ".".repeat(64);
    let rows = vec![row.as_str(); 64];
    let map = map_with(&rows, vec![], vec![]);
    let scene = scene_for(EventIndex(event)).unwrap_or_else(|| panic!("${event:04X} unregistered"));
    let cast = vec![
        ScriptedActor::new(ActorRef::PartyMember(0), Cell::new(20, 47), Direction::Up),
        ScriptedActor::new(OWNER, owner, Direction::Down),
    ];
    let mut runner = SceneRunner::new(scene.ops, cast, StepFrames::new(8).unwrap());
    let mut log = Vec::new();
    let mut ticks = Vec::new();
    let mut input = SceneInput::None;
    for tick in 0..1_000 {
        let effects = runner.tick(&map, state, input);
        input = if effects
            .iter()
            .any(|e| matches!(e, SceneEffect::DialogueOpen(_)))
        {
            SceneInput::DialogueClosed
        } else {
            SceneInput::None
        };
        ticks.extend(std::iter::repeat_n(tick, effects.len()));
        log.extend(effects);
        if runner.is_finished() {
            break;
        }
    }
    assert!(runner.is_finished(), "${event:04X} did not finish");
    assert!(
        !log.iter().any(|e| matches!(e, SceneEffect::Faulted(_))),
        "{log:?}"
    );
    (log, ticks)
}

/// The tick `$20` opened on.
fn line_tick(log: &[SceneEffect], ticks: &[usize]) -> usize {
    let at = log
        .iter()
        .position(|e| *e == SceneEffect::DialogueOpen(psiv_core::DialogueId(0x20)))
        .unwrap_or_else(|| panic!("line $20 in {log:?}"));
    ticks[at]
}

/// `$07259E..$0725B1`: the X words are compared before the first step, so an
/// owner already level with the leader turns and speaks on the tick the scene
/// starts, with no walk, as the cartridge spends no frame on the loop.
#[test]
fn an_owner_already_level_speaks_without_a_frame() {
    let mut state = GameState::new();
    let (log, ticks) = play_from(0x70, &mut state, Cell::new(20, 45));
    assert_eq!(line_tick(&log, &ticks), 0, "{log:?}");
    let line = log
        .iter()
        .position(|e| *e == SceneEffect::DialogueOpen(psiv_core::DialogueId(0x20)))
        .unwrap();
    assert!(
        !log[..line]
            .iter()
            .any(|e| matches!(e, SceneEffect::ActorMoveStarted { actor: OWNER, .. })),
        "{log:?}"
    );
    assert!(state.is_set(Flag::event(0x1A)));
}

/// The control: a cell off the leader's column, the owner walks that cell
/// first, so the line comes a step's worth of ticks later.
#[test]
fn an_owner_off_the_column_walks_before_the_line() {
    let mut state = GameState::new();
    let (log, ticks) = play_from(0x70, &mut state, Cell::new(21, 45));
    let line = log
        .iter()
        .position(|e| *e == SceneEffect::DialogueOpen(psiv_core::DialogueId(0x20)))
        .unwrap();
    assert!(
        log[..line].contains(&SceneEffect::ActorArrived {
            actor: OWNER,
            at: Cell::new(20, 45),
        }),
        "{log:?}"
    );
    assert!(line_tick(&log, &ticks) >= 8, "{log:?}");
}

/// `$07259E..$072653`: level with the leader, the line, both blocks gone, the
/// routine, the walk to (`$110,$2D0`), down, and `$1A` last.
#[test]
fn the_owner_steps_out_and_stands_aside() {
    let mut state = GameState::new();
    let log = play(0x70, &mut state);
    let at = |wanted: &SceneEffect| {
        log.iter()
            .position(|e| e == wanted)
            .unwrap_or_else(|| panic!("{wanted:?} in {log:?}"))
    };
    let order = [
        at(&SceneEffect::ActorMoveStarted {
            actor: OWNER,
            to: Cell::new(20, 45),
        }),
        at(&SceneEffect::DialogueOpen(psiv_core::DialogueId(0x20))),
        at(&SceneEffect::NpcDespawned {
            npc_index: 8,
            count: 2,
        }),
        at(&SceneEffect::ObjectRoutineSet {
            npc_index: 0,
            routine: 0x194,
        }),
        at(&SceneEffect::ActorMoveStarted {
            actor: OWNER,
            to: Cell::new(0x11, 0x2E),
        }),
        at(&SceneEffect::ActorArrived {
            actor: OWNER,
            at: Cell::new(0x11, 0x2E),
        }),
        at(&SceneEffect::FlagChanged {
            flag: Flag::event(0x1A),
            value: true,
        }),
    ];
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{order:?} {log:?}");
    let turn = log
        .iter()
        .rposition(|e| {
            matches!(
                e,
                SceneEffect::ActorFaced {
                    actor: OWNER,
                    facing: Direction::Down,
                }
            )
        })
        .expect("the last turn");
    assert!(turn > order[5] && turn < order[6]);
}

/// `$072678`: turn, line `$21`, flag `$1C`.
#[test]
fn after_the_worm_he_thanks_the_party() {
    let mut state = GameState::new();
    let log = play(0x72, &mut state);
    assert!(log.contains(&SceneEffect::DialogueOpen(psiv_core::DialogueId(0x21))));
    assert!(state.is_set(Flag::event(0x1C)));
    assert!(
        log.iter()
            .any(|e| matches!(e, SceneEffect::ActorFaced { actor: OWNER, .. }))
    );
}
