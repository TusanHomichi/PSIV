//! Whole-layout replacements (`layout_replace`) and the raw chunk grids.
//!
//! A variant is a decoded layout shipped as a first-class record with its own
//! collision, the way the map's own layout is; the raw chunk grid beside it is what
//! vehicle battle background selection indexes.

use serde::{Deserialize, Serialize};

/// A whole-layout replacement, decoded like a base layout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutVariant {
    /// The variant render.
    pub png: String,
    /// The variant's priority overlay, when it has priority tiles.
    #[serde(default)]
    pub png_over: Option<String>,
    /// Per-plane provenance: which plane changed and its ROM source.
    #[serde(default)]
    pub planes: Vec<VariantPlane>,
    /// The variant's collision grid.
    pub collision: VariantCollision,
    /// The variant's raw chunk grid, when its layout can feed vehicle battle
    /// background selection.
    #[serde(default)]
    pub vehicle_battle: Option<VehicleBattleLayout>,
}

/// One plane of a layout variant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariantPlane {
    /// `fg` or `bg`.
    pub plane: String,
    /// ROM offset of the replacement blob.
    #[serde(default)]
    pub source: Option<String>,
    /// True when the "replacement" is the map's own base blob (one plane of
    /// each retail pair is).
    #[serde(default)]
    pub identical_to_base: bool,
}

/// A variant's collision grid, rows of 4-bit types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariantCollision {
    /// The plane collision reads on this map.
    #[serde(default)]
    pub plane: Option<String>,
    /// Width in cells.
    pub width_cells: u32,
    /// Height in cells.
    pub height_cells: u32,
    /// Row-major cell types.
    pub rows: Vec<Vec<u8>>,
}

/// Raw chunk ids in the collision-authoritative layout plane.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VehicleBattleLayout {
    /// `fg` or `bg`, matching the decoded map plane.
    #[serde(default)]
    pub plane: Option<String>,
    /// Width in 32-pixel chunks.
    pub width_chunks: u32,
    /// Height in 32-pixel chunks.
    pub height_chunks: u32,
    /// Row-major raw chunk ids.
    pub rows: Vec<Vec<u16>>,
}
