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

/// Every palette record the scene emits, in order: the light's one table
/// record and the immediate writes.
fn palette_records(log: &[SceneEffect]) -> Vec<PresentationOp> {
    log.iter()
        .filter_map(|e| match e {
            SceneEffect::Presentation {
                op:
                    SceneOp::Presentation {
                        op:
                            op @ (PresentationOp::SetPaletteWords { .. }
                            | PresentationOp::PaletteCycleFromTable { .. }),
                    },
            } => Some(*op),
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
        // The light is one record naming `loc_6E908`, whose words stay in
        // the pack, then the last colour from the code's immediates. Both
        // address `Palette_Table_Buffer + $38`, word offset `$1C`.
        assert_eq!(
            palette_records(&log),
            [
                PresentationOp::PaletteCycleFromTable {
                    source_rom_addr: 0x06_E908,
                    offset: 0x1C,
                    second_skip: 0xE,
                    steps: 16,
                    frame_delay: 8,
                },
                PresentationOp::SetPaletteWords {
                    offset: 0x1C,
                    first: 0x0EC0,
                    second: 0x00CE,
                },
            ]
        );
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

/// Which frames of the engine's light and the turn after it run the map's
/// updates: `loc_6E922`'s waits do not, `RunMapUpdates` then `VInt_Prepare`
/// does once, and both `DoMapUpdateLoop`s do.
#[test]
fn the_engine_light_runs_one_map_update_before_demi_turns() {
    let mut state = party(&[CHAZ, GRYZ, DEMI, RIKA]);
    let map = map_with(&["........"; 8], vec![], vec![]);
    let scene = scene_for(EventIndex(0x2D)).expect("registered");
    let cast = (0..4)
        .map(|slot| ScriptedActor::new(ActorRef::PartyMember(slot), Cell::new(3, 6), Direction::Up))
        .collect();
    let mut runner = SceneRunner::new(scene.ops, cast, StepFrames::new(8).unwrap());
    // From the frame the light starts to the dialogue's resume: `L` light,
    // `H` the last colour and its hold, `R`/`D` Demi turning right/down;
    // each frame after records whether it updates the map (`+`) or not (`.`).
    // The one `+` before `R` is `RunMapUpdates`.
    let mut trace = String::new();
    let mut input = SceneInput::None;
    let mut started = false;
    for _ in 0..5_000 {
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
        for e in &effects {
            let mark = match e {
                SceneEffect::Presentation {
                    op:
                        SceneOp::Presentation {
                            op: PresentationOp::PaletteCycleFromTable { .. },
                        },
                } => Some('L'),
                SceneEffect::Presentation {
                    op:
                        SceneOp::Presentation {
                            op: PresentationOp::SetPaletteWords { .. },
                        },
                } => Some('H'),
                SceneEffect::ActorFaced {
                    actor: ActorRef::Character(DEMI),
                    facing: Direction::Right,
                } => Some('R'),
                SceneEffect::ActorFaced {
                    actor: ActorRef::Character(DEMI),
                    facing: Direction::Down,
                } => Some('D'),
                SceneEffect::DialogueResume => Some('|'),
                _ => None,
            };
            if let Some(mark) = mark {
                started |= mark == 'L';
                if started {
                    trace.push(mark);
                }
            }
        }
        if trace.ends_with('|') {
            break;
        }
        if started {
            trace.push(if runner.runs_map_updates() { '+' } else { '.' });
        }
    }
    let expected = format!(
        "L{}H{}+R{}D{}|",
        ".".repeat(128),
        ".".repeat(40),
        "+".repeat(15),
        "+".repeat(30),
    );
    assert_eq!(trace, expected);
}

/// `palette_cycle_writes` on made-up words (not the cartridge's): a negative
/// word restarts the table without spending a pass, the second word is read
/// `skip` bytes past the advanced pointer, past the table's end when the
/// pointer is near it, and a window too short for a read or a table with no
/// word to write gives `None`.
#[test]
fn palette_cycle_writes_follow_the_cartridge_loop() {
    // Table 0x101, 0x202, 0x303, then the terminator and the words after it.
    let words = [0x101, 0x202, 0x303, 0x8000, 0x011, 0x022, 0x033, 0x044];
    let writes = psiv_core::palette_cycle_writes(&words, 6, 7).unwrap();
    assert_eq!(
        writes,
        [
            (0x101, 0x011),
            (0x202, 0x022),
            (0x303, 0x033),
            (0x101, 0x011),
            (0x202, 0x022),
            (0x303, 0x033),
            (0x101, 0x011),
        ]
    );
    // The third pass's second word is `words[6]`: a window ending before it
    // gives `None`, one pass fewer fits.
    assert_eq!(psiv_core::palette_cycle_writes(&words[..6], 6, 3), None);
    assert_eq!(
        psiv_core::palette_cycle_writes(&words[..6], 6, 2).map(|w| w.len()),
        Some(2)
    );
    // A shorter skip reads the terminator itself as the second pass's colour.
    assert_eq!(
        psiv_core::palette_cycle_writes(&words[..6], 2, 3),
        Some(vec![(0x101, 0x303), (0x202, 0x8000), (0x303, 0x011)])
    );
    // No word to write, and an odd skip the 68000 could not read.
    assert_eq!(
        psiv_core::palette_cycle_writes(&[0x8000, 0xFFFF], 2, 1),
        None
    );
    assert_eq!(psiv_core::palette_cycle_writes(&words, 3, 1), None);
    assert_eq!(psiv_core::palette_cycle_writes(&words, 6, 0), Some(vec![]));
}
