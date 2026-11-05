//! `Event_PickingFifthCharacter` (`$68`, #83): the row from where the leader
//! stands, both ways onto the party, a NO, and what each leg writes. Record:
//! `docs/scenes/181_PickingFifthCharacter.md`. The tables are made up: the
//! cartridge's live in the local pack.

mod common;

use common::map_with;
use psiv_core::{
    ActorRef, Cell, CharId, DialogueId, Direction, EventIndex, FifthRow, FifthTable, Flag,
    GameState, MacroCommand, SceneEffect, SceneFault, SceneInput, SceneRunner, ScriptedActor,
    StepFrames, scene_for,
};

const CHAZ: CharId = CharId(0);
const RUNE: CharId = CharId(3);
const RIKA: CharId = CharId(5);
const WREN: CharId = CharId(7);

const FIRST_OBJECT: usize = 5;

/// A made-up table: (who, stand cell, first line, picked flag), friend `n`
/// standing at cell `(2 + 2n, 3)`.
fn friend(n: usize) -> (CharId, Cell, u16, u16) {
    let n8 = n as u8;
    (
        CharId(20 + n8),
        Cell::new(2 + 2 * n as u16, 3),
        0x30 + 4 * n as u16,
        0x60 + n as u16,
    )
}

fn table() -> FifthTable {
    let row = |n: usize| {
        let (who, cell, first_line, picked) = friend(n);
        // The standing shift: a stand's Y word is one row above its cell.
        FifthRow {
            x: i32::from(cell.x) * 16,
            y: (i32::from(cell.y) - 1) * 16,
            who,
            first_line,
            picked,
        }
    };
    FifthTable {
        rows: [row(0), row(1), row(2), row(3), row(4)],
        records: [0, 1, 2, 3, 4].map(|n| friend(n).0),
    }
}

fn four() -> [Option<CharId>; 5] {
    [Some(CHAZ), Some(RUNE), Some(RIKA), Some(WREN), None]
}

/// A runner on the five friends' objects with the leader below `row`'s
/// friend, facing up.
fn runner_at(row: usize, party_size: usize) -> SceneRunner {
    let stand = friend(row).1;
    let scene = scene_for(EventIndex(0x68)).expect("registered");
    let mut cast = vec![ScriptedActor::new(
        ActorRef::PartyMember(0),
        Cell::new(stand.x, stand.y + 1),
        Direction::Up,
    )];
    for slot in 1..party_size {
        cast.push(ScriptedActor::new(
            ActorRef::PartyMember(slot),
            Cell::new(stand.x, stand.y + 1 + slot as u16),
            Direction::Up,
        ));
    }
    for n in 0..5 {
        cast.push(ScriptedActor::new(
            ActorRef::Npc(FIRST_OBJECT + n),
            friend(n).1,
            Direction::Down,
        ));
    }
    let mut runner = SceneRunner::new(scene.ops, cast, StepFrames::new(8).unwrap());
    runner.set_fifth_table(table());
    runner
}

/// Plays with every dialogue closing at once, `answer` to the prompt and the
/// camera arriving, to the end.
fn play(runner: &mut SceneRunner, state: &mut GameState, answer: bool) -> Vec<SceneEffect> {
    let map = map_with(&["............"; 12], vec![], vec![]);
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
    (0x60..=0x64)
        .filter(|id| state.is_set(Flag::event(*id)))
        .collect()
}

#[test]
fn each_friend_joins_the_free_fifth_slot() {
    for row in 0..5 {
        let (who, stand, first, flag) = friend(row);
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
        assert_eq!(seat.cell, stand);
        assert_eq!(seat.facing, Direction::Down);
        assert!(log.contains(&SceneEffect::PartyMemberWalksIn { slot: 4 }));
        assert!(log.contains(&SceneEffect::PartyChanged));
    }
}

#[test]
fn a_second_friend_takes_the_first_ones_slot_and_stand() {
    // Friend 1 along; friend 4 answers YES.
    let (along, along_stand, along_first, along_flag) = friend(1);
    let (picked, picked_stand, picked_first, picked_flag) = friend(4);
    let mut state = GameState::new();
    let mut party = four();
    party[4] = Some(along);
    state.set_party(party);
    state.set(Flag::event(along_flag)).unwrap();
    let mut runner = runner_at(4, 5);
    let log = play(&mut runner, &mut state, true);
    assert_eq!(
        dialogues(&log),
        vec![picked_first, picked_first + 2, along_first + 3],
        "the one along says goodbye"
    );
    assert_eq!(state.party_slot(4), Some(picked));
    assert_eq!(picked_flags(&state), vec![picked_flag]);
    // The leaver's home object is back at their stand, with their party
    // object's facing.
    assert!(log.contains(&SceneEffect::NpcRestored {
        npc_index: FIRST_OBJECT + 1,
        facing: Direction::Up,
    }));
    assert_eq!(
        runner.actor(ActorRef::Npc(FIRST_OBJECT + 1)).unwrap().cell,
        along_stand
    );
    assert!(log.contains(&SceneEffect::NpcDespawned {
        npc_index: FIRST_OBJECT + 4,
        count: 1,
    }));
    let seat = runner.actor(ActorRef::PartyMember(4)).unwrap();
    assert_eq!(seat.cell, picked_stand);
    assert!(log.contains(&SceneEffect::PartyMemberWalksIn { slot: 4 }));
}

#[test]
fn the_exchange_keeps_the_leavers_slot_whichever_it_is() {
    // `loc_72156` scans the slots: a friend in slot 1 hands over slot 1.
    let mut state = GameState::new();
    state.set_party([Some(CHAZ), Some(friend(0).0), Some(RUNE), Some(RIKA), Some(WREN)]);
    let mut runner = runner_at(2, 5);
    let log = play(&mut runner, &mut state, true);
    assert_eq!(
        dialogues(&log),
        vec![friend(2).2, friend(2).2 + 2, friend(0).2 + 3]
    );
    assert_eq!(
        state.party(),
        [Some(CHAZ), Some(friend(2).0), Some(RUNE), Some(RIKA), Some(WREN)]
    );
    assert!(log.contains(&SceneEffect::PartyMemberWalksIn { slot: 1 }));
}

#[test]
fn no_leaves_the_party_and_the_flags_but_refills_the_records() {
    let mut state = GameState::new();
    let mut party = four();
    party[4] = Some(friend(1).0);
    state.set_party(party);
    state.set(Flag::event(friend(1).3)).unwrap();
    let asked = friend(3).0;
    state.roster_mut().seat(asked, common::stats()).unwrap();
    let record = state.roster_mut().get_mut(asked).expect("seated");
    record.curr_hp = 0;
    record.curr_tp = 0;
    record.status = 4;
    let mut runner = runner_at(3, 5);
    let log = play(&mut runner, &mut state, false);
    assert_eq!(dialogues(&log), vec![friend(3).2, friend(3).2 + 2]);
    assert_eq!(state.party_slot(4), Some(friend(1).0));
    assert_eq!(picked_flags(&state), vec![friend(1).3]);
    assert!(
        !log.iter()
            .any(|e| matches!(e, SceneEffect::NpcDespawned { .. })),
        "{log:?}"
    );
    let record = state.roster().get(asked).unwrap();
    assert_eq!((record.curr_hp, record.curr_tp), (record.max_hp, record.max_tp));
    assert_eq!(record.status, 0, "`loc_71EAE` ran before the question");
}

#[test]
fn the_macros_follow_the_exchange() {
    let along = friend(1).0;
    let mut state = GameState::new();
    let mut party = four();
    party[4] = Some(along);
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
        command(along, 3),
        command(RUNE, 2),
        MacroCommand::default(),
        MacroCommand::default(),
    ];
    let untouched = macros[1];
    let mut runner = runner_at(0, 5);
    play(&mut runner, &mut state, true);
    let got = state.macros()[0].commands;
    assert_eq!(got[0], command(CHAZ, 2));
    assert_eq!(got[1], command(RUNE, 2), "moved up over the leaver");
    assert_eq!(
        got[2],
        MacroCommand {
            character_id: friend(0).0.0,
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
        Cell::new(11, 11),
        Direction::Up,
    )];
    let mut runner = SceneRunner::new(scene.ops, cast, StepFrames::new(8).unwrap());
    runner.set_fifth_table(table());
    let mut state = GameState::new();
    let map = map_with(&["............"; 12], vec![], vec![]);
    let log = runner.tick(&map, &mut state, SceneInput::None);
    assert!(
        log.contains(&SceneEffect::Faulted(SceneFault::NoFifthRow)),
        "{log:?}"
    );
}

#[test]
fn a_runner_without_the_packs_table_faults() {
    let scene = scene_for(EventIndex(0x68)).unwrap();
    let cast = vec![ScriptedActor::new(
        ActorRef::PartyMember(0),
        Cell::new(2, 4),
        Direction::Up,
    )];
    let mut runner = SceneRunner::new(scene.ops, cast, StepFrames::new(8).unwrap());
    let mut state = GameState::new();
    let map = map_with(&["............"; 12], vec![], vec![]);
    let log = runner.tick(&map, &mut state, SceneInput::None);
    assert!(
        log.contains(&SceneEffect::Faulted(SceneFault::NoFifthTable)),
        "{log:?}"
    );
}
