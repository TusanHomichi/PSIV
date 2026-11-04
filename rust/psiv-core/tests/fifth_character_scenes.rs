//! `Event_PickingFifthCharacter` (`$68`, #83): the row from where the leader
//! stands, both ways onto the party, a NO, and what each leg writes. Record:
//! `docs/scenes/181_PickingFifthCharacter.md`.

mod common;

use common::map_with;
use psiv_core::{
    ActorRef, Cell, CharId, DialogueId, Direction, EventIndex, Flag, GameState, MacroCommand,
    SceneEffect, SceneFault, SceneInput, SceneRunner, ScriptedActor, StepFrames, scene_for,
};

const CHAZ: CharId = CharId(0);
const HAHN: CharId = CharId(2);
const RUNE: CharId = CharId(3);
const GRYZ: CharId = CharId(4);
const RIKA: CharId = CharId(5);
const DEMI: CharId = CharId(6);
const WREN: CharId = CharId(7);
const RAJA: CharId = CharId(8);
const KYRA: CharId = CharId(9);

const FIRST_OBJECT: usize = 5;

/// (who, stand cell, first line, picked flag), in table order. `$1B0,$190` is
/// cell (27, 26): the standing shift adds one row.
const FRIENDS: [(CharId, (u16, u16), u16, u16); 5] = [
    (GRYZ, (27, 26), 0x06, 0xDC),
    (HAHN, (26, 29), 0x02, 0xDB),
    (DEMI, (34, 29), 0x0A, 0xDD),
    (RAJA, (34, 25), 0x0E, 0xDE),
    (KYRA, (33, 27), 0x12, 0xDF),
];

fn four() -> [Option<CharId>; 5] {
    [Some(CHAZ), Some(RUNE), Some(RIKA), Some(WREN), None]
}

/// A runner on the spaceport's objects with the leader below `row`'s friend,
/// facing up.
fn runner_at(row: usize, party_size: usize) -> SceneRunner {
    let (_, (x, y), _, _) = FRIENDS[row];
    let scene = scene_for(EventIndex(0x68)).expect("registered");
    let mut cast = vec![ScriptedActor::new(
        ActorRef::PartyMember(0),
        Cell::new(x, y + 1),
        Direction::Up,
    )];
    for slot in 1..party_size {
        cast.push(ScriptedActor::new(
            ActorRef::PartyMember(slot),
            Cell::new(x, y + 1 + slot as u16),
            Direction::Up,
        ));
    }
    for (n, (_, (fx, fy), _, _)) in FRIENDS.iter().enumerate() {
        cast.push(ScriptedActor::new(
            ActorRef::Npc(FIRST_OBJECT + n),
            Cell::new(*fx, *fy),
            Direction::Down,
        ));
    }
    SceneRunner::new(scene.ops, cast, StepFrames::new(8).unwrap())
}

/// Plays with every dialogue closing at once, `answer` to the prompt and the
/// camera arriving, to the end.
fn play(runner: &mut SceneRunner, state: &mut GameState, answer: bool) -> Vec<SceneEffect> {
    let map = map_with(&["...."; 4], vec![], vec![]);
    let mut log = Vec::new();
    let mut input = SceneInput::None;
    for _ in 0..200 {
        if runner.is_finished() {
            break;
        }
        let effects = runner.tick(&map, state, input);
        input = if runner.is_waiting_for_camera() {
            SceneInput::CameraArrived
        } else if effects
            .iter()
            .any(|e| matches!(e, SceneEffect::ChoiceRequested))
        {
            SceneInput::Choice(answer)
        } else if effects
            .iter()
            .any(|e| matches!(e, SceneEffect::DialogueOpen(_)))
        {
            SceneInput::DialogueClosed
        } else {
            SceneInput::None
        };
        log.extend(effects);
    }
    assert!(runner.is_finished(), "{log:?}");
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

fn picked_flags(state: &GameState) -> Vec<u16> {
    (0xDB..=0xDF)
        .filter(|id| state.is_set(Flag::event(*id)))
        .collect()
}

#[test]
fn each_friend_joins_the_free_fifth_slot() {
    for (row, (who, (x, y), first, flag)) in FRIENDS.into_iter().enumerate() {
        let mut state = GameState::new();
        state.set_party(four());
        let mut runner = runner_at(row, 4);
        let log = play(&mut runner, &mut state, true);
        assert_eq!(dialogues(&log), vec![first, first + 2], "{who:?}");
        assert_eq!(state.party_slot(4), Some(who));
        assert_eq!(picked_flags(&state), vec![flag]);
        assert!(log.contains(&SceneEffect::NpcDespawned {
            npc_index: FIRST_OBJECT + row,
            count: 1,
        }));
        assert!(log.contains(&SceneEffect::ActorFaced {
            actor: ActorRef::Npc(FIRST_OBJECT + row),
            facing: Direction::Down,
        }));
        // The object stands where the friend stood, off the trail.
        let seat = runner.actor(ActorRef::PartyMember(4)).expect("seated");
        assert_eq!(seat.cell, Cell::new(x, y));
        assert_eq!(seat.facing, Direction::Down);
        assert!(log.contains(&SceneEffect::PartyMemberWalksIn { slot: 4 }));
        assert!(log.contains(&SceneEffect::PartyChanged));
    }
}

#[test]
fn a_second_friend_takes_the_first_ones_slot_and_stand() {
    // Hahn along; Kyra answers YES.
    let mut state = GameState::new();
    let mut party = four();
    party[4] = Some(HAHN);
    state.set_party(party);
    state.set(Flag::event(0xDB)).unwrap();
    let mut runner = runner_at(4, 5);
    runner
        .actor(ActorRef::PartyMember(4))
        .expect("Hahn's object");
    let log = play(&mut runner, &mut state, true);
    assert_eq!(dialogues(&log), vec![0x12, 0x14, 0x05], "Hahn says goodbye");
    assert_eq!(state.party_slot(4), Some(KYRA));
    assert_eq!(picked_flags(&state), vec![0xDF]);
    // Hahn's home object is back at his stand, with his party object's facing.
    assert!(log.contains(&SceneEffect::NpcRestored {
        npc_index: FIRST_OBJECT + 1,
        facing: Direction::Up,
    }));
    assert_eq!(
        runner.actor(ActorRef::Npc(FIRST_OBJECT + 1)).unwrap().cell,
        Cell::new(26, 29)
    );
    // Kyra's object is clear, and her party object stands at her stand.
    assert!(log.contains(&SceneEffect::NpcDespawned {
        npc_index: FIRST_OBJECT + 4,
        count: 1,
    }));
    let seat = runner.actor(ActorRef::PartyMember(4)).unwrap();
    assert_eq!(seat.cell, Cell::new(33, 27));
    assert!(log.contains(&SceneEffect::PartyMemberWalksIn { slot: 4 }));
}

#[test]
fn the_exchange_keeps_the_leavers_slot_whichever_it_is() {
    // `loc_72156` scans the slots: a friend in slot 1 hands over slot 1.
    let mut state = GameState::new();
    state.set_party([Some(CHAZ), Some(GRYZ), Some(RUNE), Some(RIKA), Some(WREN)]);
    let mut runner = runner_at(2, 5);
    let log = play(&mut runner, &mut state, true);
    assert_eq!(dialogues(&log), vec![0x0A, 0x0C, 0x09], "Gryz's goodbye");
    assert_eq!(
        state.party(),
        [Some(CHAZ), Some(DEMI), Some(RUNE), Some(RIKA), Some(WREN)]
    );
    assert!(log.contains(&SceneEffect::PartyMemberWalksIn { slot: 1 }));
}

#[test]
fn no_leaves_the_party_and_the_flags_but_refills_the_records() {
    let mut state = GameState::new();
    let mut party = four();
    party[4] = Some(HAHN);
    state.set_party(party);
    state.set(Flag::event(0xDB)).unwrap();
    state.roster_mut().seat(RAJA, common::stats()).unwrap();
    let record = state.roster_mut().get_mut(RAJA).expect("seated");
    record.curr_hp = 0;
    record.curr_tp = 0;
    record.status = 4;
    let mut runner = runner_at(3, 5);
    let log = play(&mut runner, &mut state, false);
    assert_eq!(dialogues(&log), vec![0x0E, 0x10]);
    assert_eq!(state.party_slot(4), Some(HAHN));
    assert_eq!(picked_flags(&state), vec![0xDB]);
    assert!(
        !log.iter()
            .any(|e| matches!(e, SceneEffect::NpcDespawned { .. })),
        "{log:?}"
    );
    let raja = state.roster().get(RAJA).unwrap();
    assert_eq!((raja.curr_hp, raja.curr_tp), (raja.max_hp, raja.max_tp));
    assert_eq!(raja.status, 0, "`loc_71EAE` ran before the question");
}

#[test]
fn the_macros_follow_the_exchange() {
    let mut state = GameState::new();
    let mut party = four();
    party[4] = Some(HAHN);
    state.set_party(party);
    let command = |who: CharId, command_index| MacroCommand {
        character_id: who.0,
        command_index,
        ability_id: 7,
        reserved: 0,
    };
    let macros = state.macros_mut();
    macros[0].commands = [
        command(CHAZ, 2),
        command(HAHN, 3),
        command(RUNE, 2),
        MacroCommand::default(),
        MacroCommand::default(),
    ];
    let untouched = macros[1];
    let mut runner = runner_at(0, 5);
    play(&mut runner, &mut state, true);
    let got = state.macros()[0].commands;
    assert_eq!(got[0], command(CHAZ, 2));
    assert_eq!(got[1], command(RUNE, 2), "moved up over Hahn");
    assert_eq!(
        got[2],
        MacroCommand {
            character_id: GRYZ.0,
            command_index: 1,
            ability_id: 0,
            reserved: 0,
        },
        "the new member's plain attack in the first free slot"
    );
    assert_eq!(state.macros()[1], untouched, "a macro not in use");
}

#[test]
fn nobody_ahead_of_the_leader_faults_rather_than_reading_past_the_table() {
    let scene = scene_for(EventIndex(0x68)).unwrap();
    let cast = vec![ScriptedActor::new(
        ActorRef::PartyMember(0),
        Cell::new(2, 2),
        Direction::Up,
    )];
    let mut runner = SceneRunner::new(scene.ops, cast, StepFrames::new(8).unwrap());
    let mut state = GameState::new();
    let map = map_with(&["...."; 4], vec![], vec![]);
    let log = runner.tick(&map, &mut state, SceneInput::None);
    assert!(
        log.contains(&SceneEffect::Faulted(SceneFault::NoFifthRow)),
        "{log:?}"
    );
}
