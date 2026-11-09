//! The dialogue font, the menu font, the waiting arrow and the portraits:
//! the port of `psiv_tools/dialogue_pack/art.py` and the `menu_font.png`
//! step of `emit.py`. All of them render against `Pal_Init_Line_3`.

use std::collections::BTreeMap;
use std::path::Path;

use super::{DIALOGUE_CRAM_LINE, FORMAT_VERSION, PAL_INIT_LINE_3, colours, refuse, sha256, slice};
use crate::json::Json;
use crate::text::dialogue_charset;
use crate::{PackError, gfx, png};

/// `Art_DialogueFont`: 80 1bpp glyphs of 8x16.
const FONT: usize = 0x2A3542;
const FONT_SIZE: usize = 1280;
const GLYPH_WIDTH: usize = 8;
const GLYPH_HEIGHT: usize = 16;
const GLYPH_BYTES: usize = 16;
const GLYPH_COUNT: usize = FONT_SIZE / GLYPH_BYTES;
const GLYPH_INDEX_MASK: u8 = 0x7F;
/// `ParseText`: set bits are `$F`, clear bits `$E`.
const TEXT_COLOR: u8 = 0xF;
const BACKGROUND_COLOR: u8 = 0xE;
const TILE_PIXELS: usize = 8;

/// `ArtNem_Font`, the menu font, which also holds the waiting arrow.
const MENU_FONT: usize = 0x2A303A;
const MENU_FONT_SIZE: usize = 790;
const MENU_FONT_LABEL: &str = "ArtNem_Font";
/// `FieldObj_ScrollTextArrow`: patterns `$7F6`-`$7F7` of the font at `$7C0`,
/// drawn by a 2x1 sprite mapping pinned by length and digest.
const ARROW_FONT_VRAM_TILE: usize = 0x7C0;
const ARROW_VRAM_TILE: usize = 0x7F6;
const ARROW_TILE: usize = ARROW_VRAM_TILE - ARROW_FONT_VRAM_TILE;
const ARROW_TILES: usize = 2;
const ARROW_MAPPING: usize = 0x046532;
const ARROW_MAPPING_PIN: (usize, &str) = (
    8,
    "fed580ee77ab17bbda8f583276a1339348f384f846589ba191b692ddeb8f564f",
);

/// `DialoguePortraitArtPtrs`: 0x28 entries, index 0 a null pointer.
const PORTRAIT_TABLE: usize = 0x06A4B0;
const PORTRAIT_ENTRIES: usize = 0x28;
const PORTRAIT_TILES: usize = 36;
const PORTRAIT_COLUMNS: usize = 6;
const PORTRAIT_VRAM_TILE: usize = 0x55C;
const PORTRAIT_PLANE_MAP: usize = 0x2A2B36;
/// `TextCtrlCode_Portrait`'s placements.
const PORTRAIT_TILE_X: usize = 5;
const PORTRAIT_TILE_Y: usize = 0xD;
const PORTRAIT_TALK_SLOT_TILES: usize = 12;
const PORTRAIT_CUTSCENE_TILE_X: usize = 3;
const PORTRAIT_CUTSCENE_TILE_Y: usize = 0xE;
/// `DIALOGUE_PORTRAIT_SYMBOLS`, by id.
const PORTRAIT_SYMBOLS: [Option<&str>; PORTRAIT_ENTRIES] = [
    None,
    Some("Chaz"),
    Some("Alys"),
    Some("Hahn"),
    Some("Rune"),
    Some("Gryz"),
    Some("Rika"),
    Some("Demi"),
    Some("Wren"),
    Some("Raja"),
    Some("Kyra"),
    Some("Seth"),
    Some("Saya"),
    Some("Holt"),
    Some("Principal"),
    Some("Dorin"),
    Some("Pana"),
    Some("HntGuildReceptionist"),
    Some("Baker"),
    Some("Zio"),
    Some("Juza"),
    Some("Gyuna"),
    Some("Esper"),
    Some("Esper"),
    Some("EsperChief"),
    Some("EsperChief"),
    Some("GumbiousPriest"),
    Some("GumbiousBishop"),
    Some("Lashiec"),
    Some("XeAThoul"),
    Some("XeAThoul2"),
    Some("XeAThoul2"),
    Some("FortuneTeller"),
    Some("DElmLars"),
    Some("AlysWounded"),
    Some("ReFaze"),
    Some("MissingStudent"),
    Some("Tallas"),
    Some("DyingBoy"),
    Some("Sekreas"),
];

fn hex6(value: usize) -> String {
    format!("0x{value:06X}")
}

fn safe(label: &str) -> String {
    label
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Font
// ---------------------------------------------------------------------------

/// `font_strip`: the 80 glyphs left to right, cell N is byte N.
fn font_strip(raw: &[u8]) -> (usize, Vec<u8>, Vec<bool>) {
    let width = GLYPH_COUNT * GLYPH_WIDTH;
    let mut pixels = vec![0u8; width * GLYPH_HEIGHT];
    let mut blank = Vec::new();
    for (index, glyph) in raw.chunks(GLYPH_BYTES).enumerate() {
        blank.push(glyph.iter().all(|&row| row == 0));
        for (y, row) in glyph.iter().enumerate() {
            for x in 0..GLYPH_WIDTH {
                let set = row & (0x80 >> x) != 0;
                pixels[y * width + index * GLYPH_WIDTH + x] =
                    if set { TEXT_COLOR } else { BACKGROUND_COLOR };
            }
        }
    }
    (width, pixels, blank)
}

/// `font.png` and `font_json`.
pub fn font(rom: &[u8], root: &Path, palette: &[[u8; 3]]) -> Result<(), PackError> {
    let raw = slice(rom, FONT, FONT_SIZE, "Art_DialogueFont")?;
    let (width, pixels, blank) = font_strip(raw);
    let image = png::encode_indexed(
        width as u32,
        GLYPH_HEIGHT as u32,
        &pixels,
        palette,
        &[BACKGROUND_COLOR],
    )?;
    crate::write_file(root, "dialogue/font.png", &image)?;
    let charset = dialogue_charset();
    let mut by_char = BTreeMap::new();
    let mut glyphs = Vec::new();
    for byte in 0..=255u8 {
        if let Some(c) = charset.glyph(byte) {
            by_char.insert(c.to_string(), byte);
        }
    }
    for (index, &is_blank) in blank.iter().enumerate() {
        glyphs.push(
            Json::object()
                .with("byte", index)
                .with("byte_hex", format!("0x{index:02X}"))
                .with("char", charset.glyph(index as u8).map(|c| c.to_string()))
                .with("x", index * GLYPH_WIDTH)
                .with("y", 0u8)
                .with("width", GLYPH_WIDTH)
                .with("height", GLYPH_HEIGHT)
                .with("blank", is_blank),
        );
    }
    let mut by_char_json = Json::object();
    for (c, byte) in &by_char {
        by_char_json.insert(c.as_str(), *byte);
    }
    let json = Json::object()
        .with("format_version", FORMAT_VERSION)
        .with("kind", "dialogue_font")
        .with("png", "dialogue/font.png")
        .with("png_sha256", png::image_sha256(&image)?)
        .with("charset", "dialogue")
        .with("charset_source", "script/charset.asm")
        .with(
            "source",
            Json::object()
                .with("label", "Art_DialogueFont")
                .with("rom_offset", hex6(FONT))
                .with("rom_end_exclusive", hex6(FONT + FONT_SIZE))
                .with("compression", Json::Null)
                .with("size_bytes", FONT_SIZE)
                .with("sha256", sha256(raw)),
        )
        .with(
            "glyph",
            Json::object()
                .with("width", GLYPH_WIDTH)
                .with("height", GLYPH_HEIGHT)
                .with("bits_per_pixel", 1u8)
                .with("bytes_per_glyph", GLYPH_BYTES)
                .with("count", GLYPH_COUNT)
                .with("index_mask", format!("0x{GLYPH_INDEX_MASK:02X}"))
                .with("vram_patterns_per_glyph", GLYPH_HEIGHT / TILE_PIXELS)
                .with(
                    "note",
                    "GetFontGraphics indexes the font by the text byte itself (masked with $7F); \
                     the strip is laid out so cell N is byte N. ParseText expands each 1bpp row \
                     into eight 4bpp pixels, which is the only place the two 8x8 VRAM patterns \
                     exist.",
                ),
        )
        .with(
            "palette",
            Json::object()
                .with("source", "Pal_Init_Line_3")
                .with("rom_offset", hex6(PAL_INIT_LINE_3))
                .with("cram_line", DIALOGUE_CRAM_LINE)
                .with("text_index", TEXT_COLOR)
                .with("background_index", BACKGROUND_COLOR)
                .with("transparent_index", BACKGROUND_COLOR)
                .with("colors", colours(palette))
                .with(
                    "note",
                    "The glyph cell's background is index $E, which FillTextBackground paints \
                     across the whole window; the PNG marks it transparent so glyphs composite \
                     over a window fill, and filling with index $E underneath reproduces the \
                     cartridge.",
                ),
        )
        .with("glyphs", glyphs)
        .with("by_char", by_char_json)
        .with(
            "unmapped_glyphs",
            (0..GLYPH_COUNT)
                .filter(|&i| charset.glyph(i as u8).is_none())
                .collect::<Vec<_>>(),
        );
    crate::write_json(root, "dialogue/font.json", &json).map(drop)
}

// ---------------------------------------------------------------------------
// Menu font and the waiting arrow
// ---------------------------------------------------------------------------

fn menu_font_art(rom: &[u8]) -> Result<gfx::ArtRecord, PackError> {
    gfx::decompress_art(rom, MENU_FONT, MENU_FONT_LABEL, Some(MENU_FONT_SIZE), None)
}

/// `menu_font.png`: `ArtNem_Font` sixteen tiles across.
pub fn menu_font(rom: &[u8], root: &Path, palette: &[[u8; 3]]) -> Result<(), PackError> {
    let tiles = gfx::decode_tiles(&menu_font_art(rom)?.data)?;
    let (width, height, pixels) = gfx::compose_sheet(&tiles, 16)?;
    let image = png::encode_indexed(width as u32, height as u32, &pixels, palette, &[])?;
    crate::write_file(root, "dialogue/menu_font.png", &image)
}

/// `emit_scroll_arrow`'s PNG: the 2x1 sprite's two tiles side by side.
/// Its record goes into the window and tree files, which a later part
/// writes.
pub fn scroll_arrow(rom: &[u8], root: &Path, palette: &[[u8; 3]]) -> Result<(), PackError> {
    let tiles = gfx::decode_tiles(&menu_font_art(rom)?.data)?;
    let end = ARROW_TILE + ARROW_TILES;
    if tiles.len() != 87 || end > tiles.len() {
        return refuse(format!(
            "{MENU_FONT_LABEL} decodes to {} tiles; arrow needs tile range {ARROW_TILE}:{end}",
            tiles.len()
        ));
    }
    let mapping = slice(
        rom,
        ARROW_MAPPING,
        ARROW_MAPPING_PIN.0,
        "Mappings_ScrollTextArrow",
    )?;
    if sha256(mapping) != ARROW_MAPPING_PIN.1 {
        return refuse(format!(
            "Mappings_ScrollTextArrow bytes drifted: got {}",
            crate::png::hex(mapping)
        ));
    }
    let (left, right) = (&tiles[ARROW_TILE], &tiles[ARROW_TILE + 1]);
    let mut pixels = Vec::with_capacity(16 * TILE_PIXELS);
    for row in 0..TILE_PIXELS {
        pixels.extend_from_slice(&left[row * TILE_PIXELS..(row + 1) * TILE_PIXELS]);
        pixels.extend_from_slice(&right[row * TILE_PIXELS..(row + 1) * TILE_PIXELS]);
    }
    let image = png::encode_indexed(
        (TILE_PIXELS * ARROW_TILES) as u32,
        TILE_PIXELS as u32,
        &pixels,
        palette,
        &[0],
    )?;
    crate::write_file(root, "dialogue/scroll_arrow.png", &image)
}

// ---------------------------------------------------------------------------
// Portraits
// ---------------------------------------------------------------------------

/// `emit_portraits` with `extract_dialogue_portraits`: one 48x48 PNG per id
/// the retail table names, and the index.
pub fn portraits(rom: &[u8], root: &Path, palette: &[[u8; 3]]) -> Result<(), PackError> {
    let pointers = (0..PORTRAIT_ENTRIES)
        .map(|i| Ok(crate::m68k::l(rom, PORTRAIT_TABLE + i * 4)? as usize))
        .collect::<Result<Vec<_>, PackError>>()?;
    let bounds = gfx::gap_bounds(&pointers, rom.len(), &pointers);
    let mut records: BTreeMap<usize, gfx::ArtRecord> = BTreeMap::new();
    let mut first_by_offset: BTreeMap<usize, usize> = BTreeMap::new();
    let mut entries = Vec::new();
    for (index, &pointer) in pointers.iter().enumerate() {
        if pointer == 0 {
            continue;
        }
        let symbol = PORTRAIT_SYMBOLS[index];
        let label = symbol.map_or_else(
            || format!("ArtNem_DialPortrait_{index:02X}"),
            |s| format!("ArtNem_{s}DialPortrait"),
        );
        let record =
            match records.entry(pointer) {
                std::collections::btree_map::Entry::Occupied(slot) => slot.into_mut(),
                std::collections::btree_map::Entry::Vacant(slot) => slot.insert(
                    gfx::decompress_art(rom, pointer, &label, None, bounds.get(&pointer).copied())?,
                ),
            };
        let tiles = gfx::decode_tiles(&record.data)?;
        if tiles.len() != PORTRAIT_TILES {
            return refuse(format!(
                "{label} at 0x{pointer:06X} decodes to {} patterns, not the {PORTRAIT_TILES} the \
                 6x6 plane map at 0x{PORTRAIT_PLANE_MAP:06X} names",
                tiles.len()
            ));
        }
        let (width, height, pixels) = gfx::compose_sheet(&tiles, PORTRAIT_COLUMNS)?;
        let image = png::encode_indexed(width as u32, height as u32, &pixels, palette, &[0])?;
        let name = safe(symbol.unwrap_or(&format!("Portrait{index:02X}")));
        let path = format!("dialogue/portraits/{index:02X}_{name}.png");
        crate::write_file(root, &path, &image)?;
        entries.push(
            Json::object()
                .with("id", index)
                .with("id_hex", format!("0x{index:02X}"))
                .with("symbol", symbol)
                .with("png", path)
                .with("png_sha256", png::image_sha256(&image)?)
                .with("duplicate_of", first_by_offset.get(&pointer).copied())
                .with(
                    "art",
                    Json::object()
                        .with("label", label.as_str())
                        .with("rom_offset", hex6(pointer))
                        .with("compression", "nemesis")
                        .with("compressed_size", record.compressed_size)
                        .with("tile_count", record.tile_count)
                        .with("decompressed_sha256", record.decompressed_sha256.as_str()),
                ),
        );
        first_by_offset.entry(pointer).or_insert(index);
    }
    let json = Json::object()
        .with("format_version", FORMAT_VERSION)
        .with("kind", "dialogue_portraits")
        .with(
            "table",
            Json::object()
                .with("label", "DialoguePortraitArtPtrs")
                .with("rom_offset", hex6(PORTRAIT_TABLE))
                .with("entry_count", PORTRAIT_ENTRIES)
                .with("null_entry", 0u8)
                .with(
                    "note",
                    "Index 0 is a null pointer and $F4 id 0 means hide the portrait window. The \
                     disassembly's seven shopkeeper entries at $28-$2E are not in the retail \
                     table; those portraits are reached through the shop tables near 0x068000.",
                ),
        )
        .with(
            "geometry",
            Json::object()
                .with("width", PORTRAIT_COLUMNS * TILE_PIXELS)
                .with("height", PORTRAIT_COLUMNS * TILE_PIXELS)
                .with("tile_columns", PORTRAIT_COLUMNS)
                .with("tile_rows", PORTRAIT_TILES / PORTRAIT_COLUMNS)
                .with("tile_count", PORTRAIT_TILES)
                .with("vram_tile", format!("0x{PORTRAIT_VRAM_TILE:03X}"))
                .with("plane_map_rom_offset", hex6(PORTRAIT_PLANE_MAP))
                .with("screen_tile_x", PORTRAIT_TILE_X)
                .with("screen_tile_y", PORTRAIT_TILE_Y)
                .with("screen_x", PORTRAIT_TILE_X * TILE_PIXELS)
                .with("screen_y", PORTRAIT_TILE_Y * TILE_PIXELS)
                .with("talk_slot_stride_tiles", PORTRAIT_TALK_SLOT_TILES)
                .with("cutscene_screen_tile_x", PORTRAIT_CUTSCENE_TILE_X)
                .with("cutscene_screen_tile_y", PORTRAIT_CUTSCENE_TILE_Y)
                .with(
                    "note",
                    "The plane map at 0x2A2B36 lays the 36 patterns out row-major six across from \
                     tile $55C, priority set, palette line 2. In Talk mode (Game_Mode_Routine 4) \
                     $F4's second operand shifts the portrait right by 12 tiles per slot.",
                ),
        )
        .with(
            "palette",
            Json::object()
                .with("source", "Pal_Init_Line_3")
                .with("rom_offset", hex6(PAL_INIT_LINE_3))
                .with("cram_line", DIALOGUE_CRAM_LINE)
                .with("transparent_index", 0u8)
                .with("colors", colours(palette)),
        )
        .with("count", entries.len())
        .with("distinct_art", records.len())
        .with("portraits", entries);
    crate::write_json(root, "dialogue/portraits.json", &json).map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_glyph_row_sets_text_on_its_bits_and_background_elsewhere() {
        let mut raw = vec![0u8; FONT_SIZE];
        raw[0] = 0b1000_0001;
        raw[GLYPH_BYTES] = 0xFF;
        let (width, pixels, blank) = font_strip(&raw);
        assert_eq!(width, 640);
        assert_eq!(
            (pixels[0], pixels[1], pixels[7]),
            (TEXT_COLOR, BACKGROUND_COLOR, TEXT_COLOR)
        );
        assert_eq!(pixels[8], TEXT_COLOR);
        assert_eq!((blank[0], blank[1], blank[2]), (false, false, true));
    }

    #[test]
    fn portrait_symbols_cover_the_retail_table() {
        assert!(PORTRAIT_SYMBOLS[0].is_none());
        assert_eq!(PORTRAIT_SYMBOLS[0x27], Some("Sekreas"));
    }
}
