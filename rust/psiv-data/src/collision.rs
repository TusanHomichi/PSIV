//! The 4-bit collision grid: one type per 16x16-pixel cell.

use serde::de::{self, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

use crate::map::CellPos;

/// The collision type of one 16x16-pixel cell.
///
/// `GetChunkAndCollision` assembles a 4-bit code out of the collision flag
/// (bit 14) of the cell's four tiles, adding 1, 2, 4 and 8 for top-left,
/// top-right, bottom-left and bottom-right. It is a set of labels, not a
/// bitmask -- do not do arithmetic on it.
///
/// All sixteen codes are legal: `TileCollNormalPtrs` is a sixteen-entry jump
/// table and every entry goes somewhere. The disassembly names eight of them;
/// the rest are [`CollisionType::Unnamed`], which is not an error and not a
/// fallback. Retail data uses code `$7` in 172 cells across `KadaryInn_F1` and
/// the `ZioFort` maps, and never uses `$A` or `$B` at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CollisionType {
    /// Plain walkable ground. Code `$0`.
    Normal,
    /// Walkable. Stepping onto it fires a map transition; see [`Self::blocks`].
    /// Code `$1`.
    MapChange,
    /// Walkable. The disassembly's name for it; its effect is not decoded.
    /// Code `$2`.
    Recovery,
    /// Walls, scenery, anything the walker cannot enter. Code `$8`.
    Solid,
    /// Blocks on foot. Vehicles are a later increment. Code `$9`.
    Water,
    /// Blocks on foot. Code `$A`; unused by retail data.
    Sand,
    /// Blocks on foot. Code `$B`; unused by retail data.
    Ice,
    /// A shop counter: blocks, and the shop opens from the adjacent cell.
    /// Code `$C`.
    Shop,
    /// A code the disassembly does not name: `$3`-`$7`, `$D`-`$F`.
    ///
    /// These route to `TileColl_Empty` like every code outside the blocking
    /// set, so they are walkable. Kept as data rather than collapsed into
    /// [`CollisionType::Normal`] so the pack round-trips and so a map that
    /// leans on one stays visible.
    Unnamed(u8),
}

impl CollisionType {
    /// The eight types the disassembly names, in code order.
    ///
    /// Not every type: [`CollisionType::Unnamed`] covers the other eight codes.
    pub const NAMED: [CollisionType; 8] = [
        CollisionType::Normal,
        CollisionType::MapChange,
        CollisionType::Recovery,
        CollisionType::Solid,
        CollisionType::Water,
        CollisionType::Sand,
        CollisionType::Ice,
        CollisionType::Shop,
    ];

    /// Does this type stop the walker?
    ///
    /// The blocking set is exactly the original's -- types 8 (solid), 9
    /// (water), $A (sand), $B (ice) and $C (shop) -- per the fidelity spine in
    /// docs/RUNTIME_DESIGN.md ("Fidelity spine (field mode)"). It is taken from
    /// `TileCollNormalPtrs`: 8, 9, $A and $B route to `TileColl_Solid` and $C
    /// to `TileColl_Shop`, and both of those are `moveq #1,d2 / rts`. Every
    /// other type routes to `TileColl_Empty`.
    ///
    /// Type 1 (map change) deliberately does **not** block: the walker steps
    /// onto the cell and the warp fires on entry.
    pub const fn blocks(self) -> bool {
        matches!(
            self,
            CollisionType::Solid
                | CollisionType::Water
                | CollisionType::Sand
                | CollisionType::Ice
                | CollisionType::Shop
        )
    }

    /// The ROM's 4-bit code.
    pub const fn code(self) -> u8 {
        match self {
            CollisionType::Normal => 0x0,
            CollisionType::MapChange => 0x1,
            CollisionType::Recovery => 0x2,
            CollisionType::Solid => 0x8,
            CollisionType::Water => 0x9,
            CollisionType::Sand => 0xA,
            CollisionType::Ice => 0xB,
            CollisionType::Shop => 0xC,
            CollisionType::Unnamed(code) => code,
        }
    }

    /// Does the disassembly have a name for this code?
    pub const fn is_named(self) -> bool {
        !matches!(self, CollisionType::Unnamed(_))
    }

    /// The name the disassembly annotates this code with. Matches the
    /// vocabulary `psiv_tools.layouts.COLLISION_TYPE_NAMES` emits -- and which
    /// the pack manifest carries in `collision.type_names` -- so a Rust message
    /// and a Python one describe a cell the same way.
    pub const fn name(self) -> Option<&'static str> {
        match self {
            CollisionType::Normal => Some("normal"),
            CollisionType::MapChange => Some("map_change"),
            CollisionType::Recovery => Some("recovery"),
            CollisionType::Solid => Some("solid"),
            CollisionType::Water => Some("water"),
            CollisionType::Sand => Some("sand"),
            CollisionType::Ice => Some("ice_block"),
            CollisionType::Shop => Some("shop"),
            CollisionType::Unnamed(_) => None,
        }
    }
}

impl fmt::Display for CollisionType {
    /// `solid (0x8)`, or `unnamed_7 (0x7)` -- the same spelling
    /// `psiv_tools.layouts.collision_type_name` uses for an unnamed code.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.name() {
            Some(name) => write!(f, "{name} (0x{:X})", self.code()),
            None => write!(f, "unnamed_{0:X} (0x{0:X})", self.code()),
        }
    }
}

/// A collision value that is not a 4-bit code at all.
///
/// The type is a nibble assembled from four tile flags, so `0..=15` is the
/// whole space. A larger number did not come out of `GetChunkAndCollision`, so
/// the pack is wrong rather than merely surprising.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UndefinedCollisionType(pub u8);

impl fmt::Display for UndefinedCollisionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} is not a collision type; the type is a 4-bit code, so 0..=15",
            self.0
        )
    }
}

impl std::error::Error for UndefinedCollisionType {}

impl TryFrom<u8> for CollisionType {
    type Error = UndefinedCollisionType;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x0 => Ok(CollisionType::Normal),
            0x1 => Ok(CollisionType::MapChange),
            0x2 => Ok(CollisionType::Recovery),
            0x8 => Ok(CollisionType::Solid),
            0x9 => Ok(CollisionType::Water),
            0xA => Ok(CollisionType::Sand),
            0xB => Ok(CollisionType::Ice),
            0xC => Ok(CollisionType::Shop),
            other if other <= 0xF => Ok(CollisionType::Unnamed(other)),
            other => Err(UndefinedCollisionType(other)),
        }
    }
}

/// Which plane the collision was read from.
///
/// `GetChunkAndCollision` reads `Map_Layout_BG` when the map record's
/// `$FFFFEC24` byte is non-zero and `Map_Layout_FG` when it is zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Plane {
    /// Foreground plane.
    Fg,
    /// Background plane.
    Bg,
}

/// A map's `collision` section: the grid and which plane produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collision {
    /// The plane `GetChunkAndCollision` would have read.
    pub plane: Plane,
    /// The raw `$FFFFEC24` byte that selected it.
    pub plane_byte: u8,
    /// One type per cell.
    pub grid: CollisionGrid,
}

/// A map's collision grid: `width` * `height` cells, row-major.
///
/// The grid knows its own shape, but whether that shape is the *right* one is
/// checked against the map's declared dimensions during
/// [`GameData::load`](crate::GameData::load) -- a grid on its own cannot know.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollisionGrid {
    width: u32,
    height: u32,
    cells: Vec<CollisionType>,
}

impl CollisionGrid {
    /// Build a grid from row-major cells. Fails if `cells.len()` is not
    /// `width * height`.
    ///
    /// Coordinates are `u32` throughout the crate, matching [`CellPos`] and
    /// [`Dimensions`](crate::Dimensions), so that no caller has to cast to walk
    /// from a position to the cell under it.
    pub fn new(width: u32, height: u32, cells: Vec<CollisionType>) -> Result<Self, String> {
        let expected = (width as usize).saturating_mul(height as usize);
        if cells.len() != expected {
            return Err(format!(
                "a {width}x{height} grid needs {expected} cells, got {}",
                cells.len()
            ));
        }
        Ok(CollisionGrid {
            width,
            height,
            cells,
        })
    }

    /// Width in cells.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in cells.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The type at a cell, or `None` outside the grid.
    pub fn at(&self, x: u32, y: u32) -> Option<CollisionType> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some(self.cells[y as usize * self.width as usize + x as usize])
    }

    /// The type at a position, or `None` outside the grid.
    pub fn at_pos(&self, pos: CellPos) -> Option<CollisionType> {
        self.at(pos.x, pos.y)
    }

    /// Whether a cell stops the walker. Cells outside the grid block, so a
    /// caller that forgets a bounds check walks into a wall rather than off the
    /// map.
    pub fn blocks_at(&self, x: u32, y: u32) -> bool {
        self.at(x, y).map(CollisionType::blocks).unwrap_or(true)
    }

    /// Whether a position stops the walker; outside the grid blocks.
    pub fn blocks_at_pos(&self, pos: CellPos) -> bool {
        self.blocks_at(pos.x, pos.y)
    }

    /// All cells, row-major.
    pub fn cells(&self) -> &[CollisionType] {
        &self.cells
    }

    /// The rows, top to bottom.
    pub fn rows(&self) -> impl Iterator<Item = &[CollisionType]> {
        self.cells.chunks(self.width.max(1) as usize)
    }

    /// How many cells hold a given type.
    pub fn count_of(&self, kind: CollisionType) -> usize {
        self.cells.iter().filter(|cell| **cell == kind).count()
    }
}

/// Turn parsed rows into a grid, rejecting a ragged one.
fn grid_from_rows(rows: Vec<Vec<CollisionType>>) -> Result<CollisionGrid, String> {
    let height = rows.len() as u32;
    let width = rows.first().map(Vec::len).unwrap_or(0) as u32;
    // A ragged grid has no meaningful width, so this is caught here rather than
    // left for the dimension check, which could only report the first row's
    // length and call the map the wrong size.
    for (index, row) in rows.iter().enumerate() {
        if row.len() as u32 != width {
            return Err(format!(
                "collision row {index} has {} cells but row 0 has {width}",
                row.len()
            ));
        }
    }
    CollisionGrid::new(width, height, rows.into_iter().flatten().collect())
}

fn parse_hex_row(text: &str, row: usize) -> Result<Vec<CollisionType>, String> {
    text.chars()
        .enumerate()
        .map(|(x, c)| {
            let code = c
                .to_digit(16)
                .ok_or_else(|| format!("row {row}, cell {x}: {c:?} is not a hex digit"))?;
            CollisionType::try_from(code as u8).map_err(|e| format!("row {row}, cell {x}: {e}"))
        })
        .collect()
}

/// One row on the way in. `psiv_tools.pack` emits an array of numeric codes per
/// row; a string of hex nibbles is accepted too, since a 4-bit type is exactly
/// one hex character, and that compact form is what serialising a grid produces.
enum RawRow {
    Hex(String),
    Codes(Vec<u8>),
}

impl<'de> Deserialize<'de> for RawRow {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct RawRowVisitor;

        impl<'de> Visitor<'de> for RawRowVisitor {
            type Value = RawRow;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a hex-nibble string or an array of collision codes")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<RawRow, E> {
                Ok(RawRow::Hex(value.to_owned()))
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<RawRow, A::Error> {
                let mut codes = Vec::with_capacity(seq.size_hint().unwrap_or(0));
                while let Some(code) = seq.next_element::<u8>()? {
                    codes.push(code);
                }
                Ok(RawRow::Codes(codes))
            }
        }

        deserializer.deserialize_any(RawRowVisitor)
    }
}

struct RowsVisitor;

impl<'de> Visitor<'de> for RowsVisitor {
    type Value = Vec<Vec<CollisionType>>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an array of collision rows, each a hex-nibble string or an array of codes")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut rows: Vec<Vec<CollisionType>> = Vec::with_capacity(seq.size_hint().unwrap_or(0));
        while let Some(raw) = seq.next_element::<RawRow>()? {
            let index = rows.len();
            let row = match raw {
                RawRow::Hex(text) => parse_hex_row(&text, index).map_err(de::Error::custom)?,
                RawRow::Codes(codes) => codes
                    .into_iter()
                    .enumerate()
                    .map(|(x, code)| {
                        CollisionType::try_from(code)
                            .map_err(|e| de::Error::custom(format!("row {index}, cell {x}: {e}")))
                    })
                    .collect::<Result<Vec<_>, A::Error>>()?,
            };
            rows.push(row);
        }
        Ok(rows)
    }
}

fn deserialize_rows<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<Vec<CollisionType>>, D::Error> {
    deserializer.deserialize_seq(RowsVisitor)
}

impl<'de> Deserialize<'de> for CollisionGrid {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Unknown fields are ignored rather than denied: the pack carries
        // `plane` and the declared cell counts alongside `rows`. Missing fields
        // stay hard errors -- absence is the dangerous direction, not surplus.
        #[derive(Deserialize)]
        struct Raw {
            #[serde(deserialize_with = "deserialize_rows")]
            rows: Vec<Vec<CollisionType>>,
        }

        let raw = Raw::deserialize(deserializer)?;
        grid_from_rows(raw.rows).map_err(de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for Collision {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            plane: Plane,
            plane_byte: u8,
            width_cells: u32,
            height_cells: u32,
            #[serde(deserialize_with = "deserialize_rows")]
            rows: Vec<Vec<CollisionType>>,
        }

        let raw = Raw::deserialize(deserializer)?;
        let grid = grid_from_rows(raw.rows).map_err(de::Error::custom)?;
        // The section states its own size as well as spelling it out in rows.
        // Disagreement is local and cheap to catch here, so it never reaches the
        // map-level dimension check as a confusing second-order failure.
        if grid.width() != raw.width_cells || grid.height() != raw.height_cells {
            return Err(de::Error::custom(format!(
                "collision rows are {}x{} but the section declares {}x{}",
                grid.width(),
                grid.height(),
                raw.width_cells,
                raw.height_cells
            )));
        }
        Ok(Collision {
            plane: raw.plane,
            plane_byte: raw.plane_byte,
            grid,
        })
    }
}

/// The compact hex-nibble rows a grid serialises to.
fn hex_rows(grid: &CollisionGrid) -> Vec<String> {
    grid.rows()
        .map(|row| {
            row.iter()
                .map(|cell| char::from_digit(u32::from(cell.code()), 16).unwrap_or('0'))
                .collect()
        })
        .collect()
}

impl Serialize for CollisionGrid {
    /// Emits the compact hex-nibble form. Round-tripping a grid normalises it.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("CollisionGrid", 1)?;
        state.serialize_field("rows", &hex_rows(self))?;
        state.end()
    }
}

impl Serialize for Collision {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("Collision", 5)?;
        state.serialize_field("plane", &self.plane)?;
        state.serialize_field("plane_byte", &self.plane_byte)?;
        state.serialize_field("width_cells", &self.grid.width())?;
        state.serialize_field("height_cells", &self.grid.height())?;
        state.serialize_field("rows", &hex_rows(&self.grid))?;
        state.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocking_set_is_exactly_the_originals() {
        let blocking: Vec<u8> = (0x0..=0xFu8)
            .map(|code| CollisionType::try_from(code).unwrap())
            .filter(|t| t.blocks())
            .map(|t| t.code())
            .collect();
        assert_eq!(blocking, vec![0x8, 0x9, 0xA, 0xB, 0xC]);
        // Type 1 walks; the warp fires on entry.
        assert!(!CollisionType::MapChange.blocks());
        assert!(!CollisionType::Normal.blocks());
        assert!(!CollisionType::Recovery.blocks());
        // Everything the jump table does not send to Solid or Shop is Empty,
        // unnamed codes included. Retail leans on this for code $7.
        assert!(!CollisionType::Unnamed(0x7).blocks());
    }

    #[test]
    fn codes_are_the_roms_codes() {
        assert_eq!(CollisionType::Normal.code(), 0x0);
        assert_eq!(CollisionType::MapChange.code(), 0x1);
        assert_eq!(CollisionType::Recovery.code(), 0x2);
        assert_eq!(CollisionType::Solid.code(), 0x8);
        assert_eq!(CollisionType::Water.code(), 0x9);
        assert_eq!(CollisionType::Sand.code(), 0xA);
        assert_eq!(CollisionType::Ice.code(), 0xB);
        assert_eq!(CollisionType::Shop.code(), 0xC);
        assert_eq!(CollisionType::Unnamed(0x7).code(), 0x7);
    }

    #[test]
    fn every_nibble_is_a_type_and_nothing_larger_is() {
        // `TileCollNormalPtrs` is a sixteen-entry jump table; all sixteen codes
        // are reachable, so none of them may fail to parse.
        for code in 0x0..=0xFu8 {
            let parsed = CollisionType::try_from(code).expect("every nibble is a type");
            assert_eq!(parsed.code(), code);
        }
        for named in CollisionType::NAMED {
            assert_eq!(CollisionType::try_from(named.code()), Ok(named));
            assert!(named.is_named());
        }
        for code in [0x3, 0x4, 0x5, 0x6, 0x7, 0xD, 0xE, 0xF] {
            let parsed = CollisionType::try_from(code).unwrap();
            assert_eq!(parsed, CollisionType::Unnamed(code));
            assert!(!parsed.is_named());
            assert_eq!(parsed.name(), None);
        }
        // 16 and up did not come out of a 4-bit assembly.
        for code in [16u8, 200, 255] {
            assert_eq!(
                CollisionType::try_from(code),
                Err(UndefinedCollisionType(code))
            );
        }
    }

    #[test]
    fn unnamed_codes_display_the_way_python_spells_them() {
        assert_eq!(CollisionType::Unnamed(0x7).to_string(), "unnamed_7 (0x7)");
        assert_eq!(CollisionType::Solid.to_string(), "solid (0x8)");
    }

    #[test]
    fn hex_rows_and_code_rows_produce_the_same_grid() {
        let hex: CollisionGrid =
            serde_json::from_str(r#"{"rows": ["0018", "0189", "88ab", "c210"]}"#).unwrap();
        let codes: CollisionGrid =
            serde_json::from_str(r#"{"rows": [[0,0,1,8],[0,1,8,9],[8,8,10,11],[12,2,1,0]]}"#)
                .unwrap();
        assert_eq!(hex, codes);
        assert_eq!(hex.width(), 4);
        assert_eq!(hex.height(), 4);
        assert_eq!(hex.at(3, 0), Some(CollisionType::Solid));
        assert_eq!(hex.at(0, 3), Some(CollisionType::Shop));
        assert_eq!(hex.at(4, 0), None);
        assert_eq!(hex.count_of(CollisionType::Solid), 4);
    }

    #[test]
    fn a_collision_section_carries_its_plane() {
        let section: Collision = serde_json::from_str(
            r#"{"plane": "bg", "plane_byte": 1, "width_cells": 2, "height_cells": 2,
                 "rows": [[0, 1], [8, 0]]}"#,
        )
        .unwrap();
        assert_eq!(section.plane, Plane::Bg);
        assert_eq!(section.plane_byte, 1);
        assert_eq!(section.grid.at(0, 1), Some(CollisionType::Solid));
    }

    #[test]
    fn a_collision_section_that_misstates_its_own_size_is_rejected() {
        let err = serde_json::from_str::<Collision>(
            r#"{"plane": "fg", "plane_byte": 0, "width_cells": 9, "height_cells": 2,
                 "rows": [[0, 1], [8, 0]]}"#,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("declares 9x2"), "{err}");
    }

    #[test]
    fn blocks_at_treats_outside_as_blocking() {
        let grid: CollisionGrid = serde_json::from_str(r#"{"rows": ["00", "08"]}"#).unwrap();
        assert!(!grid.blocks_at(0, 0));
        assert!(grid.blocks_at(1, 1));
        assert!(grid.blocks_at(2, 0), "outside the grid blocks");
        assert!(grid.blocks_at(0, 99));
    }

    #[test]
    fn ragged_rows_are_rejected() {
        let err = serde_json::from_str::<CollisionGrid>(r#"{"rows": ["0000", "000"]}"#)
            .unwrap_err()
            .to_string();
        assert!(err.contains("row 1 has 3 cells"), "{err}");
    }

    #[test]
    fn unnamed_nibbles_parse_rather_than_fail() {
        // Retail `KadaryInn_F1` and the `ZioFort` maps hold code $7.
        let grid: CollisionGrid = serde_json::from_str(r#"{"rows": ["0000", "00f0"]}"#).unwrap();
        assert_eq!(grid.at(2, 1), Some(CollisionType::Unnamed(0xF)));
        assert!(!grid.blocks_at(2, 1));

        let grid: CollisionGrid = serde_json::from_str(r#"{"rows": [[0, 0], [7, 0]]}"#).unwrap();
        assert_eq!(grid.at(0, 1), Some(CollisionType::Unnamed(0x7)));
    }

    #[test]
    fn a_value_too_big_for_a_nibble_is_rejected_with_its_position() {
        let err = serde_json::from_str::<CollisionGrid>(r#"{"rows": [[0, 0], [99, 0]]}"#)
            .unwrap_err()
            .to_string();
        assert!(err.contains("row 1, cell 0"), "{err}");
        assert!(err.contains("4-bit code"), "{err}");
    }

    #[test]
    fn non_hex_character_is_rejected() {
        let err = serde_json::from_str::<CollisionGrid>(r#"{"rows": ["00z0"]}"#)
            .unwrap_err()
            .to_string();
        assert!(err.contains("not a hex digit"), "{err}");
    }

    #[test]
    fn round_trips_through_the_hex_form() {
        let original: CollisionGrid =
            serde_json::from_str(r#"{"rows": ["0018", "0189", "88ab", "c210"]}"#).unwrap();
        let text = serde_json::to_string(&original).unwrap();
        assert_eq!(text, r#"{"rows":["0018","0189","88ab","c210"]}"#);
        let back: CollisionGrid = serde_json::from_str(&text).unwrap();
        assert_eq!(original, back);
    }
}
