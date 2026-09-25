//! The scene registry, and the determinism a replay depends on.
//!
//! The registry cases run the scenes the pack ships (`scene_for`) rather than
//! synthetic op lists, and hold them to what the cartridge's own scene must do:
//! Alys joins the party, the flag that stops her trigger re-firing is set, the
//! alignment branch takes the arm its actor geometry selects. The state cases
//! pin what a scene does to `GameState` beyond the runner's own effects --
//! meseta accumulating, the trigger that needs both its flag and its rect --
//! and `Scene` carrying its provenance. The determinism cases are the promise
//! the oracle lane leans on: the same inputs replay to the same state, effect
//! log and actor set, and trigger evaluation is a pure function of state and
//! position.

use crate::common::map_with;
use crate::fixtures::{ctx, frames, join_runner};
use psiv_core::{
    ActorRef, Cell, CharId, Direction, EventIndex, Flag, GameState, PixelPos, Scene, SceneEffect,
    SceneInput, SceneOp, SceneRunner, ScriptedActor, TRIGGERS, TriggerResult, scene_for,
};

#[test]
fn the_alys_scene_puts_alys_in_front_and_sets_the_flag_that_stops_the_trigger() {
    // The whole point of the scene, and the two things the fork's copy omits.
    let map = map_with(&["........", "........", "........"], vec![], vec![]);
    let mut state = GameState::new();
    state.set_party([Some(CharId(0)), None, None, None, None]);

    let scene = scene_for(EventIndex(3)).unwrap();
    let mut runner = SceneRunner::new(
        scene.ops,
        vec![
            ScriptedActor::new(
                ActorRef::Character(CharId(0)),
                Cell::new(2, 1),
                Direction::Up,
            ),
            ScriptedActor::new(ActorRef::Npc(7), Cell::new(2, 2), Direction::Down),
        ],
        frames(),
    );

    // Run it through, answering the one dialogue.
    let mut log = Vec::new();
    for _ in 0..200 {
        let input = if matches!(log.last(), Some(SceneEffect::DialogueOpenFromNpc { .. })) {
            SceneInput::DialogueClosed
        } else {
            SceneInput::None
        };
        let effects = runner.tick(&map, &mut state, input);
        log.extend(effects);
        if runner.is_finished() {
            break;
        }
    }

    assert!(runner.is_finished(), "the scene should complete");
    assert_eq!(state.party_slot(0), Some(CharId(1)), "Alys leads");
    assert_eq!(state.party_slot(1), Some(CharId(0)), "Chaz follows");
    assert!(
        state.is_set(Flag::event(0x08)),
        "EventFlag_AlysFound must be set or the trigger re-fires forever"
    );
    assert!(
        log.contains(&SceneEffect::NpcDespawned {
            npc_index: 7,
            count: 1
        }),
        "NPC-Alys is cleared"
    );
    assert!(
        log.iter()
            .any(|e| matches!(e, SceneEffect::DialogueOpenFromNpc { .. })),
        "her line comes from her object's dialogue_id"
    );
}

#[test]
fn the_alignment_branch_skips_the_step_when_already_level() {
    let map = map_with(&["........", "........", "........"], vec![], vec![]);
    let scene = scene_for(EventIndex(3)).unwrap();

    // Same row: the branch takes the skip arm, so no movement command runs.
    let mut state = GameState::new();
    let mut runner = SceneRunner::new(
        scene.ops,
        vec![
            ScriptedActor::new(
                ActorRef::Character(CharId(0)),
                Cell::new(2, 1),
                Direction::Up,
            ),
            ScriptedActor::new(ActorRef::Npc(7), Cell::new(3, 1), Direction::Down),
        ],
        frames(),
    );
    let first = runner.tick(&map, &mut state, SceneInput::None);
    assert!(
        !first.iter().any(|e| matches!(
            e,
            SceneEffect::Presentation {
                op: SceneOp::MoveActorCommand { .. }
            }
        )),
        "aligned actors skip the alignment step"
    );

    // Different rows: the command runs.
    let mut state = GameState::new();
    let mut runner = SceneRunner::new(
        scene.ops,
        vec![
            ScriptedActor::new(
                ActorRef::Character(CharId(0)),
                Cell::new(2, 1),
                Direction::Up,
            ),
            ScriptedActor::new(ActorRef::Npc(7), Cell::new(2, 2), Direction::Down),
        ],
        frames(),
    );
    let first = runner.tick(&map, &mut state, SceneInput::None);
    assert!(
        first.iter().any(|e| matches!(
            e,
            SceneEffect::Presentation {
                op: SceneOp::MoveActorCommand { .. }
            }
        )),
        "misaligned actors take the movement step"
    );
}

#[test]
fn money_ops_accumulate_on_the_state() {
    let map = map_with(&["..", ".."], vec![], vec![]);
    let mut state = GameState::new();
    static PAID: &[SceneOp] = &[
        SceneOp::AddMoney { amount: 100 },
        SceneOp::AddMoney { amount: 300 },
        SceneOp::End,
    ];
    let mut runner = SceneRunner::new(PAID, vec![], frames());
    let effects = runner.tick(&map, &mut state, SceneInput::None);
    assert_eq!(state.money(), 400);
    assert!(effects.contains(&SceneEffect::MoneyChanged { total: 400 }));
}

#[test]
fn a_scene_replays_identically() {
    let map = map_with(&["......", "......", "......"], vec![], vec![]);

    let replay = || {
        let mut state = GameState::new();
        let mut runner = join_runner();
        let mut log = Vec::new();
        for tick in 0..80 {
            // Close the dialogue on a fixed tick so the run is scripted.
            let input = if tick == 20 {
                SceneInput::DialogueClosed
            } else {
                SceneInput::None
            };
            for effect in runner.tick(&map, &mut state, input) {
                log.push((tick, effect));
            }
        }
        (state, log, runner.actors().to_vec())
    };

    let (state_a, log_a, actors_a) = replay();
    let (state_b, log_b, actors_b) = replay();
    assert_eq!(state_a, state_b);
    assert_eq!(log_a, log_b);
    assert_eq!(actors_a, actors_b);
    assert!(!log_a.is_empty());
}

#[test]
fn trigger_evaluation_is_a_pure_function_of_state_and_position() {
    // Same inputs, same answer, no matter how many times or in what order.
    let mut state = GameState::new();
    state.set(Flag::event(0x42)).unwrap();
    let positions = [
        PixelPos { x: 0x260, y: 0xF0 },
        PixelPos { x: 0x730, y: 0xAD0 },
        PixelPos { x: 0, y: 0 },
    ];

    let sweep = || {
        let mut out = Vec::new();
        for at in positions {
            for index in 0..128u8 {
                out.push(TRIGGERS[usize::from(index)].evaluate(&ctx(&state, at)));
            }
        }
        out
    };
    assert_eq!(sweep(), sweep());
}

#[test]
fn the_machine_center_trigger_needs_its_flag_and_its_rect() {
    // $06: Zio set, MachineCenter clear, x in [$710,$750], y == $AD0.
    let mut state = GameState::new();
    let at = PixelPos { x: 0x730, y: 0xAD0 };
    assert_eq!(
        TRIGGERS[0x06].evaluate(&ctx(&state, at)),
        TriggerResult::NoEvent
    );

    state.set(Flag::event(0x42)).unwrap();
    assert_eq!(
        TRIGGERS[0x06].evaluate(&ctx(&state, at)),
        TriggerResult::Fire(EventIndex(6))
    );

    state.set(Flag::event(0x43)).unwrap();
    assert_eq!(
        TRIGGERS[0x06].evaluate(&ctx(&state, at)),
        TriggerResult::NoEvent
    );
}

#[test]
fn a_scene_struct_carries_its_provenance() {
    static OPS: &[SceneOp] = &[SceneOp::End];
    let scene = Scene {
        name: "Event_Synthetic",
        event: EventIndex(0x1234),
        ops: OPS,
    };
    assert_eq!(scene.name, "Event_Synthetic");
    assert_eq!(scene.ops.len(), 1);
}
