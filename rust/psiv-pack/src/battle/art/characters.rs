//! `battle/art/characters.json` and the pose PNGs: the port of
//! `character_blocks`, `character_records`, `character_art_extents`,
//! `character_cram_line` and `verify_ui_colors` in
//! `psiv_tools/battle_art.py`, and of `character_pose_png`,
//! `character_palettes` and `build_characters` in
//! `psiv_tools/battle_art_pack.py`. The first module's docs give the PLC
//! block layout: a size word, the uncompressed art, then one Enigma mapping
//! per pose, with a second art pointer allowed mid-list.

use std::collections::BTreeMap;
use std::path::Path;

use super::{TRANSPARENT_INDEX, UI_COLOR_SITES, compose, safe};
use crate::battle::names::CHARACTERS;
use crate::decode::enigma;
use crate::json::Json;
use crate::{PackError, gfx, png};

/// `CHARACTER_PLC_TABLE`: eleven self-relative words; the last block's end
/// is the one bound the table can't give.
const PLC_TABLE: usize = 0x00825E;
const PLC_END: usize = 0x00836A;
/// `loc_86D6`: `moveq #6,d1 / moveq #6,d2`; the slot buffers are 36 words.
const COLUMNS: usize = 6;
const ROWS: usize = 6;
const SLOT_WORDS: usize = COLUMNS * ROWS;
const CRAM_LINE: u8 = 3;
/// `loc_822C`: `addi.w #$E000,d0`, priority plus line 3.
const BASE_WORD: u16 = 0xE000;
/// `CHARACTER_PALETTE_TABLE`: four 30-byte entries by `Vehicle_Index`.
const PALETTE_TABLE: usize = 0x0076A4;
const PALETTE_STRIDE: usize = 30;
const PALETTE_COLORS: usize = 15;
const VEHICLE_VARIANTS: [&str; 4] = ["on_foot", "land_rover", "ice_digger", "hydrofoil"];
/// `move.w #imm,(xxx).w` and `Palette_Table_Buffer` as a short address.
const MOVE_W_IMMEDIATE_TO_SHORT: u16 = 0x31FC;
const PALETTE_TABLE_BUFFER: usize = 0xFB00;
const PNG_DIRECTORY: &str = "battle/art/characters";
const LABEL_SOURCE: &str = "ps4disasm";

/// `POSE_LABELS`: the public disassembly's names, advisory only.
const POSE_LABELS: [&[&str]; 11] = [
    &["Idle", "Cast"],
    &[
        "Idle",
        "Cast",
        "Slasher",
        "ThrowSlasher",
        "TwoSlashers",
        "ThrowTwoSlashers",
    ],
    &["Idle", "Cast", "ArmsRaised"],
    &["Idle", "Cast", "ChargeMagic"],
    &["Idle", "Cast", "ArmsRaised"],
    &["Idle", "Cast"],
    &["Idle", "Cast", "Charge", "LoadGun", "FireGun"],
    &["Idle", "Cast", "Charge", "LoadGun", "FireGun"],
    &["Idle", "Cast", "ArmsRaised", "ArmsRaised2"],
    &[
        "Idle",
        "Cast",
        "Slasher",
        "ThrowSlasher",
        "TwoSlashers",
        "ThrowTwoSlashers",
        "ChargeMagic",
        "Cast2",
    ],
    &["Idle", "Cast"],
];

fn refuse<T>(message: String) -> Result<T, PackError> {
    Err(PackError(message))
}

fn hex6(value: usize) -> String {
    format!("0x{value:06X}")
}

fn hex4(value: u16) -> String {
    format!("0x{value:04X}")
}

fn word(rom: &[u8], at: usize) -> Result<u16, PackError> {
    Ok(crate::m68k::w(rom, at)?)
}

/// One pose: its mapping and the art it draws from.
struct Pose {
    mapping: usize,
    art: usize,
}

/// One PLC block (`character_records`).
struct Record {
    index: usize,
    plc: usize,
    vram_bytes: u16,
    art: Vec<usize>,
    poses: Vec<Pose>,
}

/// `_is_mapping`: a pose mapping decodes as Enigma; art does not.
fn is_mapping(rom: &[u8], offset: usize) -> bool {
    enigma::decompress(rom, offset, 0, Some(2048))
        .is_ok_and(|(words, _)| (1..=512).contains(&words.len()))
}

/// `character_blocks` and `character_records`.
fn records(rom: &[u8]) -> Result<Vec<Record>, PackError> {
    let starts = (0..CHARACTERS.len())
        .map(|i| Ok(PLC_TABLE + usize::from(word(rom, PLC_TABLE + 2 * i)?)))
        .collect::<Result<Vec<_>, PackError>>()?;
    if !starts.is_sorted() {
        return refuse(
            "CharBattleLongRangePLCOffs offsets are not ascending; the block boundaries cannot \
             be derived from them"
                .into(),
        );
    }
    let ends = starts.iter().skip(1).copied().chain([PLC_END]);
    starts
        .iter()
        .zip(ends)
        .enumerate()
        .map(|(index, (&start, end))| {
            let size = end.saturating_sub(start);
            if size < 2 || !(size - 2).is_multiple_of(4) {
                return refuse(format!(
                    "PLC block at 0x{start:06X} is {size} bytes, which is not a size word \
                     followed by whole longs"
                ));
            }
            let longs = (0..(size - 2) / 4)
                .map(|i| crate::m68k::l(rom, start + 2 + 4 * i).map(|v| v as usize))
                .collect::<Result<Vec<_>, _>>()?;
            let Some(&first) = longs.first() else {
                return refuse(format!("PLC block at 0x{start:06X} has no art pointer"));
            };
            let mut art = vec![first];
            let mut poses = Vec::new();
            let mut current = first;
            for &value in &longs[1..] {
                if is_mapping(rom, value) {
                    poses.push(Pose {
                        mapping: value,
                        art: current,
                    });
                } else {
                    current = value;
                    art.push(value);
                }
            }
            Ok(Record {
                index,
                plc: start,
                vram_bytes: word(rom, start)?,
                art,
                poses,
            })
        })
        .collect()
}

/// A pose's whole mapping (before the 36-word slot cuts it) and its length.
fn pose_words(rom: &[u8], pose: &Pose) -> Result<(Vec<u16>, usize), PackError> {
    Ok(enigma::decompress(rom, pose.mapping, 0, Some(2048))?)
}

fn highest(words: &[u16]) -> usize {
    words
        .iter()
        .map(|w| usize::from(w & 0x07FF))
        .max()
        .unwrap_or(0)
        + 1
}

/// `character_art_extents`: the patterns each blob holds, from the highest
/// pattern any pose drawing from it names.
fn extents(rom: &[u8], records: &[Record]) -> Result<BTreeMap<usize, usize>, PackError> {
    let mut out = BTreeMap::new();
    for pose in records.iter().flat_map(|r| &r.poses) {
        let (words, _) = pose_words(rom, pose)?;
        let entry = out.entry(pose.art).or_insert(0);
        *entry = (*entry).max(highest(&words));
    }
    Ok(out)
}

/// `character_cram_line`: line 3 as `loc_7634` leaves it, index 0 unwritten.
fn cram_line(rom: &[u8], vehicle: usize) -> Result<Vec<u16>, PackError> {
    let at = PALETTE_TABLE + vehicle * PALETTE_STRIDE;
    let mut words = vec![0u16];
    for i in 0..PALETTE_COLORS {
        words.push(word(rom, at + 2 * i)?);
    }
    Ok(words)
}

fn rgb(words: &[u16]) -> Result<Vec<[u8; 3]>, PackError> {
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_be_bytes()).collect();
    Ok(gfx::palette_rgb(&gfx::decode_palette(&bytes)?))
}

/// `verify_ui_colors`: each immediate is where and what it should be.
fn verify_ui_colors(rom: &[u8]) -> Result<Vec<Json>, PackError> {
    UI_COLOR_SITES
        .iter()
        .map(|&(at, line, index, value)| {
            let (opcode, immediate, destination) =
                (word(rom, at)?, word(rom, at + 2)?, word(rom, at + 4)?);
            let expected = PALETTE_TABLE_BUFFER + line * 32 + index * 2;
            if opcode != MOVE_W_IMMEDIATE_TO_SHORT
                || immediate != value
                || usize::from(destination) != expected
            {
                return refuse(format!(
                    "0x{at:06X}: expected `move.w #${value:04X},(Palette_Table_Buffer+${:02X})` \
                     but found opcode 0x{opcode:04X} immediate 0x{immediate:04X} destination \
                     0x{destination:04X}",
                    line * 32 + index * 2
                ));
            }
            Ok(Json::object()
                .with("rom_offset", hex6(at))
                .with("cram_line", line)
                .with("color_index", index)
                .with("value", hex4(value)))
        })
        .collect()
}

/// `character_palettes`: line 3 in each `Vehicle_Index` variant.
fn palettes(rom: &[u8]) -> Result<Vec<Json>, PackError> {
    VEHICLE_VARIANTS
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let words = cram_line(rom, index)?;
            Ok(Json::object()
                .with("vehicle_index", index)
                .with("name", *name)
                .with("rom_offset", hex6(PALETTE_TABLE + index * PALETTE_STRIDE))
                .with(
                    "words",
                    words[1..].iter().map(|&w| hex4(w)).collect::<Vec<_>>(),
                )
                .with(
                    "colors",
                    rgb(&words)?
                        .into_iter()
                        .map(|c| c.to_vec())
                        .collect::<Vec<_>>(),
                ))
        })
        .collect()
}

/// `character_pose_png`: the pose at the on-foot palette, its mapping cut to
/// the slot and drawn without the base word (priority and line 3 are no
/// pixel).
fn pose_png(
    rom: &[u8],
    pose: &Pose,
    patterns: usize,
    palette: &[[u8; 3]],
) -> Result<Vec<u8>, PackError> {
    let raw = rom.get(pose.art..pose.art + patterns * 32).ok_or_else(|| {
        PackError(format!(
            "character art 0x{:06X} at 0x{:06X} (+{}) runs past the end of the ROM",
            pose.art,
            pose.art,
            patterns * 32
        ))
    })?;
    let tiles = gfx::decode_tiles(raw)?;
    let (mut words, _) = pose_words(rom, pose)?;
    words.truncate(SLOT_WORDS);
    if words.iter().any(|w| w & 0x6000 != 0) {
        return refuse(format!(
            "pose mapping at 0x{:06X} sets palette bits",
            pose.mapping
        ));
    }
    let (width, height, pixels) = compose(&words, COLUMNS, &tiles, 0)?;
    Ok(png::encode_indexed(
        width as u32,
        height as u32,
        &pixels,
        palette,
        &[TRANSPARENT_INDEX as u8],
    )?)
}

/// `build_characters`: write the pose PNGs and return the index.
pub fn write(rom: &[u8], root: &Path) -> Result<Json, PackError> {
    let records = records(rom)?;
    let extents = extents(rom, &records)?;
    let mut palette = rgb(&cram_line(rom, 0)?)?;
    palette[TRANSPARENT_INDEX] = [0, 0, 0];
    let mut entries = Vec::new();
    let mut total_poses = 0;
    for record in &records {
        let symbol = CHARACTERS[record.index];
        let labels = POSE_LABELS[record.index];
        let mut poses = Vec::new();
        for (number, pose) in record.poses.iter().enumerate() {
            let image = pose_png(rom, pose, extents[&pose.art], &palette)?;
            let path = format!("{PNG_DIRECTORY}/{}_pose{number}.png", safe(symbol));
            crate::write_file(root, &path, &image)?;
            total_poses += 1;
            let (words, consumed) = pose_words(rom, pose)?;
            poses.push(
                Json::object()
                    .with("pose", number)
                    .with("label", labels.get(number).copied())
                    .with("label_source", LABEL_SOURCE)
                    .with("png", path.as_str())
                    .with("png_sha256", png::image_sha256(&image)?)
                    .with("mapping_rom_offset", hex6(pose.mapping))
                    .with("mapping_consumed", consumed)
                    .with("cell_count", words.len())
                    .with("fits_slot", words.len() <= SLOT_WORDS)
                    .with("art_rom_offset", hex6(pose.art))
                    .with("patterns_used", highest(&words)),
            );
        }
        let art: Vec<Json> = record
            .art
            .iter()
            .map(|&offset| {
                let patterns = extents.get(&offset).copied().unwrap_or(0);
                Json::object()
                    .with("rom_offset", hex6(offset))
                    .with("compression", Json::Null)
                    .with("pattern_count", patterns)
                    .with("size_bytes", patterns * 32)
                    .with("rom_end_exclusive", hex6(offset + patterns * 32))
            })
            .collect();
        entries.push(
            Json::object()
                .with("id", record.index + 1)
                .with("symbol", symbol)
                .with("plc_rom_offset", hex6(record.plc))
                .with("vram_bytes", record.vram_bytes)
                .with("art", art)
                .with("pose_count", poses.len())
                .with("poses", poses),
        );
    }
    let palette_json = Json::object()
        .with("cram_line", CRAM_LINE)
        .with(
            "table",
            Json::object()
                .with("label", "loc_76A4")
                .with("rom_offset", hex6(PALETTE_TABLE))
                .with("entry_size", PALETTE_STRIDE)
                .with("entry_count", VEHICLE_VARIANTS.len()),
        )
        .with("first_index", 1u8)
        .with("color_count", PALETTE_COLORS)
        .with("png_pixels_are", "CRAM indices within line 3, 0-15")
        .with("png_baked_variant", 0u8)
        .with("transparent_index", TRANSPARENT_INDEX)
        .with("variants", palettes(rom)?)
        .with("ui_colors", verify_ui_colors(rom)?)
        .with(
            "note",
            "All eleven characters share CRAM line 3, selected by Vehicle_Index. The PNGs bake \
             variant 0 for convenience; the pixel indices are the authority and all four \
             variants are here.",
        );
    Ok(Json::object()
        .with("format_version", crate::battle::FORMAT_VERSION)
        .with("kind", "battle_character_art")
        .with("count", entries.len())
        .with("pose_count", total_poses)
        .with(
            "plc_table",
            Json::object()
                .with("label", "CharBattleLongRangePLCOffs")
                .with("rom_offset", hex6(PLC_TABLE))
                .with("entry_count", CHARACTERS.len()),
        )
        .with("grid_cells", vec![COLUMNS, ROWS])
        .with("slot_words", SLOT_WORDS)
        .with("base_word", hex4(BASE_WORD))
        .with("palette", palette_json)
        .with(
            "pose_labels",
            Json::object()
                .with("source", LABEL_SOURCE)
                .with("advisory", true)
                .with(
                    "note",
                    "A pose is addressed by its index in the PLC block; the names come from the \
                     public disassembly and nothing keys on them.",
                ),
        )
        .with("characters", entries))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ui_colour_site_must_hold_its_move_and_destination() {
        let mut rom = vec![0u8; 0x7700];
        for (at, line, index, value) in UI_COLOR_SITES {
            let destination = (PALETTE_TABLE_BUFFER + line * 32 + index * 2) as u16;
            for (i, w) in [MOVE_W_IMMEDIATE_TO_SHORT, value, destination]
                .iter()
                .enumerate()
            {
                rom[at + 2 * i..at + 2 * i + 2].copy_from_slice(&w.to_be_bytes());
            }
        }
        assert_eq!(verify_ui_colors(&rom).unwrap().len(), 3);
        rom[0x007678 + 5] ^= 2;
        assert!(verify_ui_colors(&rom).is_err());
    }

    #[test]
    fn line_three_keeps_index_zero_unwritten() {
        let mut rom = vec![0u8; PALETTE_TABLE + 4 * PALETTE_STRIDE];
        rom[PALETTE_TABLE + PALETTE_STRIDE] = 0x0E;
        let words = cram_line(&rom, 1).unwrap();
        assert_eq!(words.len(), 16);
        assert_eq!((words[0], words[1]), (0, 0x0E00));
    }

    #[test]
    fn the_highest_pattern_is_the_extent() {
        assert_eq!(highest(&[0x0800 | 5, 0xE003]), 6);
    }
}
