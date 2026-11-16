//! Which one-frame-a-pass palette fades each scene's retail bytes call (#637).
//!
//! Three library routines fade the whole palette one pass a frame, and each
//! has its own op, which the runtime's CRAM model runs with its frames:
//!
//! | Routine | Op |
//! | --- | --- |
//! | `InitVRAMAndCRAM` (`$05A658`) | [`SceneOp::InitVramAndCram`] |
//! | `Pal_FadeIn` (`$0421D4`) | [`SceneOp::FadeIn`] |
//! | `PalFadeOut_ClrSpriteTbl` (`$04223E`) | [`SceneOp::FadeOut`] |
//!
//! A scene that records one where its bytes call none spends frames and
//! draws a fade the cartridge does not, and one it leaves out the reverse.
//! The check reads every `jsr`/`jmp` to them inside a scene's documented
//! ranges off the US image, in address order, and each scene's ops of the
//! three kinds, in op order, must be those calls. It skips without the
//! image. [`KNOWN`] lists the scenes that do not match yet, with the issue
//! that fixes them; a listed scene that matches fails too, so the list only
//! shrinks. The variable fades have their own check, `palette_fades_image`.
//!
//! `Cutscene_DarkForce1Defeated` calls `loc_64C1E` at `$07722A`:
//! its two shared planet-screen fades are inserted at that call boundary,
//! between the caller's second and third Init, without expanding its range.
//! Lashiec calls that helper at `$077B68`, after its third Init/world write
//! and before Gumbious; its shared fades are inserted there in the same way.
//!
//! The ship's route is attributed as its bytes are (#654).
//! `Cutscene_LeRoofAgain` and `Cutscene_FindingAirCastle` end their bytes in
//! `jmp Cutscene_InsideSpaceship` and their ops in [`INSIDE_SPACESHIP_ROUTE`]:
//! each is checked on its ops before the route against its own bytes, and
//! must end in the route exactly when its bytes make that `jmp`. The route is
//! `Cutscene_InsideSpaceship`'s alone: its first [`ENTRY_OPS`] against its own
//! bytes, and on each way out of the menu the fades
//! [`trace`](super::ship_route_trace::trace) sees against the calls
//! `loc_63BC4` makes on that way ([`SHIP_WAYS`]), which branches, so it is
//! read by way rather than in address order.

use super::SCENES;
use super::dialogue_callers_image::rom;
use super::documented_ranges::{Ranges, documented_ranges};
use super::flight::INSIDE_SPACESHIP_ROUTE;
use super::ship_route_trace::{ENTRY_OPS, Way, before_route, fades, insert, retarget, trace};
use super::wait_census::shared_code::SHARED_CODE;
use crate::SceneOp;

/// A fixed fade routine, by its op.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Fixed {
    Init,
    In,
    Out,
}

/// The routines' US entry points.
const ROUTINES: &[(u32, Fixed)] = &[
    (0x05_A658, Fixed::Init),
    (0x04_21D4, Fixed::In),
    (0x04_223E, Fixed::Out),
];

/// `Cutscene_InsideSpaceship`, whose ops are the ship's route.
const SHIP: &str = "Cutscene_InsideSpaceship";
/// Its entry, the target of the callers' `jmp`.
const SHIP_ENTRY: u32 = 0x07_606C;

/// `loc_63BC4`'s fixed fade calls on each way out of the menu, in the order
/// they run (`ps4.asm:133499-134714`). The kinds are read off the image.
const SHIP_WAYS: [(Way, &[u32]); 3] = [
    (
        Way::Spaceport,
        &[
            0x06_3E0C, // the windows closed (:133670), then `bsr loc_64568`
            0x06_45D6, // the takeoff's fade-in (:134195)
            0x06_472C, // loc_6472C (:134294)
            0x06_4756, // :134303, then `bsr loc_64C1E`
            0x06_4C8E, // the planet screen's fade-in (:134678)
            0x06_4CFA, // its closing `jmp` (:134714)
            0x06_47B8, // the transit's fade-in (:134323)
            0x06_483A, // loc_6483A, before the landing (:134360)
        ],
    ),
    (
        Way::Elsewhere,
        &[
            0x06_3E0C, // the windows closed (:133670), then `bsr loc_6488A`
            0x06_48F8, // the takeoff's fade-in (:134404)
            0x06_49A2, // loc_649A2 (:134455)
            0x06_49CC, // :134464, then `bsr loc_64C1E`
            0x06_4C8E, // the planet screen's fade-in (:134678)
            0x06_4CFA, // its closing `jmp` (:134714)
            0x06_4A2E, // the transit's fade-in (:134484)
            0x06_4AB0, // before the landing (:134521)
        ],
    ),
    // loc_63E5E: the windows closed, then the map reloaded (:133702).
    (Way::Cancel, &[0x06_3E8A]),
];

/// Scenes whose ops do not match their calls yet, and the issue that fixes
/// each.
const KNOWN: &[(&str, u32)] = &[("Cutscene_Ending", 652)];

/// The fixed fade an op is, if it is one.
fn recorded(op: &SceneOp) -> Option<Fixed> {
    match op {
        SceneOp::InitVramAndCram => Some(Fixed::Init),
        SceneOp::FadeIn => Some(Fixed::In),
        SceneOp::FadeOut => Some(Fixed::Out),
        _ => None,
    }
}

/// The calls in `ranges` of the image, in address order: every `jsr
/// (routine).l` and `jmp (routine).l`, with its address.
fn retail_calls(rom: &[u8], ranges: &[(usize, usize)]) -> Vec<(usize, Fixed)> {
    let mut ranges = ranges.to_vec();
    ranges.sort_unstable();
    ranges.dedup();
    let mut calls = Vec::new();
    for (start, end) in ranges {
        for at in (start..end.min(rom.len().saturating_sub(5))).step_by(2) {
            let bytes = &rom[at..at + 6];
            if bytes[..2] == [0x4E, 0xB9] || bytes[..2] == [0x4E, 0xF9] {
                let target = u32::from_be_bytes(bytes[2..].try_into().unwrap());
                if let Some(&(_, fixed)) = ROUTINES.iter().find(|(addr, _)| *addr == target) {
                    calls.push((at, fixed));
                }
            }
        }
    }
    calls
}

/// Whether `ranges` hold a `jmp` to the ship's entry.
fn jumps_to_ship(rom: &[u8], ranges: &[(usize, usize)]) -> bool {
    let jmp = [&[0x4E, 0xF9][..], &SHIP_ENTRY.to_be_bytes()].concat();
    ranges.iter().any(|&(start, end)| {
        (start..end.min(rom.len().saturating_sub(5)))
            .step_by(2)
            .any(|at| rom[at..at + 6] == jmp[..])
    })
}

/// `loc_63BC4`'s span (`wait_census`'s shared code for the ship).
fn ship_helper() -> (usize, usize) {
    let &(_, start, end) = SHARED_CODE
        .iter()
        .find(|(scene, ..)| *scene == SHIP)
        .expect("the ship's helper is shared code");
    (start as usize, end as usize)
}

/// The kinds of the calls at `addresses`, `None` where none is.
fn kinds(rom: &[u8], addresses: &[u32]) -> Vec<Option<Fixed>> {
    let calls = retail_calls(rom, &[ship_helper()]);
    addresses
        .iter()
        .map(|&at| {
            calls
                .iter()
                .find(|(call, _)| *call == at as usize)
                .map(|(_, fixed)| *fixed)
        })
        .collect()
}

/// What is wrong with [`SHIP_WAYS`]: an address with no call in the helper,
/// and a call in the helper on no way.
fn ship_way_problems(rom: &[u8], ways: &[(Way, &[u32])]) -> Vec<String> {
    let mut problems = Vec::new();
    for &(way, addresses) in ways {
        for (at, kind) in addresses.iter().zip(kinds(rom, addresses)) {
            if kind.is_none() {
                problems.push(format!("{way:?}: no fixed fade call at ${at:06X}"));
            }
        }
    }
    for (at, fixed) in retail_calls(rom, &[ship_helper()]) {
        if !ways
            .iter()
            .any(|(_, addresses)| addresses.contains(&(at as u32)))
        {
            problems.push(format!("${at:06X} {fixed:?} is on no way out of the menu"));
        }
    }
    problems
}

/// The fixed fades of `ops`, in op order.
fn native(ops: &[SceneOp]) -> Vec<Fixed> {
    ops.iter().filter_map(recorded).collect()
}

/// `calls` and `native` side by side, when they differ.
fn compare(name: &str, calls: &[(usize, Fixed)], native: &[Fixed]) -> Option<String> {
    if calls
        .iter()
        .map(|(_, fixed)| *fixed)
        .eq(native.iter().copied())
    {
        return None;
    }
    let at: Vec<String> = calls
        .iter()
        .map(|(at, fixed)| format!("${at:06X} {fixed:?}"))
        .collect();
    Some(format!(
        "{name}: retail calls [{}], the scene records {native:?}",
        at.join(", ")
    ))
}

/// Each scene with documented ranges whose ops are not its calls, by name.
fn mismatches(rom: &[u8], ranges: &Ranges, scenes: &[crate::Scene]) -> Vec<(&'static str, String)> {
    let mut problems = Vec::new();
    for scene in scenes {
        let Some(ranges) = ranges.get(scene.name) else {
            continue;
        };
        let mut calls = retail_calls(rom, ranges);
        let planet_call = match scene.name {
            "Cutscene_DarkForce1Defeated" => Some(0x07722A),
            "Cutscene_LashiecDefeated" => Some(0x077B68),
            _ => None,
        };
        if let Some(planet_call) = planet_call {
            // The caller owns Init; loc_64C1E owns its In/Out pair.
            // Count the helper's calls at that boundary, not by ROM address.
            let at = calls.partition_point(|(at, _)| *at < planet_call);
            calls.splice(at..at, retail_calls(rom, &[(0x064C1E, 0x064D00)]));
        }
        let mut found = Vec::new();
        if scene.name == SHIP {
            // The entry's own bytes, then the helper by way.
            let entry = &scene.ops[..ENTRY_OPS.min(scene.ops.len())];
            found.extend(compare(scene.name, &calls, &native(entry)));
            for (way, addresses) in SHIP_WAYS {
                let mut retail = calls.clone();
                for (&at, kind) in addresses.iter().zip(kinds(rom, addresses)) {
                    retail.extend(kind.map(|kind| (at as usize, kind)));
                }
                let name = format!("{} {way:?}", scene.name);
                found.extend(compare(&name, &retail, &fades(&trace(scene.ops, way))));
            }
        } else {
            let own = before_route(scene.ops);
            found.extend(compare(
                scene.name,
                &calls,
                &native(own.unwrap_or(scene.ops)),
            ));
            match (jumps_to_ship(rom, ranges), own.is_some()) {
                (true, false) => found.push(format!(
                    "{}: its bytes jump to {SHIP} but its ops do not end in the route",
                    scene.name
                )),
                (false, true) => found.push(format!(
                    "{}: its ops end in the route but its bytes do not jump to {SHIP}",
                    scene.name
                )),
                _ => {}
            }
        }
        if !found.is_empty() {
            problems.push((scene.name, found.join("\n")));
        }
    }
    problems
}

/// Every mismatch `known` does not list, and every scene `known` lists that
/// matches.
fn report(
    rom: &[u8],
    ranges: &Ranges,
    scenes: &[crate::Scene],
    known: &[(&str, u32)],
) -> Vec<String> {
    let found = mismatches(rom, ranges, scenes);
    let mut problems: Vec<String> = found
        .iter()
        .filter(|(name, _)| !known.iter().any(|(known, _)| known == name))
        .map(|(_, problem)| problem.clone())
        .collect();
    for (known, issue) in known {
        if scenes.iter().any(|scene| scene.name == *known)
            && !found.iter().any(|(name, _)| name == known)
        {
            problems.push(format!(
                "{known} now records the fades its bytes call: remove it from KNOWN (#{issue})"
            ));
        }
    }
    problems
}

#[test]
fn every_scene_records_the_fixed_fades_its_bytes_call() {
    let Some(rom) = rom() else {
        return;
    };
    let problems = report(rom, &documented_ranges(), SCENES, KNOWN);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// #655's scenes against the current US image, including Dorin's fade-in
/// and Saya's fade-in already restored on main before this repair.
#[test]
fn dorin_and_alys_wounded_calls_are_the_verified_us_sites() {
    let Some(rom) = rom() else { return };
    for (name, span, calls) in [
        (
            "Cutscene_Dorin",
            (0x074034, 0x0741E6),
            &[
                (0x074034, Fixed::Init),
                (0x07403A, Fixed::In),
                (0x07411C, Fixed::In),
            ][..],
        ),
        (
            "Cutscene_AlysWounded",
            (0x074B72, 0x075200),
            &[
                (0x074B72, Fixed::Init),
                (0x074B78, Fixed::In),
                (0x074C14, Fixed::Init),
                (0x074C1A, Fixed::In),
                (0x074D12, Fixed::In),
                (0x074EBC, Fixed::Out),
                (0x07500E, Fixed::In),
            ][..],
        ),
    ] {
        assert_eq!(retail_calls(rom, &[span]), calls, "{name}");
        let scene = SCENES.iter().find(|scene| scene.name == name).unwrap();
        assert_eq!(compare(name, calls, &native(scene.ops)), None);
        assert!(!KNOWN.iter().any(|(known, _)| *known == name));
    }
}

#[test]
fn the_ships_ways_are_the_helpers_calls() {
    let Some(rom) = rom() else {
        return;
    };
    let problems = ship_way_problems(rom, &SHIP_WAYS);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// The common planet screen is one US call site, visited by both takeoffs.
#[test]
fn the_flights_have_five_unique_fade_in_sites() {
    let Some(rom) = rom() else { return };
    let sites: std::collections::BTreeSet<u32> = SHIP_WAYS
        .iter()
        .flat_map(|(_, addresses)| {
            addresses
                .iter()
                .copied()
                .zip(kinds(rom, addresses))
                .filter_map(|(at, kind)| (kind == Some(Fixed::In)).then_some(at))
        })
        .collect();
    assert_eq!(
        sites.into_iter().collect::<Vec<_>>(),
        [0x0645D6, 0x0647B8, 0x0648F8, 0x064A2E, 0x064C8E]
    );
}

/// Negative control: an exception whose scene already matches is reported,
/// so the list only shrinks.
#[test]
fn a_listed_scene_that_matches_is_reported() {
    let Some(rom) = rom() else {
        return;
    };
    let ranges = documented_ranges();
    let scene = SCENES
        .iter()
        .find(|scene| scene.name == "Cutscene_ProfoundDarkness")
        .unwrap();
    assert_eq!(
        report(
            rom,
            &ranges,
            std::slice::from_ref(scene),
            &[("Cutscene_ProfoundDarkness", 1)]
        ),
        [
            "Cutscene_ProfoundDarkness now records the fades its bytes call: remove it from KNOWN (#1)"
        ]
    );
}

/// Negative controls: in every matching scene that records them, each op of
/// the three dropped, swapped for each of the other two, or doubled is
/// reported.
#[test]
fn a_wrong_fixed_fade_op_is_reported() {
    let Some(rom) = rom() else {
        return;
    };
    let ranges = documented_ranges();
    let kinds = [SceneOp::InitVramAndCram, SceneOp::FadeIn, SceneOp::FadeOut];
    let mut altered_count = 0;
    for scene in SCENES {
        if KNOWN.iter().any(|(name, _)| *name == scene.name) || !ranges.contains_key(scene.name) {
            continue;
        }
        for (index, op) in scene.ops.iter().enumerate() {
            if recorded(op).is_none() {
                continue;
            }
            let mut variants: Vec<Vec<SceneOp>> = Vec::new();
            let mut dropped = scene.ops.to_vec();
            dropped.remove(index);
            if scene.name == SHIP {
                retarget(&mut dropped, index, false);
            }
            variants.push(dropped);
            let mut doubled = scene.ops.to_vec();
            insert(&mut doubled, index, *op);
            variants.push(doubled);
            for other in kinds.iter().filter(|kind| recorded(kind) != recorded(op)) {
                let mut swapped = scene.ops.to_vec();
                swapped[index] = *other;
                variants.push(swapped);
            }
            for ops in variants {
                let altered = crate::Scene {
                    ops: Box::leak(ops.into_boxed_slice()),
                    ..*scene
                };
                assert_eq!(
                    mismatches(rom, &ranges, &[altered]).len(),
                    1,
                    "{} op {index} altered passed",
                    scene.name
                );
                altered_count += 1;
            }
        }
    }
    assert!(
        altered_count > 100,
        "only {altered_count} alterations checked"
    );
}

/// Negative controls for the ship (#654): the route as its bytes call it
/// matches on every way, and with any of its fixed fades dropped or doubled
/// it does not; a caller that loses the route, or a scene that gains it
/// without the `jmp`, is reported.
#[test]
fn the_production_route_matches_and_a_changed_route_or_caller_does_not() {
    let Some(rom) = rom() else {
        return;
    };
    let ranges = documented_ranges();
    let named = |name: &str| *SCENES.iter().find(|scene| scene.name == name).unwrap();
    let with_ops = |name: &str, ops: Vec<SceneOp>| crate::Scene {
        ops: Box::leak(ops.into_boxed_slice()),
        ..named(name)
    };
    let as_called = INSIDE_SPACESHIP_ROUTE.to_vec();
    assert_eq!(
        mismatches(rom, &ranges, &[with_ops(SHIP, as_called.clone())]),
        []
    );
    let mut altered = 0;
    for (index, op) in as_called.iter().enumerate() {
        if recorded(op).is_none() {
            continue;
        }
        let mut dropped = as_called.clone();
        dropped.remove(index);
        retarget(&mut dropped, index, false);
        let mut doubled = as_called.clone();
        insert(&mut doubled, index, *op);
        for ops in [dropped, doubled] {
            assert_eq!(
                mismatches(rom, &ranges, &[with_ops(SHIP, ops)]).len(),
                1,
                "route op {index} altered passed"
            );
            altered += 1;
        }
    }
    // Twelve fixed fades: the entry's two, the menu's two fade-outs and the
    // two flights' eight.
    assert_eq!(altered, 24);
    for caller in ["Cutscene_LeRoofAgain", "Cutscene_FindingAirCastle"] {
        let own = before_route(named(caller).ops).unwrap().to_vec();
        assert_eq!(
            mismatches(rom, &ranges, &[with_ops(caller, own)]).len(),
            1,
            "{caller} without the route passed"
        );
    }
    let mut gained = named("Cutscene_ProfoundDarkness").ops.to_vec();
    gained.extend(INSIDE_SPACESHIP_ROUTE);
    assert_eq!(
        mismatches(
            rom,
            &ranges,
            &[with_ops("Cutscene_ProfoundDarkness", gained)]
        )
        .len(),
        1
    );
}

/// Negative controls: a way that lists an address with no call, or loses the
/// only listing of a call, is reported.
#[test]
fn a_wrong_ship_way_is_reported() {
    let Some(rom) = rom() else {
        return;
    };
    let with = |cancel: &'static [u32]| {
        let mut ways = SHIP_WAYS;
        ways[2] = (Way::Cancel, cancel);
        ship_way_problems(rom, &ways)
    };
    assert_eq!(with(&[0x06_3E8A]), Vec::<String>::new());
    assert_eq!(with(&[]), ["$063E8A Out is on no way out of the menu"]);
    assert_eq!(
        with(&[0x06_3E8A, 0x06_3E90]),
        ["Cancel: no fixed fade call at $063E90"]
    );
}

/// Zio's direct calls in execution order. Its elevator helpers and the
/// spaceport helper have no fixed fades; Init's internal Out belongs to Init.
#[test]
fn zio_defeated_calls_are_the_verified_us_sites() {
    let Some(rom) = rom() else { return };
    let calls = [
        (0x075A12, Fixed::Init),
        (0x075A18, Fixed::In),
        (0x075B10, Fixed::Out),
        (0x075B4E, Fixed::In),
        (0x075C9A, Fixed::Out),
        (0x075CA0, Fixed::Init),
        (0x075CA6, Fixed::In),
        (0x075D66, Fixed::In),
        (0x075D72, Fixed::Out),
        (0x075D78, Fixed::Init),
        (0x075D7E, Fixed::In),
        (0x075D90, Fixed::Init),
        (0x075D96, Fixed::In),
        (0x075EBC, Fixed::In),
    ];
    assert_eq!(retail_calls(rom, &[(0x075A12, 0x075EE0)]), calls);
    assert!(retail_calls(rom, &[(0x075EE0, 0x075FC8), (0x06B6F4, 0x06B86E)]).is_empty());
    // Both BSRs return into the linear scene body; neither callee owns a fade.
    for (at, target) in [(0x075C0C_usize, 0x075EE0_i32), (0x075C42, 0x075F54)] {
        assert_eq!(&rom[at..at + 2], &[0x61, 0]);
        let displacement = i16::from_be_bytes(rom[at + 2..at + 4].try_into().unwrap());
        assert_eq!(at as i32 + 2 + i32::from(displacement), target);
    }
    assert_eq!(&rom[0x075EDE..0x075EE0], &[0x4E, 0x75]);
    // $075D6C calls the spaceport helper, returns to the first repaired Out.
    assert_eq!(&rom[0x075D6C..0x075D72], &[0x4E, 0xB9, 0, 6, 0xB6, 0xF4]);
    assert_eq!(
        compare(
            "Cutscene_ZioDefeated",
            &calls,
            &native(super::post_rika_cutscenes::ZIO_DEFEATED.ops)
        ),
        None
    );
}

#[path = "post_zio_fades_image.rs"]
mod post_zio;

#[path = "lashiec_elevator_fades_image.rs"]
mod lashiec_elevator;
