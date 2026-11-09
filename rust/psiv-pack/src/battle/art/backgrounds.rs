//! `battle/art/backgrounds.json` and the 32 background PNGs: the port of
//! `background_selection`, `_reachable` and `build_backgrounds` in
//! `psiv_tools/battle_art_pack.py`, with the parts of `psiv_tools/planes.py`
//! and `psiv_tools/gfx.py` they draw on (`_battle_pointers`, `_gap_bounds`,
//! `decompress_art`, `decode_mapping`, `battle_cram`, `render`). Those
//! modules cite `Battle_SetupBackground`'s three tables and why each count is
//! the cartridge's: every art blob and mapping is bounded by the next address
//! the pointer table names.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::{compose, safe};
use crate::decode::{enigma, nemesis};
use crate::json::Json;
use crate::{PackError, gfx, png};

/// `BATTLE_BG_TABLE`: art, plane mapping and palette pointers per entry.
const TABLE: usize = 0x006ED4;
const COUNT: usize = 0x20;
const STRIDE: usize = 12;
const PALETTE_COLORS: usize = 13;
/// `BATTLE_BG_COLUMNS` (the plane's width) and `BATTLE_BG_ROWS`.
const COLUMNS: usize = 64;
const ROWS: usize = 24;
/// `PLANE_BUFFER_WORDS`: 64 x 32.
const PLANE_BUFFER_WORDS: usize = 64 * 32;
/// The three mappings no later pointer bounds (`BATTLE_BG_EXTRA_LANDMARKS`).
const EXTRA_LANDMARKS: [usize; 3] = [0x237A60, 0x23A226, 0x27AE00];
const PNG_DIRECTORY: &str = "battle/art/backgrounds";

/// `BATTLE_BG_PALETTE_SYMBOLS` and `BATTLE_BG_ART_SYMBOLS`: several
/// backgrounds share one art blob under different palettes.
const PALETTE_SYMBOLS: [&str; COUNT] = [
    "MotaDesert",
    "MotaBeach",
    "MotaGrass",
    "MotaSea",
    "Dezo1",
    "Dezo2",
    "BioPlant",
    "Wreckage",
    "PlateSys",
    "ClimCenter",
    "WeaponPlant",
    "VahalFort",
    "PlateSysF1",
    "AcademyBasement",
    "LadeaTower",
    "ZioFort",
    "GaruberkTw",
    "TonoeBasement",
    "AirCastle",
    "MonsenAndIslandCave",
    "MonsenAndIslandCave",
    "Rykros",
    "Zema",
    "Reshel",
    "CourageTw",
    "StrengthTw",
    "AngerTw",
    "TheEdge",
    "ElsydeonCave",
    "Passageway",
    "CarnivorousTrees",
    "DarkForce1",
];
const ART_SYMBOLS: [&str; COUNT] = [
    "MotaDesert",
    "MotaBeach",
    "MotaGrass",
    "MotaSea",
    "Dezo",
    "Dezo",
    "PowerPlants",
    "PowerPlants",
    "PowerPlants",
    "PowerPlants",
    "PowerPlants",
    "PowerPlants",
    "PowerPlants",
    "AcademyBasement",
    "LadeaTower",
    "ZioFortAirCastle",
    "GaruberkTw",
    "TonoeBasement",
    "ZioFortAirCastle",
    "MonsenCavePassageway",
    "IslandAndElsydeonCave",
    "Rykros",
    "Zema",
    "Reshel",
    "RykrosTowers",
    "RykrosTowers",
    "RykrosTowers",
    "TheEdge",
    "IslandAndElsydeonCave",
    "MonsenCavePassageway",
    "CarnivorousTrees",
    "DarkForce1",
];

/// `Battle_SetupBackground`, and its `Event_Battle_Index` test.
const SETUP: usize = 0x006BFC;
const SETUP_SIGNATURE: [u8; 12] = [
    0x70, 0x00, 0x10, 0x39, 0x00, 0xff, 0xec, 0xfc, 0x6b, 0x0a, 0x41, 0xfa,
];
const EVENT_INDEXES: usize = 0x006C8C;
const EVENT_COUNT: usize = 27;
const FIELD_INDEXES: usize = 0x006CA8;
const FIELD_COUNT: usize = 416;
const MOTA_INDEXES: usize = 0x0586EC;
const MOTA_COUNT: usize = 214;
/// `FieldRoutine_Interaction: bra.w`, the end of the Motavia table.
const FIELD_ROUTINE_INTERACTION: usize = 0x0587C2;
const NO_BACKGROUND: u8 = 0xFF;
/// `loc_6C24`: index 4 becomes 5 once `EventFlag_DarkForce2` is set.
const DARK_FORCE_2_SWAP: (u8, u8) = (4, 5);
/// `GetMotaBattleBGIndex` stores index+1 and keeps 0 for index 0.
const MOTA_STORED_BIAS: u8 = 1;

fn refuse<T>(message: String) -> Result<T, PackError> {
    Err(PackError(message))
}

fn hex6(value: usize) -> String {
    format!("0x{value:06X}")
}

fn slice(rom: &[u8], at: usize, len: usize) -> Result<&[u8], PackError> {
    rom.get(at..at + len)
        .ok_or_else(|| PackError(format!("0x{at:06X} (+{len}) runs past the end of the ROM")))
}

/// `_gap_bounds`: from each wanted offset to the next landmark.
fn gap_bounds(landmarks: &[usize], rom_size: usize, wanted: &[usize]) -> BTreeMap<usize, usize> {
    let landmarks: BTreeSet<usize> = landmarks.iter().copied().filter(|&o| o != 0).collect();
    wanted
        .iter()
        .copied()
        .filter(|&o| o != 0)
        .map(|offset| {
            let next = landmarks
                .range(offset + 1..)
                .next()
                .copied()
                .unwrap_or(rom_size);
            (offset, next - offset)
        })
        .collect()
}

/// The selection record and its three tables: event, field map, Motavia.
type Selection = (Json, Vec<u8>, Vec<u8>, Vec<u8>);

/// `background_selection`: the three tables, checked at both ends.
fn selection(rom: &[u8]) -> Result<Selection, PackError> {
    if slice(rom, SETUP, SETUP_SIGNATURE.len())? != SETUP_SIGNATURE {
        return refuse(format!(
            "Battle_SetupBackground at {} does not start with its Event_Battle_Index test",
            hex6(SETUP)
        ));
    }
    if slice(rom, FIELD_ROUTINE_INTERACTION, 2)? != [0x60, 0x00] {
        return refuse(format!(
            "FieldRoutine_Interaction at {}, the end of MotaBattleBGIndexes, does not start \
             with its bra.w",
            hex6(FIELD_ROUTINE_INTERACTION)
        ));
    }
    let event = slice(rom, EVENT_INDEXES, EVENT_COUNT)?.to_vec();
    let field = slice(rom, FIELD_INDEXES, FIELD_COUNT)?.to_vec();
    let mota = slice(rom, MOTA_INDEXES, MOTA_COUNT)?.to_vec();
    let json = Json::object()
        .with("routine", "Battle_SetupBackground")
        .with("rom_offset", hex6(SETUP))
        .with(
            "order",
            vec!["event_battle", "field_map", "motavia_terrain"],
        )
        .with(
            "event_battle",
            Json::object()
                .with("table", "EventBattleBGIndexes")
                .with("rom_offset", hex6(EVENT_INDEXES))
                .with("count", EVENT_COUNT)
                .with("indexed_by", "Event_Battle_Index, when it is not negative")
                .with("indexes", event.clone()),
        )
        .with(
            "field_map",
            Json::object()
                .with("table", "Battle_BackgroundIndexes")
                .with("rom_offset", hex6(FIELD_INDEXES))
                .with("count", FIELD_COUNT)
                .with("indexed_by", "Field_Map_Index, when it is not zero")
                .with("none", NO_BACKGROUND)
                .with("indexes", field.clone()),
        )
        .with(
            "motavia_terrain",
            Json::object()
                .with("table", "MotaBattleBGIndexes")
                .with("rom_offset", hex6(MOTA_INDEXES))
                .with("count", MOTA_COUNT)
                .with(
                    "indexed_by",
                    "the Motavia overworld chunk id under Character_1 (GetChunkAndCollision), \
                     when Field_Map_Index is 0",
                )
                .with("stored_bias", MOTA_STORED_BIAS)
                .with(
                    "note",
                    "GetMotaBattleBGIndex stores index+1 and keeps 0 for index 0, so a \
                     background number is max(stored - 1, 0).",
                )
                .with("indexes", mota.clone()),
        )
        .with(
            "vehicle_mounted",
            Json::object()
                .with("selector", "Vehicle_Index != 0")
                .with(
                    "event_battle",
                    "event_battle remains first when a scene supplies Event_Battle_Index",
                )
                .with(
                    "mapped_planets",
                    "Battle_BackgroundIndexes[Field_Map_Index]",
                )
                .with(
                    "motavia",
                    "MotaBattleBGIndexes[raw_chunk] when Field_Map_Index is 0",
                )
                .with(
                    "debug_unmapped_fallback",
                    "use the Motavia chunk table only when a mounted debug map has no field-map \
                     background entry",
                )
                .with("source", "Battle_SetupBackground and GetMotaBattleBGIndex"),
        )
        .with(
            "dark_force_2_swap",
            Json::object()
                .with("from", DARK_FORCE_2_SWAP.0)
                .with("to", DARK_FORCE_2_SWAP.1)
                .with("flag", "EventFlag_DarkForce2")
                .with(
                    "note",
                    "loc_6C24 swaps index 4 for 5 once the flag is set; it is the only path \
                     that reaches background 5.",
                ),
        );
    Ok((json, event, field, mota))
}

/// `_reachable`: how often each background is selected, per path.
fn reachable(event: &[u8], field: &[u8], mota: &[u8]) -> BTreeMap<u8, [usize; 3]> {
    let mut counts: BTreeMap<u8, [usize; 3]> = BTreeMap::new();
    for &index in event {
        counts.entry(index).or_default()[0] += 1;
    }
    for &index in field.iter().filter(|&&i| i != NO_BACKGROUND) {
        counts.entry(index).or_default()[1] += 1;
    }
    for &stored in mota {
        counts
            .entry(stored.saturating_sub(MOTA_STORED_BIAS))
            .or_default()[2] += 1;
    }
    counts
}

/// `decompress_art` with an exact documented length: the tiles, the
/// documented size and the header's pattern count.
fn art(
    rom: &[u8],
    offset: usize,
    label: &str,
    size: usize,
) -> Result<(Vec<[u8; 64]>, u16), PackError> {
    let header = nemesis::read_header(rom, offset)?;
    let (data, consumed) = nemesis::decompress(rom, offset)?;
    let declared = usize::from(header.row_count) * 4;
    if data.len() != declared {
        return refuse(format!(
            "{label} at 0x{offset:06X}: decoded {} bytes but the header declares {} patterns \
             ({declared} bytes)",
            data.len(),
            header.tile_count
        ));
    }
    if consumed != size && consumed != size + 1 {
        return refuse(format!(
            "{label} at 0x{offset:06X}: decompressor consumed {consumed} bytes, but the \
             documented stream is {size} bytes (a trailing lookahead byte would allow at most \
             one more)"
        ));
    }
    Ok((gfx::decode_tiles(&data)?, header.tile_count))
}

/// `decode_mapping` with an exact documented length and cell count.
fn mapping(rom: &[u8], offset: usize, label: &str, size: usize) -> Result<Vec<u16>, PackError> {
    let (words, consumed) = enigma::decompress(rom, offset, 0, Some(PLANE_BUFFER_WORDS))?;
    if consumed != size {
        return refuse(format!(
            "{label} at 0x{offset:06X}: decompressor consumed {consumed} bytes, but the stream \
             is documented as {size}"
        ));
    }
    if words.len() != COLUMNS * ROWS {
        return refuse(format!(
            "{label} at 0x{offset:06X}: decoded {} cells, but its consumer copies {}",
            words.len(),
            COLUMNS * ROWS
        ));
    }
    Ok(words)
}

/// `battle_cram`: line 0 is black, the 13 colours, black to 16; three black
/// lines after it.
fn battle_cram(colors: &[[u8; 3]]) -> Vec<[u8; 3]> {
    let mut cram = vec![[0, 0, 0]];
    cram.extend_from_slice(colors);
    cram.resize(64, [0, 0, 0]);
    cram
}

/// `build_backgrounds`: write the PNGs and return the index.
pub fn write(rom: &[u8], root: &Path) -> Result<Json, PackError> {
    let pointer = |i: usize, column: usize| {
        crate::m68k::l(rom, TABLE + i * STRIDE + column * 4).map(|v| v as usize)
    };
    let mut arts = Vec::new();
    let mut maps = Vec::new();
    let mut palettes = Vec::new();
    for i in 0..COUNT {
        arts.push(pointer(i, 0)?);
        maps.push(pointer(i, 1)?);
        palettes.push(pointer(i, 2)?);
    }
    let landmarks: Vec<usize> = arts
        .iter()
        .chain(&maps)
        .copied()
        .chain(EXTRA_LANDMARKS)
        .collect();
    let map_bounds = gap_bounds(&landmarks, rom.len(), &maps);
    let art_bounds = gap_bounds(&landmarks, rom.len(), &arts);
    let (selection_json, event, field, mota) = selection(rom)?;
    let reachable = reachable(&event, &field, &mota);

    let mut entries = Vec::new();
    let mut distinct = BTreeSet::new();
    for index in 0..COUNT {
        let (art_at, map_at, palette_at) = (arts[index], maps[index], palettes[index]);
        distinct.insert(art_at);
        let (art_symbol, symbol) = (ART_SYMBOLS[index], PALETTE_SYMBOLS[index]);
        let art_size = art_bounds[&art_at];
        let map_size = map_bounds[&map_at];
        let (tiles, patterns) = art(
            rom,
            art_at,
            &format!("ArtNem_{art_symbol}BattleBG"),
            art_size,
        )?;
        let words = mapping(
            rom,
            map_at,
            &format!("MapEni_{art_symbol}BattleBG"),
            map_size,
        )?;
        let colors = gfx::palette_rgb(&gfx::decode_palette(slice(
            rom,
            palette_at,
            PALETTE_COLORS * 2,
        )?)?);
        let (width, height, pixels) = compose(&words, COLUMNS, &tiles, 0)?;
        let image = png::encode_indexed(
            width as u32,
            height as u32,
            &pixels,
            &battle_cram(&colors),
            &[],
        )?;
        let path = format!("{PNG_DIRECTORY}/{index:02}_{}.png", safe(symbol));
        crate::write_file(root, &path, &image)?;
        let counts = reachable.get(&(index as u8)).copied().unwrap_or_default();
        let selected = Json::object()
            .with("event_battles", counts[0])
            .with("maps", counts[1])
            .with("motavia_terrain", counts[2]);
        entries.push(
            Json::object()
                .with("index", index)
                .with("symbol", symbol)
                .with("art_symbol", art_symbol)
                .with("png", path.as_str())
                .with("png_sha256", png::image_sha256(&image)?)
                .with("width_cells", COLUMNS)
                .with("height_cells", ROWS)
                .with("width_pixels", COLUMNS * 8)
                .with("height_pixels", ROWS * 8)
                .with(
                    "art",
                    Json::object()
                        .with("rom_offset", hex6(art_at))
                        .with("compression", "nemesis")
                        .with("compressed_size", art_size)
                        .with("pattern_count", patterns),
                )
                .with(
                    "mapping",
                    Json::object()
                        .with("rom_offset", hex6(map_at))
                        .with("compression", "enigma")
                        .with("compressed_size", map_size)
                        .with("cell_count", COLUMNS * ROWS)
                        .with("base_tile", 0u8)
                        .with("art_vram_tile", 0u8),
                )
                .with(
                    "palette",
                    Json::object()
                        .with("label", format!("Pal_{symbol}BattleBG"))
                        .with("rom_offset", hex6(palette_at))
                        .with("cram_line", 0u8)
                        .with("first_index", 1u8)
                        .with("color_count", PALETTE_COLORS)
                        .with("index_0_forced_black", true)
                        .with(
                            "colors",
                            colors.iter().map(|c| c.to_vec()).collect::<Vec<_>>(),
                        ),
                )
                .with("selected_by", selected)
                .with(
                    "selectable",
                    counts.iter().sum::<usize>() > 0 || index == usize::from(DARK_FORCE_2_SWAP.1),
                ),
        );
    }
    Ok(Json::object()
        .with("format_version", crate::battle::FORMAT_VERSION)
        .with("kind", "battle_backgrounds")
        .with("count", entries.len())
        .with(
            "table",
            Json::object()
                .with("label", "BattleBGArtPtrs")
                .with("rom_offset", hex6(TABLE))
                .with("entry_size", STRIDE)
                .with("entry_count", COUNT)
                .with("columns", "art, plane mapping, palette"),
        )
        .with("distinct_art_blobs", distinct.len())
        .with("plane_size_cells", vec![COLUMNS, ROWS])
        .with(
            "palette_note",
            "loc_6C3C clears colour 0 and copies 13 words to Palette_Table_Buffer+2, so a \
             background palette is CRAM line 0 indices 1-13 with index 0 forced black. The PNGs \
             bake it: unlike an enemy body, a background has exactly one palette.",
        )
        .with("selection", selection_json)
        .with("backgrounds", entries))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gap_runs_to_the_next_nonzero_landmark() {
        let bounds = gap_bounds(&[0, 10, 30, 20], 100, &[10, 30, 0]);
        assert_eq!(bounds.into_iter().collect::<Vec<_>>(), [(10, 10), (30, 70)]);
    }

    #[test]
    fn motavia_stores_index_plus_one_and_maps_skip_none() {
        let counts = reachable(&[4], &[NO_BACKGROUND, 2], &[0, 1, 3]);
        assert_eq!(counts[&0], [0, 0, 2]);
        assert_eq!(counts[&2], [0, 1, 1]);
        assert_eq!(counts[&4], [1, 0, 0]);
        assert!(!counts.contains_key(&NO_BACKGROUND));
    }

    #[test]
    fn the_cram_is_line_zero_with_index_zero_black() {
        let cram = battle_cram(&[[1, 2, 3]; 13]);
        assert_eq!(cram.len(), 64);
        assert_eq!(
            (cram[0], cram[1], cram[13], cram[14]),
            ([0, 0, 0], [1, 2, 3], [1, 2, 3], [0, 0, 0])
        );
    }
}
