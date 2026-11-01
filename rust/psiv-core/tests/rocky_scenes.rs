//! Scene outcomes for Rocky (#81): `$73` and `$74`, on each of the routine's
//! three maps. Every case cites the retail bytes (`docs/scenes/141_Rocky.md`).

mod common;

use common::grid;
use psiv_core::{
    ActorRef, Cell, Direction, EventIndex, FieldMap, Flag, GameState, MapId, SceneEffect,
    SceneInput, SceneRunner, ScriptedActor, StepFrames, scene_for,
};

const SHORTCAKE: u8 = 0x91;

/// A played scene's effects with the tick of each.
fn play(event: u16, map_id: u16, state: &mut GameState) -> Vec<(usize, SceneEffect)> {
    let row = ".".repeat(64);
    let rows = vec![row.as_str(); 64];
    let map = FieldMap::new(MapId(map_id), grid(&rows), vec![], vec![]).unwrap();
    let scene = scene_for(EventIndex(event)).unwrap_or_else(|| panic!("${event:04X} unregistered"));
    let mut cast = vec![ScriptedActor::new(
        ActorRef::PartyMember(0),
        Cell::new(40, 45),
        Direction::Up,
    )];
    for npc in 0..12 {
        cast.push(ScriptedActor::new(
            ActorRef::Npc(npc),
            Cell::new(38, 44),
            Direction::Down,
        ));
    }
    let mut runner = SceneRunner::new(scene.ops, cast, StepFrames::new(8).unwrap());
    let mut log = Vec::new();
    let mut input = SceneInput::None;
    for tick in 0..2_000 {
        let effects = runner.tick(&map, state, input);
        input = if effects.iter().any(|e| {
            matches!(
                e,
                SceneEffect::DialogueOpen(_) | SceneEffect::DialogueResume
            )
        }) {
            SceneInput::DialogueClosed
        } else if effects
            .iter()
            .any(|e| matches!(e, SceneEffect::MapRequested { .. }))
        {
            SceneInput::MapLoaded
        } else {
            SceneInput::None
        };
        log.extend(effects.into_iter().map(|e| (tick, e)));
        if runner.is_finished() {
            break;
        }
    }
    assert!(runner.is_finished(), "${event:04X} did not finish: {log:?}");
    assert!(
        !log.iter()
            .any(|(_, e)| matches!(e, SceneEffect::Faulted(_))),
        "{log:?}"
    );
    log
}

fn lines(log: &[(usize, SceneEffect)]) -> Vec<u16> {
    log.iter()
        .filter_map(|(_, e)| match e {
            SceneEffect::DialogueOpen(id) => Some(id.0),
            _ => None,
        })
        .collect()
}

fn has(log: &[(usize, SceneEffect)], wanted: &SceneEffect) -> bool {
    log.iter().any(|(_, e)| e == wanted)
}

/// `$07269E`: turns (object 7), line `$2C`, flag `$20`.
#[test]
fn found_at_monsen_turns_speaks_and_sets_his_flag() {
    let mut state = GameState::new();
    let log = play(0x73, 0x73, &mut state);
    assert_eq!(lines(&log), [0x2C]);
    assert!(state.is_set(Flag::event(0x20)));
    assert!(log.iter().any(|(_, e)| matches!(
        e,
        SceneEffect::ActorFaced {
            actor: ActorRef::Npc(7),
            ..
        }
    )));
}

/// `loc_72866`, each row: the map, the object, the first line and the flag for
/// the map he leaves. He runs eleven cells left at four frames a cell
/// (`loc_4BC80`'s step offset 2, so 44 ticks, not the walk's 88), is gone, the
/// text resumes, and only that map's flag is on, whichever was on before.
#[test]
fn without_a_shortcake_he_runs_off_each_map_by_its_row() {
    for (map, npc, line, flag) in [
        (0x39, 11, 0x35, 0x22),
        (0x73, 7, 0x2A, 0x23),
        (0x7B, 9, 0x4C, 0x21),
    ] {
        let mut state = GameState::new();
        for stale in [0x21, 0x22, 0x23] {
            state.set(Flag::event(stale)).unwrap();
        }
        let log = play(0x74, map, &mut state);
        assert_eq!(lines(&log), [line]);
        let actor = ActorRef::Npc(npc);
        let to = Cell::new(38 - 11, 44);
        let tick_of = |wanted: &SceneEffect| {
            log.iter()
                .find_map(|(tick, e)| (e == wanted).then_some(*tick))
                .unwrap_or_else(|| panic!("${map:02X}: {wanted:?} in {log:?}"))
        };
        let started = tick_of(&SceneEffect::ActorMoveStarted { actor, to });
        let arrived = tick_of(&SceneEffect::ActorArrived { actor, at: to });
        assert_eq!(arrived - started, 44, "${map:02X}");
        assert!(has(
            &log,
            &SceneEffect::ObjectRoutineSet {
                npc_index: npc,
                routine: 0x2F8,
            }
        ));
        assert!(has(
            &log,
            &SceneEffect::NpcDespawned {
                npc_index: npc,
                count: 1,
            }
        ));
        assert!(has(&log, &SceneEffect::DialogueResume));
        for id in [0x21, 0x22, 0x23] {
            assert_eq!(
                state.is_set(Flag::event(id)),
                id == flag,
                "${map:02X} ${id:02X}"
            );
        }
        assert!(!state.is_set(Flag::event(0x24)));
    }
}

/// `loc_727CA`: the caught line of the row, every shortcake gone with the
/// holes closed, `$24` set, the map loaded and the owner's line in its tree.
/// This arm clears no escape flag.
#[test]
fn with_a_shortcake_he_goes_home_and_the_cakes_go() {
    for (map, line) in [(0x39, 0x36), (0x73, 0x2B), (0x7B, 0x4D)] {
        let mut state = GameState::new();
        for item in [0x01, SHORTCAKE, 0x02, SHORTCAKE] {
            state.inventory_mut().add(item).unwrap();
        }
        state.set(Flag::event(0x22)).unwrap();
        let log = play(0x74, map, &mut state);
        assert_eq!(lines(&log), [line, 0x46]);
        assert!(!state.inventory().contains(SHORTCAKE));
        assert_eq!(state.inventory().slots()[..3], [0x01, 0x02, 0x00]);
        assert!(state.is_set(Flag::event(0x24)));
        assert!(state.is_set(Flag::event(0x22)));
        assert!(
            log.iter()
                .any(|(_, e)| matches!(e, SceneEffect::MapRequested { .. }))
        );
        assert!(has(
            &log,
            &SceneEffect::ActorFaced {
                actor: ActorRef::Npc(1),
                facing: Direction::Right,
            }
        ));
    }
}
