//! Specification of the event engine: state, triggers, and scenes.

mod common;

use common::{map_with, torus};
use psiv_core::{
    ActorRef, AxisPredicate, Cell, CharId, Condition, CustomTrigger, DialogueId, DialogueSource,
    Direction, EventIndex, Flag, GameState, PixelPos, PositionPredicate, Scene, SceneEffect,
    SceneFault, SceneInput, SceneOp, SceneRunner, ScriptedActor, StepFrames, TRIGGERS, Trigger,
    TriggerContext, TriggerResult, Unsupported, evaluate_list, scene_for,
};

const FRAMES: u8 = 8;

fn frames() -> StepFrames {
    StepFrames::new(FRAMES).unwrap()
}

fn ctx<'a>(state: &'a GameState, at: PixelPos) -> TriggerContext<'a> {
    TriggerContext {
        state,
        at,
        standing: None,
        previously_standing: None,
    }
}

// ---------------------------------------------------------------------------
// Trigger evaluation against the real table
// ---------------------------------------------------------------------------

#[test]
fn the_alys_trigger_fires_from_the_cell_the_pixel_literals_name() {
    // $260 / $F0 is column 38, row 16 once the standing-cell shift is undone.
    let state = GameState::new();
    let at = PixelPos::from_cell(Cell::new(38, 16));
    assert_eq!(at, PixelPos { x: 0x260, y: 0xF0 });

    assert_eq!(
        TRIGGERS[0x03].evaluate(&ctx(&state, at)),
        TriggerResult::Fire(EventIndex(3))
    );

    // The row above is below the threshold; the column beside it is wrong.
    assert_eq!(
        TRIGGERS[0x03].evaluate(&ctx(&state, PixelPos::from_cell(Cell::new(38, 15)))),
        TriggerResult::NoEvent
    );
    assert_eq!(
        TRIGGERS[0x03].evaluate(&ctx(&state, PixelPos::from_cell(Cell::new(39, 20)))),
        TriggerResult::NoEvent
    );
}

#[test]
fn every_axis_predicate_variant_behaves() {
    let state = GameState::new();
    let cases: [(AxisPredicate, i32, bool); 12] = [
        (AxisPredicate::Any, -9999, true),
        (AxisPredicate::Exact(0x100), 0x100, true),
        (AxisPredicate::Exact(0x100), 0x101, false),
        (AxisPredicate::AtLeast(0x100), 0x100, true),
        (AxisPredicate::AtLeast(0x100), 0x0FF, false),
        (AxisPredicate::AtLeast(0x100), 0x200, true),
        (AxisPredicate::AtMost(0x100), 0x100, true),
        (AxisPredicate::AtMost(0x100), 0x101, false),
        (AxisPredicate::AtMost(0x100), 0x000, true),
        (AxisPredicate::Between(0x100, 0x120), 0x100, true),
        (AxisPredicate::Between(0x100, 0x120), 0x120, true),
        (AxisPredicate::Between(0x100, 0x120), 0x121, false),
    ];

    for (predicate, value, expected) in cases {
        let trigger = Trigger::Condition(Condition {
            require_set: &[],
            require_clear: &[],
            position: PositionPredicate::new(predicate, AxisPredicate::Any),
            event: EventIndex(1),
        });
        let hit = trigger.evaluate(&ctx(&state, PixelPos { x: value, y: 0 }))
            == TriggerResult::Fire(EventIndex(1));
        assert_eq!(hit, expected, "{predicate:?} against {value:#X}");
    }
}

#[test]
fn a_rectangle_needs_both_axes() {
    // $39 LutzRevelation: x in [$1E0,$210] and y in [$1D0,$1F0].
    let state = GameState::new();
    let inside = PixelPos { x: 0x1F0, y: 0x1E0 };
    assert_eq!(
        TRIGGERS[0x39].evaluate(&ctx(&state, inside)),
        TriggerResult::Fire(EventIndex(0x8014))
    );
    for outside in [
        PixelPos { x: 0x1F0, y: 0x1C0 },
        PixelPos { x: 0x1F0, y: 0x200 },
        PixelPos { x: 0x1D0, y: 0x1E0 },
        PixelPos { x: 0x220, y: 0x1E0 },
    ] {
        assert_eq!(
            TRIGGERS[0x39].evaluate(&ctx(&state, outside)),
            TriggerResult::NoEvent,
            "{outside:?} is outside the rect"
        );
    }
}

#[test]
fn an_event_list_stops_at_its_first_hit() {
    // `RunEvents` scans the map's list in order and returns on the first pass,
    // so list order is behaviour.
    let state = GameState::new();
    let at = PixelPos::from_cell(Cell::new(38, 16));

    // $08 (flags only, fires on a fresh state) listed before $03.
    let hit = evaluate_list(&TRIGGERS, &[0x08, 0x03], &ctx(&state, at));
    assert_eq!(hit, Some((0x08, TriggerResult::Fire(EventIndex(0x0C)))));

    // Reversed, the Alys check wins.
    let hit = evaluate_list(&TRIGGERS, &[0x03, 0x08], &ctx(&state, at));
    assert_eq!(hit, Some((0x03, TriggerResult::Fire(EventIndex(3)))));
}

#[test]
fn a_list_of_misses_returns_nothing() {
    let mut state = GameState::new();
    state.set(Flag::event(0x08)).unwrap();
    state.set(Flag::event(0x0D)).unwrap();
    let at = PixelPos::from_cell(Cell::new(0, 1));
    assert_eq!(
        evaluate_list(&TRIGGERS, &[0x03, 0x08], &ctx(&state, at)),
        None
    );
}

#[test]
fn undecidable_entries_are_reported_never_silently_skipped() {
    let state = GameState::new();
    let at = PixelPos { x: 0, y: 0 };

    // $7B needs an inventory. It must surface rather than read as "no event".
    let hit = evaluate_list(&TRIGGERS, &[0x7B], &ctx(&state, at));
    assert_eq!(
        hit,
        Some((
            0x7B,
            TriggerResult::Unsupported(CustomTrigger::PenguFeedStolen, Unsupported::Inventory)
        ))
    );

    // A real hit later in the list still wins over an earlier undecidable one.
    let hit = evaluate_list(&TRIGGERS, &[0x7B, 0x08], &ctx(&state, at));
    assert_eq!(hit, Some((0x08, TriggerResult::Fire(EventIndex(0x0C)))));
}

#[test]
fn the_recovery_trigger_reads_the_collision_context() {
    let state = GameState::new();
    let mut c = ctx(&state, PixelPos { x: 0, y: 0 });
    c.standing = Some(2);
    c.previously_standing = Some(0);
    assert_eq!(
        TRIGGERS[0x12].evaluate(&c),
        TriggerResult::Fire(EventIndex(0x21))
    );
    c.previously_standing = Some(2);
    assert_eq!(TRIGGERS[0x12].evaluate(&c), TriggerResult::NoEvent);
}

// ---------------------------------------------------------------------------
// Scene running
// ---------------------------------------------------------------------------

/// The opening-act shape: an NPC walks over, turns, talks, joins the party,
/// despawns, and the camera moves. Synthetic, but the same op sequence
/// `Event_AlysFound` is described as using.
static JOIN_SCENE: &[SceneOp] = &[
    SceneOp::MoveActor {
        actor: ActorRef::Npc(0),
        to: Cell::new(3, 1),
    },
    SceneOp::WaitForActor {
        actor: ActorRef::Npc(0),
    },
    SceneOp::Face {
        actor: ActorRef::Npc(0),
        facing: Direction::Right,
    },
    SceneOp::RunDialogue {
        source: DialogueSource::Entry(DialogueId(7)),
    },
    SceneOp::SetParty {
        slots: [Some(CharId(1)), Some(CharId(0)), None, None, None],
    },
    SceneOp::SetFlag {
        flag: Flag::event(0x08),
        value: true,
    },
    SceneOp::DespawnNpc { npc_index: 0 },
    SceneOp::MoveCamera {
        x: 0x30,
        y: 0x10,
        speed: 2,
    },
    SceneOp::End,
];

fn join_runner() -> SceneRunner {
    SceneRunner::new(
        JOIN_SCENE,
        vec![ScriptedActor::new(
            ActorRef::Npc(0),
            Cell::new(1, 1),
            Direction::Down,
        )],
        frames(),
    )
}

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
            SceneEffect::NpcDespawned { npc_index: 0 },
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
    // `FieldObj_GetAutoInput` closes the X gap before the Y gap by default.
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

// ---------------------------------------------------------------------------
// Registry and determinism
// ---------------------------------------------------------------------------

#[test]
fn the_registry_holds_the_transcribed_scenes() {
    let alys = scene_for(EventIndex(3)).expect("Event_AlysFound is transcribed");
    assert_eq!(alys.name, "Event_AlysFound");
    assert_eq!(alys.ops.len(), 12, "the transcription doc counts 12 ops");

    let alone = scene_for(EventIndex(0xA0)).expect("Event_PiataChazAlone is transcribed");
    assert_eq!(alone.name, "Event_PiataChazAlone");

    // Untranscribed scenes report absent rather than returning something wrong.
    assert!(
        scene_for(EventIndex(0x9F)).is_none(),
        "GameStart is not in yet"
    );
}

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
        log.contains(&SceneEffect::NpcDespawned { npc_index: 7 }),
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

// ---------------------------------------------------------------------------
// Oracle-facing sampling and multi-map scenes
// ---------------------------------------------------------------------------

#[test]
fn step_durations_match_the_cartridges_countdown_ladder() {
    // Hardware: x_step_duration counts $1000 -> 0 in $200 steps, eight states,
    // and only the walked axis is non-zero.
    let map = map_with(&["....", "....", "...."], vec![], vec![]);
    let mut state = psiv_core::FieldState::new(
        &map,
        Cell::new(0, 1),
        Direction::Down,
        StepFrames::default(),
    )
    .unwrap();

    assert_eq!(state.step_durations_8_8(), (0, 0), "at rest");
    assert_eq!(state.step_remaining_frames(), 0);

    let mut ladder = Vec::new();
    for _ in 0..8 {
        state.tick(&map, psiv_core::Input::Direction(Direction::Right));
        ladder.push(state.step_durations_8_8());
    }
    assert_eq!(
        ladder,
        vec![
            (0x0E00, 0),
            (0x0C00, 0),
            (0x0A00, 0),
            (0x0800, 0),
            (0x0600, 0),
            (0x0400, 0),
            (0x0200, 0),
            (0, 0),
        ],
        "the $200-per-frame ladder, landing at zero"
    );

    // Walking vertically moves the other column.
    state.tick(&map, psiv_core::Input::Direction(Direction::Down));
    assert_eq!(state.step_durations_8_8(), (0, 0x0E00));
}

#[test]
fn a_scene_can_be_recast_across_a_map_change() {
    // The opening event tours five maps; NPC indices only mean anything
    // relative to one map's object list, so the cast is re-seated on the way.
    let first = map_with(&["....", "....", "...."], vec![], vec![]);
    let second = map_with(&["......", "......", "......"], vec![], vec![]);
    let mut state = GameState::new();

    static TOUR: &[SceneOp] = &[
        SceneOp::LoadMap {
            map: 0x13,
            prev_map: 0x11,
            start_x: 4,
            start_y: 4,
            facing: Direction::Down,
            align: 0,
        },
        SceneOp::Face {
            actor: ActorRef::Npc(0),
            facing: Direction::Left,
        },
        SceneOp::End,
    ];

    let mut runner = SceneRunner::new(
        TOUR,
        vec![ScriptedActor::new(
            ActorRef::Npc(3),
            Cell::new(1, 1),
            Direction::Down,
        )],
        frames(),
    );

    // Without a recast the scene faults on the op naming an actor the new map
    // does not have — loudly, which is the point.
    let mut doomed = runner.clone();
    let effects = doomed.tick(&first, &mut state, SceneInput::None);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, SceneEffect::Faulted(SceneFault::UnknownActor { .. }))),
        "a stale cast must fault rather than drive ghosts"
    );

    // The runtime's actual flow: load the map on MapRequested, recast, carry on.
    let mut runner2 = SceneRunner::new(
        &TOUR[..1],
        vec![ScriptedActor::new(
            ActorRef::Npc(3),
            Cell::new(1, 1),
            Direction::Down,
        )],
        frames(),
    );
    let effects = runner2.tick(&first, &mut state, SceneInput::None);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, SceneEffect::MapRequested { .. })),
        "the map change is reported so the runtime can act"
    );

    runner.recast(vec![ScriptedActor::new(
        ActorRef::Npc(0),
        Cell::new(2, 2),
        Direction::Up,
    )]);
    let effects = runner.tick(&second, &mut state, SceneInput::None);
    assert!(effects.iter().any(|e| matches!(
        e,
        SceneEffect::ActorFaced {
            actor: ActorRef::Npc(0),
            ..
        }
    )));
    assert_eq!(
        runner.actor(ActorRef::Npc(0)).unwrap().facing,
        Direction::Left
    );
}

#[test]
fn face_is_a_direct_write_not_a_step() {
    // Scenes are not bound by the walker's commit-a-whole-cell rule.
    let map = map_with(&["....", "....", "...."], vec![], vec![]);
    let mut state = GameState::new();
    static TURN: &[SceneOp] = &[
        SceneOp::Face {
            actor: ActorRef::Npc(0),
            facing: Direction::Right,
        },
        SceneOp::End,
    ];
    let mut runner = SceneRunner::new(
        TURN,
        vec![ScriptedActor::new(
            ActorRef::Npc(0),
            Cell::new(1, 1),
            Direction::Down,
        )],
        frames(),
    );

    runner.tick(&map, &mut state, SceneInput::None);
    let actor = runner.actor(ActorRef::Npc(0)).unwrap();
    assert_eq!(actor.facing, Direction::Right);
    assert_eq!(actor.cell, Cell::new(1, 1), "facing must not move anyone");
    assert!(!actor.is_walking());
}

#[test]
fn the_transcribed_party_words_match_the_oracles_observations() {
    // Observed on tape: 00FFFFFF (Chaz alone), 0001FFFF (Chaz then Alys),
    // 0100FFFF (Alys leading, post-AlysFound).
    let mut state = GameState::new();

    state.set_party([Some(CharId(0)), None, None, None, None]);
    assert_eq!(state.party_slot(0), Some(CharId(0)));
    assert_eq!(state.party_len(), 1);

    state.set_party([Some(CharId(0)), Some(CharId(1)), None, None, None]);
    assert_eq!(state.party_len(), 2);

    state.set_party([Some(CharId(1)), Some(CharId(0)), None, None, None]);
    assert_eq!(state.party_slot(0), Some(CharId(1)), "Alys leads");
    assert_eq!(state.party_slot(1), Some(CharId(0)));
}
