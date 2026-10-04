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
