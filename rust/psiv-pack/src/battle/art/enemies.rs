//! `battle/art/enemies.json` and the 153 body PNGs: the port of
//! `enemy_records`, `enemy_art_bounds`, `build_tile_bank` and `enemy_mapping`
//! in `psiv_tools/battle_art.py`, and of `enemy_body_png` and
//! `build_enemies` in `psiv_tools/battle_art_pack.py`. The first module's
//! docs give the record layout and the three facts proven about it: the
//! `+$00` word is a VRAM reservation, the mapping addresses the three art
//! blobs in the order art #3, #1, #2, and the width byte is a half-width.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::{
    ENEMY_FIXED_COLORS, ENEMY_PALETTE_COLORS, ENEMY_PALETTE_FIRST_INDEX, ENEMY_PALETTE_STRIDE,
    ENEMY_PALETTE_TABLE, LINE_DEPENDENT_INDEX, TRANSPARENT_INDEX, UI_COLOR_14, UI_COLOR_SITES,
    compose, enemy_palette, enemy_palette_words, safe,
};
use crate::battle::names::enemy_symbols;
use crate::decode::{enigma, nemesis};
use crate::json::Json;
use crate::{PackError, gfx, png};

/// `ENEMY_ART_TABLE`: the 20-byte graphics records.
const ART_TABLE: usize = 0x27F3AE;
const ART_STRIDE: usize = 20;
const ENEMY_COUNT: usize = 153;
/// The fields in the order the mapping addresses them: art #3, #1, #2.
const BANK_ORDER: [usize; 3] = [2, 0, 1];
/// ProfoundDarkness2 and 3 store a raw word array where the others store an
/// Enigma stream (`RAW_MAPPING_ENEMY_IDS`).
const RAW_MAPPING_IDS: [usize; 2] = [0x86, 0x87];
const PNG_DIRECTORY: &str = "battle/art/enemies";

fn refuse<T>(message: String) -> Result<T, PackError> {
    Err(PackError(message))
}

fn hex6(value: usize) -> String {
    format!("0x{value:06X}")
}

/// One graphics record (`enemy_records`).
pub struct Record {
    pub id: usize,
    pub symbol: &'static str,
    pub offset: usize,
    vram_patterns: u16,
    pub art: [usize; 3],
    pub mapping: usize,
    half_width: u8,
    height: u8,
}

/// `enemy_records`: the 153 records, nothing decompressed.
pub fn records(rom: &[u8]) -> Result<Vec<Record>, PackError> {
    let symbols = enemy_symbols();
    (0..ENEMY_COUNT)
        .map(|id| {
            let at = ART_TABLE + id * ART_STRIDE;
            let long = |o: usize| crate::m68k::l(rom, at + o).map(|v| v as usize);
            let raw = rom.get(at..at + ART_STRIDE).ok_or_else(|| {
                PackError(format!(
                    "Pointer at 0x{at:06X} (+4) runs past the end of the ROM"
                ))
            })?;
            Ok(Record {
                id,
                symbol: symbols[id],
                offset: at,
                vram_patterns: u16::from_be_bytes([raw[0], raw[1]]),
                art: [long(2)?, long(6)?, long(10)?],
                mapping: long(14)?,
                half_width: raw[18],
                height: raw[19],
            })
        })
        .collect()
}

/// `enemy_art_bounds`: from each art or mapping offset to the next one.
pub fn bounds(records: &[Record], rom_size: usize) -> BTreeMap<usize, usize> {
    let landmarks: Vec<usize> = records
        .iter()
        .flat_map(|r| r.art.iter().copied().chain([r.mapping]))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    landmarks
        .iter()
        .enumerate()
        .map(|(i, &offset)| {
            let next = landmarks.get(i + 1).copied().unwrap_or(rom_size);
            (offset, next - offset)
        })
        .collect()
}

/// Where one blob landed in the bank (`_art_record`'s source, as the index
/// keeps it).
pub struct Source {
    pub field: usize,
    pub offset: usize,
    pub tile_count: u16,
    pub first_pattern: usize,
}

/// `build_tile_bank`: the three blobs in `BANK_ORDER`, each checked against
/// its header and the gap to the next landmark; `sources` stays in record
/// order.
pub fn bank(
    rom: &[u8],
    record: &Record,
    bounds: &BTreeMap<usize, usize>,
) -> Result<(Vec<[u8; 64]>, Vec<Source>), PackError> {
    bank_order(rom, record, bounds, &BANK_ORDER)
}

/// loc_7BFA uploads Art #1 at the fighter base copied by loc_12618.
pub fn attack_bank(
    rom: &[u8],
    record: &Record,
    bounds: &BTreeMap<usize, usize>,
) -> Result<(Vec<[u8; 64]>, Vec<Source>), PackError> {
    bank_order(rom, record, bounds, &[0])
}

fn bank_order(
    rom: &[u8],
    record: &Record,
    bounds: &BTreeMap<usize, usize>,
    order: &[usize],
) -> Result<(Vec<[u8; 64]>, Vec<Source>), PackError> {
    let mut tiles = Vec::new();
    let mut sources: Vec<Option<Source>> = vec![None, None, None];
    for &field in order {
        let offset = record.art[field];
        let label = format!("ArtNem_{}Battle{}", record.symbol, field + 1);
        let header = nemesis::read_header(rom, offset)?;
        let (data, consumed) = nemesis::decompress(rom, offset)?;
        if data.len() != usize::from(header.row_count) * 4 {
            return refuse(format!(
                "{label} at 0x{offset:06X}: decoded {} bytes but the header declares {} patterns",
                data.len(),
                header.tile_count
            ));
        }
        if let Some(&bound) = bounds.get(&offset)
            && consumed > bound + 1
        {
            return refuse(format!(
                "{label} at 0x{offset:06X}: consumed {consumed} bytes and ran into the next \
                 blob, which starts {bound} bytes in"
            ));
        }
        sources[field] = Some(Source {
            field,
            offset,
            tile_count: header.tile_count,
            first_pattern: tiles.len(),
        });
        tiles.extend(gfx::decode_tiles(&data)?);
    }
    Ok((tiles, sources.into_iter().flatten().collect()))
}

/// `enemy_mapping`: the body's pattern-name words, its format and size.
pub fn mapping(rom: &[u8], record: &Record) -> Result<(Vec<u16>, &'static str, usize), PackError> {
    let columns = 2 * usize::from(record.half_width);
    let expected = columns * usize::from(record.height);
    let (words, format) = if RAW_MAPPING_IDS.contains(&record.id) {
        let words = (0..expected)
            .map(|i| crate::m68k::w(rom, record.mapping + 2 * i))
            .collect::<Result<Vec<_>, _>>()?;
        (words, "raw_words")
    } else {
        let (words, _) = enigma::decompress(rom, record.mapping, 0, Some(4096)).map_err(|e| {
            PackError(format!(
                "enemy 0x{:02X} ({}): the mapping at 0x{:06X} is not an Enigma stream ({}). \
                 Only [134, 135] are known to store a raw word array there; a new one needs \
                 its own proof before it is trusted.",
                record.id, record.symbol, record.mapping, e.0
            ))
        })?;
        (words, "enigma")
    };
    if words.len() != expected {
        return refuse(format!(
            "enemy 0x{:02X} ({}): mapping at 0x{:06X} decoded {} words but the record's {}x{} \
             half-size means {columns}x{} = {expected} cells",
            record.id,
            record.symbol,
            record.mapping,
            words.len(),
            record.half_width,
            record.height,
            record.height
        ));
    }
    Ok((words, format, columns))
}

fn hex4(value: u16) -> String {
    format!("0x{value:04X}")
}

/// `_enemy_palette_layout`.
fn palette_layout(indices: &BTreeSet<u8>) -> Json {
    let mut by_line = Json::object();
    for (_, line, index, value) in UI_COLOR_SITES {
        if index == LINE_DEPENDENT_INDEX {
            by_line.insert(line.to_string(), hex4(value));
        }
    }
    let mut fixed = Json::object();
    for (index, value) in ENEMY_FIXED_COLORS {
        fixed.insert(index.to_string(), hex4(value));
    }
    Json::object()
        .with(
            "table",
            Json::object()
                .with("label", "loc_27FFA2")
                .with("rom_offset", hex6(ENEMY_PALETTE_TABLE))
                .with("entry_size", ENEMY_PALETTE_STRIDE)
                .with("entry_count", ENEMY_COUNT),
        )
        .with("cram_lines", vec![1u8, 2])
        .with(
            "line_chosen_by",
            "bit 7 of the battle slot's Enemy_Positions entry",
        )
        .with(
            "png_pixels_are",
            "CRAM indices within the enemy's line, 0-15",
        )
        .with("first_table_index", ENEMY_PALETTE_FIRST_INDEX)
        .with("table_color_count", ENEMY_PALETTE_COLORS)
        .with("fixed_colors", fixed)
        .with("ui_color_14", hex4(UI_COLOR_14))
        .with("index_15_by_cram_line", by_line)
        .with("transparent_index", TRANSPARENT_INDEX)
        .with(
            "placeholder_indices",
            vec![TRANSPARENT_INDEX, LINE_DEPENDENT_INDEX],
        )
        .with(
            "body_color_indices_used",
            indices.iter().copied().collect::<Vec<_>>(),
        )
        .with(
            "note",
            "Index 15 is the only entry that differs between CRAM lines 1 and 2, and no enemy \
             body pixel references it or index 14. The PNG palette is therefore the whole \
             line-independent set, with black placeholders at the transparent index and at 15.",
        )
}

/// `build_enemies`: write the body PNGs and return the index.
pub fn write(rom: &[u8], root: &Path) -> Result<Json, PackError> {
    let records = records(rom)?;
    let bounds = bounds(&records, rom.len());
    let mut entries = Vec::new();
    let mut seen = BTreeSet::new();
    for record in &records {
        let (tiles, sources) = bank(rom, record, &bounds)?;
        let (words, format, columns) = mapping(rom, record)?;
        if words.iter().any(|w| w & 0x6000 != 0) {
            return refuse(format!(
                "enemy 0x{:02X} ({}): its mapping sets palette bits, so the body cannot be \
                 emitted without a CRAM line",
                record.id, record.symbol
            ));
        }
        let (width, height, pixels) = compose(&words, columns, &tiles, 0)?;
        seen.extend(pixels.iter().copied());
        let image = png::encode_indexed(
            width as u32,
            height as u32,
            &pixels,
            &enemy_palette(rom, record.id)?,
            &[TRANSPARENT_INDEX as u8],
        )?;
        let holes = words
            .iter()
            .filter(|&&w| w & 0x07FF != 0 && tiles[usize::from(w & 0x07FF)] == [0; 64])
            .count();
        let used = words
            .iter()
            .map(|w| usize::from(w & 0x07FF))
            .max()
            .unwrap_or(0)
            + 1;
        let path = format!(
            "{PNG_DIRECTORY}/{:03}_{}.png",
            record.id,
            safe(record.symbol)
        );
        crate::write_file(root, &path, &image)?;
        let rows = usize::from(record.height);
        entries.push(
            Json::object()
                .with("id", record.id)
                .with("symbol", record.symbol)
                .with("png", path.as_str())
                .with("png_sha256", png::image_sha256(&image)?)
                .with("record_offset", hex6(record.offset))
                .with("half_width_cells", record.half_width)
                .with("width_cells", columns)
                .with("height_cells", rows)
                .with("width_pixels", columns * 8)
                .with("height_pixels", rows * 8)
                .with("vram_patterns", record.vram_patterns)
                .with("base_pattern", 0u8)
                .with("bank_patterns", tiles.len())
                .with("patterns_used", used)
                .with("body_complete", holes == 0)
                .with("body_holes", holes)
                .with(
                    "mapping",
                    Json::object()
                        .with("rom_offset", hex6(record.mapping))
                        .with("format", format)
                        .with("cell_count", words.len()),
                )
                .with(
                    "art",
                    sources
                        .iter()
                        .map(|s| {
                            Json::object()
                                .with("field", format!("+0x{:02X}", 2 + 4 * s.field))
                                .with("rom_offset", hex6(s.offset))
                                .with("compression", "nemesis")
                                .with("pattern_count", s.tile_count)
                                .with("first_pattern", s.first_pattern)
                        })
                        .collect::<Vec<_>>(),
                )
                .with(
                    "palette_words",
                    enemy_palette_words(rom, record.id)?
                        .into_iter()
                        .map(hex4)
                        .collect::<Vec<_>>(),
                )
                .with(
                    "palette_rom_offset",
                    hex6(ENEMY_PALETTE_TABLE + record.id * ENEMY_PALETTE_STRIDE),
                ),
        );
    }
    Ok(Json::object()
        .with("kind", "battle_enemy_art")
        .with("count", entries.len())
        .with(
            "art_table",
            Json::object()
                .with("label", "loc_27F3AE")
                .with("rom_offset", hex6(ART_TABLE))
                .with("entry_size", ART_STRIDE)
                .with("entry_count", ENEMY_COUNT),
        )
        .with(
            "art_bank_order",
            BANK_ORDER.iter().map(|f| f + 1).collect::<Vec<_>>(),
        )
        .with("raw_mapping_enemy_ids", RAW_MAPPING_IDS.to_vec())
        .with("palette_layout", palette_layout(&seen))
        .with(
            "body_layer_only",
            Json::object()
                .with("overlay", "loc_7C56 EnemySpriteMappingsOffs piece lists")
                .with(
                    "note",
                    "This index is the static plane body. The additive enemy_overlays.json \
                     index carries the dynamic tile replacement frames and body placements.",
                ),
        )
        .with("enemies", entries)
        .with("format_version", crate::battle::FORMAT_VERSION))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_run_to_the_next_landmark_or_the_rom_end() {
        let record = |art: [usize; 3], mapping| Record {
            id: 0,
            symbol: "X",
            offset: 0,
            vram_patterns: 0,
            art,
            mapping,
            half_width: 1,
            height: 1,
        };
        let records = [record([10, 30, 20], 40), record([20, 50, 10], 60)];
        let b = bounds(&records, 100);
        assert_eq!((b[&10], b[&20], b[&30], b[&60]), (10, 10, 10, 40));
    }

    #[test]
    fn a_raw_mapping_is_half_width_doubled_by_height_words() {
        let mut rom = vec![0u8; 0x40];
        rom[0x10..0x18].copy_from_slice(&[0, 1, 0, 2, 0, 3, 0, 4]);
        let record = Record {
            id: 0x86,
            symbol: "ProfoundDarkness2",
            offset: 0,
            vram_patterns: 0,
            art: [0, 0, 0],
            mapping: 0x10,
            half_width: 1,
            height: 2,
        };
        let (words, format, columns) = mapping(&rom, &record).unwrap();
        assert_eq!((words, format, columns), (vec![1, 2, 3, 4], "raw_words", 2));
    }
}
