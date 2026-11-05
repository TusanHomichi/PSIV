//! Scene outcomes for the guild's missing student (#81): `$76` to `$79`. Every
//! case cites the retail bytes (`docs/scenes/160_MissingStudent.md`).

mod common;

use common::map_with;
use psiv_core::{
    ActorRef, Cell, Direction, EventIndex, Flag, GameState, SceneEffect, SceneInput, SceneOp,
    SceneRunner, ScriptedActor, StepFrames, scene_for,
};

const DORM_OWNER: u16 = 0x27;
const STUDENT_SICK: u16 = 0x28;
const PEROLYMATE: u16 = 0x29;
const STUDENT_RECOVERED: u16 = 0x2A;
const ITEM_PEROLYMATE: u8 = 0x93;

/// Runs `event` to its end, answering each dialogue and map load at once, and
/// returns every effect in order.
fn play(event: u16, state: &mut GameState) -> Vec<SceneEffect> {
    let map = map_with(&["....", "....", "....", "...."], vec![], vec![]);
    let scene = scene_for(EventIndex(event)).unwrap_or_else(|| panic!("${event:04X} unregistered"));
    let mut cast = vec![ScriptedActor::new(
        ActorRef::PartyMember(0),
        Cell::new(2, 3),
        Direction::Up,
    )];
    for npc in 0..5 {
        cast.push(ScriptedActor::new(
            ActorRef::Npc(npc),
            Cell::new(1, 1),
            Direction::Down,
        ));
    }
    let mut runner = SceneRunner::new(scene.ops, cast, StepFrames::new(8).unwrap());
    let mut log = Vec::new();
    let mut input = SceneInput::None;
    for _ in 0..1_000 {
        let effects = runner.tick(&map, state, input);
        input = SceneInput::None;
        for effect in &effects {
            match effect {
                SceneEffect::DialogueOpen(_) => input = SceneInput::DialogueClosed,
                SceneEffect::MapRequested { .. } => input = SceneInput::MapLoaded,
                SceneEffect::Faulted(fault) => panic!("${event:04X}: {fault:?}"),
                _ => {}
            }
        }
        log.extend(effects);
        if runner.is_finished() {
            return log;
        }
    }
    panic!("${event:04X} did not finish");
}

/// The dialogue entries the scene opened, in order.
fn lines(log: &[SceneEffect]) -> Vec<u16> {
    log.iter()
        .filter_map(|effect| match effect {
            SceneEffect::DialogueOpen(id) => Some(id.0),
            _ => None,
        })
        .collect()
}

/// The object facings the scene wrote for `npc`, in order.
fn facings(log: &[SceneEffect], npc: usize) -> Vec<Direction> {
    log.iter()
        .filter_map(|effect| match effect {
            SceneEffect::ActorFaced { actor, facing } if *actor == ActorRef::Npc(npc) => {
                Some(*facing)
            }
            _ => None,
        })
        .collect()
}

fn state_with(flags: &[u16]) -> GameState {
    let mut state = GameState::new();
    for flag in flags {
        state.set(Flag::event(*flag)).unwrap();
    }
    state
}

fn set(state: &GameState, flag: u16) -> bool {
    state.is_set(Flag::event(flag))
}

/// The index of the first op `wanted` accepts, for the checks on order.
fn index_of(event: u16, wanted: impl Fn(&SceneOp) -> bool) -> usize {
    scene_for(EventIndex(event))
        .unwrap()
        .ops
        .iter()
        .position(wanted)
        .unwrap()
}

/// `$072898..$0728BD`: entry `$6C`, then the flag.
#[test]
fn the_dorm_owner_speaks_and_sets_her_flag() {
    let mut state = GameState::new();
    let log = play(0x76, &mut state);
    assert_eq!(lines(&log), [0x6C]);
    assert!(set(&state, DORM_OWNER));
}

/// `$0728D4..$0728E5`: with `EventFlag_DormOwner` clear, entry `$52` and no
/// write.
#[test]
fn the_student_only_talks_before_the_owner_has_spoken() {
    let mut state = GameState::new();
    let log = play(0x77, &mut state);
    assert_eq!(lines(&log), [0x52]);
    assert!(!set(&state, STUDENT_SICK));
    assert!(
        !log.iter()
            .any(|e| matches!(e, SceneEffect::MapRequested { .. })),
        "no collapse"
    );
}

/// `$0728E6..$0729E3`: the two lines around the look round, then the inn.
#[test]
fn the_student_looks_round_collapses_and_the_party_wakes_at_the_inn() {
    let mut state = state_with(&[DORM_OWNER]);
    let log = play(0x77, &mut state);
    assert_eq!(lines(&log), [0x4C, 0x4D, 0x53]);
    assert!(set(&state, STUDENT_SICK));
    use Direction::{Down, Left, Right, Up};
    assert_eq!(
        facings(&log, 4)[1..],
        [Down, Right, Up, Left, Down, Right, Down],
        "the seven `Event_UpdateObjFacing` calls after the first line"
    );
    let flag_at = log
        .iter()
        .position(|e| {
            matches!(e, SceneEffect::FlagChanged { flag, value: true }
                if *flag == Flag::event(STUDENT_SICK))
        })
        .unwrap();
    let map_at = log
        .iter()
        .position(|e| matches!(e, SceneEffect::MapRequested { .. }))
        .unwrap();
    assert!(flag_at < map_at, "`$07299E` comes before the load");
    let load = index_of(0x77, |op| matches!(op, SceneOp::LoadMap { .. }));
    assert_eq!(
        scene_for(EventIndex(0x77)).unwrap().ops[load + 1],
        SceneOp::FadeIn,
        "`$0729D6`: the fade follows `RefreshMap`"
    );
}

/// `$0729FA..$072A0D`: the medicine goes through `GetItem`, and the list closes
/// up behind it.
#[test]
fn the_old_man_takes_one_perolymate_and_the_bag_closes_up() {
    let mut state = GameState::new();
    state.inventory_mut().add(0x01).unwrap();
    state.inventory_mut().add(ITEM_PEROLYMATE).unwrap();
    state.inventory_mut().add(0x02).unwrap();
    state.inventory_mut().add(ITEM_PEROLYMATE).unwrap();
    let log = play(0x78, &mut state);
    assert_eq!(lines(&log), [0x4F]);
    assert!(set(&state, PEROLYMATE));
    assert_eq!(state.inventory().slots()[..4], [0x01, 0x02, 0x93, 0x00]);
    assert!(log.contains(&SceneEffect::InventoryChanged));
}

/// `$072A1E`: no Perolymate, entry `$53`, nothing written.
#[test]
fn the_old_man_without_the_medicine_writes_nothing() {
    let mut state = GameState::new();
    state.inventory_mut().add(0x01).unwrap();
    let before = state.snapshot();
    let log = play(0x78, &mut state);
    assert_eq!(lines(&log), [0x53]);
    assert_eq!(state.snapshot(), before);
    assert!(!log.contains(&SceneEffect::InventoryChanged));
}

/// `GetItem` searches only as many slots as there are items, so a hole hides
/// the last one. The scene takes the absent path, as the cartridge would.
#[test]
fn a_perolymate_behind_a_hole_goes_unfound() {
    let mut state = GameState::new();
    state.inventory_mut().add(0x01).unwrap();
    state.inventory_mut().add(0x02).unwrap();
    state.inventory_mut().add(ITEM_PEROLYMATE).unwrap();
    state.inventory_mut().remove(0).unwrap();
    let log = play(0x78, &mut state);
    assert_eq!(lines(&log), [0x53]);
    assert!(!set(&state, PEROLYMATE));
    assert_eq!(state.inventory().slots()[..3], [0x00, 0x02, 0x93]);
}

/// `$072A3C..$072A4D`: before the medicine, entry `$50` and no write.
#[test]
fn the_student_in_bed_only_talks_before_the_medicine() {
    let mut state = GameState::new();
    let log = play(0x79, &mut state);
    assert_eq!(lines(&log), [0x50]);
    assert!(!set(&state, STUDENT_RECOVERED));
}

/// `$072A4E..$072ACF`: home to the dorm, with the fade before the load.
#[test]
fn the_student_recovers_and_the_party_stands_in_the_dorm() {
    let mut state = state_with(&[PEROLYMATE]);
    let log = play(0x79, &mut state);
    assert_eq!(lines(&log), [0x54, 0x5B]);
    assert!(set(&state, STUDENT_RECOVERED));
    let fade = index_of(0x79, |op| matches!(op, SceneOp::FadeIn));
    let load = index_of(0x79, |op| matches!(op, SceneOp::LoadMap { .. }));
    assert!(fade < load, "`$072A84` comes before `RefreshMap`");
}
