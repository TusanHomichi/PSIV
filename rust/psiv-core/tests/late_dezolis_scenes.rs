//! Scene outcomes for the Esper Mansion's guard events and the Gumbious
//! Temple's theft (issue #83): each case runs the registered scene through
//! `SceneRunner`, answering every block at once, and holds it to what the
//! cartridge's routine does, cited per case (`docs/scenes/99_*` to `101_*`).

mod common;

use common::map_with;
use psiv_core::{
    ActorRef, Cell, DialogueId, Direction, EventIndex, Flag, GameState, SceneEffect, SceneInput,
    SceneRunner, ScriptedActor, StepFrames, scene_for,
};

/// Runs `event` from `state` with the party leader facing `leader`, the two
/// door guards below it, and returns every effect.
fn play(event: u16, state: &mut GameState, leader: Direction) -> Vec<SceneEffect> {
    let map = map_with(
        &["........", "........", "........", "........"],
        vec![],
        vec![],
    );
    let scene =
        scene_for(EventIndex(event)).unwrap_or_else(|| panic!("${event:04X} is not registered"));
    let cast = vec![
        ScriptedActor::new(ActorRef::PartyMember(0), Cell::new(2, 2), leader),
        ScriptedActor::new(ActorRef::Npc(0), Cell::new(2, 1), Direction::Down),
        ScriptedActor::new(ActorRef::Npc(1), Cell::new(3, 1), Direction::Down),
        ScriptedActor::new(ActorRef::Npc(6), Cell::new(2, 0), Direction::Down),
        ScriptedActor::new(ActorRef::PartyMember(1), Cell::new(2, 3), leader),
        ScriptedActor::new(ActorRef::PartyMember(2), Cell::new(2, 3), leader),
        ScriptedActor::new(ActorRef::PartyMember(3), Cell::new(2, 3), leader),
        ScriptedActor::new(ActorRef::PartyMember(4), Cell::new(2, 3), leader),
    ];
    let mut runner = SceneRunner::new(scene.ops, cast, StepFrames::new(8).unwrap());
    let mut log: Vec<SceneEffect> = Vec::new();
    for _ in 0..2_000 {
        let input = match log.last() {
            Some(SceneEffect::DialogueOpen(_) | SceneEffect::DialogueResume) => {
                SceneInput::DialogueClosed
            }
            _ => SceneInput::None,
        };
        log.extend(runner.tick(&map, state, input));
        if runner.is_finished() {
            break;
        }
    }
    assert!(runner.is_finished(), "{} did not finish", scene.name);
    assert!(
        !log.iter().any(|e| matches!(e, SceneEffect::Faulted(_))),
        "{} faulted: {log:?}",
        scene.name
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

fn dialogue_writes(log: &[SceneEffect]) -> Vec<(usize, u16)> {
    log.iter()
        .filter_map(|e| match e {
            SceneEffect::NpcDialogueSet {
                npc_index,
                dialogue_id,
            } => Some((*npc_index, *dialogue_id)),
            _ => None,
        })
        .collect()
}

/// `$0709CE..$0709E5`: neither flag set. Entry 0 turns the party away and the
/// event ends at `jmp Event_GetAndRunDialogue` (`$0709E8`), before any
/// dialogue-id write or the temp flag (`$070A20`).
#[test]
fn guards_turn_the_party_away_without_the_trees_or_dark_force_2() {
    let mut state = GameState::new();
    let log = play(0x4F, &mut state, Direction::Up);
    assert_eq!(dialogues(&log), vec![0]);
    assert!(dialogue_writes(&log).is_empty());
    assert!(!state.is_set(Flag::temp(0x1A)), "the door stays shut");
}

/// `$0709F0..$070A29` through the `$95` arm (`bne` at `$0709E4` is not taken
/// to `loc_70A08`): entry 1, both guards' dialogue ids written `1`
/// (`$14(a4)` at `$070A14`, `$54(a4)` at `$070A1A`), temp flag `$1A` set.
#[test]
fn the_fought_trees_open_the_door_with_entry_1() {
    let mut state = GameState::new();
    state.set(Flag::event(0x95)).unwrap();
    let log = play(0x4F, &mut state, Direction::Up);
    assert_eq!(dialogues(&log), vec![1]);
    assert_eq!(dialogue_writes(&log), vec![(0, 1), (1, 1)]);
    assert!(
        state.is_set(Flag::temp(0x1A)),
        "TempEveFlag_EspMansionGuards"
    );
}

/// `loc_709EE` (`$0709EE..$070A07`): Dark Force 2 defeated wins over the
/// trees, entry 2 and dialogue ids `2`.
#[test]
fn dark_force_2_down_opens_the_door_with_entry_2_and_wins_over_the_trees() {
    let mut state = GameState::new();
    state.set(Flag::event(0x95)).unwrap();
    state.set(Flag::event(0x9E)).unwrap();
    let log = play(0x4F, &mut state, Direction::Up);
    assert_eq!(dialogues(&log), vec![2]);
    assert_eq!(dialogue_writes(&log), vec![(0, 2), (1, 2)]);
    assert!(state.is_set(Flag::temp(0x1A)));
}

/// `$0709A2..$0709CD`: each guard turns the opposite way to the leader
/// (`move.w $6(a3), d0 / bchg #2, d0 / jsr Event_UpdateObjFacing`).
#[test]
fn both_guards_turn_to_face_the_leader() {
    for (leader, wanted) in [
        (Direction::Up, Direction::Down),
        (Direction::Down, Direction::Up),
        (Direction::Left, Direction::Right),
    ] {
        let mut state = GameState::new();
        let log = play(0x4F, &mut state, leader);
        for npc in [0, 1] {
            assert!(
                log.contains(&SceneEffect::ActorFaced {
                    actor: ActorRef::Npc(npc),
                    facing: wanted,
                }),
                "guard {npc} faces {wanted:?} when the leader faces {leader:?}: {log:?}"
            );
        }
    }
}

/// `$06FE28..$06FE55`: the persistent guards run tree 20's entry `$2E`,
/// rewrite objects 3 and 4 to dialogue `$23` and set Inner Sanctuary `$96`.
#[test]
fn rune_talks_the_inner_guards_down() {
    let mut state = GameState::new();
    let log = play(0x45, &mut state, Direction::Up);
    assert_eq!(dialogues(&log), vec![0x2E]);
    assert_eq!(dialogue_writes(&log), vec![(3, 0x23), (4, 0x23)]);
    assert!(state.is_set(Flag::event(0x96)), "EventFlag_InnerSanctuary");
    assert!(
        !state.is_set(Flag::temp(0x1A)),
        "the outer door is untouched"
    );
}

/// `$06FE56..$070189`: the monk's entry `$32` and three resumes, the torch
/// (object 0) and its block (object 1) cleared once the figures have spoken,
/// `$98` the last write, tree 37 loaded first and tree 38 last.
#[test]
fn the_torch_is_stolen_in_one_conversation_and_flag_98_is_the_last_write() {
    let mut state = GameState::new();
    let log = play(0x46, &mut state, Direction::Up);
    assert_eq!(dialogues(&log), vec![0x32]);
    let resumes = log
        .iter()
        .filter(|e| matches!(e, SceneEffect::DialogueResume))
        .count();
    assert_eq!(resumes, 3, "the three resumes of the same text");
    let despawn = log
        .iter()
        .position(|e| {
            matches!(
                e,
                SceneEffect::NpcDespawned {
                    npc_index: 0,
                    count: 2
                }
            )
        })
        .expect("the torch and its block leave");
    let second_resume = log
        .iter()
        .enumerate()
        .filter(|(_, e)| matches!(e, SceneEffect::DialogueResume))
        .nth(1)
        .map(|(i, _)| i)
        .unwrap();
    assert!(despawn < second_resume, "gone before the second resume");
    assert!(
        state.is_set(Flag::event(0x98)),
        "EventFlag_EclipseTorchStolen"
    );
    assert!(
        log.iter()
            .rposition(|e| matches!(e, SceneEffect::FlagChanged { .. }))
            .is_some_and(|i| i + 1 >= log.len() - 2),
        "the flag is the scene's last write"
    );
}

/// The monk turns to the leader (`$06FE76`) and the party faces down
/// (`$06FEBA..$06FEF5`).
#[test]
fn the_monk_faces_the_leader_and_the_party_faces_down() {
    let mut state = GameState::new();
    let log = play(0x46, &mut state, Direction::Left);
    assert!(log.contains(&SceneEffect::ActorFaced {
        actor: ActorRef::Npc(6),
        facing: Direction::Right,
    }));
    for member in 0..5 {
        assert!(
            log.contains(&SceneEffect::ActorFaced {
                actor: ActorRef::PartyMember(member),
                facing: Direction::Down,
            }),
            "member {member}"
        );
    }
}
