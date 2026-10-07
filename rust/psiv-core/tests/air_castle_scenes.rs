//! Scene outcomes on the Air Castle and Garuberk Tower stretch (lane C8):
//! each case runs the registered scene through `SceneRunner` and holds it to
//! what the cartridge's routine does, cited per case (`docs/scenes/102_*` to
//! `104_*`).

mod common;

use common::map_with;
use psiv_core::battle::{StatPair, StatTriple, Stats, status};
use psiv_core::{
    ActorRef, Cell, CharId, Direction, EventIndex, Flag, GameState, PresentationOp, SceneEffect,
    SceneInput, SceneOp, SceneRunner, ScriptedActor, StepFrames, scene_for,
};

fn member(curr_hp: u16, status: u8) -> Stats {
    Stats {
        name_bytes: [0; 6],
        profession: 0,
        level: 30,
        experience: 0,
        curr_hp,
        max_hp: 300,
        curr_tp: 3,
        max_tp: 90,
        status,
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
        curr_skill_uses: [0, 1, 0, 0, 0, 0, 0, 0],
        max_skill_uses: [4, 3, 0, 0, 0, 0, 0, 0],
        enemy_id: 0,
        gain_exp_flag: false,
        weapon_elements: Default::default(),
        physical_prop_save: 0,
    }
}

/// Chaz poisoned at 12 HP, Wren (record 7) shut down by bit 6 at full HP, as
/// the route carries her from Dark Force 1, and Rune out of the party, hurt.
fn party() -> GameState {
    let mut state = GameState::default();
    let roster = state.roster_mut();
    roster
        .seat(CharId(0), member(12, status::POISONED))
        .unwrap();
    roster
        .seat(CharId(7), member(300, status::ANDROID_DEAD))
        .unwrap();
    roster.seat(CharId(3), member(5, status::DEAD)).unwrap();
    state.set_party([Some(CharId(0)), Some(CharId(7)), None, None, None]);
    state
}

/// Runs `event` to its end with the leader on `leader` (the rest of the party
/// behind), counting the frames it blocks, and returns the effects with the
/// frame each was emitted on. A map request is answered at once, as the
/// runtime's load does; the test map is open ground.
fn play_at(event: u16, state: &mut GameState, leader: Cell) -> Vec<(u32, SceneEffect)> {
    let map = map_with(&["........"; 8], vec![], vec![]);
    let scene =
        scene_for(EventIndex(event)).unwrap_or_else(|| panic!("${event:04X} is not registered"));
    let cast = vec![
        ScriptedActor::new(ActorRef::PartyMember(0), leader, Direction::Up),
        ScriptedActor::new(ActorRef::PartyMember(1), leader, Direction::Up),
    ];
    let mut runner = SceneRunner::new(scene.ops, cast, StepFrames::new(8).unwrap());
    let mut log: Vec<(u32, SceneEffect)> = Vec::new();
    for frame in 0..1_000 {
        let input = match log.last() {
            Some((_, SceneEffect::MapRequested { .. })) => SceneInput::MapLoaded,
            _ => SceneInput::None,
        };
        for effect in runner.tick(&map, state, input) {
            log.push((frame, effect));
        }
        if runner.is_finished() {
            break;
        }
    }
    assert!(runner.is_finished(), "{} did not finish", scene.name);
    assert!(
        !log.iter()
            .any(|(_, e)| matches!(e, SceneEffect::Faulted(_))),
        "{} faulted: {log:?}",
        scene.name
    );
    log
}

fn play(event: u16, state: &mut GameState) -> Vec<(u32, SceneEffect)> {
    play_at(event, state, Cell::new(1, 1))
}

/// Every `PlaySound` with its frame.
fn sounds(log: &[(u32, SceneEffect)]) -> Vec<(u32, u8)> {
    log.iter()
        .filter_map(|(frame, e)| match e {
            SceneEffect::Presentation {
                op: SceneOp::PlaySound { id },
            } => Some((*frame, *id)),
            _ => None,
        })
        .collect()
}

/// A live chunk write: the frame and `(chunk x, chunk y, id)` in write order.
type ChunkWrite = (u32, Vec<(u32, u32, u16)>);

/// Every live chunk write, in order.
fn chunk_writes(log: &[(u32, SceneEffect)]) -> Vec<ChunkWrite> {
    log.iter()
        .filter_map(|(frame, e)| match e {
            SceneEffect::MapChunksWritten { chunks } => Some((*frame, chunks.clone())),
            _ => None,
        })
        .collect()
}

/// `Event_Recovery` (`$06D37C`, `ps4.asm:146852-146934`): the recovery SFX, six
/// sixteen-frame flash loops with no map update (`loc_6D3BA` / `loc_6D40A`,
/// `moveq #$F, d7` around `VInt_Prepare`), the Res SFX, then `RecoverStats`
/// (`ps4.asm:136522-136534`): every party member's HP, TP and skill uses to
/// the maximum and the status byte to zero, an android's shutdown bit
/// included. A member outside the party is not touched.
#[test]
fn the_recovery_tile_restores_the_party_after_ninety_six_frames() {
    let mut state = party();
    let log = play(0x21, &mut state);

    assert_eq!(
        sounds(&log),
        vec![(0, 0xCD), (96, 0xCC)],
        "SFX Recovery, then Res"
    );

    let flashes: Vec<bool> = log
        .iter()
        .filter_map(|(_, e)| match e {
            SceneEffect::Presentation {
                op:
                    SceneOp::Presentation {
                        op:
                            PresentationOp::PaletteToneFlash {
                                brighten,
                                words: 32,
                                frames: 16,
                            },
                    },
            } => Some(*brighten),
            _ => None,
        })
        .collect();
    assert_eq!(flashes, vec![true, false, true, false, true, false]);

    let cured: Vec<(u32, CharId)> = log
        .iter()
        .filter_map(|(frame, e)| match e {
            SceneEffect::RosterChanged { who } => Some((*frame, *who)),
            _ => None,
        })
        .collect();
    assert_eq!(cured, vec![(96, CharId(0)), (96, CharId(7))]);

    for who in [CharId(0), CharId(7)] {
        let stats = state.roster().get(who).unwrap();
        assert_eq!((stats.curr_hp, stats.curr_tp), (300, 90), "{who:?}");
        assert_eq!(
            stats.status, 0,
            "{who:?}: RecoverStats writes a zero status"
        );
        assert_eq!(stats.curr_skill_uses, stats.max_skill_uses, "{who:?}");
    }
    let rune = state.roster().get(CharId(3)).unwrap();
    assert_eq!(
        (rune.curr_hp, rune.status),
        (5, status::DEAD),
        "not in the party"
    );
}

/// The leader at (2,4) stands in chunk (1,1) (`curr_x_pos` 32, `curr_y_pos` 48);
/// the chunk above is (1,0): the door the opening animates.
const AT_DOOR: Cell = Cell::new(2, 4);

/// `Event_GaruberkTwDoorOpening1` (`$06F43A`, `ps4.asm:148638-148692`): after the
/// guard, `SFXID_Fusion` and `loc_6F4D4`'s nine rows, each the pair above and
/// under the leader then four frames; the door ends open, `$34` over `$3C`.
/// `Event_GaruberkTwDoorOpening2` (`$06F4F0`) walks `loc_6F588` and ends on `$3E`.
#[test]
fn the_door_openings_step_their_tables_around_the_leader() {
    let opening_1: [(u16, u16); 9] = [
        (0x30, 0x38),
        (0x31, 0x39),
        (0x32, 0x3A),
        (0x31, 0x39),
        (0x30, 0x38),
        (0x31, 0x39),
        (0x32, 0x3A),
        (0x33, 0x3B),
        (0x34, 0x3C),
    ];
    let opening_2: [(u16, u16); 9] = [
        (0x35, 0x38),
        (0x36, 0x39),
        (0x37, 0x3A),
        (0x36, 0x39),
        (0x35, 0x38),
        (0x36, 0x39),
        (0x37, 0x3A),
        (0x3D, 0x3B),
        (0x3E, 0x3C),
    ];
    for (event, table) in [(0x35, opening_1), (0x36, opening_2)] {
        let mut state = GameState::default();
        let log = play_at(event, &mut state, AT_DOOR);
        assert_eq!(sounds(&log), vec![(0, 0xD9)], "${event:02X}: SFXID_Fusion");
        let expected: Vec<ChunkWrite> = table
            .iter()
            .zip(0u32..)
            .map(|(&(above, under), row)| (4 * row, vec![(1, 0, above), (1, 1, under)]))
            .collect();
        assert_eq!(chunk_writes(&log), expected, "${event:02X}");
        let last = log.last().map(|(frame, _)| *frame);
        assert_eq!(last, Some(36), "${event:02X}: nine rows of four frames");
    }
}

/// `Event_GaruberkTwDoorEntered1` (`$06F5A4`, `ps4.asm:148747-148848`): fade out,
/// the map's own transition, `$3C` under the leader and `$3E` at `y - $10` on
/// arrival, fade in, `SFXID_EnemyAttack3`, one cell down, the party overlapped,
/// `SFXID_EnemyAttack4` and `loc_6F714` back to the closed door around the new
/// position. `$38` (`$06F724`) writes `$34` on arrival and walks `loc_6F894`.
#[test]
fn a_door_entered_warps_opens_the_arrival_steps_out_and_shuts_it() {
    // The arrival stands on the door cell (2,3), chunk column 1: `curr_y_pos`
    // 32 is chunk row 1 and `y - $10` row 0. One cell down, (2,4), the pair is
    // rows 0 and 1.
    let arrival = Cell::new(2, 3);
    /// The event, the chunk it writes above the arrival, and its closing rows.
    type Entered = (u16, u16, [(u16, u16); 5]);
    let cases: [Entered; 2] = [
        (
            0x37,
            0x3E,
            [
                (0x3E, 0x3C),
                (0x3D, 0x3B),
                (0x37, 0x3A),
                (0x36, 0x39),
                (0x35, 0x38),
            ],
        ),
        (
            0x38,
            0x34,
            [
                (0x34, 0x3C),
                (0x33, 0x3B),
                (0x32, 0x3A),
                (0x31, 0x39),
                (0x30, 0x38),
            ],
        ),
    ];
    for (event, lintel, closing) in cases {
        let mut state = GameState::default();
        let log = play_at(event, &mut state, arrival);
        let request = log
            .iter()
            .find_map(|(frame, e)| {
                matches!(
                    e,
                    SceneEffect::MapRequested {
                        op: SceneOp::TakeMapTransition
                    }
                )
                .then_some(*frame)
            })
            .expect("DoMapTransitionData on the map's own table");
        assert_eq!(request, 14, "${event:02X}: after the fade out");
        let writes = chunk_writes(&log);
        assert_eq!(
            writes[0].1,
            vec![(1, 1, 0x3C), (1, 0, lintel)],
            "${event:02X} arrival"
        );
        let arrived = log
            .iter()
            .find_map(|(frame, e)| match e {
                SceneEffect::ActorArrived {
                    actor: ActorRef::PartyMember(0),
                    at,
                } => Some((*frame, *at)),
                _ => None,
            })
            .expect("the leader walks out");
        assert_eq!(arrived.1, AT_DOOR, "${event:02X}: one cell down");
        let shut: Vec<ChunkWrite> = closing
            .iter()
            .zip(0u32..)
            .map(|(&(above, under), row)| {
                (writes[1].0 + 4 * row, vec![(1, 0, above), (1, 1, under)])
            })
            .collect();
        assert_eq!(writes[1..], shut[..], "${event:02X} shuts behind the party");
        assert!(
            writes[1].0 >= arrived.0,
            "${event:02X}: shut after the walk"
        );
        let ids: Vec<u8> = sounds(&log).into_iter().map(|(_, id)| id).collect();
        assert_eq!(ids, vec![0xD7, 0xD8], "${event:02X}: step out, then shut");
    }
}

/// `Event_GaruberkTwEyeAction1` (`$06F8A4`, `ps4.asm:148951-149019`): the flag at
/// once, sixty frames of the eye, then Part2's other layout, both planes.
/// `Event_GaruberkTwEyeAction2` (`$06F996`, `ps4.asm:149020-149073`): 120
/// frames, then its flag, and no layout change.
#[test]
fn the_eyes_set_their_flags_and_the_first_replaces_the_layout() {
    let mut state = GameState::default();
    let log = play(0x39, &mut state);
    assert!(state.is_set(Flag::temp(0x14)));
    let replaced: Vec<(u32, u32, u32)> = log
        .iter()
        .filter_map(|(frame, e)| match e {
            SceneEffect::MapLayoutReplaced { fg, bg } => Some((*frame, *fg, *bg)),
            _ => None,
        })
        .collect();
    assert_eq!(replaced, vec![(60, 0x1C_8F7A, 0x1C_92BA)]);

    let mut state = GameState::default();
    let log = play(0x3A, &mut state);
    assert!(state.is_set(Flag::temp(0x16)));
    let flagged = log.iter().find_map(|(frame, e)| match e {
        SceneEffect::FlagChanged { .. } => Some(*frame),
        _ => None,
    });
    assert_eq!(flagged, Some(120), "the flag after the eye's 120 frames");
    assert!(
        !log.iter()
            .any(|(_, e)| matches!(e, SceneEffect::MapLayoutReplaced { .. }))
    );
}
