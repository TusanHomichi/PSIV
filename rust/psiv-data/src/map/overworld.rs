//! The paged overworld's page hooks and the composed patch tiles they resolve into.
//!
//! A hook is kept raw -- the flag it tests, the page that invokes it and the stores it
//! makes -- beside the flag-by-flag result a full-map build composed from those same
//! hooks, and the 32px atlas whose entries hold the composed FG/BG pairs.

use serde::{Deserialize, Serialize};

/// One raw paged-overworld hook, in page-loader order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverworldLayoutPatch {
    /// Plane whose page copy invokes the hook (`fg` or `bg`).
    pub triggered_by_plane: String,
    /// Source page index.
    pub page: u8,
    /// ROM routine address, retained for provenance.
    pub routine: String,
    /// Event flag tested by the hook.
    pub event_flag: OverworldPatchFlag,
    /// Stores into the rolling layout window, resolved to world coordinates.
    pub writes: Vec<OverworldLayoutWrite>,
}

/// Source event-flag identity for a page hook.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverworldPatchFlag {
    /// Event bank bit index.
    pub id: u16,
    /// Same id in hexadecimal, as the source decoder emits it.
    pub id_hex: String,
    /// Disassembly symbol if known.
    pub symbol: Option<String>,
}

/// Raw page-hook write; `chunk_ids` retain the cartridge's byte notation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverworldLayoutWrite {
    /// Plane written (`fg` or `bg`).
    pub plane: String,
    /// First horizontal chunk coordinate.
    pub chunk_x: u32,
    /// Chunk row.
    pub chunk_y: u32,
    /// First collision-cell column (`chunk_x * 2`).
    pub cell_x: u32,
    /// First collision-cell row (`chunk_y * 2`).
    pub cell_y: u32,
    /// Consecutive raw chunk bytes, spelled as hex strings.
    pub chunk_ids: Vec<String>,
    /// Displacement from the page hook's layout pointer.
    pub displacement: i32,
}

/// One flag's final composed chunks after all its page hooks run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverworldPatch {
    /// Event bank bit index; active when set.
    pub event_flag: u16,
    /// Distinct touched chunks in world coordinates.
    pub tiles: Vec<OverworldPatchTile>,
}

/// Both visual planes and the collision-authoritative chunk at one location.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverworldPatchTile {
    /// Horizontal world chunk coordinate.
    pub chunk_x: u32,
    /// Vertical world chunk coordinate.
    pub chunk_y: u32,
    /// Final FG chunk byte.
    pub fg_chunk_id: u16,
    /// Final BG chunk byte.
    pub bg_chunk_id: u16,
    /// Final chunk id in the plane `GetChunkAndCollision` reads.
    pub collision_chunk_id: u16,
    /// Row-major 2x2 collision nibble values from that chunk.
    pub collision: [u8; 4],
    /// Entry in `patch_tiles`, whose PNG holds the composed FG/BG picture.
    pub patch_tile: u32,
}

/// A map's patch-tile atlas: raw chunks and composed overworld plane pairs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchTiles {
    /// The atlas PNG, one row of tiles.
    pub png: String,
    /// The above-sprites overlay atlas. An overworld has one even if its
    /// composed results contain no priority pixels, to erase stale base ones.
    #[serde(default)]
    pub png_over: Option<String>,
    /// Tile edge length in pixels (32: a chunk is 2x2 collision cells).
    pub tile_pixels: u32,
    /// How many tiles the atlas holds.
    pub count: u32,
    /// Raw tiles first by chunk id, then composed FG/BG pairs by ids.
    pub tiles: Vec<PatchTile>,
}

/// One atlas tile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchTile {
    /// Position in the atlas row; `x` is its pixel offset.
    pub index: u32,
    /// Raw chunk id for a one-plane `MapDataManager`/scene tile; absent for a
    /// composed overworld FG/BG tile, which is not a raw chunk.
    pub chunk_id: Option<u16>,
    /// FG member of a composed overworld tile.
    #[serde(default)]
    pub fg_chunk_id: Option<u16>,
    /// BG member of a composed overworld tile.
    #[serde(default)]
    pub bg_chunk_id: Option<u16>,
    /// Pixel x of this tile inside the atlas PNG.
    pub x: u32,
    /// How many of the tile's 8px cells carry the priority bit.
    #[serde(default)]
    pub priority_tiles: u32,
    /// Row-major 2x2 collision values for live scene writes. Older atlases
    /// only carried map-load write cells and may omit this definition.
    #[serde(default)]
    pub collision: Option<[u8; 4]>,
}
