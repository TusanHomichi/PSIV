//! Which retail frame each [`SceneOp::DmaPlanes`] spends (#595).
//!
//! `DMAPlanes_VInt` (`$041678`) and its one-plane forms `DMAPlane_A_VInt`
//! (`$041682`) and `DMAPlane_B_VInt` (`$0416A6`) queue the plane DMA and end
//! in `VInt_Prepare`: one frame, no map or object update. The runner spends
//! that frame on every `DmaPlanes`, so each op must stand for one such call.
//! In every registered scene the ops (the ship's route excluded, as
//! [`super::wait_census`] does) equal the calls in the scene's documented
//! bytes, or the difference is a [`ROWS`] entry saying why. More ops than
//! calls would add frames retail does not spend, unless a call runs in a
//! loop the ops unroll; fewer leave a call's frame to another op, a
//! mechanic, or nothing, which #599 classifies. A call that is the only
//! frame of a mechanic's loop is that mechanic's tick, once per pass, and its
//! row says which mechanic and loop.
//!
//! The image check skips without the local US image.

use super::documented_ranges::documented_ranges;
use super::{INSIDE_SPACESHIP_ROUTE, SCENES, dialogue_callers_image::rom};
use crate::{RideOp, SceneOp};

/// `DMAPlanes_VInt`, `DMAPlane_A_VInt`, `DMAPlane_B_VInt`.
const DMA_HELPERS: [u32; 3] = [0x04_1678, 0x04_1682, 0x04_16A6];

/// `VInt_Prepare`: a mechanic's loop that calls it spends a frame the helper
/// call does not account for.
const VINT_PREPARE: u32 = 0x04_204C;

/// A runtime mechanic whose per-frame tick is a retail loop's helper call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mechanic {
    /// [`RideOp::Conveyor`]: one tick per pixel the leader is carried, each
    /// the carry loop's `DMAPlane_A_VInt` (`scene_runner/mechanics.rs`).
    ConveyorRide,
}

impl Mechanic {
    fn runs(self, op: &SceneOp) -> bool {
        match self {
            Self::ConveyorRide => matches!(op, SceneOp::Ride(RideOp::Conveyor { .. })),
        }
    }
}

/// Why a scene's ops and calls differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Why {
    /// A call inside a retail loop, which the ops unroll: the loop's own
    /// address.
    Unrolled(u32),
    /// Calls in a loop whose only frame they are, which a mechanic's tick
    /// spends once per pass: the mechanic and the loop's head.
    Mechanic(Mechanic, u32),
    /// Calls whose frames another op or a runtime mechanic may spend, or
    /// nothing does: the issue that classifies them.
    Unclassified(u32),
}

/// One scene whose `DmaPlanes` ops and helper calls differ.
#[derive(Debug, Clone, Copy)]
struct Row {
    scene: &'static str,
    ops: usize,
    calls: usize,
    why: Why,
}

const fn open(scene: &'static str, ops: usize, calls: usize) -> Row {
    Row {
        scene,
        ops,
        calls,
        why: Why::Unclassified(599),
    }
}

/// A conveyor belt: its one call ends each pass of the carry loop at `head`.
const fn belt(scene: &'static str, head: u32) -> Row {
    Row {
        scene,
        ops: 0,
        calls: 1,
        why: Why::Mechanic(Mechanic::ConveyorRide, head),
    }
}

const ROWS: &[Row] = &[
    // `loc_74302`: `Panel_Destroy` and `DMAPlanes_VInt` once per panel
    // left (`Panel_Num`, two at that point), transcribed as two pairs.
    Row {
        scene: "Cutscene_Alshline",
        ops: 14,
        calls: 13,
        why: Why::Unrolled(0x07_4302),
    },
    open("Event_GameStart", 0, 4),
    open("Event_RuneFlaeli", 0, 1),
    open("Cutscene_AlysWounded", 1, 3),
    open("Event_MachineCenterAppearing", 0, 1),
    open("Cutscene_PsycoWand", 18, 27),
    open("Cutscene_ZioDefeated", 5, 9),
    open("Cutscene_SpaceshipSabotage", 4, 9),
    open("Cutscene_CrashLaanding", 4, 7),
    open("Event_TylerGraveOpening", 0, 1),
    // The carry loops: `Field_UpdateObjects` moves the leader one pixel
    // (`FieldObj_Step_Offset` 0), then the camera, then `DMAPlane_A_VInt`
    // (`$06CF48`, `$06D090`, `$06D1D8`, `$06D320`), then the chunk test
    // branches back. Each pass is one carry tick, repeated per pixel.
    belt("Event_ConveyorBeltDown", 0x06_CEB6),
    belt("Event_ConveyorBeltUp", 0x06_CFFE),
    belt("Event_ConveyorBeltRight", 0x06_D146),
    belt("Event_ConveyorBeltLeft", 0x06_D28E),
    open("Event_GaruberkTwDoorOpening1", 0, 1),
    open("Event_GaruberkTwDoorOpening2", 0, 1),
    open("Event_GaruberkTwDoorEntered1", 0, 3),
    open("Event_GaruberkTwDoorEntered2", 0, 3),
    open("Event_JuzaDefeated", 0, 1),
    // `$0705A2`, after `RefreshPlaneFG`/`BG`: the stretch `ReloadMapChunks`
    // stands for.
    open("Event_MeetingLeRoof", 1, 2),
    open("Event_EclipseTorchUsed", 2, 3),
    open("Cutscene_Elsydeon", 19, 23),
    open("Event_SilenceTmDoorOpening", 0, 1),
    // The ending intro now records its DMA after dialogue 9 explicitly.
    open("Cutscene_Ending", 86, 95),
    open("Event_HuntersGuild", 0, 3),
    open("Event_WreckageEngine", 0, 1),
    open("Event_GettingOffVehicle", 0, 1),
];

/// The `jsr (abs).l` target at `at`. The helpers sit at `$041xxx`, out of a
/// `bsr.w`'s reach from every scene body.
fn jsr_target(rom: &[u8], at: usize) -> Option<u32> {
    (rom.get(at..at + 2)? == [0x4E, 0xB9])
        .then(|| u32::from_be_bytes(rom[at + 2..at + 6].try_into().unwrap()))
}

/// Each registered scene's `DmaPlanes` ops, and its calls to the helpers in
/// its documented bytes (overlapping ranges merged, so a range the scene's
/// document and the registry both list counts once).
fn counts(rom: &[u8], scenes: &[(&str, &[SceneOp])]) -> Vec<(String, usize, usize)> {
    let ranges = documented_ranges();
    let mut out = Vec::new();
    for &(name, ops) in scenes {
        let ops = ops
            .strip_suffix(INSIDE_SPACESHIP_ROUTE.as_slice())
            .unwrap_or(ops);
        let dma = ops
            .iter()
            .filter(|op| matches!(op, SceneOp::DmaPlanes))
            .count();
        let mut spans = ranges.get(name).cloned().unwrap_or_default();
        spans.sort_unstable();
        let mut merged: Vec<(usize, usize)> = Vec::new();
        for (start, end) in spans {
            match merged.last_mut() {
                Some(last) if start <= last.1 => last.1 = last.1.max(end),
                _ => merged.push((start, end)),
            }
        }
        let calls = merged
            .iter()
            .flat_map(|&(start, end)| (start..end).step_by(2))
            .filter(|&at| jsr_target(rom, at).is_some_and(|t| DMA_HELPERS.contains(&t)))
            .count();
        out.push((name.to_owned(), dma, calls));
    }
    out
}

/// Every scene whose ops and calls differ without a row saying so, and
/// every row that no longer matches its scene.
fn problems(rom: &[u8], counted: &[(String, usize, usize)], rows: &[Row]) -> Vec<String> {
    let mut problems = Vec::new();
    for (name, ops, calls) in counted {
        let row = rows.iter().find(|row| row.scene == name);
        match row {
            None if ops != calls => problems.push(format!(
                "{name}: {ops} DmaPlanes for {calls} helper calls, with no row"
            )),
            Some(row) if (row.ops, row.calls) != (*ops, *calls) => problems.push(format!(
                "{name}: the row says {} for {}, the scene has {ops} for {calls}",
                row.ops, row.calls
            )),
            _ => {}
        }
    }
    for row in rows {
        if !counted.iter().any(|(name, ..)| name == row.scene) {
            problems.push(format!("{}: a row for no registered scene", row.scene));
        }
        if row.ops == row.calls {
            problems.push(format!("{}: a row for counts that agree", row.scene));
        }
        match row.why {
            // Only a loop accounts for more ops than calls: its body must
            // call a helper and branch back to it.
            Why::Unrolled(at) => {
                let at = at as usize;
                let body = (at..at + 0x20)
                    .step_by(2)
                    .find(|&p| jsr_target(rom, p).is_some_and(|t| DMA_HELPERS.contains(&t)));
                let back = body.and_then(|p| {
                    let dbf = p + 6;
                    (rom[dbf] == 0x51 && rom[dbf + 1] & 0xF8 == 0xC8).then(|| {
                        let disp = i16::from_be_bytes([rom[dbf + 2], rom[dbf + 3]]);
                        (dbf as i64 + 2 + i64::from(disp)) as usize
                    })
                });
                if back != Some(at) {
                    problems.push(format!(
                        "{}: ${at:06X} is no loop around a helper call",
                        row.scene
                    ));
                }
            }
            Why::Mechanic(mechanic, head) => {
                problems.extend(mechanic_problems(rom, row, mechanic, head));
            }
            Why::Unclassified(_) if row.ops > row.calls => problems.push(format!(
                "{}: {} DmaPlanes for {} calls spend frames retail does not",
                row.scene, row.ops, row.calls
            )),
            Why::Unclassified(_) => {}
        }
    }
    problems
}

/// A [`Why::Mechanic`] row holds when the scene runs the mechanic, and the
/// loop at `head` lies in the scene's documented bytes, branches back to
/// `head` with a `bra.w`, holds every call the ops leave out, and waits no
/// other frame.
fn mechanic_problems(rom: &[u8], row: &Row, mechanic: Mechanic, head: u32) -> Vec<String> {
    let mut problems = Vec::new();
    let ops = SCENES
        .iter()
        .find(|scene| scene.name == row.scene)
        .map_or(&[][..], |scene| scene.ops);
    if !ops.iter().any(|op| mechanic.runs(op)) {
        problems.push(format!("{}: runs no {mechanic:?}", row.scene));
    }
    let at = head as usize;
    let documented = documented_ranges()
        .get(row.scene)
        .is_some_and(|spans| spans.iter().any(|&(start, end)| (start..end).contains(&at)));
    let back = (at..at + 0x100).step_by(2).find(|&p| {
        rom[p..p + 2] == [0x60, 0x00]
            && (p as i64 + 2 + i64::from(i16::from_be_bytes([rom[p + 2], rom[p + 3]]))) == at as i64
    });
    let Some(back) = back.filter(|_| documented) else {
        problems.push(format!(
            "{}: ${head:06X} is no loop of the scene's",
            row.scene
        ));
        return problems;
    };
    let targets: Vec<u32> = (at..back)
        .step_by(2)
        .filter_map(|p| jsr_target(rom, p))
        .collect();
    let calls = targets.iter().filter(|t| DMA_HELPERS.contains(t)).count();
    if calls != row.calls - row.ops {
        problems.push(format!(
            "{}: the loop at ${head:06X} makes {calls} of the {} uncovered calls",
            row.scene,
            row.calls - row.ops
        ));
    }
    if targets.contains(&VINT_PREPARE) {
        problems.push(format!(
            "{}: the loop at ${head:06X} waits another frame",
            row.scene
        ));
    }
    problems
}

fn registered() -> Vec<(&'static str, &'static [SceneOp])> {
    SCENES.iter().map(|scene| (scene.name, scene.ops)).collect()
}

#[test]
fn every_dma_planes_op_is_a_retail_vint_dma_call() {
    let Some(rom) = rom() else { return };
    let counted = counts(rom, &registered());
    assert_eq!(problems(rom, &counted, ROWS), Vec::<String>::new());
}

/// Negative controls: a `DmaPlanes` added to a scene with no row, a row
/// dropped, a row's count off by one, an unclassified row with more ops
/// than calls, and a loop address that is no loop.
#[test]
fn the_dma_census_rejects_an_extra_op_and_a_stale_row() {
    let Some(rom) = rom() else { return };
    let mut counted = counts(rom, &registered());
    let again = "Cutscene_LeRoofAgain";
    let at = counted.iter().position(|(name, ..)| name == again).unwrap();
    assert_eq!((counted[at].1, counted[at].2), (3, 3));
    counted[at].1 += 1;
    assert_eq!(
        problems(rom, &counted, ROWS),
        [format!(
            "{again}: 4 DmaPlanes for 3 helper calls, with no row"
        )]
    );

    let counted = counts(rom, &registered());
    let dropped: Vec<Row> = ROWS
        .iter()
        .copied()
        .filter(|row| row.scene != "Cutscene_Ending")
        .collect();
    assert_eq!(
        problems(rom, &counted, &dropped),
        ["Cutscene_Ending: 86 DmaPlanes for 95 helper calls, with no row"]
    );

    let mut stale = ROWS.to_vec();
    stale[1].calls += 1;
    assert_eq!(
        problems(rom, &counted, &stale),
        ["Event_GameStart: the row says 0 for 5, the scene has 0 for 4"]
    );

    let mut extra = ROWS.to_vec();
    extra[0].why = Why::Unclassified(599);
    assert_eq!(
        problems(rom, &counted, &extra),
        ["Cutscene_Alshline: 14 DmaPlanes for 13 calls spend frames retail does not"]
    );

    let mut no_loop = ROWS.to_vec();
    no_loop[0].why = Why::Unrolled(0x07_4308);
    assert_eq!(
        problems(rom, &counted, &no_loop),
        ["Cutscene_Alshline: $074308 is no loop around a helper call"]
    );
}

/// Negative controls for the platforms and belts (#600): a platform scene
/// with one `DmaPlanes` omitted or doubled, a belt row on another belt's
/// loop, a carry row on a scene without the carry, a loop that holds fewer
/// calls than the row leaves out, and a loop that waits another frame.
#[test]
fn the_dma_census_rejects_a_lost_or_doubled_platform_frame_and_a_wrong_loop() {
    let Some(rom) = rom() else { return };
    let platform = "Event_WpnPlntMovingPlatform2";
    let ops = SCENES.iter().find(|s| s.name == platform).unwrap().ops;
    let first = ops
        .iter()
        .position(|op| matches!(op, SceneOp::DmaPlanes))
        .unwrap();
    let mut omitted = ops.to_vec();
    omitted.remove(first);
    let mut doubled = ops.to_vec();
    doubled.insert(first, SceneOp::DmaPlanes);
    for (changed, count) in [(&omitted, 1), (&doubled, 3)] {
        let scenes: Vec<(&str, &[SceneOp])> = registered()
            .into_iter()
            .map(|(name, ops)| {
                if name == platform {
                    (name, changed.as_slice())
                } else {
                    (name, ops)
                }
            })
            .collect();
        assert_eq!(
            problems(rom, &counts(rom, &scenes), ROWS),
            [format!(
                "{platform}: {count} DmaPlanes for 2 helper calls, with no row"
            )]
        );
    }

    let counted = counts(rom, &registered());
    let down = ROWS
        .iter()
        .position(|row| row.scene == "Event_ConveyorBeltDown")
        .unwrap();
    let mut wrong_loop = ROWS.to_vec();
    wrong_loop[down].why = Why::Mechanic(Mechanic::ConveyorRide, 0x06_CFFE);
    assert_eq!(
        problems(rom, &counted, &wrong_loop),
        ["Event_ConveyorBeltDown: $06CFFE is no loop of the scene's"]
    );

    let start = ROWS
        .iter()
        .position(|row| row.scene == "Event_GameStart")
        .unwrap();
    let mut no_carry = ROWS.to_vec();
    no_carry[start].why = Why::Mechanic(Mechanic::ConveyorRide, 0x06_CEB6);
    assert_eq!(
        problems(rom, &counted, &no_carry),
        [
            "Event_GameStart: runs no ConveyorRide",
            "Event_GameStart: $06CEB6 is no loop of the scene's",
        ]
    );

    let mut short = ROWS.to_vec();
    short[down].calls = 2;
    let mut recounted = counted.clone();
    let at = recounted
        .iter()
        .position(|(name, ..)| name == "Event_ConveyorBeltDown")
        .unwrap();
    recounted[at].2 = 2;
    assert_eq!(
        problems(rom, &recounted, &short),
        ["Event_ConveyorBeltDown: the loop at $06CEB6 makes 1 of the 2 uncovered calls"]
    );

    // `Field_UpdateObjects`, the loop's first call, made a `VInt_Prepare`.
    let mut waits = rom.to_vec();
    let call = (0x06_CEB6..0x06_CF66)
        .step_by(2)
        .find(|&p| jsr_target(rom, p).is_some_and(|t| !DMA_HELPERS.contains(&t)))
        .unwrap();
    waits[call + 2..call + 6].copy_from_slice(&VINT_PREPARE.to_be_bytes());
    assert_eq!(
        problems(&waits, &counted, ROWS),
        ["Event_ConveyorBeltDown: the loop at $06CEB6 waits another frame"]
    );
}
