//! Event triggers: the `RunEventsJmpTbl` checks as data.
//!
//! # How the cartridge runs them
//!
//! `RunEvents` (`ps4.asm:114979`) walks the current map's event list — a
//! `$FF`-terminated array of table indices at `Map_Events_Addr` — and calls
//! each index's routine until one fires:
//!
//! ```text
//! RunEvents:
//!     movea.l (Map_Events_Addr).w, a0
//!     lea     (Character_1).w, a1
//!     tst.w   x_step_duration(a1)     ; return if we are moving to a different tile
//!     bne.w   .f1
//!     tst.w   y_step_duration(a1)
//!     bne.w   .f1
//! ```
//!
//! Two things follow. **Order matters** — the first listed index whose check
//! passes wins, and the scan stops there. And the whole scan is skipped while
//! the leader is mid-step, so triggers are evaluated only when the party is
//! tile-aligned: the same "on landing" model as map transitions.
//!
//! On a hit the routine writes `Event_Index` and the caller flips
//! `Game_Mode_Routine` to `$C` (`FieldRoutine_Event`).
//!
//! # Coordinates are pixels
//!
//! Every position test reads the integer word of `curr_x_pos`/`curr_y_pos`, so
//! the literals are **pixel** positions — `$260`, `$F0` — not cells. Convert
//! with [`PixelPos::from_cell`], which applies the standing-cell shift
//! `GetChunkAndCollision` uses (`addi.w #$10,d6` before the shift down, so the
//! occupied cell is one row below `curr_y_pos / 16`).
//!
//! # Comparison semantics
//!
//! The whole `RunEvent_*` block uses only `beq`, `bne`, `bcs` and `bhi` — no
//! signed branch appears anywhere in it. Since every branch jumps *away* to
//! `RunEvent_NoEvent`, `cmpi.w #V,pos` + `bcs` passes on `pos >= V` and `+
//! bhi` passes on `pos <= V`, both **unsigned and inclusive**. A pair of them
//! is therefore a closed interval. The `XYRangeJmpTbl` helper used by four
//! custom routines is different — its rectangles are half-open `[t, t+span)` —
//! which is why those stay explicit variants instead of folding into
//! [`AxisPredicate::Between`].

use crate::geom::{CELL_PIXELS, Cell};
use crate::state::{Flag, GameState};
use crate::trigger_custom;

/// A pixel position, as the cartridge's `curr_x_pos`/`curr_y_pos` word pair.
///
/// Signed because [`PixelPos::from_cell`] subtracts a cell from the row and
/// the top row of a map lands at `-16`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PixelPos {
    /// Horizontal pixel position.
    pub x: i32,
    /// Vertical pixel position.
    pub y: i32,
}

impl PixelPos {
    /// The pixel position a party standing on `cell` reports.
    ///
    /// `GetChunkAndCollision` adds `$10` to Y before shifting down to a cell,
    /// so the occupied cell is one row below `curr_y_pos / 16` and the inverse
    /// subtracts it back. X has no such shift.
    ///
    /// Worth pinning by example: the Alys trigger wants `y >= $F0` and
    /// `x == $260`, which is cell row 16 and column 38.
    #[must_use]
    pub const fn from_cell(cell: Cell) -> PixelPos {
        PixelPos {
            x: cell.x as i32 * CELL_PIXELS,
            y: (cell.y as i32 - 1) * CELL_PIXELS,
        }
    }
}

/// A test on one axis. Bounds are inclusive, comparisons unsigned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisPredicate {
    /// No test on this axis.
    Any,
    /// `cmpi.w #V,pos` + `bne` away — passes on `pos == V`.
    Exact(i32),
    /// `cmpi.w #V,pos` + `bcs` away — passes on `pos >= V`.
    AtLeast(i32),
    /// `cmpi.w #V,pos` + `bhi` away — passes on `pos <= V`.
    AtMost(i32),
    /// Both of the above — the closed interval `[lo, hi]`.
    Between(i32, i32),
}

impl AxisPredicate {
    /// Whether `value` satisfies the predicate.
    #[must_use]
    pub const fn matches(self, value: i32) -> bool {
        match self {
            AxisPredicate::Any => true,
            AxisPredicate::Exact(v) => value == v,
            AxisPredicate::AtLeast(v) => value >= v,
            AxisPredicate::AtMost(v) => value <= v,
            AxisPredicate::Between(lo, hi) => value >= lo && value <= hi,
        }
    }
}

/// The position half of a trigger condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PositionPredicate {
    /// Test on `curr_x_pos`.
    pub x: AxisPredicate,
    /// Test on `curr_y_pos`.
    pub y: AxisPredicate,
}

impl PositionPredicate {
    /// No position test at all — the commonest shape by far.
    pub const ANYWHERE: PositionPredicate = PositionPredicate {
        x: AxisPredicate::Any,
        y: AxisPredicate::Any,
    };

    /// Builds a predicate.
    #[must_use]
    pub const fn new(x: AxisPredicate, y: AxisPredicate) -> PositionPredicate {
        PositionPredicate { x, y }
    }

    /// Whether `at` satisfies both axes.
    #[must_use]
    pub const fn matches(self, at: PixelPos) -> bool {
        self.x.matches(at.x) && self.y.matches(at.y)
    }
}

/// What a firing trigger writes into `Event_Index`.
///
/// `FieldRoutine_Event` (`ps4.asm:120542`) tests bit 15: set means a cutscene
/// index (masked with `$7FFF`), clear means a plain event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventIndex(pub u16);

impl EventIndex {
    /// Whether this dispatches through the cutscene table.
    #[must_use]
    pub const fn is_cutscene(self) -> bool {
        self.0 & 0x8000 != 0
    }

    /// The index into whichever pointer table applies.
    #[must_use]
    pub const fn pointer_index(self) -> u16 {
        self.0 & 0x7FFF
    }
}

/// A trigger that is not a flags-plus-position formula.
///
/// Kept as named variants rather than approximated, because each one does
/// something the formula cannot express. Six are implemented exactly; four
/// need state this crate does not model and report themselves as unsupported
/// rather than guessing — see [`Trigger::evaluate`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CustomTrigger {
    /// `$0D` — reads the map *layout* byte at the standing cell and requires
    /// `$53`. Needs plane bytes the engine is not given.
    RidingElevator,
    /// `$22` — two layout-byte reads, `$3C` below and optionally `$3E` above.
    /// Needs plane bytes the engine is not given.
    EnterGrbkTwDoor,
    /// `$12` — fires on the rising edge of standing on collision type 2.
    Recovery,
    /// `$0E` — two flag-selected half-open probes.
    VahFortMovingPlatform,
    /// `$0F` — four flag-selected half-open probes.
    WpnPlntMovingPlatform,
    /// `$10` — twelve half-open probes with flag-swapped outcomes.
    VahFortConveyorBelt,
    /// `$11` — twelve half-open probes with flag-swapped outcomes.
    WpnPlntConveyorBelt,
    /// `$35` — two complementary regions with different flag sets.
    CarnivorousTrees,
    /// `$5C..$70` — region test plus a 1-in-32 draw that also *advances* the
    /// global RNG every frame the region matches. Needs the ported RNG.
    MileSandWorm,
    /// `$7B` — requires the Pengu Feed in the inventory. Needs an inventory.
    PenguFeedStolen,
}

/// Why a custom trigger could not be decided here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Unsupported {
    /// Needs the map's layout (chunk-index) plane, which the engine does not
    /// carry — it holds collision, not layout.
    MapLayoutBytes,
    /// Needs the cartridge's RNG, not yet ported (`UpdateRNGSeed`).
    Rng,
    /// Needs the party's inventory, which this crate does not model.
    Inventory,
}

/// One entry of `RunEventsJmpTbl`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// A stub that never fires: `moveq #0,d7 / rts`.
    Never,
    /// `$4D` — fires unconditionally and **writes no `Event_Index`**, so the
    /// game enters event mode on whatever value was already there. Reproduced
    /// as its own variant because calling it a no-op would be wrong.
    AlwaysWithoutIndex,
    /// The formulaic shape: flags plus a position predicate.
    Condition(Condition),
    /// Anything else.
    Custom(CustomTrigger),
}

/// The formulaic trigger: flag tests and a position test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Condition {
    /// Flags that must be set.
    pub require_set: &'static [Flag],
    /// Flags that must be clear.
    pub require_clear: &'static [Flag],
    /// Where the party must be standing.
    pub position: PositionPredicate,
    /// What gets written to `Event_Index`.
    pub event: EventIndex,
}

/// Everything a trigger check can read.
///
/// Position is the party's pixel position; the two collision fields mirror
/// `Tile_Collision_Standing` / `Saved_Tile_Collision_Standing` and are only
/// read by [`CustomTrigger::Recovery`] and the two platform routines.
#[derive(Debug, Clone, Copy)]
pub struct TriggerContext<'a> {
    /// The persistent state.
    pub state: &'a GameState,
    /// Where the party is standing, in pixels.
    pub at: PixelPos,
    /// The collision type of the cell being stood on, as a raw 4-bit value.
    pub standing: Option<u8>,
    /// The previous such value, for edge detection.
    pub previously_standing: Option<u8>,
}

/// The result of checking one trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerResult {
    /// The check did not pass.
    NoEvent,
    /// The check passed; run this event.
    Fire(EventIndex),
    /// The check passed but writes no index — see
    /// [`Trigger::AlwaysWithoutIndex`].
    FireWithoutIndex,
    /// This entry cannot be decided here. Never silently treated as `NoEvent`.
    Unsupported(CustomTrigger, Unsupported),
}

impl Trigger {
    /// Runs this trigger's check.
    #[must_use]
    pub fn evaluate(&self, ctx: &TriggerContext<'_>) -> TriggerResult {
        match self {
            Trigger::Never => TriggerResult::NoEvent,
            Trigger::AlwaysWithoutIndex => TriggerResult::FireWithoutIndex,
            Trigger::Condition(condition) => condition.evaluate(ctx),
            Trigger::Custom(custom) => trigger_custom::evaluate(*custom, ctx),
        }
    }
}

impl Condition {
    /// Runs the flag tests then the position test, in the cartridge's order.
    #[must_use]
    pub fn evaluate(&self, ctx: &TriggerContext<'_>) -> TriggerResult {
        if !self.matches(ctx) {
            return TriggerResult::NoEvent;
        }
        TriggerResult::Fire(self.event)
    }

    /// Whether the condition holds, without producing a result.
    #[must_use]
    pub fn matches(&self, ctx: &TriggerContext<'_>) -> bool {
        self.require_set.iter().all(|&f| ctx.state.is_set(f))
            && self.require_clear.iter().all(|&f| ctx.state.is_clear(f))
            && self.position.matches(ctx.at)
    }
}

/// Evaluates a map's event list in order and returns the first hit.
///
/// `indices` is the map's `$FF`-terminated `Map_Events_Addr` list with the
/// terminator already stripped — the pack carries it per map. The scan stops
/// at the first entry that fires, exactly as `RunEvents` does, so list order
/// is behaviour and must be preserved from the data.
///
/// Entries that cannot be decided here are returned as-is so a caller can
/// handle or log them; the scan continues past them, which matches the
/// cartridge only when the undecidable check would have failed. That caveat is
/// documented on [`TriggerResult::Unsupported`] and there are exactly four such
/// routines.
#[must_use]
pub fn evaluate_list(
    table: &[Trigger],
    indices: &[u8],
    ctx: &TriggerContext<'_>,
) -> Option<(u8, TriggerResult)> {
    let mut unsupported = None;
    for &index in indices {
        let Some(trigger) = table.get(usize::from(index)) else {
            continue;
        };
        match trigger.evaluate(ctx) {
            TriggerResult::NoEvent => {}
            TriggerResult::Unsupported(custom, why) => {
                if unsupported.is_none() {
                    unsupported = Some((index, TriggerResult::Unsupported(custom, why)));
                }
            }
            hit => return Some((index, hit)),
        }
    }
    unsupported
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_conversion_matches_the_standing_cell_shift() {
        // The Alys trigger's literals: y >= $F0 is row 16, x == $260 is col 38.
        let at = PixelPos::from_cell(Cell::new(38, 16));
        assert_eq!(at.x, 0x260);
        assert_eq!(at.y, 0xF0);
    }

    #[test]
    fn the_top_row_sits_one_cell_above_the_origin() {
        assert_eq!(PixelPos::from_cell(Cell::new(0, 0)).y, -16);
        assert_eq!(PixelPos::from_cell(Cell::new(0, 1)).y, 0);
    }

    #[test]
    fn axis_bounds_are_inclusive() {
        assert!(AxisPredicate::AtLeast(0xF0).matches(0xF0));
        assert!(!AxisPredicate::AtLeast(0xF0).matches(0xEF));
        assert!(AxisPredicate::AtMost(0x280).matches(0x280));
        assert!(!AxisPredicate::AtMost(0x280).matches(0x281));
        assert!(AxisPredicate::Between(0x1E0, 0x1F0).matches(0x1E0));
        assert!(AxisPredicate::Between(0x1E0, 0x1F0).matches(0x1F0));
        assert!(!AxisPredicate::Between(0x1E0, 0x1F0).matches(0x1F1));
    }

    #[test]
    fn event_index_splits_cutscenes_off_bit_15() {
        assert!(!EventIndex(3).is_cutscene());
        assert_eq!(EventIndex(3).pointer_index(), 3);
        assert!(EventIndex(0x8009).is_cutscene());
        assert_eq!(EventIndex(0x8009).pointer_index(), 9);
    }
}
