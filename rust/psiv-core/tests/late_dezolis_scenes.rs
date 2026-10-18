//! Scene outcomes for the Esper Mansion's guard events and the Gumbious
//! Temple's theft (issue #83): each case runs the registered scene through
//! `SceneRunner`, answering every block at once, and holds it to what the
//! cartridge's routine does, cited per case (`docs/scenes/99_*` to `101_*`).

mod common;

use common::map_with;
use psiv_core::{
    ActorRef, Cell, DialogueId, Direction, EventIndex, Flag, GameState, PixelPos, SceneEffect,
    SceneInput, SceneRunner, ScriptedActor, StepFrames, scene_for,
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

/// The flags a log changed, in order.
fn flag_writes(log: &[SceneEffect]) -> Vec<(Flag, bool)> {
    log.iter()
        .filter_map(|e| match e {
            SceneEffect::FlagChanged { flag, value } => Some((*flag, *value)),
            _ => None,
        })
        .collect()
}

/// `$070A4E..$070A75` and `$070A76..$070A9D` (`docs/scenes/113_InnerSanctuaryGuard.md`):
/// `lea Field_Obj_Secondary, a4` / `move.w $6(Character_1), d0` / `bchg #2,
/// d0` / `jsr Event_UpdateObjFacing`, then `moveq #entry, d0` / `jsr
/// Event_GetAndRunDialogue` and the tail `jmp EventFlags_Set`. The literals
/// are the cartridge's: entries `$35` and `$36`, flags `$A2` and `$A3`.
#[test]
fn the_inner_sanctuary_old_man_turns_speaks_and_sets_his_flag() {
    for (event, entry, flag) in [(0x51_u16, 0x35_u16, 0xA2_u16), (0x52, 0x36, 0xA3)] {
        for (leader, wanted) in [
            (Direction::Up, Direction::Down),
            (Direction::Down, Direction::Up),
            (Direction::Left, Direction::Right),
            (Direction::Right, Direction::Left),
        ] {
            let mut state = GameState::new();
            let log = play(event, &mut state, leader);
            let faced = log
                .iter()
                .position(|e| {
                    *e == SceneEffect::ActorFaced {
                        actor: ActorRef::Npc(0),
                        facing: wanted,
                    }
                })
                .unwrap_or_else(|| panic!("${event:02X}: object 0 faces {wanted:?}: {log:?}"));
            let spoke = log
                .iter()
                .position(|e| matches!(e, SceneEffect::DialogueOpen(_)))
                .expect("a line");
            assert!(faced < spoke, "${event:02X} turns before it speaks");
            assert_eq!(dialogues(&log), vec![entry], "${event:02X}");
            assert_eq!(
                flag_writes(&log),
                vec![(Flag::event(flag), true)],
                "${event:02X} sets only its flag"
            );
            assert!(dialogue_writes(&log).is_empty());
        }
    }
}

/// `loc_71572`, the direction table `Event_AngerTowerAlys` indexes with an
/// object's `facing_dir` halved: the cartridge's eight bytes, typed here
/// from the image (`$071572..$071579`: `00 01 00 FF 01 00 FF 00`).
const ALYS_STEP_TABLE: [u8; 8] = [0x00, 0x01, 0x00, 0xFF, 0x01, 0x00, 0xFF, 0x00];

/// The cartridge's `facing_dir` words: 0 down, 4 up, 8 right, `$C` left.
fn facing_word(facing: Direction) -> usize {
    match facing {
        Direction::Down => 0x0,
        Direction::Up => 0x4,
        Direction::Right => 0x8,
        Direction::Left => 0xC,
    }
}

/// `move.b (a0,d2.w), d0` / `move.b $1(a0,d2.w), d1` / `ext.w`, with `d2` the
/// facing word `lsr.w #1`.
fn table_step(word: usize) -> (i32, i32) {
    let at = word >> 1;
    (
        i32::from(ALYS_STEP_TABLE[at] as i8),
        i32::from(ALYS_STEP_TABLE[at + 1] as i8),
    )
}

/// An actor's whole-pixel position (`curr_x_pos`, `curr_y_pos`).
fn pixel(actor: &ScriptedActor) -> (i32, i32) {
    let origin = PixelPos::from_cell(actor.cell);
    let (dx, dy) = actor.render_offset_16ths(StepFrames::new(8).unwrap());
    (origin.x + dx, origin.y + dy)
}

struct AlysRun {
    log: Vec<SceneEffect>,
    /// (leader, object 0) whole pixels after every tick, starting before the first.
    positions: Vec<((i32, i32), (i32, i32))>,
    ticks_to_retained_line: usize,
}

/// Runs `$62` with Chaz at `(5,5)` facing `leader` and the illusion (object 0)
/// on the cell he faces, answering the lines and the battle at once.
fn play_alys(state: &mut GameState, leader: Direction) -> AlysRun {
    let map = map_with(&[".........."; 10], vec![], vec![]);
    let scene = scene_for(EventIndex(0x62)).expect("$62 is registered");
    let (dx, dy) = leader.delta();
    let start = Cell::new(5, 5);
    let alys = Cell::new((5 + dx) as u16, (5 + dy) as u16);
    let cast = vec![
        ScriptedActor::new(ActorRef::PartyMember(0), start, leader),
        ScriptedActor::new(ActorRef::Npc(0), alys, Direction::Down),
    ];
    let mut runner = SceneRunner::new(scene.ops, cast, StepFrames::new(8).unwrap());
    let snapshot = |runner: &SceneRunner| {
        (
            pixel(runner.actor(ActorRef::PartyMember(0)).unwrap()),
            pixel(runner.actor(ActorRef::Npc(0)).unwrap()),
        )
    };
    let mut run = AlysRun {
        log: Vec::new(),
        positions: vec![snapshot(&runner)],
        ticks_to_retained_line: 0,
    };
    let mut first_closed = None;
    for tick in 0..2_000 {
        let input = match run.log.last() {
            Some(SceneEffect::DialogueOpen(_)) => {
                first_closed.get_or_insert(tick);
                SceneInput::DialogueClosed
            }
            Some(SceneEffect::BattleRequested { .. }) => SceneInput::BattleFinished {
                outcome: psiv_core::battle::Outcome::Victory,
            },
            _ => SceneInput::None,
        };
        let effects = runner.tick(&map, state, input);
        if effects.contains(&SceneEffect::DialogueOpen(DialogueId(8))) {
            run.ticks_to_retained_line = tick - first_closed.unwrap_or(tick);
        }
        run.log.extend(effects);
        if runner.is_finished() {
            break;
        }
        if run.ticks_to_retained_line == 0 {
            run.positions.push(snapshot(&runner));
        }
    }
    assert!(runner.is_finished(), "$62 did not finish");
    assert!(
        !run.log.iter().any(|e| matches!(e, SceneEffect::Faulted(_))),
        "$62 faulted: {:?}",
        run.log
    );
    run
}

/// The pixel path retail takes, from the table bytes alone: object 0 turns
/// to face the leader (`bchg #2`), steps eight frames along its facing, the
/// leader sixteen against his (`neg.w`), object 0 eight more.
fn retail_path(
    leader: Direction,
    start: ((i32, i32), (i32, i32)),
) -> Vec<((i32, i32), (i32, i32))> {
    let alys_step = table_step(facing_word(leader) ^ 4);
    let (lx, ly) = table_step(facing_word(leader));
    let leader_step = (-lx, -ly);
    let mut at = start;
    let mut path = vec![at];
    for (frames, alys_moves) in [(8, true), (16, false), (8, true)] {
        for _ in 0..frames {
            if alys_moves {
                at.1 = (at.1.0 + alys_step.0, at.1.1 + alys_step.1);
            } else {
                at.0 = (at.0.0 + leader_step.0, at.0.1 + leader_step.1);
            }
            path.push(at);
        }
    }
    path
}

/// `$0714A8..$071535`: the three `Event_StepObject` loops, frame by frame, for
/// every facing the leader can talk from. Each frame moves one object one
/// pixel; the illusion ends on Chaz's cell and Chaz one cell back.
#[test]
fn the_anger_tower_illusion_and_chaz_step_by_retails_table() {
    for leader in [
        Direction::Up,
        Direction::Down,
        Direction::Left,
        Direction::Right,
    ] {
        let mut state = GameState::new();
        let run = play_alys(&mut state, leader);
        let mut moved = run.positions.clone();
        moved.dedup();
        let expected = retail_path(leader, run.positions[0]);
        assert_eq!(moved, expected, "leader facing {leader:?}");
        let (lx, ly) = table_step(facing_word(leader));
        let start = run.positions[0];
        let end = *expected.last().unwrap();
        assert_eq!(
            end.1, start.0,
            "the illusion ends on Chaz's cell ({leader:?})"
        );
        assert_eq!(
            end.0,
            (start.0.0 - 16 * lx, start.0.1 - 16 * ly),
            "Chaz one cell back ({leader:?})"
        );
        // Every step frame moves something: 32 frames, no pause between loops.
        let changes = run.positions.windows(2).filter(|w| w[0] != w[1]).count();
        assert_eq!(changes, 32, "{leader:?}");
        assert_eq!(
            run.ticks_to_retained_line, 32,
            "the retained line opens on the tick after the 32 step frames ({leader:?})"
        );
        // The map object's per-frame positions reach the field map.
        let reported: Vec<(i32, i32)> = run
            .log
            .iter()
            .filter_map(|e| match e {
                SceneEffect::Presentation {
                    op:
                        psiv_core::SceneOp::Presentation {
                            op: psiv_core::PresentationOp::NpcPixelPosition { npc: 0, x, y },
                        },
                } => Some((*x, *y)),
                _ => None,
            })
            .collect();
        let alys_path: Vec<(i32, i32)> = expected.iter().map(|at| at.1).collect();
        let mut alys_moves = alys_path.clone();
        alys_moves.dedup();
        assert_eq!(reported, alys_moves[1..].to_vec(), "{leader:?}");
    }
}

/// The leader's sixteen frames, pixel by pixel: one pixel each frame against
/// his facing, never two and never a cell jump.
#[test]
fn chaz_backs_off_one_pixel_a_frame() {
    let mut state = GameState::new();
    let run = play_alys(&mut state, Direction::Up);
    let leader: Vec<(i32, i32)> = run.positions.iter().map(|at| at.0).collect();
    let mut path = leader.clone();
    path.dedup();
    let start = path[0];
    let wanted: Vec<(i32, i32)> = (0..=16).map(|i| (start.0, start.1 + i)).collect();
    assert_eq!(path, wanted);
}

/// `$07148A..$071570`: the lines, the flag, Chaz's placement and the battle,
/// in the cartridge's order.
#[test]
fn the_anger_tower_illusion_speaks_sets_e4_places_chaz_and_fights_battle_18() {
    let mut state = GameState::new();
    let run = play_alys(&mut state, Direction::Up);
    assert_eq!(dialogues(&run.log), vec![7, 8]);
    assert_eq!(flag_writes(&run.log), vec![(Flag::event(0xE4), true)]);
    let placed = run
        .log
        .iter()
        .position(|e| {
            *e == SceneEffect::ActorPlaced {
                actor: ActorRef::PartyMember(0),
                // `$1F0,$1F0`: column `$1F`, the standing row `$1F + 1`.
                at: Cell::new(0x1F, 0x20),
            }
        })
        .expect("Chaz is set at $1F0,$1F0");
    let faced = run
        .log
        .iter()
        .position(|e| {
            *e == SceneEffect::ActorFaced {
                actor: ActorRef::PartyMember(0),
                facing: Direction::Down,
            }
        })
        .expect("facing down");
    let battle = run
        .log
        .iter()
        .position(|e| matches!(e, SceneEffect::BattleRequested { index: 0x18 }))
        .expect("battle $18");
    let flag = run
        .log
        .iter()
        .position(|e| matches!(e, SceneEffect::FlagChanged { .. }))
        .unwrap();
    assert!(flag < placed && placed < faced && faced < battle);
    assert!(run.log.iter().any(|e| matches!(
        e,
        SceneEffect::Presentation {
            op: psiv_core::SceneOp::SetMapLoadFlags {
                set: 0x08,
                clear: 0
            }
        }
    )));
}

/// The facing table is the cartridge's: each facing word halved indexes the
/// unit step `Direction::delta` gives, so the runner's direction is the
/// table's for every facing.
#[test]
fn the_step_table_is_the_cartridges_unit_step_per_facing() {
    for facing in [
        Direction::Down,
        Direction::Up,
        Direction::Right,
        Direction::Left,
    ] {
        assert_eq!(
            table_step(facing_word(facing)),
            facing.delta(),
            "{facing:?}"
        );
    }
}

/// Runs `$99` from `state`, answering lines and map loads at once; returns
/// every effect and the ticks the scene took.
fn play_enter_anger_tower(state: &mut GameState) -> (Vec<SceneEffect>, usize) {
    let map = map_with(&["........"; 4], vec![], vec![]);
    let scene = scene_for(EventIndex(0x99)).expect("$99 is registered");
    let cast = vec![ScriptedActor::new(
        ActorRef::PartyMember(0),
        Cell::new(2, 2),
        Direction::Up,
    )];
    let mut runner = SceneRunner::new(scene.ops, cast.clone(), StepFrames::new(8).unwrap());
    let mut log: Vec<SceneEffect> = Vec::new();
    for tick in 0..2_000 {
        let input = match log.last() {
            Some(SceneEffect::DialogueOpen(_) | SceneEffect::DialogueResume) => {
                SceneInput::DialogueClosed
            }
            Some(SceneEffect::MapRequested { .. }) => {
                runner.recast(cast.clone());
                SceneInput::MapLoaded
            }
            _ => SceneInput::None,
        };
        log.extend(runner.tick(&map, state, input));
        if runner.is_finished() {
            assert!(
                !log.iter().any(|e| matches!(e, SceneEffect::Faulted(_))),
                "$99 faulted: {log:?}"
            );
            return (log, tick + 1);
        }
    }
    panic!("$99 did not finish");
}

fn presented(log: &[SceneEffect]) -> Vec<psiv_core::SceneOp> {
    log.iter()
        .filter_map(|e| match e {
            SceneEffect::Presentation { op } | SceneEffect::MapRequested { op } => Some(*op),
            _ => None,
        })
        .collect()
}

/// `$0737CC..$07381F`, Elsydeon (`$D9`) clear: dialogue 5, `Sound_StopMusic`,
/// a fade, Rykros (`$2`) from the Anger Tower (`$FC`) at `$44,$C2` facing 0,
/// the Rykros field music (`$90`), and no Anger Tower flag.
#[test]
fn without_elsydeon_the_barrier_sends_the_party_back_to_rykros() {
    use psiv_core::SceneOp;
    let mut state = GameState::new();
    let (log, _) = play_enter_anger_tower(&mut state);
    assert_eq!(dialogues(&log), vec![5]);
    assert_eq!(
        presented(&log),
        vec![
            SceneOp::PlaySound { id: 0xFB },
            SceneOp::FadeOut,
            SceneOp::LoadMap {
                map: 0x002,
                prev_map: 0xFC,
                start_x: 0x44,
                start_y: 0xC2,
                facing: Direction::Down,
                align: 0,
                clear_load_flags: 0x08,
            },
            SceneOp::PlaySound { id: 0x90 },
            SceneOp::FadeIn,
        ]
    );
    assert!(flag_writes(&log).is_empty());
}

/// `loc_73820` (`$073820..$07387F`), Elsydeon set: dialogue 6,
/// `SFXID_BarrierBroken` (`$E6`), forty frames of `Pal_IncreaseTone` with
/// `Pal_Init_Line_3` (`$296300`) held over `$FFFFFB40` (sixteen words), forty
/// frames of `loc_58666` over all 64 words, the resumed text and Anger Tower
/// `$E0`.
#[test]
fn with_elsydeon_the_barrier_breaks_and_sets_e0() {
    use psiv_core::{PaletteHold, PresentationOp, SceneOp};
    let mut state = GameState::new();
    state.set(Flag::event(0xD9)).unwrap();
    let (log, ticks) = play_enter_anger_tower(&mut state);
    // Tick 1 opens line 6; tick 2 closes it and starts the first loop, whose
    // `WaitFrames(40)` (the `moveq #$27` loop's forty `VInt_Prepare` frames)
    // holds the scene through tick 41; tick 42 starts the second loop, held
    // through tick 81; tick 82 resumes the text and tick 83 closes it, sets
    // `$E0` and ends: 3 handshake ticks and the cartridge's 80 frames.
    assert_eq!(ticks, 83);
    assert_eq!(dialogues(&log), vec![6]);
    assert_eq!(
        log.iter()
            .filter(|e| matches!(e, SceneEffect::DialogueResume))
            .count(),
        1
    );
    assert_eq!(
        presented(&log),
        vec![
            SceneOp::PlaySound { id: 0xE6 },
            SceneOp::Presentation {
                op: PresentationOp::PaletteIncreaseTone {
                    frames: 40,
                    hold: Some(PaletteHold {
                        source_rom_addr: 0x0029_6300,
                        destination_ram: 0xFFFF_FB40,
                        words: 16,
                    }),
                },
            },
            SceneOp::WaitFrames { frames: 40 },
            SceneOp::Presentation {
                op: PresentationOp::PaletteToneFlash {
                    brighten: false,
                    words: 64,
                    frames: 40,
                },
            },
            SceneOp::WaitFrames { frames: 40 },
        ]
    );
    assert_eq!(flag_writes(&log), vec![(Flag::event(0xE0), true)]);
}

/// `loc_6FC5C`, typed from the US image (`$06FC5C..$06FC75`): five rows of a
/// delay byte and four chunk ids, then `$FF`.
const SILENCE_DOOR_TABLE: [u8; 26] = [
    0x06, 0x54, 0x55, 0x5E, 0x5F, 0x06, 0x56, 0x57, 0x60, 0x61, 0x06, 0x58, 0x59, 0x62, 0x63, 0x06,
    0x5A, 0x5B, 0x64, 0x65, 0x06, 0x5C, 0x5D, 0x66, 0x67, 0xFF,
];

/// `$06FBEE..$06FC75`: `SFXID_DoorOpened` (`$E2`), then each table row's four
/// chunks into BG `(15,8)`, `(16,8)` (`a1`, `d2 = 8`) and `(15,9)`, `(16,9)`
/// (`a2`, `d2 = 9`), each followed by `delay + 1` frames of
/// `DMAPlane_B_VInt`. No flag, no dialogue.
#[test]
fn the_silence_temple_door_steps_open_by_retails_table() {
    use psiv_core::SceneOp;
    let mut state = GameState::new();
    let log = play(0x42, &mut state, Direction::Up);
    let mut wanted: Vec<SceneEffect> = vec![SceneEffect::Presentation {
        op: SceneOp::PlaySound { id: 0xE2 },
    }];
    for row in SILENCE_DOOR_TABLE[..25].chunks(5) {
        wanted.push(SceneEffect::MapChunksWritten {
            chunks: vec![
                (15, 8, u16::from(row[1])),
                (16, 8, u16::from(row[2])),
                (15, 9, u16::from(row[3])),
                (16, 9, u16::from(row[4])),
            ],
        });
        wanted.push(SceneEffect::Presentation {
            op: SceneOp::WaitFrames {
                frames: u16::from(row[0]) + 1,
            },
        });
    }
    let observed: Vec<SceneEffect> = log
        .iter()
        .filter(|e| {
            matches!(
                e,
                SceneEffect::MapChunksWritten { .. } | SceneEffect::Presentation { .. }
            )
        })
        .cloned()
        .collect();
    assert_eq!(observed, wanted);
    assert!(flag_writes(&log).is_empty());
    assert!(dialogues(&log).is_empty());
}
