//! The image half of the dialogue-entry census: every caller re-derived from
//! the US image and held against [`DIALOGUE_CALLERS`].
//!
//! `Event_ZioNurvus` ends its presentation with `moveq #$B, d0 / jsr
//! Event_GetAndRunDialogue2` (`ps4.asm:148622-148623`). The port transcribed
//! that as a `RunDialogueResume`, which reopens `Saved_Dialogue_Addr`: in play
//! that is whatever dialogue ran last, and after a load there is nothing to
//! resume. Every other `Event_GetAndRunDialogue2` caller sat on the standard
//! window, which destroys the panels the cartridge leaves up; seven of the
//! eight `Event_GetAndRunDialogue3` callers sat on a window that destroys them
//! too (`docs/scenes/DIALOGUE_CALLERS.md`).
//!
//! These tests pin the class, not the instances. They re-derive from the US
//! image every `jsr`/`jmp` to the five `Event_GetAndRunDialogue*` entries,
//! every entry `d0` can carry into the call (the loader just before it and
//! each `moveq`/`bra` that lands on it), and every inline `jsr
//! GetDialogueByID` a transcribed scene's bytes hold. Each caller is tied to
//! the scene whose documented range (`docs/scenes`) holds it, and
//! [`DIALOGUE_CALLERS`] must hold exactly those rows and the audit document
//! name every caller. `every_dialogue_entry_caller_runs_its_window`
//! (`dialogue_callers.rs`) then holds the typed registry to that table.
//!
//! A caller whose scene is not transcribed is named in [`UNTRANSCRIBED`], so a
//! lane that transcribes it later has to move it into the table instead of
//! dropping it out of the census. Each routine's exit is read from the image
//! too, and must be the exit of the [`DialogueWindow`] that
//! [`ROUTINE_WINDOWS`] gives it.
//!
//! The ROM is a local input: without it the image tests print a note and
//! pass, like the repository's other ROM-reading tests. The controls at the
//! end show each leg reports its own defect.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use super::dialogue_callers::{
    D1, D2, D3, D4, D5, DIALOGUE_CALLERS, DYN, INLINE, R1, R2, R3, R4, R5, RESUME, ROUTINE_WINDOWS,
    Row, routine_name,
};
use super::documented_ranges::{Ranges, documented_ranges, repo};
use crate::scene::DialogueWindow;

/// The five entries, each opening with `jsr GetDialogueByID`
/// (`ps4.asm:121603..121718`), in image order.
pub(super) const ROUTINES: [u32; 5] = [D1, D2, D3, D4, D5];
/// The five `Event_RunDialogue*` resume entries, each its `Get` routine's
/// body after the opening `jsr GetDialogueByID` (`ps4.asm:121603..121718`).
pub(super) const RESUMES: [u32; 5] = [R1, R2, R3, R4, R5];
/// The routines themselves, through `Panel_DestroyAll` (`$5AE74`): their own
/// `jsr GetDialogueByID` are not callers.
const ROUTINE_SPAN: (usize, usize) = (0x5AC66, 0x5AE74);

const JSR: u16 = 0x4EB9;
const JMP: u16 = 0x4EF9;

/// The six bytes of `jsr`/`jmp (target).l`.
const fn call(opcode: u16, target: u32) -> [u8; 6] {
    let o = opcode.to_be_bytes();
    let t = target.to_be_bytes();
    [o[0], o[1], t[0], t[1], t[2], t[3]]
}

pub(super) const GET_DIALOGUE_BY_ID: [u8; 6] = call(JSR, INLINE);
/// `popdlg` (`ps4.macrosetup.asm`, compressed dialogue): `moveq #-1, d0` /
/// `move.w Saved_Dialogue_Addr, d0` / `movea.l d0, a0`.
const POPDLG: [u8; 8] = [0x70, 0xFF, 0x30, 0x38, 0xEC, 0xF0, 0x20, 0x40];
/// How far after a `popdlg` the call that runs the text it reopened sits.
const POPDLG_REACH: usize = 0x50;
const PANEL_DESTROY_ALL: [u8; 6] = call(JSR, 0x5AE74);
const WINDOW_DESTROY: [u8; 6] = call(JSR, 0x687CC);
const PAL_FADE_OUT: [u8; 6] = call(JSR, 0x4223E);
const RUN_TEXT: [u8; 6] = call(JSR, 0x6A090);
const RUN_TEXT3: [u8; 6] = call(JSR, 0x6AD00);
const MAP_LOAD_CHUNKS: [u8; 6] = call(JMP, 0x51ADC);
/// `moveq #0, d0` / `move.b d0, Panel_Num`, `Windows_Opened_Num`, `$ECA4` /
/// `rts`.
const FORGET_WINDOWS: [u8; 16] = [
    0x70, 0x00, 0x11, 0xC0, 0xED, 0x02, 0x11, 0xC0, 0xEC, 0x9A, 0x11, 0xC0, 0xEC, 0xA4, 0x4E, 0x75,
];

/// A `jsr`/`jmp` or byte run a routine's body must (`true`) or must not
/// (`false`) contain.
type Signature = &'static [(&'static str, &'static [u8], bool)];

/// What tells the exits apart, read from the image (`$5AC66..$5AE73`): the
/// body of the routine a window stands for. Only the five `Get` windows have
/// one; `Cutscene5` and `EndingIntro` belong to no `Get` entry.
fn exit_signature(window: DialogueWindow) -> Option<Signature> {
    const PANELS: &str = "jsr Panel_DestroyAll";
    const WINDOWS: &str = "jsr Window_Destroy";
    const FADE: &str = "jsr PalFadeOut_ClrSpriteTbl";
    const TEXT: &str = "jsr RunText";
    const TEXT3: &str = "jsr RunText3";
    const CHUNKS: &str = "jmp Map_LoadChunks";
    const FORGET: &str = "the zeroed window counts";
    Some(match window {
        DialogueWindow::Standard => &[
            (PANELS, &PANEL_DESTROY_ALL, true),
            (WINDOWS, &WINDOW_DESTROY, true),
            (FADE, &PAL_FADE_OUT, false),
            (TEXT, &RUN_TEXT, true),
            (CHUNKS, &MAP_LOAD_CHUNKS, true),
            (FORGET, &FORGET_WINDOWS, false),
        ],
        DialogueWindow::Retained => &[
            (PANELS, &PANEL_DESTROY_ALL, false),
            (WINDOWS, &WINDOW_DESTROY, false),
            (FADE, &PAL_FADE_OUT, false),
            (TEXT, &RUN_TEXT, true),
            (CHUNKS, &MAP_LOAD_CHUNKS, false),
            (FORGET, &FORGET_WINDOWS, true),
        ],
        DialogueWindow::Cutscene3 => &[
            (PANELS, &PANEL_DESTROY_ALL, false),
            (WINDOWS, &WINDOW_DESTROY, true),
            (FADE, &PAL_FADE_OUT, false),
            (TEXT, &RUN_TEXT, true),
            (CHUNKS, &MAP_LOAD_CHUNKS, true),
            (FORGET, &FORGET_WINDOWS, false),
        ],
        DialogueWindow::Ending => &[
            (PANELS, &PANEL_DESTROY_ALL, false),
            (WINDOWS, &WINDOW_DESTROY, true),
            (FADE, &PAL_FADE_OUT, false),
            (TEXT3, &RUN_TEXT3, true),
            (TEXT, &RUN_TEXT, false),
            (CHUNKS, &MAP_LOAD_CHUNKS, true),
        ],
        DialogueWindow::Cutscene => &[
            (PANELS, &PANEL_DESTROY_ALL, true),
            (WINDOWS, &WINDOW_DESTROY, true),
            (FADE, &PAL_FADE_OUT, true),
            (TEXT, &RUN_TEXT, true),
            (CHUNKS, &MAP_LOAD_CHUNKS, true),
            (FORGET, &FORGET_WINDOWS, false),
        ],
        _ => return None,
    })
}

/// Instructions seen between a `d0` loader and its call that leave `d0`
/// alone: `move.b #1, Render_Sprites_In_Cutscenes` and `clr.b` of it.
const D0_SAFE_GAPS: [&[u8]; 3] = [
    &[],
    &[0x11, 0xFC, 0x00, 0x01, 0xEC, 0xFD],
    &[0x42, 0x38, 0xEC, 0xFD],
];

/// A caller no documented scene holds: the routine, the call address, the
/// event label and its `EventPtrs` index, the entries the image gives `d0`,
/// and the owning issue. The spans are in `docs/scenes/DIALOGUE_CALLERS.md`.
type Untranscribed = (u32, u32, &'static str, u16, &'static [u16], u32);

const UNTRANSCRIBED: &[Untranscribed] = &[];

/// `(call address, routine)` -> the entries `d0` carries into the call.
type Callers = BTreeMap<(usize, u32), BTreeSet<u16>>;

fn word(rom: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([rom[at], rom[at + 1]])
}

/// `(entry, length)` of a `d0` literal load at `at`, if there is one:
/// `moveq #n, d0`, or `move.b`/`move.w #n, d0` with `n` a byte.
fn literal(rom: &[u8], at: usize) -> Option<(u16, usize)> {
    let w = word(rom, at);
    if (0x7000..=0x707F).contains(&w) {
        return Some((w & 0xFF, 2));
    }
    if (w == 0x103C || w == 0x303C) && word(rom, at + 2) <= 0xFF {
        return Some((word(rom, at + 2), 4));
    }
    None
}

/// Where a `bra.s`/`bra.w` at `at` goes.
fn branch_target(rom: &[u8], at: usize) -> Option<usize> {
    let w = word(rom, at);
    let next = at as i64 + 2;
    let target = if w == 0x6000 {
        next + i64::from(word(rom, at + 2) as i16)
    } else if w & 0xFF00 == 0x6000 && w & 0xFF != 0xFF {
        next + i64::from((w & 0xFF) as u8 as i8)
    } else {
        return None;
    };
    usize::try_from(target).ok()
}

/// Every literal entry `d0` carries into the call at `at`, or `{DYN}`.
///
/// The fall-through loader is a `moveq`/`move.b`/`move.w #n, d0` ending right
/// before the call, or before one of [`D0_SAFE_GAPS`]. Each branch loader is
/// such a load followed at once by a `bra` that lands on the call
/// (`Event_RajaSick`'s three `moveq`/`bra.s loc_70C72`). A call with no
/// literal path is `DYN`: `d0` comes from RAM or a table.
fn d0_entries(rom: &[u8], at: usize) -> BTreeSet<u16> {
    let mut found = BTreeSet::new();
    for gap in D0_SAFE_GAPS {
        let end = at - gap.len();
        if &rom[end..at] != gap {
            continue;
        }
        for length in [2, 4] {
            if let Some((entry, len)) = literal(rom, end - length)
                && len == length
            {
                found.insert(entry);
                break;
            }
        }
        if !found.is_empty() {
            break;
        }
    }
    for start in (at - 0x100..at).step_by(2) {
        if let Some((entry, len)) = literal(rom, start)
            && branch_target(rom, start + len) == Some(at)
        {
            found.insert(entry);
        }
    }
    if found.is_empty() {
        found.insert(DYN);
    }
    found
}

/// Every even `jsr`/`jmp` to the five routines and every even `jsr
/// GetDialogueByID` outside them, with its entries. The census keeps the
/// inline callers only inside a scene's documented bytes.
fn rom_callers(rom: &[u8]) -> Callers {
    let mut found = Callers::new();
    for at in (0..rom.len().saturating_sub(5)).step_by(2) {
        let opcode = word(rom, at);
        if opcode != JSR && opcode != JMP {
            continue;
        }
        if (ROUTINE_SPAN.0..ROUTINE_SPAN.1).contains(&at) {
            continue;
        }
        let target = u32::from_be_bytes([rom[at + 2], rom[at + 3], rom[at + 4], rom[at + 5]]);
        if ROUTINES.contains(&target) || (target == INLINE && opcode == JSR) {
            found.insert((at, target), d0_entries(rom, at));
        } else if RESUMES.contains(&target) {
            found.insert((at, target), BTreeSet::from([RESUME]));
        }
    }
    for at in inline_resumes(rom) {
        found.insert((at, INLINE), BTreeSet::from([RESUME]));
    }
    found
}

/// Every `popdlg` whose reopened text the scene's own window code runs (`jsr
/// RunText`/`RunText3` before any `Event_RunDialogue*` call): an inline
/// resume, keyed by the `popdlg`. A `popdlg` followed by neither only peeks
/// at the saved address (`Event_MeetingDorin`'s `cmpi.b #$FF, -1(a0)`).
fn inline_resumes(rom: &[u8]) -> Vec<usize> {
    let resume_calls: Vec<[u8; 6]> = RESUMES
        .iter()
        .flat_map(|r| [call(JSR, *r), call(JMP, *r)])
        .collect();
    (0..rom.len().saturating_sub(POPDLG.len()))
        .step_by(2)
        .filter(|&at| rom[at..at + POPDLG.len()] == POPDLG)
        .filter(|&at| {
            let end = (at + POPDLG_REACH).min(rom.len() - 6);
            let calls = (at + POPDLG.len()..end).step_by(2).map(|j| &rom[j..j + 6]);
            calls
                .map(|w| {
                    if resume_calls.iter().any(|c| c == w) {
                        Some(false)
                    } else if w == RUN_TEXT || w == RUN_TEXT3 {
                        Some(true)
                    } else {
                        None
                    }
                })
                .find_map(|kind| kind)
                .unwrap_or(false)
        })
        .collect()
}

/// One routine's bytes, from its entry to the next routine (or `$5AE74`).
fn routine_body(rom: &[u8], routine: u32) -> &[u8] {
    let start = routine as usize;
    let end = ROUTINES
        .iter()
        .map(|r| *r as usize)
        .filter(|r| *r > start)
        .min()
        .unwrap_or(ROUTINE_SPAN.1);
    &rom[start..end]
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// Every routine whose exit is not the exit of the window `windows` gives it.
fn exit_problems(rom: &[u8], windows: &[(u32, DialogueWindow)]) -> Vec<String> {
    let mut problems = Vec::new();
    for routine in ROUTINES {
        let label = routine_name(routine);
        let Some(&(_, window)) = windows.iter().find(|(r, _)| *r == routine) else {
            problems.push(format!("{label}: ROUTINE_WINDOWS gives it no window"));
            continue;
        };
        let body = routine_body(rom, routine);
        if !body.starts_with(&GET_DIALOGUE_BY_ID) {
            problems.push(format!("{label}: does not open with jsr GetDialogueByID"));
        }
        let Some(signature) = exit_signature(window) else {
            problems.push(format!(
                "{label}: ROUTINE_WINDOWS gives it DialogueWindow::{window:?}, which no Get \
                 routine's exit is"
            ));
            continue;
        };
        for (name, needle, wanted) in signature {
            if contains(body, needle) != *wanted {
                problems.push(format!(
                    "{label}: {} {name}, so its exit is not what DialogueWindow::{window:?} \
                     stands for",
                    if *wanted { "lacks" } else { "has" }
                ));
            }
        }
    }
    problems
}

/// The name a [`DIALOGUE_CALLERS`] row gives a routine.
fn routine_const(routine: u32) -> &'static str {
    match routine {
        D1 => "D1",
        D2 => "D2",
        D3 => "D3",
        D4 => "D4",
        D5 => "D5",
        R1 => "R1",
        R2 => "R2",
        R3 => "R3",
        R4 => "R4",
        R5 => "R5",
        _ => "INLINE",
    }
}

fn entry_text(entry: u16) -> String {
    if entry == DYN {
        "DYN".into()
    } else if entry == RESUME {
        "RESUME".into()
    } else {
        format!("0x{entry:02X}")
    }
}

fn entries_text(entries: &BTreeSet<u16>) -> String {
    let shown: Vec<String> = entries.iter().map(|e| entry_text(*e)).collect();
    format!("[{}]", shown.join(", "))
}

type OwnedRow = (String, u32, u32, u16);

fn row_text(row: &OwnedRow) -> String {
    format!(
        "(\"{}\", {}, 0x{:06X}, {}),",
        row.0,
        routine_const(row.1),
        row.2,
        entry_text(row.3)
    )
}

/// The documents and table a census reads.
struct Inputs<'a> {
    rom: &'a [u8],
    ranges: &'a Ranges,
    table: &'a [Row],
    doc: &'a str,
    untranscribed: &'a [Untranscribed],
}

/// Everything wrong between the image's callers, the documented ranges,
/// [`DIALOGUE_CALLERS`], [`UNTRANSCRIBED`] and the audit document.
fn census(inputs: &Inputs) -> Vec<String> {
    let mut problems = Vec::new();
    let mut expected: BTreeSet<OwnedRow> = BTreeSet::new();
    let callers = rom_callers(inputs.rom);
    for (&(at, routine), entries) in &callers {
        let names: BTreeSet<&str> = inputs
            .ranges
            .iter()
            .filter(|(_, spans)| spans.iter().any(|(s, e)| (*s..*e).contains(&at)))
            .map(|(name, _)| name.as_str())
            .collect();
        let label = routine_name(routine);
        let listed = inputs.doc.contains(&format!("${at:06X}"));
        if names.len() > 1 {
            problems.push(format!(
                "${at:06X} {label}: documented ranges of {names:?} overlap"
            ));
            continue;
        }
        let held = inputs
            .untranscribed
            .iter()
            .find(|u| u.0 == routine && u.1 as usize == at);
        let Some(scene) = names.first() else {
            if routine == INLINE {
                continue;
            }
            match held {
                None => problems.push(format!(
                    "${at:06X} {label} (entries {}): no documented scene holds this caller and \
                     it is not in UNTRANSCRIBED; transcribe its scene or add it to \
                     UNTRANSCRIBED and the uncovered-callers table of \
                     docs/scenes/DIALOGUE_CALLERS.md",
                    entries_text(entries)
                )),
                Some(held) => {
                    let want: BTreeSet<u16> = held.4.iter().copied().collect();
                    if &want != entries {
                        problems.push(format!(
                            "${at:06X} {}: image entries {}, UNTRANSCRIBED expects {}",
                            held.2,
                            entries_text(entries),
                            entries_text(&want)
                        ));
                    }
                }
            }
            if !listed {
                problems.push(format!(
                    "${at:06X} {label}: missing from DIALOGUE_CALLERS.md"
                ));
            }
            continue;
        };
        if held.is_some() {
            problems.push(format!(
                "${at:06X} {label}: {scene}'s documented range now holds this caller; remove \
                 it from UNTRANSCRIBED and add its rows to DIALOGUE_CALLERS"
            ));
        }
        for entry in entries {
            expected.insert(((*scene).to_owned(), routine, at as u32, *entry));
        }
        if !listed {
            problems.push(format!(
                "${at:06X} {scene} {label}: missing from DIALOGUE_CALLERS.md; add it to \
                 {scene}'s row of the census table"
            ));
        }
    }
    for u in inputs.untranscribed {
        if !callers.contains_key(&(u.1 as usize, u.0)) {
            problems.push(format!(
                "UNTRANSCRIBED lists ${:06X}, which the image does not call",
                u.1
            ));
        }
    }
    let table: BTreeSet<OwnedRow> = inputs
        .table
        .iter()
        .map(|r| (r.0.to_owned(), r.1, r.2, r.3))
        .collect();
    let mut missing: Vec<&OwnedRow> = expected.difference(&table).collect();
    missing.sort_by_key(|r| (r.2, r.3));
    let mut extra: Vec<&OwnedRow> = table.difference(&expected).collect();
    extra.sort_by_key(|r| (r.2, r.3));
    for row in missing {
        problems.push(format!(
            "${:06X} {}: the image calls {} here; add `{}` to DIALOGUE_CALLERS in \
             rust/psiv-core/src/scenes/dialogue_callers.rs",
            row.2,
            row.0,
            routine_const(row.1),
            row_text(row)
        ));
    }
    for row in extra {
        problems.push(format!(
            "${:06X} {}: DIALOGUE_CALLERS has `{}`, which the image and the documented ranges \
             do not give; remove or correct it",
            row.2,
            row.0,
            row_text(row)
        ));
    }
    problems
}

fn audit_doc() -> String {
    std::fs::read_to_string(repo().join("docs/scenes/DIALOGUE_CALLERS.md")).unwrap()
}

/// The US image, or `None` (with a note) when the local input is absent.
pub(super) fn rom() -> Option<&'static [u8]> {
    static ROM: OnceLock<Option<Vec<u8>>> = OnceLock::new();
    let rom = ROM.get_or_init(|| {
        let path = repo().join("Phantasy Star IV (USA).md");
        std::fs::read(&path).ok()
    });
    if rom.is_none() {
        eprintln!("ROM not present at the repository root; skipping");
    }
    rom.as_deref()
}

/// The image, the documents and the tables as the repository has them.
struct Fixture {
    rom: &'static [u8],
    ranges: Ranges,
    doc: String,
}

impl Fixture {
    fn load() -> Option<Self> {
        Some(Self {
            rom: rom()?,
            ranges: documented_ranges(),
            doc: audit_doc(),
        })
    }

    fn inputs(&self) -> Inputs<'_> {
        Inputs {
            rom: self.rom,
            ranges: &self.ranges,
            table: DIALOGUE_CALLERS,
            doc: &self.doc,
            untranscribed: UNTRANSCRIBED,
        }
    }
}

#[test]
fn every_caller_in_the_image_is_in_the_table() {
    let Some(fixture) = Fixture::load() else {
        return;
    };
    let problems = census(&fixture.inputs());
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn each_routine_exit_is_its_window() {
    let Some(rom) = rom() else {
        return;
    };
    let problems = exit_problems(rom, ROUTINE_WINDOWS);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn the_image_caller_counts() {
    // Counts from a plain byte search over every even offset, independent of
    // `rom_callers`' decoding.
    let Some(rom) = rom() else {
        return;
    };
    let mut counts = BTreeMap::new();
    for routine in ROUTINES.into_iter().chain(RESUMES) {
        let mut n = 0;
        for opcode in [JSR, JMP] {
            let needle = call(opcode, routine);
            n += rom
                .windows(6)
                .enumerate()
                .filter(|(at, w)| at % 2 == 0 && *w == needle)
                .count();
        }
        counts.insert(routine_const(routine), n);
    }
    let want = BTreeMap::from([
        ("D1", 155),
        ("D2", 14),
        ("D3", 8),
        ("D4", 3),
        ("D5", 23),
        ("R1", 76),
        ("R2", 4),
        ("R3", 7),
        ("R4", 22),
        ("R5", 11),
    ]);
    assert_eq!(counts, want);
    let mut decoded = BTreeMap::new();
    for (_, routine) in rom_callers(rom).keys() {
        if *routine != INLINE {
            *decoded.entry(routine_const(*routine)).or_insert(0) += 1;
        }
    }
    assert_eq!(decoded, want);
}

#[test]
fn zio_nurvus_is_the_scene_h22_was_found_in() {
    // Guards the guard: the census must cover the H22 scene itself.
    let Some(rom) = rom() else {
        return;
    };
    assert_eq!(rom_callers(rom)[&(0x06F3D4, D2)], BTreeSet::from([0x0B]));
    assert!(DIALOGUE_CALLERS.contains(&("Event_ZioNurvus", D2, 0x06F3D4, 0x0B)));
}

#[test]
fn branch_loaded_entries_are_read() {
    // `Event_RajaSick` (`ps4.asm:150224-150250`): `$5D`, `$5E`, `$5F` reach
    // the `jmp` at `$070C72`, three of them by `bra.s`.
    let Some(rom) = rom() else {
        return;
    };
    assert_eq!(
        rom_callers(rom)[&(0x070C72, D1)],
        BTreeSet::from([0x5D, 0x5E, 0x5F])
    );
}

// Controls: each leg of the census fails on its own defect.

fn census_with<'a>(fixture: &'a Fixture, edit: impl FnOnce(&mut Inputs<'a>)) -> Vec<String> {
    let mut inputs = fixture.inputs();
    edit(&mut inputs);
    census(&inputs)
}

fn patched(rom: &[u8], at: usize, before: &[u8], after: &[u8]) -> Vec<u8> {
    assert_eq!(
        &rom[at..at + before.len()],
        before,
        "the image at ${at:06X}"
    );
    let mut patched = rom.to_vec();
    patched[at..at + after.len()].copy_from_slice(after);
    assert_eq!(patched.len(), rom.len());
    patched
}

#[test]
fn control_a_new_image_caller_is_reported() {
    let Some(fixture) = Fixture::load() else {
        return;
    };
    // `moveq #1, d0 / jsr Event_GetAndRunDialogue3` over the `$FF` fill at
    // the image's end, which nothing runs.
    let at = 0x2FFFC0;
    let mut code = vec![0x70, 0x01];
    code.extend(call(JSR, D3));
    let rom = patched(fixture.rom, at, &[0xFF; 8], &code);
    let problems = census_with(&fixture, |i| i.rom = &rom);
    assert!(
        problems.iter().any(|p| p
            .starts_with(&format!("${:06X} Event_GetAndRunDialogue3", at + 2))
            && p.contains("UNTRANSCRIBED")),
        "{problems:#?}"
    );
}

#[test]
fn control_a_new_caller_in_a_scene_says_which_row_to_add() {
    let Some(fixture) = Fixture::load() else {
        return;
    };
    // MeetingKyra's `jsr Event_GetAndRunDialogue3` at `$0777D8` retargeted to
    // `Event_GetAndRunDialogue5`: the row it needs is a D5 row.
    let rom = patched(fixture.rom, 0x0777D8, &call(JSR, D3), &call(JSR, D5));
    let problems = census_with(&fixture, |i| i.rom = &rom);
    assert!(
        problems.contains(
            &"$0777D8 Cutscene_MeetingKyra: the image calls D5 here; add \
              `(\"Cutscene_MeetingKyra\", D5, 0x0777D8, 0x28),` to DIALOGUE_CALLERS in \
              rust/psiv-core/src/scenes/dialogue_callers.rs"
                .to_owned()
        ),
        "{problems:#?}"
    );
}

#[test]
fn control_a_changed_image_entry_is_reported() {
    let Some(fixture) = Fixture::load() else {
        return;
    };
    let rom = patched(fixture.rom, 0x06F3D2, &[0x70, 0x0B], &[0x70, 0x0C]);
    let problems = census_with(&fixture, |i| i.rom = &rom);
    let reported = |a: &str, b: &str| problems.iter().any(|p| p.contains(a) && p.contains(b));
    assert!(reported("Event_ZioNurvus", "0x0C"), "{problems:#?}");
    assert!(reported("Event_ZioNurvus", "remove"), "{problems:#?}");
}

#[test]
fn control_a_lost_branch_entry_is_reported() {
    let Some(fixture) = Fixture::load() else {
        return;
    };
    // `moveq #$5D, d0 / bra.s loc_70C72` at `$070C68`: point the `bra.s` one
    // word short and `$5D` no longer reaches the call.
    let rom = patched(
        fixture.rom,
        0x070C68,
        &[0x70, 0x5D, 0x60, 0x06],
        &[0x70, 0x5D, 0x60, 0x04],
    );
    let problems = census_with(&fixture, |i| i.rom = &rom);
    assert!(
        problems.iter().any(
            |p| p.starts_with("$070C72 Event_RajaSick: DIALOGUE_CALLERS has") && p.contains("0x5D")
        ),
        "{problems:#?}"
    );
}

#[test]
fn control_a_dropped_table_row_is_reported() {
    let Some(fixture) = Fixture::load() else {
        return;
    };
    let table: Vec<Row> = DIALOGUE_CALLERS
        .iter()
        .copied()
        .filter(|row| row.0 != "Event_Juza")
        .collect();
    assert!(table.len() < DIALOGUE_CALLERS.len());
    let problems = census_with(&fixture, |i| i.table = &table);
    assert!(
        problems
            .iter()
            .any(|p| p.starts_with("$06FB42 Event_Juza: the image calls D2")),
        "{problems:#?}"
    );
}

#[test]
fn control_a_row_under_the_wrong_routine_is_reported() {
    let Some(fixture) = Fixture::load() else {
        return;
    };
    let row = ("Cutscene_DemiRescue", D3, 0x074A92, 0x42);
    assert!(DIALOGUE_CALLERS.contains(&row));
    let table: Vec<Row> = DIALOGUE_CALLERS
        .iter()
        .map(|r| {
            if *r == row {
                (row.0, D5, row.2, row.3)
            } else {
                *r
            }
        })
        .collect();
    let problems = census_with(&fixture, |i| i.table = &table);
    assert_eq!(
        problems.iter().filter(|p| p.starts_with("$074A92")).count(),
        2,
        "{problems:#?}"
    );
}

#[test]
fn control_a_caller_claimed_by_two_scenes_is_reported() {
    let Some(fixture) = Fixture::load() else {
        return;
    };
    let mut table = DIALOGUE_CALLERS.to_vec();
    table.push(("Event_DarkForce2", D2, 0x06F3D4, 0x0B));
    let problems = census_with(&fixture, |i| i.table = &table);
    assert!(
        problems
            .iter()
            .any(|p| p.starts_with("$06F3D4 Event_DarkForce2: DIALOGUE_CALLERS has")),
        "{problems:#?}"
    );
}

#[test]
fn control_overlapping_ranges_are_reported() {
    let Some(fixture) = Fixture::load() else {
        return;
    };
    let mut ranges = fixture.ranges.clone();
    let kyra = ranges["Cutscene_MeetingKyra"].clone();
    ranges.get_mut("Event_Burstroc").unwrap().extend(kyra);
    let problems = census_with(&fixture, |i| i.ranges = &ranges);
    assert!(
        problems
            .iter()
            .any(|p| p.contains("overlap") && p.contains("$0777A6")),
        "{problems:#?}"
    );
}

#[test]
fn control_a_missing_audit_row_is_reported() {
    let Some(fixture) = Fixture::load() else {
        return;
    };
    let doc = fixture.doc.replace("$06F3D4", "$------");
    let problems = census_with(&fixture, |i| i.doc = &doc);
    assert!(
        problems
            .iter()
            .any(|p| p.starts_with("$06F3D4 Event_ZioNurvus Event_GetAndRunDialogue2: missing")),
        "{problems:#?}"
    );
    let doc = fixture.doc.replace("$072DDE", "$------");
    let problems = census_with(&fixture, |i| i.doc = &doc);
    assert!(
        problems.iter().any(|p| p.starts_with(
            "$072DDE Event_DyingBoy Event_GetAndRunDialogue: missing from DIALOGUE_CALLERS.md"
        )),
        "{problems:#?}"
    );
}

#[test]
fn control_a_transcribed_caller_left_in_untranscribed_is_reported() {
    let Some(fixture) = Fixture::load() else {
        return;
    };
    let mut untranscribed = UNTRANSCRIBED.to_vec();
    untranscribed.push((D2, 0x06FB42, "Event_Juza", 0x40, &[0x48], 0));
    let problems = census_with(&fixture, |i| i.untranscribed = &untranscribed);
    assert!(
        problems
            .iter()
            .any(|p| p.contains("remove it from UNTRANSCRIBED")),
        "{problems:#?}"
    );
}

#[test]
fn control_a_stale_untranscribed_entry_is_reported() {
    let Some(fixture) = Fixture::load() else {
        return;
    };
    // Every caller has a scene now, so take the dying boy's documented range
    // away and list its first caller with an entry the image does not give.
    let mut ranges = fixture.ranges.clone();
    ranges.remove("Event_DyingBoy");
    let untranscribed: Vec<Untranscribed> =
        vec![(D1, 0x072DDE, "Event_DyingBoy", 0x86, &[0x32], 283)];
    let problems = census_with(&fixture, |i| {
        i.ranges = &ranges;
        i.untranscribed = &untranscribed;
    });
    assert!(
        problems
            .iter()
            .any(|p| p.contains("UNTRANSCRIBED expects [0x32]")),
        "{problems:#?}"
    );
}

#[test]
fn control_an_untranscribed_entry_the_image_does_not_call_is_reported() {
    let Some(fixture) = Fixture::load() else {
        return;
    };
    let mut untranscribed = UNTRANSCRIBED.to_vec();
    untranscribed.push((D3, 0x072DDE, "Event_DyingBoy", 0x86, &[0x33], 283));
    let problems = census_with(&fixture, |i| i.untranscribed = &untranscribed);
    assert!(
        problems.contains(&"UNTRANSCRIBED lists $072DDE, which the image does not call".to_owned()),
        "{problems:#?}"
    );
}

#[test]
fn control_a_changed_exit_is_reported() {
    let Some(rom) = rom() else {
        return;
    };
    // Give `Event_GetAndRunDialogue3` the `Panel_DestroyAll` its siblings
    // have, over its `bclr #0, $ECA4` (`$5AD78`): then it is no longer the
    // window that keeps the panels.
    let rom = patched(
        rom,
        0x5AD78,
        &[0x08, 0xB8, 0x00, 0x00, 0xEC, 0xA4],
        &PANEL_DESTROY_ALL,
    );
    let problems = exit_problems(&rom, ROUTINE_WINDOWS);
    assert_eq!(
        problems,
        vec![
            "Event_GetAndRunDialogue3: has jsr Panel_DestroyAll, so its exit is not what \
             DialogueWindow::Cutscene3 stands for"
        ]
    );
}

#[test]
fn control_a_window_mapping_change_is_reported() {
    let Some(rom) = rom() else {
        return;
    };
    let swapped = |to: DialogueWindow| -> Vec<(u32, DialogueWindow)> {
        ROUTINE_WINDOWS
            .iter()
            .map(|&(r, w)| if r == D3 { (r, to) } else { (r, w) })
            .collect()
    };
    let problems = exit_problems(rom, &swapped(DialogueWindow::Cutscene));
    assert!(
        problems
            .iter()
            .any(|p| p.starts_with("Event_GetAndRunDialogue3: lacks"))
            && problems
                .iter()
                .all(|p| p.starts_with("Event_GetAndRunDialogue3")),
        "{problems:#?}"
    );
    let problems = exit_problems(rom, &swapped(DialogueWindow::Cutscene5));
    assert_eq!(
        problems,
        vec![
            "Event_GetAndRunDialogue3: ROUTINE_WINDOWS gives it DialogueWindow::Cutscene5, \
             which no Get routine's exit is"
        ]
    );
    let dropped: Vec<_> = ROUTINE_WINDOWS
        .iter()
        .copied()
        .filter(|(r, _)| *r != D4)
        .collect();
    assert_eq!(
        exit_problems(rom, &dropped),
        vec!["Event_GetAndRunDialogue4: ROUTINE_WINDOWS gives it no window"]
    );
}
