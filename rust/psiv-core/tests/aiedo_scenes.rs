//! Scene outcomes for Aiedo's side events (#81): Rocky's owner and the bail
//! quest. Every case cites the retail bytes (`docs/scenes/120_*` to `124_*`).

mod common;

use common::map_with;
use psiv_core::{
    ActorRef, Cell, DialogueId, Direction, EventIndex, Flag, GameState, SceneEffect, SceneInput,
    SceneOp, SceneRunner, ScriptedActor, StepFrames, scene_for,
};

/// What the runtime answers: the yes/no, and the byte at `-$2(a0)` the last
/// dialogue stopped after.
#[derive(Clone, Copy)]
struct Answers {
    yes: bool,
    last_byte: Option<u8>,
}

const PLAIN: Answers = Answers {
    yes: true,
    last_byte: None,
};

/// The leader facing up from below three map objects, so a facing flip is
/// visible.
fn cast() -> Vec<ScriptedActor> {
    vec![
        ScriptedActor::new(ActorRef::PartyMember(0), Cell::new(2, 3), Direction::Up),
        ScriptedActor::new(ActorRef::Npc(0), Cell::new(1, 1), Direction::Left),
        ScriptedActor::new(ActorRef::Npc(1), Cell::new(2, 1), Direction::Left),
        ScriptedActor::new(ActorRef::Npc(5), Cell::new(3, 1), Direction::Left),
    ]
}

/// Runs `event` to its end, answering every block at once.
fn play(event: u16, state: &mut GameState, answers: Answers) -> Vec<SceneEffect> {
    let map = map_with(&["....", "....", "....", "...."], vec![], vec![]);
    let scene = scene_for(EventIndex(event)).unwrap_or_else(|| panic!("${event:04X} unregistered"));
    let mut runner = SceneRunner::new(scene.ops, cast(), StepFrames::new(8).unwrap());
    let mut log: Vec<SceneEffect> = Vec::new();
    for _ in 0..200 {
        let input = match log.last() {
            Some(SceneEffect::DialogueOpen(_) | SceneEffect::DialogueResume) => {
                runner.set_dialogue_last_byte(answers.last_byte);
                SceneInput::DialogueClosed
            }
            Some(SceneEffect::ChoiceRequested) => SceneInput::Choice(answers.yes),
            Some(SceneEffect::MapRequested { .. }) => SceneInput::MapLoaded,
            _ => SceneInput::None,
        };
        log.extend(runner.tick(&map, state, input));
        if runner.is_finished() {
            break;
        }
    }
    assert!(runner.is_finished(), "${event:04X} did not finish");
    assert!(
        !log.iter().any(|e| matches!(e, SceneEffect::Faulted(_))),
        "{log:?}"
    );
    log
}

fn dialogues(log: &[SceneEffect]) -> Vec<u16> {
    log.iter()
        .filter_map(|e| match e {
            SceneEffect::DialogueOpen(DialogueId(id)) => Some(*id),
            _ => None,
        })
        .collect()
}

fn faced(log: &[SceneEffect]) -> Vec<(ActorRef, Direction)> {
    log.iter()
        .filter_map(|e| match e {
            SceneEffect::ActorFaced { actor, facing } => Some((*actor, *facing)),
            _ => None,
        })
        .collect()
}

fn set(id: u16, state: &GameState) -> bool {
    state.is_set(Flag::event(id))
}

/// `Event_RockyOwner`, `Event_GirlPrison` and `Event_GirlsBail` share one
/// shape: the object at `$FFFFC340` or `Field_Obj_Secondary` turns to face
/// the leader (up, so it faces down), speaks one entry and sets one flag.
#[test]
fn the_three_talks_face_the_leader_speak_and_set_their_flag() {
    for (event, npc, entry, flag) in [
        (0x75, 1, 0x5E, 0x1F),
        (0x80, 1, 0x5F, 0x7A),
        (0x81, 0, 0x50, 0x7B),
    ] {
        let mut state = GameState::new();
        let log = play(event, &mut state, PLAIN);
        assert_eq!(
            faced(&log),
            vec![(ActorRef::Npc(npc), Direction::Down)],
            "${event:02X}"
        );
        assert_eq!(dialogues(&log), vec![entry], "${event:02X}");
        assert!(set(flag, &state), "${event:02X} sets ${flag:02X}");
    }
}

fn bail(money: u32, answers: Answers) -> (GameState, Vec<SceneEffect>) {
    let mut state = GameState::new();
    state.set_money(money);
    // A stale `$7F` from an earlier visit: the body clears it first.
    state.set(Flag::event(0x7F)).unwrap();
    let log = play(0x82, &mut state, answers);
    (state, log)
}

const PAID: Answers = Answers {
    yes: true,
    last_byte: Some(0x3A),
};

#[test]
fn a_yes_that_ends_on_the_exclamation_mark_pays_the_bail() {
    let (state, log) = bail(60_000, PAID);
    assert_eq!(dialogues(&log), vec![0x51]);
    assert_eq!(faced(&log), vec![(ActorRef::Npc(0), Direction::Down)]);
    assert!(set(0x7F, &state), "the purse covered it");
    assert!(set(0x7C, &state), "EventFlag_BailPaid");
    assert_eq!(state.money(), 10_000);
    assert!(log.contains(&SceneEffect::MoneyChanged { total: 10_000 }));
}

#[test]
fn exactly_the_bail_is_enough() {
    // `subi.l #50000, d0 / bmi`: a zero difference is not negative.
    let (state, _) = bail(50_000, PAID);
    assert!(set(0x7F, &state));
    assert_eq!(state.money(), 0);
}

#[test]
fn a_short_purse_clears_7f() {
    let (state, _) = bail(
        49_999,
        Answers {
            yes: true,
            last_byte: Some(0x35),
        },
    );
    assert!(
        !set(0x7F, &state),
        "the clear at the top of the body stands"
    );
    assert!(!set(0x7C, &state));
    assert_eq!(state.money(), 49_999);
}

#[test]
fn no_pays_nothing_whatever_the_text_ended_on() {
    // Negative control: the byte alone does not pay; the YES test comes first.
    let (state, _) = bail(
        60_000,
        Answers {
            yes: false,
            last_byte: Some(0x3A),
        },
    );
    assert!(!set(0x7C, &state));
    assert_eq!(state.money(), 60_000);
}

#[test]
fn yes_pays_only_when_the_text_ended_on_3a() {
    // Negative controls: the period, and no stop at all.
    for last_byte in [Some(0x35), None] {
        let (state, _) = bail(
            60_000,
            Answers {
                yes: true,
                last_byte,
            },
        );
        assert!(!set(0x7C, &state), "{last_byte:?}");
        assert_eq!(state.money(), 60_000);
    }
}

#[test]
fn the_bailed_out_sisters_are_taken_home_to_uzo() {
    let mut state = GameState::new();
    let log = play(0x83, &mut state, PLAIN);
    assert!(set(0x7D, &state), "EventFlag_GirlsBailedOut");
    assert_eq!(dialogues(&log), vec![0x61, 0x0A]);
    let load = log
        .iter()
        .find_map(|e| match e {
            SceneEffect::MapRequested { op } => Some(*op),
            _ => None,
        })
        .expect("a map load");
    assert_eq!(
        load,
        SceneOp::LoadMap {
            map: 0x0E4,
            prev_map: 0x0E0,
            start_x: 0x38,
            start_y: 0x46,
            facing: Direction::Up,
            align: 4,
            clear_load_flags: 0x08,
        }
    );
    // The US turn comes after the load, before the fade in.
    let load_at = log
        .iter()
        .position(|e| matches!(e, SceneEffect::MapRequested { .. }))
        .unwrap();
    let face_at = log
        .iter()
        .position(|e| matches!(e, SceneEffect::ActorFaced { .. }))
        .expect("object 0 turns");
    assert!(face_at > load_at);
    assert_eq!(faced(&log), vec![(ActorRef::Npc(0), Direction::Down)]);
}

/// `Event_AiedoManSavings`: entry `$3D` to its `$F7`, object 0 turns to the
/// leader, the same text resumes, and the body reloads the map's chunks.
#[test]
fn the_man_turns_between_the_two_halves_of_his_line() {
    let mut state = GameState::new();
    let before = state.snapshot();
    let log = play(0x22, &mut state, PLAIN);
    let shape: Vec<&str> = log
        .iter()
        .filter_map(|e| match e {
            SceneEffect::DialogueOpen(DialogueId(0x3D)) => Some("dialogue $3D"),
            SceneEffect::ActorFaced {
                actor: ActorRef::Npc(0),
                facing: Direction::Down,
            } => Some("object 0 faces down"),
            SceneEffect::DialogueResume => Some("resume"),
            SceneEffect::Presentation {
                op:
                    SceneOp::Presentation {
                        op: psiv_core::PresentationOp::ReloadMapChunks,
                    },
            } => Some("chunks"),
            _ => None,
        })
        .collect();
    assert_eq!(
        shape,
        ["dialogue $3D", "object 0 faces down", "resume", "chunks"]
    );
    assert_eq!(state.snapshot(), before, "no flag, no money");
}

/// `Event_AiedoManCrushOnAlys`: object 5 turns, entry `$3C` runs to its
/// `$F7`, his routine becomes `$BC` and the scene waits for him to be gone,
/// then clears his slot and resumes the text.
#[test]
fn the_admirer_speaks_runs_off_and_the_party_answers() {
    let mut state = GameState::new();
    let log = play(0x0E, &mut state, PLAIN);
    let shape: Vec<String> = log
        .iter()
        .filter_map(|e| match e {
            SceneEffect::DialogueOpen(DialogueId(id)) => Some(format!("dialogue {id:#X}")),
            SceneEffect::ActorFaced { actor, .. } => Some(format!("{actor:?} faced")),
            SceneEffect::ObjectRoutineStarted { npc_index, routine } => {
                Some(format!("object {npc_index} routine {routine:#X}"))
            }
            SceneEffect::NpcDespawned { npc_index, count } => {
                Some(format!("despawn {npc_index}+{count}"))
            }
            SceneEffect::DialogueResume => Some("resume".to_owned()),
            _ => None,
        })
        .collect();
    assert_eq!(
        shape,
        [
            "Npc(5) faced",
            "dialogue 0x3C",
            "object 5 routine 0xBC",
            "despawn 5+1",
            "resume",
        ]
    );
}

/// The wait holds while the object is on the map and lets go the tick after
/// it clears its slot. Negative control: an object that never leaves keeps
/// the scene waiting.
#[test]
fn the_scene_waits_until_the_object_has_cleared_its_slot() {
    let scene = scene_for(EventIndex(0x0E)).unwrap();
    let npcs: Vec<psiv_core::Npc> = (0..6)
        .map(|i| psiv_core::Npc::new(psiv_core::NpcId(i), Cell::new(i, 0), Direction::Down))
        .collect();
    let mut map = map_with(&["......", "......", "......"], vec![], npcs);
    let mut runner = SceneRunner::new(scene.ops, cast(), StepFrames::new(8).unwrap());
    let mut state = GameState::new();
    runner.tick(&map, &mut state, SceneInput::None);
    runner.tick(&map, &mut state, SceneInput::DialogueClosed);
    assert_eq!(runner.running_object_routine(), Some(5));
    for _ in 0..100 {
        let effects = runner.tick(&map, &mut state, SceneInput::None);
        assert!(!effects.contains(&SceneEffect::DialogueResume));
    }
    assert_eq!(runner.running_object_routine(), Some(5), "still on the map");
    map.set_npc_active(5, false).unwrap();
    let effects = runner.tick(&map, &mut state, SceneInput::None);
    assert_eq!(runner.running_object_routine(), None);
    assert!(effects.contains(&SceneEffect::DialogueResume));
}
