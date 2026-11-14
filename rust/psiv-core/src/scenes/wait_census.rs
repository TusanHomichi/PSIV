//! Which retail loop each scene's blocking wait transcribes (#470).
//!
//! Retail scenes block in two library loops that differ by one call:
//! `DoMapUpdateLoop` (`$05A71E`) runs `RunMapUpdates` and `VInt_Prepare`;
//! `DoMainUpdatesLoop` (`$05A73C`) runs `Field_UpdateObjects` first. Others
//! block in a loop of their own. Every [`SceneOp::Wait`] of a censused scene
//! is claimed by a row here naming the retail call or loop it stands for, and
//! every library-loop call inside the scene's documented bytes has a row, so
//! a wait whose retail loop is missing, folded into another op or only
//! approximated is on record, with the issue that will fix it.
//!
//! Two shared helpers block for a fixed number of frames too, and are read
//! by hand from the US bytes rather than counted as waits by their calls:
//! `Event_StepObject` (`$05A99E`) runs `d2 + 1` passes of map updates with
//! no field-object update, and `Event_UpdateObjFacing` (`$05A936`) ends in
//! one `VInt_Prepare`. [`HELPER_CENSUSED`] scenes give each call a row.
//! The motion helpers (`Event_MoveCamera`, `Event_MoveCharacter`,
//! `Event_MoveObjectStatic`) block until a position is reached: those are
//! #470's actor and camera loops, transcribed by their own ops, not waits.
//!
//! [`FRAME_CENSUSED`] scenes also give a row to every bare `VInt_Prepare`
//! frame outside their own loops ([`Retail::Frame`]) and every
//! `VInt_PrepareLoop` ([`Retail::Frames`]), which [`SceneOp::WaitFrames`]
//! transcribes. Palette fades and the DMA helpers that wait for VInt are
//! transcribed by their own ops and have no rows.
//!
//! The native runner treats every `Wait` the same today: map updates, no
//! field-object update. The census is what lets that change per loop
//! without guessing (#470); this file changes no behaviour.
//!
//! Scene files are censused one at a time ([`CENSUSED`]); the image check
//! skips without the local US image.

use super::documented_ranges::documented_ranges;
use super::{INSIDE_SPACESHIP_ROUTE, SCENES, dialogue_callers_image::rom};
use crate::SceneOp;

/// `DoMapUpdateLoop`.
const MAP_LOOP: u32 = 0x05_A71E;
/// `DoMainUpdatesLoop`.
const MAIN_LOOP: u32 = 0x05_A73C;
/// `Field_UpdateObjects`.
const UPDATE_OBJECTS: u32 = 0x04_47D8;
/// `RunMapUpdates`.
const RUN_MAP_UPDATES: u32 = 0x05_4938;
/// `VInt_Prepare`: one frame.
const VINT_PREPARE: u32 = 0x04_204C;
/// `Event_StepObject`.
const STEP_OBJECT: u32 = 0x05_A99E;
/// `Event_UpdateObjFacing`.
const UPDATE_OBJ_FACING: u32 = 0x05_A936;
/// `VInt_PrepareLoop`: `d0 + 1` frames, no updates.
const VINT_LOOP: u32 = 0x05_A7AC;

/// The retail wait a row describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Retail {
    /// `jsr DoMapUpdateLoop` at the address: map updates, no objects.
    Map(u32),
    /// `jsr DoMainUpdatesLoop` at the address: objects, then map updates.
    Main(u32),
    /// The scene's own loop from the address to its last branch back there;
    /// whether a pass calls `Field_UpdateObjects` and `RunMapUpdates`.
    Own {
        at: u32,
        objects: bool,
        map_updates: bool,
    },
    /// `jsr Event_StepObject` at the address: `d2 + 1` passes of map
    /// updates, no objects.
    Step(u32),
    /// `jsr Event_UpdateObjFacing` at the address: one frame, no map updates.
    Facing(u32),
    /// A bare `jsr VInt_Prepare` at the address, outside any loop: one
    /// frame, no map updates, and objects when `jsr Field_UpdateObjects`
    /// comes right before it.
    Frame { at: u32, objects: bool },
    /// `jsr VInt_PrepareLoop` at the address: `d0 + 1` frames, no updates.
    Frames(u32),
}

/// What transcribes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Claim {
    /// The scene's `SceneOp::Wait` at this op index.
    Wait(usize),
    /// Another op at this index waits it out (a palette loop's own frames).
    Within(usize),
    /// The op at this index records the loop's count in its own fields, but
    /// the runner does not block on it; the row's `defect` adds the wait.
    Recorded(usize),
    /// No op does: the issue that adds it.
    Missing(u32),
}

/// One retail wait of one scene. `defect` names the issue when the claim
/// is approximate (a fixed count for a loop that ends on state, or map
/// updates the loop does not run). `exact` keeps the count checked for a
/// defect that is only the map updates ([`updates`]).
#[derive(Debug, Clone, Copy)]
struct Row {
    scene: &'static str,
    retail: Retail,
    claim: Claim,
    defect: Option<u32>,
    exact: bool,
}

const fn row(scene: &'static str, retail: Retail, claim: Claim) -> Row {
    Row {
        scene,
        retail,
        claim,
        defect: None,
        exact: false,
    }
}

const fn frame(at: u32, objects: bool) -> Retail {
    Retail::Frame { at, objects }
}

const fn own(at: u32, objects: bool, map_updates: bool) -> Retail {
    Retail::Own {
        at,
        objects,
        map_updates,
    }
}

/// The scenes whose every `Wait` the census claims, by file.
const CENSUSED: &[&str] = &[
    // post_rika_events.rs
    "Event_MachineCenterAppearing",
    "Event_GettingLandRover",
    "Event_RuneLadaeTower",
    "Event_PsycoWandChest",
    "Event_ZioFortBarrier",
    "Event_ZioNurvus",
    // post_rika_cutscenes.rs
    "Cutscene_DemiRescue",
    "Cutscene_AlysWounded",
    "Cutscene_PsycoWand",
    "Cutscene_ZioDefeated",
    // post_zio_cutscenes.rs
    "Cutscene_MeetingWren",
    // Cutscene_InsideSpaceship's documented bytes are its 18-byte dispatch;
    // its six waits live in the destination menu's code, censused with it.
    "Cutscene_CrashLaanding",
    "Cutscene_Landale",
    "Event_KuranArrival",
    "Event_NearDarkForce1",
    "Event_DarkForce1",
    "Cutscene_DarkForce1Defeated",
    "Event_Juza",
    "Event_JuzaDefeated",
    // dezo_endgame.rs
    "Event_StrengthTowerTop",
    "Event_CourageTowerTop",
    "Event_DeVars",
    "Event_SaLews",
    "Event_ReFaze",
    "Event_DeVarsDefeated",
    "Event_SaLewsDefeated",
    "Cutscene_BeforeElsydeonCave",
    "Cutscene_Reunion",
    "Event_AngerTowerTop",
    "Event_SilenceTmDoorOpening",
    "Event_AngerTowerAlys",
    "Event_EnterAngerTower",
    "Event_AngerTowerExitTop",
    "Cutscene_ProfoundDarkness",
    // dezo_campaign.rs. `Cutscene_LeRoofAgain` ends in the ship's route,
    // whose waits are `Cutscene_InsideSpaceship`'s ([`censused_ops`]).
    "Event_MeetingLeRoof",
    "Cutscene_LeRoofAgain",
    "Event_CarnivorousTrees",
    "Event_SavingKyra",
    "Cutscene_MeetingKyra",
    "Event_EclipseTorchUsed",
    "Event_DarkForce2",
    "Cutscene_LutzRevelation",
];

/// The censused scenes whose shared-helper calls ([`Retail::Step`],
/// [`Retail::Facing`]) have rows too. The files censused before
/// dezo_endgame.rs wait for theirs (#572).
const HELPER_CENSUSED: &[&str] = &[
    "Event_StrengthTowerTop",
    "Event_CourageTowerTop",
    "Event_DeVars",
    "Event_SaLews",
    "Event_ReFaze",
    "Event_DeVarsDefeated",
    "Event_SaLewsDefeated",
    "Cutscene_BeforeElsydeonCave",
    "Cutscene_Reunion",
    "Event_AngerTowerTop",
    "Event_SilenceTmDoorOpening",
    "Event_AngerTowerAlys",
    "Event_EnterAngerTower",
    "Event_AngerTowerExitTop",
    "Cutscene_ProfoundDarkness",
    "Event_MeetingLeRoof",
    "Cutscene_LeRoofAgain",
    "Event_CarnivorousTrees",
    "Event_SavingKyra",
    "Cutscene_MeetingKyra",
    "Event_EclipseTorchUsed",
    "Event_DarkForce2",
    "Cutscene_LutzRevelation",
];

/// The censused scenes whose bare frames ([`Retail::Frame`]) and
/// `VInt_PrepareLoop` calls ([`Retail::Frames`]) have rows too: dezo_campaign.rs
/// onwards.
const FRAME_CENSUSED: &[&str] = &[
    "Event_MeetingLeRoof",
    "Cutscene_LeRoofAgain",
    "Event_CarnivorousTrees",
    "Event_SavingKyra",
    "Cutscene_MeetingKyra",
    "Event_EclipseTorchUsed",
    "Event_DarkForce2",
    "Cutscene_LutzRevelation",
];

/// dezo_endgame.rs's missing and approximated waits.
const DEZO: u32 = 571;
/// dezo_campaign.rs's missing and approximated waits.
const CAMPAIGN: u32 = 589;

use Claim::{Missing, Recorded, Wait, Within};
use Retail::{Facing, Frames, Main, Map, Step};

/// Whether the row's count must equal the retail one.
fn counted(row: &Row) -> bool {
    row.defect.is_none() || row.exact
}

mod rows;
use rows::ROWS;

fn scene(name: &str) -> &'static crate::Scene {
    SCENES
        .iter()
        .find(|scene| scene.name == name)
        .unwrap_or_else(|| panic!("{name} is not a registered scene"))
}

/// The ops of `name` its own retail bytes transcribe: all of them, but for a
/// scene that ends in the ship's route ([`INSIDE_SPACESHIP_ROUTE`]), whose
/// waits live in the destination menu's code and are censused with
/// `Cutscene_InsideSpaceship`.
fn censused_ops(name: &str) -> &'static [SceneOp] {
    let ops = scene(name).ops;
    ops.strip_suffix(INSIDE_SPACESHIP_ROUTE.as_slice())
        .unwrap_or(ops)
}

/// Whether a pass of the retail wait runs `RunMapUpdates`, as a native
/// `Wait` does.
fn runs_map_updates(retail: Retail) -> bool {
    match retail {
        Map(_) | Main(_) | Step(_) => true,
        Retail::Own { map_updates, .. } => map_updates,
        Facing(_) | Retail::Frame { .. } | Frames(_) => false,
    }
}

/// The address a row's retail call or loop starts at.
fn retail_at(retail: Retail) -> u32 {
    match retail {
        Map(at) | Main(at) | Step(at) | Facing(at) | Frames(at) => at,
        Retail::Own { at, .. } | Retail::Frame { at, .. } => at,
    }
}

/// The frames one retail loop of `op` waits, for an op that is not a
/// `Wait` but waits a library loop out itself: a palette loop's per-step
/// delay, or the barrier beams' updates through one `DoMainUpdatesLoop`.
/// `None` for an op that waits nothing.
fn within_ticks(op: &SceneOp) -> Option<u32> {
    match op {
        SceneOp::Presentation {
            op: crate::PresentationOp::PaletteRampFromTable { frame_delay, .. },
        } => Some(u32::from(*frame_delay)),
        SceneOp::Presentation {
            op: crate::PresentationOp::BarrierBeamUpdates { frames, .. },
        } => Some(u32::from(*frames)),
        SceneOp::StepObjectAlongFacing { frames, .. } => Some(u32::from(*frames)),
        SceneOp::WaitFrames { frames } => Some(u32::from(*frames)),
        _ => None,
    }
}

/// The loop count an op records without the runner blocking on it.
fn recorded_ticks(op: &SceneOp) -> Option<u32> {
    match op {
        SceneOp::ObjectAnimation { frames, .. } => Some(u32::from(*frames)),
        _ => None,
    }
}

/// The frames the row's claim stands for, from the claimed op.
fn claimed_ticks(row: &Row) -> Option<u32> {
    let op = |index: usize| scene(row.scene).ops.get(index);
    match row.claim {
        Wait(index) => match op(index) {
            Some(SceneOp::Wait { ticks }) => Some(u32::from(*ticks)),
            _ => None,
        },
        Within(index) => op(index).and_then(within_ticks),
        Recorded(index) => op(index).and_then(recorded_ticks),
        Missing(_) => None,
    }
}

/// Every `Wait` of a censused scene claimed by exactly one row, and every
/// row's claim naming an op that can carry it.
fn claim_problems(rows: &[Row]) -> Vec<String> {
    let mut problems = Vec::new();
    for &name in CENSUSED {
        let ops = censused_ops(name);
        for (index, op) in ops.iter().enumerate() {
            let claims = rows
                .iter()
                .filter(|row| row.scene == name && row.claim == Wait(index))
                .count();
            if matches!(op, SceneOp::Wait { .. }) && claims != 1 {
                problems.push(format!("{name} op {index}: a Wait with {claims} claims"));
            }
        }
    }
    for row in rows {
        if !CENSUSED.contains(&row.scene) {
            problems.push(format!("{}: a row for a scene not censused", row.scene));
        }
        if matches!(row.retail, Step(_) | Facing(_)) && !HELPER_CENSUSED.contains(&row.scene) {
            problems.push(format!(
                "{}: a helper row for a scene not censused for helpers",
                row.scene
            ));
        }
        if matches!(row.retail, Retail::Frame { .. } | Frames(_))
            && !FRAME_CENSUSED.contains(&row.scene)
        {
            problems.push(format!(
                "{}: a frame row for a scene not censused for frames",
                row.scene
            ));
        }
        let ops = censused_ops(row.scene);
        match row.claim {
            Wait(index) => {
                if !matches!(ops.get(index), Some(SceneOp::Wait { .. })) {
                    problems.push(format!(
                        "{} op {index}: claimed as a Wait, is {:?}",
                        row.scene,
                        ops.get(index)
                    ));
                }
                // A native `Wait` runs map updates on every tick.
                if !runs_map_updates(row.retail) && row.defect.is_none() {
                    problems.push(format!(
                        "{} op {index}: a Wait for a loop with no map updates, without a defect",
                        row.scene
                    ));
                }
                if row.exact && runs_map_updates(row.retail) {
                    problems.push(format!(
                        "{} op {index}: exact for a loop that runs map updates",
                        row.scene
                    ));
                }
            }
            Within(index) => {
                if ops.get(index).and_then(within_ticks).is_none() {
                    problems.push(format!(
                        "{} op {index}: Within names an op that waits no loop, {:?}",
                        row.scene,
                        ops.get(index)
                    ));
                }
                if !matches!(row.retail, Map(_) | Main(_) | Step(_) | Frames(_)) {
                    problems.push(format!("{} op {index}: Within an own loop", row.scene));
                }
            }
            Recorded(index) => {
                if ops.get(index).and_then(recorded_ticks).is_none() {
                    problems.push(format!(
                        "{} op {index}: Recorded names an op that records no count, {:?}",
                        row.scene,
                        ops.get(index)
                    ));
                }
                if row.defect.is_none() {
                    problems.push(format!(
                        "{} op {index}: Recorded without the defect that adds the wait",
                        row.scene
                    ));
                }
            }
            Missing(_) => {}
        }
        if row.exact && (row.defect.is_none() || !matches!(row.claim, Wait(_))) {
            problems.push(format!(
                "{}: exact on a row that is not a Wait with a defect",
                row.scene
            ));
        }
    }
    problems
}

#[test]
fn every_wait_of_a_censused_scene_is_claimed_once() {
    let problems = claim_problems(ROWS);
    assert!(problems.is_empty(), "{problems:#?}");
}

/// The `jsr` target at `at`, if the word there is `jsr (abs).l`.
fn jsr_target(rom: &[u8], at: usize) -> Option<u32> {
    (rom.get(at..at + 2)? == [0x4E, 0xB9])
        .then(|| u32::from_be_bytes(rom[at + 2..at + 6].try_into().unwrap()))
}

/// `at`, moved back over what may sit between a count's load and its call:
/// a `move.l d7,-(sp)` that saves an outer loop's counter, or a
/// `move.b #n,(abs).l` sound request.
fn before_count(rom: &[u8], at: usize) -> usize {
    if rom[at - 2..at] == [0x2F, 0x07] {
        at - 2
    } else if rom[at - 8..at - 6] == [0x13, 0xFC] && rom[at - 6] == 0 {
        at - 8
    } else {
        at
    }
}

/// The word-loaded tick count before a loop call: `moveq #n,d0` or
/// `move.w #n,d0` ending at `at`, or before what [`before_count`] skips, as
/// the `dbf` runs it (n + 1).
fn loop_ticks(rom: &[u8], at: usize) -> Option<u32> {
    let at = before_count(rom, at);
    if rom[at - 2] == 0x70 {
        return Some(u32::from(rom[at - 1]) + 1);
    }
    (rom[at - 4..at - 2] == [0x30, 0x3C])
        .then(|| u32::from(u16::from_be_bytes([rom[at - 2], rom[at - 1]])) + 1)
}

/// `Event_StepObject`'s passes: `moveq #n,d2` or `move.w #n,d2` ending at
/// `at`, as its `dbf` runs it (n + 1).
fn step_ticks(rom: &[u8], at: usize) -> Option<u32> {
    if rom[at - 2] == 0x74 {
        return Some(u32::from(rom[at - 1]) + 1);
    }
    (rom[at - 4..at - 2] == [0x34, 0x3C])
        .then(|| u32::from(u16::from_be_bytes([rom[at - 2], rom[at - 1]])) + 1)
}

/// The end of the scene's own loop at `at`: the last word within `$100`
/// bytes that branches back to it (`dbf`, `bcc.s`/`bra.s`, `bcc.w`).
fn loop_end(rom: &[u8], at: usize) -> Option<usize> {
    let back = |p: usize| -> Option<usize> {
        let word = u16::from_be_bytes([rom[p], rom[p + 1]]);
        let disp16 = || i16::from_be_bytes([rom[p + 2], rom[p + 3]]) as isize;
        let target = if word & 0xFFF8 == 0x51C8 {
            p as isize + 2 + disp16()
        } else if word & 0xF000 == 0x6000 && word & 0x00FF != 0 && word & 0x0F00 != 0x0100 {
            p as isize + 2 + (word as u8 as i8) as isize
        } else if word & 0xF0FF == 0x6000 && word & 0x0F00 != 0x0100 {
            p as isize + 2 + disp16()
        } else {
            return None;
        };
        (target == at as isize).then_some(p)
    };
    (at..at + 0x100).step_by(2).rev().find_map(back)
}

/// The passes of the scene's own loop from `at` to the `dbf` at `end`: the
/// `moveq #n,dN` or `move.w #n,dN` ending at `at` that loads the `dbf`'s
/// register, as the `dbf` runs it (n + 1). `None` when the loop ends on a
/// `bcc` or its count is loaded elsewhere.
fn own_ticks(rom: &[u8], at: usize, end: usize) -> Option<u32> {
    let word = u16::from_be_bytes([rom[end], rom[end + 1]]);
    if word & 0xFFF8 != 0x51C8 {
        return None;
    }
    let register = word & 7;
    if u16::from(rom[at - 2]) == 0x70 | register << 1 {
        return Some(u32::from(rom[at - 1]) + 1);
    }
    (u16::from_be_bytes([rom[at - 4], rom[at - 3]]) == 0x303C | register << 9)
        .then(|| u32::from(u16::from_be_bytes([rom[at - 2], rom[at - 1]])) + 1)
}

fn calls_in(rom: &[u8], from: usize, to: usize, target: u32) -> bool {
    calls(rom, from, to, target) > 0
}

fn calls(rom: &[u8], from: usize, to: usize, target: u32) -> usize {
    (from..to)
        .step_by(2)
        .filter(|&p| jsr_target(rom, p) == Some(target))
        .count()
}

/// Every row's retail call or loop as the US image has it, and every
/// claimed count equal to the retail one unless the row names a defect.
fn retail_problems(rom: &[u8], rows: &[Row]) -> Vec<String> {
    let mut problems = Vec::new();
    for row in rows {
        let ticks = claimed_ticks(row);
        let call = |at: u32, want: u32, problems: &mut Vec<String>| {
            if jsr_target(rom, at as usize) != Some(want) {
                problems.push(format!("{} ${at:06X}: not a jsr to ${want:06X}", row.scene));
            }
        };
        let retail = match row.retail {
            Map(at) | Main(at) => {
                let want = if matches!(row.retail, Map(_)) {
                    MAP_LOOP
                } else {
                    MAIN_LOOP
                };
                call(at, want, &mut problems);
                Some((at, loop_ticks(rom, at as usize)))
            }
            Step(at) => {
                call(at, STEP_OBJECT, &mut problems);
                Some((at, step_ticks(rom, at as usize)))
            }
            Facing(at) => {
                call(at, UPDATE_OBJ_FACING, &mut problems);
                Some((at, Some(1)))
            }
            Retail::Frame { at, objects } => {
                call(at, VINT_PREPARE, &mut problems);
                let after_objects = jsr_target(rom, at as usize - 6) == Some(UPDATE_OBJECTS);
                if after_objects != objects {
                    problems.push(format!("{} ${at:06X}: objects is not {objects}", row.scene));
                }
                Some((at, Some(1)))
            }
            Frames(at) => {
                call(at, VINT_LOOP, &mut problems);
                Some((at, loop_ticks(rom, at as usize)))
            }
            Retail::Own {
                at,
                objects,
                map_updates,
            } => {
                let at = at as usize;
                let Some(end) = loop_end(rom, at) else {
                    problems.push(format!("{} ${at:06X}: no branch back", row.scene));
                    continue;
                };
                if calls_in(rom, at, end, UPDATE_OBJECTS) != objects {
                    problems.push(format!("{} ${at:06X}: objects is not {objects}", row.scene));
                }
                if calls_in(rom, at, end, RUN_MAP_UPDATES) != map_updates {
                    problems.push(format!(
                        "{} ${at:06X}: map updates is not {map_updates}",
                        row.scene
                    ));
                }
                // The `Wait`s a transcription splits one loop into add up
                // to its passes; checked once, at the split's first row.
                let split: Vec<&Row> = rows
                    .iter()
                    .filter(|other| {
                        other.scene == row.scene
                            && other.retail == row.retail
                            && matches!(other.claim, Wait(_))
                    })
                    .collect();
                if let (Some(first), Some(passes)) = (split.first(), own_ticks(rom, at, end))
                    && std::ptr::eq(*first, row)
                    && split.iter().all(|other| counted(other))
                {
                    let ticks: u32 = split.iter().filter_map(|other| claimed_ticks(other)).sum();
                    if ticks != passes {
                        problems.push(format!(
                            "{} ${at:06X}: claims {ticks} frames, the loop runs {passes}",
                            row.scene
                        ));
                    }
                }
                None
            }
        };
        if let Some((at, retail)) = retail {
            if matches!(row.claim, Within(_) | Recorded(_)) && retail.is_none() {
                problems.push(format!("{} ${at:06X}: no loop count to check", row.scene));
            }
            if let (Some(ticks), Some(retail)) = (ticks, retail)
                && ticks != retail
                && counted(row)
            {
                problems.push(format!(
                    "{} ${at:06X}: claims {ticks} frames, the loop runs {retail}",
                    row.scene
                ));
            }
            if let (Recorded(_), Some(ticks), Some(retail)) = (row.claim, ticks, retail)
                && ticks != retail
            {
                problems.push(format!(
                    "{} ${at:06X}: records {ticks} frames, the loop runs {retail}",
                    row.scene
                ));
            }
        }
    }
    problems
}

#[test]
fn every_row_is_the_retail_wait_it_names() {
    let Some(rom) = rom() else {
        return;
    };
    let problems = retail_problems(rom, ROWS);
    assert!(problems.is_empty(), "{problems:#?}");
}

/// The retail call or loop at `at` that a censused scene must give a row:
/// a library loop, a shared helper in a [`HELPER_CENSUSED`] scene, or a
/// frame in a [`FRAME_CENSUSED`] scene outside the scene's own loops
/// (`rows` names those).
fn needs_row(rom: &[u8], rows: &[Row], name: &str, at: usize) -> Option<Retail> {
    let at32 = at as u32;
    let frames = FRAME_CENSUSED.contains(&name);
    match jsr_target(rom, at)? {
        MAP_LOOP => Some(Map(at32)),
        MAIN_LOOP => Some(Main(at32)),
        STEP_OBJECT if HELPER_CENSUSED.contains(&name) => Some(Step(at32)),
        UPDATE_OBJ_FACING if HELPER_CENSUSED.contains(&name) => Some(Facing(at32)),
        VINT_LOOP if frames => Some(Frames(at32)),
        VINT_PREPARE if frames => {
            let in_own_loop = rows.iter().any(|row| match row.retail {
                Retail::Own { at: start, .. } if row.scene == name => {
                    let start = start as usize;
                    loop_end(rom, start).is_some_and(|end| (start..end).contains(&at))
                }
                _ => false,
            });
            (!in_own_loop).then(|| frame(at32, jsr_target(rom, at - 6) == Some(UPDATE_OBJECTS)))
        }
        _ => None,
    }
}

/// Every library-loop call, and every shared-helper call of a
/// [`HELPER_CENSUSED`] scene, inside a censused scene's documented bytes
/// without a row.
fn coverage_problems(rom: &[u8], rows: &[Row]) -> Vec<String> {
    let ranges = documented_ranges();
    let mut problems = Vec::new();
    for &name in CENSUSED {
        let Some(spans) = ranges.get(name) else {
            problems.push(format!("{name}: no documented retail range"));
            continue;
        };
        // A scene's document and the registry may claim the same bytes.
        let mut seen = std::collections::BTreeSet::new();
        for &(start, end) in spans {
            for at in (start..end.saturating_sub(5)).step_by(2) {
                if !seen.insert(at) {
                    continue;
                }
                let Some(retail) = needs_row(rom, rows, name, at) else {
                    continue;
                };
                if !rows
                    .iter()
                    .any(|row| row.scene == name && row.retail == retail)
                {
                    problems.push(format!("{name} ${at:06X}: {retail:?} has no row"));
                }
            }
        }
    }
    problems
}

#[test]
fn every_library_loop_call_in_a_censused_scene_has_a_row() {
    let Some(rom) = rom() else {
        return;
    };
    let problems = coverage_problems(rom, ROWS);
    assert!(problems.is_empty(), "{problems:#?}");
}

/// The two fixed-frame helpers are what [`Retail::Step`] and
/// [`Retail::Facing`] say, read from the US bytes: `Event_StepObject`'s
/// loop (from past its two step stores) runs map updates and no object
/// update; `Event_UpdateObjFacing` reaches its `rts` through one
/// `VInt_Prepare` and neither update.
#[test]
fn the_shared_helpers_wait_as_their_rows_say() {
    let Some(rom) = rom() else {
        return;
    };
    let step = STEP_OBJECT as usize + 8;
    let end = loop_end(rom, step).expect("Event_StepObject's dbf");
    assert!(calls_in(rom, step, end, RUN_MAP_UPDATES));
    assert!(!calls_in(rom, step, end, UPDATE_OBJECTS));
    assert_eq!(calls(rom, step, end, VINT_PREPARE), 1);
    assert_eq!(rom[end..end + 2], [0x51, 0xCA], "dbf d2");

    let facing = UPDATE_OBJ_FACING as usize;
    let rts = (facing..facing + 0x100)
        .step_by(2)
        .find(|&p| rom[p..p + 2] == [0x4E, 0x75])
        .expect("Event_UpdateObjFacing's rts");
    assert_eq!(calls(rom, facing, rts, VINT_PREPARE), 1);
    assert!(!calls_in(rom, facing, rts, RUN_MAP_UPDATES));
    assert!(!calls_in(rom, facing, rts, UPDATE_OBJECTS));
    assert_eq!(loop_end(rom, facing), None, "no loop");

    let frames = VINT_LOOP as usize;
    let end = loop_end(rom, frames).expect("VInt_PrepareLoop's dbf");
    assert_eq!(calls(rom, frames, end, VINT_PREPARE), 1);
    assert!(!calls_in(rom, frames, end, RUN_MAP_UPDATES));
    assert!(!calls_in(rom, frames, end, UPDATE_OBJECTS));
    assert_eq!(rom[end..end + 2], [0x51, 0xC8], "dbf d0");
    assert_eq!(rom[end + 4..end + 6], [0x4E, 0x75], "rts");
}

/// The dezo_campaign.rs census rejects an omitted row of each kind it
/// added (a `Wait`'s claim, a bare frame, a `VInt_PrepareLoop`), a frame or
/// own loop claimed by a `Wait` with no defect, a claim on the ship's route,
/// and own-loop passes that do not add up.
#[test]
fn the_campaign_checks_reject_an_omission_and_a_wrong_count() {
    let without = |keep: &dyn Fn(&Row) -> bool| -> Vec<Row> {
        ROWS.iter().copied().filter(|row| keep(row)).collect()
    };
    let lutz = "Cutscene_LutzRevelation";
    let le_roof = "Event_MeetingLeRoof";
    let again = "Cutscene_LeRoofAgain";

    let omitted = without(&|row| !(row.scene == lutz && row.claim == Wait(12)));
    assert_eq!(
        claim_problems(&omitted),
        [format!("{lutz} op 12: a Wait with 0 claims")]
    );

    let mut unflagged = ROWS.to_vec();
    for row in &mut unflagged {
        if row.scene == "Event_EclipseTorchUsed" && row.claim == Wait(5) {
            row.defect = None;
            row.exact = false;
        }
        if row.scene == le_roof && row.claim == Wait(9) {
            row.defect = None;
            row.exact = false;
        }
        if row.scene == lutz && row.claim == Wait(10) {
            row.defect = Some(CAMPAIGN);
            row.exact = true;
        }
    }
    assert_eq!(
        claim_problems(&unflagged),
        [
            format!("{le_roof} op 9: a Wait for a loop with no map updates, without a defect"),
            "Event_EclipseTorchUsed op 5: a Wait for a loop with no map updates, without a defect"
                .to_owned(),
            format!("{lutz} op 10: exact for a loop that runs map updates"),
        ]
    );

    // The appended route's first `Wait` (224 ticks) is not the scene's own.
    let route_wait = censused_ops(again).len() + 9;
    assert!(matches!(
        scene(again).ops[route_wait],
        SceneOp::Wait { ticks: 224 }
    ));
    let mut on_route = ROWS.to_vec();
    on_route.push(row(again, Map(0x07_846C), Wait(route_wait)));
    assert_eq!(
        claim_problems(&on_route),
        [format!(
            "{again} op {route_wait}: claimed as a Wait, is None"
        )]
    );

    let Some(rom) = rom() else {
        return;
    };
    // The 180-pass loop's `Wait` moved to the 71-pass fade-in loop, and one
    // half of the Machine Center's split loop dropped.
    let mut miscounted = ROWS.to_vec();
    for row in &mut miscounted {
        if row.scene == le_roof && row.claim == Wait(9) {
            row.retail = own(0x07_05DC, false, false);
        }
        if row.scene == "Cutscene_MeetingKyra" && row.claim == Within(10) {
            row.retail = Frames(0x07_77FC);
        }
    }
    let machine = "Event_MachineCenterAppearing";
    miscounted.retain(|row| !(row.scene == machine && row.claim == Wait(7)));
    let machine_passes = own_ticks(rom, 0x06_B514, loop_end(rom, 0x06_B514).unwrap()).unwrap();
    let machine_claimed =
        claimed_ticks(&row(machine, own(0x06_B514, true, true), Wait(5))).unwrap();
    assert_eq!(
        retail_problems(rom, &miscounted),
        [
            format!(
                "{machine} $06B514: claims {machine_claimed} frames, the loop runs {machine_passes}"
            ),
            format!("{le_roof} $0705DC: claims 180 frames, the loop runs 71"),
            "Cutscene_MeetingKyra $0777FC: claims 60 frames, the loop runs 120".to_owned(),
        ]
    );

    let mut wrong_objects = ROWS.to_vec();
    for row in &mut wrong_objects {
        if row.scene == "Event_SavingKyra" && row.retail == frame(0x07_08EE, true) {
            row.retail = frame(0x07_08EE, false);
        }
    }
    assert_eq!(
        retail_problems(rom, &wrong_objects),
        ["Event_SavingKyra $0708EE: objects is not false".to_owned()]
    );

    for (scene, retail) in [
        ("Event_CarnivorousTrees", frame(0x07_07A0, false)),
        ("Event_SavingKyra", frame(0x07_08EE, true)),
        ("Cutscene_MeetingKyra", Frames(0x07_77FC)),
        (again, Map(0x07_846C)),
        (lutz, Facing(0x07_7964)),
    ] {
        let omitted = without(&|row| !(row.scene == scene && row.retail == retail));
        assert_eq!(
            coverage_problems(rom, &omitted),
            [format!(
                "{scene} ${:06X}: {retail:?} has no row",
                retail_at(retail)
            )]
        );
    }
}

/// The checks reject what they exist to catch: an omitted claim or call, a
/// wrong retail count, a recorded count without its defect.
#[test]
fn the_checks_reject_an_omission_and_a_wrong_count() {
    let without = |keep: &dyn Fn(&Row) -> bool| -> Vec<Row> {
        ROWS.iter().copied().filter(|row| keep(row)).collect()
    };
    let strength = "Event_StrengthTowerTop";

    let omitted = without(&|row| !(row.scene == strength && row.claim == Wait(1)));
    assert_eq!(
        claim_problems(&omitted),
        [format!("{strength} op 1: a Wait with 0 claims")]
    );

    let mut unflagged = ROWS.to_vec();
    for row in &mut unflagged {
        if row.scene == strength && row.claim == Recorded(2) {
            row.defect = None;
        }
    }
    assert_eq!(
        claim_problems(&unflagged),
        [format!(
            "{strength} op 2: Recorded without the defect that adds the wait"
        )]
    );

    let Some(rom) = rom() else {
        return;
    };
    // `Wait(1)` (60 frames) moved to the 15-frame call after it, and
    // `Recorded(2)` (15) to the 39-frame one.
    let mut miscounted = ROWS.to_vec();
    for row in &mut miscounted {
        if row.scene == strength && row.claim == Wait(1) {
            row.retail = Main(0x07_0E7C);
        }
        if row.scene == strength && row.claim == Recorded(2) {
            row.retail = Main(0x07_0F04);
        }
        if row.scene == "Event_AngerTowerAlys" && row.claim == Within(2) {
            row.retail = Step(0x07_1502);
        }
    }
    assert_eq!(
        retail_problems(rom, &miscounted),
        [
            format!("{strength} $070E7C: claims 60 frames, the loop runs 15"),
            format!("{strength} $070F04: records 15 frames, the loop runs 39"),
            "Event_AngerTowerAlys $071502: claims 8 frames, the loop runs 16".to_owned(),
        ]
    );

    for (scene, retail) in [
        (strength, Main(0x07_0F16)),
        (strength, Step(0x07_0F44)),
        ("Event_ReFaze", Facing(0x07_1590)),
    ] {
        let omitted = without(&|row| !(row.scene == scene && row.retail == retail));
        assert_eq!(
            coverage_problems(rom, &omitted),
            [format!(
                "{scene} ${:06X}: {retail:?} has no row",
                retail_at(retail)
            )]
        );
    }
}
