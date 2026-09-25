//! Field objects: the `LoadMapObjects` entries and the art they bind to.
//!
//! The placement a record stores is in 8-pixel units and a cell is 16, so the cell
//! the object's collision reads from is a floor and 85 retail objects sit on a half
//! cell: `x_pixels`/`y_pixels` are authoritative for drawing, the cell for collision.

use super::cell::CellPos;
use super::facing::{Facing, SpriteFacing};
use serde::{Deserialize, Serialize};

/// A field object: townsfolk, guards, the ones that just stand there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Npc {
    /// Position in this map's object list.
    pub index: u32,
    /// Where the record sits in the ROM.
    pub record_offset: String,
    /// Byte offset into `FieldObjectsJmpTbl`; the object's behaviour routine.
    pub object_id: u16,
    /// The routine's symbol, such as `NPCType2`.
    #[serde(default)]
    pub symbol: Option<String>,
    /// X in pixels, as the record stores it.
    pub x_pixels: u32,
    /// Y in pixels, as the record stores it.
    pub y_pixels: u32,
    /// Column, in cells.
    ///
    /// `LoadMapObjects` places objects with `lsl.w #3`, so object coordinates
    /// are in 8-pixel units and 85 of the cartridge's 949 objects sit on a half
    /// cell. This is the floor -- the cell the object's collision reads from --
    /// and for those 85 it is lossy. [`Npc::x_pixels`] and [`Npc::y_pixels`]
    /// are the authoritative placement; prefer them for rendering, and use the
    /// cell for collision and lookup.
    ///
    /// Chests and transitions are not like this: they store bytes already
    /// scaled by 16, so their cell is the stored byte with no division.
    pub x_cell: u32,
    /// Row, in cells, with the standing-cell shift applied.
    pub y_cell: u32,
    /// Which way the object faces.
    pub facing: Facing,
    /// Index into this map's dialogue tree.
    pub dialogue_id: u16,
    /// How to draw this object, when it has art: which sheet and sequences.
    #[serde(default)]
    pub sprite: Option<SpriteRef>,
    /// Why this object has no sprite (invisible trigger, vehicle-gated art,
    /// ...) when `sprite` is `None`. Exactly one of the two is set.
    #[serde(default)]
    pub sprite_reason: Option<String>,
    /// First VRAM tile of the object's art.
    pub art_tile: u32,
    /// Whether the object's type sets render-flags bit 3 (`$2(a3)`), which
    /// the cartridge tests before BOTH the talk probe and object collision.
    /// A `false` here is a monster you can see, not speak to, and walk
    /// straight through. Defaults `true` for packs predating the field.
    #[serde(default = "interactable_default")]
    pub interactable: bool,
    /// Whether the object's type sets render-flags bit 0, bypassing both
    /// camera-plane subtractions in `FieldObj_CalcSpritePos`.
    #[serde(default)]
    pub camera_bypass: bool,
}

const fn interactable_default() -> bool {
    true
}

impl Npc {
    /// The cell the object occupies.
    pub const fn pos(&self) -> CellPos {
        CellPos::new(self.x_cell, self.y_cell)
    }
}

/// An object's binding to a sprite sheet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpriteRef {
    /// Sheet id inside the index file named by `sheets`.
    pub sheet: String,
    /// The index file, pack-root-relative (`sprites/npcs.json`).
    pub sheets: String,
    /// The object's facing, resolved for convenience. `Unknown` covers the
    /// census's one odd byte (AiedoPub object 2, facing `$10`) — preserved,
    /// not rejected; renderers treat it as down.
    #[serde(default)]
    pub facing: Option<SpriteFacing>,
    /// Sequence to play at rest.
    pub idle_sequence: String,
    /// Sequence to play while moving.
    pub walk_sequence: String,
}
