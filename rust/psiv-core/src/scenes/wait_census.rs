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
//! The native runner treats every `Wait` the same today: map updates, no
//! field-object update. The census is what lets that change per loop
//! without guessing (#470); this file changes no behaviour.
//!
//! Scene files are censused one at a time ([`CENSUSED`]); the image check
//! skips without the local US image.

use super::documented_ranges::documented_ranges;
use super::{SCENES, dialogue_callers_image::rom};
use crate::SceneOp;

/// `DoMapUpdateLoop`.
const MAP_LOOP: u32 = 0x05_A71E;
/// `DoMainUpdatesLoop`.
const MAIN_LOOP: u32 = 0x05_A73C;
/// `Field_UpdateObjects`.
const UPDATE_OBJECTS: u32 = 0x04_47D8;
/// `RunMapUpdates`.
const RUN_MAP_UPDATES: u32 = 0x05_4938;

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
}

/// What transcribes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Claim {
    /// The scene's `SceneOp::Wait` at this op index.
    Wait(usize),
    /// Another op at this index waits it out (a palette loop's own frames).
    Within(usize),
    /// No op does: the issue that adds it.
    Missing(u32),
}

/// One retail wait of one scene. `defect` names the issue when the claim
/// is approximate (a fixed count for a loop that ends on state, or map
/// updates the loop does not run).
struct Row {
    scene: &'static str,
    retail: Retail,
    claim: Claim,
    defect: Option<u32>,
}

const fn row(scene: &'static str, retail: Retail, claim: Claim) -> Row {
    Row {
        scene,
        retail,
        claim,
        defect: None,
    }
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
];

use Claim::{Missing, Wait, Within};
use Retail::{Main, Map};

#[rustfmt::skip]
const ROWS: &[Row] = &[
    // post_rika_events.rs
    // One loop of `$170 + 1` passes the transcription splits at the door
    // chunk's beat (d7 `$3D`).
    row("Event_MachineCenterAppearing", own(0x06_B514, true, true), Wait(5)),
    row("Event_MachineCenterAppearing", own(0x06_B514, true, true), Wait(7)),
    row("Event_GettingLandRover", Main(0x06_DF08), Wait(3)),
    row("Event_GettingLandRover", Map(0x06_DF2E), Wait(6)),
    // The BG camera's scroll: `RunMapUpdates` and `VInt_Prepare`, no objects.
    row("Event_GettingLandRover", own(0x06_DF52, false, true), Wait(9)),
    row("Event_GettingLandRover", Map(0x06_DF98), Wait(12)),
    row("Event_GettingLandRover", Map(0x06_DFDA), Wait(19)),
    Row { defect: Some(496), ..row("Event_PsycoWandChest", own(0x06_EA4E, true, false), Wait(3)) },
    Row { defect: Some(496), ..row("Event_PsycoWandChest", own(0x06_EA88, true, false), Wait(6)) },
    Row { defect: Some(496), ..row("Event_PsycoWandChest", own(0x06_EAA0, true, false), Wait(8)) },
    row("Event_PsycoWandChest", Map(0x06_EAB6), Missing(496)),
    Row { defect: Some(496), ..row("Event_PsycoWandChest", own(0x06_EB32, true, false), Wait(14)) },
    row("Event_PsycoWandChest", Map(0x06_EB5E), Missing(496)),
    row("Event_ZioFortBarrier", Main(0x06_ED0C), Wait(15)),
    // `Palette_Line_4`'s ramp waits `DoMapUpdateLoop($1D)` between steps.
    row("Event_ZioFortBarrier", Map(0x06_ED62), Within(17)),
    row("Event_ZioFortBarrier", Map(0x06_ED70), Wait(18)),
    row("Event_ZioFortBarrier", Map(0x06_EDBC), Wait(23)),
    row("Event_ZioFortBarrier", Main(0x06_EDEA), Wait(28)),
    row("Event_ZioNurvus", Map(0x06_F2F4), Wait(1)),
    row("Event_ZioNurvus", Map(0x06_F300), Missing(496)),
    row("Event_ZioNurvus", own(0x06_F384, true, true), Wait(6)),
    row("Event_ZioNurvus", Map(0x06_F3A4), Wait(7)),
    row("Event_ZioNurvus", Map(0x06_F3CC), Wait(11)),
];

fn scene(name: &str) -> &'static crate::Scene {
    SCENES
        .iter()
        .find(|scene| scene.name == name)
        .unwrap_or_else(|| panic!("{name} is not a registered scene"))
}

#[test]
fn every_wait_of_a_censused_scene_is_claimed_once() {
    let mut problems = Vec::new();
    for &name in CENSUSED {
        let ops = scene(name).ops;
        for (index, op) in ops.iter().enumerate() {
            let claims = ROWS
                .iter()
                .filter(|row| row.scene == name && row.claim == Wait(index))
                .count();
            match (matches!(op, SceneOp::Wait { .. }), claims) {
                (true, 0) => problems.push(format!("{name} op {index}: a Wait no row claims")),
                (false, n) if n > 0 => {
                    problems.push(format!("{name} op {index}: claimed as a Wait, is {op:?}"));
                }
                _ => {}
            }
        }
    }
    for row in ROWS {
        if !CENSUSED.contains(&row.scene) {
            problems.push(format!("{}: a row for a scene not censused", row.scene));
        }
        if let Within(index) = row.claim {
            match scene(row.scene).ops.get(index) {
                Some(SceneOp::Wait { .. }) | None => {
                    problems.push(format!(
                        "{} op {index}: Within names no other op",
                        row.scene
                    ));
                }
                Some(_) => {}
            }
        }
    }
    assert!(problems.is_empty(), "{problems:#?}");
}

/// The `jsr` target at `at`, if the word there is `jsr (abs).l`.
fn jsr_target(rom: &[u8], at: usize) -> Option<u32> {
    (rom.get(at..at + 2)? == [0x4E, 0xB9])
        .then(|| u32::from_be_bytes(rom[at + 2..at + 6].try_into().unwrap()))
}

/// The word-loaded tick count before a loop call: `moveq #n,d0` or
/// `move.w #n,d0` ending at `at`, as the `dbf` runs it (n + 1).
fn loop_ticks(rom: &[u8], at: usize) -> Option<u32> {
    if rom[at - 2] == 0x70 {
        return Some(u32::from(rom[at - 1]) + 1);
    }
    (rom[at - 4..at - 2] == [0x30, 0x3C])
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

fn calls_in(rom: &[u8], from: usize, to: usize, target: u32) -> bool {
    (from..to)
        .step_by(2)
        .any(|p| jsr_target(rom, p) == Some(target))
}

#[test]
fn every_row_is_the_retail_wait_it_names() {
    let Some(rom) = rom() else {
        return;
    };
    let mut problems = Vec::new();
    for row in ROWS {
        let ticks = match row.claim {
            Wait(index) => match scene(row.scene).ops[index] {
                SceneOp::Wait { ticks } => Some(u32::from(ticks)),
                _ => None,
            },
            _ => None,
        };
        match row.retail {
            Map(at) | Main(at) => {
                let want = if matches!(row.retail, Map(_)) {
                    MAP_LOOP
                } else {
                    MAIN_LOOP
                };
                if jsr_target(rom, at as usize) != Some(want) {
                    problems.push(format!("{} ${at:06X}: not a jsr to ${want:06X}", row.scene));
                }
                if let (Some(ticks), Some(retail)) = (ticks, loop_ticks(rom, at as usize))
                    && ticks != retail
                    && row.defect.is_none()
                {
                    problems.push(format!(
                        "{} ${at:06X}: Wait {ticks}, the loop runs {retail}",
                        row.scene
                    ));
                }
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
            }
        }
    }
    assert!(problems.is_empty(), "{problems:#?}");
}

#[test]
fn every_library_loop_call_in_a_censused_scene_has_a_row() {
    let Some(rom) = rom() else {
        return;
    };
    let ranges = documented_ranges();
    let mut problems = Vec::new();
    for &name in CENSUSED {
        let Some(spans) = ranges.get(name) else {
            problems.push(format!("{name}: no documented retail range"));
            continue;
        };
        for &(start, end) in spans {
            for at in (start..end.saturating_sub(5)).step_by(2) {
                let retail = match jsr_target(rom, at) {
                    Some(MAP_LOOP) => Map(at as u32),
                    Some(MAIN_LOOP) => Main(at as u32),
                    _ => continue,
                };
                if !ROWS
                    .iter()
                    .any(|row| row.scene == name && row.retail == retail)
                {
                    problems.push(format!("{name} ${at:06X}: {retail:?} has no row"));
                }
            }
        }
    }
    assert!(problems.is_empty(), "{problems:#?}");
}
