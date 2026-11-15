//! The shared code a censused scene's documented bytes jump into
//! ([`SHARED_CODE`]), and the checks that keep it and every row inside the
//! bytes the census reads: a span must be reached from its scene and closed,
//! and a row must lie in its scene's censused bytes (#470, #522).

use super::{ROWS, Row, SHIP, jsr_target, retail_at};
use crate::scenes::dialogue_callers_image::rom;
use crate::scenes::documented_ranges::{Ranges, documented_ranges};

/// `GetPlaneAOffset`: the one routine outside the ship's code that it calls
/// with `bsr`.
const GET_PLANE_A_OFFSET: u32 = 0x06_893A;

/// Shared code a censused scene's documented bytes jump into, whose waits are
/// the scene's: `(scene, start, end_exclusive)`. Each span starts at a target
/// of the scene's other censused bytes, and every `bsr` inside it lands in
/// the bytes the scene is censused over or at [`BSR_OUTSIDE`]
/// ([`shared_code_problems`]).
pub(in crate::scenes) const SHARED_CODE: &[Shared] = &[
    // `jmp loc_63BC4`, through `loc_64C1E`'s closing `jmp`
    // (`ps4.asm:133499-134714`): the destination menu, its screens, both
    // flights with their tables, and the planet screen.
    (SHIP, 0x06_3BC4, 0x06_4D00),
    // `jsr Event_DezoSpaceportAppearing` at `$0771C8`, through its last
    // subroutine's `rts` (`loc_6C168`, `ps4.asm:145470-145560`): the grave's
    // `$170 + 1` pass loop.
    ("Cutscene_Landale", 0x06_C040, 0x06_C1BC),
    // `jsr loc_705D6` at `$078406`: `Event_MeetingLeRoof`'s fade-in loop
    // (`ps4.asm:149798-149828`) and the colour step it calls each pass,
    // `loc_70638` (`:149838-149855`).
    ("Cutscene_LeRoofAgain", 0x07_05D6, 0x07_0626),
    ("Cutscene_LeRoofAgain", 0x07_0638, 0x07_0672),
];

/// The routines outside a [`SHARED_CODE`] span its `bsr`s may reach.
const BSR_OUTSIDE: &[u32] = &[GET_PLANE_A_OFFSET];

/// One scene's shared code: `(scene, start, end_exclusive)`.
pub(in crate::scenes) type Shared = (&'static str, u32, u32);

/// The bytes `name` is censused over: its documented ranges and its
/// `shared` code ([`SHARED_CODE`]). `None` when nothing documents the scene.
pub(super) fn scanned_spans(
    ranges: &Ranges,
    shared: &[Shared],
    name: &str,
) -> Option<Vec<(usize, usize)>> {
    let mut spans = ranges.get(name)?.clone();
    spans.extend(
        shared
            .iter()
            .filter(|(scene, ..)| *scene == name)
            .map(|&(_, start, end)| (start as usize, end as usize)),
    );
    Some(spans)
}

/// Every row's retail call or loop inside the bytes its scene is censused
/// over, so a row cannot stand for a wait the coverage check never reads.
pub(super) fn placement_problems(rows: &[Row], shared: &[Shared]) -> Vec<String> {
    let ranges = documented_ranges();
    rows.iter()
        .filter_map(|row| {
            let at = retail_at(row.retail) as usize;
            let spans = scanned_spans(&ranges, shared, row.scene).unwrap_or_default();
            (!spans.iter().any(|&(start, end)| (start..end).contains(&at))).then(|| {
                format!(
                    "{} ${at:06X}: outside the scene's censused bytes",
                    row.scene
                )
            })
        })
        .collect()
}

#[test]
fn every_row_lies_in_its_scenes_censused_bytes() {
    let problems = placement_problems(ROWS, SHARED_CODE);
    assert!(problems.is_empty(), "{problems:#?}");
}

/// The target of a `bsr.s`/`bsr.w` at `at`.
fn bsr_target(rom: &[u8], at: usize) -> Option<u32> {
    let word = u16::from_be_bytes([rom[at], rom[at + 1]]);
    if word & 0xFF00 != 0x6100 || word & 0xFF == 0xFF {
        return None;
    }
    let disp = match word as u8 {
        0 => i16::from_be_bytes([rom[at + 2], rom[at + 3]]) as isize,
        short => short as i8 as isize,
    };
    u32::try_from(at as isize + 2 + disp).ok()
}

/// Each [`SHARED_CODE`] span reached from its scene and closed: it starts at
/// a `jsr`/`jmp`/`bsr` target of the scene's other censused bytes, and every
/// `bsr` inside it lands in the scene's censused bytes or at
/// [`BSR_OUTSIDE`]. A span cut short leaves a called routine out.
pub(super) fn shared_code_problems(rom: &[u8], shared: &[Shared]) -> Vec<String> {
    let ranges = documented_ranges();
    let mut problems = Vec::new();
    for &(name, start, end) in shared {
        let spans = scanned_spans(&ranges, shared, name).unwrap_or_default();
        let target = |at: usize| {
            let word = u16::from_be_bytes([rom[at], rom[at + 1]]);
            (word == 0x4EF9)
                .then(|| u32::from_be_bytes(rom[at + 2..at + 6].try_into().unwrap()))
                .or_else(|| jsr_target(rom, at))
                .or_else(|| bsr_target(rom, at))
        };
        let span = (start as usize, end as usize);
        let reached = spans
            .iter()
            .filter(|&&other| other != span)
            .any(|&(from, to)| {
                (from..to.saturating_sub(5))
                    .step_by(2)
                    .any(|at| target(at) == Some(start))
            });
        if !reached {
            problems.push(format!(
                "{name} ${start:06X}: not reached from its other bytes"
            ));
        }
        for at in (start as usize..end as usize).step_by(2) {
            let Some(called) = bsr_target(rom, at) else {
                continue;
            };
            let inside = spans
                .iter()
                .any(|&(from, to)| (from..to).contains(&(called as usize)));
            if !inside && !BSR_OUTSIDE.contains(&called) {
                problems.push(format!(
                    "{name} ${at:06X}: bsr to ${called:06X}, outside its censused bytes"
                ));
            }
        }
    }
    problems
}

#[test]
fn shared_code_is_reached_and_closed() {
    let Some(rom) = rom() else {
        return;
    };
    let problems = shared_code_problems(rom, SHARED_CODE);
    assert!(problems.is_empty(), "{problems:#?}");
}
