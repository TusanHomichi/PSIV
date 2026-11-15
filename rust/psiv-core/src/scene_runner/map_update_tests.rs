use super::*;

#[test]
fn retail_waits_dispatch_updates_but_vint_and_ending_start_waits_do_not() {
    let map = FieldMap::new(
        crate::MapId(3),
        crate::CollisionGrid::filled(2, 2, 0).unwrap(),
        vec![],
        vec![],
    )
    .unwrap();
    let mut state = GameState::new();
    let mut runner = SceneRunner::new(
        &[
            SceneOp::Wait { ticks: 3 },
            SceneOp::WaitFrames { frames: 3 },
            SceneOp::WaitForStart,
            SceneOp::End,
        ],
        vec![],
        StepFrames::default(),
    );
    for expected in [true, true, true, false, false, false, false, false] {
        runner.tick(&map, &mut state, SceneInput::None);
        assert_eq!(runner.runs_map_updates(), expected);
    }
    runner.tick(&map, &mut state, SceneInput::EndingContinue);
    assert!(!runner.runs_map_updates());
    assert!(runner.is_finished());
}

#[test]
fn the_ending_start_gate_is_released_only_by_the_ending_continue_input() {
    let map = FieldMap::new(
        crate::MapId(3),
        crate::CollisionGrid::filled(2, 2, 0).unwrap(),
        vec![],
        vec![],
    )
    .unwrap();
    let mut state = GameState::new();
    let mut runner = SceneRunner::new(
        &[SceneOp::WaitForStart, SceneOp::End],
        vec![],
        StepFrames::default(),
    );
    runner.tick(&map, &mut state, SceneInput::None);
    assert!(runner.awaiting_ending_start());
    for input in [
        SceneInput::None,
        SceneInput::DialogueClosed,
        SceneInput::Choice(true),
        SceneInput::CameraArrived,
    ] {
        runner.tick(&map, &mut state, input);
        assert!(
            runner.awaiting_ending_start(),
            "{input:?} must not release the ending's Start gate"
        );
    }
    runner.tick(&map, &mut state, SceneInput::EndingContinue);
    assert!(!runner.awaiting_ending_start());
    assert!(runner.is_finished());
}

#[test]
fn learn_technique_faults_on_a_character_seat_that_does_not_exist() {
    let map = FieldMap::new(
        crate::MapId(3),
        crate::CollisionGrid::filled(2, 2, 0).unwrap(),
        vec![],
        vec![],
    )
    .unwrap();
    let mut state = GameState::new();
    let mut runner = SceneRunner::new(
        &[SceneOp::LearnTechnique {
            character: 5,
            technique: 0x10,
        }],
        vec![],
        StepFrames::default(),
    );
    let effects = runner.tick(&map, &mut state, SceneInput::None);
    assert!(
        effects
            .iter()
            .any(|effect| matches!(effect, SceneEffect::Faulted(SceneFault::BadWrite))),
        "{effects:?}"
    );
}

#[test]
fn actor_arrival_and_camera_completion_still_own_their_last_update_frame() {
    use crate::{DialogueId, DialogueSource, DialogueWindow, Direction};
    let map = FieldMap::new(
        crate::MapId(3),
        crate::CollisionGrid::filled(3, 3, 0).unwrap(),
        vec![],
        vec![],
    )
    .unwrap();
    let actor = ActorRef::PartyMember(0);
    let mut state = GameState::new();
    state.set_party([Some(CharId(0)), None, None, None, None]);
    const WALK_INTO_DIALOGUE: &[SceneOp] = &[
        SceneOp::MoveActor {
            actor: ActorRef::PartyMember(0),
            to: Cell::new(2, 1),
        },
        SceneOp::WaitForActor {
            actor: ActorRef::PartyMember(0),
        },
        SceneOp::RunDialogue {
            source: DialogueSource::Entry(DialogueId(0)),
            window: DialogueWindow::Standard,
        },
    ];
    let mut runner = SceneRunner::new(
        WALK_INTO_DIALOGUE,
        vec![ScriptedActor::new(actor, Cell::new(1, 1), Direction::Right)],
        StepFrames::default(),
    );
    runner.tick(&map, &mut state, SceneInput::None);
    let mut arrival_frame = false;
    for _ in 0..16 {
        let motion = runner.completes_map_update_loop();
        let effects = runner.tick(&map, &mut state, SceneInput::None);
        if effects
            .iter()
            .any(|e| matches!(e, SceneEffect::DialogueOpen(_)))
        {
            assert!(
                motion,
                "arrival frame had a map-update loop before it unblocked"
            );
            assert!(
                !runner.runs_map_updates(),
                "the following dialogue parks updates"
            );
            arrival_frame = true;
            break;
        }
    }
    assert!(arrival_frame);
    let mut camera = SceneRunner::new(
        &[SceneOp::AlignVehicleBoarding { index: 1 }, SceneOp::End],
        vec![ScriptedActor::new(actor, Cell::new(1, 1), Direction::Right)],
        StepFrames::default(),
    );
    camera.tick(&map, &mut state, SceneInput::None);
    assert!(camera.completes_map_update_loop());
    camera.tick(&map, &mut state, SceneInput::CameraArrived);
    assert!(camera.is_finished());
    assert!(!camera.runs_map_updates());
}

/// Each tick's effects and whether it updated the map, for `ops` run with
/// every palette wait answered with `fade` frames.
fn fade_trace(ops: &'static [SceneOp], fade: u16) -> Vec<(Vec<SceneEffect>, bool)> {
    let map = FieldMap::new(
        crate::MapId(3),
        crate::CollisionGrid::filled(2, 2, 0).unwrap(),
        vec![],
        vec![],
    )
    .unwrap();
    let mut state = GameState::new();
    let mut runner = SceneRunner::new(ops, vec![], StepFrames::default());
    let mut ticks = Vec::new();
    while !runner.is_finished() && ticks.len() < 40 {
        let effects = runner.tick(&map, &mut state, SceneInput::None);
        if runner.awaits_palette() {
            runner.release_palette(fade);
        }
        ticks.push((effects, runner.runs_map_updates()));
    }
    ticks
}

fn tick_of(ticks: &[(Vec<SceneEffect>, bool)], op: SceneOp) -> usize {
    ticks
        .iter()
        .position(|(effects, _)| effects.contains(&SceneEffect::Presentation { op }))
        .unwrap_or_else(|| panic!("{op:?} never emitted"))
}

/// `InitVRAMAndCRAM` (#637): its `PalFadeOut_ClrSpriteTbl` first, for the
/// frames the runtime answers, then the op and `DMAPlanes_VInt`'s frame,
/// then the next op; none of those frames updates the map, though the op
/// before it was a map-update `Wait`.
#[test]
fn init_vram_and_cram_fades_out_then_spends_its_frame_without_map_updates() {
    static OPS: &[SceneOp] = &[
        SceneOp::Wait { ticks: 2 },
        SceneOp::InitVramAndCram,
        SceneOp::PlaySound { id: 1 },
        SceneOp::End,
    ];
    let ticks = fade_trace(OPS, 5);
    let fade = tick_of(&ticks, SceneOp::FadeOut);
    let init = tick_of(&ticks, SceneOp::InitVramAndCram);
    let next = tick_of(&ticks, SceneOp::PlaySound { id: 1 });
    assert_eq!((init - fade, next - init), (5, 1));
    assert!(ticks[..fade].iter().any(|(_, updates)| *updates));
    assert!(ticks[fade..next].iter().all(|(_, updates)| !updates));
    let fade_outs = ticks
        .iter()
        .flat_map(|(effects, _)| effects)
        .filter(|effect| {
            **effect
                == SceneEffect::Presentation {
                    op: SceneOp::FadeOut,
                }
        })
        .count();
    assert_eq!(fade_outs, 1, "one fade per InitVRAMAndCRAM");
}

/// `Pal_FadeIn` and `PalFadeOut_ClrSpriteTbl` wait for the frames the
/// runtime answers, with no map update.
#[test]
fn the_fixed_fades_wait_for_their_answer_without_map_updates() {
    static OPS: &[SceneOp] = &[
        SceneOp::FadeOut,
        SceneOp::FadeIn,
        SceneOp::PlaySound { id: 1 },
        SceneOp::End,
    ];
    let ticks = fade_trace(OPS, 4);
    let out = tick_of(&ticks, SceneOp::FadeOut);
    let fade_in = tick_of(&ticks, SceneOp::FadeIn);
    let next = tick_of(&ticks, SceneOp::PlaySound { id: 1 });
    assert_eq!((fade_in - out, next - fade_in), (4, 4));
    assert!(ticks[out..next].iter().all(|(_, updates)| !updates));
}

/// Negative control for the two above: a map-update `Wait` of the same
/// length in the fade's place is seen updating the map, so the check can
/// fail.
#[test]
fn a_wait_in_the_fades_place_is_seen_updating_the_map() {
    static OPS: &[SceneOp] = &[
        SceneOp::Wait { ticks: 2 },
        SceneOp::Wait { ticks: 5 },
        SceneOp::PlaySound { id: 1 },
        SceneOp::End,
    ];
    let ticks = fade_trace(OPS, 5);
    let next = tick_of(&ticks, SceneOp::PlaySound { id: 1 });
    assert!(ticks[2..next].iter().any(|(_, updates)| *updates));
}
