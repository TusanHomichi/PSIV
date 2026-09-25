//! The per-map runtime record, `maps/<id>_<symbol>.json`.
//!
//! Field names here are `psiv_tools.pack`'s, exactly. That module's docstring
//! calls the emitted JSON "a versioned interface, so field names here are
//! stable and `format_version` moves when they are not"; this is the other end
//! of that interface, and the way to change either is to change both.
//!
//! # Coordinates
//!
//! Logical position lives on the 16-pixel collision-cell grid, as
//! docs/RUNTIME_DESIGN.md specifies and `GetChunkAndCollision` models. The pack
//! spells out which space each number is in and so does this module: `*_cell`
//! is a collision cell, `*_pixels` is a pixel, `*_byte` is the raw byte the map
//! record stores.
//!
//! The three are not interchangeable, and the gap between them is not just a
//! scale factor. `GetChunkAndCollision` does `addi.w #$10,d6` before shifting Y
//! down to a cell, so the cell a character *occupies* is one row below
//! `curr_y_pos / 16`. Every Y a map record stores is a `curr_y_pos`, so the
//! packer applies that shift and emits the occupied cell in `y_cell`. Use
//! `y_cell`; `y_byte` and `y_pixels` are there to re-derive it, not to walk on.

// The record's sections live in submodules: `cell` is the coordinate space,
// `warp`, `interaction`, `object` and `treasure` are what a walker meets on
// one, `effects` and `overworld` are what `MapDataManager` and the paged
// overworld patch after load, and `settings` are the map's own scalars.

mod cell;
mod effects;
mod facing;
mod interaction;
mod layout;
mod object;
mod overworld;
mod reference;
mod settings;
mod treasure;
mod warp;

pub use cell::{COLLISION_CELL_PIXELS, Cell, CellPos, CellRect, Dimensions};
pub use effects::{
    EffectGate, EffectPath, EffectWrite, MapEffect, PaletteAffectedSprites, PaletteEffect,
    PaletteSpriteReplacement, ResolvedCell,
};
pub use facing::{Direction, Facing, SpriteFacing};
pub use interaction::{InteractionArea, InteractionFlagType, InteractionSource};
pub use layout::{LayoutVariant, VariantCollision, VariantPlane, VehicleBattleLayout};
pub use object::{Npc, SpriteRef};
pub use overworld::{
    OverworldLayoutPatch, OverworldLayoutWrite, OverworldPatch, OverworldPatchFlag,
    OverworldPatchTile, PatchTiles,
};
pub use reference::{MapRef, RangeRef};
pub use settings::{Flags, Music};
pub use treasure::{ContentsType, Treasure};
pub use warp::{TransitionTable, Warp, WarpSource};

use crate::camera::MapScroll;
use crate::collision::{Collision, CollisionType};
use crate::ids::MapId;
use serde::{Deserialize, Serialize};

/// The ROM holds exactly 43 Kosinski-compressed dialogue trees.
///
/// They are numbered from one -- `DialogueTree1` through `DialogueTree43` -- so
/// a valid binding is `1..=43` and there is no tree 0. Retail maps bind 36 of
/// the 43.
pub const DIALOGUE_TREE_COUNT: u8 = 43;

/// A field map as the runtime needs it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapRecord {
    /// The pack format this record was written in. Repeated per map so a single
    /// file is self-describing.
    pub format_version: u32,
    /// Index into `FieldMapPtrs`.
    pub id: MapId,
    /// The `MapID_*` symbol.
    #[serde(default)]
    pub symbol: Option<String>,
    /// The composed render, relative to the pack directory.
    pub png: String,
    /// The priority-tile overlay render (tiles the VDP draws above sprites),
    /// or `None` for the 22 maps with zero priority tiles.
    #[serde(default)]
    pub png_over: Option<String>,
    /// Size, in cells, chunks and pixels.
    pub dimensions: Dimensions,
    /// The collision grid and the plane it came from.
    pub collision: Collision,
    /// The map-load camera bytes and optional initial 16.16 step counters.
    #[serde(default)]
    pub scroll: MapScroll,
    /// What plays on arrival.
    pub music: Music,
    /// The four per-map flags.
    pub flags: Flags,
    /// Which of the 43 dialogue trees this map's objects speak from, `1..=43`.
    pub dialogue_tree: u8,
    /// `RunEventsJmpTbl` trigger indices in evaluation order (`[0]` is the
    /// retail idle case — no map carries an empty list).
    #[serde(default)]
    pub events: Vec<u16>,
    /// Map transitions from both tables, in record order.
    pub warps: Vec<Warp>,
    /// Areas checked by `Interaction_ChkMapAreas`, in record order.  Packs
    /// emitted before interaction extraction may omit this field.
    #[serde(default)]
    pub interaction_areas: Vec<InteractionArea>,
    /// `LoadMapObjects` entries.
    pub npcs: Vec<Npc>,
    /// `LoadTreasureChests` entries.
    pub treasure_chests: Vec<Treasure>,
    /// `MapDataManager` entries for this map: flag-gated load-time patches
    /// (`docs/field/MAP_EFFECTS.md`). Absent on packs predating the extraction.
    #[serde(default)]
    pub map_effects: Vec<MapEffect>,
    /// Decoded whole-layout replacements referenced by `layout_replace`
    /// writes, shipped as first-class variants with their own collision.
    #[serde(default)]
    pub layout_variants: Vec<LayoutVariant>,
    /// Raw chunk ids from the plane `GetChunkAndCollision` uses. Vehicle battle
    /// background selection indexes this grid before the collision nibble is
    /// resolved; keeping it beside the collision grid avoids reconstructing
    /// chunk identity from a lossy four-bit view.
    #[serde(default)]
    pub vehicle_battle: Option<VehicleBattleLayout>,
    /// The 32px patch-tile atlas for `layout_write`s and composed overworld
    /// page hooks, or `None` when neither path patches pixels.
    #[serde(default)]
    pub patch_tiles: Option<PatchTiles>,
    /// Original overworld page-hook records. They are kept separate from
    /// `MapDataManager` and from their render/collision resolution below.
    #[serde(default)]
    pub layout_patches: Vec<OverworldLayoutPatch>,
    /// Flag-selected, composed page-hook results for this full-map build.
    /// Retail also replays hooks when a page streams mid-map; this record does
    /// not claim to model a flag changing while the same map remains loaded.
    #[serde(default)]
    pub overworld_patches: Option<Vec<OverworldPatch>>,
}

impl MapRecord {
    /// The map's symbol, or its id in hex when it has none. For messages.
    pub fn label(&self) -> String {
        match &self.symbol {
            Some(symbol) => symbol.clone(),
            None => self.id.to_string(),
        }
    }

    /// Is this cell inside the map?
    pub fn contains(&self, pos: CellPos) -> bool {
        pos.x < self.dimensions.width_cells && pos.y < self.dimensions.height_cells
    }

    /// The collision type at a cell, or `None` outside the map.
    pub fn collision_at(&self, pos: CellPos) -> Option<CollisionType> {
        self.collision.grid.at_pos(pos)
    }

    /// Does this cell stop the walker? Outside the map blocks.
    pub fn blocks_at(&self, pos: CellPos) -> bool {
        self.collision.grid.blocks_at_pos(pos)
    }

    /// Every warp whose trigger rectangle covers a cell.
    ///
    /// More than one can match, and which one fires depends on the collision
    /// type being stood on -- see [`Warp::table`]. Resolving that is the game
    /// core's job, not the schema's, so this hands back all of them in record
    /// order.
    pub fn warps_at(&self, pos: CellPos) -> impl Iterator<Item = &Warp> {
        self.warps
            .iter()
            .filter(move |warp| warp.rect.is_some_and(|rect| rect.contains(pos)))
    }
}
