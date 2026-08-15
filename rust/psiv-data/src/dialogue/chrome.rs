//! What the dialogue window is made of: the glyph strip (`font.json`), the
//! frame tiles and geometry (`window.json`), and the `$F4` portrait table
//! (`portraits.json`).
//!
//! Pure description. Nothing here knows how a renderer draws it, but between
//! the geometry rule, the role flips and the two window rects it says exactly
//! what has to be drawn where.

use std::collections::BTreeMap;
use std::fmt;

use serde::Deserialize;

// ---------------------------------------------------------------------------
// font.json
// ---------------------------------------------------------------------------

/// `dialogue/font.json`: the 8x16 glyph strip.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FontFile {
    /// Pack format; checked against [`DIALOGUE_FORMAT_VERSION`].
    pub format_version: u32,
    /// The strip, pack-root-relative. White glyphs on a transparent ground:
    /// the cartridge fills the window with colour index $E first and draws the
    /// glyphs over it, so compositing reproduces it.
    pub png: String,
    /// Character to font byte, which is also the cell index in the strip.
    pub by_char: BTreeMap<char, u8>,
    /// Every cell of the strip, including the blank and the unmapped ones.
    pub glyphs: Vec<Glyph>,
    /// Strip metrics.
    pub glyph: GlyphMetrics,
    /// Font bytes with no character: art the charset does not reach.
    pub unmapped_glyphs: Vec<u8>,
}

/// One cell of the glyph strip.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Glyph {
    /// The font byte, which is the cell index.
    pub byte: u8,
    /// The character it draws, when the charset maps one.
    #[serde(default)]
    pub char: Option<char>,
    /// X of the cell in the strip, in pixels.
    pub x: i32,
    /// Y of the cell in the strip, in pixels (always 0: the strip is one row).
    pub y: i32,
    /// Cell width, 8.
    pub width: u32,
    /// Cell height, 16.
    pub height: u32,
    /// Whether every pixel is background.
    pub blank: bool,
}

/// Strip-wide glyph metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct GlyphMetrics {
    /// Cells in the strip.
    pub count: u32,
    /// 8.
    pub width: u32,
    /// 16.
    pub height: u32,
}

// ---------------------------------------------------------------------------
// window.json
// ---------------------------------------------------------------------------

/// `dialogue/window.json`: the frame's tiles and how they are laid out.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct WindowFile {
    /// Pack format; checked against [`DIALOGUE_FORMAT_VERSION`].
    pub format_version: u32,
    /// The tile strip, pack-root-relative. Nothing in it is transparent.
    pub png: String,
    /// Border thickness, cell size, and the open animation.
    pub geometry: WindowGeometry,
    /// The nine tile roles, keyed by [`Role::as_str`].
    pub roles: BTreeMap<String, RoleTile>,
    /// The message box.
    pub text_window: TextWindow,
    /// The portrait box.
    pub portrait_window: PortraitWindow,
    /// Every window in `WinGroup_Dialogue`, in record order.
    pub windows: Vec<WindowRecord>,
    /// `Pal_Init_Line_3`, the CRAM line the whole window renders in.
    pub palette: WindowPalette,
}

impl WindowFile {
    /// One role's tile.
    #[must_use]
    pub fn role(&self, role: Role) -> Option<&RoleTile> {
        self.roles.get(role.as_str())
    }
}

/// The nine tiles `Window_Draw` uses, and nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Top-left corner.
    CornerTopLeft,
    /// Top-right corner: the same tile, flipped horizontally.
    CornerTopRight,
    /// Bottom-left corner: flipped vertically.
    CornerBottomLeft,
    /// Bottom-right corner: flipped both ways.
    CornerBottomRight,
    /// Top edge, repeated across.
    EdgeTop,
    /// Bottom edge.
    EdgeBottom,
    /// Left edge, repeated down.
    EdgeLeft,
    /// Right edge.
    EdgeRight,
    /// Interior fill.
    Fill,
}

impl Role {
    /// Every role, in the order the geometry rule names them.
    pub const ALL: [Role; 9] = [
        Role::CornerTopLeft,
        Role::EdgeTop,
        Role::CornerTopRight,
        Role::EdgeLeft,
        Role::Fill,
        Role::EdgeRight,
        Role::CornerBottomLeft,
        Role::EdgeBottom,
        Role::CornerBottomRight,
    ];

    /// The key this role has in `window.json`'s `roles` map.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Role::CornerTopLeft => "corner_top_left",
            Role::CornerTopRight => "corner_top_right",
            Role::CornerBottomLeft => "corner_bottom_left",
            Role::CornerBottomRight => "corner_bottom_right",
            Role::EdgeTop => "edge_top",
            Role::EdgeBottom => "edge_bottom",
            Role::EdgeLeft => "edge_left",
            Role::EdgeRight => "edge_right",
            Role::Fill => "fill",
        }
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One role's tile in the strip, with the flips its plane word carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct RoleTile {
    /// Tile index in the blob, which is also the cell index in the strip.
    pub tile: u32,
    /// X of the cell in the strip, in pixels.
    pub x: i32,
    /// Y of the cell in the strip, in pixels.
    pub y: i32,
    /// 8.
    pub width: u32,
    /// 8.
    pub height: u32,
    /// Whether the plane word flips it horizontally.
    pub flip_h: bool,
    /// Whether the plane word flips it vertically.
    pub flip_v: bool,
    /// Whether the plane word sets priority (all nine do).
    pub priority: bool,
}

/// Border thickness, cell size, and how a window opens.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct WindowGeometry {
    /// Border thickness in cells: 1.
    pub border_cells: u32,
    /// Cell size in pixels: 8.
    pub cell_pixels: u32,
    /// Whether the frame casts a shadow: it does not.
    pub shadow: bool,
    /// How the frame grows when it opens.
    pub open_animation: OpenAnimation,
}

/// `loc_68690`: the frame is redrawn wider from the centre each step.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct OpenAnimation {
    /// `"horizontal"`.
    pub axis: String,
    /// `"center"`.
    pub from: String,
    /// Cells added per step: 2.
    pub step_cells: u32,
    /// Frames per step: 1.
    pub frames_per_step: u32,
}

/// The message box.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TextWindow {
    /// Index into [`WindowFile::windows`].
    pub window: u32,
    /// The whole box including its frame, in Genesis screen pixels.
    pub rect: PixelRect,
    /// 32.
    pub chars_per_line: u32,
    /// 2.
    pub lines_per_window: u32,
    /// 8.
    pub glyph_width: u32,
    /// 16.
    pub glyph_height: u32,
    /// The text area in cells.
    pub interior: CellRegion,
}

/// The portrait box. The portrait art covers the frame completely -- the PNGs
/// carry their own border.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PortraitWindow {
    /// Index into [`WindowFile::windows`].
    pub window: u32,
    /// Where the 48x48 picture goes, in Genesis screen pixels.
    pub rect: PixelRect,
}

/// One `WinGroup_Dialogue` record.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct WindowRecord {
    /// Record index in the group.
    pub index: u32,
    /// The pack's name for it: `controls`, `dialogue`, `portrait`, `yes_no`.
    pub name: String,
    /// Its rect in Genesis screen pixels, frame included.
    pub rect: PixelRect,
    /// Width in cells, frame included.
    pub width_cells: u32,
    /// Height in cells, frame included.
    pub height_cells: u32,
}

/// A rect in Genesis screen pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct PixelRect {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width.
    pub width: i32,
    /// Height.
    pub height: i32,
}

/// A region in 8-pixel cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct CellRegion {
    /// Left edge in cells.
    pub x_cell: u32,
    /// Top edge in cells.
    pub y_cell: u32,
    /// Width in cells.
    pub width_cells: u32,
    /// Height in cells.
    pub height_cells: u32,
}

/// `Pal_Init_Line_3` and the two indices the window is drawn in.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct WindowPalette {
    /// Sixteen RGB triples.
    pub colors: Vec<[u8; 3]>,
    /// The index the interior is filled with: $E.
    pub fill_index: u8,
}

// ---------------------------------------------------------------------------
// portraits.json
// ---------------------------------------------------------------------------

/// `dialogue/portraits.json`: the `$F4` table.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PortraitFile {
    /// Pack format; checked against [`DIALOGUE_FORMAT_VERSION`].
    pub format_version: u32,
    /// Declared portrait count, validated against `portraits.len()`.
    pub count: u32,
    /// The portraits, id 1 upward. Id 0 is the table's null entry and is not
    /// listed: `$F4` id 0 hides the window.
    pub portraits: Vec<Portrait>,
    /// Where the picture goes and how big it is.
    pub geometry: PortraitGeometry,
}

/// One portrait.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Portrait {
    /// `$F4` id.
    pub id: u8,
    /// Who it is, from the art label: `Chaz`, `Alys`, ...
    pub symbol: String,
    /// The PNG, pack-root-relative.
    pub png: String,
    /// The portrait this one duplicates, when the ROM stores the art twice.
    #[serde(default)]
    pub duplicate_of: Option<u8>,
}

/// Portrait size and placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct PortraitGeometry {
    /// 48.
    pub width: u32,
    /// 48.
    pub height: u32,
    /// Screen x in Genesis pixels: 40.
    pub screen_x: i32,
    /// Screen y in Genesis pixels: 104.
    pub screen_y: i32,
    /// In talk mode, `$F4`'s second operand shifts the portrait right by this
    /// many tiles per slot.
    pub talk_slot_stride_tiles: u32,
}
