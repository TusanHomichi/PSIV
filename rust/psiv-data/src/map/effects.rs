//! `MapDataManager` entries for one map: the flag-gated patches of map load.
//!
//! Each entry is a routine the loader may run; it is decoded into execution paths,
//! each with the gates that reach it and the writes it performs, so the runtime
//! replays the writes the cartridge's own gates selected. A palette copy the decoder
//! once deferred is carried with the NPC sheet variants the extractor baked for it.

use serde::{Deserialize, Serialize};

/// One `MapDataManager` jump-table entry as it applies to this map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapEffect {
    /// The jump-table entry index.
    pub entry: u16,
    /// Whether the decoder fully decoded the routine. `false` entries carry
    /// `reason` instead of paths and a consumer must treat the map as
    /// possibly incompletely patched.
    pub decoded: bool,
    /// The write kinds this entry produces, for census-level checks.
    #[serde(default)]
    pub kinds: Vec<String>,
    /// Execution paths, each with the exact gate conditions that reach it.
    #[serde(default)]
    pub paths: Vec<EffectPath>,
    /// Why decoding stopped, when `decoded` is false.
    #[serde(default)]
    pub reason: Option<String>,
}

/// One execution route through an effect routine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectPath {
    /// Flag conditions that must all hold for this path to run.
    #[serde(default)]
    pub gates: Vec<EffectGate>,
    /// True when the path has no gates — it runs on every load.
    #[serde(default)]
    pub unconditional: bool,
    /// The dormant dispatcher abort: a routine returning non-zero skips the
    /// rest of the map's list. No retail routine sets it.
    #[serde(default)]
    pub aborts_remaining_entries: bool,
    /// Recognised-but-not-modelled instructions stepped over on this path.
    #[serde(default)]
    pub deferred: Vec<String>,
    /// Decoded palette copies with their already-extracted NPC sheet variants.
    #[serde(default)]
    pub deferred_effects: Vec<PaletteEffect>,
    /// The writes this path performs.
    #[serde(default)]
    pub writes: Vec<EffectWrite>,
}

/// A formerly deferred CRAM copy resolved by the asset extractor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaletteEffect {
    /// The extractor currently resolves `palette_write` here.
    pub resolved_as: String,
    /// Sprite replacements produced with the copied palette.
    pub affects: PaletteAffectedSprites,
}

/// The NPC assets affected by a palette copy. Map pixels use other CRAM lines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaletteAffectedSprites {
    /// Replacements retain the object's movement and dialogue identity.
    pub npc_sheets: Vec<PaletteSpriteReplacement>,
}

/// One NPC's base sheet and the variant baked with the copied CRAM words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaletteSpriteReplacement {
    /// Index in the map's object list.
    pub npc_index: usize,
    /// Base sheet expected by the extraction record.
    pub from: String,
    /// Sheet to draw when the enclosing path's gates hold.
    pub to: String,
}

/// One flag condition on an effect path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectGate {
    /// `event_flags` or `chest_flags` — the only banks retail gates on.
    pub bank: String,
    /// The flag id within the bank.
    pub flag: u16,
    /// `set` or `clear`.
    pub required: String,
    /// The disassembly's name, when it has one.
    #[serde(default)]
    pub symbol: Option<String>,
}

/// One write a path performs. Kind-specific fields are optional so one type
/// covers the union; the `kind` string is authoritative.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectWrite {
    /// `object_despawn` | `object_rewrite` | `object_dialogue` |
    /// `layout_write` | `layout_replace`.
    pub kind: String,
    /// ROM address of the writing instruction.
    #[serde(default)]
    pub at: Option<String>,
    /// Record object index, for the object kinds.
    #[serde(default)]
    pub object_index: Option<u32>,
    /// The new object id, for `object_rewrite`.
    #[serde(default)]
    pub object_id: Option<u16>,
    /// The new dialogue id, for `object_dialogue`.
    #[serde(default)]
    pub dialogue_id: Option<u16>,
    /// Chunk-level coordinates, for `layout_write`.
    #[serde(default)]
    pub cell_x: Option<u32>,
    /// Row, in collision cells, for `layout_write`.
    #[serde(default)]
    pub cell_y: Option<u32>,
    /// The chunk id written, for `layout_write`.
    #[serde(default)]
    pub chunk_id: Option<u16>,
    /// Which plane the write targets.
    #[serde(default)]
    pub plane: Option<String>,
    /// The replacement layout's ROM source, for `layout_replace` — matches a
    /// [`LayoutVariant`](crate::LayoutVariant) plane source.
    #[serde(default)]
    pub source: Option<String>,
    /// The flag bank label, for `flag_clear` (the door the routine calls;
    /// "chest_flags"/"temp_flags" both name the $F140 temp door).
    #[serde(default)]
    pub bank: Option<String>,
    /// The flag id within the bank, for `flag_clear`.
    #[serde(default)]
    pub flag: Option<u16>,
    /// Per-cell collision resolution: always exactly 4 cells in
    /// `(0,0) (1,0) (0,1) (1,1)` order, absolute coordinates.
    #[serde(default)]
    pub cells: Vec<ResolvedCell>,
    /// Whether this write's plane is the one collision reads on this map
    /// (276 retail maps read BG, 83 FG). `false` means picture-only: the
    /// patch tile still draws, the cells never touch the grid.
    #[serde(default)]
    pub collision_authoritative: Option<bool>,
    /// Index into the map's [`PatchTiles`](crate::map::PatchTiles) atlas for
    /// this write's chunk.
    #[serde(default)]
    pub patch_tile: Option<u32>,
}

/// A resolved collision cell for a `layout_write`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedCell {
    /// Column, in collision cells.
    pub x: u32,
    /// Row, in collision cells.
    pub y: u32,
    /// The 4-bit collision type the written chunk imposes.
    pub collision: u8,
}
