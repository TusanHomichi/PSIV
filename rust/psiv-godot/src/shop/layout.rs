//! Retail geometry for the shop and inn windows.
//!
//! The rectangles are the camera-remapped Piata decode in
//! `docs/camp/SHOP_LAYOUT_DECODED.md`, the rendered outer rectangle of each
//! window group in the 40x28 visible cell frame. `chrome` blits the frames and
//! `draw` decides what each one shows.

pub(super) const CELL: f32 = 8.0;
pub(super) const MONEY: CellRect = CellRect::new(2, 2, 13, 3);
pub(super) const PORTRAIT: CellRect = CellRect::new(6, 7, 6, 6);
pub(super) const MAIN_MESSAGE: CellRect = CellRect::new(3, 20, 34, 6);
pub(super) const BUY_SELL_MENU: CellRect = CellRect::new(5, 14, 8, 5);
pub(super) const BUY_LIST: CellRect = CellRect::new(14, 6, 20, 3);
pub(super) const BUY_QUANTITY: CellRect = CellRect::new(6, 13, 7, 5);
pub(super) const SELL_ITEM: CellRect = CellRect::new(18, 2, 14, 3);
pub(super) const SELL_PANE: CellRect = CellRect::new(7, 13, 7, 5);
pub(super) const INN_CONFIRM: CellRect = CellRect::new(6, 14, 7, 5);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CellRect {
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) w: i32,
    pub(super) h: i32,
}

impl CellRect {
    pub(super) const fn new(x: i32, y: i32, w: i32, h: i32) -> CellRect {
        CellRect { x, y, w, h }
    }
}
