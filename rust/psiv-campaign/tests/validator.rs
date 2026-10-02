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
    assert_eq!(ids.last(), Some(&"aiedo"));
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
            "{\"do\": \"open_chest\", \"chest\": ",
            "{\"do\": \"open_chest\", \"chest\": 90",
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
        "{\"do\": \"go_to_map\", \"map\": 0, \"via_warp\": 3}\n        ], \"note\"",
        "{\"do\": \"go_to_map\", \"map\": 70, \"via_warp\": 8}\n        ], \"note\"",
    )) else {
        return;
    };
    assert!(!report.is_ok(), "a refuge must return to the patrol's map");
}
