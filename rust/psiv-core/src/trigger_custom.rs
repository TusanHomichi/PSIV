//! The `RunEvent_*` routines that are not a flags-plus-position formula.
//!
//! Ten labels, covering thirty of the table's 128 slots, all transcribed
//! exactly here. The four that read more than flags and position take it from
//! [`TriggerContext`]: the live layout bytes (`RidingElevator`,
//! `EnterGrbkTwDoor`), the field RNG (`MileSandWorm`) and the inventory
//! (`PenguFeedStolen`).
//!
//! Four of them share the `XYRangeJmpTbl` helper (`ps4.asm:112576`), whose
//! rectangles are **half-open** — `cmp.w d2,d0 / bhi` fails the test unless
//! `px >= tx`, and `cmp.w d2,d6 / bls` fails unless `px < tx + span`. That is
//! a different rule from the inline `bcs`/`bhi` pairs elsewhere in the table,
//! which are closed intervals, and the two must not be merged.

use crate::state::Flag;
use crate::trigger::{CustomTrigger, EventIndex, PixelPos, TriggerContext, TriggerResult};

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
        CustomTrigger::RidingElevator => riding_elevator(ctx),
        CustomTrigger::EnterGrbkTwDoor => enter_grbk_tw_door(ctx),
        CustomTrigger::MileSandWorm => mile_sand_worm(ctx),
        CustomTrigger::PenguFeedStolen => pengu_feed_stolen(ctx),
    }
}

/// `$0D RunEvent_RidingElevator` (`ps4.asm:115250`).
///
/// ```text
///     move.w  curr_y_pos(a4), d2
///     addi.w  #$10, d2            ; the standing cell is one row below y
///     ...  jsr GetMapLayoutOffset  ; chunk = (x >> 5, (y + $10) >> 5)
///     cmpi.b  #$53, (a1)
///     bne.w   RunEvent_NoEvent
///     move.w  #$14, (Event_Index).w
/// ```
///
/// No flags and no position box: standing on the open elevator door chunk is
/// the whole test, read from the *live* layout (patches included).
fn riding_elevator(ctx: &TriggerContext<'_>) -> TriggerResult {
    const OPEN_ELEVATOR_CHUNK: u16 = 0x53;
    if ctx.layout_below == Some(OPEN_ELEVATOR_CHUNK) {
        TriggerResult::Fire(EventIndex(0x14))
    } else {
        TriggerResult::NoEvent
    }
}

/// `$22 RunEvent_EnterGrbkTwDoor` (`ps4.asm:115755`).
///
/// ```text
///     ... (y + $10) >> 5 ...  cmpi.b #$3C, (a1) / bne.w RunEvent_NoEvent
///     move.w  #$37, (Event_Index).w
///     ... (y - $10) >> 5 ...  cmpi.b #$3E, (a1) / bne.s  done
///     move.w  #$38, (Event_Index).w
/// ```
///
/// The first read gates the routine; the second only upgrades `$37` to `$38`.
/// An above-row read that falls off the plane cannot equal `$3E` here, so it
/// leaves `$37` (retail would read whatever byte the wrapped offset lands on).
fn enter_grbk_tw_door(ctx: &TriggerContext<'_>) -> TriggerResult {
    const DOOR_CHUNK: u16 = 0x3C;
    const DOOR_ABOVE_CHUNK: u16 = 0x3E;
    if ctx.layout_below != Some(DOOR_CHUNK) {
        return TriggerResult::NoEvent;
    }
    let event = if ctx.layout_above == Some(DOOR_ABOVE_CHUNK) {
        0x38
    } else {
        0x37
    };
    TriggerResult::Fire(EventIndex(event))
}

/// `$5C..$70 RunEvent_MileSandWorm` (`ps4.asm:116449`).
///
/// ```text
///     EventFlag_MileSandWorm ($1B) set      -> no event
///     cmpi.w #$170, curr_x_pos / bhi        -> no event   (x <= $170)
///     cmpi.w #$150, curr_y_pos / bcs        -> no event   (y >= $150)
///     cmpi.w #$2B0, curr_y_pos / bhi        -> no event   (y <= $2B0)
///     jsr     UpdateRNGSeed
///     (RNG_Seed).w & $1F != 0               -> no event
///     move.w  #$71, (Event_Index).w
/// ```
///
/// Comparisons are unsigned and inclusive. The draw happens **only** once the
/// flag and the box pass, and then exactly once — a draw on any other path
/// would shift every later battle roll.
fn mile_sand_worm(ctx: &TriggerContext<'_>) -> TriggerResult {
    const SAND_WORM_FOUGHT: Flag = Flag::event(0x1B);

    if ctx.state.is_set(SAND_WORM_FOUGHT) {
        return TriggerResult::NoEvent;
    }
    let in_box = ctx.at.x <= 0x170 && ctx.at.y >= 0x150 && ctx.at.y <= 0x2B0;
    if !in_box {
        return TriggerResult::NoEvent;
    }
    let word = ctx.rng.borrow_mut().step();
    if word & 0x1F == 0 {
        TriggerResult::Fire(EventIndex(0x71))
    } else {
        TriggerResult::NoEvent
    }
}

/// `$7B RunEvent_PenguFeedStolen` (`ps4.asm:116572`).
///
/// ```text
///     move.b  #ItemID_PenguFeed ($92), d0 / jsr GetItem
///     bne.w   RunEvent_NoEvent        ; GetItem returns 0 when FOUND
///     cmpi.w  #$260, curr_y_pos(a4)
///     bne.w   RunEvent_NoEvent
///     move.w  #$96, (Event_Index).w
/// ```
///
/// Fires when the Feed is *not* in the inventory, by `GetItem`'s own scan
/// ([`Inventory::get_item`](crate::inventory::Inventory::get_item)).
fn pengu_feed_stolen(ctx: &TriggerContext<'_>) -> TriggerResult {
    const PENGU_FEED: u8 = 0x92;
    if ctx.state.inventory().get_item(PENGU_FEED) || ctx.at.y != 0x260 {
        return TriggerResult::NoEvent;
    }
    TriggerResult::Fire(EventIndex(0x96))
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
    use crate::battle::Lcg41;
    use crate::state::GameState;
    use std::cell::RefCell;

    /// A context whose RNG the test does not look at.
    fn ctx<'a>(state: &'a GameState, x: i32, y: i32) -> TriggerContext<'a> {
        let rng: &'static RefCell<Lcg41> = Box::leak(Box::new(RefCell::new(Lcg41::new(1))));
        with_rng(state, x, y, rng)
    }

    fn with_rng<'a>(
        state: &'a GameState,
        x: i32,
        y: i32,
        rng: &'a RefCell<Lcg41>,
    ) -> TriggerContext<'a> {
        TriggerContext {
            state,
            at: PixelPos { x, y },
            standing: None,
            previously_standing: None,
            layout_below: None,
            layout_above: None,
            rng,
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
    fn riding_elevator_needs_the_open_door_chunk_under_the_leader() {
        let state = GameState::new();
        let mut c = ctx(&state, 0, 0);
        c.layout_below = Some(0x53);
        assert_eq!(riding_elevator(&c), TriggerResult::Fire(EventIndex(0x14)));
        for other in [0x52, 0x54, 0x4F] {
            c.layout_below = Some(other);
            assert_eq!(riding_elevator(&c), TriggerResult::NoEvent);
        }
        c.layout_below = None;
        assert_eq!(riding_elevator(&c), TriggerResult::NoEvent, "off the plane");
        // The row above is not consulted.
        c.layout_below = Some(0x4F);
        c.layout_above = Some(0x53);
        assert_eq!(riding_elevator(&c), TriggerResult::NoEvent);
    }

    #[test]
    fn grbk_tower_door_gates_on_3c_below_and_upgrades_on_3e_above() {
        let state = GameState::new();
        let mut c = ctx(&state, 0, 0);
        c.layout_below = Some(0x3C);
        assert_eq!(
            enter_grbk_tw_door(&c),
            TriggerResult::Fire(EventIndex(0x37)),
            "no chunk above: plain door"
        );
        c.layout_above = Some(0x3D);
        assert_eq!(
            enter_grbk_tw_door(&c),
            TriggerResult::Fire(EventIndex(0x37))
        );
        c.layout_above = Some(0x3E);
        assert_eq!(
            enter_grbk_tw_door(&c),
            TriggerResult::Fire(EventIndex(0x38))
        );
        for other in [Some(0x3B), Some(0x3D), None] {
            c.layout_below = other;
            assert_eq!(
                enter_grbk_tw_door(&c),
                TriggerResult::NoEvent,
                "the second read cannot rescue a failed first read"
            );
        }
    }

    /// The seed in `cell`, without disturbing it.
    fn peek(cell: &RefCell<Lcg41>) -> Lcg41 {
        cell.borrow().clone()
    }

    /// A seed whose first `UpdateRNGSeed` draw has exactly `low_five` in its
    /// low five bits, found by search so the tests stay tied to
    /// `Lcg41::step`.
    fn seed_drawing(low_five: u16) -> Lcg41 {
        (1u32..)
            .map(Lcg41::new)
            .find(|seed| {
                let mut probe = seed.clone();
                probe.step() & 0x1F == low_five
            })
            .unwrap()
    }

    #[test]
    fn sand_worm_draws_once_and_fires_only_on_a_zero_low_five() {
        let state = GameState::new();

        let hit = RefCell::new(seed_drawing(0));
        let before = peek(&hit);
        assert_eq!(
            mile_sand_worm(&with_rng(&state, 0x170, 0x150, &hit)),
            TriggerResult::Fire(EventIndex(0x71))
        );
        let mut expected = before.clone();
        expected.step();
        assert_eq!(peek(&hit), expected, "exactly one draw");

        // Bit 4 set and bits 0..3 clear: a mask of $0F would wrongly fire.
        let miss = RefCell::new(seed_drawing(0x10));
        let before = peek(&miss);
        assert_eq!(
            mile_sand_worm(&with_rng(&state, 0x170, 0x2B0, &miss)),
            TriggerResult::NoEvent
        );
        let mut expected = before.clone();
        expected.step();
        assert_eq!(peek(&miss), expected, "a miss still consumed one draw");
    }

    #[test]
    fn sand_worm_box_boundaries_use_the_cartridges_comparisons() {
        let state = GameState::new();
        // Every cell in the box fires with a seed that draws zero, so the
        // draw itself is the observable: untouched seed means "rejected".
        let probe = |x: i32, y: i32| {
            let rng = RefCell::new(seed_drawing(0));
            let before = peek(&rng);
            let result = mile_sand_worm(&with_rng(&state, x, y, &rng));
            let drew = peek(&rng) != before;
            (result, drew)
        };
        let fire = (TriggerResult::Fire(EventIndex(0x71)), true);
        let reject = (TriggerResult::NoEvent, false);
        assert_eq!(probe(0x170, 0x150), fire, "x <= $170, y >= $150");
        assert_eq!(probe(0x171, 0x150), reject, "x = $171 is out (bhi)");
        assert_eq!(probe(0x170, 0x14F), reject, "y = $14F is out (bcs)");
        assert_eq!(probe(0x170, 0x2B0), fire, "y <= $2B0");
        assert_eq!(probe(0x170, 0x2B1), reject, "y = $2B1 is out (bhi)");
        assert_eq!(probe(0, 0x200), fire, "x has no lower bound");
        assert_eq!(probe(0xA0, 0x310), reject, "Mile's arrival row is outside");
    }

    #[test]
    fn sand_worm_flag_blocks_before_any_draw() {
        let mut state = GameState::new();
        state.set(Flag::event(0x1B)).unwrap();
        let rng = RefCell::new(seed_drawing(0));
        let before = peek(&rng);
        assert_eq!(
            mile_sand_worm(&with_rng(&state, 0x100, 0x200, &rng)),
            TriggerResult::NoEvent
        );
        assert_eq!(
            peek(&rng),
            before,
            "the fought flag short-circuits the draw"
        );
    }

    #[test]
    fn pengu_feed_stolen_fires_when_the_feed_is_missing_at_y_260() {
        let mut state = GameState::new();
        let at = |state: &GameState, y: i32| pengu_feed_stolen(&ctx(state, 0x100, y));

        assert_eq!(at(&state, 0x260), TriggerResult::Fire(EventIndex(0x96)));
        assert_eq!(at(&state, 0x25F), TriggerResult::NoEvent, "bne: exact y");
        assert_eq!(at(&state, 0x261), TriggerResult::NoEvent);

        state.inventory_mut().add(0x92).unwrap();
        assert_eq!(
            at(&state, 0x260),
            TriggerResult::NoEvent,
            "the Feed is still in the bag"
        );
    }

    #[test]
    fn pengu_feed_stolen_uses_get_items_count_limited_scan() {
        // Slot 0 emptied by a battle removal: one item left past the hole.
        // GetItem counts 2 non-empty slots and scans only slots 0..2, so the
        // Feed in slot 2 is missed and the trigger fires.
        let mut state = GameState::new();
        let inv = state.inventory_mut();
        inv.add(0x01).unwrap();
        inv.add(0x02).unwrap();
        inv.add(0x92).unwrap();
        inv.remove(0);
        assert!(inv.contains(0x92));
        assert_eq!(
            pengu_feed_stolen(&ctx(&state, 0, 0x260)),
            TriggerResult::Fire(EventIndex(0x96))
        );
    }
}
