//! `DestroyAllWindows`, frame by frame: the camp's windows closing one at a
//! time before a teleport.
//!
//! The teleport message's acknowledgement (`Win_ItemTeleportMsg`'s
//! `loc_5CB16`, `ps4.asm:124235-124245`; `Win_TechTeleportMsg`, `:129447`)
//! calls `DestroyAllWindows` (`ps4.asm:117527-117543`), which destroys every
//! open window, the last opened first, each through `Window_Destroy`'s
//! animated close (`loc_6881E`, `ps4.asm:139965-140054`). That close blanks the
//! window, then redraws it two cells narrower on each pass, one cell further
//! right, until it is two cells wide; with `Window_Render_Mode` bit 1 set (as
//! `DestroyAllWindows` sets it) only every other pass waits for a frame
//! (`bchg #2`), and a last frame puts the background back. A window `w` cells
//! wide (its record's width byte plus one) therefore takes
//! `ceil(ceil(w / 2) / 2) + 1` frames. `Field_MenuLoop` (`ps4.asm:117143-117148`)
//! then spends one more frame before `Field_MenuExit` sees no window and
//! `MenuExit_Teleport` starts the fade.
//!
//! Measured on tape 36 (`oracle/tapes/36_teleport_windows.tape`):
//! `Windows_Opened_Num` falls from 7 on the acknowledgement, frame 8355, and
//! the seven windows take 8, 5, 3, 5, 5, 4 and 4 frames (`$45`, `$36`, `$2F`,
//! `$0E`, `$01`, `$06`, `$00`), the formula's counts for their widths. Each
//! step is on screen one frame after the pass that drew it, so the frame after
//! the acknowledgement shows the first blank window and the last step (the
//! root's background back) shows on 8389, the frame `Field_MenuLoop` spends.

use psiv_data::{GameData, MenuWindow};

/// One window on the cartridge's stack: its `WinGroup_Menu` record as screen
/// cells, the frame included.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowRecord {
    /// The record's index in `WinGroup_Menu`.
    pub index: u8,
    /// The window's left column.
    pub x: i32,
    /// The window's top row.
    pub y: i32,
    /// Its width in cells.
    pub width: i32,
    /// Its height in cells.
    pub height: i32,
}

/// A rectangle of screen cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowRect {
    /// Left column.
    pub x: i32,
    /// Top row.
    pub y: i32,
    /// Width in cells.
    pub width: i32,
    /// Height in cells.
    pub height: i32,
}

impl WindowRecord {
    /// The pack's record (`dialogue/window.json`'s `menu_windows`).
    #[must_use]
    pub fn from_pack(window: &MenuWindow) -> WindowRecord {
        let cells = |value: u32| i32::try_from(value).unwrap_or(i32::MAX);
        WindowRecord {
            index: u8::try_from(window.index).unwrap_or(u8::MAX),
            x: cells(window.x_cell),
            y: cells(window.y_cell),
            width: cells(window.width_cells),
            height: cells(window.height_cells),
        }
    }

    /// The frames `loc_6881E` spends closing this window.
    #[must_use]
    pub fn close_frames(self) -> u16 {
        let passes = (self.width as u16).div_ceil(2);
        passes.div_ceil(2) + 1
    }

    /// The blank window on close step `step`, or `None` on the last step,
    /// which restores the background. Step `j` shows pass `2j`: two cells
    /// narrower per pass, never under two, and one cell further right per pass
    /// until the left edge reaches the centre the routine computed.
    #[must_use]
    pub fn closing_rect(self, step: u16) -> Option<WindowRect> {
        if step + 1 >= self.close_frames() {
            return None;
        }
        let pass = 2 * i32::from(step);
        Some(WindowRect {
            x: self.x + pass.min(self.width / 2 - 1),
            y: self.y,
            width: (self.width - 2 * pass).max(2),
            height: self.height,
        })
    }

    /// The window's own rectangle.
    #[must_use]
    pub fn rect(self) -> WindowRect {
        WindowRect {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
        }
    }
}

// The records each routine passes `Window_Create`. Their geometry is the
// pack's (`dialogue/window.json`'s `menu_windows`, read from `WinGroup_Menu`);
// these are only the indices the routines compute.

/// The root options (`Win_MenuOptions`, `ps4.asm:117201-117247`).
pub(super) const ROOT: u8 = 0x00;
/// The meseta, opened with the root.
pub(super) const MESETA: u8 = 0x01;
/// The item's USE/LOOK/DSCD (`Win_ItemAction`).
pub(super) const ITEM_ACTION: u8 = 0x2F;
/// `Win_ItemTeleportMsg`'s window.
pub(super) const ITEM_TELEPORT_MESSAGE: u8 = 0x45;
/// `Win_TechTeleportMsg`'s window.
pub(super) const TECH_TELEPORT_MESSAGE: u8 = 0x75;

/// The party's stats: `CalcWinIDAndCharNum` counts one record down from
/// `$06` for each member past the first.
pub(super) fn party_stats(members: usize) -> u8 {
    0x07 - members.clamp(1, 5) as u8
}

/// Page `page` of a list of `count` rows, eight a page, each page eight
/// records on from the last: the full page is the column's first record and
/// a shorter one is `8 - rows` further (`MenuOptions_Item`'s `subq #7` and
/// `neg`, `ps4.asm:117321-117349`).
fn list_page(first: u8, count: usize, page: usize) -> u8 {
    let rows = count.saturating_sub(page * 8).clamp(1, 8) as u8;
    first + page as u8 * 8 + (8 - rows)
}

/// The pack's page `page`, `count` items when ITEM opened (`Win_ItemList`).
pub(super) fn item_list(count: usize, page: usize) -> u8 {
    list_page(0x07, count, page)
}

/// The caster's technique list, page `page` of `count` (`Win_TechList`).
pub(super) fn tech_list(count: usize, page: usize) -> u8 {
    list_page(0x51, count, page)
}

/// `Win_TechCharList`, one record down from `$50` (two members) for each
/// member more; one member skips it (`MenuOptions_Tech`,
/// `ps4.asm:117360-117368`).
pub(super) fn tech_casters(members: usize) -> u8 {
    0x52 - members.clamp(2, 5) as u8
}

/// Town-list page `page` with `rows` towns, from `first`: `$36` for a
/// TELEPIPE (`Win_TelepipePlaceList`), `$66` for RYUKA, five records a page.
pub(super) fn town_page(first: u8, page: usize, rows: usize) -> u8 {
    first + page as u8 * 5 + (5 - rows.clamp(1, 5) as u8)
}

/// The pack's records for `indices`, in order. A pack without them (a
/// synthetic one) has no stack to close.
pub(super) fn records(data: &GameData, indices: &[u8]) -> Vec<WindowRecord> {
    let Some(dialogue) = data.dialogue() else {
        return Vec::new();
    };
    indices
        .iter()
        .filter_map(|&index| dialogue.window.menu_windows.get(usize::from(index)))
        .map(WindowRecord::from_pack)
        .collect()
}

/// `DestroyAllWindows` over a stack, counted in frames from the
/// acknowledgement (frame 0).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowTeardown {
    /// The open windows, the first opened first.
    stack: Vec<WindowRecord>,
    elapsed: u16,
}

/// What the screen shows on one teardown frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeardownView {
    /// The windows still drawn whole, the first opened first.
    pub open: Vec<WindowRecord>,
    /// The window closing, and its blank rectangle this frame.
    pub closing: Option<(WindowRecord, WindowRect)>,
}

impl WindowTeardown {
    pub(super) fn new(stack: Vec<WindowRecord>) -> WindowTeardown {
        WindowTeardown { stack, elapsed: 0 }
    }

    /// The windows on the stack when the teardown began.
    #[must_use]
    pub fn stack(&self) -> &[WindowRecord] {
        &self.stack
    }

    /// The close steps of every window, last opened first.
    fn close_frames(&self) -> u16 {
        self.stack.iter().map(|window| window.close_frames()).sum()
    }

    /// The frames from the acknowledgement to the frame `MenuExit_Teleport`
    /// runs on: every close step, then `Field_MenuLoop`'s frame.
    #[must_use]
    pub fn frames(&self) -> u16 {
        self.close_frames() + 1
    }

    /// Advances one frame; true on the frame the menu exits.
    pub(super) fn tick(&mut self) -> bool {
        self.elapsed = self.elapsed.saturating_add(1);
        self.finished()
    }

    pub(super) fn finished(&self) -> bool {
        self.elapsed >= self.frames()
    }

    /// The screen this frame. The acknowledgement's own frame still shows
    /// every window; frame `n` shows close step `n - 1`.
    #[must_use]
    pub fn view(&self) -> TeardownView {
        let Some(mut step) = self.elapsed.checked_sub(1) else {
            return TeardownView {
                open: self.stack.clone(),
                closing: None,
            };
        };
        for (at, window) in self.stack.iter().enumerate().rev() {
            let frames = window.close_frames();
            if step < frames {
                return TeardownView {
                    open: self.stack[..at].to_vec(),
                    closing: window.closing_rect(step).map(|rect| (*window, rect)),
                };
            }
            step -= frames;
        }
        TeardownView {
            open: Vec::new(),
            closing: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(index: u8, x: i32, width: i32) -> WindowRecord {
        WindowRecord {
            index,
            x,
            y: 21,
            width,
            height: 5,
        }
    }

    #[test]
    fn the_indices_are_the_routines() {
        assert_eq!(item_list(1, 0), 0x0E, "one item: the shortest page");
        assert_eq!(item_list(8, 0), 0x07, "a full page");
        assert_eq!(item_list(9, 1), 0x16, "a second page of one");
        assert_eq!(town_page(0x36, 0, 5), 0x36);
        assert_eq!(town_page(0x36, 2, 1), 0x44);
        assert_eq!(tech_casters(2), 0x50);
        assert_eq!(tech_casters(5), 0x4D);
        assert_eq!(tech_list(1, 0), 0x58);
        assert_eq!(party_stats(1), 0x06);
        assert_eq!(party_stats(5), 0x02);
    }

    /// A window 26 cells wide: the blank window at full width, then four
    /// cells narrower and two further right each frame down to two cells,
    /// then the background, eight frames in all.
    #[test]
    fn a_window_narrows_two_cells_a_pass_two_passes_a_frame() {
        let message = window(0, 7, 26);
        assert_eq!(message.close_frames(), 8);
        let shown: Vec<_> = (0..8)
            .map(|step| message.closing_rect(step).map(|rect| (rect.x, rect.width)))
            .collect();
        assert_eq!(
            shown,
            [
                Some((7, 26)),
                Some((9, 22)),
                Some((11, 18)),
                Some((13, 14)),
                Some((15, 10)),
                Some((17, 6)),
                Some((19, 2)),
                None,
            ]
        );
    }

    /// An odd width ends on a pass of two cells: the `addq` before
    /// `loc_688BA` puts back the cell the last subtraction took.
    #[test]
    fn an_odd_width_ends_two_cells_wide() {
        let even = window(0, 0, 8); // passes 8, 6, 4, 2
        assert_eq!(even.close_frames(), 3);
        assert_eq!(even.closing_rect(1).unwrap().width, 4);
        let odd = window(0, 4, 9); // passes 9, 7, 5, 3, 2
        assert_eq!(odd.close_frames(), 4);
        assert_eq!(odd.closing_rect(2).unwrap().width, 2);
        assert_eq!(odd.closing_rect(2).unwrap().x, 4 + 3, "the centre");
        assert_eq!(odd.closing_rect(3), None);
    }

    /// The stack closes last-opened first; the acknowledgement's frame still
    /// shows every window, and the menu exits a frame after the last step.
    #[test]
    fn the_stack_closes_from_the_top_then_spends_one_more_frame() {
        let stack = vec![window(0, 0, 9), window(1, 10, 26)];
        let mut teardown = WindowTeardown::new(stack);
        assert_eq!(teardown.frames(), 4 + 8 + 1);
        assert_eq!(teardown.view().open.len(), 2);
        assert_eq!(teardown.view().closing, None);
        teardown.tick();
        let view = teardown.view();
        assert_eq!(
            (view.open.len(), view.closing.map(|(w, _)| w.index)),
            (1, Some(1))
        );
        for _ in 0..8 {
            teardown.tick();
        }
        assert_eq!(teardown.view().closing.map(|(w, _)| w.index), Some(0));
        let mut frames = 9;
        while !teardown.tick() {
            frames += 1;
        }
        assert_eq!(frames + 1, teardown.frames());
        assert!(teardown.view().open.is_empty() && teardown.view().closing.is_none());
    }
}
