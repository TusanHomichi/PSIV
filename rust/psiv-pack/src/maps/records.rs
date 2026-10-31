//! `FieldMapPtrs` and the per-map record grammar: `psiv_tools/maps/records.py`,
//! which owns the retail citations. `GameMode_LoadFieldMap` is the whole
//! specification: a record has no length or section offsets, so it is read by
//! replaying the loader's `a0`, section by section.

use crate::PackError;
use crate::json::Json;
use crate::map_symbols::{FIELD_OBJECT_SYMBOLS, MAP_SYMBOLS};
use crate::png::hex;
use crate::sound_defs::MUSIC_NAMES;

pub const FIELD_MAP_PTRS: usize = 0x100000;
pub const MAP_COUNT: usize = 417;
pub const POINTER_SIZE: usize = 4;
/// Every `PtrMap_Null*` entry points at `ErrorTrap`.
pub const ERROR_TRAP: usize = 0x000200;
/// The table's first 16 longs: entries 3 to $F are the first run of null
/// maps, so this pins both the address and the null convention.
const SIGNATURE: &str = "001006840010fea20011966400000200000002000000020000000200000002000000\
                         020000000200000002000000020000000200000002000000020000000200";

const WORD_TERMINATOR: u16 = 0xFFFF;
const BYTE_TERMINATOR: u16 = 0xFF;
/// `loc_51A1A` only: a "decompress to RAM" marker before a 6-byte body.
const SPRITE_TO_RAM_MARKER: u16 = 0xFFFE;

const TRANSITION_SIZE: usize = 10;
const OBJECT_SIZE: usize = 10;
const INTERACTION_SIZE: usize = 10;
const CHEST_SIZE: usize = 6;
const TILE_ANIMATION_SIZE: usize = 2;

/// `loc_53F14`: 48 CRAM words, for lines 0, 1 and 3.
const PALETTE_SIZE: usize = 96;
const PALETTE_LINES: [u8; 3] = [0, 1, 3];
const PALETTE_LINE_FROM_PAL_INIT: u8 = 2;

/// Jump-table sizes, hard bounds on the index a record stores.
const MAP_UPDATE_ROUTINES: u16 = 0x40;
const EVENT_ROUTINES: u16 = 0x80;
const MAP_DATA_MANAGER_ROUTINES: u16 = 0xA0;
const FIELD_OBJECT_STRIDE: u16 = 4;
const INTERACTION_ROUTINES: u8 = 8;

const MUSIC_ID_BASE: i32 = 0x81;
const XY_RANGES: [&str; 15] = [
    "Null",
    "XYPlus40",
    "XYPlus20",
    "XYExact",
    "XLower",
    "XHigher",
    "YLower",
    "YHigher",
    "XYLowerWithPlayerY",
    "XPlus20_YPlus10",
    "XPlus10_YPlus60",
    "XPlus40_YPlus20",
    "XPlus10_YPlus20",
    "XPlus60_YPlus10",
    "XPlus40_YPlus10",
];
/// Chest byte 3 of a meseta chest counts hundreds (`loc_66BEE`).
const CHEST_MESETA_MULTIPLIER: u32 = 100;

fn be16(data: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes([data[offset], data[offset + 1]])
}

pub(crate) fn be32(data: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

pub(crate) fn hex6(value: usize) -> String {
    format!("0x{value:06X}")
}

/// `_map_reference`: the id, its hex form and its symbol, if any.
pub fn map_reference(map_id: usize) -> Json {
    Json::object()
        .with("id", map_id)
        .with("id_hex", format!("0x{map_id:03X}"))
        .with("symbol", MAP_SYMBOLS.get(map_id).copied())
}

fn xy_range(value: u16) -> Json {
    Json::object()
        .with("id", value)
        .with("name", XY_RANGES.get(usize::from(value)).copied())
}

/// Where a section sits, the part of its JSON every section shares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    fn json(self, data: &[u8]) -> Json {
        Json::object()
            .with("rom_offset", hex6(self.start))
            .with("rom_end_exclusive", hex6(self.end))
            .with("size_bytes", self.end - self.start)
            .with("raw_hex", hex(&data[self.start..self.end]))
    }

    pub fn len(self) -> usize {
        self.end - self.start
    }

    pub fn is_empty(self) -> bool {
        self.start == self.end
    }
}

/// A replay of the loader's `a0`.
struct Cursor<'a> {
    data: &'a [u8],
    offset: usize,
    map_id: usize,
    symbol: &'static str,
}

impl Cursor<'_> {
    fn fail(&self, message: impl std::fmt::Display) -> PackError {
        PackError(format!(
            "map 0x{:03X} ({}) at {}: {message}",
            self.map_id,
            self.symbol,
            hex6(self.offset)
        ))
    }

    fn check(&self, size: usize) -> Result<(), PackError> {
        if self.offset + size > self.data.len() {
            return Err(self.fail(format!(
                "record runs past the end of the ROM reading {size} bytes"
            )));
        }
        Ok(())
    }

    fn byte(&mut self) -> Result<u8, PackError> {
        self.check(1)?;
        self.offset += 1;
        Ok(self.data[self.offset - 1])
    }

    fn word(&mut self) -> Result<u16, PackError> {
        let value = self.peek_word()?;
        self.offset += 2;
        Ok(value)
    }

    fn long(&mut self) -> Result<u32, PackError> {
        self.check(4)?;
        self.offset += 4;
        Ok(be32(self.data, self.offset - 4))
    }

    fn peek_word(&self) -> Result<u16, PackError> {
        self.check(2)?;
        Ok(be16(self.data, self.offset))
    }

    fn peek_byte(&self) -> Result<u8, PackError> {
        self.check(1)?;
        Ok(self.data[self.offset])
    }

    fn skip(&mut self, size: usize) -> Result<usize, PackError> {
        self.check(size)?;
        self.offset += size;
        Ok(self.offset - size)
    }

    fn since(&self, start: usize) -> Span {
        Span {
            start,
            end: self.offset,
        }
    }

    fn pointer(&mut self, what: &str) -> Result<usize, PackError> {
        let value = self.long()? as usize;
        if value == 0 || value >= self.data.len() {
            return Err(self.fail(format!("{what} pointer {} is outside the ROM", hex6(value))));
        }
        Ok(value)
    }
}

/// One (word, Kosinski pointer) pair: `loc_519D2` or `loc_51A5C`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtEntry {
    pub span: Span,
    pub word: u16,
    pub source: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtList {
    pub span: Span,
    pub terminator: u16,
    pub entries: Vec<ArtEntry>,
}

/// One `loc_51A1A` entry: $FFFE before the body sends it to RAM.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NemesisSprite {
    pub span: Span,
    pub to_ram: bool,
    pub tile_number: u16,
    pub source: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NemesisSprites {
    pub span: Span,
    pub terminator: u16,
    pub entries: Vec<NemesisSprite>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dimensions {
    pub span: Span,
    pub fg_row_size: u8,
    pub fg_column_size: u8,
    pub bg_row_size: u8,
    pub bg_column_size: u8,
}

/// A plane's half of `loc_51AB2`: the step counters follow a zero mode byte.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScrollPlane {
    pub word: u16,
    pub step_counters: Option<(u32, u32)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scroll {
    pub span: Span,
    pub mode: u8,
    pub padding: u8,
    pub fg: ScrollPlane,
    pub bg: ScrollPlane,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chunks {
    pub span: Span,
    pub pointers: Vec<usize>,
}

/// A terminated list of jump-table indexes, of bytes or of words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexList {
    pub span: Span,
    pub width: usize,
    pub ids: Vec<u16>,
}

/// A fixed-size record: where it sits and what it decodes to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry<T> {
    pub rom_offset: usize,
    pub record: T,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fixed<T> {
    pub span: Span,
    pub terminator: u16,
    pub record_size: usize,
    pub entries: Vec<Entry<T>>,
}

/// `DoMapTransitionData`, 10 bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transition {
    pub source_x_tile: u8,
    pub source_y_tile: u8,
    pub range: u16,
    pub target: u16,
    pub destination_x_tile: u8,
    pub destination_y_tile: u8,
    pub facing_dir: u8,
    pub character_alignment: u8,
}

/// `LoadMapObjects`, 10 bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Object {
    pub object_id: u16,
    pub facing_dir: u8,
    pub dialogue_id: u8,
    pub art_tile: u16,
    pub x_tile: u16,
    pub y_tile: u16,
}

impl Object {
    /// The `FieldObjectsJmpTbl` name, for an id on the table's stride.
    pub fn symbol(&self) -> Option<&'static str> {
        self.object_id
            .is_multiple_of(FIELD_OBJECT_STRIDE)
            .then(|| {
                FIELD_OBJECT_SYMBOLS
                    .get(usize::from(self.object_id / FIELD_OBJECT_STRIDE))
                    .copied()
            })
            .flatten()
    }
}

/// `LoadTreasureChests`, 6 bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chest {
    pub white_chest: bool,
    pub is_meseta: bool,
    pub chest_flag: u8,
    pub contents: u8,
    pub x_tile: u8,
    pub y_tile: u8,
}

/// `Interaction_ChkMapAreas`, 10 bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Interaction {
    pub x_tile: u16,
    pub y_tile: u16,
    pub range: u16,
    pub flag_type: u8,
    pub flag: u8,
    pub interaction_type: u8,
    pub parameter: u8,
}

/// `LoadTileAnimations`, 2 bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TileAnimation {
    pub animation_id: u8,
    pub parameter: u8,
}

trait Record: Sized {
    fn decode(raw: &[u8]) -> Self;
    fn json(&self) -> Json;
}

fn word_at(raw: &[u8], at: usize) -> u16 {
    be16(raw, at)
}

impl Record for Transition {
    fn decode(raw: &[u8]) -> Self {
        Transition {
            source_x_tile: raw[0],
            source_y_tile: raw[1],
            range: word_at(raw, 2),
            target: word_at(raw, 4),
            destination_x_tile: raw[6],
            destination_y_tile: raw[7],
            facing_dir: raw[8],
            character_alignment: raw[9],
        }
    }

    fn json(&self) -> Json {
        let point = |x: u8, y: u8| {
            Json::object()
                .with("x_tile", x)
                .with("y_tile", y)
                .with("x", u32::from(x) * 16)
                .with("y", u32::from(y) * 16)
        };
        Json::object()
            .with("source", point(self.source_x_tile, self.source_y_tile))
            .with("range", xy_range(self.range))
            .with("target", map_reference(usize::from(self.target)))
            .with(
                "destination",
                point(self.destination_x_tile, self.destination_y_tile),
            )
            .with("facing_dir", self.facing_dir)
            .with("character_alignment", self.character_alignment)
    }
}

impl Record for Object {
    fn decode(raw: &[u8]) -> Self {
        Object {
            object_id: word_at(raw, 0),
            facing_dir: raw[2],
            dialogue_id: raw[3],
            art_tile: word_at(raw, 4),
            x_tile: word_at(raw, 6),
            y_tile: word_at(raw, 8),
        }
    }

    fn json(&self) -> Json {
        Json::object()
            .with("object_id", self.object_id)
            .with("symbol", self.symbol())
            .with("facing_dir", self.facing_dir)
            .with("dialogue_id", self.dialogue_id)
            .with("art_tile", self.art_tile)
            .with("x_tile", self.x_tile)
            .with("y_tile", self.y_tile)
            .with("x", u32::from(self.x_tile) * 8)
            .with("y", u32::from(self.y_tile) * 8)
    }
}

impl Record for Chest {
    fn decode(raw: &[u8]) -> Self {
        Chest {
            white_chest: raw[0] != 0,
            is_meseta: raw[1] != 0,
            chest_flag: raw[2],
            contents: raw[3],
            x_tile: raw[4],
            y_tile: raw[5],
        }
    }

    fn json(&self) -> Json {
        let (item, meseta) = if self.is_meseta {
            (
                None,
                Some(u32::from(self.contents) * CHEST_MESETA_MULTIPLIER),
            )
        } else {
            (Some(self.contents), None)
        };
        Json::object()
            .with("white_chest", self.white_chest)
            .with(
                "object_symbol",
                if self.white_chest {
                    "WhiteTreasureChest"
                } else {
                    "TreasureChest"
                },
            )
            .with(
                "contents_type",
                if self.is_meseta { "meseta" } else { "item" },
            )
            .with("item_id", item)
            .with("meseta", meseta)
            .with("chest_flag", self.chest_flag)
            .with("x_tile", self.x_tile)
            .with("y_tile", self.y_tile)
            .with("x", u32::from(self.x_tile) * 16)
            .with("y", u32::from(self.y_tile) * 16)
    }
}

impl Record for Interaction {
    fn decode(raw: &[u8]) -> Self {
        Interaction {
            x_tile: word_at(raw, 0),
            y_tile: word_at(raw, 2),
            range: word_at(raw, 4),
            flag_type: raw[6],
            flag: raw[7],
            interaction_type: raw[8],
            parameter: raw[9],
        }
    }

    fn json(&self) -> Json {
        let flag_name = match self.flag_type {
            0 => Some("story"),
            1 => Some("chest"),
            2 => Some("temporary"),
            _ => None,
        };
        Json::object()
            .with("x_tile", self.x_tile)
            .with("y_tile", self.y_tile)
            .with("x", u32::from(self.x_tile) * 8)
            .with("y", u32::from(self.y_tile) * 8)
            .with("range", xy_range(self.range))
            .with(
                "flag_type",
                Json::object()
                    .with("id", self.flag_type)
                    .with("name", flag_name),
            )
            .with("flag", self.flag)
            .with("interaction_type", self.interaction_type)
            .with("parameter", self.parameter)
    }
}

impl Record for TileAnimation {
    fn decode(raw: &[u8]) -> Self {
        TileAnimation {
            animation_id: raw[0],
            parameter: raw[1],
        }
    }

    fn json(&self) -> Json {
        Json::object()
            .with("animation_id", self.animation_id)
            .with("parameter", self.parameter)
    }
}

/// `loc_539E2` and `loc_53A04`; the world maps read no bytes (`None`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayoutPointers {
    pub span: Span,
    pub planes: Option<(usize, usize)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DialoguePointer {
    pub span: Span,
    pub pointer: usize,
    pub tree: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PalettePointer {
    pub span: Span,
    pub pointer: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Flags {
    pub span: Span,
    pub poison: u8,
    pub random_battles: u8,
    pub town_teleport: u8,
    pub dungeon_teleport: u8,
}

/// One real map's record, as the loader walks it.
#[derive(Clone, Debug, PartialEq)]
pub struct MapRecord {
    pub id: usize,
    pub symbol: &'static str,
    pub pointer: usize,
    pub end: usize,
    pub general_var: u8,
    pub music_id: u8,
    pub tilesets: ArtList,
    pub sprites: NemesisSprites,
    pub sprite_data: ArtList,
    pub dimensions: Dimensions,
    pub scroll: Scroll,
    pub chunks: Chunks,
    pub map_updates: IndexList,
    pub transitions: Fixed<Transition>,
    pub transitions_2: Fixed<Transition>,
    pub objects: Fixed<Object>,
    pub treasure_chests: Fixed<Chest>,
    pub tile_animations: Fixed<TileAnimation>,
    pub layout: LayoutPointers,
    pub dialogue: DialoguePointer,
    pub interaction_areas: Fixed<Interaction>,
    pub events: IndexList,
    pub palette: PalettePointer,
    pub flags: Flags,
    pub map_data_manager: IndexList,
    /// `extract_maps`' encounter binding: research data the game does not
    /// read from here (the battle section writes its own table).
    pub encounters: Json,
}

fn read_art_list(
    cursor: &mut Cursor,
    what: &str,
    negative_terminator: bool,
) -> Result<ArtList, PackError> {
    let start = cursor.offset;
    let mut entries = Vec::new();
    loop {
        let head = cursor.peek_word()?;
        if head == WORD_TERMINATOR || (negative_terminator && head >= 0x8000) {
            break;
        }
        let entry_start = cursor.offset;
        let word = cursor.word()?;
        let source = cursor.pointer(what)?;
        entries.push(ArtEntry {
            span: cursor.since(entry_start),
            word,
            source,
        });
    }
    let terminator = cursor.word()?;
    Ok(ArtList {
        span: cursor.since(start),
        terminator,
        entries,
    })
}

fn read_nemesis_sprites(cursor: &mut Cursor) -> Result<NemesisSprites, PackError> {
    let start = cursor.offset;
    let mut entries = Vec::new();
    let terminator = loop {
        let entry_start = cursor.offset;
        let head = cursor.word()?;
        let to_ram = head == SPRITE_TO_RAM_MARKER;
        if !to_ram && head >= 0x8000 {
            break head;
        }
        let tile_number = if to_ram { cursor.word()? } else { head };
        let source = cursor.pointer("sprite art")?;
        entries.push(NemesisSprite {
            span: cursor.since(entry_start),
            to_ram,
            tile_number,
            source,
        });
    };
    Ok(NemesisSprites {
        span: cursor.since(start),
        terminator,
        entries,
    })
}

fn read_scroll_plane(cursor: &mut Cursor) -> Result<ScrollPlane, PackError> {
    let word = cursor.word()?;
    let step_counters = if word & 0xFF == 0 {
        Some((cursor.long()?, cursor.long()?))
    } else {
        None
    };
    Ok(ScrollPlane {
        word,
        step_counters,
    })
}

fn read_scroll(cursor: &mut Cursor) -> Result<Scroll, PackError> {
    let start = cursor.offset;
    let mode = cursor.byte()?;
    let padding = cursor.byte()?;
    let fg = read_scroll_plane(cursor)?;
    let bg = read_scroll_plane(cursor)?;
    Ok(Scroll {
        span: cursor.since(start),
        mode,
        padding,
        fg,
        bg,
    })
}

fn read_chunks(cursor: &mut Cursor) -> Result<Chunks, PackError> {
    let start = cursor.offset;
    let mut pointers = Vec::new();
    while cursor.peek_word()? != WORD_TERMINATOR {
        pointers.push(cursor.pointer("chunk table")?);
    }
    cursor.word()?;
    Ok(Chunks {
        span: cursor.since(start),
        pointers,
    })
}

fn read_index_list(
    cursor: &mut Cursor,
    width: usize,
    limit: u16,
    what: &str,
) -> Result<IndexList, PackError> {
    let start = cursor.offset;
    let terminator = if width == 1 {
        BYTE_TERMINATOR
    } else {
        WORD_TERMINATOR
    };
    let mut ids = Vec::new();
    loop {
        let head = if width == 1 {
            u16::from(cursor.peek_byte()?)
        } else {
            cursor.peek_word()?
        };
        cursor.offset += width;
        if head == terminator {
            break;
        }
        if head >= limit {
            return Err(cursor.fail(format!(
                "{what} index 0x{head:0digits$X} is outside the {limit}-entry jump table",
                digits = width * 2
            )));
        }
        ids.push(head);
    }
    Ok(IndexList {
        span: cursor.since(start),
        width,
        ids,
    })
}

/// `GoPast_FFFF_Terminator`: step words until $FFFF, and return its address.
fn scan_to_word_terminator(cursor: &Cursor, mut offset: usize) -> Result<usize, PackError> {
    loop {
        if offset + 2 > cursor.data.len() {
            return Err(cursor.fail("section is not $FFFF-terminated before the end of the ROM"));
        }
        if be16(cursor.data, offset) == WORD_TERMINATOR {
            return Ok(offset);
        }
        offset += 2;
    }
}

fn read_fixed<T: Record>(
    cursor: &mut Cursor,
    size: usize,
    what: &str,
    negative_terminator: bool,
    cross_check: bool,
) -> Result<Fixed<T>, PackError> {
    let start = cursor.offset;
    let mut entries = Vec::new();
    loop {
        let head = cursor.peek_word()?;
        if head == WORD_TERMINATOR || (negative_terminator && head >= 0x8000) {
            break;
        }
        let rom_offset = cursor.skip(size)?;
        entries.push(Entry {
            rom_offset,
            record: T::decode(&cursor.data[rom_offset..rom_offset + size]),
        });
    }
    if cross_check {
        let scanned = scan_to_word_terminator(cursor, start)?;
        if scanned != cursor.offset {
            return Err(cursor.fail(format!(
                "{what}: the record walk ends at {} but GoPast_FFFF_Terminator stops at {}; \
                 the loader's two paths through this section disagree",
                hex6(cursor.offset),
                hex6(scanned)
            )));
        }
    }
    let terminator = cursor.word()?;
    Ok(Fixed {
        span: cursor.since(start),
        terminator,
        record_size: size,
        entries,
    })
}

/// Walk one record. `dialogue_trees` is `(start offset, tree)`, in the
/// region's order.
pub fn walk_map(
    data: &[u8],
    map_id: usize,
    pointer: usize,
    dialogue_trees: &[(usize, usize)],
) -> Result<MapRecord, PackError> {
    let symbol = MAP_SYMBOLS[map_id];
    let mut c = Cursor {
        data,
        offset: pointer,
        map_id,
        symbol,
    };
    let cursor = &mut c;
    let general_var = cursor.byte()?;
    let music_id = cursor.byte()?;
    let tilesets = read_art_list(cursor, "tileset art", true)?;
    let sprites = read_nemesis_sprites(cursor)?;
    let sprite_data = read_art_list(cursor, "sprite data", false)?;

    let start = cursor.offset;
    let (fg_row_size, fg_column_size, bg_row_size, bg_column_size) = (
        cursor.byte()?,
        cursor.byte()?,
        cursor.byte()?,
        cursor.byte()?,
    );
    let dimensions = Dimensions {
        span: cursor.since(start),
        fg_row_size,
        fg_column_size,
        bg_row_size,
        bg_column_size,
    };

    let scroll = read_scroll(cursor)?;
    let chunks = read_chunks(cursor)?;
    let map_updates = read_index_list(cursor, 1, MAP_UPDATE_ROUTINES, "map update")?;
    let transitions = read_fixed(cursor, TRANSITION_SIZE, "transitions", false, true)?;
    let transitions_2 = read_fixed(cursor, TRANSITION_SIZE, "transitions 2", false, true)?;
    let objects = read_fixed(cursor, OBJECT_SIZE, "objects", true, true)?;
    let treasure_chests = read_fixed(cursor, CHEST_SIZE, "treasure chests", true, true)?;
    let tile_animations = read_fixed(cursor, TILE_ANIMATION_SIZE, "tile animations", true, false)?;

    let start = cursor.offset;
    // The world maps (MapID 0 and 1) take loc_107DC2 / loc_10BE02 instead.
    let planes = if map_id & 0xFFFE != 0 {
        let fg = cursor.pointer("FG layout")?;
        let bg = cursor.pointer("BG layout")?;
        Some((fg, bg))
    } else {
        None
    };
    let layout = LayoutPointers {
        span: cursor.since(start),
        planes,
    };

    let start = cursor.offset;
    let dialogue_pointer = cursor.pointer("dialogue tree")?;
    let Some(&(_, tree)) = dialogue_trees.iter().find(|(s, _)| *s == dialogue_pointer) else {
        return Err(cursor.fail(format!(
            "dialogue pointer {} does not match the start of any of the 43 Kosinski \
             dialogue trees",
            hex6(dialogue_pointer)
        )));
    };
    let dialogue = DialoguePointer {
        span: cursor.since(start),
        pointer: dialogue_pointer,
        tree,
    };

    let interaction_areas: Fixed<Interaction> =
        read_fixed(cursor, INTERACTION_SIZE, "interaction areas", false, true)?;
    for entry in &interaction_areas.entries {
        if entry.record.interaction_type >= INTERACTION_ROUTINES {
            return Err(cursor.fail(format!(
                "interaction area at {} selects routine {}, outside InteractionRoutines",
                hex6(entry.rom_offset),
                entry.record.interaction_type
            )));
        }
    }
    let events = read_index_list(cursor, 1, EVENT_ROUTINES, "event")?;

    let start = cursor.offset;
    let palette_pointer = cursor.pointer("palette")?;
    if palette_pointer + PALETTE_SIZE > data.len() {
        return Err(cursor.fail(format!(
            "palette at {} runs past the end of the ROM",
            hex6(palette_pointer)
        )));
    }
    let palette = PalettePointer {
        span: cursor.since(start),
        pointer: palette_pointer,
    };

    let start = cursor.offset;
    let (poison, random_battles, town_teleport, dungeon_teleport) = (
        cursor.byte()?,
        cursor.byte()?,
        cursor.byte()?,
        cursor.byte()?,
    );
    let flags = Flags {
        span: cursor.since(start),
        poison,
        random_battles,
        town_teleport,
        dungeon_teleport,
    };

    let map_data_manager = read_index_list(cursor, 2, MAP_DATA_MANAGER_ROUTINES, "MapDataManager")?;

    Ok(MapRecord {
        id: map_id,
        symbol,
        pointer,
        end: cursor.offset,
        general_var,
        music_id,
        tilesets,
        sprites,
        sprite_data,
        dimensions,
        scroll,
        chunks,
        map_updates,
        transitions,
        transitions_2,
        objects,
        treasure_chests,
        tile_animations,
        layout,
        dialogue,
        interaction_areas,
        events,
        palette,
        flags,
        map_data_manager,
        encounters: Json::Null,
    })
}

fn extend(mut base: Json, more: Json) -> Json {
    let Json::Object(pairs) = more else {
        panic!("extend with a non-object");
    };
    for (key, value) in pairs {
        base.insert(key, value);
    }
    base
}

fn art_list_json(list: &ArtList, data: &[u8], key: &str, sign_extend: bool) -> Json {
    let entries: Vec<Json> = list
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let value: Json = if sign_extend {
                format!("0x{:08X}", 0xFFFF0000u32 | u32::from(entry.word)).into()
            } else {
                entry.word.into()
            };
            Json::object()
                .with("index", index)
                .with("rom_offset", hex6(entry.span.start))
                .with(key, value)
                .with("source_offset", hex6(entry.source))
                .with("compression", "kosinski")
                .with("raw_hex", hex(&data[entry.span.start..entry.span.end]))
        })
        .collect();
    list.span
        .json(data)
        .with("terminator", format!("0x{:04X}", list.terminator))
        .with("entry_count", entries.len())
        .with("entries", entries)
}

fn scroll_plane_json(base: Json, plane: &str, scroll: &ScrollPlane) -> Json {
    let counters = scroll.step_counters.map(|(x, y)| {
        Json::object()
            .with("x", format!("0x{x:08X}"))
            .with("y", format!("0x{y:08X}"))
    });
    base.with(format!("{plane}_scroll_mode"), scroll.word & 0xFF)
        .with(
            format!("{plane}_scroll_word"),
            format!("0x{:04X}", scroll.word),
        )
        .with(format!("{plane}_step_counters"), counters)
}

fn index_list_json(list: &IndexList, data: &[u8]) -> Json {
    let terminator = if list.width == 1 {
        format!("0x{BYTE_TERMINATOR:02X}")
    } else {
        format!("0x{WORD_TERMINATOR:04X}")
    };
    list.span
        .json(data)
        .with("terminator", terminator)
        .with("count", list.ids.len())
        .with("ids", list.ids.clone())
}

fn fixed_json<T: Record>(list: &Fixed<T>, data: &[u8]) -> Json {
    let entries: Vec<Json> = list
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            extend(
                Json::object()
                    .with("index", index)
                    .with("rom_offset", hex6(entry.rom_offset)),
                entry.record.json(),
            )
            .with(
                "raw_hex",
                hex(&data[entry.rom_offset..entry.rom_offset + list.record_size]),
            )
        })
        .collect();
    list.span
        .json(data)
        .with("terminator", format!("0x{:04X}", list.terminator))
        .with("record_size", list.record_size)
        .with("count", entries.len())
        .with("entries", entries)
}

impl MapRecord {
    /// The record as `_walk_map` returns it, plus the encounter binding.
    pub fn to_json(&self, data: &[u8]) -> Json {
        let index = i32::from(self.music_id) - MUSIC_ID_BASE;
        let music_symbol = usize::try_from(index)
            .ok()
            .and_then(|i| MUSIC_NAMES.get(i).copied());
        let music = Json::object()
            .with("id", self.music_id)
            .with("id_hex", format!("0x{:02X}", self.music_id))
            .with("symbol", music_symbol)
            .with("changes_music", self.music_id != 0);

        let sprites: Vec<Json> = self
            .sprites
            .entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                Json::object()
                    .with("index", index)
                    .with("rom_offset", hex6(entry.span.start))
                    .with("destination", if entry.to_ram { "ram" } else { "vram" })
                    .with("tile_number", entry.tile_number)
                    .with("source_offset", hex6(entry.source))
                    .with("compression", "nemesis")
                    .with("raw_hex", hex(&data[entry.span.start..entry.span.end]))
            })
            .collect();
        let sprites = self
            .sprites
            .span
            .json(data)
            .with("terminator", format!("0x{:04X}", self.sprites.terminator))
            .with("entry_count", sprites.len())
            .with("entries", sprites);

        let dimensions = self
            .dimensions
            .span
            .json(data)
            .with("fg_row_size", self.dimensions.fg_row_size)
            .with("fg_column_size", self.dimensions.fg_column_size)
            .with("bg_row_size", self.dimensions.bg_row_size)
            .with("bg_column_size", self.dimensions.bg_column_size);

        let scroll = self
            .scroll
            .span
            .json(data)
            .with("mode", self.scroll.mode)
            .with("padding_byte", format!("0x{:02X}", self.scroll.padding));
        let scroll = scroll_plane_json(scroll, "fg", &self.scroll.fg);
        let scroll = scroll_plane_json(scroll, "bg", &self.scroll.bg);

        let chunks = self
            .chunks
            .span
            .json(data)
            .with("terminator", format!("0x{WORD_TERMINATOR:04X}"))
            .with("pointer_count", self.chunks.pointers.len())
            .with("compression", "kosinski")
            .with(
                "pointers",
                self.chunks
                    .pointers
                    .iter()
                    .map(|p| hex6(*p))
                    .collect::<Vec<_>>(),
            );

        let layout = self.layout.span.json(data);
        let layout = match self.layout.planes {
            Some((fg, bg)) => layout
                .with("present", true)
                .with("fg_offset", hex6(fg))
                .with("bg_offset", hex6(bg))
                .with("compression", "kosinski"),
            None => layout.with("present", false).with(
                "reason",
                "loc_539E2/loc_53A04 test Field_Map_Index & $FFFE and take the world-map \
                 path, which reads no bytes from the record",
            ),
        };

        let dialogue = self
            .dialogue
            .span
            .json(data)
            .with("pointer", hex6(self.dialogue.pointer))
            .with("compression", "kosinski")
            .with("tree", self.dialogue.tree)
            .with("label", format!("DialogueTree{}", self.dialogue.tree));

        let pointer = self.palette.pointer;
        let mut lines = Json::object();
        for (slot, line) in PALETTE_LINES.iter().enumerate() {
            let chunk = pointer + slot * 32;
            lines.insert(
                line.to_string(),
                Json::object()
                    .with("rom_offset", hex6(chunk))
                    .with("raw_hex", hex(&data[chunk..chunk + 32])),
            );
        }
        let palette = self
            .palette
            .span
            .json(data)
            .with("pointer", hex6(pointer))
            .with("palette_size_bytes", PALETTE_SIZE)
            .with("lines_from_map", PALETTE_LINES.to_vec())
            .with("line_from_pal_init_3", PALETTE_LINE_FROM_PAL_INIT)
            .with("lines", lines);

        let teleport = self.flags.dungeon_teleport;
        let flags = self
            .flags
            .span
            .json(data)
            .with("poison", self.flags.poison)
            .with("random_battles", self.flags.random_battles)
            .with("town_teleport", self.flags.town_teleport)
            .with(
                "dungeon_teleport_index",
                (teleport & 0x80 == 0).then_some(teleport),
            )
            .with("dungeon_teleport_raw", format!("0x{teleport:02X}"));

        map_reference(self.id)
            .with("is_null", false)
            .with(
                "pointer_offset",
                hex6(FIELD_MAP_PTRS + self.id * POINTER_SIZE),
            )
            .with("pointer", hex6(self.pointer))
            .with("rom_offset", hex6(self.pointer))
            .with("rom_end_exclusive", hex6(self.end))
            .with("size_bytes", self.end - self.pointer)
            .with("general_var", self.general_var)
            .with("music", music)
            .with(
                "tilesets",
                art_list_json(&self.tilesets, data, "vram_tile", false),
            )
            .with("sprites", sprites)
            .with(
                "sprite_data",
                art_list_json(&self.sprite_data, data, "ram_address", true),
            )
            .with("dimensions", dimensions)
            .with("scroll", scroll)
            .with("chunks", chunks)
            .with("map_updates", index_list_json(&self.map_updates, data))
            .with("transitions", fixed_json(&self.transitions, data))
            .with("transitions_2", fixed_json(&self.transitions_2, data))
            .with("objects", fixed_json(&self.objects, data))
            .with("treasure_chests", fixed_json(&self.treasure_chests, data))
            .with("tile_animations", fixed_json(&self.tile_animations, data))
            .with("layout", layout)
            .with("dialogue", dialogue)
            .with(
                "interaction_areas",
                fixed_json(&self.interaction_areas, data),
            )
            .with("events", index_list_json(&self.events, data))
            .with("palette", palette)
            .with("flags", flags)
            .with(
                "map_data_manager",
                index_list_json(&self.map_data_manager, data),
            )
            .with("raw_hex", hex(&data[self.pointer..self.end]))
            .with("encounters", self.encounters.clone())
    }
}

/// `_check_pointer_table`, returning the table's JSON.
pub fn check_pointer_table(data: &[u8]) -> Result<Json, PackError> {
    let signature: Vec<u8> = (0..SIGNATURE.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&SIGNATURE[i..i + 2], 16).expect("hex signature"))
        .collect();
    let start = FIELD_MAP_PTRS;
    let end = start + MAP_COUNT * POINTER_SIZE;
    let actual = data.get(start..start + signature.len());
    if actual != Some(signature.as_slice()) {
        return Err(PackError(format!(
            "FieldMapPtrs signature mismatch at {}: expected {SIGNATURE}, got {}",
            hex6(start),
            hex(actual.unwrap_or_default())
        )));
    }
    let occurrences = crate::m68k::find_all(data, SIGNATURE).len();
    if occurrences != 1 {
        return Err(PackError(format!(
            "FieldMapPtrs signature occurs {occurrences} times in this ROM; it must be unique \
             for the offset to be self-verifying"
        )));
    }
    let first = be32(data, start) as usize;
    if first != end {
        return Err(PackError(format!(
            "FieldMapPtrs is {MAP_COUNT} longs ending at {}, but its first entry (Map_Motavia) \
             is {}; the table is supposed to be flush against the first map record",
            hex6(end),
            hex6(first)
        )));
    }
    Ok(Json::object()
        .with("label", "FieldMapPtrs")
        .with("rom_offset", hex6(start))
        .with("rom_end_exclusive", hex6(end))
        .with("entry_size", POINTER_SIZE)
        .with("count", MAP_COUNT)
        .with("signature_hex", SIGNATURE)
        .with("error_trap", hex6(ERROR_TRAP))
        .with(
            "indexing",
            "Field_Map_Index * 4; the long is an absolute 68000 address and the ROM maps at \
             0, so it is also the retail file offset",
        )
        .with("flush_against_first_record", hex6(first)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const AT: usize = 0x10;
    const TREE: usize = 0x20;
    const ART: [u8; 4] = [0, 0, 0, 0x30];

    /// A minimal record for a non-world map, with `edit` applied to its parts
    /// in the loader's order.
    fn record_parts() -> Vec<Vec<u8>> {
        let mut layout = ART.to_vec();
        layout.extend(ART);
        vec![
            vec![7, 0x81],             // general var, music
            vec![0xFF, 0xFF],          // tilesets
            vec![0xFF, 0xFF],          // Nemesis sprites
            vec![0xFF, 0xFF],          // Kosinski sprites
            vec![1, 2, 3, 4],          // dimensions
            vec![0, 0, 0, 1, 0, 1],    // scroll: no step counters
            vec![0xFF, 0xFF],          // chunks
            vec![0xFF],                // map updates
            vec![0xFF, 0xFF],          // transitions
            vec![0xFF, 0xFF],          // transitions 2
            vec![0xFF, 0xFF],          // objects
            vec![0xFF, 0xFF],          // chests
            vec![0xFF, 0xFF],          // tile animations
            layout,                    // FG and BG layout
            vec![0, 0, 0, TREE as u8], // dialogue tree
            vec![0xFF, 0xFF],          // interaction areas
            vec![0xFF],                // events
            vec![0, 0, 0, 0x30],       // palette
            vec![0, 1, 2, 0x85],       // flags
            vec![0xFF, 0xFF],          // MapDataManager
        ]
    }

    fn walk(parts: &[Vec<u8>]) -> Result<MapRecord, PackError> {
        let mut rom = vec![0u8; AT];
        rom.extend(parts.concat());
        rom.resize(rom.len() + 0x100, 0);
        walk_map(&rom, 2, AT, &[(TREE, 5)])
    }

    #[test]
    fn a_minimal_record_walks_to_its_terminator() {
        let parts = record_parts();
        let record = walk(&parts).unwrap();
        assert_eq!(record.end, AT + parts.concat().len());
        assert_eq!(record.general_var, 7);
        assert_eq!(record.dialogue.tree, 5);
        assert_eq!(record.layout.planes, Some((0x30, 0x30)));
        assert_eq!(record.flags.dungeon_teleport, 0x85);
    }

    #[test]
    fn the_scroll_section_reads_counters_only_after_a_zero_mode() {
        let mut parts = record_parts();
        parts[5] = vec![0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 2, 0, 1];
        let record = walk(&parts).unwrap();
        assert_eq!(record.scroll.fg.step_counters, Some((1, 2)));
        assert_eq!(record.scroll.bg.step_counters, None);
    }

    #[test]
    fn fffe_in_the_nemesis_list_sends_an_entry_to_ram_and_goes_on() {
        let mut parts = record_parts();
        parts[2] = vec![
            0xFF, 0xFE, 0, 9, 0, 0, 0, 0x30, 0, 8, 0, 0, 0, 0x30, 0xFF, 0xFD,
        ];
        let record = walk(&parts).unwrap();
        let sprites = &record.sprites;
        assert_eq!(sprites.terminator, 0xFFFD);
        assert_eq!(sprites.entries.len(), 2);
        assert!(sprites.entries[0].to_ram);
        assert_eq!(sprites.entries[0].tile_number, 9);
        assert!(!sprites.entries[1].to_ram);
        assert_eq!(sprites.entries[1].tile_number, 8);
    }

    #[test]
    fn only_some_lists_end_on_any_negative_word() {
        // Tilesets end on $8000; Kosinski sprites read it as a tile number.
        let mut parts = record_parts();
        parts[1] = vec![0x80, 0x00];
        assert_eq!(walk(&parts).unwrap().tilesets.terminator, 0x8000);
        let mut parts = record_parts();
        parts[3] = vec![0x80, 0x00, 0, 0, 0, 0x30, 0xFF, 0xFF];
        assert_eq!(walk(&parts).unwrap().sprite_data.entries[0].word, 0x8000);
    }

    #[test]
    fn an_index_past_its_jump_table_is_refused() {
        let mut parts = record_parts();
        parts[7] = vec![0x40, 0xFF];
        let error = walk(&parts).unwrap_err().0;
        assert!(
            error.contains("map update index 0x40 is outside the 64-entry"),
            "{error}"
        );
        let mut parts = record_parts();
        parts[19] = vec![0x00, 0xA0, 0xFF, 0xFF];
        let error = walk(&parts).unwrap_err().0;
        assert!(error.contains("MapDataManager index 0x00A0"), "{error}");
    }

    #[test]
    fn the_walk_and_the_terminator_scan_must_agree() {
        // An object holding $FFFF at an even offset: the loader's other path
        // would stop inside it.
        let mut parts = record_parts();
        parts[10] = vec![0, 4, 0, 0, 0xFF, 0xFF, 0, 1, 0, 1, 0xFF, 0xFF];
        let error = walk(&parts).unwrap_err().0;
        assert!(
            error.contains("objects: the record walk ends at"),
            "{error}"
        );
    }

    #[test]
    fn a_pointer_outside_the_rom_and_an_unknown_tree_are_refused() {
        let mut parts = record_parts();
        parts[14] = vec![0, 0, 0, 0x21];
        let error = walk(&parts).unwrap_err().0;
        assert!(error.contains("does not match the start of any"), "{error}");
        let mut parts = record_parts();
        parts[17] = vec![0x7F, 0, 0, 0];
        let error = walk(&parts).unwrap_err().0;
        assert!(
            error.contains("palette pointer 0x7F000000 is outside"),
            "{error}"
        );
    }

    #[test]
    fn a_routine_past_interaction_routines_is_refused() {
        let mut parts = record_parts();
        parts[15] = vec![0, 0, 0, 0, 0, 0, 0, 0, 8, 0, 0xFF, 0xFF];
        let error = walk(&parts).unwrap_err().0;
        assert!(error.contains("selects routine 8"), "{error}");
    }

    #[test]
    fn a_rom_without_the_table_is_refused() {
        let error = check_pointer_table(&vec![0; 0x200000]).unwrap_err().0;
        assert!(
            error.starts_with("FieldMapPtrs signature mismatch at 0x100000"),
            "{error}"
        );
        let error = check_pointer_table(&[0; 16]).unwrap_err().0;
        assert!(error.ends_with(", got "), "{error}");
    }

    #[test]
    fn an_object_off_the_table_stride_has_no_symbol() {
        let object = |object_id| Object {
            object_id,
            facing_dir: 0,
            dialogue_id: 0,
            art_tile: 0,
            x_tile: 0,
            y_tile: 0,
        };
        assert_eq!(object(0).symbol(), Some(FIELD_OBJECT_SYMBOLS[0]));
        assert_eq!(object(2).symbol(), None);
        assert_eq!(object(0xFFFC).symbol(), None);
    }
}
