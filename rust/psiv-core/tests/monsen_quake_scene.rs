//! Scene outcomes for the earthquake (#81): `$2C`, cited to the retail bytes
//! (`docs/scenes/143_MonsenEarthquake.md`).

mod common;

use common::map_with;
use psiv_core::{
    ActorRef, Cell, CharId, Direction, EventIndex, Flag, GameState, PresentationOp, SceneEffect,
    SceneInput, SceneOp, SceneRunner, ScriptedActor, StepFrames, scene_for,
};

const RUNE: CharId = CharId(3);

fn play(rune: bool) -> Vec<SceneEffect> {
    let row = ".".repeat(64);
    let rows = vec![row.as_str(); 64];
    let map = map_with(&rows, vec![], vec![]);
    let scene = scene_for(EventIndex(0x2C)).expect("registered");
    let mut state = GameState::new();
    let mut party = [
        Some(CharId(0)),
        Some(CharId(4)),
        Some(CharId(5)),
        Some(CharId(6)),
        None,
    ];
    if rune {
        party[4] = Some(RUNE);
    }
    state.set_party(party);
    let mut cast = vec![ScriptedActor::new(
        ActorRef::Npc(6),
        Cell::new(47, 34),
        Direction::Down,
    )];
    for slot in 0..5 {
        cast.push(ScriptedActor::new(
            ActorRef::PartyMember(slot),
            Cell::new(47, 36),
            Direction::Up,
        ));
    }
    let mut runner = SceneRunner::new(scene.ops, cast, StepFrames::new(8).unwrap());
    let mut log = Vec::new();
    let mut input = SceneInput::None;
    for _ in 0..4_000 {
        let effects = runner.tick(&map, &mut state, input);
        input = if runner.is_waiting_for_camera() {
            SceneInput::CameraArrived
        } else if effects.iter().any(|e| {
            matches!(
                e,
                SceneEffect::DialogueOpen(_) | SceneEffect::DialogueResume
            )
        }) {
            SceneInput::DialogueClosed
        } else {
            SceneInput::None
        };
        log.extend(effects);
        if runner.is_finished() {
            break;
        }
    }
    assert!(runner.is_finished(), "did not finish: {log:?}");
    assert!(
        !log.iter().any(|e| matches!(e, SceneEffect::Faulted(_))),
        "{log:?}"
    );
    assert!(state.is_set(Flag::event(0x60)));
    log
}

/// Entry `$40` opens once and has nine `$F7` stops, so nine resumes.
#[test]
fn the_talk_resumes_at_each_of_its_nine_stops() {
    let log = play(false);
    let opens: Vec<_> = log
        .iter()
        .filter_map(|e| match e {
            SceneEffect::DialogueOpen(id) => Some(id.0),
            _ => None,
        })
        .collect();
    assert_eq!(opens, [0x40]);
    let resumes = log
        .iter()
        .filter(|e| matches!(e, SceneEffect::DialogueResume))
        .count();
    assert_eq!(resumes, 9);
}

/// `loc_6E11E` is 120 plain frames; `loc_6E634` is 60 with its opening rows.
#[test]
fn the_shakes_are_the_routines_two() {
    let shakes: Vec<_> = play(false)
        .iter()
        .filter_map(|e| match e {
            SceneEffect::Presentation {
                op:
                    SceneOp::Presentation {
                        op: PresentationOp::Quake { frames, opening },
                    },
            } => Some((*frames, *opening)),
            _ => None,
        })
        .collect();
    assert_eq!(shakes, [(120, false), (60, true)]);
}

/// `bmi.s` after `Event_GetCharacter`: Rune walks and turns only if present.
#[test]
fn rune_walks_only_when_he_is_in_the_party() {
    let rune_moved = |log: &[SceneEffect]| {
        log.iter().any(|e| {
            matches!(
                e,
                SceneEffect::ActorMoveStarted { to, .. } if *to == Cell::new(0x2A, 0x22)
            )
        })
    };
    assert!(rune_moved(&play(true)));
    assert!(!rune_moved(&play(false)));
}

/// The four created objects, and all cleared at once at `$06E5E6`.
#[test]
fn four_objects_come_and_go() {
    let log = play(false);
    let created: Vec<_> = log
        .iter()
        .filter_map(|e| match e {
            SceneEffect::Presentation {
                op: SceneOp::CreateFieldObject { slot, art_tile, .. },
            } => Some((*slot, *art_tile)),
            _ => None,
        })
        .collect();
    assert_eq!(created, [(8, 0x42B), (9, 0x3E3), (10, 0x42B), (11, 0x39B)]);
    assert!(log.contains(&SceneEffect::NpcDespawned {
        npc_index: 8,
        count: 4,
    }));
}
