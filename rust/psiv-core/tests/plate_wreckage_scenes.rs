//! Scene outcomes for Phonon (`$6C`) and the Plate System's engine (`$2D`),
//! #81. Records: `docs/scenes/204_Phonon.md`, `205_PlateSysEngine.md`.

mod common;

use common::map_with;
use psiv_core::battle::{StatPair, StatTriple, Stats};
use psiv_core::{
    ActorRef, Cell, CharId, Direction, EventIndex, Flag, GameState, PresentationOp, SceneEffect,
    SceneInput, SceneOp, SceneRunner, ScriptedActor, StepFrames, scene_for,
};

const CHAZ: CharId = CharId(0);
const RUNE: CharId = CharId(3);
const GRYZ: CharId = CharId(4);
const RIKA: CharId = CharId(5);
const DEMI: CharId = CharId(6);

fn member() -> Stats {
    Stats {
        name_bytes: [0; 6],
        profession: 0,
        level: 20,
        experience: 0,
        curr_hp: 100,
        max_hp: 100,
        curr_tp: 0,
        max_tp: 0,
        status: 0,
        strength: StatTriple::uniform(8),
        mental: StatTriple::uniform(6),
        agility: StatTriple::uniform(7),
        dexterity: StatTriple::uniform(5),
        attack: StatPair::default(),
        defence: StatPair::default(),
        mental_defence: StatPair::default(),
        element_props: [0; 14],
        element_shadow: [0; 14],
        equipment: [0; 4],
        techniques: [0; 16],
        skills: [0; 8],
        curr_skill_uses: [0; 8],
        max_skill_uses: [0; 8],
        enemy_id: 0,
        gain_exp_flag: false,
        weapon_elements: Default::default(),
        physical_prop_save: 0,
    }
}

fn party(members: &[CharId]) -> GameState {
    let mut state = GameState::default();
    let mut slots = [None; 5];
    for (slot, who) in members.iter().enumerate() {
        state.roster_mut().seat(*who, member()).unwrap();
        slots[slot] = Some(*who);
    }
    state.set_party(slots);
    state
}

/// Runs `event` to its end, answering windows and the camera at once.
fn play(event: u16, state: &mut GameState) -> Vec<SceneEffect> {
    let map = map_with(&["........"; 8], vec![], vec![]);
    let scene = scene_for(EventIndex(event)).expect("registered");
    let mut cast = Vec::new();
    for slot in 0..state.party_len() {
        cast.push(ScriptedActor::new(
            ActorRef::PartyMember(slot),
            Cell::new(3, 6),
            Direction::Up,
        ));
    }
    for npc in 0..4 {
        cast.push(ScriptedActor::new(
            ActorRef::Npc(npc),
            Cell::new(1, 1),
            Direction::Down,
        ));
    }
    let mut runner = SceneRunner::new(scene.ops, cast, StepFrames::new(8).unwrap());
    let mut log = Vec::new();
    let mut input = SceneInput::None;
    for _ in 0..5_000 {
        let effects = runner.tick(&map, state, input);
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
    assert!(runner.is_finished(), "${event:04X}: {log:?}");
    assert!(
        !log.iter().any(|e| matches!(e, SceneEffect::Faulted(_))),
        "{log:?}"
    );
    log
}

fn lines(log: &[SceneEffect]) -> Vec<u16> {
    log.iter()
        .filter_map(|e| match e {
            SceneEffect::DialogueOpen(id) => Some(id.0),
            _ => None,
        })
        .collect()
}

fn chest_facings(log: &[SceneEffect]) -> Vec<Direction> {
    log.iter()
        .filter_map(|e| match e {
            SceneEffect::ActorFaced {
                actor: ActorRef::Npc(3),
                facing,
            } => Some(*facing),
            _ => None,
        })
        .collect()
}

#[test]
fn demi_takes_phonon_from_the_chest() {
    let mut state = party(&[CHAZ, RIKA, DEMI]);
    state.set(Flag::chest(0x42)).unwrap();
    let log = play(0x6C, &mut state);
    assert_eq!(lines(&log), [0x4E]);
    assert_eq!(state.roster().get(DEMI).unwrap().skills[5], 0x11);
    assert!(state.is_set(Flag::chest(0x42)), "the chest stays open");
    assert_eq!(chest_facings(&log), [Direction::Up]);
}

/// Without Demi the lid shuts and the chest's flag is cleared, so the chest
/// can be opened again once she joins.
#[test]
fn without_demi_the_chest_closes_again() {
    let mut state = party(&[CHAZ, RIKA, GRYZ]);
    state.set(Flag::chest(0x42)).unwrap();
    let log = play(0x6C, &mut state);
    assert_eq!(lines(&log), [0x4F]);
    assert!(!state.is_set(Flag::chest(0x42)));
    assert_eq!(chest_facings(&log), [Direction::Up, Direction::Down]);
    assert_eq!(state.roster().get(GRYZ).unwrap().skills, [0; 8]);
}

fn light_steps(log: &[SceneEffect]) -> Vec<(u16, u16)> {
    log.iter()
        .filter_map(|e| match e {
            SceneEffect::Presentation {
                op:
                    SceneOp::Presentation {
                        op: PresentationOp::SetPaletteWords { first, second, .. },
                    },
            } => Some((*first, *second)),
            _ => None,
        })
        .collect()
}

#[test]
fn the_engine_reading_ends_with_the_flag() {
    for (members, rune) in [
        (vec![CHAZ, GRYZ, DEMI, RIKA], false),
        (vec![CHAZ, GRYZ, DEMI, RIKA, RUNE], true),
    ] {
        let mut state = party(&members);
        let log = play(0x2D, &mut state);
        assert_eq!(lines(&log), [0x41]);
        assert!(log.iter().any(|e| matches!(e, SceneEffect::DialogueResume)));
        assert!(state.is_set(Flag::event(0x61)));
        let steps = light_steps(&log);
        assert_eq!(steps.len(), 17, "sixteen pulses and the last colour");
        assert_eq!(steps[5], (0x0A80, 0x7007), "the read past the table");
        assert_eq!(steps[16], (0x0EC0, 0x00CE));
        let rune_faced = log.iter().any(|e| {
            matches!(
                e,
                SceneEffect::ActorFaced {
                    actor: ActorRef::PartyMember(4) | ActorRef::Character(RUNE),
                    facing: Direction::Right
                }
            )
        });
        assert_eq!(rune_faced, rune, "Rune turns only when he is there");
    }
}
