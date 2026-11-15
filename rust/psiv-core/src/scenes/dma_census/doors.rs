//! The five door exceptions (#601), with each static call assigned to its
//! consuming phase. The table delay is a DBF counter, not a frame count.
//! No extra `DmaPlanes` follows a wait that already spends those frames.

use super::{DMA_HELPERS, Row, SceneOp, Why, jsr_target};
use crate::{
    ActorRef, Cell, Direction, GameState, SceneInput, SceneRunner, ScriptedActor, StepFrames,
};

pub(super) const fn row(scene: &'static str, calls: usize) -> Row {
    Row {
        scene,
        ops: 0,
        calls,
        why: Why::DoorFrames,
    }
}

struct Door {
    name: &'static str,
    table: usize,
    rows: usize,
    /// Arrival DMA, movement-loop DMA, table-loop DMA (or just the table).
    calls: &'static [usize],
    /// First table write in the scene.
    first: usize,
    silence: bool,
}

const DOORS: &[Door] = &[
    Door {
        name: "Event_GaruberkTwDoorOpening1",
        table: 0x6F4D4,
        rows: 9,
        calls: &[0x6F4C6],
        first: 1,
        silence: false,
    },
    Door {
        name: "Event_GaruberkTwDoorOpening2",
        table: 0x6F588,
        rows: 9,
        calls: &[0x6F57A],
        first: 1,
        silence: false,
    },
    Door {
        name: "Event_GaruberkTwDoorEntered1",
        table: 0x6F714,
        rows: 5,
        calls: &[0x6F612, 0x6F666, 0x6F706],
        first: 9,
        silence: false,
    },
    Door {
        name: "Event_GaruberkTwDoorEntered2",
        table: 0x6F894,
        rows: 5,
        calls: &[0x6F792, 0x6F7E6, 0x6F886],
        first: 9,
        silence: false,
    },
    Door {
        name: "Event_SilenceTmDoorOpening",
        table: 0x6FC5C,
        rows: 5,
        calls: &[0x6FC4E],
        first: 1,
        silence: true,
    },
];

pub(super) fn problems(rom: &[u8], name: &str, ops: &[SceneOp]) -> Vec<String> {
    let Some(door) = DOORS.iter().find(|d| d.name == name) else {
        return vec![format!("{name}: no door phase assignment")];
    };
    let mut errors = Vec::new();
    let leader = ActorRef::PartyMember(0);
    let stride = if door.silence { 5 } else { 3 };
    let table_call = *door.calls.last().unwrap();
    let helper = if door.silence {
        DMA_HELPERS[2]
    } else {
        DMA_HELPERS[0]
    };
    if jsr_target(rom, table_call) != Some(helper)
        || rom[table_call + 6..table_call + 8] != [0x51, 0xC8]
    {
        errors.push(format!("{name}: table has no DMA/DBF loop"));
    }
    let head = (table_call as i64
        + 8
        + i64::from(i16::from_be_bytes([
            rom[table_call + 8],
            rom[table_call + 9],
        ]))) as usize;
    // The back edge encloses a refresh and RunMapUpdates for Garuberk,
    // and just the DMA for Silence Temple. Neither loop calls VInt_Prepare
    // separately: its frame is already in the DMA helper.
    let targets: Vec<_> = (head..table_call)
        .step_by(2)
        .filter_map(|at| jsr_target(rom, at))
        .collect();
    if head > table_call
        || targets.contains(&0x05_4938) == door.silence
        || targets.contains(&super::VINT_PREPARE)
    {
        errors.push(format!("{name}: table loop update policy changed"));
    }
    if ops.len() != door.first + door.rows * 2 {
        errors.push(format!("{name}: table has extra or missing ops"));
    }
    for index in 0..door.rows {
        let at = door.table + index * stride;
        let frames = u16::from(rom[at]) + 1;
        let wait = if door.silence {
            SceneOp::WaitFrames { frames }
        } else {
            SceneOp::Wait { ticks: frames }
        };
        let op = door.first + index * 2;
        let writes = match ops.get(op) {
            Some(SceneOp::WriteMapChunks { chunks }) if door.silence => {
                *chunks
                    == [
                        (15, 8, u16::from(rom[at + 1])),
                        (16, 8, u16::from(rom[at + 2])),
                        (15, 9, u16::from(rom[at + 3])),
                        (16, 9, u16::from(rom[at + 4])),
                    ]
            }
            Some(SceneOp::WriteActorMapChunks { actor, chunks }) if !door.silence => {
                *actor == leader
                    && *chunks
                        == [
                            (0, -32, u16::from(rom[at + 1])),
                            (0, 0, u16::from(rom[at + 2])),
                        ]
            }
            _ => false,
        };
        if !writes || ops.get(op + 1) != Some(&wait) {
            errors.push(format!(
                "{name}: row {} must write before {frames} frames (map updates={})",
                index + 1,
                !door.silence
            ));
        }
    }
    if rom[door.table + door.rows * stride] != 0xFF {
        errors.push(format!("{name}: table has no terminator"));
    }
    if door.calls.len() == 3 {
        // Arrival: the existing wait spends the DMA frame, but the CPU
        // refresh boundary and its RunMapUpdates are owned by #546. Do not
        // add another DMA wait here. Movement spends one B-plane DMA per
        // object-update iteration, including arrival (SceneRunner's actor
        // block / completes_map_update_loop; runtime field_tick).
        let above = if name.ends_with('1') { 0x3E } else { 0x34 };
        let arrival_write = matches!(ops.get(2), Some(SceneOp::WriteActorMapChunks { actor, chunks })
            if *actor == leader && *chunks == [(0, 0, 0x3C), (0, -16, above)]);
        if jsr_target(rom, door.calls[0]) != Some(DMA_HELPERS[0])
            || !arrival_write
            || ops.get(3) != Some(&SceneOp::WaitFrames { frames: 1 })
            || ops.get(4) != Some(&SceneOp::FadeIn)
        {
            errors.push(format!(
                "{name}: arrival frame/refresh assignment (#546) changed"
            ));
        }
        if jsr_target(rom, door.calls[1]) != Some(DMA_HELPERS[2])
            || ops.get(6)
                != Some(&SceneOp::MoveActorOffset {
                    actor: leader,
                    dx: 0,
                    dy: 16,
                    wait: true,
                })
            || ops.get(7) != Some(&SceneOp::OverlapCharacters)
        {
            errors.push(format!(
                "{name}: movement DMA must be spent by MoveActorOffset before overlap"
            ));
        }
    }
    errors
}

/// The arrival wait already spends one frame. Its missing refresh/update
/// semantics remain #546's work. The step-out's B-plane DMA repeats once
/// per movement tick; completing the actor block still owns map updates.
#[test]
fn garuberk_arrival_and_step_out_have_existing_frame_consumers() {
    let map = crate::FieldMap::new(
        crate::MapId(0),
        crate::CollisionGrid::filled(8, 8, 0).unwrap(),
        vec![],
        vec![],
    )
    .unwrap();
    for door in DOORS.iter().filter(|d| d.calls.len() == 3) {
        let ops = super::SCENES
            .iter()
            .find(|s| s.name == door.name)
            .unwrap()
            .ops;
        let cast = || {
            vec![ScriptedActor::new(
                ActorRef::PartyMember(0),
                Cell::new(2, 2),
                Direction::Down,
            )]
        };
        let mut state = GameState::new();
        let mut arrival = SceneRunner::new(&ops[2..4], cast(), StepFrames::default());
        let effects = arrival.tick(&map, &mut state, SceneInput::None);
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, crate::SceneEffect::MapChunksWritten { .. }))
        );
        assert!(!arrival.is_finished());
        // This is the current refresh-owned gap, not retail update parity.
        assert!(!arrival.runs_map_updates(), "#546 owns the arrival update");
        arrival.tick(&map, &mut state, SceneInput::None);
        assert!(arrival.is_finished(), "one DMA frame, no extra wait");

        let mut walk = SceneRunner::new(&ops[6..8], cast(), StepFrames::default());
        walk.tick(&map, &mut state, SceneInput::None);
        assert!(walk.runs_map_updates());
        for frame in 1..=8 {
            let completes = walk.completes_map_update_loop();
            walk.tick(&map, &mut state, SceneInput::None);
            assert!(completes || walk.runs_map_updates());
            assert_eq!(walk.is_finished(), frame == 8);
        }
        assert_eq!(
            walk.actor(ActorRef::PartyMember(0)).unwrap().cell,
            Cell::new(2, 3)
        );
    }
}

/// Execute the actual table ops: each write is immediate, its delay owns
/// exactly delay+1 frames, and only Garuberk runs map updates on those frames.
#[test]
fn door_table_frames_and_update_policy_match_the_us_image() {
    let Some(rom) = super::rom() else { return };
    for door in DOORS {
        let ops = super::SCENES
            .iter()
            .find(|s| s.name == door.name)
            .unwrap()
            .ops;
        assert!(problems(rom, door.name, ops).is_empty());
        let mut runner = SceneRunner::new(
            &ops[door.first..],
            vec![ScriptedActor::new(
                ActorRef::PartyMember(0),
                Cell::new(4, 4),
                Direction::Down,
            )],
            StepFrames::default(),
        );
        let map = crate::FieldMap::new(
            crate::MapId(0),
            crate::CollisionGrid::filled(8, 8, 0).unwrap(),
            vec![],
            vec![],
        )
        .unwrap();
        let mut state = GameState::new();
        let stride = if door.silence { 5 } else { 3 };
        for index in 0..door.rows {
            let frames = usize::from(rom[door.table + index * stride]) + 1;
            for frame in 0..frames {
                let effects = runner.tick(&map, &mut state, SceneInput::None);
                assert_eq!(
                    effects
                        .iter()
                        .filter(|e| matches!(e, crate::SceneEffect::MapChunksWritten { .. }))
                        .count(),
                    usize::from(frame == 0),
                    "{} row {index} frame {frame}: {effects:?}",
                    door.name
                );
                assert_eq!(
                    runner.runs_map_updates(),
                    !door.silence,
                    "{} row {index} frame {frame}",
                    door.name
                );
                assert!(!runner.is_finished());
            }
        }
        runner.tick(&map, &mut state, SceneInput::None);
        assert!(runner.is_finished());
    }
}

#[test]
fn door_census_rejects_missing_doubled_and_wrong_policy_frames() {
    let Some(rom) = super::rom() else { return };
    for door in DOORS {
        let ops = super::SCENES
            .iter()
            .find(|s| s.name == door.name)
            .unwrap()
            .ops;
        if door.calls.len() == 3 {
            for frames in [0, 2] {
                let mut changed = ops.to_vec();
                changed[3] = SceneOp::WaitFrames { frames };
                assert_eq!(problems(rom, door.name, &changed).len(), 1);
            }
            let mut changed = ops.to_vec();
            changed[6] = SceneOp::MoveActorOffset {
                actor: ActorRef::PartyMember(0),
                dx: 0,
                dy: 16,
                wait: false,
            };
            assert_eq!(problems(rom, door.name, &changed).len(), 1);
        }
        for index in 0..door.rows {
            let at = door.first + index * 2 + 1;
            for change in [-1_i16, 1] {
                let mut changed = ops.to_vec();
                changed[at] = match ops[at] {
                    SceneOp::Wait { ticks } => SceneOp::Wait {
                        ticks: (ticks as i16 + change) as u16,
                    },
                    SceneOp::WaitFrames { frames } => SceneOp::WaitFrames {
                        frames: (frames as i16 + change) as u16,
                    },
                    _ => unreachable!(),
                };
                assert_eq!(problems(rom, door.name, &changed).len(), 1);
            }
            let mut changed = ops.to_vec();
            changed[at] = match ops[at] {
                SceneOp::Wait { ticks } => SceneOp::WaitFrames { frames: ticks },
                SceneOp::WaitFrames { frames } => SceneOp::Wait { ticks: frames },
                _ => unreachable!(),
            };
            assert_eq!(problems(rom, door.name, &changed).len(), 1);
        }
    }
}

/// Reject literal copies of the Garuberk chunk-pair tables in the runtime
/// fixture. Read the values from the private local image, including in the
/// negative controls, so this guard cannot itself publish a replacement copy.
fn copied_pair_table(source: &str, pairs: &[u16]) -> bool {
    let numbers: Vec<u16> = source
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .filter_map(|word| {
            let word = word.replace('_', "");
            match word.strip_prefix("0x") {
                Some(hex) => u16::from_str_radix(hex, 16).ok(),
                None => word.parse().ok(),
            }
        })
        .collect();
    numbers.windows(pairs.len()).any(|window| window == pairs)
}

#[test]
fn runtime_door_fixture_keeps_retail_pair_tables_in_the_local_image() {
    let Some(rom) = super::rom() else { return };
    let source = include_str!("../../../../psiv-runtime/src/scene_garuberk_tests.rs");
    for door in DOORS.iter().filter(|door| !door.silence) {
        let pairs: Vec<_> = (0..door.rows)
            .flat_map(|index| {
                let at = door.table + 3 * index;
                [u16::from(rom[at + 1]), u16::from(rom[at + 2])]
            })
            .collect();
        assert!(
            !copied_pair_table(source, &pairs),
            "{}: runtime fixture contains a private retail table",
            door.name
        );
        // Both common Rust spellings of the original fixture must be caught.
        for hex in [false, true] {
            let rows = pairs
                .as_chunks::<2>()
                .0
                .iter()
                .map(|row| {
                    if hex {
                        format!("(0x{:X}, 0x{:X}),", row[0], row[1])
                    } else {
                        format!("({}, {}),", row[0], row[1])
                    }
                })
                .collect::<String>();
            assert!(copied_pair_table(&format!("let rows = [{rows}];"), &pairs));
        }
    }
}
