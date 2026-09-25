//! Trigger evaluation against the retail table.
//!
//! The table cases run against `TRIGGERS` itself, not against synthetic entries:
//! the pixel literals in the table are what the cartridge dispatches on, so a
//! case that pins one room's coordinates pins the decode. One case drives every
//! `AxisPredicate` variant through a `Condition` the table has no room for, and
//! the list cases cover `evaluate_list`'s ordering rule, including the
//! undecidable entries that must surface rather than read as "no event".

use crate::fixtures::ctx;
use psiv_core::{
    AxisPredicate, Cell, Condition, CustomTrigger, EventIndex, Flag, GameState, PixelPos,
    PositionPredicate, TRIGGERS, Trigger, TriggerResult, Unsupported, evaluate_list,
};

#[test]
fn the_alys_trigger_fires_from_the_cell_the_pixel_literals_name() {
    // $260 / $F0 is column 38, row 16 once the standing-cell shift is undone.
    let state = GameState::new();
    let at = PixelPos::from_cell(Cell::new(38, 16));
    assert_eq!(at, PixelPos { x: 0x260, y: 0xF0 });

    assert_eq!(
        TRIGGERS[0x03].evaluate(&ctx(&state, at)),
        TriggerResult::Fire(EventIndex(3))
    );

    // The row above is below the threshold; the column beside it is wrong.
    assert_eq!(
        TRIGGERS[0x03].evaluate(&ctx(&state, PixelPos::from_cell(Cell::new(38, 15)))),
        TriggerResult::NoEvent
    );
    assert_eq!(
        TRIGGERS[0x03].evaluate(&ctx(&state, PixelPos::from_cell(Cell::new(39, 20)))),
        TriggerResult::NoEvent
    );
}

#[test]
fn every_axis_predicate_variant_behaves() {
    let state = GameState::new();
    let cases: [(AxisPredicate, i32, bool); 12] = [
        (AxisPredicate::Any, -9999, true),
        (AxisPredicate::Exact(0x100), 0x100, true),
        (AxisPredicate::Exact(0x100), 0x101, false),
        (AxisPredicate::AtLeast(0x100), 0x100, true),
        (AxisPredicate::AtLeast(0x100), 0x0FF, false),
        (AxisPredicate::AtLeast(0x100), 0x200, true),
        (AxisPredicate::AtMost(0x100), 0x100, true),
        (AxisPredicate::AtMost(0x100), 0x101, false),
        (AxisPredicate::AtMost(0x100), 0x000, true),
        (AxisPredicate::Between(0x100, 0x120), 0x100, true),
        (AxisPredicate::Between(0x100, 0x120), 0x120, true),
        (AxisPredicate::Between(0x100, 0x120), 0x121, false),
    ];

    for (predicate, value, expected) in cases {
        let trigger = Trigger::Condition(Condition {
            require_set: &[],
            require_clear: &[],
            position: PositionPredicate::new(predicate, AxisPredicate::Any),
            event: EventIndex(1),
        });
        let hit = trigger.evaluate(&ctx(&state, PixelPos { x: value, y: 0 }))
            == TriggerResult::Fire(EventIndex(1));
        assert_eq!(hit, expected, "{predicate:?} against {value:#X}");
    }
}

#[test]
fn a_rectangle_needs_both_axes() {
    // $39 LutzRevelation: x in [$1E0,$210] and y in [$1D0,$1F0].
    let state = GameState::new();
    let inside = PixelPos { x: 0x1F0, y: 0x1E0 };
    assert_eq!(
        TRIGGERS[0x39].evaluate(&ctx(&state, inside)),
        TriggerResult::Fire(EventIndex(0x8014))
    );
    for outside in [
        PixelPos { x: 0x1F0, y: 0x1C0 },
        PixelPos { x: 0x1F0, y: 0x200 },
        PixelPos { x: 0x1D0, y: 0x1E0 },
        PixelPos { x: 0x220, y: 0x1E0 },
    ] {
        assert_eq!(
            TRIGGERS[0x39].evaluate(&ctx(&state, outside)),
            TriggerResult::NoEvent,
            "{outside:?} is outside the rect"
        );
    }
}

#[test]
fn an_event_list_stops_at_its_first_hit() {
    // `RunEvents` scans the map's list in order and returns on the first pass,
    // so list order is behaviour.
    let state = GameState::new();
    let at = PixelPos::from_cell(Cell::new(38, 16));

    // $08 (flags only, fires on a fresh state) listed before $03.
    let hit = evaluate_list(&TRIGGERS, &[0x08, 0x03], &ctx(&state, at));
    assert_eq!(hit, Some((0x08, TriggerResult::Fire(EventIndex(0x0C)))));

    // Reversed, the Alys check wins.
    let hit = evaluate_list(&TRIGGERS, &[0x03, 0x08], &ctx(&state, at));
    assert_eq!(hit, Some((0x03, TriggerResult::Fire(EventIndex(3)))));
}

#[test]
fn a_list_of_misses_returns_nothing() {
    let mut state = GameState::new();
    state.set(Flag::event(0x08)).unwrap();
    state.set(Flag::event(0x0D)).unwrap();
    let at = PixelPos::from_cell(Cell::new(0, 1));
    assert_eq!(
        evaluate_list(&TRIGGERS, &[0x03, 0x08], &ctx(&state, at)),
        None
    );
}

#[test]
fn undecidable_entries_are_reported_never_silently_skipped() {
    let state = GameState::new();
    let at = PixelPos { x: 0, y: 0 };

    // $7B needs an inventory. It must surface rather than read as "no event".
    let hit = evaluate_list(&TRIGGERS, &[0x7B], &ctx(&state, at));
    assert_eq!(
        hit,
        Some((
            0x7B,
            TriggerResult::Unsupported(CustomTrigger::PenguFeedStolen, Unsupported::Inventory)
        ))
    );

    // A real hit later in the list still wins over an earlier undecidable one.
    let hit = evaluate_list(&TRIGGERS, &[0x7B, 0x08], &ctx(&state, at));
    assert_eq!(hit, Some((0x08, TriggerResult::Fire(EventIndex(0x0C)))));
}

#[test]
fn the_recovery_trigger_reads_the_collision_context() {
    let state = GameState::new();
    let mut c = ctx(&state, PixelPos { x: 0, y: 0 });
    c.standing = Some(2);
    c.previously_standing = Some(0);
    assert_eq!(
        TRIGGERS[0x12].evaluate(&c),
        TriggerResult::Fire(EventIndex(0x21))
    );
    c.previously_standing = Some(2);
    assert_eq!(TRIGGERS[0x12].evaluate(&c), TriggerResult::NoEvent);
}
