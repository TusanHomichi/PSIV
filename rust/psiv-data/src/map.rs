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

use crate::collision::{Collision, CollisionType};
use crate::ids::MapId;
use serde::{Deserialize, Serialize};

/// One collision cell is 16x16 pixels: `GetChunkAndCollision` picks a quadrant
/// with bit 1 of the tile coordinate, so a cell covers 2x2 eight-pixel tiles.
pub const COLLISION_CELL_PIXELS: u32 = 16;

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
    /// Size, in cells, chunks and pixels.
    pub dimensions: Dimensions,
    /// The collision grid and the plane it came from.
    pub collision: Collision,
    /// What plays on arrival.
    pub music: Music,
    /// The four per-map flags.
    pub flags: Flags,
    /// Which of the 43 dialogue trees this map's objects speak from, `1..=43`.
    pub dialogue_tree: u8,
    /// Map transitions from both tables, in record order.
    pub warps: Vec<Warp>,
    /// `LoadMapObjects` entries.
    pub npcs: Vec<Npc>,
    /// `LoadTreasureChests` entries.
    pub treasure_chests: Vec<Treasure>,
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

/// Map size, in cells, chunks and pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dimensions {
    /// Width in collision cells. This is the coordinate space of everything
    /// else in the record.
    pub width_cells: u32,
    /// Height in collision cells.
    pub height_cells: u32,
    /// Width in chunks; a chunk is 4x4 tiles, so 2x2 cells.
    pub width_chunks: u32,
    /// Height in chunks.
    pub height_chunks: u32,
    /// Width in pixels, for the renderer.
    pub width_pixels: u32,
    /// Height in pixels.
    pub height_pixels: u32,
    /// Always [`COLLISION_CELL_PIXELS`]; carried so the pack is readable on its
    /// own and so a drift in the packer is caught rather than assumed away.
    pub cell_pixels: u32,
}

/// A position on the collision-cell grid.
///
/// Not deserialised directly -- the pack spells its coordinates `x_cell` and
/// `y_cell` at each site, so that no number in the JSON is ambiguous about its
/// unit. This is the type they are read into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CellPos {
    /// Column, in cells, from the left edge.
    pub x: u32,
    /// Row, in cells, from the top edge.
    pub y: u32,
}

impl CellPos {
    /// A position from a column and a row.
    pub const fn new(x: u32, y: u32) -> CellPos {
        CellPos { x, y }
    }
}

/// A cell position as the pack spells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    /// Column, in cells.
    pub x_cell: u32,
    /// Row, in cells, already carrying the standing-cell shift.
    pub y_cell: u32,
}

impl Cell {
    /// As a [`CellPos`].
    pub const fn pos(self) -> CellPos {
        CellPos::new(self.x_cell, self.y_cell)
    }
}

/// A half-open rectangle of collision cells: `x..x + width`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellRect {
    /// Leftmost column, in cells.
    pub x: u32,
    /// Topmost row, in cells.
    pub y: u32,
    /// Width in cells. The packer never emits zero; it emits no rectangle.
    pub width: u32,
    /// Height in cells.
    pub height: u32,
}

impl CellRect {
    /// Does this rectangle cover a cell?
    pub fn contains(&self, pos: CellPos) -> bool {
        pos.x >= self.x
            && pos.y >= self.y
            && pos.x < self.x.saturating_add(self.width)
            && pos.y < self.y.saturating_add(self.height)
    }

    /// One past the last cell on each axis.
    pub fn end(&self) -> CellPos {
        CellPos::new(
            self.x.saturating_add(self.width),
            self.y.saturating_add(self.height),
        )
    }
}

/// Which transition table a warp came from.
///
/// `RunMapTransitions` walks one or the other depending on the collision type
/// the player is standing on, so the table decides when a warp is even
/// considered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub enum TransitionTable {
    /// `Map_Transition_Data_Addr`, walked by `MapTransTile_Normal` for every
    /// standing collision type except map-change, solid, ice and shop: map
    /// edges, cave mouths, doormats.
    Normal,
    /// `Map_Transition_Data_2_Addr`, walked by `MapTransTile_MapChange`,
    /// reached only from a standing collision type of 1 whose previously
    /// occupied cell was not also type 1. These are doorways.
    MapChange,
}

impl TryFrom<u8> for TransitionTable {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(TransitionTable::Normal),
            2 => Ok(TransitionTable::MapChange),
            other => Err(format!(
                "transition table {other} does not exist; the ROM has tables 1 and 2"
            )),
        }
    }
}

impl From<TransitionTable> for u8 {
    fn from(value: TransitionTable) -> u8 {
        match value {
            TransitionTable::Normal => 1,
            TransitionTable::MapChange => 2,
        }
    }
}

/// A map transition.
///
/// The ROM stores a source coordinate and an `XYRangeJmpTbl` index, not a
/// rectangle; the packer transcribes the fifteen routines into [`Warp::rect`]
/// and clips it to the map, because five of them are open-ended half-planes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Warp {
    /// Position in this map's warp list.
    pub index: u32,
    /// Which table the record came from.
    pub table: TransitionTable,
    /// The collision the player must be standing on for this table to be
    /// walked: `any_walkable_tile` or `map_change_tile`.
    pub trigger: String,
    /// Where the record sits in the ROM, for tracing back.
    pub record_offset: String,
    /// The `XYRangeJmpTbl` entry that produced [`Warp::rect`].
    pub range: RangeRef,
    /// The record's coordinate, raw and resolved.
    pub source: WarpSource,
    /// The trigger area in cells, clipped to the map.
    ///
    /// `None` when the range produces no area at all -- `Null` never fires, and
    /// a clipped half-plane can fall entirely outside the map. Such a warp is
    /// dead data, not an error; the manifest counts them.
    pub rect: Option<CellRect>,
    /// The map this transition leads to.
    pub target: MapRef,
    /// Where the player lands on the target map, in that map's cells.
    pub destination: Cell,
    /// Which way the player faces on arrival.
    pub facing: Facing,
    /// The record's `character_alignment` byte; not decoded yet.
    pub character_alignment: u8,
}

/// A transition's source coordinate, raw and resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WarpSource {
    /// The raw X byte from the record.
    pub x_byte: u32,
    /// The raw Y byte from the record, one row above the cell it means.
    pub y_byte: u32,
    /// Column, in cells.
    pub x_cell: u32,
    /// Row, in cells, with the standing-cell shift applied.
    pub y_cell: u32,
}

impl WarpSource {
    /// The occupied cell this record is talking about.
    pub const fn pos(self) -> CellPos {
        CellPos::new(self.x_cell, self.y_cell)
    }
}

/// An `XYRangeJmpTbl` entry, by index and name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RangeRef {
    /// Index into `XYRangeJmpTbl`, `0..15`.
    pub id: u8,
    /// The disassembly's name for it, such as `XYExact`.
    pub name: String,
}

/// A facing direction, as `Map_Start_Facing_Dir` encodes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Facing {
    /// The raw byte: 0 down, 4 up, 8 right, `$C` left.
    pub id: u8,
    /// The decoded direction, or `None` for a byte outside those four.
    #[serde(default)]
    pub name: Option<Direction>,
}

impl Facing {
    /// The decoded direction, if the byte is one of the four the ROM defines.
    pub fn direction(&self) -> Option<Direction> {
        self.name
    }
}

/// One of the four facings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /// Facing down the screen; the raw byte is 0.
    Down,
    /// Raw byte 4.
    Up,
    /// Raw byte 8.
    Right,
    /// Raw byte `$C`.
    Left,
}

/// A reference to another map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapRef {
    /// The referenced map's id.
    pub id: MapId,
    /// Its `MapID_*` symbol, when it has one.
    #[serde(default)]
    pub symbol: Option<String>,
}

impl MapRef {
    /// The symbol, or the id in hex. For messages.
    pub fn label(&self) -> String {
        match &self.symbol {
            Some(symbol) => symbol.clone(),
            None => self.id.to_string(),
        }
    }
}

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
    /// First VRAM tile of the object's art.
    pub art_tile: u32,
}

impl Npc {
    /// The cell the object occupies.
    pub const fn pos(&self) -> CellPos {
        CellPos::new(self.x_cell, self.y_cell)
    }
}

/// What a chest holds. Byte 1 of the chest record selects between the two, and
/// byte 3 is read as whichever was selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentsType {
    /// Byte 3 is an inventory item id.
    Item,
    /// Byte 3 is a meseta count, in hundreds.
    Meseta,
}

/// A treasure chest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Treasure {
    /// Position in this map's chest list.
    pub index: u32,
    /// Where the record sits in the ROM.
    pub record_offset: String,
    /// Column, in cells.
    pub x_cell: u32,
    /// Row, in cells, with the standing-cell shift applied.
    pub y_cell: u32,
    /// X in pixels.
    pub x_pixels: u32,
    /// Y in pixels.
    pub y_pixels: u32,
    /// Whether this is one of the white chests.
    pub white_chest: bool,
    /// The chest object's symbol.
    #[serde(default)]
    pub object_symbol: Option<String>,
    /// Which of [`Treasure::item_id`] and [`Treasure::meseta`] is meaningful.
    pub contents_type: ContentsType,
    /// Set when `contents_type` is [`ContentsType::Item`].
    #[serde(default)]
    pub item_id: Option<u16>,
    /// The item's symbol, when it holds an item.
    #[serde(default)]
    pub item_symbol: Option<String>,
    /// Set when `contents_type` is [`ContentsType::Meseta`]. Already multiplied
    /// out: the ROM stores hundreds.
    #[serde(default)]
    pub meseta: Option<u32>,
    /// The save flag that remembers this chest was opened.
    pub chest_flag: u16,
}

impl Treasure {
    /// The cell the chest occupies.
    pub const fn pos(&self) -> CellPos {
        CellPos::new(self.x_cell, self.y_cell)
    }
}

/// The map's music. Id 0 means "keep playing whatever is playing", which is
/// what `changes_music` records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Music {
    /// The music id as the map record stores it.
    pub id: u8,
    /// The track's symbol, such as `MotabiaTown`.
    #[serde(default)]
    pub symbol: Option<String>,
    /// False when the id is 0 and the current track keeps playing.
    pub changes_music: bool,
}

/// The four per-map flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Flags {
    /// Non-zero on maps that drain HP as the party walks.
    pub poison: u8,
    /// Non-zero where random encounters roll.
    pub random_battles: u8,
    /// Non-zero where the town-teleport technique may be used.
    pub town_teleport: u8,
    /// Which dungeon-teleport destination this map belongs to.
    ///
    /// `None` when the stored byte has bit 7 set, which the extractor reads as
    /// "no index" rather than as index `$80 | n`. Nine retail maps do this: the
    /// eight `ValleyMaze*` parts and `Passageway`.
    #[serde(default)]
    pub dungeon_teleport_index: Option<u8>,
}

impl Flags {
    /// Does walking here cost HP?
    pub fn poisons(&self) -> bool {
        self.poison != 0
    }

    /// Do random encounters roll here?
    pub fn rolls_random_battles(&self) -> bool {
        self.random_battles != 0
    }

    /// May the town-teleport technique be used here?
    pub fn allows_town_teleport(&self) -> bool {
        self.town_teleport != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rects_are_half_open() {
        let rect = CellRect {
            x: 2,
            y: 3,
            width: 2,
            height: 1,
        };
        assert!(rect.contains(CellPos::new(2, 3)));
        assert!(rect.contains(CellPos::new(3, 3)));
        assert!(!rect.contains(CellPos::new(4, 3)));
        assert!(!rect.contains(CellPos::new(2, 4)));
        assert!(!rect.contains(CellPos::new(1, 3)));
        assert_eq!(rect.end(), CellPos::new(4, 4));
    }

    #[test]
    fn an_empty_rect_contains_nothing() {
        let rect = CellRect {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        };
        assert!(!rect.contains(CellPos::new(0, 0)));
    }

    #[test]
    fn flag_helpers_read_the_bytes() {
        let flags = Flags {
            poison: 0,
            random_battles: 1,
            town_teleport: 0,
            dungeon_teleport_index: Some(3),
        };
        assert!(!flags.poisons());
        assert!(flags.rolls_random_battles());
        assert!(!flags.allows_town_teleport());
    }

    #[test]
    fn contents_type_uses_the_packers_spelling() {
        let item: ContentsType = serde_json::from_str("\"item\"").unwrap();
        let meseta: ContentsType = serde_json::from_str("\"meseta\"").unwrap();
        assert_eq!(item, ContentsType::Item);
        assert_eq!(meseta, ContentsType::Meseta);
        assert!(serde_json::from_str::<ContentsType>("\"gold\"").is_err());
    }

    #[test]
    fn transition_tables_are_the_roms_two() {
        assert_eq!(
            serde_json::from_str::<TransitionTable>("1").unwrap(),
            TransitionTable::Normal
        );
        assert_eq!(
            serde_json::from_str::<TransitionTable>("2").unwrap(),
            TransitionTable::MapChange
        );
        let err = serde_json::from_str::<TransitionTable>("3")
            .unwrap_err()
            .to_string();
        assert!(err.contains("tables 1 and 2"), "{err}");
        assert_eq!(
            serde_json::to_string(&TransitionTable::MapChange).unwrap(),
            "2"
        );
    }

    #[test]
    fn facing_decodes_the_four_bytes_and_tolerates_the_rest() {
        let up: Facing = serde_json::from_str(r#"{"id": 4, "name": "up"}"#).unwrap();
        assert_eq!(up.direction(), Some(Direction::Up));
        let odd: Facing = serde_json::from_str(r#"{"id": 7, "name": null}"#).unwrap();
        assert_eq!(odd.direction(), None);
        assert_eq!(odd.id, 7);
    }
}
