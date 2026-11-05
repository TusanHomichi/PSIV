//! The ship's destination screen: the port of `psiv_tools/ship_menu_pack.py`,
//! which owns the retail citations (#216, slice 5). `loc_63BC4` draws a
//! planetary map behind its windows; this replays the drawing for each of the
//! eight cover combinations, and writes the strings, windows and palette.
//!
//! `ship_menu.json` is `psiv_data::ShipMenuJson`. What the game doesn't read
//! (provenance, each record's `rom_offset`, a palette entry's word, each
//! background's digest) has no typed home yet and is laid over it.

use std::path::Path;

use psiv_data::{
    BackgroundRecord, Colors, FlightCaption, PaletteEntry, RomHash, ShipMenuJson, ShipWindow,
};
use sha2::{Digest, Sha256};

use crate::decode::{Call, Decodes};
use crate::gfx;
use crate::json::{Json, overlay};
use crate::png::hex;
use crate::text::decode_name;
use crate::{PackError, Rom};

const DIRECTORY: &str = "ship_menu";
const SCREEN_WIDTH: usize = 320;
const SCREEN_HEIGHT: usize = 224;
const CELLS_WIDE: usize = 40;
const CELLS_HIGH: usize = 28;
const PLANE_STRIDE: usize = 64;
const PLANE_WORDS: usize = 64 * 32;

/// `loc_63F00`: Nemesis art and the VRAM tile each blob lands at.
const ART: [(usize, usize); 2] = [(0x1D7FAE, 0x100), (0x1D8A0E, 0x1EB)];
/// `loc_63F2C`: the two pictures, 26 x 21 cells at screen cell (2, 2).
const PICTURE_A: (usize, u16) = (0x1DDB88, 0x100);
const PICTURE_B: (usize, u16) = (0x1DDC2A, 0x2100);
const PICTURE_ORIGIN: (usize, usize) = (2, 2);
const PICTURE_SIZE: (usize, usize) = (26, 21);
/// `loc_1DE158`: 32 words into CRAM lines 0 and 1.
const PALETTE_OFFSET: usize = 0x1DE158;
const PALETTE_WORDS: usize = 32;
/// The covers: (rom offset, base word, cell x, cell y, cols, rows).
type Cover = (usize, u16, usize, usize, usize, usize);
const RYKROS_MAPS: [Cover; 3] = [
    (0x1DDBFE, 0x100, 11, 8, 1, 2),
    (0x1DDC06, 0x100, 19, 13, 2, 2),
    (0x1DDC20, 0x100, 12, 5, 2, 2),
];
/// Single words written after the Rykros maps.
const RYKROS_WORDS: [(usize, usize, u16); 7] = [
    (13, 4, 0x21EB),
    (11, 7, 0x21EB),
    (11, 10, 0x21EB),
    (25, 7, 0x21EB),
    (23, 10, 0x21EB),
    (26, 6, 0x21EC),
    (22, 11, 0x21EC),
];
const KURAN_MAP: Cover = (0x1DDC12, 0x100, 6, 12, 3, 2);
const AIR_CASTLE_MAP: Cover = (0x1DDBF2, 0x100, 15, 12, 2, 2);
const COVER_RYKROS: usize = 1;
const COVER_KURAN: usize = 2;
const COVER_AIR_CASTLE: usize = 4;
/// `WinGroup_Event`: the window ids this screen creates.
const WINDOW_IDS: [u8; 6] = [5, 6, 7, 8, 9, 10];
const WIN_GROUP_EVENT: usize = 0x0693A0;
const WIN_GROUP_EVENT_HEAD: [[u8; 4]; 7] = [
    [0x19, 0x04, 0x07, 0x15],
    [0x21, 0x05, 0x03, 0x14],
    [0x05, 0x05, 0x05, 0x0D],
    [0x06, 0x04, 0x1B, 0x0E],
    [0x05, 0x05, 0x03, 0x0E],
    [0x19, 0x04, 0x07, 0x15],
    [0x0A, 0x02, 0x1A, 0x09],
];
const PROMPT_OFFSET: usize = 0x2AAA46;
const SUFFIX_OFFSET: usize = 0x2AAA60;
const NAMES_OFFSET: usize = 0x2AAA7A;
const WORLD_NAMES: usize = 6;
/// `SpaceTravel_PlaceNamePtrs`, read by `loc_64C4A`.
const FLIGHT_CAPTION_PTRS: usize = 0x2AAADE;
/// `WinGroup_PlaceName`: nine name-length-indexed records.
const PLACE_WINDOWS: usize = 0x0695F8;

fn refuse<T>(message: String) -> Result<T, PackError> {
    Err(PackError(message))
}

fn vram_tiles(decodes: &Decodes) -> Result<Vec<Option<[u8; 64]>>, PackError> {
    let mut tiles = vec![None; 0x800];
    for (offset, first) in ART {
        let data = decodes.get(Call::Nemesis { offset })?;
        for (index, tile) in gfx::decode_tiles(&data.bytes())?.into_iter().enumerate() {
            tiles[first + index] = Some(tile);
        }
    }
    Ok(tiles)
}

fn map_words(
    decodes: &Decodes,
    offset: usize,
    base: u16,
    count: usize,
) -> Result<Vec<u16>, PackError> {
    let decoded = decodes.get(Call::Enigma {
        offset,
        base_tile: base,
        max_words: Some(PLANE_WORDS),
    })?;
    let crate::decode::Decoded::Words(words, _) = decoded.as_ref() else {
        return refuse(format!("mapping at 0x{offset:06X} did not decode to words"));
    };
    if words.len() < count {
        return refuse(format!(
            "mapping at 0x{offset:06X} decoded {} cells, wanted {count}",
            words.len()
        ));
    }
    Ok(words[..count].to_vec())
}

/// `PlaneMapToRAM`: `cols` words a row, `rows` rows, from cell (x, y).
fn blit(plane: &mut [u16], words: &[u16], (x, y, cols, rows): (usize, usize, usize, usize)) {
    for row in 0..rows {
        for col in 0..cols {
            plane[((y + row) % 32) * PLANE_STRIDE + (x + col) % PLANE_STRIDE] =
                words[row * cols + col];
        }
    }
}

/// `build_planes`: plane A and plane B as `loc_63F2C` leaves them.
fn build_planes(decodes: &Decodes, covers: usize) -> Result<(Vec<u16>, Vec<u16>), PackError> {
    let mut plane_a = vec![0u16; PLANE_WORDS];
    let mut plane_b = vec![0u16; PLANE_WORDS];
    let (cols, rows) = PICTURE_SIZE;
    let (x, y) = PICTURE_ORIGIN;
    for (plane, (offset, base)) in [(&mut plane_a, PICTURE_A), (&mut plane_b, PICTURE_B)] {
        blit(
            plane,
            &map_words(decodes, offset, base, cols * rows)?,
            (x, y, cols, rows),
        );
    }
    // In `loc_63F2C`'s order: the Rykros maps and words, then Kuran, then the
    // Air Castle.
    if covers & COVER_RYKROS != 0 {
        for (offset, base, x, y, w, h) in RYKROS_MAPS {
            blit(
                &mut plane_a,
                &map_words(decodes, offset, base, w * h)?,
                (x, y, w, h),
            );
        }
        for (x, y, word) in RYKROS_WORDS {
            plane_a[y * PLANE_STRIDE + x] = word;
        }
    }
    for (flag, (offset, base, x, y, w, h)) in
        [(COVER_KURAN, KURAN_MAP), (COVER_AIR_CASTLE, AIR_CASTLE_MAP)]
    {
        if covers & flag != 0 {
            blit(
                &mut plane_a,
                &map_words(decodes, offset, base, w * h)?,
                (x, y, w, h),
            );
        }
    }
    Ok((plane_a, plane_b))
}

/// `compose_background`: 320 x 224 global CRAM indices, backdrop 0. A
/// high-priority cell beats a low one on either plane, and plane A wins ties.
fn compose_background(
    decodes: &Decodes,
    covers: usize,
    tiles: &[Option<[u8; 64]>],
) -> Result<Vec<u8>, PackError> {
    let (plane_a, plane_b) = build_planes(decodes, covers)?;
    let mut out = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT];
    for cy in 0..CELLS_HIGH {
        for cx in 0..CELLS_WIDE {
            let mut cells = Vec::with_capacity(2);
            for plane in [&plane_a, &plane_b] {
                let word = plane[cy * PLANE_STRIDE + cx];
                let tile = usize::from(word & 0x07FF);
                let art = match tiles[tile] {
                    Some(art) => art,
                    None if word != 0 => {
                        return refuse(format!(
                            "cell ({cx},{cy}) names VRAM tile 0x{tile:03X}, not loaded"
                        ));
                    }
                    None => [0; 64],
                };
                cells.push((word, art));
            }
            for y in 0..8 {
                for x in 0..8 {
                    let mut chosen = 0;
                    let mut best = -1;
                    for (rank, (word, art)) in cells.iter().enumerate() {
                        let source_x = if word & 0x0800 != 0 { 7 - x } else { x };
                        let source_y = if word & 0x1000 != 0 { 7 - y } else { y };
                        let pixel = art[source_y * 8 + source_x];
                        if pixel == 0 {
                            continue;
                        }
                        let score = if word & 0x8000 != 0 { 2 } else { 0 } + i32::from(rank == 0);
                        if score > best {
                            best = score;
                            chosen = ((word >> 13) & 3) as u8 * 16 + pixel;
                        }
                    }
                    out[(cy * 8 + y) * SCREEN_WIDTH + cx * 8 + x] = chosen;
                }
            }
        }
    }
    Ok(out)
}

fn rgb(word: u16) -> [u8; 3] {
    gfx::Color::decode(word).rgb()
}

fn window(rom: &[u8], id: u8, offset: usize) -> Result<(ShipWindow, Json), PackError> {
    let Some(&[width, height, x, y]) = rom.get(offset..offset + 4) else {
        return refuse(format!("window {id} runs past the ROM"));
    };
    let record = ShipWindow {
        id,
        width: width + 1,
        height: height + 1,
        x,
        y,
    };
    Ok((
        record,
        Json::object().with("rom_offset", format!("0x{offset:06X}")),
    ))
}

fn until(rom: &[u8], offset: usize, end: u8) -> Result<&[u8], PackError> {
    let tail = rom.get(offset..).unwrap_or(&[]);
    let Some(stop) = tail.iter().position(|&b| b == end) else {
        return refuse(format!("no 0x{end:02X} after 0x{offset:06X}"));
    };
    Ok(&tail[..stop])
}

/// The record, the research laid over it, and the eight backgrounds.
pub fn extract_ship_menu(
    rom: &[u8],
    decodes: &Decodes,
    stamp: &str,
) -> Result<(ShipMenuJson, Json, Vec<Vec<u8>>), PackError> {
    let tiles = vram_tiles(decodes)?;
    let backgrounds = (0..8)
        .map(|covers| compose_background(decodes, covers, &tiles))
        .collect::<Result<Vec<_>, _>>()?;

    let raw = rom
        .get(PALETTE_OFFSET..PALETTE_OFFSET + PALETTE_WORDS * 2)
        .ok_or_else(|| PackError("the palette runs past the ROM".into()))?;
    let words: Vec<u16> = raw
        .as_chunks::<2>()
        .0
        .iter()
        .map(|w| u16::from_be_bytes(*w))
        .collect();
    let palette: Vec<PaletteEntry> = words
        .iter()
        .map(|&w| PaletteEntry { rgb: rgb(w) })
        .collect();
    let palette_notes: Vec<Json> = words
        .iter()
        .map(|w| Json::object().with("word", format!("0x{w:04X}")))
        .collect();

    for (i, expected) in WIN_GROUP_EVENT_HEAD.iter().enumerate() {
        let at = WIN_GROUP_EVENT + i * 8;
        if rom.get(at..at + 4) != Some(&expected[..]) {
            return refuse("WinGroup_Event is not at 0x0693A0".into());
        }
    }
    let (windows, window_notes): (Vec<_>, Vec<_>) = WINDOW_IDS
        .iter()
        .map(|&id| window(rom, id, WIN_GROUP_EVENT + usize::from(id) * 8))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .unzip();
    let (place_windows, place_notes): (Vec<_>, Vec<_>) = (0..9u8)
        .map(|id| window(rom, id, PLACE_WINDOWS + usize::from(id) * 8))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .unzip();

    let mut names = Vec::with_capacity(WORLD_NAMES);
    let mut cursor = NAMES_OFFSET;
    for _ in 0..WORLD_NAMES {
        let raw = until(rom, cursor, 0xFE)?;
        names.push(decode_name(raw, "window")?);
        cursor += raw.len() + 1;
    }
    let suffix = until(rom, SUFFIX_OFFSET, 0xFF)?;
    let (first, second) = match suffix.iter().position(|&b| b == 0xFC) {
        Some(at) => (&suffix[..at], &suffix[at + 1..]),
        None => (suffix, &[][..]),
    };

    let mut flight_captions = Vec::with_capacity(WORLD_NAMES);
    let mut caption_notes = Vec::with_capacity(WORLD_NAMES);
    for world in 0..WORLD_NAMES {
        let entry = FLIGHT_CAPTION_PTRS + world * 4;
        let offset = rom
            .get(entry..entry + 4)
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize)
            .unwrap_or(usize::MAX);
        if offset >= rom.len() {
            return refuse(format!("flight caption {world} pointer is outside the ROM"));
        }
        let window = &rom[offset..(offset + 33).min(rom.len())];
        let raw = match window.iter().position(|&b| b == 0xFF) {
            Some(end) if end > 0 && window[..end].iter().all(|&b| b < 0xF0) => &window[..end],
            _ => {
                return refuse(format!(
                    "flight caption {world} is not a bounded RunText2 line"
                ));
            }
        };
        flight_captions.push(FlightCaption {
            world: world as u8,
            text: decode_name(raw, "dialogue")?,
            glyph_count: raw.len(),
            instant: true,
        });
        caption_notes.push(Json::object().with("rom_offset", format!("0x{offset:06X}")));
    }

    let record = ShipMenuJson {
        format_version: 1,
        kind: "ship_menu".into(),
        rom_sha256: serde_json::from_value::<RomHash>(stamp.into())
            .map_err(|error| PackError(format!("the ROM's hash doesn't read: {error}")))?,
        palette,
        colors: Colors {
            highlight: rgb(0x0EEE),
            dark: rgb(0x0400),
        },
        windows,
        prompt: decode_name(until(rom, PROMPT_OFFSET, 0xFE)?, "window")?,
        confirm_suffix: vec![
            decode_name(first, "window")?,
            decode_name(second, "window")?,
        ],
        names,
        flight_captions,
        place_windows,
        backgrounds: (0..8)
            .map(|covers| BackgroundRecord {
                covers,
                file: format!("{DIRECTORY}/background_{covers}.idx"),
            })
            .collect(),
    };
    let research = Json::object()
        .with(
            "screen",
            Json::object()
                .with("width", SCREEN_WIDTH)
                .with("height", SCREEN_HEIGHT),
        )
        .with("palette", palette_notes)
        .with("windows", window_notes)
        .with("place_windows", place_notes)
        .with("flight_captions", caption_notes)
        .with(
            "backgrounds",
            backgrounds
                .iter()
                .map(|data| Json::object().with("sha256", hex(&Sha256::digest(data))))
                .collect::<Vec<_>>(),
        )
        .with(
            "provenance",
            Json::object()
                .with(
                    "loader",
                    "loc_63F00, loc_63F2C, loc_64144 (ps4.asm:133745-133925)",
                )
                .with(
                    "art",
                    ART.iter()
                        .map(|(o, t)| {
                            Json::object()
                                .with("rom_offset", format!("0x{o:06X}"))
                                .with("vram_tile", format!("0x{t:03X}"))
                        })
                        .collect::<Vec<_>>(),
                )
                .with("palette", format!("0x{PALETTE_OFFSET:06X}"))
                .with(
                    "flight_caption_ptrs",
                    format!("0x{FLIGHT_CAPTION_PTRS:06X}"),
                )
                .with("place_windows", format!("0x{PLACE_WINDOWS:06X}")),
        );
    Ok((record, research, backgrounds))
}

/// The section.
pub fn build(rom: &Rom, root: &Path) -> Result<(), PackError> {
    let decodes = Decodes::new(rom.bytes());
    let stamp = hex(&Sha256::digest(rom.bytes()));
    let (record, research, backgrounds) = extract_ship_menu(rom.bytes(), &decodes, &stamp)?;
    for (covers, data) in backgrounds.iter().enumerate() {
        crate::write_file(root, &format!("{DIRECTORY}/background_{covers}.idx"), data)?;
    }
    let value = serde_json::to_value(&record)
        .map_err(|error| PackError(format!("cannot serialize ship_menu.json: {error}")))?;
    let mut file = Json::from(&value);
    overlay(&mut file, &research);
    crate::write_json(root, &format!("{DIRECTORY}/ship_menu.json"), &file).map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plane_wraps_at_its_edges() {
        let mut plane = vec![0u16; PLANE_WORDS];
        blit(&mut plane, &[1, 2, 3, 4], (63, 31, 2, 2));
        assert_eq!(plane[31 * 64 + 63], 1);
        assert_eq!(plane[31 * 64], 2);
        assert_eq!(plane[63], 3);
        assert_eq!(plane[0], 4);
    }

    #[test]
    fn until_needs_its_terminator() {
        assert_eq!(until(&[1, 2, 0xFE], 0, 0xFE).unwrap(), &[1, 2]);
        assert!(until(&[1, 2], 0, 0xFE).is_err());
    }
}
