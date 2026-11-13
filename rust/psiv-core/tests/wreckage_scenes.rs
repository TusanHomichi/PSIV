//! Scene outcomes for the Wreckage's system (`$29`), #81. Record:
//! `docs/scenes/206_WreckageEngine.md`.

mod common;

use common::map_with;
use psiv_core::battle::{StatPair, StatTriple, Stats};
use psiv_core::{
    ActorRef, Cell, CharId, Direction, EventIndex, Flag, GameState, SceneEffect, SceneInput,
    SceneOp, SceneRunner, ScriptedActor, StepFrames, scene_for,
};

const CHAZ: CharId = CharId(0);
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

const LEADER_AT: Cell = Cell::new(4, 5);
const SECOND_AT: Cell = Cell::new(4, 6);

/// Runs `$29` to its end and returns the effects and where each party slot
/// ended up.
fn play(state: &mut GameState) -> (Vec<SceneEffect>, Vec<Cell>) {
    let map = map_with(&["........"; 10], vec![], vec![]);
    let scene = scene_for(EventIndex(0x29)).expect("registered");
    let mut cast = Vec::new();
    for slot in 0..state.party_len() {
        let at = match slot {
            0 => LEADER_AT,
            1 => SECOND_AT,
            n => Cell::new(4, 6 + n as u16),
        };
        cast.push(ScriptedActor::new(
            ActorRef::PartyMember(slot),
            at,
            Direction::Up,
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
    assert!(runner.is_finished(), "{log:?}");
    assert!(
        !log.iter().any(|e| matches!(e, SceneEffect::Faulted(_))),
        "{log:?}"
    );
    let cells = (0..state.party_len())
        .map(|slot| runner.actor(ActorRef::PartyMember(slot)).unwrap().cell)
        .collect();
    (log, cells)
}

fn lines(log: &[SceneEffect]) -> Vec<u16> {
    log.iter()
        .filter_map(|e| match e {
            SceneEffect::DialogueOpen(id) => Some(id.0),
            _ => None,
        })
        .collect()
}

fn portraits(log: &[SceneEffect]) -> Vec<u8> {
    log.iter()
        .filter_map(|e| match e {
            SceneEffect::ResumePortrait { portrait } => Some(*portrait),
            _ => None,
        })
        .collect()
}

fn chunk_writes(log: &[SceneEffect]) -> Vec<(u32, u32, u16)> {
    log.iter()
        .filter_map(|e| match e {
            SceneEffect::MapChunksWritten { chunks } => Some(chunks.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

fn moved(log: &[SceneEffect]) -> bool {
    log.iter()
        .any(|e| matches!(e, SceneEffect::ActorMoveStarted { .. }))
}

#[test]
fn rika_reads_the_system_and_the_floor_lights_outward() {
    let mut state = party(&[CHAZ, RIKA, DEMI, GRYZ]);
    let (log, cells) = play(&mut state);
    assert_eq!(lines(&log), [0x47]);
    assert_eq!(
        log.iter()
            .filter(|e| matches!(e, SceneEffect::DialogueResume))
            .count(),
        2
    );
    assert_eq!(portraits(&log), [7], "Demi, the first past Chaz and Rika");
    assert!(state.is_set(Flag::event(0x40)));
    let writes = chunk_writes(&log);
    assert_eq!(writes.len(), 32, "eight passes of four");
    assert!(writes.iter().all(|&(_, _, id)| id == 0xA6));
    assert_eq!(
        &writes[..4],
        &[
            (15, 12, 0xA6),
            (14, 12, 0xA6),
            (15, 13, 0xA6),
            (14, 13, 0xA6)
        ]
    );
    assert_eq!(
        &writes[28..],
        &[(22, 12, 0xA6), (7, 12, 0xA6), (22, 13, 0xA6), (7, 13, 0xA6)]
    );
    assert!(moved(&log), "Rika and the leader trade places");
    assert_eq!(cells[..2], [LEADER_AT, SECOND_AT], "and trade back");
}

/// Rika in slot 0 skips both swaps (`tst.b d1 / beq`), and the second one's
/// face and pan with it.
#[test]
fn rika_leading_reads_it_where_she_stands() {
    let mut state = party(&[RIKA, CHAZ, GRYZ]);
    let (log, _) = play(&mut state);
    assert!(!moved(&log));
    assert_eq!(portraits(&log), [5], "Gryz");
    assert!(state.is_set(Flag::event(0x40)));
}

fn faced(log: &[SceneEffect]) -> Vec<(ActorRef, Direction)> {
    log.iter()
        .filter_map(|e| match e {
            SceneEffect::ActorFaced { actor, facing } => Some((*actor, *facing)),
            _ => None,
        })
        .collect()
}

fn pans(log: &[SceneEffect]) -> usize {
    log.iter()
        .filter(|e| {
            matches!(
                e,
                SceneEffect::Presentation {
                    op: SceneOp::PanCamera { .. }
                }
            )
        })
        .count()
}

/// The two turns the source notes count as the port's facing frames (Rika
/// after the swap, the leader after the swap back), and both pans.
#[test]
fn rika_following_turns_twice_and_pans_twice() {
    let mut state = party(&[CHAZ, RIKA, DEMI]);
    let (log, _) = play(&mut state);
    assert_eq!(
        faced(&log),
        [
            (ActorRef::Character(RIKA), Direction::Up),
            (ActorRef::PartyMember(0), Direction::Up)
        ]
    );
    assert_eq!(pans(&log), 2);
}

/// The recorded deviation: with Rika absent the cartridge's `tst.b d1` sees
/// `-1` and takes the swap path at both tests; the port takes `if_leads` at
/// both, so nothing moves or turns and the camera does not pan back
/// (`docs/source-notes/README.md`, `Event_WreckageEngine`).
#[test]
fn rika_absent_skips_both_swaps() {
    let mut state = party(&[CHAZ, DEMI, GRYZ]);
    let (log, cells) = play(&mut state);
    assert!(!moved(&log));
    assert!(faced(&log).is_empty());
    assert_eq!(pans(&log), 1, "only the pan to the floor");
    assert_eq!(chunk_writes(&log).len(), 32);
    assert_eq!(portraits(&log), [7], "Demi");
    assert!(state.is_set(Flag::event(0x40)));
    assert_eq!(cells[..2], [LEADER_AT, SECOND_AT]);
}

/// Read before: entry `$48` and nothing else.
#[test]
fn once_read_it_is_one_line() {
    let mut state = party(&[CHAZ, RIKA, DEMI]);
    state.set(Flag::event(0x40)).unwrap();
    let (log, _) = play(&mut state);
    assert_eq!(lines(&log), [0x48]);
    assert!(chunk_writes(&log).is_empty());
    assert!(portraits(&log).is_empty());
}

#[test]
fn the_radar_reset_writes_the_last_two_palette_words() {
    let mut state = party(&[CHAZ, RIKA, DEMI]);
    let (log, _) = play(&mut state);
    let offsets: Vec<_> = log
        .iter()
        .filter_map(|effect| match effect {
            SceneEffect::Presentation {
                op:
                    SceneOp::Presentation {
                        op: psiv_core::PresentationOp::SetPaletteWords { offset, .. },
                    },
            } => Some(*offset),
            _ => None,
        })
        .collect();
    // $FFFFFB7C/$FFFFFB7E, relative to Palette_Table_Buffer ($FFFFFB00).
    assert_eq!(offsets, [0x7C / 2]);
}
