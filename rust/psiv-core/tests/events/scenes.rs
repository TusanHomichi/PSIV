//! Scene running: one scene's ops driven through `SceneRunner`.
//!
//! Every case builds a small op list, ticks it against a synthetic map and
//! pins what each tick emits: a move's progress and its render offset, the
//! exact tick a wait releases on, the dialogue handoff, the branch and choice
//! arms, and the faults a malformed scene must raise rather than hang on. The
//! end-to-end case pins the whole effect log of the shared `JOIN_SCENE`.

use crate::common::{map_with, torus};
use crate::fixtures::{frames, join_runner};
use psiv_core::{
    ActorRef, Cell, CharId, DialogueId, DialogueSource, Direction, Flag, GameState, SceneEffect,
    SceneFault, SceneInput, SceneOp, SceneRunner, ScriptedActor,
};

#[test]
fn a_scene_runs_end_to_end_with_a_pinned_effect_log() {
    let map = map_with(&["......", "......", "......"], vec![], vec![]);
    let mut state = GameState::new();
    let mut runner = join_runner();
    let mut log = Vec::new();

    // Tick 1 starts the walk and blocks on WaitForActor.
    log.extend(runner.tick(&map, &mut state, SceneInput::None));
    assert_eq!(
        log,
        vec![SceneEffect::ActorMoveStarted {
            actor: ActorRef::Npc(0),
            to: Cell::new(3, 1)
        }]
    );

    // Actors advance before the script each tick, so the walk's first frame is
    // tick 2. Two cells at eight frames each land the actor on tick 17.
    for _ in 0..15 {
        log.extend(runner.tick(&map, &mut state, SceneInput::None));
    }
    assert_eq!(log.len(), 1, "still walking");
    assert_ne!(
        runner.actor(ActorRef::Npc(0)).unwrap().cell,
        Cell::new(3, 1)
    );

    // Arrival releases the wait, and the script runs on to the dialogue, which
    // blocks — all within the same tick.
    log.extend(runner.tick(&map, &mut state, SceneInput::None));
    assert_eq!(
        runner.actor(ActorRef::Npc(0)).unwrap().cell,
        Cell::new(3, 1)
    );
    assert_eq!(
        &log[1..],
        &[
            SceneEffect::ActorArrived {
                actor: ActorRef::Npc(0),
                at: Cell::new(3, 1)
            },
            SceneEffect::ActorFaced {
                actor: ActorRef::Npc(0),
                facing: Direction::Right
            },
            SceneEffect::DialogueOpen(DialogueId(7)),
        ]
    );

    // While the window is open nothing advances, however long it takes.
    for _ in 0..30 {
        assert!(runner.tick(&map, &mut state, SceneInput::None).is_empty());
    }
    assert!(
        state.is_clear(Flag::event(0x08)),
        "the scene is still blocked"
    );

    // Closing it runs the rest in one tick.
    let tail = runner.tick(&map, &mut state, SceneInput::DialogueClosed);
    assert_eq!(
        tail,
        vec![
            SceneEffect::PartyChanged,
            SceneEffect::FlagChanged {
                flag: Flag::event(0x08),
                value: true
            },
            SceneEffect::NpcDespawned {
                npc_index: 0,
                count: 1
            },
            SceneEffect::Presentation {
                op: SceneOp::MoveCamera {
                    x: 0x30,
                    y: 0x10,
                    speed: 2
                }
            },
            SceneEffect::Finished,
        ]
    );

    assert!(runner.is_finished());
    assert!(state.is_set(Flag::event(0x08)));
    assert_eq!(state.party_slot(0), Some(CharId(1)), "Alys leads");
    assert_eq!(state.party_slot(1), Some(CharId(0)));
    assert_eq!(state.party_len(), 2);

    // A finished runner is inert.
    assert!(runner.tick(&map, &mut state, SceneInput::None).is_empty());
}

#[test]
fn an_actor_walks_x_first_then_y() {
    // `FieldObj_GetAutoInput` closes the X gap before the Y gap by default;
    // the oracle's house-exit walk (tape 27, frames 1820..1900) confirms it.
    // `SetFollowMode` bit 1 flips to Y-first.
    let map = map_with(&["......", "......", "......", "......"], vec![], vec![]);
    let mut state = GameState::new();
    static WALK: &[SceneOp] = &[
        SceneOp::MoveActor {
            actor: ActorRef::Npc(0),
            to: Cell::new(2, 3),
        },
        SceneOp::WaitForActor {
            actor: ActorRef::Npc(0),
        },
        SceneOp::End,
    ];
    let mut runner = SceneRunner::new(
        WALK,
        vec![ScriptedActor::new(
            ActorRef::Npc(0),
            Cell::new(0, 1),
            Direction::Down,
        )],
        frames(),
    );

    let mut path = Vec::new();
    for _ in 0..64 {
        runner.tick(&map, &mut state, SceneInput::None);
        let actor = runner.actor(ActorRef::Npc(0)).unwrap();
        if path.last() != Some(&actor.cell) {
            path.push(actor.cell);
        }
        if runner.is_finished() {
            break;
        }
    }

    assert_eq!(
        path,
        vec![
            Cell::new(0, 1),
            Cell::new(1, 1),
            Cell::new(2, 1),
            Cell::new(2, 2),
            Cell::new(2, 3)
        ],
        "X first, then Y"
    );
}

#[test]
fn a_walking_actor_reports_a_render_offset_like_the_party() {
    let map = map_with(&["....", "....", "...."], vec![], vec![]);
    let mut state = GameState::new();
    static WALK: &[SceneOp] = &[
        SceneOp::MoveActor {
            actor: ActorRef::Npc(0),
            to: Cell::new(2, 1),
        },
        SceneOp::WaitForActor {
            actor: ActorRef::Npc(0),
        },
        SceneOp::End,
    ];
    let mut runner = SceneRunner::new(
        WALK,
        vec![ScriptedActor::new(
            ActorRef::Npc(0),
            Cell::new(0, 1),
            Direction::Down,
        )],
        frames(),
    );

    let mut offsets = Vec::new();
    for _ in 0..8 {
        runner.tick(&map, &mut state, SceneInput::None);
        offsets.push(
            runner
                .actor(ActorRef::Npc(0))
                .unwrap()
                .render_offset_16ths(frames()),
        );
    }
    assert_eq!(
        offsets,
        vec![
            (0, 0),
            (2, 0),
            (4, 0),
            (6, 0),
            (8, 0),
            (10, 0),
            (12, 0),
            (14, 0)
        ],
        "the first tick starts the walk; the rest interpolate"
    );
    assert_eq!(
        runner.actor(ActorRef::Npc(0)).unwrap().facing,
        Direction::Right
    );
}

#[test]
fn a_wait_blocks_for_exactly_its_tick_count() {
    let map = map_with(&["..", ".."], vec![], vec![]);
    let mut state = GameState::new();
    static WAITING: &[SceneOp] = &[
        SceneOp::Wait { ticks: 3 },
        SceneOp::SetFlag {
            flag: Flag::event(1),
            value: true,
        },
        SceneOp::End,
    ];
    let mut runner = SceneRunner::new(WAITING, vec![], frames());

    for tick in 0..3 {
        assert!(
            runner.tick(&map, &mut state, SceneInput::None).is_empty(),
            "tick {tick} should still be waiting"
        );
    }
    let done = runner.tick(&map, &mut state, SceneInput::None);
    assert_eq!(
        done,
        vec![
            SceneEffect::FlagChanged {
                flag: Flag::event(1),
                value: true
            },
            SceneEffect::Finished
        ]
    );
}

#[test]
fn a_zero_tick_wait_does_not_block() {
    let map = map_with(&["..", ".."], vec![], vec![]);
    let mut state = GameState::new();
    static NOWAIT: &[SceneOp] = &[SceneOp::Wait { ticks: 0 }, SceneOp::End];
    let mut runner = SceneRunner::new(NOWAIT, vec![], frames());
    assert_eq!(
        runner.tick(&map, &mut state, SceneInput::None),
        vec![SceneEffect::Finished]
    );
}

#[test]
fn stepping_a_field_object_blocks_the_following_scene_for_all_64_frames() {
    let map = map_with(&["..", ".."], vec![], vec![]);
    let mut state = GameState::new();
    static OPS: &[SceneOp] = &[
        SceneOp::CreateFieldObject {
            slot: 7,
            object_id: 0x188,
            art_tile: 0x3A5,
            x: 480,
            y: 160,
        },
        SceneOp::StepFieldObject {
            slot: 7,
            step_x: 0,
            step_y: 0x8000,
            frames: 64,
        },
        SceneOp::SetFlag {
            flag: Flag::event(1),
            value: true,
        },
        SceneOp::End,
    ];
    let mut runner = SceneRunner::new(OPS, vec![], frames());
    assert_eq!(
        runner.tick(&map, &mut state, SceneInput::None),
        vec![
            SceneEffect::Presentation { op: OPS[0] },
            SceneEffect::Presentation { op: OPS[1] },
        ]
    );
    for tick in 1..64 {
        assert!(
            runner.tick(&map, &mut state, SceneInput::None).is_empty(),
            "motion frame {tick}"
        );
        assert!(!state.is_set(Flag::event(1)));
    }
    let done = runner.tick(&map, &mut state, SceneInput::None);
    assert!(state.is_set(Flag::event(1)));
    assert!(done.contains(&SceneEffect::Finished));
}

#[test]
fn dialogue_resume_retains_or_replaces_its_window_routine() {
    use psiv_core::DialogueWindow;
    let map = map_with(&["..", ".."], vec![], vec![]);
    let mut state = GameState::new();
    static OPS: &[SceneOp] = &[
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(0)),
            window: DialogueWindow::Cutscene,
        },
        SceneOp::RunDialogueResume,
        SceneOp::RunDialogueResumeWithWindow {
            window: DialogueWindow::Ending,
        },
        SceneOp::End,
    ];
    let mut runner = SceneRunner::new(OPS, vec![], frames());
    runner.tick(&map, &mut state, SceneInput::None);
    assert_eq!(runner.dialogue_window(), DialogueWindow::Cutscene);
    runner.tick(&map, &mut state, SceneInput::DialogueClosed);
    assert_eq!(runner.dialogue_window(), DialogueWindow::Cutscene);
    runner.tick(&map, &mut state, SceneInput::DialogueClosed);
    assert_eq!(runner.dialogue_window(), DialogueWindow::Ending);
    runner.tick(&map, &mut state, SceneInput::DialogueEnded);
    assert!(runner.is_finished());
}

#[test]
fn a_flag_branch_takes_the_arm_the_state_selects() {
    let map = map_with(&["..", ".."], vec![], vec![]);
    static BRANCHING: &[SceneOp] = &[
        SceneOp::BranchFlag {
            flag: Flag::event(0x20),
            if_set: 3,
            if_clear: 1,
        },
        SceneOp::SetFlag {
            flag: Flag::event(0x21),
            value: true,
        },
        SceneOp::End,
        SceneOp::SetFlag {
            flag: Flag::event(0x22),
            value: true,
        },
        SceneOp::End,
    ];

    let mut clear_state = GameState::new();
    let mut runner = SceneRunner::new(BRANCHING, vec![], frames());
    runner.tick(&map, &mut clear_state, SceneInput::None);
    assert!(clear_state.is_set(Flag::event(0x21)));
    assert!(clear_state.is_clear(Flag::event(0x22)));

    let mut set_state = GameState::new();
    set_state.set(Flag::event(0x20)).unwrap();
    let mut runner = SceneRunner::new(BRANCHING, vec![], frames());
    runner.tick(&map, &mut set_state, SceneInput::None);
    assert!(set_state.is_set(Flag::event(0x22)));
    assert!(set_state.is_clear(Flag::event(0x21)));
}

#[test]
fn a_choice_blocks_then_jumps_on_the_answer() {
    let map = map_with(&["..", ".."], vec![], vec![]);
    static ASKING: &[SceneOp] = &[
        SceneOp::BranchChoice {
            if_yes: 1,
            if_no: 3,
        },
        SceneOp::SetFlag {
            flag: Flag::event(0x30),
            value: true,
        },
        SceneOp::End,
        SceneOp::SetFlag {
            flag: Flag::event(0x31),
            value: true,
        },
        SceneOp::End,
    ];

    for (answer, expected) in [(true, Flag::event(0x30)), (false, Flag::event(0x31))] {
        let mut state = GameState::new();
        let mut runner = SceneRunner::new(ASKING, vec![], frames());

        let asked = runner.tick(&map, &mut state, SceneInput::None);
        assert_eq!(asked, vec![SceneEffect::ChoiceRequested]);
        assert!(runner.tick(&map, &mut state, SceneInput::None).is_empty());

        runner.tick(&map, &mut state, SceneInput::Choice(answer));
        assert!(
            state.is_set(expected),
            "answering {answer} took the wrong arm"
        );
    }
}

#[test]
fn an_unknown_actor_faults_rather_than_being_ignored() {
    let map = map_with(&["..", ".."], vec![], vec![]);
    let mut state = GameState::new();
    static BAD: &[SceneOp] = &[
        SceneOp::Face {
            actor: ActorRef::Npc(9),
            facing: Direction::Up,
        },
        SceneOp::End,
    ];
    let mut runner = SceneRunner::new(BAD, vec![], frames());

    let effects = runner.tick(&map, &mut state, SceneInput::None);
    assert_eq!(
        effects,
        vec![SceneEffect::Faulted(SceneFault::UnknownActor {
            actor: ActorRef::Npc(9)
        })]
    );
    assert!(runner.is_finished());
}

#[test]
fn a_jump_loop_with_no_wait_faults_instead_of_hanging() {
    let map = map_with(&["..", ".."], vec![], vec![]);
    let mut state = GameState::new();
    static LOOP: &[SceneOp] = &[SceneOp::Jump { to: 0 }];
    let mut runner = SceneRunner::new(LOOP, vec![], frames());

    assert_eq!(
        runner.tick(&map, &mut state, SceneInput::None),
        vec![SceneEffect::Faulted(SceneFault::Runaway)]
    );
}

#[test]
fn a_jump_past_the_end_faults() {
    let map = map_with(&["..", ".."], vec![], vec![]);
    let mut state = GameState::new();
    static BAD: &[SceneOp] = &[SceneOp::Jump { to: 99 }, SceneOp::End];
    let mut runner = SceneRunner::new(BAD, vec![], frames());
    assert_eq!(
        runner.tick(&map, &mut state, SceneInput::None),
        vec![SceneEffect::Faulted(SceneFault::BadJump { target: 99 })]
    );
}

#[test]
fn running_off_the_end_finishes_cleanly() {
    let map = map_with(&["..", ".."], vec![], vec![]);
    let mut state = GameState::new();
    static NO_END: &[SceneOp] = &[SceneOp::PlaySound { id: 3 }];
    let mut runner = SceneRunner::new(NO_END, vec![], frames());
    assert_eq!(
        runner.tick(&map, &mut state, SceneInput::None),
        vec![
            SceneEffect::Presentation {
                op: SceneOp::PlaySound { id: 3 }
            },
            SceneEffect::Finished
        ]
    );
}

#[test]
fn scene_actors_walk_a_wrapping_map_like_everyone_else() {
    let map = torus(&["....", "....", "....", "...."]);
    let mut state = GameState::new();
    static WALK: &[SceneOp] = &[
        SceneOp::MoveActor {
            actor: ActorRef::Npc(0),
            to: Cell::new(0, 1),
        },
        SceneOp::WaitForActor {
            actor: ActorRef::Npc(0),
        },
        SceneOp::End,
    ];
    let mut runner = SceneRunner::new(
        WALK,
        vec![ScriptedActor::new(
            ActorRef::Npc(0),
            Cell::new(3, 1),
            Direction::Down,
        )],
        frames(),
    );

    for _ in 0..64 {
        runner.tick(&map, &mut state, SceneInput::None);
        if runner.is_finished() {
            break;
        }
    }
    assert_eq!(
        runner.actor(ActorRef::Npc(0)).unwrap().cell,
        Cell::new(0, 1)
    );
}
