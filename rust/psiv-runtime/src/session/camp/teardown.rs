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

/// One window on the cartridge's stack: its `WinGroup_Menu` record
/// (`ps4.asm:140181` onwards) as screen cells, the frame included (each size
/// is the record's byte plus one).
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
    /// A record from its four bytes: width, height, x, y.
    const fn from_bytes(index: u8, width: u8, height: u8, x: u8, y: u8) -> WindowRecord {
        WindowRecord {
            index,
            x: x as i32,
            y: y as i32,
            width: width as i32 + 1,
            height: height as i32 + 1,
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

/// `WinGroup_Menu[0]`, the root options (`ps4.asm:140182`).
pub(super) const ROOT: WindowRecord = WindowRecord::from_bytes(0x00, 0x08, 0x0E, 0x04, 0x02);
/// `WinGroup_Menu[1]`, the meseta (`ps4.asm:140185`).
pub(super) const MESETA: WindowRecord = WindowRecord::from_bytes(0x01, 0x0C, 0x02, 0x03, 0x17);
/// `WinGroup_Menu[$2F]`, the item's USE/LOOK/DSCD (`ps4.asm:140323`).
pub(super) const ITEM_ACTION: WindowRecord = WindowRecord::from_bytes(0x2F, 0x07, 0x06, 0x13, 0x02);
/// The item teleport message's record, `WinGroup_Menu[$45]`
/// (`ps4.asm:140389`), which runs `Win_ItemTeleportMsg`.
pub(super) const ITEM_TELEPORT_MESSAGE: WindowRecord =
    WindowRecord::from_bytes(0x45, 0x19, 0x04, 0x07, 0x15);
/// The technique teleport message's record, `WinGroup_Menu[$75]`
/// (`ps4.asm:140533`), which runs `Win_TechTeleportMsg`.
pub(super) const TECH_TELEPORT_MESSAGE: WindowRecord =
    WindowRecord::from_bytes(0x75, 0x19, 0x04, 0x07, 0x15);

/// The party's stats, `WinGroup_Menu[$02..$06]` (`ps4.asm:140188-140200`):
/// `CalcWinIDAndCharNum` picks `$02` for five members up to `$06` for one,
/// five rows each.
pub(super) fn party_stats(members: usize) -> WindowRecord {
    let members = members.clamp(1, 5) as u8;
    WindowRecord::from_bytes(0x07 - members, 0x0B, 5 * members, 0x1A, 0x01)
}

/// Page `page` of a list of `count` rows, two cells a row, eight rows a page:
/// `Win_ItemList` (`$07..$2E`, x 2 to 6, `ps4.asm:140203-140320`) and
/// `Win_TechList` (`$51..$60`, x 8 and 9, `:140425-140470`). The record is the
/// row count's (`MenuOptions_Item`, `ps4.asm:117321-117349`); a full page is
/// the first record of its column.
fn list_page(first: u8, width: u8, x: u8, count: usize, page: usize) -> WindowRecord {
    let rows = count.saturating_sub(page * 8).clamp(1, 8) as u8;
    let column = page as u8;
    WindowRecord::from_bytes(
        first + column * 8 + (8 - rows),
        width,
        2 * rows + 1,
        x + column,
        0x03,
    )
}

/// The pack's page `page`, `count` items when ITEM opened.
pub(super) fn item_list(count: usize, page: usize) -> WindowRecord {
    list_page(0x07, 0x0D, 0x02, count, page)
}

/// The caster's technique list, page `page` of `count`.
pub(super) fn tech_list(count: usize, page: usize) -> WindowRecord {
    list_page(0x51, 0x0B, 0x08, count, page)
}

/// `Win_TechCharList`, `$4D` for five members to `$50` for two
/// (`ps4.asm:140413-140422`); one member skips it (`MenuOptions_Tech`,
/// `ps4.asm:117360-117368`).
pub(super) fn tech_casters(members: usize) -> WindowRecord {
    let members = members.clamp(2, 5) as u8;
    WindowRecord::from_bytes(0x52 - members, 0x08, 2 * members + 1, 0x02, 0x0D)
}

/// Town-list page `page` with `rows` towns: `$36..$44` for a TELEPIPE
/// (`ps4.asm:140344-140386`), `$66` onwards for RYUKA, at (18,13) and one cell
/// further right for each later page.
pub(super) fn town_page(first: u8, page: usize, rows: usize) -> WindowRecord {
    let rows = rows.clamp(1, 5) as u8;
    let column = page as u8;
    WindowRecord::from_bytes(
        first + column * 5 + (5 - rows),
        0x0D,
        2 * rows + 1,
        0x12 + column,
        0x0D,
    )
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

    /// Tape 36's stack after ITEM, the TELEPIPE, USE and MILE, Chaz alone
    /// with one item: the first opened first.
    fn tape_36() -> Vec<WindowRecord> {
        vec![
            ROOT,
            party_stats(1),
            MESETA,
            item_list(1, 0),
            ITEM_ACTION,
            town_page(0x36, 0, 5),
            ITEM_TELEPORT_MESSAGE,
        ]
    }

    #[test]
    fn the_records_are_tape_36s_windows() {
        let indices: Vec<u8> = tape_36().iter().map(|window| window.index).collect();
        assert_eq!(indices, [0x00, 0x06, 0x01, 0x0E, 0x2F, 0x36, 0x45]);
        assert_eq!(
            ITEM_TELEPORT_MESSAGE.rect(),
            WindowRect {
                x: 7,
                y: 21,
                width: 26,
                height: 5
            }
        );
        assert_eq!(item_list(9, 1).index, 0x0F + 7, "a second page of one");
        assert_eq!(item_list(8, 0).index, 0x07, "a full page");
        assert_eq!(town_page(0x36, 2, 1).index, 0x44);
        assert_eq!(tech_casters(2).index, 0x50);
        assert_eq!(tech_casters(5).index, 0x4D);
        assert_eq!(tech_list(1, 0).index, 0x58);
        assert_eq!(party_stats(5).index, 0x02);
    }

    /// `Windows_Opened_Num` on tape 36: 6 from 8355, 5 from 8363, 4 from
    /// 8368, 3 from 8371, 2 from 8376, 1 from 8381, 0 from 8385; the fade
    /// starts on 8390.
    #[test]
    fn each_window_takes_the_frames_tape_36_measured() {
        let frames: Vec<u16> = tape_36()
            .iter()
            .rev()
            .map(|window| window.close_frames())
            .collect();
        assert_eq!(frames, [8, 5, 3, 5, 5, 4, 4]);
        let teardown = WindowTeardown::new(tape_36());
        assert_eq!(teardown.frames(), 8390 - 8355);
    }

    /// The message's close on tape 36, frames 8355 to 8363: the text until
    /// the frame after the acknowledgement, then a blank window 26 cells wide
    /// at column 7, four cells narrower and two further right each frame, and
    /// gone on 8363.
    #[test]
    fn the_message_narrows_as_tape_36_draws_it() {
        let mut teardown = WindowTeardown::new(tape_36());
        let at = |teardown: &WindowTeardown| teardown.view();
        assert_eq!(at(&teardown).open.len(), 7, "8355: every window");
        assert_eq!(at(&teardown).closing, None);
        let mut shown = Vec::new();
        for _ in 8356..=8363 {
            teardown.tick();
            let view = at(&teardown);
            shown.push(
                view.closing
                    .map(|(window, rect)| (window.index, rect.x, rect.width)),
            );
            assert_eq!(view.open.len(), 6);
        }
        assert_eq!(
            shown,
            [
                Some((0x45, 7, 26)),
                Some((0x45, 9, 22)),
                Some((0x45, 11, 18)),
                Some((0x45, 13, 14)),
                Some((0x45, 15, 10)),
                Some((0x45, 17, 6)),
                Some((0x45, 19, 2)),
                None,
            ]
        );
        teardown.tick();
        assert_eq!(
            at(&teardown).closing.map(|(window, _)| window.index),
            Some(0x36),
            "8364: the town list"
        );
    }

    #[test]
    fn the_menu_exits_after_the_last_window_and_one_loop_frame() {
        let mut teardown = WindowTeardown::new(tape_36());
        let mut frame = 8355;
        while !teardown.tick() {
            frame += 1;
        }
        frame += 1;
        assert_eq!(frame, 8390, "MenuExit_Teleport");
        let view = teardown.view();
        assert!(view.open.is_empty() && view.closing.is_none());
    }

    /// An odd width ends on a pass of two cells: the `addq` before
    /// `loc_688BA` puts back the cell the last subtraction took.
    #[test]
    fn an_odd_width_ends_two_cells_wide() {
        let action = ITEM_ACTION; // 8 cells: passes 8, 6, 4, 2
        assert_eq!(action.close_frames(), 3);
        assert_eq!(action.closing_rect(1).unwrap().width, 4);
        let root = ROOT; // 9 cells: passes 9, 7, 5, 3, 2
        assert_eq!(root.close_frames(), 4);
        assert_eq!(root.closing_rect(2).unwrap().width, 2);
        assert_eq!(root.closing_rect(2).unwrap().x, root.x + 3, "the centre");
        assert_eq!(root.closing_rect(3), None);
    }
}
