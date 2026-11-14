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
//! mechanic, or nothing, which #599 classifies.
//!
//! The image check skips without the local US image.

use super::documented_ranges::documented_ranges;
use super::{INSIDE_SPACESHIP_ROUTE, SCENES, dialogue_callers_image::rom};
use crate::SceneOp;

/// `DMAPlanes_VInt`, `DMAPlane_A_VInt`, `DMAPlane_B_VInt`.
const DMA_HELPERS: [u32; 3] = [0x04_1678, 0x04_1682, 0x04_16A6];

/// Why a scene's ops and calls differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Why {
    /// A call inside a retail loop, which the ops unroll: the loop's own
    /// address.
    Unrolled(u32),
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
    open("Event_VahFortMovingPlatform1", 0, 2),
    open("Event_VahFortMovingPlatform2", 0, 2),
    open("Event_WpnPlntMovingPlatform1", 0, 2),
    open("Event_WpnPlntMovingPlatform2", 0, 2),
    open("Event_WpnPlntMovingPlatform3", 0, 2),
    open("Event_WpnPlntMovingPlatform4", 0, 2),
    open("Event_ConveyorBeltDown", 0, 1),
    open("Event_ConveyorBeltUp", 0, 1),
    open("Event_ConveyorBeltRight", 0, 1),
    open("Event_ConveyorBeltLeft", 0, 1),
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
    open("Cutscene_Ending", 85, 95),
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
            Why::Unclassified(_) if row.ops > row.calls => problems.push(format!(
                "{}: {} DmaPlanes for {} calls spend frames retail does not",
                row.scene, row.ops, row.calls
            )),
            Why::Unclassified(_) => {}
        }
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
        ["Cutscene_Ending: 85 DmaPlanes for 95 helper calls, with no row"]
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
