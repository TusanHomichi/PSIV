//! Every write of the panel byte, `Render_Sprites_In_Cutscenes` (`$FFFFECFD`),
//! is a [`SceneOp::SetRenderSpritesInCutscene`] (#630).
//!
//! Retail writes `move.b #1, (Render_Sprites_In_Cutscenes).w` before most
//! dialogues of a panel cutscene: the text routine's return clears the byte
//! (`Event_RunDialogue3`, `ps4.asm:121662`), and `TextCtrlCode_Portrait`
//! (`ps4.asm:142661`) reads it to put a portrait in the panel window
//! (`WinGroup_Event` record 4, (24,112)) rather than the talk one ((40,104)).
//! A transcription that writes it only before its first dialogue draws every
//! later portrait at the talk position.
//!
//! In every registered scene (the ship's route excluded, as
//! [`super::wait_census`] does) the ops' values, in op order, equal the
//! writes in the scene's documented bytes, in address order: `move.b #1`
//! (`11FC 0001 ECFD`) is `enabled: true`, `clr.b` (`4238 ECFD`) is
//! `enabled: false`. A scene whose ops and writes differ for a reason is a
//! [`ROWS`] entry naming both counts and the reason.
//!
//! The image check skips without the local US image.

use super::documented_ranges::documented_ranges;
use super::{INSIDE_SPACESHIP_ROUTE, SCENES, dialogue_callers_image::rom};
use crate::SceneOp;

/// `move.b #1, ($ECFD).w`.
const SET: [u8; 6] = [0x11, 0xFC, 0x00, 0x01, 0xEC, 0xFD];
/// `clr.b ($ECFD).w`.
const CLEAR: [u8; 4] = [0x42, 0x38, 0xEC, 0xFD];

/// Why a scene's ops and writes differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Why {
    /// Writes in bytes the transcription does not run: the issue that will.
    Untranscribed(u32),
}

/// One scene whose ops and writes differ.
#[derive(Debug, Clone, Copy)]
struct Row {
    scene: &'static str,
    ops: usize,
    writes: usize,
    why: Why,
}

const ROWS: &[Row] = &[];

/// One scene's op values, and its writes as `(address, value)`.
#[derive(Debug, Clone)]
struct Counted {
    scene: String,
    ops: Vec<bool>,
    writes: Vec<(usize, bool)>,
}

/// The write at `at`, if one starts there.
fn write_at(rom: &[u8], at: usize) -> Option<bool> {
    if rom.get(at..at + SET.len()) == Some(&SET) {
        Some(true)
    } else if rom.get(at..at + CLEAR.len()) == Some(&CLEAR) {
        Some(false)
    } else {
        None
    }
}

/// Each registered scene's ops and the writes in its documented bytes
/// (overlapping ranges merged, so a range the scene's document and the
/// registry both list counts once).
fn counts(rom: &[u8], scenes: &[(&str, &[SceneOp])]) -> Vec<Counted> {
    let ranges = documented_ranges();
    let mut out = Vec::new();
    for &(name, ops) in scenes {
        let ops = ops
            .strip_suffix(INSIDE_SPACESHIP_ROUTE.as_slice())
            .unwrap_or(ops);
        let ops = ops
            .iter()
            .filter_map(|op| match op {
                SceneOp::SetRenderSpritesInCutscene { enabled } => Some(*enabled),
                _ => None,
            })
            .collect();
        let mut spans = ranges.get(name).cloned().unwrap_or_default();
        spans.sort_unstable();
        let mut merged: Vec<(usize, usize)> = Vec::new();
        for (start, end) in spans {
            match merged.last_mut() {
                Some(last) if start <= last.1 => last.1 = last.1.max(end),
                _ => merged.push((start, end)),
            }
        }
        let writes = merged
            .iter()
            .flat_map(|&(start, end)| (start..end).step_by(2))
            .filter_map(|at| write_at(rom, at).map(|value| (at, value)))
            .collect();
        out.push(Counted {
            scene: name.to_owned(),
            ops,
            writes,
        });
    }
    out
}

/// Every scene whose ops and writes differ without a row saying so, and
/// every row that no longer matches its scene.
fn problems(counted: &[Counted], rows: &[Row]) -> Vec<String> {
    let mut problems = Vec::new();
    for c in counted {
        let name = &c.scene;
        let row = rows.iter().find(|row| row.scene == name);
        let (ops, writes) = (c.ops.len(), c.writes.len());
        match row {
            None => {
                let values: Vec<bool> = c.writes.iter().map(|&(_, v)| v).collect();
                if let Some(at) = (0..ops.max(writes)).find(|&i| c.ops.get(i) != values.get(i)) {
                    let write = c.writes.get(at).map_or("no write".to_owned(), |&(a, v)| {
                        format!("${a:06X} = {}", u8::from(v))
                    });
                    let op = c
                        .ops
                        .get(at)
                        .map_or("no op".to_owned(), |v| format!("enabled: {v}"));
                    problems.push(format!(
                        "{name}: write {at} is {write}, op {at} is {op} ({ops} ops, {writes} writes, no row)"
                    ));
                }
            }
            Some(row) if (row.ops, row.writes) != (ops, writes) => {
                let Why::Untranscribed(issue) = row.why;
                problems.push(format!(
                    "{name}: the row (#{issue}) says {} ops for {} writes, the scene has {ops} for {writes}",
                    row.ops, row.writes
                ));
            }
            Some(_) => {}
        }
    }
    for row in rows {
        match counted.iter().find(|c| c.scene == row.scene) {
            None => problems.push(format!("{}: a row for no registered scene", row.scene)),
            Some(c) if c.ops.len() == c.writes.len() => {
                problems.push(format!("{}: a row for counts that agree", row.scene))
            }
            Some(_) => {}
        }
    }
    problems
}

fn registered() -> Vec<(&'static str, &'static [SceneOp])> {
    SCENES.iter().map(|scene| (scene.name, scene.ops)).collect()
}

#[test]
fn every_panel_byte_write_is_an_op() {
    let Some(rom) = rom() else { return };
    let counted = counts(rom, &registered());
    assert_eq!(problems(&counted, ROWS), Vec::<String>::new());
}

/// Negative controls, each through the real counting: an op dropped (#630's
/// defect: the Ending's second dialogue loses its write), an op added where
/// retail writes nothing, a `clr.b` transcribed as `enabled: true`, two ops
/// out of order, and the rows: a stale count, a row for counts that agree
/// and a row for no scene.
#[test]
fn the_panel_byte_census_rejects_a_missing_extra_or_misordered_op() {
    let Some(rom) = rom() else { return };
    let scene = |name: &str| SCENES.iter().find(|s| s.name == name).unwrap();
    let is_write = |op: &SceneOp| matches!(op, SceneOp::SetRenderSpritesInCutscene { .. });
    let check = |name: &'static str, ops: &[SceneOp], rows: &[Row]| {
        problems(&counts(rom, &[(name, ops)]), rows)
    };

    let ending = scene("Cutscene_Ending").ops;
    let second = ending
        .iter()
        .enumerate()
        .filter(|(_, op)| is_write(op))
        .nth(1)
        .unwrap()
        .0;
    let mut missing = ending.to_vec();
    missing.remove(second);
    assert_eq!(
        check("Cutscene_Ending", &missing, ROWS),
        ["Cutscene_Ending: write 25 is $079BAC = 1, op 25 is no op (25 ops, 26 writes, no row)"]
    );

    let mut extra = scene("Cutscene_MeetingSeth").ops.to_vec();
    extra.insert(0, SceneOp::SetRenderSpritesInCutscene { enabled: true });
    assert_eq!(
        check("Cutscene_MeetingSeth", &extra, ROWS),
        [
            "Cutscene_MeetingSeth: write 1 is no write, op 1 is enabled: true (2 ops, 1 writes, no row)"
        ]
    );

    let crash = scene("Cutscene_CrashLaanding").ops;
    let writes: Vec<usize> = (0..crash.len()).filter(|&i| is_write(&crash[i])).collect();
    let mut flipped = crash.to_vec();
    flipped[writes[3]] = SceneOp::SetRenderSpritesInCutscene { enabled: true };
    assert_eq!(
        check("Cutscene_CrashLaanding", &flipped, ROWS),
        [
            "Cutscene_CrashLaanding: write 3 is $076C1A = 0, op 3 is enabled: true (8 ops, 8 writes, no row)"
        ]
    );

    let lashiec = scene("Cutscene_LashiecDefeated").ops;
    let writes: Vec<usize> = (0..lashiec.len())
        .filter(|&i| is_write(&lashiec[i]))
        .collect();
    let mut swapped = lashiec.to_vec();
    swapped.swap(writes[1], writes[2]);
    assert_eq!(
        check("Cutscene_LashiecDefeated", &swapped, ROWS),
        [
            "Cutscene_LashiecDefeated: write 1 is $077B22 = 1, op 1 is enabled: false (3 ops, 3 writes, no row)"
        ]
    );

    // A row accounts for the difference only at the counts it names.
    let row = Row {
        scene: "Cutscene_Ending",
        ops: 25,
        writes: 26,
        why: Why::Untranscribed(630),
    };
    assert_eq!(
        check("Cutscene_Ending", &missing, &[row]),
        Vec::<String>::new()
    );
    let stale = Row { ops: 24, ..row };
    assert_eq!(
        check("Cutscene_Ending", &missing, &[stale]),
        ["Cutscene_Ending: the row (#630) says 24 ops for 26 writes, the scene has 25 for 26"]
    );
    assert_eq!(
        check("Cutscene_Ending", ending, &[row]),
        [
            "Cutscene_Ending: the row (#630) says 25 ops for 26 writes, the scene has 26 for 26",
            "Cutscene_Ending: a row for counts that agree",
        ]
    );
    let nowhere = Row {
        scene: "Cutscene_Nowhere",
        ..row
    };
    assert_eq!(
        check("Cutscene_Ending", ending, &[nowhere]),
        ["Cutscene_Nowhere: a row for no registered scene"]
    );
}
