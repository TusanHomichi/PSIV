//! The two overworlds' paged layouts, MapID 0 (Motavia) and MapID 1 (Dezolis):
//! the port of `psiv_tools/overworld.py`, which owns the retail citations
//! (#216, slice 3), and of `pack_layouts.decode_map_section`, the one entry
//! point that knows both layout paths exist.
//!
//! A plane is sixteen raw 1,024-byte pages named by a table of longs, and the
//! hook routine each page copy jumps to rewrites cells from event flags.
//! [`PatchDecoder`] reads those routines out of the cartridge: it accepts the
//! encodings they use, and an unrecognised opcode is a failure rather than a
//! silently truncated patch.

use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};

use super::layouts::{
    Blob, COLLISION_CELL_TILES, Layout, MapLayout, MapLayoutSpec, Plane, collision_at,
    decode_chunks, decode_collision, decode_layout_section, decode_tilesets, layout_spec,
    spec_with_planes,
};
use super::records::{MapRecord, hex6};
use crate::PackError;
use crate::decode::Decodes;
use crate::json::Json;
use crate::map_symbols::event_flag_symbol;
use crate::png::hex;

/// `move.w #$FF,d2` around a `move.l (a0)+,(a1)+`: 256 longs.
pub const PAGE_BYTES: usize = 0x400;
/// `GetMapLayoutChunkFG`: `lsl.w #7,d1`.
pub const ROW_BYTES: usize = 128;
pub const PAGE_ROWS: usize = PAGE_BYTES / ROW_BYTES;
pub const PAGE_COUNT: usize = 16;
/// `andi.w #$1F,d1` in `GetMapLayoutChunkFG`: the RAM window is four pages.
pub const WINDOW_ROWS: usize = 32;
pub const WINDOW_PAGES: usize = WINDOW_ROWS / PAGE_ROWS;
pub const WIDTH_CHUNKS: usize = ROW_BYTES;
pub const HEIGHT_CHUNKS: usize = PAGE_COUNT * PAGE_ROWS;
pub const WIDTH_PIXELS: usize = WIDTH_CHUNKS * 32;
pub const HEIGHT_PIXELS: usize = HEIGHT_CHUNKS * 32;

/// The two `Map_Layout` regions. A patch reaches the other plane by a negative
/// displacement, so a write resolves as a RAM address.
const MAP_LAYOUT_FG: i64 = 0xA000;
const MAP_LAYOUT_BG: i64 = 0xB000;
const LAYOUT_RAM_SPAN: i64 = 0x1000;
/// `GetChunkAndCollision` type 1, the only one `MapTransTile_MapChange` fires
/// from.
const MAP_CHANGE_COLLISION_TYPE: u8 = 0x1;

/// One `lea default / btst / lea alternate` selector in the load path.
#[derive(Clone, Copy)]
struct CodeSite {
    label: &'static str,
    rom_offset: usize,
    motavia: usize,
    dezolis: usize,
}

impl CodeSite {
    /// `lea (first).l,a2 / btst #0,($FFFFEC29).w / beq.s *+8 / lea (second).l,a2`:
    /// the whole shape proves bit 0 clear takes the first table.
    fn signature(&self) -> Vec<u8> {
        let mut out = vec![0x45, 0xF9];
        out.extend_from_slice(&(self.motavia as u32).to_be_bytes());
        out.extend_from_slice(&[0x08, 0x38, 0x00, 0x00, 0xEC, 0x29, 0x67, 0x06, 0x45, 0xF9]);
        out.extend_from_slice(&(self.dezolis as u32).to_be_bytes());
        out
    }

    fn table_for(&self, map_id: usize) -> usize {
        if map_id & 1 != 0 {
            self.dezolis
        } else {
            self.motavia
        }
    }

    fn to_json(self) -> Json {
        Json::object()
            .with("label", self.label)
            .with("rom_offset", hex6(self.rom_offset))
            .with(
                "selector",
                "btst #0,(Field_Map_Index+1); clear = Motavia, set = Dezolis",
            )
            .with("motavia", hex6(self.motavia))
            .with("dezolis", hex6(self.dezolis))
    }
}

const fn site(label: &'static str, rom_offset: usize, motavia: usize, dezolis: usize) -> CodeSite {
    CodeSite {
        label,
        rom_offset,
        motavia,
        dezolis,
    }
}

const PLANES: [Plane; 2] = [Plane::Fg, Plane::Bg];
const PAGE_TABLE_FG: CodeSite = site("loc_53A26", 0x053A44, 0x107DC2, 0x115584);
const PAGE_TABLE_BG: CodeSite = site("loc_53A8E", 0x053AAC, 0x10BE02, 0x1175C4);
const PATCH_TABLE_FG: CodeSite = site("loc_53BB6", 0x053BC2, 0x053BDC, 0x053BFC);
const PATCH_TABLE_BG: CodeSite = site("loc_53CAE", 0x053CBA, 0x053CD4, 0x053CF4);

fn page_table_site(plane: Plane) -> CodeSite {
    match plane {
        Plane::Fg => PAGE_TABLE_FG,
        Plane::Bg => PAGE_TABLE_BG,
    }
}

fn patch_table_site(plane: Plane) -> CodeSite {
    match plane {
        Plane::Fg => PATCH_TABLE_FG,
        Plane::Bg => PATCH_TABLE_BG,
    }
}

const PAGE_COPY_FG: usize = 0x053BB6;
const PAGE_COPY_BG: usize = 0x053CAE;
/// `move.w #$FF,d2 / move.l (a0)+,(a1)+ / dbf d2 / add.w d1,d1`.
const PAGE_COPY_SIGNATURE: [u8; 10] = [0x34, 0x3C, 0x00, 0xFF, 0x22, 0xD8, 0x51, 0xCA, 0xFF, 0xFC];
const PAGE_COPY_SIGNATURE_TAIL: [u8; 2] = [0xD2, 0x41];
const GET_MAP_LAYOUT_CHUNK_FG: usize = 0x053EE6;
const GET_MAP_LAYOUT_CHUNK_BG: usize = 0x053EEC;
const GET_MAP_LAYOUT_CHUNK_SIGNATURE: [u8; 20] = [
    0x43, 0xF8, 0xA0, 0x00, 0x60, 0x04, 0x43, 0xF8, 0xB0, 0x00, 0x02, 0x41, 0x00, 0x1F, 0xEF, 0x49,
    0xD0, 0x41, 0x43, 0xF1,
];
const GET_MAP_LAYOUT_CHUNK_SIGNATURE_TAIL: [u8; 4] = [0x00, 0x00, 0x4E, 0x75];
const EVENT_FLAGS_TEST: u32 = 0x057624;

fn page_copy_signature() -> Vec<u8> {
    [&PAGE_COPY_SIGNATURE[..], &PAGE_COPY_SIGNATURE_TAIL].concat()
}

fn get_map_layout_chunk_signature() -> Vec<u8> {
    [
        &GET_MAP_LAYOUT_CHUNK_SIGNATURE[..],
        &GET_MAP_LAYOUT_CHUNK_SIGNATURE_TAIL,
    ]
    .concat()
}

/// The loader's own test: `Field_Map_Index & $FFFE == 0`.
pub fn is_overworld(map_id: usize) -> bool {
    map_id & 0xFFFE == 0
}

fn require_overworld(map_id: usize) -> Result<(), PackError> {
    if is_overworld(map_id) {
        return Ok(());
    }
    Err(PackError(format!(
        "map 0x{map_id:03X} reads its layout from its record; only [0, 1] take the paged path"
    )))
}

fn check_bytes(rom: &[u8], offset: usize, expected: &[u8], what: &str) -> Result<(), PackError> {
    let end = (offset + expected.len()).min(rom.len());
    let actual = rom.get(offset..end).unwrap_or(&[]);
    if actual == expected {
        return Ok(());
    }
    Err(PackError(format!(
        "{what} at {}: expected {}, got {}",
        hex6(offset),
        hex(expected),
        hex(actual)
    )))
}

fn upper(plane: Plane) -> String {
    plane.name().to_uppercase()
}

/// `check_code_sites`: prove every hardcoded address against the retail
/// opcodes. The tables are `lea` operands inside the load path, so the
/// instructions naming them are the only binding there is.
pub fn check_code_sites(rom: &[u8]) -> Result<Json, PackError> {
    for plane in PLANES {
        let site = page_table_site(plane);
        let what = format!("{} page table selector", upper(plane));
        check_bytes(rom, site.rom_offset, &site.signature(), &what)?;
    }
    for plane in PLANES {
        let site = patch_table_site(plane);
        let what = format!("{} patch table selector", upper(plane));
        check_bytes(rom, site.rom_offset, &site.signature(), &what)?;
    }
    let copy = page_copy_signature();
    check_bytes(rom, PAGE_COPY_FG, &copy, "FG page copy loop")?;
    check_bytes(rom, PAGE_COPY_BG, &copy, "BG page copy loop")?;
    let chunk = get_map_layout_chunk_signature();
    check_bytes(rom, GET_MAP_LAYOUT_CHUNK_FG, &chunk, "GetMapLayoutChunkFG")?;
    Ok(Json::object()
        .with(
            "page_tables",
            Json::object()
                .with("fg", PAGE_TABLE_FG.to_json())
                .with("bg", PAGE_TABLE_BG.to_json()),
        )
        .with(
            "patch_tables",
            Json::object()
                .with("fg", PATCH_TABLE_FG.to_json())
                .with("bg", PATCH_TABLE_BG.to_json()),
        )
        .with(
            "page_copy",
            Json::object()
                .with(
                    "routines",
                    Json::object()
                        .with("fg", hex6(PAGE_COPY_FG))
                        .with("bg", hex6(PAGE_COPY_BG)),
                )
                .with("signature_hex", hex(&copy))
                .with("compression", Json::Null)
                .with("longs_copied", PAGE_BYTES / 4),
        )
        .with(
            "get_map_layout_chunk",
            Json::object()
                .with("fg", hex6(GET_MAP_LAYOUT_CHUNK_FG))
                .with("bg", hex6(GET_MAP_LAYOUT_CHUNK_BG))
                .with("signature_hex", hex(&chunk))
                .with("window_rows", WINDOW_ROWS)
                .with("row_bytes", ROW_BYTES),
        ))
}

/// One 1,024-byte block of chunk ids, as one table entry names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    pub index: usize,
    pub rom_offset: usize,
    pub sha256: String,
    /// The lowest page index sharing this offset, when the table repeats one.
    pub aliases: Option<usize>,
}

impl Page {
    fn first_row(&self) -> usize {
        self.index * PAGE_ROWS
    }

    fn to_json(&self) -> Json {
        Json::object()
            .with("index", self.index)
            .with("first_row", self.first_row())
            .with("rom_offset", hex6(self.rom_offset))
            .with("rom_end", hex6(self.rom_offset + PAGE_BYTES))
            .with("sha256", self.sha256.as_str())
            .with("aliases_page", self.aliases)
    }
}

/// The sixteen longs one plane of one overworld streams its rows from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageTable {
    pub plane: Plane,
    pub rom_offset: usize,
    pub pages: Vec<Page>,
}

impl PageTable {
    fn distinct_pages(&self) -> usize {
        self.pages.iter().filter(|p| p.aliases.is_none()).count()
    }

    fn data_start(&self) -> usize {
        self.pages.iter().map(|p| p.rom_offset).min().unwrap_or(0)
    }

    fn data_end(&self) -> usize {
        self.pages.iter().map(|p| p.rom_offset).max().unwrap_or(0) + PAGE_BYTES
    }

    fn to_json(&self) -> Json {
        Json::object()
            .with("plane", self.plane.name())
            .with("rom_offset", hex6(self.rom_offset))
            .with("entry_size", 4)
            .with("page_count", self.pages.len())
            .with("distinct_pages", self.distinct_pages())
            .with("page_bytes", PAGE_BYTES)
            .with("page_rows", PAGE_ROWS)
            .with("compression", Json::Null)
            .with("data_rom_offset", hex6(self.data_start()))
            .with("data_rom_end", hex6(self.data_end()))
            .with(
                "pages",
                self.pages.iter().map(Page::to_json).collect::<Vec<_>>(),
            )
    }
}

fn long_at(rom: &[u8], offset: usize) -> usize {
    u32::from_be_bytes([
        rom[offset],
        rom[offset + 1],
        rom[offset + 2],
        rom[offset + 3],
    ]) as usize
}

/// `read_page_table`: a table's distinct pages are contiguous and ascending in
/// the cartridge, or the table is being read in the wrong place.
pub fn read_page_table(rom: &[u8], map_id: usize, plane: Plane) -> Result<PageTable, PackError> {
    require_overworld(map_id)?;
    let site = page_table_site(plane);
    let what = format!("{} page table selector", upper(plane));
    check_bytes(rom, site.rom_offset, &site.signature(), &what)?;
    let offset = site.table_for(map_id);
    if offset + PAGE_COUNT * 4 > rom.len() {
        return Err(PackError(format!(
            "Page table at {} runs past the end of the ROM",
            hex6(offset)
        )));
    }
    let mut pages: Vec<Page> = Vec::with_capacity(PAGE_COUNT);
    for index in 0..PAGE_COUNT {
        let pointer = long_at(rom, offset + index * 4);
        if pointer + PAGE_BYTES > rom.len() {
            return Err(PackError(format!(
                "Page {index} of the {} table at {} points at {}, which does not hold \
                 {PAGE_BYTES} bytes",
                upper(plane),
                hex6(offset),
                hex6(pointer)
            )));
        }
        let aliases = pages
            .iter()
            .find(|page| page.rom_offset == pointer)
            .map(|page| page.index);
        pages.push(Page {
            index,
            rom_offset: pointer,
            sha256: hex(&Sha256::digest(&rom[pointer..pointer + PAGE_BYTES])),
            aliases,
        });
    }
    let distinct: Vec<usize> = pages
        .iter()
        .filter(|page| page.aliases.is_none())
        .map(|page| page.rom_offset)
        .collect();
    let ascending = distinct
        .iter()
        .enumerate()
        .all(|(i, &at)| at == distinct[0] + i * PAGE_BYTES);
    if !ascending {
        return Err(PackError(format!(
            "The {} distinct pages of the {} table at {} are not contiguous and ascending; the \
             table is being read in the wrong place",
            distinct.len(),
            upper(plane),
            hex6(offset)
        )));
    }
    Ok(PageTable {
        plane,
        rom_offset: offset,
        pages,
    })
}

/// `decode_plane`: one plane's 128x128 chunk grid. The whole region is one
/// `Blob`, with equal lengths, because the copy is raw.
pub fn decode_plane(rom: &[u8], table: &PageTable) -> Result<Layout, PackError> {
    let mut cells = Vec::with_capacity(table.pages.len() * PAGE_BYTES);
    for page in &table.pages {
        cells.extend_from_slice(&rom[page.rom_offset..page.rom_offset + PAGE_BYTES]);
    }
    if cells.len() != WIDTH_CHUNKS * HEIGHT_CHUNKS {
        return Err(PackError(format!(
            "{} pages of {PAGE_BYTES} bytes make {} cells, not the {} a \
             {WIDTH_CHUNKS}x{HEIGHT_CHUNKS} grid needs",
            table.pages.len(),
            cells.len(),
            WIDTH_CHUNKS * HEIGHT_CHUNKS
        )));
    }
    let aliased: Vec<&Page> = table
        .pages
        .iter()
        .filter(|page| page.aliases.is_some())
        .collect();
    let anomaly = aliased.first().and_then(|first| {
        let target = first.aliases?;
        let listed: Vec<Json> = aliased
            .iter()
            .map(|page| {
                Json::object()
                    .with("index", page.index)
                    .with("aliases_page", page.aliases)
            })
            .collect();
        Some(
            Json::object()
                .with("plane", table.plane.name())
                .with("kind", "aliased_pages")
                .with("table_offset", hex6(table.rom_offset))
                .with("page_count", table.pages.len())
                .with("distinct_pages", table.distinct_pages())
                .with("aliased_pages", listed)
                .with(
                    "effect",
                    format!(
                        "chunk rows {}..{} repeat rows {}..{}; the table stores the same page \
                         pointer for those indexes",
                        first.first_row(),
                        HEIGHT_CHUNKS - 1,
                        target * PAGE_ROWS,
                        target * PAGE_ROWS + PAGE_ROWS - 1
                    ),
                ),
        )
    });
    let (start, end) = (table.data_start(), table.data_end());
    Ok(Layout {
        plane: table.plane,
        width_chunks: WIDTH_CHUNKS,
        height_chunks: HEIGHT_CHUNKS,
        cells,
        blob: Blob {
            rom_offset: start,
            compressed_length: end - start,
            decompressed_length: end - start,
            sha256: hex(&Sha256::digest(&rom[start..end])),
        },
        anomaly,
    })
}

/// One store into `Map_Layout`, resolved to plane and chunk coordinates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatchWrite {
    pub plane: Plane,
    pub chunk_x: usize,
    pub chunk_y: usize,
    pub chunk_ids: Vec<u8>,
    pub displacement: i16,
}

impl PatchWrite {
    pub fn to_json(&self) -> Json {
        Json::object()
            .with("plane", self.plane.name())
            .with("chunk_x", self.chunk_x)
            .with("chunk_y", self.chunk_y)
            .with("cell_x", self.chunk_x * COLLISION_CELL_TILES)
            .with("cell_y", self.chunk_y * COLLISION_CELL_TILES)
            .with(
                "chunk_ids",
                self.chunk_ids
                    .iter()
                    .map(|value| format!("0x{value:02X}"))
                    .collect::<Vec<_>>(),
            )
            .with("displacement", self.displacement)
    }
}

/// One event-flag-gated block of writes inside one page's hook routine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayoutPatch {
    pub plane: Plane,
    pub page: usize,
    pub routine: usize,
    pub event_flag: u16,
    pub writes: Vec<PatchWrite>,
}

impl LayoutPatch {
    pub fn event_symbol(&self) -> Option<&'static str> {
        event_flag_symbol(self.event_flag)
    }

    pub fn to_json(&self) -> Json {
        Json::object()
            .with("triggered_by_plane", self.plane.name())
            .with("page", self.page)
            .with("routine", hex6(self.routine))
            .with(
                "event_flag",
                Json::object()
                    .with("id", self.event_flag)
                    .with("id_hex", format!("0x{:02X}", self.event_flag))
                    .with("symbol", self.event_symbol()),
            )
            .with(
                "writes",
                self.writes
                    .iter()
                    .map(PatchWrite::to_json)
                    .collect::<Vec<_>>(),
            )
    }
}

const RTS: u16 = 0x4E75;
const MOVE_W_IMM_D0: u16 = 0x303C;
const MOVE_W_IMM_D1: u16 = 0x323C;
const JSR_ABS_L: u16 = 0x4EB9;
const BSR_W: u16 = 0x6100;
const MOVEQ_0_D0: u16 = 0x7000;
const BEQ_SHORT: u16 = 0x67;

/// A store into `(a1)`: its size, whether its source is an immediate rather
/// than d0, and whether it carries a displacement. A byte immediate is still
/// encoded as a word, high half ignored.
fn store_form(opcode: u16) -> Option<(usize, bool, bool)> {
    Some(match opcode {
        0x22BC => (4, true, false),  // move.l #imm,(a1)
        0x237C => (4, true, true),   // move.l #imm,d(a1)
        0x2280 => (4, false, false), // move.l d0,(a1)
        0x2340 => (4, false, true),  // move.l d0,d(a1)
        0x12BC => (1, true, false),  // move.b #imm,(a1)
        0x137C => (1, true, true),   // move.b #imm,d(a1)
        0x1280 => (1, false, false), // move.b d0,(a1)
        0x1340 => (1, false, true),  // move.b d0,d(a1)
        _ => return None,
    })
}

/// A reader for the instruction forms the hook routines are made of. Not a
/// 68000 disassembler: it refuses everything they don't use.
struct PatchDecoder<'rom> {
    rom: &'rom [u8],
    plane: Plane,
    page: usize,
    routine: usize,
    pos: usize,
}

impl PatchDecoder<'_> {
    fn fail(&self, message: String) -> PackError {
        PackError(format!(
            "{} page {} hook at {}, offset {}: {message}",
            upper(self.plane),
            self.page,
            hex6(self.routine),
            hex6(self.pos)
        ))
    }

    /// `length` big-endian bytes; bytes past the end of the ROM aren't there,
    /// and the opcode tests refuse what they leave.
    fn read(&mut self, length: usize) -> u32 {
        let value = self
            .rom
            .iter()
            .skip(self.pos)
            .take(length)
            .fold(0u32, |value, &byte| (value << 8) | u32::from(byte));
        self.pos += length;
        value
    }

    fn word(&mut self) -> u16 {
        self.read(2) as u16
    }

    fn signed_word(&mut self) -> i16 {
        self.word() as i16
    }

    fn decode(&mut self) -> Result<Vec<LayoutPatch>, PackError> {
        let mut patches = Vec::new();
        loop {
            let opcode = self.word();
            if opcode == RTS {
                return Ok(patches);
            }
            if opcode != MOVE_W_IMM_D0 {
                return Err(self.fail(format!(
                    "opcode 0x{opcode:04X} is not `move.w #imm,d0` or `rts`"
                )));
            }
            let event_flag = self.word();
            if self.word() != JSR_ABS_L || self.read(4) != EVENT_FLAGS_TEST {
                return Err(
                    self.fail("the flag load is not followed by `jsr (EventFlags_Test).l`".into())
                );
            }
            let branch = self.word();
            if branch >> 8 != BEQ_SHORT || branch & 0xFF == 0 {
                return Err(self.fail(format!(
                    "expected a short `beq` after the flag test, got 0x{branch:04X}"
                )));
            }
            let end = self.pos + usize::from(branch & 0xFF);
            let writes = self.block(end)?;
            patches.push(LayoutPatch {
                plane: self.plane,
                page: self.page,
                routine: self.routine,
                event_flag,
                writes,
            });
        }
    }

    /// One flag's writes, up to where its `beq` lands.
    fn block(&mut self, end: usize) -> Result<Vec<PatchWrite>, PackError> {
        let (mut column, mut row) = (None, None);
        let mut base = None;
        let mut d0_is_zero = false;
        let mut writes = Vec::new();
        while self.pos < end {
            let opcode = self.word();
            match opcode {
                MOVE_W_IMM_D0 => column = Some(self.word()),
                MOVE_W_IMM_D1 => row = Some(self.word()),
                BSR_W => {
                    let from = self.pos as i64;
                    let target = (from + i64::from(self.signed_word())) as usize;
                    base = Some(match target {
                        GET_MAP_LAYOUT_CHUNK_FG => Plane::Fg,
                        GET_MAP_LAYOUT_CHUNK_BG => Plane::Bg,
                        _ => {
                            return Err(self.fail(format!(
                                "`bsr.w {}` is not GetMapLayoutChunkFG/BG",
                                hex6(target)
                            )));
                        }
                    });
                    if column.is_none() || row.is_none() {
                        return Err(self.fail(
                            "GetMapLayoutChunk is reached without both coordinates set".into(),
                        ));
                    }
                    d0_is_zero = false;
                }
                MOVEQ_0_D0 => d0_is_zero = true,
                _ => {
                    let Some(form) = store_form(opcode) else {
                        return Err(self.fail(format!(
                            "opcode 0x{opcode:04X} is outside this decoder's vocabulary"
                        )));
                    };
                    let (Some(base), Some(column), Some(row)) = (base, column, row) else {
                        return Err(
                            self.fail("a store runs before GetMapLayoutChunk has set a1".into())
                        );
                    };
                    writes.push(self.store(form, base, column, row, d0_is_zero)?);
                }
            }
        }
        if self.pos != end {
            return Err(self.fail(format!("the block overruns its `beq` target {}", hex6(end))));
        }
        Ok(writes)
    }

    fn store(
        &mut self,
        (size, immediate, has_displacement): (usize, bool, bool),
        base: Plane,
        column: u16,
        row: u16,
        d0_is_zero: bool,
    ) -> Result<PatchWrite, PackError> {
        let value = if immediate {
            if size == 4 {
                self.read(4)
            } else {
                self.read(2) & 0xFF
            }
        } else {
            if !d0_is_zero {
                return Err(self.fail("a store reads d0 without a preceding `moveq #0,d0`".into()));
            }
            0
        };
        let displacement = if has_displacement {
            self.signed_word()
        } else {
            0
        };
        let bytes = value.to_be_bytes();
        self.resolve(base, column, row, displacement, bytes[4 - size..].to_vec())
    }

    /// `_resolve`: turn `d(a1)` into a plane and a chunk coordinate. A
    /// displacement of $F000 reaches the same cell of the other plane, so the
    /// only reading that works is the RAM address.
    fn resolve(
        &self,
        base: Plane,
        column: u16,
        row: u16,
        displacement: i16,
        chunk_ids: Vec<u8>,
    ) -> Result<PatchWrite, PackError> {
        let window_rows = WINDOW_ROWS as i64;
        let row_bytes = ROW_BYTES as i64;
        let window_row = i64::from(row) & (window_rows - 1);
        let origin = match base {
            Plane::Fg => MAP_LAYOUT_FG,
            Plane::Bg => MAP_LAYOUT_BG,
        };
        let address = origin + window_row * row_bytes + i64::from(column) + i64::from(displacement);
        let plane = if (MAP_LAYOUT_FG..MAP_LAYOUT_FG + LAYOUT_RAM_SPAN).contains(&address) {
            Plane::Fg
        } else if (MAP_LAYOUT_BG..MAP_LAYOUT_BG + LAYOUT_RAM_SPAN).contains(&address) {
            Plane::Bg
        } else {
            return Err(self.fail(format!(
                "displacement {displacement} puts the store at 0x{address:04X}, outside both \
                 Map_Layout regions"
            )));
        };
        let offset = address & (LAYOUT_RAM_SPAN - 1);
        // The window is modular, so take the row move nearest zero.
        let delta = (offset / row_bytes - window_row + window_rows / 2).rem_euclid(window_rows)
            - window_rows / 2;
        let chunk_x = offset.rem_euclid(row_bytes);
        let chunk_y = i64::from(row) + delta;
        if !(0..WIDTH_CHUNKS as i64).contains(&chunk_x)
            || !(0..HEIGHT_CHUNKS as i64).contains(&chunk_y)
        {
            return Err(self.fail(format!(
                "the store resolves to chunk ({chunk_x}, {chunk_y}), off the map"
            )));
        }
        let length = chunk_ids.len();
        if chunk_x as usize + length > WIDTH_CHUNKS {
            return Err(self.fail(format!(
                "a {length}-byte store at column {chunk_x} runs past the row"
            )));
        }
        Ok(PatchWrite {
            plane,
            chunk_x: chunk_x as usize,
            chunk_y: chunk_y as usize,
            chunk_ids,
            displacement,
        })
    }
}

/// `read_patch_table`: entries are `dc.w routine - table`.
fn read_patch_table(rom: &[u8], map_id: usize, plane: Plane) -> Result<Vec<usize>, PackError> {
    require_overworld(map_id)?;
    let site = patch_table_site(plane);
    let what = format!("{} patch table selector", upper(plane));
    check_bytes(rom, site.rom_offset, &site.signature(), &what)?;
    let base = site.table_for(map_id);
    Ok((0..PAGE_COUNT)
        .map(|page| {
            let at = base + page * 2;
            let offset = i16::from_be_bytes([rom[at], rom[at + 1]]);
            (base as i64 + i64::from(offset)) as usize
        })
        .collect())
}

/// `decode_patches`: every event-gated layout write both planes' page hooks
/// perform, at absolute chunk coordinates.
pub fn decode_patches(rom: &[u8], map_id: usize) -> Result<Vec<LayoutPatch>, PackError> {
    require_overworld(map_id)?;
    check_bytes(
        rom,
        GET_MAP_LAYOUT_CHUNK_FG,
        &get_map_layout_chunk_signature(),
        "GetMapLayoutChunkFG",
    )?;
    let mut patches = Vec::new();
    for plane in PLANES {
        for (page, routine) in read_patch_table(rom, map_id, plane)?
            .into_iter()
            .enumerate()
        {
            let mut decoder = PatchDecoder {
                rom,
                plane,
                page,
                routine,
                pos: routine,
            };
            patches.extend(decoder.decode()?);
        }
    }
    Ok(patches)
}

/// The decoded layout section MapID 0 and 1 do not carry in their records.
#[derive(Clone, Debug, PartialEq)]
pub struct Overworld {
    pub map_id: usize,
    pub symbol: &'static str,
    pub tables: [PageTable; 2],
    pub layout: MapLayout,
    pub patches: Vec<LayoutPatch>,
    pub anomalies: Vec<Json>,
}

impl Overworld {
    /// The format facts and provenance, without the patch list.
    pub fn to_json(&self) -> Json {
        let mut tables = Json::object();
        for table in &self.tables {
            tables.insert(table.plane.name(), table.to_json());
        }
        let events: BTreeSet<String> = self
            .patches
            .iter()
            .map(|patch| match patch.event_symbol() {
                Some(symbol) => symbol.to_owned(),
                None => format!("0x{:02X}", patch.event_flag),
            })
            .collect();
        Json::object()
            .with("map_id", self.map_id)
            .with("symbol", self.symbol)
            .with(
                "source",
                "paged tables; the record carries no layout pointers",
            )
            .with("width_chunks", WIDTH_CHUNKS)
            .with("height_chunks", HEIGHT_CHUNKS)
            .with("width_pixels", WIDTH_PIXELS)
            .with("height_pixels", HEIGHT_PIXELS)
            .with("page_bytes", PAGE_BYTES)
            .with("page_rows", PAGE_ROWS)
            .with("page_count", PAGE_COUNT)
            .with("window_rows", WINDOW_ROWS)
            .with("window_pages", WINDOW_PAGES)
            .with("row_bytes", ROW_BYTES)
            .with("compression", Json::Null)
            .with(
                "wraps",
                Json::object()
                    .with("x_pixels", WIDTH_PIXELS)
                    .with("y_pixels", HEIGHT_PIXELS),
            )
            .with("tables", tables)
            .with("layout_fg", self.layout.fg.to_json())
            .with("layout_bg", self.layout.bg.to_json())
            .with("collision", self.layout.collision.to_json())
            .with("patch_count", self.patches.len())
            .with("patched_events", events.into_iter().collect::<Vec<_>>())
            .with("anomalies", self.anomalies.clone())
    }
}

/// `overworld_spec`: every field comes from the record as it does for an
/// interior map, except that `layout_fg` and `layout_bg` hold the page table
/// addresses.
pub fn overworld_spec(record: &MapRecord) -> Result<MapLayoutSpec, PackError> {
    require_overworld(record.id)?;
    if record.layout.planes.is_some() {
        return Err(PackError(format!(
            "map 0x{:03X} ({}) carries a layout section; it is not one of the paged world maps",
            record.id, record.symbol
        )));
    }
    Ok(spec_with_planes(
        record,
        PAGE_TABLE_FG.table_for(record.id),
        PAGE_TABLE_BG.table_for(record.id),
    ))
}

/// `decode_overworld`: one overworld's layout, collision and event patches.
pub fn decode_overworld(
    rom: &[u8],
    decodes: &Decodes,
    record: &MapRecord,
) -> Result<Overworld, PackError> {
    let map_id = record.id;
    require_overworld(map_id)?;
    check_code_sites(rom)?;
    let spec = overworld_spec(record)?;
    let tables = [
        read_page_table(rom, map_id, Plane::Fg)?,
        read_page_table(rom, map_id, Plane::Bg)?,
    ];
    let fg = decode_plane(rom, &tables[0])?;
    let bg = decode_plane(rom, &tables[1])?;
    for (name, width, height) in [
        ("FG", spec.width_chunks_fg, spec.height_chunks_fg),
        ("BG", spec.width_chunks_bg, spec.height_chunks_bg),
    ] {
        if (width, height) != (WIDTH_CHUNKS, HEIGHT_CHUNKS) {
            return Err(PackError(format!(
                "map 0x{map_id:03X} declares a {width}x{height} {name} grid, but the paged path \
                 always builds {WIDTH_CHUNKS}x{HEIGHT_CHUNKS}"
            )));
        }
    }
    let chunks = decode_chunks(rom, decodes, &spec.chunk_blobs)?;
    for layout in [&fg, &bg] {
        let highest = layout.cells.iter().copied().max().unwrap_or(0);
        if usize::from(highest) >= chunks.words.len() {
            return Err(PackError(format!(
                "{} layout names chunk 0x{highest:02X} but the map loads only {} chunks",
                upper(layout.plane),
                chunks.words.len()
            )));
        }
    }
    let collision = decode_collision(
        &chunks,
        match spec.collision_plane() {
            Plane::Bg => &bg,
            Plane::Fg => &fg,
        },
    )?;
    let patterns = decode_tilesets(rom, decodes, &spec.tilesets)?;
    let anomalies = [fg.anomaly.clone(), bg.anomaly.clone()]
        .into_iter()
        .flatten()
        .collect();
    Ok(Overworld {
        map_id,
        symbol: record.symbol,
        tables,
        layout: MapLayout {
            spec,
            chunks,
            fg,
            bg,
            collision,
            patterns,
        },
        patches: decode_patches(rom, map_id)?,
        anomalies,
    })
}

/// `apply_patches`: one plane with the writes of `patches` applied, in routine
/// order; a later patch wins.
pub fn apply_patches(layout: &Layout, patches: &[&LayoutPatch]) -> Layout {
    let mut cells = BTreeMap::new();
    for write in patches
        .iter()
        .flat_map(|patch| &patch.writes)
        .filter(|write| write.plane == layout.plane)
    {
        for (step, &chunk_id) in write.chunk_ids.iter().enumerate() {
            cells.insert((write.chunk_x + step, write.chunk_y), chunk_id);
        }
    }
    let mut patched = layout.clone();
    for ((chunk_x, chunk_y), chunk_id) in cells {
        patched.cells[chunk_y * WIDTH_CHUNKS + chunk_x] = chunk_id;
    }
    patched
}

/// `opening_patches`: the patches that turn one of `cells` into a map-change
/// cell, which is how the pack proves which event opens a door.
pub fn opening_patches<'a>(
    overworld: &'a Overworld,
    cells: &[(usize, usize)],
) -> Result<Vec<&'a LayoutPatch>, PackError> {
    let plane = overworld.layout.collision_layout();
    let chunks = &overworld.layout.chunks;
    let before = cells
        .iter()
        .map(|&(x, y)| collision_at(chunks, plane, x, y))
        .collect::<Result<Vec<_>, _>>()?;
    let mut found = Vec::new();
    for patch in &overworld.patches {
        if !patch.writes.iter().any(|write| write.plane == plane.plane) {
            continue;
        }
        let after = apply_patches(plane, &[patch]);
        for (&(x, y), &was) in cells.iter().zip(&before) {
            if was != MAP_CHANGE_COLLISION_TYPE
                && collision_at(chunks, &after, x, y)? == MAP_CHANGE_COLLISION_TYPE
            {
                found.push(patch);
                break;
            }
        }
    }
    Ok(found)
}

/// One map's decoded layout, by whichever of the two paths its id selects.
#[derive(Clone, Debug, PartialEq)]
pub enum MapSection {
    /// Two Kosinski blobs the record points at, and their length anomalies.
    Stored(Box<MapLayout>, Vec<Json>),
    Paged(Box<Overworld>),
}

impl MapSection {
    pub fn layout(&self) -> &MapLayout {
        match self {
            MapSection::Stored(layout, _) => layout,
            MapSection::Paged(overworld) => &overworld.layout,
        }
    }

    pub fn anomalies(&self) -> &[Json] {
        match self {
            MapSection::Stored(_, anomalies) => anomalies,
            MapSection::Paged(overworld) => &overworld.anomalies,
        }
    }

    pub fn overworld(&self) -> Option<&Overworld> {
        match self {
            MapSection::Stored(..) => None,
            MapSection::Paged(overworld) => Some(overworld),
        }
    }
}

/// `decode_map_section`: everything downstream of here sees a [`MapLayout`]
/// and cannot tell which path produced it.
pub fn decode_map_section(
    rom: &[u8],
    decodes: &Decodes,
    record: &MapRecord,
) -> Result<MapSection, PackError> {
    if record.layout.planes.is_some() {
        let (layout, anomalies) = decode_layout_section(rom, decodes, layout_spec(record)?)?;
        return Ok(MapSection::Stored(Box::new(layout), anomalies));
    }
    if !is_overworld(record.id) {
        return Err(PackError(format!(
            "map 0x{:03X} ({}) has no layout section and is not one of the paged world maps",
            record.id, record.symbol
        )));
    }
    Ok(MapSection::Paged(Box::new(decode_overworld(
        rom, decodes, record,
    )?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decoder(routine: &[u8], plane: Plane) -> (Vec<u8>, usize, Plane) {
        // The hook sits above GetMapLayoutChunk, so `bsr.w` displacements are
        // real ones.
        let at = GET_MAP_LAYOUT_CHUNK_BG + 0x100;
        let mut rom = vec![0; at];
        rom.extend_from_slice(routine);
        (rom, at, plane)
    }

    fn decode(routine: &[u8], plane: Plane) -> Result<Vec<LayoutPatch>, PackError> {
        let (rom, at, plane) = decoder(routine, plane);
        PatchDecoder {
            rom: &rom,
            plane,
            page: 3,
            routine: at,
            pos: at,
        }
        .decode()
    }

    /// `move.w #flag,d0 / jsr EventFlags_Test / beq.s end`, then `body`, `rts`.
    fn gated(flag: u16, body: &[u8]) -> Vec<u8> {
        let mut out = vec![0x30, 0x3C];
        out.extend_from_slice(&flag.to_be_bytes());
        out.extend_from_slice(&[0x4E, 0xB9]);
        out.extend_from_slice(&EVENT_FLAGS_TEST.to_be_bytes());
        out.extend_from_slice(&[0x67, body.len() as u8]);
        out.extend_from_slice(body);
        out.extend_from_slice(&RTS.to_be_bytes());
        out
    }

    /// `move.w #column,d0 / move.w #row,d1 / bsr.w GetMapLayoutChunk`, for a
    /// routine starting at the test's address and `prefix` bytes in.
    fn locate(column: u16, row: u16, target: usize, prefix: usize) -> Vec<u8> {
        let mut out = vec![0x30, 0x3C];
        out.extend_from_slice(&column.to_be_bytes());
        out.extend_from_slice(&[0x32, 0x3C]);
        out.extend_from_slice(&row.to_be_bytes());
        out.extend_from_slice(&[0x61, 0x00]);
        let from = GET_MAP_LAYOUT_CHUNK_BG + 0x100 + prefix + out.len();
        let displacement = target as i64 - from as i64;
        out.extend_from_slice(&(displacement as i16).to_be_bytes());
        out
    }

    // The gate's own 12 bytes: move.w, flag, jsr, address, beq.
    const GATE: usize = 12;

    #[test]
    fn a_hook_with_nothing_gated_is_no_patches() {
        assert_eq!(decode(&RTS.to_be_bytes(), Plane::Fg).unwrap(), []);
    }

    #[test]
    fn a_store_resolves_to_its_plane_and_row() {
        // Row 40 is window row 8 at column 5. move.l #imm,(a1), then a byte
        // store on the other plane: $F000 from BG's $B000 reaches FG's $A000.
        let mut body = locate(5, 40, GET_MAP_LAYOUT_CHUNK_BG, GATE);
        body.extend_from_slice(&[0x22, 0xBC, 1, 2, 3, 4]);
        body.extend_from_slice(&[0x13, 0x7C, 0xEE, 0x09]);
        body.extend_from_slice(&0xF000u16.to_be_bytes());
        let patches = decode(&gated(0x66, &body), Plane::Bg).unwrap();
        let [patch] = patches.as_slice() else {
            panic!("{patches:?}");
        };
        assert_eq!(patch.event_flag, 0x66);
        assert_eq!(patch.page, 3);
        assert_eq!(
            patch.writes,
            [
                PatchWrite {
                    plane: Plane::Bg,
                    chunk_x: 5,
                    chunk_y: 40,
                    chunk_ids: vec![1, 2, 3, 4],
                    displacement: 0,
                },
                PatchWrite {
                    plane: Plane::Fg,
                    chunk_x: 5,
                    chunk_y: 40,
                    chunk_ids: vec![9],
                    displacement: -0x1000,
                },
            ]
        );
    }

    #[test]
    fn a_row_move_takes_the_nearest_representative() {
        // Row 32 is window row 0; $F80 lands on window row 31 of the same
        // plane, which is one row back: the real row is 31, not 63.
        let mut body = locate(0, 32, GET_MAP_LAYOUT_CHUNK_FG, GATE);
        body.extend_from_slice(&[0x13, 0x7C, 0x00, 0x07]);
        body.extend_from_slice(&0x0F80u16.to_be_bytes());
        let patches = decode(&gated(0x34, &body), Plane::Fg).unwrap();
        let write = &patches[0].writes[0];
        assert_eq!(write.plane, Plane::Fg);
        assert_eq!((write.chunk_x, write.chunk_y), (0, 31));
        // $1000 from window row 1 lands on the same row of the other plane.
        let mut body = locate(0, 33, GET_MAP_LAYOUT_CHUNK_FG, GATE);
        body.extend_from_slice(&[0x13, 0x7C, 0x00, 0x07]);
        body.extend_from_slice(&0x1000u16.to_be_bytes());
        let patches = decode(&gated(0x34, &body), Plane::Fg).unwrap();
        let write = &patches[0].writes[0];
        assert_eq!(write.plane, Plane::Bg);
        assert_eq!((write.chunk_x, write.chunk_y), (0, 33));
    }

    #[test]
    fn the_decoder_refuses_what_it_has_not_read() {
        let refused = |routine: &[u8], wanted: &str| {
            let error = decode(routine, Plane::Fg).unwrap_err();
            assert!(error.0.contains(wanted), "{}", error.0);
            assert!(error.0.starts_with("FG page 3 hook at 0x"), "{}", error.0);
        };
        refused(&[0x4E, 0x71], "is not `move.w #imm,d0` or `rts`");
        let mut wrong_jsr = gated(0x66, &[]);
        wrong_jsr[9] ^= 1;
        refused(&wrong_jsr, "EventFlags_Test");
        let mut no_branch = gated(0x66, &[0x4E, 0x71]);
        no_branch[11] = 0;
        refused(&no_branch, "expected a short `beq`");
        refused(
            &gated(0x66, &[0x4E, 0x71]),
            "outside this decoder's vocabulary",
        );
        refused(
            &gated(0x66, &[0x22, 0xBC, 0, 0, 0, 0]),
            "before GetMapLayoutChunk",
        );
        // A store reading d0 needs the moveq, and the bsr forgets it.
        let mut stale = vec![0x70, 0x00];
        stale.extend(locate(0, 0, GET_MAP_LAYOUT_CHUNK_FG, GATE + 2));
        stale.extend_from_slice(&[0x12, 0x80]);
        refused(&gated(0x66, &stale), "without a preceding `moveq #0,d0`");
        // A bsr somewhere else.
        refused(
            &gated(0x66, &locate(0, 0, GET_MAP_LAYOUT_CHUNK_FG + 2, GATE)),
            "is not GetMapLayoutChunkFG/BG",
        );
        // A bsr before the coordinates.
        let from = GET_MAP_LAYOUT_CHUNK_BG + 0x100 + GATE + 2;
        let displacement = GET_MAP_LAYOUT_CHUNK_FG as i64 - from as i64;
        let mut early = vec![0x61, 0x00];
        early.extend_from_slice(&(displacement as i16).to_be_bytes());
        refused(&gated(0x66, &early), "without both coordinates");
        // A long store that runs past its row.
        let mut past = locate(126, 0, GET_MAP_LAYOUT_CHUNK_FG, GATE);
        past.extend_from_slice(&[0x22, 0xBC, 0, 0, 0, 0]);
        refused(&gated(0x66, &past), "runs past the row");
        // A displacement off both regions.
        let mut off = locate(0, 0, GET_MAP_LAYOUT_CHUNK_FG, GATE);
        off.extend_from_slice(&[0x13, 0x7C, 0, 1, 0x20, 0x00]);
        refused(&gated(0x66, &off), "outside both Map_Layout regions");
        // A block that overruns its beq: the store's immediate crosses it.
        let mut over = locate(0, 0, GET_MAP_LAYOUT_CHUNK_FG, GATE);
        over.extend_from_slice(&[0x12, 0xBC]);
        refused(&gated(0x66, &over), "overruns its `beq` target");
    }

    #[test]
    fn only_the_two_world_maps_take_the_paged_path() {
        assert!(is_overworld(0) && is_overworld(1));
        assert!(!is_overworld(2));
        let error = read_page_table(&[], 2, Plane::Fg).unwrap_err();
        assert!(error.0.contains("only [0, 1]"), "{}", error.0);
    }

    #[test]
    fn a_rom_without_the_opcodes_is_refused_by_name() {
        let error = check_code_sites(&vec![0; 0x60000]).unwrap_err();
        assert!(
            error
                .0
                .starts_with("FG page table selector at 0x053A44: expected 45f9"),
            "{}",
            error.0
        );
    }

    #[test]
    fn a_later_patch_wins_and_other_planes_are_left_alone() {
        let write = |plane, chunk_id| PatchWrite {
            plane,
            chunk_x: 1,
            chunk_y: 0,
            chunk_ids: vec![chunk_id, 9],
            displacement: 0,
        };
        let patch = |writes| LayoutPatch {
            plane: Plane::Fg,
            page: 0,
            routine: 0,
            event_flag: 0,
            writes,
        };
        let layout = Layout {
            plane: Plane::Fg,
            width_chunks: WIDTH_CHUNKS,
            height_chunks: HEIGHT_CHUNKS,
            cells: vec![0; WIDTH_CHUNKS * HEIGHT_CHUNKS],
            blob: Blob {
                rom_offset: 0,
                compressed_length: 0,
                decompressed_length: 0,
                sha256: String::new(),
            },
            anomaly: None,
        };
        let first = patch(vec![write(Plane::Fg, 5)]);
        let second = patch(vec![write(Plane::Fg, 6), write(Plane::Bg, 7)]);
        let patched = apply_patches(&layout, &[&first, &second]);
        assert_eq!(&patched.cells[..4], &[0, 6, 9, 0]);
    }
}
