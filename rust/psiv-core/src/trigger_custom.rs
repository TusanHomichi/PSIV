//! The `RunEvent_*` routines that are not a flags-plus-position formula.
//!
//! Ten labels, covering thirty of the table's 128 slots. Six are transcribed
//! exactly here; four need state this crate does not carry and say so instead
//! of guessing.
//!
//! Four of them share the `XYRangeJmpTbl` helper (`ps4.asm:112576`), whose
//! rectangles are **half-open** — `cmp.w d2,d0 / bhi` fails the test unless
//! `px >= tx`, and `cmp.w d2,d6 / bls` fails unless `px < tx + span`. That is
//! a different rule from the inline `bcs`/`bhi` pairs elsewhere in the table,
//! which are closed intervals, and the two must not be merged.

use crate::state::Flag;
use crate::trigger::{
    CustomTrigger, EventIndex, PixelPos, TriggerContext, TriggerResult, Unsupported,
};

/// `XYRange_XYPlus20`: `tx <= px < tx+$20 && ty <= py < ty+$20`.
const SPAN_20: (i32, i32) = (0x20, 0x20);
/// `XYRange_XPlus40_YPlus20`: `tx <= px < tx+$40 && ty <= py < ty+$20`.
const SPAN_40_20: (i32, i32) = (0x40, 0x20);

/// The helper's half-open containment test.
const fn in_span(at: PixelPos, target: (i32, i32), span: (i32, i32)) -> bool {
    at.x >= target.0 && at.x < target.0 + span.0 && at.y >= target.1 && at.y < target.1 + span.1
}

/// Collision type 2 — `TileColl_Recovery`, the value `RunEvent_Recovery` edges on.
const RECOVERY_COLLISION: u8 = 2;

/// Runs a custom check.
pub fn evaluate(custom: CustomTrigger, ctx: &TriggerContext<'_>) -> TriggerResult {
    match custom {
        CustomTrigger::Recovery => recovery(ctx),
        CustomTrigger::VahFortMovingPlatform => vah_fort_moving_platform(ctx),
        CustomTrigger::WpnPlntMovingPlatform => wpn_plnt_moving_platform(ctx),
        CustomTrigger::VahFortConveyorBelt => vah_fort_conveyor(ctx),
        CustomTrigger::WpnPlntConveyorBelt => wpn_plnt_conveyor(ctx),
        CustomTrigger::CarnivorousTrees => carnivorous_trees(ctx),
        CustomTrigger::RidingElevator | CustomTrigger::EnterGrbkTwDoor => {
            TriggerResult::Unsupported(custom, Unsupported::MapLayoutBytes)
        }
        CustomTrigger::MileSandWorm => TriggerResult::Unsupported(custom, Unsupported::Rng),
        CustomTrigger::PenguFeedStolen => {
            TriggerResult::Unsupported(custom, Unsupported::Inventory)
        }
    }
}

/// `$12 RunEvent_Recovery` (`ps4.asm:115574`).
///
/// ```text
///     cmpi.b  #2, (Tile_Collision_Standing).w
///     bne.w   RunEvent_NoEvent
///     cmpi.b  #2, (Saved_Tile_Collision_Standing).w
///     beq.w   RunEvent_NoEvent
///     move.w  #$21, (Event_Index).w
/// ```
///
/// A rising edge: standing on a recovery cell now, and not on one before. No
/// coordinates and no flags.
fn recovery(ctx: &TriggerContext<'_>) -> TriggerResult {
    let now = ctx.standing == Some(RECOVERY_COLLISION);
    let before = ctx.previously_standing == Some(RECOVERY_COLLISION);
    if now && !before {
        TriggerResult::Fire(EventIndex(0x21))
    } else {
        TriggerResult::NoEvent
    }
}

/// One flag-selected probe: the target Y depends on whether a temp flag is set.
struct Platform {
    x: i32,
    y_when_clear: i32,
    y_when_set: i32,
    selector: Flag,
    event: u16,
}

fn platforms(ctx: &TriggerContext<'_>, probes: &[Platform]) -> TriggerResult {
    // Both platform routines open with the same guard.
    if ctx.previously_standing == Some(RECOVERY_COLLISION) {
        return TriggerResult::NoEvent;
    }
    for probe in probes {
        let y = if ctx.state.is_set(probe.selector) {
            probe.y_when_set
        } else {
            probe.y_when_clear
        };
        if in_span(ctx.at, (probe.x, y), SPAN_40_20) {
            return TriggerResult::Fire(EventIndex(probe.event));
        }
    }
    TriggerResult::NoEvent
}

/// `$0E RunEvent_VahFortMovingPlatform` (`ps4.asm:115267`).
fn vah_fort_moving_platform(ctx: &TriggerContext<'_>) -> TriggerResult {
    platforms(
        ctx,
        &[
            Platform {
                x: 0x2C0,
                y_when_clear: 0x210,
                y_when_set: 0x2B0,
                selector: Flag::temp(0x09),
                event: 0x15,
            },
            Platform {
                x: 0x2C0,
                y_when_clear: 0x390,
                y_when_set: 0x2F0,
                selector: Flag::temp(0x0A),
                event: 0x16,
            },
        ],
    )
}

/// `$0F RunEvent_WpnPlntMovingPlatform` (`ps4.asm:115307`).
fn wpn_plnt_moving_platform(ctx: &TriggerContext<'_>) -> TriggerResult {
    platforms(
        ctx,
        &[
            Platform {
                x: 0x1C0,
                y_when_clear: 0x290,
                y_when_set: 0x370,
                selector: Flag::temp(0x0D),
                event: 0x17,
            },
            Platform {
                x: 0x220,
                y_when_clear: 0x150,
                y_when_set: 0x230,
                selector: Flag::temp(0x0E),
                event: 0x18,
            },
            Platform {
                x: 0x380,
                y_when_clear: 0x150,
                y_when_set: 0x230,
                selector: Flag::temp(0x0F),
                event: 0x19,
            },
            Platform {
                x: 0x3E0,
                y_when_clear: 0x290,
                y_when_set: 0x370,
                selector: Flag::temp(0x10),
                event: 0x1A,
            },
        ],
    )
}

/// A conveyor group: some probe points that share an outcome, where a temp
/// flag swaps the event written.
struct Conveyor {
    targets: &'static [(i32, i32)],
    default_event: u16,
    swapped_event: u16,
    selector: Option<Flag>,
}

fn conveyors(ctx: &TriggerContext<'_>, groups: &[Conveyor]) -> TriggerResult {
    for group in groups {
        if !group
            .targets
            .iter()
            .any(|&target| in_span(ctx.at, target, SPAN_20))
        {
            continue;
        }
        let swapped = group
            .selector
            .is_some_and(|selector| ctx.state.is_set(selector));
        let event = if swapped {
            group.swapped_event
        } else {
            group.default_event
        };
        return TriggerResult::Fire(EventIndex(event));
    }
    TriggerResult::NoEvent
}

/// `$10 RunEvent_VahFortConveyorBelt` (`ps4.asm:115383`). Probes are tried in
/// listed order; the first hit wins.
fn vah_fort_conveyor(ctx: &TriggerContext<'_>) -> TriggerResult {
    conveyors(
        ctx,
        &[
            Conveyor {
                targets: &[(0x180, 0x210), (0x180, 0x290)],
                default_event: 0x1D,
                swapped_event: 0x1D,
                selector: None,
            },
            Conveyor {
                targets: &[(0x440, 0x210), (0x440, 0x290)],
                default_event: 0x1E,
                swapped_event: 0x1E,
                selector: None,
            },
            Conveyor {
                targets: &[(0x180, 0x310), (0x180, 0x390)],
                default_event: 0x1D,
                swapped_event: 0x1E,
                selector: Some(Flag::temp(0x0C)),
            },
            Conveyor {
                targets: &[(0x440, 0x310), (0x440, 0x390)],
                default_event: 0x1D,
                swapped_event: 0x1E,
                selector: Some(Flag::temp(0x0B)),
            },
            Conveyor {
                targets: &[(0x1C0, 0x3D0), (0x220, 0x3D0)],
                default_event: 0x1F,
                swapped_event: 0x20,
                selector: Some(Flag::temp(0x0C)),
            },
            Conveyor {
                targets: &[(0x3A0, 0x3D0), (0x400, 0x3D0)],
                default_event: 0x20,
                swapped_event: 0x1F,
                selector: Some(Flag::temp(0x0B)),
            },
        ],
    )
}

/// `$11 RunEvent_WpnPlntConveyorBelt` (`ps4.asm:115482`).
fn wpn_plnt_conveyor(ctx: &TriggerContext<'_>) -> TriggerResult {
    conveyors(
        ctx,
        &[
            Conveyor {
                targets: &[(0x120, 0x130), (0x120, 0x1D0)],
                default_event: 0x1D,
                swapped_event: 0x1E,
                selector: Some(Flag::temp(0x11)),
            },
            Conveyor {
                targets: &[(0x4A0, 0x130), (0x4A0, 0x1D0)],
                default_event: 0x1D,
                swapped_event: 0x1E,
                selector: Some(Flag::temp(0x12)),
            },
            Conveyor {
                targets: &[
                    (0x160, 0x210),
                    (0x1C0, 0x210),
                    (0x320, 0x210),
                    (0x380, 0x210),
                ],
                default_event: 0x1F,
                swapped_event: 0x20,
                selector: Some(Flag::temp(0x11)),
            },
            Conveyor {
                targets: &[
                    (0x240, 0x210),
                    (0x2A0, 0x210),
                    (0x400, 0x210),
                    (0x460, 0x210),
                ],
                default_event: 0x20,
                swapped_event: 0x1F,
                selector: Some(Flag::temp(0x12)),
            },
        ],
    )
}

/// `$35 RunEvent_CarnivorousTrees` (`ps4.asm:115977`).
///
/// Two arms over **complementary** regions — the region test branches away to
/// the second arm rather than failing the routine, so being outside arm A's
/// box is arm B's precondition, not a rejection.
fn carnivorous_trees(ctx: &TriggerContext<'_>) -> TriggerResult {
    const ECLIPSE_TORCH: Flag = Flag::event(0x9C);
    const RAJA_SICK: Flag = Flag::event(0x94);
    const CARNIVOROUS_TREES: Flag = Flag::event(0x95);
    const KYRA_JOINED: Flag = Flag::event(0xA0);

    if ctx.state.is_set(ECLIPSE_TORCH) {
        return TriggerResult::NoEvent;
    }

    let in_arm_a = ctx.at.y <= 0xC0 && ctx.at.x >= 0xBA0 && ctx.at.x <= 0xBB0;
    if in_arm_a {
        // $4C, upgraded to $4D when Raja is sick and the trees have not fired.
        let upgraded = ctx.state.is_set(RAJA_SICK) && ctx.state.is_clear(CARNIVOROUS_TREES);
        return TriggerResult::Fire(EventIndex(if upgraded { 0x4D } else { 0x4C }));
    }

    if ctx.state.is_set(CARNIVOROUS_TREES) && ctx.state.is_clear(KYRA_JOINED) {
        return TriggerResult::Fire(EventIndex(0x8013));
    }
    TriggerResult::NoEvent
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::GameState;

    fn ctx<'a>(state: &'a GameState, x: i32, y: i32) -> TriggerContext<'a> {
        TriggerContext {
            state,
            at: PixelPos { x, y },
            standing: None,
            previously_standing: None,
        }
    }

    #[test]
    fn the_helper_rectangle_is_half_open() {
        let target = (0x100, 0x200);
        assert!(in_span(PixelPos { x: 0x100, y: 0x200 }, target, SPAN_20));
        assert!(in_span(PixelPos { x: 0x11F, y: 0x21F }, target, SPAN_20));
        assert!(
            !in_span(PixelPos { x: 0x120, y: 0x200 }, target, SPAN_20),
            "the upper bound is exclusive, unlike the inline ranges"
        );
        assert!(!in_span(PixelPos { x: 0x0FF, y: 0x200 }, target, SPAN_20));
    }

    #[test]
    fn recovery_fires_only_on_the_rising_edge() {
        let state = GameState::new();
        let mut c = ctx(&state, 0, 0);

        c.standing = Some(2);
        c.previously_standing = Some(0);
        assert_eq!(recovery(&c), TriggerResult::Fire(EventIndex(0x21)));

        c.previously_standing = Some(2);
        assert_eq!(recovery(&c), TriggerResult::NoEvent, "still standing on it");

        c.standing = Some(0);
        c.previously_standing = Some(0);
        assert_eq!(recovery(&c), TriggerResult::NoEvent);
    }

    #[test]
    fn a_moving_platform_probe_follows_its_selector_flag() {
        let mut state = GameState::new();
        assert_eq!(
            vah_fort_moving_platform(&ctx(&state, 0x2C0, 0x210)),
            TriggerResult::Fire(EventIndex(0x15)),
            "flag clear: the platform is at $210"
        );

        // The routine's guard: standing on recovery last frame suppresses it.
        let mut guarded = ctx(&state, 0x2C0, 0x210);
        guarded.previously_standing = Some(2);
        assert_eq!(vah_fort_moving_platform(&guarded), TriggerResult::NoEvent);

        state.set(Flag::temp(0x09)).unwrap();
        let moved = ctx(&state, 0x2C0, 0x210);
        assert_eq!(
            vah_fort_moving_platform(&moved),
            TriggerResult::NoEvent,
            "flag set: the platform moved to $2B0"
        );
        let followed = ctx(&state, 0x2C0, 0x2B0);
        assert_eq!(
            vah_fort_moving_platform(&followed),
            TriggerResult::Fire(EventIndex(0x15))
        );
    }

    #[test]
    fn a_conveyor_group_swaps_its_event_on_the_terminal_flag() {
        let mut state = GameState::new();
        let c = ctx(&state, 0x180, 0x310);
        assert_eq!(vah_fort_conveyor(&c), TriggerResult::Fire(EventIndex(0x1D)));

        state.set(Flag::temp(0x0C)).unwrap();
        let flipped = ctx(&state, 0x180, 0x310);
        assert_eq!(
            vah_fort_conveyor(&flipped),
            TriggerResult::Fire(EventIndex(0x1E))
        );
    }

    #[test]
    fn carnivorous_trees_has_two_complementary_arms() {
        let mut state = GameState::new();

        // Arm A, plain.
        let inside = ctx(&state, 0xBA0, 0xC0);
        assert_eq!(
            carnivorous_trees(&inside),
            TriggerResult::Fire(EventIndex(0x4C))
        );

        // Arm A, upgraded.
        state.set(Flag::event(0x94)).unwrap();
        let upgraded = ctx(&state, 0xBA0, 0xC0);
        assert_eq!(
            carnivorous_trees(&upgraded),
            TriggerResult::Fire(EventIndex(0x4D))
        );

        // Outside arm A is arm B's precondition, not a rejection.
        let outside = ctx(&state, 0x100, 0x400);
        assert_eq!(carnivorous_trees(&outside), TriggerResult::NoEvent);
        state.set(Flag::event(0x95)).unwrap();
        let armed = ctx(&state, 0x100, 0x400);
        assert_eq!(
            carnivorous_trees(&armed),
            TriggerResult::Fire(EventIndex(0x8013))
        );

        // The whole routine is gated on the Eclipse Torch being unused.
        state.set(Flag::event(0x9C)).unwrap();
        let gated = ctx(&state, 0x100, 0x400);
        assert_eq!(carnivorous_trees(&gated), TriggerResult::NoEvent);
    }

    #[test]
    fn the_four_undecidable_routines_say_so() {
        let state = GameState::new();
        let c = ctx(&state, 0, 0);
        for (custom, why) in [
            (CustomTrigger::RidingElevator, Unsupported::MapLayoutBytes),
            (CustomTrigger::EnterGrbkTwDoor, Unsupported::MapLayoutBytes),
            (CustomTrigger::MileSandWorm, Unsupported::Rng),
            (CustomTrigger::PenguFeedStolen, Unsupported::Inventory),
        ] {
            assert_eq!(
                evaluate(custom, &c),
                TriggerResult::Unsupported(custom, why),
                "{custom:?} must not silently read as NoEvent"
            );
        }
    }
}
