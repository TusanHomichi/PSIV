//! Which bare display-bit calls each scene's retail bytes make (#653).
//!
//! VDP register 1's display-enable bit is set by `VDP_EnableDisplay`
//! (`$04222E`) and cleared by `ClearSpriteTableBuf` (`$04227C`). The palette
//! fades call them inside their own routines, and their ops carry that
//! (`crate::palette`, the runtime's CRAM model). A scene that calls one
//! itself needs a record of it, or the runtime keeps the display as the last
//! fade left it: the screen stays dark after a `VDP_EnableDisplay`, or lit
//! after a `ClearSpriteTableBuf`.
//!
//! The check reads every `jsr`/`jmp` to the two inside a scene's documented
//! ranges off the US image, in address order. The `VDP_EnableDisplay` calls
//! must be the scene's [`PresentationOp::EnableDisplay`] records, in order.
//! No op records a bare `ClearSpriteTableBuf`: a scene that calls one must
//! be in [`DARK_ALREADY`], with the reason no frame tells the difference. It
//! skips without the image.

use super::SCENES;
use super::dialogue_callers_image::rom;
use super::documented_ranges::{Ranges, documented_ranges};
use crate::{PresentationOp, SceneOp};

/// `VDP_EnableDisplay`'s US entry point.
const ENABLE: u32 = 0x04_222E;
/// `ClearSpriteTableBuf`'s US entry point.
const DISABLE: u32 = 0x04_227C;

/// Scenes that clear the display bit themselves, and why the runtime need
/// not: the screen is black either side of the call.
const DARK_ALREADY: &[(&str, &str)] = &[(
    "Cutscene_PsycoWand",
    "$07536A: the buffer was cleared (`trap #0`) and uploaded the frame before, \
     and the next call is InitVRAMAndCRAM, whose fade-out over black clears the \
     bit itself after one frame",
)];

/// A display-bit call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Call {
    Enable,
    Disable,
}

/// The calls in `ranges` of the image, in address order.
fn retail_calls(rom: &[u8], ranges: &[(usize, usize)]) -> Vec<(usize, Call)> {
    let mut ranges = ranges.to_vec();
    ranges.sort_unstable();
    ranges.dedup();
    let mut calls = Vec::new();
    for (start, end) in ranges {
        for at in (start..end.min(rom.len().saturating_sub(5))).step_by(2) {
            let bytes = &rom[at..at + 6];
            if bytes[..2] == [0x4E, 0xB9] || bytes[..2] == [0x4E, 0xF9] {
                match u32::from_be_bytes(bytes[2..].try_into().unwrap()) {
                    ENABLE => calls.push((at, Call::Enable)),
                    DISABLE => calls.push((at, Call::Disable)),
                    _ => {}
                }
            }
        }
    }
    calls
}

fn recorded(op: &SceneOp) -> bool {
    matches!(
        op,
        SceneOp::Presentation {
            op: PresentationOp::EnableDisplay
        }
    )
}

/// Each scene with documented ranges whose records are not its calls.
fn mismatches(rom: &[u8], ranges: &Ranges, scenes: &[crate::Scene]) -> Vec<String> {
    let mut problems = Vec::new();
    for scene in scenes {
        let Some(ranges) = ranges.get(scene.name) else {
            continue;
        };
        let calls = retail_calls(rom, ranges);
        let enables = calls.iter().filter(|(_, call)| *call == Call::Enable);
        let records = scene.ops.iter().filter(|op| recorded(op)).count();
        if enables.clone().count() != records {
            let at: Vec<String> = enables.map(|(at, _)| format!("${at:06X}")).collect();
            problems.push(format!(
                "{}: retail calls VDP_EnableDisplay at [{}], the scene records {records}",
                scene.name,
                at.join(", ")
            ));
        }
        let listed = DARK_ALREADY.iter().any(|(name, _)| *name == scene.name);
        let disables: Vec<String> = calls
            .iter()
            .filter(|(_, call)| *call == Call::Disable)
            .map(|(at, _)| format!("${at:06X}"))
            .collect();
        if disables.is_empty() == listed {
            problems.push(format!(
                "{}: retail calls ClearSpriteTableBuf at [{}], DARK_ALREADY {} it",
                scene.name,
                disables.join(", "),
                if listed { "lists" } else { "does not list" }
            ));
        }
    }
    problems
}

#[test]
fn every_scene_records_the_display_calls_its_bytes_make() {
    let Some(rom) = rom() else {
        return;
    };
    let problems = mismatches(rom, &documented_ranges(), SCENES);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    // The two calls the scenes make, so a range or address that drifts
    // cannot pass by finding none.
    let ranges = documented_ranges();
    let found: usize = SCENES
        .iter()
        .filter_map(|scene| ranges.get(scene.name))
        .map(|ranges| retail_calls(rom, ranges).len())
        .sum();
    assert_eq!(
        found, 3,
        "Alshline's and the Ending's enables, PsycoWand's clear"
    );
}

/// Negative controls: each record dropped or doubled, a record added to a
/// scene that makes no call, and `DARK_ALREADY`'s scene unlisted are each
/// reported.
#[test]
fn a_wrong_display_record_is_reported() {
    let Some(rom) = rom() else {
        return;
    };
    let ranges = documented_ranges();
    let mut altered_count = 0;
    let enable = SceneOp::Presentation {
        op: PresentationOp::EnableDisplay,
    };
    for scene in SCENES {
        if !ranges.contains_key(scene.name) {
            continue;
        }
        let mut variants: Vec<Vec<SceneOp>> = Vec::new();
        for (index, op) in scene.ops.iter().enumerate() {
            if recorded(op) {
                let mut dropped = scene.ops.to_vec();
                dropped.remove(index);
                variants.push(dropped);
                let mut doubled = scene.ops.to_vec();
                doubled.insert(index, *op);
                variants.push(doubled);
            }
        }
        if !scene.ops.iter().any(recorded) {
            let mut added = scene.ops.to_vec();
            added.insert(0, enable);
            variants.push(added);
        }
        for ops in variants {
            let altered = crate::Scene {
                ops: Box::leak(ops.into_boxed_slice()),
                ..*scene
            };
            assert_eq!(
                mismatches(rom, &ranges, &[altered]).len(),
                1,
                "{} altered passed",
                scene.name
            );
            altered_count += 1;
        }
    }
    assert!(
        altered_count > 100,
        "only {altered_count} alterations checked"
    );
    let psyco = SCENES
        .iter()
        .find(|scene| scene.name == DARK_ALREADY[0].0)
        .unwrap();
    let renamed = crate::Scene {
        name: "Cutscene_Alshline",
        ..*psyco
    };
    let mut ranges = ranges;
    ranges.insert("Cutscene_Alshline".into(), ranges[psyco.name].clone());
    assert_eq!(
        mismatches(rom, &ranges, &[renamed]),
        [
            "Cutscene_Alshline: retail calls ClearSpriteTableBuf at [$07536A], DARK_ALREADY does not list it"
        ]
    );
}
