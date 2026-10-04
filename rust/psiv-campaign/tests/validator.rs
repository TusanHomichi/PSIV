//! The route validator: it accepts the shipped route and rejects mutated
//! copies (negative controls), naming chapter, objective index and reason.

mod common;

use common::{MAIN_ROUTE, pack};
use psiv_campaign::route::Route;
use psiv_campaign::validate::{Report, validate};

fn main_text() -> String {
    std::fs::read_to_string(MAIN_ROUTE).expect("routes/main.json is committed")
}

fn run(text: &str) -> Option<Report> {
    let pack = pack()?;
    let route = Route::parse(text).expect("route parses");
    Some(validate(&route, &pack.data, &pack.battle))
}

/// Replaces exactly one occurrence, so a mutation that stops matching the
/// file fails loudly instead of silently testing the unmodified route.
fn mutate(text: &str, from: &str, to: &str) -> String {
    assert_eq!(
        text.matches(from).count(),
        1,
        "mutation anchor {from:?} must occur exactly once"
    );
    text.replace(from, to)
}

#[test]
fn the_shipped_route_parses_and_has_its_chapters_in_order() {
    let route = Route::parse(&main_text()).expect("main.json parses");
    let ids: Vec<&str> = route.chapters.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids.first(), Some(&"academy"));
    assert_eq!(ids.last(), Some(&"dezolis-first-control"));
    assert_eq!(
        ids[ids.len() - 4..],
        [
            "mota-spaceport",
            "zelan-wren-canceller",
            "zelan-sabotage",
            "dezolis-first-control"
        ]
    );
    assert!(ids.contains(&"nurvus-zio"));
    assert!(ids.contains(&"aiedo"));
    assert!(ids.contains(&"north-bank"));
    for chapter in &route.chapters {
        assert!(!chapter.source.is_empty(), "{} cites no source", chapter.id);
        assert!(!chapter.closing.is_empty(), "{} has no closing", chapter.id);
    }
}

#[test]
fn the_validator_accepts_the_shipped_route() {
    let Some(report) = run(&main_text()) else {
        return;
    };
    assert!(
        report.is_ok(),
        "errors: {:#?}",
        report
            .errors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    );
    assert!(report.objectives > 150);
    assert!(report.planned_warps > 80);
}

#[test]
fn a_wrong_item_id_is_rejected_with_its_location() {
    let text = mutate(
        &main_text(),
        "\"item\": \"STEL-SWORD\", \"count\": 1",
        "\"item\": \"NO-SUCH-ITEM\", \"count\": 1",
    );
    let Some(report) = run(&text) else { return };
    let error = report
        .errors
        .iter()
        .find(|e| e.reason.contains("NO-SUCH-ITEM"))
        .expect("the bad item is reported");
    assert_eq!(error.chapter, "zema-outfit");
    assert!(error.objective.is_some());
    assert!(error.to_string().contains("zema-outfit"));
    // A numeric id past the table is rejected the same way.
    let numeric = mutate(
        &main_text(),
        "\"item\": \"CIRCLET\", \"count\": 1",
        "\"item\": 250, \"count\": 1",
    );
    let report = run(&numeric).unwrap();
    assert!(report.errors.iter().any(|e| e.reason.contains("item #250")));
}

#[test]
fn an_unreachable_go_to_is_rejected() {
    // (0,0) is not a walkable cell of Motavia.
    let text = mutate(&main_text(), "\"cell\": [84, 68]", "\"cell\": [0, 0]");
    let Some(report) = run(&text) else { return };
    let error = report
        .errors
        .iter()
        .find(|e| e.chapter == "north-bank")
        .expect("the bad go_to is reported in its chapter");
    assert!(error.objective.is_some());
    assert!(
        error.reason.contains("no walkable chain"),
        "{}",
        error.reason
    );
}

/// The Rika bridge opens on flag $35. Remove the chapter's claim and the
/// crossing is no longer statically walkable: the validator uses the flags
/// the route claims, not an open world.
#[test]
fn a_walk_that_needs_an_unclaimed_flag_is_rejected() {
    let text = mutate(
        &main_text(),
        "{\"do\": \"expect\", \"flags_set\": [\"event:0x34\", \"event:0x35\"]",
        "{\"do\": \"expect\", \"flags_set\": [\"event:0x34\"]",
    );
    let text = text.replace(
        "\"closing\": [\n        {\"flags_set\": [\"event:0x34\", \"event:0x35\"]",
        "\"closing\": [\n        {\"flags_set\": [\"event:0x34\"]",
    );
    let Some(report) = run(&text) else { return };
    assert!(
        report.errors.iter().any(|e| e.chapter == "north-bank"),
        "the crossing must fail without $35: {:?}",
        report.errors
    );
}

#[test]
fn bad_object_chest_flag_and_slot_ids_are_rejected() {
    let base = main_text();
    for (from, to, needle) in [
        (
            "{\"do\": \"open_chest\", \"chest\": 0, \"note\": \"native leg walks to (40,33)",
            "{\"do\": \"open_chest\", \"chest\": 90, \"note\": \"native leg walks to (40,33)",
            "chest",
        ),
        (
            "\"slot\": 0, \"note\": \"native saved-map-0AA\"",
            "\"slot\": 9, \"note\": \"native saved-map-0AA\"",
            "slot 9",
        ),
        (
            "{\"do\": \"expect\", \"flags_set\": [\"event:0x35\"], \"map\": 0, \"cell\": [84, 64]",
            "{\"do\": \"expect\", \"flags_set\": [\"event:0x9999\"], \"map\": 0, \"cell\": [84, 64]",
            "flag",
        ),
    ] {
        let text = mutate(&base, from, to);
        let Some(report) = run(&text) else { return };
        assert!(
            report.errors.iter().any(|e| e.reason.contains(needle)),
            "{needle}: {:?}",
            report.errors
        );
    }
}

#[test]
fn the_route_visits_mile_before_zema_and_again_for_the_inn() {
    // RunEvent_MileSandWorm is transcribed (#54): Mile is on the route again,
    // once in `holt` and once in `rune-dorin` for the inn.
    let route = Route::parse(&main_text()).expect("main.json parses");
    let visits = |chapter: &str| {
        route
            .chapters
            .iter()
            .find(|c| c.id == chapter)
            .expect("chapter exists")
            .objectives
            .iter()
            .filter(|s| {
                matches!(
                    s.objective,
                    psiv_campaign::route::Objective::GoToMap { map: 29, .. }
                )
            })
            .count()
    };
    assert!(visits("holt") >= 1, "holt visits Mile");
    assert!(visits("rune-dorin") >= 1, "rune-dorin visits Mile");
}

#[test]
fn a_patrol_refuge_that_ends_on_another_map_is_rejected() {
    let Some(report) = run(&mutate(
        &main_text(),
        "{\"do\": \"go_to_map\", \"map\": 65, \"via_warp\": 0},\n          {\"do\": \"go_to_map\", \"map\": 0, \"via_warp\": 3}\n        ], \"note\"",
        "{\"do\": \"go_to_map\", \"map\": 65, \"via_warp\": 0},\n          {\"do\": \"go_to_map\", \"map\": 70, \"via_warp\": 8}\n        ], \"note\"",
    )) else {
        return;
    };
    assert!(!report.is_ok(), "a refuge must return to the patrol's map");
}

/// The Ladea Tower's corridors are closed to the Land Rover, and the tower's
/// own map load is what parks it: `GameMode_LoadFieldMap` clears
/// `Vehicle_Index` unless `Map_Load_Flags` bit 0 or 2 is set
/// (`ps4.asm:107507-107517`). The route asserts the parked selector before it
/// walks the corridors; drop that assertion and the walk is statically
/// impossible for a mounted party.
const TOWER_PARKED: &str = "        {\"do\": \"expect\", \"map\": 140, \"vehicle\": 0, \"note\": \"the tower's map load parks the Land Rover (GameMode_LoadFieldMap, ps4.asm:107517): no dismount press is needed, and the tower's one-cell corridors are on foot\"},\n";

#[test]
fn the_tower_walk_needs_the_vehicle_parked() {
    let text = mutate(&main_text(), TOWER_PARKED, "");
    let Some(report) = run(&text) else { return };
    assert!(
        report
            .errors
            .iter()
            .any(|e| e.chapter == "ladea-tower-rune"),
        "a mounted party cannot walk the tower: {:?}",
        report.errors
    );
}

#[test]
fn dismounting_on_foot_and_a_bad_vehicle_are_rejected() {
    // A dismount press after the tower's load has already parked the machine.
    let on_foot = mutate(
        &main_text(),
        TOWER_PARKED,
        &format!("{TOWER_PARKED}        {{\"do\": \"dismount\"}},\n"),
    );
    let Some(report) = run(&on_foot) else { return };
    assert!(
        report
            .errors
            .iter()
            .any(|e| e.reason.contains("the party on foot")),
        "{:?}",
        report.errors
    );
    let bad = mutate(
        &main_text(),
        "\"vehicle\": 1, \"flags_set\": [\"event:0x44\"], \"note\"",
        "\"vehicle\": 9, \"flags_set\": [\"event:0x44\"], \"note\"",
    );
    let report = run(&bad).unwrap();
    assert!(
        report.errors.iter().any(|e| e.reason.contains("vehicle 9")),
        "{:?}",
        report.errors
    );
}

#[test]
fn a_boarding_objective_must_name_a_world() {
    // A destination that is not a World_Index is rejected with its location.
    let bad = mutate(
        &main_text(),
        "{\"do\": \"board\", \"step\": \"up\", \"to\": 3,",
        "{\"do\": \"board\", \"step\": \"up\", \"to\": 9,",
    );
    let Some(report) = run(&bad) else { return };
    let error = report
        .errors
        .iter()
        .find(|e| e.reason.contains("not a World_Index"))
        .expect("the bad world is reported");
    assert_eq!(error.chapter, "mota-spaceport");
    // A name nothing carries is rejected too, whether or not the local pack has
    // the destination screen to resolve names against.
    let unknown = mutate(
        &main_text(),
        "{\"do\": \"board\", \"step\": \"up\", \"to\": 3,",
        "{\"do\": \"board\", \"step\": \"up\", \"to\": \"Atlantis\",",
    );
    let report = run(&unknown).unwrap();
    assert!(
        report.errors.iter().any(|e| e.reason.contains("Atlantis")),
        "{:?}",
        report.errors
    );
}

#[test]
fn the_zelan_chapters_reject_a_bad_boarding_and_a_missing_object() {
    // The sabotage's boarding names Kuran; any other number is rejected with the
    // chapter that holds it.
    let bad_world = mutate(
        &main_text(),
        "{\"do\": \"board\", \"step\": \"down\", \"to\": 4,",
        "{\"do\": \"board\", \"step\": \"down\", \"to\": 9,",
    );
    let Some(report) = run(&bad_world) else {
        return;
    };
    let error = report
        .errors
        .iter()
        .find(|e| e.reason.contains("not a World_Index"))
        .expect("the bad world is reported");
    assert_eq!(error.chapter, "zelan-sabotage");
    // Zelan F1 has two objects (Wren and the elevator): a seventh does not exist.
    let bad_npc = mutate(
        &main_text(),
        "{\"do\": \"talk\", \"npc\": 0, \"note\": \"Wren,",
        "{\"do\": \"talk\", \"npc\": 7, \"note\": \"Wren,",
    );
    let report = run(&bad_npc).unwrap();
    let error = report
        .errors
        .iter()
        .find(|e| e.reason.contains("object 7 does not exist"))
        .expect("the missing object is reported");
    assert_eq!(error.chapter, "zelan-wren-canceller");
}
