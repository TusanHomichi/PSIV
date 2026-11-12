//! `presentation/`: the port of `psiv_tools/presentation_pack.py`, whose
//! docs give the reasons (#216, slice 7f): every scene panel `Panel_Create`
//! can draw, the opening narration's background, the palettes scenes read,
//! the `LoadArt` previews, the objects scenes build, the shopkeeper
//! portrait, and `panels.json`, which records them all. `panels.json` is the
//! one pack file Python writes unsorted, so it keeps the dict order.

mod objects;

use std::collections::HashMap;
use std::path::Path;

use crate::decode::{Decodes, enigma, nemesis};
use crate::json::Json;
use crate::maps::extract_maps;
use crate::sprites::objects::{facing_table_extents, scan_field_objects};
use crate::{PackError, Rom, gfx, png};

const ROM_PANEL_TABLE: usize = 0x07B000;
const PANEL_RECORD_SIZE: usize = 0x1E;
const FORMAT: &str = "psiv-scene-presentation-v2";
const SCREEN_WIDTH: usize = 320;
const SCREEN_HEIGHT: usize = 224;

/// `PANEL_BANK_BASES`: the record table for each 64-id bank; bank 3
/// (`PanelData_Part4`) has no records.
const PANEL_BANK_BASES: [Option<usize>; 7] = [
    Some(0x07B000),
    Some(0x09F940),
    Some(0x2B0000),
    None,
    Some(0x2B9010),
    Some(0x2D6750),
    Some(0x2EECD0),
];

/// `DIALOGUE_ACTION_PANEL_IDS`: every panel a retail dialogue action names.
const DIALOGUE_ACTION_PANEL_IDS: [u16; 163] = [
    0x000, 0x001, 0x002, 0x003, 0x004, 0x005, 0x006, 0x007, 0x008, 0x009, 0x00A, 0x00B, 0x00C,
    0x00D, 0x00E, 0x00F, 0x010, 0x011, 0x012, 0x013, 0x014, 0x015, 0x016, 0x017, 0x018, 0x019,
    0x023, 0x024, 0x027, 0x028, 0x029, 0x02A, 0x02B, 0x02C, 0x02D, 0x02E, 0x02F, 0x030, 0x031,
    0x032, 0x035, 0x036, 0x037, 0x038, 0x039, 0x03A, 0x03D, 0x03E, 0x03F, 0x041, 0x043, 0x044,
    0x045, 0x046, 0x047, 0x048, 0x049, 0x04B, 0x04C, 0x04D, 0x04E, 0x04F, 0x050, 0x053, 0x054,
    0x055, 0x058, 0x059, 0x05C, 0x068, 0x06B, 0x06E, 0x06F, 0x071, 0x072, 0x077, 0x078, 0x079,
    0x07B, 0x07F, 0x081, 0x082, 0x083, 0x084, 0x085, 0x089, 0x08A, 0x08C, 0x08D, 0x08F, 0x090,
    0x091, 0x094, 0x095, 0x096, 0x097, 0x09A, 0x09C, 0x09D, 0x09F, 0x0A1, 0x0A2, 0x0A3, 0x0A6,
    0x0A7, 0x100, 0x101, 0x102, 0x103, 0x104, 0x105, 0x106, 0x108, 0x109, 0x10B, 0x10C, 0x111,
    0x112, 0x113, 0x114, 0x115, 0x117, 0x119, 0x11C, 0x11D, 0x11E, 0x11F, 0x120, 0x121, 0x122,
    0x123, 0x124, 0x126, 0x127, 0x128, 0x12A, 0x137, 0x138, 0x13C, 0x141, 0x142, 0x143, 0x144,
    0x145, 0x146, 0x15B, 0x15C, 0x15E, 0x172, 0x174, 0x176, 0x178, 0x17A, 0x17B, 0x17D, 0x17F,
    0x180, 0x181, 0x183, 0x184, 0x185, 0x186, 0x18D,
];

/// `ENDING_PANEL_IDS`: `Cutscene_Ending`'s whole panel stream.
const ENDING_PANEL_IDS: [u16; 50] = [
    0x147, 0x148, 0x149, 0x14A, 0x14B, 0x14C, 0x14D, 0x14E, 0x14F, 0x150, 0x151, 0x152, 0x153,
    0x154, 0x155, 0x156, 0x157, 0x158, 0x159, 0x15A, 0x15D, 0x15F, 0x160, 0x161, 0x162, 0x163,
    0x164, 0x165, 0x166, 0x167, 0x168, 0x169, 0x16A, 0x16B, 0x16C, 0x16D, 0x16E, 0x16F, 0x170,
    0x171, 0x173, 0x175, 0x177, 0x179, 0x17C, 0x17E, 0x182, 0x187, 0x188, 0x189,
];

/// The transcribed story scenes' own panels.
const SCENE_PANEL_IDS: [u16; 9] = [0x25, 0x26, 0x33, 0x34, 0x3B, 0x3C, 0x18B, 0x18F, 0x190];

/// The opening narration's art, mapping and palette.
const OPENING_ART: usize = 0x1C_F1F2;
const OPENING_MAPPING: usize = 0x1D_25BE;
const OPENING_PALETTE: usize = 0x1D_2A3C;

/// `PALETTE_REQUESTS`: palette words a scene reads from the ROM, by address
/// and word count; the Python list cites each one's reader.
const PALETTE_REQUESTS: [(usize, usize); 12] = [
    (0x1D_2A3C, 64),
    (0x1D_E0B8, 16),
    (0x06_E908, 14),
    (0x29_754E, 17),
    (0x29_756E, 17),
    (0x29_758E, 17),
    (0x09_F2BC, 64),
    (0x29_6300, 17),
    (0x07_82DA, 54),
    (0x06_EE16, 21),
    (0x2E_EA04, 16),
    (0x07_30FC, 16),
];

fn refuse<T>(message: String) -> Result<T, PackError> {
    Err(PackError(message))
}

fn hex6(value: usize) -> String {
    format!("0x{value:06X}")
}

/// `PANEL_IDS`: sorted and distinct.
pub fn panel_ids() -> Vec<u16> {
    let mut ids: Vec<u16> = (0x1A..0x23)
        .chain(SCENE_PANEL_IDS)
        .chain(DIALOGUE_ACTION_PANEL_IDS)
        .chain(ENDING_PANEL_IDS)
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// `panel_record_offset`.
fn panel_record_offset(id: u16) -> Result<usize, PackError> {
    let bank = usize::from(id >> 6);
    match PANEL_BANK_BASES.get(bank) {
        None => refuse(format!("panel id 0x{id:X} is outside the retail table")),
        Some(None) => refuse(format!(
            "panel id 0x{id:X} is in retail PanelData_Part4, which has no records"
        )),
        Some(Some(base)) => Ok(base + usize::from(id & 0x3F) * PANEL_RECORD_SIZE),
    }
}

fn slice<'a>(rom: &'a [u8], at: usize, len: usize, what: &str) -> Result<&'a [u8], PackError> {
    rom.get(at..at + len)
        .ok_or_else(|| PackError(format!("{what} at 0x{at:06X} runs past the ROM")))
}

/// `_palette`: sixteen colours.
fn palette16(rom: &[u8], at: usize) -> Result<Vec<[u8; 3]>, PackError> {
    Ok(gfx::palette_rgb(&gfx::decode_palette(slice(
        rom, at, 32, "palette",
    )?)?))
}

/// One drawn panel layer: its size, its CRAM indices and its colours.
struct Layer {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
    colours: Vec<[u8; 3]>,
}

/// `_layer_pixels`: one plane of a panel. A mapping that stops a row short
/// (retail panel 0x14E's plane B) leaves that row at the VDP's clear value.
fn layer(
    rom: &[u8],
    art: &[[u8; 64]],
    mapping: usize,
    base_tile: u16,
    columns: usize,
    rows: usize,
    palette: usize,
) -> Result<Layer, PackError> {
    let expected = columns * rows;
    let (words, _) = enigma::decompress(rom, mapping, base_tile, Some(expected))?;
    if words.len() > expected || !words.len().is_multiple_of(columns) {
        return refuse(format!(
            "mapping at 0x{mapping:06X} decoded {} words, expected {columns}x{rows}",
            words.len()
        ));
    }
    let (width, height, pixels) =
        gfx::compose(&words, columns, art, usize::from(base_tile & 0x07FF))?;
    let mut layer = Layer {
        width,
        height,
        pixels,
        colours: palette16(rom, palette)?,
    };
    layer.clear_rows_below(rows);
    Ok(layer)
}

impl Layer {
    /// `Panel_Create` leaves the cell rows a mapping never reached at the
    /// VDP's clear value, which draws nothing.
    fn clear_rows_below(&mut self, rows: usize) {
        let missing = rows.saturating_sub(self.height / 8);
        let width = self.width;
        let pixels = &mut self.pixels;
        pixels.resize(pixels.len() + width * missing * 8, 0);
        self.height += missing * 8;
    }
}

fn be(record: &[u8], at: usize, size: usize) -> usize {
    record[at..at + size]
        .iter()
        .fold(0, |acc, &b| (acc << 8) | usize::from(b))
}

/// `decode_panel`: one panel drawn on the 320x224 screen, plane A over
/// plane B, and its record.
fn panel(rom: &[u8], root: &Path, id: u16) -> Result<(Json, PanelPalettes), PackError> {
    let offset = panel_record_offset(id)?;
    let record = slice(
        rom,
        offset,
        PANEL_RECORD_SIZE,
        &format!("panel 0x{id:02X} record"),
    )?;
    let destination = be(record, 0, 2);
    let art_address = be(record, 2, 4);
    let (columns, rows, x, y) = (
        usize::from(record[6]),
        usize::from(record[7]),
        usize::from(record[8]),
        usize::from(record[9]),
    );
    let art_a = be(record, 10, 2) as u16;
    let palette_a = be(record, 12, 4);
    let mapping_a = be(record, 16, 4);
    let art_b = be(record, 20, 2) as u16;
    let palette_b = be(record, 22, 4);
    let mapping_b = be(record, 26, 4);

    let (raw, consumed) = nemesis::decompress(rom, art_address)?;
    let tiles = gfx::decode_tiles(&raw)?;
    let a = layer(rom, &tiles, mapping_a, art_a, columns, rows, palette_a)?;
    if (a.width, a.height) != (columns * 8, rows * 8) {
        return refuse("Panel A dimensions disagree with its record".into());
    }
    let b = if art_b != 0xFFFF && mapping_b != 0 {
        let b = layer(rom, &tiles, mapping_b, art_b, columns, rows, palette_b)?;
        if (b.width, b.height) != (a.width, a.height) {
            return refuse("Panel B dimensions disagree with Panel A".into());
        }
        Some(b)
    } else {
        None
    };

    // Index 0 is transparent; A takes 1..16 and B 17..32, B behind A.
    let mut palette = vec![[0u8; 3]];
    palette.extend(&a.colours);
    if let Some(b) = &b {
        palette.extend(&b.colours);
    }
    palette.resize(33.max(palette.len()), [0, 0, 0]);
    let mut pixels = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT];
    let mut blit = |layer: &Layer, base: u8| {
        for row in 0..layer.height {
            let sy = y * 8 + row;
            if sy >= SCREEN_HEIGHT {
                continue;
            }
            for col in 0..layer.width {
                let sx = x * 8 + col;
                if sx >= SCREEN_WIDTH {
                    continue;
                }
                let value = layer.pixels[row * layer.width + col] & 0x0F;
                if value != 0 {
                    pixels[sy * SCREEN_WIDTH + sx] = base + value;
                }
            }
        }
    };
    if let Some(b) = &b {
        blit(b, 17);
    }
    blit(&a, 1);
    let name = format!("panel_{id:02x}.png");
    let image = png::encode_indexed(
        SCREEN_WIDTH as u32,
        SCREEN_HEIGHT as u32,
        &pixels,
        &palette,
        &[0],
    )?;
    crate::write_file(root, &format!("presentation/panels/{name}"), &image)?;
    let has_b = art_b != 0xFFFF;
    let palettes = PanelPalettes {
        a: (art_a != 0xFFFF).then_some(palette_a),
        b: has_b.then_some(palette_b),
    };
    let record = Json::object()
        .with("id", id)
        .with("png", format!("presentation/panels/{name}"))
        .with("record_offset", hex6(offset))
        .with("art_address", hex6(art_address))
        .with("art_consumed", consumed)
        .with("destination_tile", destination)
        .with("columns", columns)
        .with("rows", rows)
        .with("x_cells", x)
        .with("y_cells", y)
        .with("art_tile_a", art_a)
        .with("palette_a", hex6(palette_a))
        .with("mapping_a", hex6(mapping_a))
        .with("art_tile_b", art_b)
        .with("palette_b", has_b.then(|| hex6(palette_b)))
        .with("mapping_b", has_b.then(|| hex6(mapping_b)))
        .with("decoded_size", vec![a.width, a.height]);
    Ok((record, palettes))
}

/// `decode_opening_background`.
fn opening(rom: &[u8], root: &Path) -> Result<Json, PackError> {
    let (raw, art_consumed) = nemesis::decompress(rom, OPENING_ART)?;
    let tiles = gfx::decode_tiles(&raw)?;
    let (words, mapping_consumed) =
        enigma::decompress(rom, OPENING_MAPPING, 0x2010, Some(40 * 16))?;
    if words.len() != 40 * 16 {
        return refuse(format!(
            "opening mapping decoded {} words, expected 640",
            words.len()
        ));
    }
    let (width, height, pixels) = gfx::compose(&words, 40, &tiles, 0x10)?;
    let colours = gfx::palette_rgb(&gfx::decode_palette(slice(
        rom,
        OPENING_PALETTE,
        128,
        "opening palette",
    )?)?);
    let image = png::encode_indexed(width as u32, height as u32, &pixels, &colours, &[0])?;
    crate::write_file(root, "presentation/opening_background.png", &image)?;
    Ok(Json::object()
        .with("art", hex6(OPENING_ART))
        .with("art_consumed", art_consumed)
        .with("mapping", hex6(OPENING_MAPPING))
        .with("mapping_consumed", mapping_consumed)
        .with("palette", hex6(OPENING_PALETTE))
        .with("size", vec![width, height])
        .with("screen_y", 40u8))
}

/// `panel_palette_requests`: each distinct sixteen-word palette
/// `Panel_Draw` copies for a layer it draws (art tile not `$FFFF`), in
/// address order, leaving out what `PALETTE_REQUESTS` names.
fn panel_palette_requests(panels: &[PanelPalettes]) -> Vec<(usize, usize)> {
    let mut found: Vec<usize> = panels
        .iter()
        .flat_map(|panel| [panel.a, panel.b])
        .flatten()
        .filter(|address| !PALETTE_REQUESTS.iter().any(|(named, _)| named == address))
        .collect();
    found.sort_unstable();
    found.dedup();
    found.into_iter().map(|address| (address, 16)).collect()
}

/// `decode_palette_requests`.
fn palettes(rom: &[u8], panels: &[PanelPalettes]) -> Result<Vec<Json>, PackError> {
    PALETTE_REQUESTS
        .into_iter()
        .chain(panel_palette_requests(panels))
        .map(|(address, words)| {
            let raw = slice(rom, address, words * 2, "palette")?;
            Ok(Json::object()
                .with("rom_addr", hex6(address))
                .with("words", words)
                .with("raw_hex", png::hex(raw)))
        })
        .collect()
}

/// The panels' palettes, for the shared palette list.
struct PanelPalettes {
    a: Option<usize>,
    b: Option<usize>,
}

/// Everything `emit_presentation` writes, `panels.json` last.
pub fn build(rom: &Rom, root: &Path) -> Result<Json, PackError> {
    let data = rom.bytes();
    let mut panels = Vec::new();
    let mut drawn = Vec::new();
    for id in panel_ids() {
        let (record, palettes) = panel(data, root, id)?;
        panels.push(record);
        drawn.push(palettes);
    }
    let opening = opening(data, root)?;
    let decodes = Decodes::new(data);
    let maps = extract_maps(data, &decodes)?;
    let routines = scan_field_objects(data)?;
    let extents = facing_table_extents(&routines);
    let inputs = objects::Inputs {
        decodes: &decodes,
        maps: &maps,
        routines: routines
            .into_iter()
            .map(|routine| (routine.object_id(), routine))
            .collect::<HashMap<_, _>>(),
        extents,
    };
    let load_art = objects::load_art(data, root, &inputs)?;
    let temporary = objects::temporary_objects(data, root, &inputs)?;
    let portraits = vec![objects::portrait(data, root)?];
    let banks: Vec<Json> = PANEL_BANK_BASES
        .iter()
        .enumerate()
        .filter_map(|(bank, base)| {
            base.map(|base| {
                Json::object()
                    .with("first_id", format!("0x{:03X}", bank * 0x40))
                    .with("last_id", format!("0x{:03X}", bank * 0x40 + 0x3F))
                    .with("record_table", hex6(base))
            })
        })
        .collect();
    let palette_records = palettes(data, &drawn)?;
    let part = Json::object()
        .with("record_table", hex6(ROM_PANEL_TABLE))
        .with("record_size", PANEL_RECORD_SIZE)
        .with("panels", panels.len())
        .with("palettes", palette_records.len())
        .with("load_art", load_art.len())
        .with("temporary_objects", temporary.len())
        .with("portraits", portraits.len());
    let coverage = Json::object()
        .with("panel_records", panels.len())
        .with("load_art_records", load_art.len())
        .with("temporary_object_keys", temporary.len())
        .with("generic_portraits", portraits.len());
    let manifest = Json::object()
        .with("format", FORMAT)
        .with("source_rom_sha256", crate::rom::EXPECTED_SHA256)
        .with("record_table", hex6(ROM_PANEL_TABLE))
        .with("record_size", PANEL_RECORD_SIZE)
        .with("record_banks", banks)
        .with("screen", vec![SCREEN_WIDTH, SCREEN_HEIGHT])
        .with("opening_background", opening)
        .with("palettes", palette_records)
        .with("panels", panels)
        .with("load_art", load_art)
        .with("temporary_objects", temporary)
        .with("portraits", portraits)
        .with("coverage", coverage.clone());
    let mut text = crate::json::dumps(&manifest, false);
    text.push('\n');
    crate::write_file(root, "presentation/panels.json", text.as_bytes())?;
    // The manifest records what was emitted; panels.json holds the rest.
    Ok(Json::object().with("presentation", part.with("coverage", coverage)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_ids_are_sorted_distinct_and_banked() {
        let ids = panel_ids();
        assert!(ids.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(ids.len(), 231);
        assert!([0x18B, 0x18F, 0x190].iter().all(|id| ids.contains(id)));
        assert_eq!(
            panel_record_offset(0x41).unwrap(),
            0x09F940 + PANEL_RECORD_SIZE
        );
        assert!(panel_record_offset(0xC0).unwrap_err().0.contains("Part4"));
        assert!(panel_record_offset(0x1C0).is_err());
    }

    #[test]
    fn a_short_mapping_leaves_its_missing_rows_clear() {
        let mut layer = Layer {
            width: 16,
            height: 8,
            pixels: vec![5; 16 * 8],
            colours: Vec::new(),
        };
        layer.clear_rows_below(3);
        assert_eq!((layer.width, layer.height), (16, 24));
        assert!(layer.pixels[..16 * 8].iter().all(|&p| p == 5));
        assert!(layer.pixels[16 * 8..].iter().all(|&p| p == 0));
        assert_eq!(layer.pixels.len(), 16 * 24);
        layer.clear_rows_below(3);
        assert_eq!(layer.height, 24, "a full layer is left as it is");
    }

    #[test]
    fn panel_palettes_are_listed_once_in_address_order_after_the_named_ones() {
        let named = PALETTE_REQUESTS[1].0;
        let drawn = [
            PanelPalettes {
                a: Some(0x300),
                b: Some(0x100),
            },
            PanelPalettes {
                a: Some(0x100),
                b: None,
            },
            PanelPalettes {
                a: Some(named),
                b: Some(0x200),
            },
        ];
        assert_eq!(
            panel_palette_requests(&drawn),
            [(0x100, 16), (0x200, 16), (0x300, 16)]
        );
    }
}
