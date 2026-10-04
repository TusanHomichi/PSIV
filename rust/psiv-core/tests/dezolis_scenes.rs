//! Scene outcomes for the Dezolis route events added by the event census
//! (`docs/scenes/EVENT_COVERAGE.md`, S7).
//!
//! Each case runs the registered scene through `SceneRunner`, answering every
//! block the moment it appears, and holds it to the state and effects the
//! cartridge's routine produces (cited per case).

mod common;

use common::map_with;
use psiv_core::{
    ActorRef, Cell, CharId, DialogueId, Direction, EventIndex, Flag, GameState, PixelPos,
    SceneEffect, SceneInput, SceneOp, SceneRunner, ScriptedActor, StepFrames, TRIGGERS,
    TriggerContext, TriggerResult, scene_for,
};

fn run(event: u16, state: &mut GameState) -> Vec<SceneEffect> {
    let map = map_with(
        &["........", "........", "........", "........"],
        vec![],
        vec![],
    );
    let scene =
        scene_for(EventIndex(event)).unwrap_or_else(|| panic!("${event:04X} is not registered"));
    let cast = vec![
        ScriptedActor::new(ActorRef::PartyMember(0), Cell::new(2, 2), Direction::Down),
        ScriptedActor::new(
            ActorRef::Character(CharId(0)),
            Cell::new(2, 2),
            Direction::Down,
        ),
    ];
    let mut runner = SceneRunner::new(scene.ops, cast, StepFrames::new(8).unwrap());
    let mut log: Vec<SceneEffect> = Vec::new();
    for _ in 0..2_000 {
        let input = match log.last() {
            Some(SceneEffect::DialogueOpen(_) | SceneEffect::DialogueOpenFromNpc { .. }) => {
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

fn trigger_context(state: &GameState) -> TriggerContext<'_> {
    TriggerContext {
        state,
        at: PixelPos { x: 0x120, y: 0x2F0 },
        standing: None,
        previously_standing: None,
        layout_below: None,
        layout_above: None,
        rng: Box::leak(Box::default()),
    }
}

/// `Event_OutsideRajaTemple` (`$06FC76..$06FC93`, `docs/scenes/91_*`): load
/// `DialogueTree14` (`$1E99F0`), run entry 6 in the standard window, then
/// tail-call `EventFlags_Set` with `$80`.
#[test]
fn outside_raja_temple_runs_the_snowstorm_exchange_then_sets_the_flag() {
    let mut state = GameState::new();
    assert!(!state.is_set(Flag::event(0x80)));
    let log = run(0x43, &mut state);

    let tree = log.iter().position(|e| {
        matches!(
            e,
            SceneEffect::Presentation {
                op: SceneOp::SetDialogueTree {
                    rom_addr: 0x001E_99F0
                }
            }
        )
    });
    let dialogue = log
        .iter()
        .position(|e| matches!(e, SceneEffect::DialogueOpen(DialogueId(6))));
    assert!(tree.is_some(), "DialogueTree14 is loaded: {log:?}");
    assert!(dialogue.is_some(), "entry 6 runs: {log:?}");
    assert!(tree < dialogue, "the tree loads before its entry runs");
    assert_eq!(
        log.iter()
            .filter(|e| matches!(e, SceneEffect::DialogueOpen(_)))
            .count(),
        1,
        "one dialogue"
    );
    assert!(state.is_set(Flag::event(0x80)), "EventFlag_Snowstorm");
}

/// The flag is written after the dialogue closes, not before: a scene that
/// stalls on its dialogue leaves the trigger armed (the cartridge's own order).
#[test]
fn outside_raja_temple_sets_the_flag_only_after_the_dialogue() {
    let map = map_with(&["........", "........"], vec![], vec![]);
    let scene = scene_for(EventIndex(0x43)).unwrap();
    let mut state = GameState::new();
    let mut runner = SceneRunner::new(scene.ops, vec![], StepFrames::new(8).unwrap());
    let first = runner.tick(&map, &mut state, SceneInput::None);
    assert!(
        first
            .iter()
            .any(|e| matches!(e, SceneEffect::DialogueOpen(DialogueId(6))))
    );
    assert!(
        !state.is_set(Flag::event(0x80)),
        "still waiting on the text"
    );
    assert!(!runner.is_finished());
    runner.tick(&map, &mut state, SceneInput::DialogueClosed);
    assert!(state.is_set(Flag::event(0x80)));
    assert!(runner.is_finished());
}

/// `RunEvent_OutsideRajaTemple` (slot `$31`, `$056D3C..`) has no position test:
/// it fires from anywhere until `$80` is set, and the scene's flag is what
/// stops it (the loop H28 would otherwise have run forever).
#[test]
fn the_trigger_fires_anywhere_until_the_scene_sets_its_flag() {
    let mut state = GameState::new();
    assert_eq!(
        TRIGGERS[0x31].evaluate(&trigger_context(&state)),
        TriggerResult::Fire(EventIndex(0x43))
    );
    run(0x43, &mut state);
    assert_eq!(
        TRIGGERS[0x31].evaluate(&trigger_context(&state)),
        TriggerResult::NoEvent,
        "with Snowstorm set the trigger takes RunEvent_NoEvent"
    );
}

// ---------------------------------------------------------------------------
// Event_TylerGraveOpening ($0044, docs/scenes/92_*)
// ---------------------------------------------------------------------------

/// Tyler's grave objects, as the pack's `120_Tyler.json` places them (objects
/// 2..13, as cells): the two gravestone halves and ten invisible blocks.
const GRAVE_OBJECTS: [(u16, u16); 12] = [
    (18, 25), // 2: gravestone half, x = $120
    (22, 25), // 3: gravestone half
    (19, 25), // 4
    (20, 25), // 5
    (20, 25), // 6
    (21, 25), // 7
    (18, 24), // 8
    (19, 24), // 9
    (20, 24), // 10
    (20, 24), // 11
    (21, 24), // 12
    (22, 24), // 13
];

fn npc(id: u16, x: u16, y: u16) -> psiv_core::Npc {
    psiv_core::Npc::new(psiv_core::NpcId(id), Cell::new(x, y), Direction::Down)
}

/// A Tyler-shaped map: objects 0 and 1 are the town's fires, then the grave.
fn tyler() -> psiv_core::FieldMap {
    let mut npcs = vec![npc(0, 40, 37), npc(1, 46, 19)];
    npcs.extend(
        GRAVE_OBJECTS
            .iter()
            .enumerate()
            .map(|(i, &(x, y))| npc(i as u16 + 2, x, y)),
    );
    let row = ".".repeat(50);
    let rows = vec![row.as_str(); 40];
    map_with(&rows, vec![], npcs)
}

fn grave_runner(map: &psiv_core::FieldMap) -> SceneRunner {
    let scene = scene_for(EventIndex(0x44)).unwrap();
    let mut cast = vec![ScriptedActor::new(
        ActorRef::PartyMember(0),
        Cell::new(20, 26),
        Direction::Up,
    )];
    for (i, n) in map.npcs().iter().enumerate() {
        cast.push(ScriptedActor::new(ActorRef::Npc(i), n.cell, n.facing));
    }
    SceneRunner::new(scene.ops, cast, StepFrames::new(8).unwrap())
}

/// What `Runtime::translate_scene_effect` does with the effects that reach
/// the field map: the per-frame position reports and scripted facings.
fn apply_to_map(map: &mut psiv_core::FieldMap, effects: &[SceneEffect]) {
    for effect in effects {
        match effect {
            SceneEffect::Presentation {
                op:
                    SceneOp::Presentation {
                        op: psiv_core::PresentationOp::NpcPixelPosition { npc, x, y },
                    },
            } => map.set_npc_pixel_position(*npc, *x, *y).unwrap(),
            SceneEffect::ActorFaced {
                actor: ActorRef::Npc(index),
                facing,
            } => map.set_npc_facing(*index, *facing).unwrap(),
            _ => {}
        }
    }
}

/// Runs the grave scene on `map`, closing every dialogue at once; returns the
/// effects and the number of ticks it took.
fn run_grave(map: &mut psiv_core::FieldMap, state: &mut GameState) -> (Vec<SceneEffect>, usize) {
    let mut runner = grave_runner(map);
    let mut log: Vec<SceneEffect> = Vec::new();
    let mut ticks = 0;
    while !runner.is_finished() && ticks < 2_000 {
        let input = match log.last() {
            Some(SceneEffect::DialogueOpen(_) | SceneEffect::DialogueResume) => {
                SceneInput::DialogueClosed
            }
            _ => SceneInput::None,
        };
        let effects = runner.tick(map, state, input);
        apply_to_map(map, &effects);
        log.extend(effects);
        ticks += 1;
    }
    assert!(runner.is_finished(), "the grave scene did not finish");
    assert!(
        !log.iter().any(|e| matches!(e, SceneEffect::Faulted(_))),
        "{log:?}"
    );
    (log, ticks)
}

fn dialogues(log: &[SceneEffect]) -> Vec<u16> {
    log.iter()
        .filter_map(|e| match e {
            SceneEffect::DialogueOpen(DialogueId(id)) => Some(*id),
            _ => None,
        })
        .collect()
}

fn pixel_x(map: &psiv_core::FieldMap, index: usize) -> i32 {
    let n = map.npcs()[index];
    i32::from(n.cell.x) * 16 + i32::from(n.offset.x)
}

/// The four cells under the new stair chunk: BG chunk (10,12) is cells
/// (20..21, 24..25).
const STAIR_CELLS: [(u16, u16); 4] = [(20, 24), (21, 24), (20, 25), (21, 25)];

#[test]
fn the_grave_cells_are_blocked_before_the_slide_and_open_after_it() {
    let mut map = tyler();
    for (x, y) in STAIR_CELLS {
        assert!(
            map.npc_at(Cell::new(x, y)).is_some(),
            "({x},{y}) blocked before"
        );
    }
    let mut state = GameState::new();
    let (log, _) = run_grave(&mut map, &mut state);
    for (x, y) in STAIR_CELLS {
        assert!(
            map.npc_at(Cell::new(x, y)).is_none(),
            "({x},{y}) open after"
        );
    }
    assert!(log.iter().any(|e| matches!(
        e,
        SceneEffect::MapChunksWritten { chunks } if chunks == &vec![(10, 12, 0x47)]
    )));
}

#[test]
fn cells_open_part_way_through_the_slide_follow_the_drift_frame_by_frame() {
    // After 32 frames the left objects are 8 px left and the right ones 16 px
    // right: (20,24) and (20,25) hold objects still straddling their cells.
    let mut map = tyler();
    let mut state = GameState::new();
    let mut runner = grave_runner(&map);
    let mut input = SceneInput::None;
    let mut drift_frames = 0;
    let mut halfway = None;
    while !runner.is_finished() {
        let effects = runner.tick(&map, &mut state, input);
        input = SceneInput::None;
        if effects
            .iter()
            .any(|e| matches!(e, SceneEffect::DialogueOpen(_)))
        {
            input = SceneInput::DialogueClosed;
        }
        if effects.iter().any(|e| {
            matches!(
                e,
                SceneEffect::Presentation {
                    op: SceneOp::Presentation {
                        op: psiv_core::PresentationOp::NpcPixelPosition { npc: 2, .. }
                    }
                }
            )
        }) {
            drift_frames += 1;
        }
        apply_to_map(&mut map, &effects);
        if drift_frames == 32 && halfway.is_none() {
            halfway = Some((pixel_x(&map, 2), pixel_x(&map, 3), pixel_x(&map, 7)));
        }
        if effects
            .iter()
            .any(|e| matches!(e, SceneEffect::DialogueResume))
        {
            input = SceneInput::DialogueClosed;
        }
    }
    assert_eq!(halfway, Some((0x118, 0x170, 0x150 + 16)));
}

#[test]
fn each_object_ends_16_px_left_or_32_px_right_of_where_it_began() {
    let mut map = tyler();
    let before: Vec<i32> = (0..map.npcs().len()).map(|i| pixel_x(&map, i)).collect();
    let mut state = GameState::new();
    run_grave(&mut map, &mut state);
    for index in [2, 4, 5, 8, 9, 10] {
        assert_eq!(pixel_x(&map, index), before[index] - 16, "object {index}");
    }
    for index in [3, 6, 7, 11, 12, 13] {
        assert_eq!(pixel_x(&map, index), before[index] + 32, "object {index}");
    }
    for index in [0, 1] {
        assert_eq!(pixel_x(&map, index), before[index], "the fires stay put");
    }
    assert_eq!(map.npcs()[2].facing, Direction::Right);
    assert_eq!(map.npcs()[3].facing, Direction::Left);
}

#[test]
fn first_interaction_plays_both_halves_of_the_inscription_then_sets_the_flag() {
    let mut map = tyler();
    let mut state = GameState::new();
    let (log, _) = run_grave(&mut map, &mut state);
    assert_eq!(
        dialogues(&log),
        vec![0x1E],
        "entry $1E opens before the slide"
    );
    assert!(
        log.iter().any(|e| matches!(e, SceneEffect::DialogueResume)),
        "its second half resumes after the grave opens"
    );
    assert!(state.is_set(Flag::event(0x84)), "EventFlag_TylerGrave");
}

#[test]
fn with_the_flag_already_set_the_first_dialogue_and_the_resume_are_skipped() {
    // A reloaded Tyler: the stones are back at $120 but `$84` is set.
    let mut map = tyler();
    let mut state = GameState::new();
    state.write(Flag::event(0x84), true).unwrap();
    let (log, _) = run_grave(&mut map, &mut state);
    assert!(dialogues(&log).is_empty());
    assert!(!log.iter().any(|e| matches!(e, SceneEffect::DialogueResume)));
    assert_eq!(pixel_x(&map, 2), 0x110, "the slide still plays");
}

#[test]
fn once_the_stone_has_moved_interacting_again_reads_entry_22_and_changes_nothing() {
    let mut map = tyler();
    let mut state = GameState::new();
    run_grave(&mut map, &mut state);
    let after: Vec<i32> = (0..map.npcs().len()).map(|i| pixel_x(&map, i)).collect();
    let (log, _) = run_grave(&mut map, &mut state);
    assert_eq!(dialogues(&log), vec![0x22]);
    assert!(
        !log.iter()
            .any(|e| matches!(e, SceneEffect::MapChunksWritten { .. }))
    );
    let again: Vec<i32> = (0..map.npcs().len()).map(|i| pixel_x(&map, i)).collect();
    assert_eq!(after, again);
}

#[test]
fn the_slide_takes_64_frames_and_the_pause_after_it_60() {
    let mut map = tyler();
    let mut state = GameState::new();
    let (_, ticks) = run_grave(&mut map, &mut state);
    // Tick 1 branches and opens entry $1E; tick 2 closes it and starts the
    // drift; ticks 3..=66 are the 64 `DoMainUpdatesLoop` frames; ticks
    // 67..=126 the 60 of `DoMapUpdateLoop($3B)`, ending in the resume; tick
    // 127 closes it and sets the flag.
    assert_eq!(ticks, 127);
}

/// Whether `cell` holds an object after each of the 64 drift frames.
fn occupancy_by_frame(cell: Cell) -> Vec<bool> {
    let mut map = tyler();
    let mut state = GameState::new();
    let mut runner = grave_runner(&map);
    let mut input = SceneInput::None;
    let mut seen = Vec::new();
    while !runner.is_finished() {
        let effects = runner.tick(&map, &mut state, input);
        input = if effects.iter().any(|e| {
            matches!(
                e,
                SceneEffect::DialogueOpen(_) | SceneEffect::DialogueResume
            )
        }) {
            SceneInput::DialogueClosed
        } else {
            SceneInput::None
        };
        let reported = effects.iter().any(|e| {
            matches!(
                e,
                SceneEffect::Presentation {
                    op: SceneOp::Presentation {
                        op: psiv_core::PresentationOp::NpcPixelPosition { npc: 2, .. }
                    }
                }
            )
        });
        apply_to_map(&mut map, &effects);
        if reported {
            seen.push(map.npc_at(cell).is_some());
        }
    }
    seen
}

#[test]
fn a_stair_cell_frees_on_the_frame_its_last_object_leaves_it() {
    // (20,25) holds object 5 (leaves at once, -0.25 px a frame) and object 6
    // (+0.5 px a frame: $140 reaches $150, cell 21, on frame 32).
    let seen = occupancy_by_frame(Cell::new(20, 25));
    assert_eq!(seen.len(), 64);
    assert!(seen[..31].iter().all(|&held| held), "held through frame 31");
    assert!(seen[31..].iter().all(|&held| !held), "free from frame 32");
}
