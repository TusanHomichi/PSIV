//! `title/`: the port of `psiv_tools/title_pack.py`, whose docs give the
//! reasons (#216, slice 7e). The Sega logo and the title's art are drawn
//! from their Enigma mappings as the cartridge places them; the background
//! is composed from its three pieces; and every surface is re-encoded at
//! each sampled tape-25 frame against the CRAM words [`boot`] derives from
//! the cartridge's own routines.

mod boot;

use std::collections::BTreeSet;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::decode::enigma;
use crate::json::Json;
use crate::{PackError, Rom, gfx, png};

/// `Pal_TitleScreen`: four CRAM lines.
const PALETTE: usize = 0x1D29BC;
const PALETTE_NAME: &str = "Pal_TitleScreen";
/// `PLANE_BUFFER_WORDS`.
const PLANE_BUFFER_WORDS: usize = 64 * 32;
const SCREEN: (usize, usize) = (320, 224);
const SOURCE: &str = "ROM: GameMode_Sega, DoSegaPaletteCycling, loc_410E2, Pal_FadeIn, \
PalFadeOut_ClrSpriteTbl, TitleRoutine_FadingText, RunTitleFadingText, loc_427DA, \
TitleRoutine_PressStartButton, DoPressStartButtonCyclingPal, loc_42C90 \
(psiv_tools.title_pack.derive_palette_cycle)";
const REPLAY: &str = "indexed title surfaces re-encoded against the derived CRAM words";

/// One `planes.MAPPINGS` entry the title draws.
struct Spec {
    label: &'static str,
    rom_offset: usize,
    compressed_size: usize,
    base_tile: u16,
    columns: usize,
    rows: usize,
    art: (&'static str, usize, usize),
    art_vram_tile: usize,
    source: &'static str,
    note: Option<&'static str>,
}

/// The Nemesis art the title mappings name: label, offset, documented size.
const SEGA_LOGO_ART: (&str, usize, usize) = ("ArtNem_SegaLogo", 266_704, 1004);
const PS_TITLE_ART: (&str, usize, usize) = ("ArtNem_TitlePSTitle", 1_885_306, 2608);
const SUBTITLE_ART: (&str, usize, usize) = ("ArtNem_TitleTheEndOfTheMillennium", 1_887_914, 1020);
const COPYRIGHT_ART: (&str, usize, usize) = ("ArtNem_TitleCopyrightText", 1_889_136, 242);
const PRESS_START_ART: (&str, usize, usize) = ("ArtNem_PressStartButton", 1_888_934, 202);
const BACKGROUND_ART: (&str, usize, usize) = ("ArtNem_TitleBackground", 1_889_378, 7568);

const fn spec(
    label: &'static str,
    (rom_offset, compressed_size): (usize, usize),
    base_tile: u16,
    (columns, rows): (usize, usize),
    art: (&'static str, usize, usize),
    art_vram_tile: usize,
    source: &'static str,
) -> Spec {
    Spec {
        label,
        rom_offset,
        compressed_size,
        base_tile,
        columns,
        rows,
        art,
        art_vram_tile,
        source,
        note: None,
    }
}

const SEGA_LOGO: Spec = Spec {
    note: Some(
        "the loader fills all 64 CRAM entries with $0E00 before fading in, so this screen has \
         no per-line palette to render against",
    ),
    ..spec(
        "MapEni_SegaLogo",
        (0x0415BC, 32),
        0x0010,
        (0x11, 5),
        SEGA_LOGO_ART,
        0x010,
        "GameMode_Sega, ps4.asm:84268 (revision != 0 branch)",
    )
};

/// `TITLE_ART_LABELS`, in order.
const ASSETS: [(&str, Spec); 5] = [
    ("sega_logo", SEGA_LOGO),
    (
        "title_logo",
        spec(
            "MapEni_TitlePSTitle",
            (0x1D20F2, 52),
            0x0010,
            (0x11, 0x0D),
            PS_TITLE_ART,
            0x010,
            "TitleRoutine_FadingText, ps4.asm:86495",
        ),
    ),
    (
        "subtitle",
        spec(
            "MapEni_TitleTheEndOfTheMillennium",
            (0x1D2126, 44),
            0x0092,
            (0x1D, 3),
            SUBTITLE_ART,
            0x092,
            "TitleRoutine_FadingText, ps4.asm:86504 (revision != 0 branch)",
        ),
    ),
    (
        "press_start",
        spec(
            "MapEni_PressStartButton",
            (0x1D2160, 18),
            0x40D8,
            (0x12, 1),
            PRESS_START_ART,
            0x0D8,
            "loc_4295C, ps4.asm:86612 (revision != 0 branch)",
        ),
    ),
    (
        "copyright",
        spec(
            "MapEni_TitleCopyrightText",
            (0x1D2152, 14),
            0x20CA,
            (0x11, 1),
            COPYRIGHT_ART,
            0x0CA,
            "loc_4295C, ps4.asm:86621 (revision != 0 branch)",
        ),
    ),
];

const fn background(
    label: &'static str,
    at: usize,
    size: usize,
    columns: usize,
    line: &'static str,
) -> Spec {
    spec(
        label,
        (at, size),
        0x60E3,
        (columns, 0x1C),
        BACKGROUND_ART,
        0x0E3,
        line,
    )
}

/// `BACKGROUND_PIECES`, with where each lands, and the transfer strip.
const PIECES: [(Spec, usize); 3] = [
    (
        background(
            "MapEni_TitleBGLeftPart",
            0x1D22CA,
            162,
            6,
            "TitleRoutine_FadingText, ps4.asm:86299 (revision != 0 branch)",
        ),
        0,
    ),
    (
        background(
            "MapEni_TitleBarBGBottomPart",
            0x1D223E,
            140,
            8,
            "TitleRoutine_FadingText, ps4.asm:86295 (revision != 0 branch)",
        ),
        48,
    ),
    (
        background(
            "MapEni_TitleBGRightPart",
            0x1D236C,
            594,
            0x1A,
            "TitleRoutine_FadingText, ps4.asm:86303 (revision != 0 branch)",
        ),
        112,
    ),
];
const TRANSFER: Spec = background(
    "MapEni_TitleBarBGTopPart",
    0x1D2172,
    204,
    8,
    "TitleRoutine_FadingText, ps4.asm:86291 (revision != 0 branch)",
);

fn hex6(value: usize) -> String {
    format!("0x{value:06X}")
}

fn sha256(bytes: &[u8]) -> String {
    png::hex(&Sha256::digest(bytes))
}

fn rgb_lists(colours: &[[u8; 3]]) -> Json {
    Json::from(colours.iter().map(|c| c.to_vec()).collect::<Vec<_>>())
}

/// `decode_mapping`: the words, checked to their documented size and the
/// consumer's cell count, and the record `planes.py` keeps for them.
fn decode_mapping(rom: &[u8], spec: &Spec) -> Result<(Vec<u16>, Json), PackError> {
    let (offset, label) = (spec.rom_offset, spec.label);
    let header = enigma::read_header(rom, offset, spec.base_tile)?;
    let (words, consumed) =
        enigma::decompress(rom, offset, spec.base_tile, Some(PLANE_BUFFER_WORDS))?;
    if consumed != spec.compressed_size {
        return Err(PackError(format!(
            "{label} at 0x{offset:06X}: decompressor consumed {consumed} bytes, but the stream \
             is documented as {}",
            spec.compressed_size
        )));
    }
    let expected = spec.columns * spec.rows;
    if words.len() != expected {
        return Err(PackError(format!(
            "{label} at 0x{offset:06X}: decoded {} cells, but its consumer copies {expected}",
            words.len()
        )));
    }
    let compressed = rom
        .get(offset..offset + spec.compressed_size)
        .ok_or_else(|| PackError(format!("{label} runs past the end of the ROM")))?;
    let tiles: Vec<u16> = words.iter().map(|w| w & 0x07FF).collect();
    let lines: BTreeSet<u16> = words.iter().map(|w| (w >> 13) & 3).collect();
    let distinct: BTreeSet<u16> = words.iter().copied().collect();
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_be_bytes()).collect();
    let count = |mask: u16| words.iter().filter(|&&w| w & mask != 0).count();
    let record = Json::object()
        .with("label", label)
        .with("rom_offset", hex6(offset))
        .with("compression", "enigma")
        .with("compressed_size", spec.compressed_size)
        .with("rom_end_exclusive", hex6(offset + spec.compressed_size))
        .with("compressed_sha256", sha256(compressed))
        .with("base_tile", format!("0x{:04X}", spec.base_tile))
        .with("inline_bits", header.inline_bits)
        .with(
            "flag_bits",
            Json::object()
                .with("raw", format!("0x{:02X}", header.flags))
                .with("v_flip", header.flags & 0x02 != 0)
                .with("h_flip", header.flags & 0x01 != 0),
        )
        .with(
            "incremental_word",
            format!("0x{:04X}", header.incremental_word),
        )
        .with("literal_word", format!("0x{:04X}", header.literal_word))
        .with("cell_count", words.len())
        .with("distinct_words", distinct.len())
        .with(
            "tile_range",
            vec![
                tiles.iter().copied().min().unwrap_or(0),
                tiles.iter().copied().max().unwrap_or(0),
            ],
        )
        .with("palette_lines", lines.into_iter().collect::<Vec<_>>())
        .with("cells_h_flipped", count(0x0800))
        .with("cells_v_flipped", count(0x1000))
        .with("cells_high_priority", count(0x8000))
        .with("decoded_sha256", sha256(&bytes));
    Ok((words, record))
}

/// `art_tiles` for a Nemesis blob: the patterns and `decompress_art`'s
/// record.
fn art_tiles(
    rom: &[u8],
    (label, offset, size): (&str, usize, usize),
) -> Result<(Vec<[u8; 64]>, Json), PackError> {
    let art = gfx::decompress_art(rom, offset, label, Some(size), None)?;
    let record = Json::object()
        .with("label", label)
        .with("rom_offset", hex6(offset))
        .with("compression", "nemesis")
        .with("compressed_size", art.compressed_size)
        .with("rom_end_exclusive", hex6(offset + art.compressed_size))
        .with("consumed", art.consumed)
        .with(
            "trailing_lookahead_byte",
            art.consumed == art.compressed_size + 1,
        )
        .with("tile_count", art.tile_count)
        .with("xor_mode", art.xor_mode)
        .with("header_word", format!("0x{:04X}", art.header_word))
        .with("decompressed_size", art.data.len())
        .with("decompressed_sha256", art.decompressed_sha256.as_str())
        .with("compressed_sha256", art.compressed_sha256.as_str());
    Ok((gfx::decode_tiles(&art.data)?, record))
}

/// A drawn surface: its indexed pixels, kept for the replay frames.
struct Surface {
    file: String,
    width: usize,
    height: usize,
    pixels: Vec<u8>,
    transparent: bool,
}

/// The mapping record with the asset's own fields laid over it.
fn mapping_json(spec: &Spec, mut mapping: Json, tiles: usize) -> Json {
    let first = spec.art_vram_tile;
    let spans = matches!(
        mapping.get("tile_range"),
        Some(Json::Array(range)) if range.as_slice() == [Json::from(first), Json::from(first + tiles - 1)]
    );
    mapping.insert("source", spec.source);
    mapping.insert("columns", spec.columns);
    mapping.insert("rows", spec.rows);
    mapping.insert("art_label", spec.art.0);
    mapping.insert("art_vram_tile", format!("0x{first:03X}"));
    mapping.insert("art_tile_count", tiles);
    mapping.insert("spans_art_exactly", spans);
    if let Some(note) = spec.note {
        mapping.insert("note", note);
    }
    mapping
}

/// `_decode_mapping_asset`: one drawn mapping, written as `name`.
fn asset(
    rom: &[u8],
    root: &Path,
    spec: &Spec,
    cram: &[[u8; 3]],
    name: &str,
    transparent: bool,
) -> Result<(Json, Surface), PackError> {
    let (words, mapping) = decode_mapping(rom, spec)?;
    let (tiles, art) = art_tiles(rom, spec.art)?;
    let (width, height, pixels) = gfx::compose(&words, spec.columns, &tiles, spec.art_vram_tile)?;
    let image = png::encode_indexed(
        width as u32,
        height as u32,
        &pixels,
        cram,
        if transparent { &[0] } else { &[] },
    )?;
    let path = format!("title/{name}");
    crate::write_file(root, &path, &image)?;
    let json = Json::object()
        .with("png", path.as_str())
        .with("png_sha256", png::image_sha256(&image)?)
        .with("size_pixels", vec![spec.columns * 8, spec.rows * 8])
        .with("mapping", mapping_json(spec, mapping, tiles.len()))
        .with("art", art);
    Ok((
        json,
        Surface {
            file: name.to_owned(),
            width,
            height,
            pixels,
            transparent,
        },
    ))
}

/// `_background`: the three pieces on one 320x224 canvas, each piece on
/// its own, and the transfer strip.
fn background_surfaces(
    rom: &[u8],
    root: &Path,
    cram: &[[u8; 3]],
) -> Result<(Json, Json, Json, Surface, Surface), PackError> {
    let (screen_w, screen_h) = SCREEN;
    let mut canvas = vec![0u8; screen_w * screen_h];
    let mut pieces = Json::object();
    let mut placements = Vec::new();
    for (spec, x) in &PIECES {
        let (words, mapping) = decode_mapping(rom, spec)?;
        let (tiles, art) = art_tiles(rom, spec.art)?;
        let (width, height, pixels) =
            gfx::compose(&words, spec.columns, &tiles, spec.art_vram_tile)?;
        if height != screen_h || x + width > screen_w {
            return Err(PackError(format!(
                "title background piece {} is {width}x{height} at x={x}, not a slice of the \
                 320x224 frame",
                spec.label
            )));
        }
        for row in 0..height {
            canvas[row * screen_w + x..row * screen_w + x + width]
                .copy_from_slice(&pixels[row * width..(row + 1) * width]);
        }
        let key = spec.label.trim_start_matches("MapEni_");
        let path = format!("title/{}.png", key.to_lowercase());
        let image = png::encode_indexed(width as u32, height as u32, &pixels, cram, &[])?;
        crate::write_file(root, &path, &image)?;
        pieces.insert(
            key,
            Json::object()
                .with("png", path.as_str())
                .with("png_sha256", png::image_sha256(&image)?)
                .with("size_pixels", vec![width, height])
                .with("mapping", mapping_json(spec, mapping, tiles.len()))
                .with("art", art),
        );
        placements.push(
            Json::object()
                .with("piece", key)
                .with("x_pixels", *x)
                .with("y_pixels", 0u8)
                .with("width_pixels", width)
                .with("height_pixels", height),
        );
    }
    let (transfer, transfer_surface) =
        asset(rom, root, &TRANSFER, cram, "titlebarbgtoppart.png", false)?;
    let image = png::encode_indexed(screen_w as u32, screen_h as u32, &canvas, cram, &[])?;
    crate::write_file(root, "title/background.png", &image)?;
    let background = Json::object()
        .with("png", "title/background.png")
        .with("png_sha256", png::image_sha256(&image)?)
        .with("size_pixels", vec![screen_w, screen_h])
        .with("placements", placements)
        .with(
            "source",
            "plane_b at title frame 401: left + bottom + right",
        );
    let surface = Surface {
        file: "background.png".into(),
        width: screen_w,
        height: screen_h,
        pixels: canvas,
        transparent: false,
    };
    Ok((background, pieces, transfer, surface, transfer_surface))
}

/// `_emit_replay_variants`: every surface re-encoded at each sampled frame.
fn replay(
    root: &Path,
    cycle: Json,
    frames: &[(u32, [u16; 64])],
    surfaces: &[&Surface],
) -> Result<Json, PackError> {
    let mut summary = Json::object();
    for (frame, words) in frames {
        let raw: Vec<u8> = words.iter().flat_map(|w| w.to_be_bytes()).collect();
        let colours = gfx::palette_rgb(&gfx::decode_palette(&raw)?);
        let directory = format!("title/replay/frame_{frame}");
        for surface in surfaces {
            let image = png::encode_indexed(
                surface.width as u32,
                surface.height as u32,
                &surface.pixels,
                &colours,
                if surface.transparent { &[0] } else { &[] },
            )?;
            crate::write_file(root, &format!("{directory}/{}", surface.file), &image)?;
        }
        let changed = cycle
            .get("frames")
            .and_then(|f| f.get(&frame.to_string()))
            .and_then(|f| f.get("changed_entries_from_previous"))
            .cloned()
            .unwrap_or(Json::Null);
        summary.insert(
            frame.to_string(),
            Json::object()
                .with("changed_entries_from_previous", changed)
                .with("directory", directory),
        );
    }
    let mut payload = cycle;
    payload.insert("frames", summary);
    payload.insert("source", SOURCE);
    payload.insert("replay", REPLAY);
    let sha = crate::write_json(root, "title/palette_cycle.json", &payload)?;
    Ok(Json::object()
        .with("path", "title/palette_cycle.json")
        .with("sha256", sha)
        .with("frames", frames.iter().map(|(f, _)| *f).collect::<Vec<_>>())
        .with("surface_count", surfaces.len()))
}

fn cell(x: u8, y: u8, w: u8, h: u8) -> Json {
    Json::object()
        .with("x_cell", x)
        .with("y_cell", y)
        .with("width_cells", w)
        .with("height_cells", h)
}

/// `emit_title`.
pub fn build(rom: &Rom, root: &Path) -> Result<(), PackError> {
    let data = rom.bytes();
    let raw = data
        .get(PALETTE..PALETTE + 128)
        .ok_or_else(|| PackError(format!("{PALETTE_NAME} runs past the end of the ROM")))?;
    let cram = gfx::palette_rgb(&gfx::decode_palette(raw)?);
    let mut assets = Json::object();
    let mut surfaces = Vec::new();
    for (name, spec) in &ASSETS {
        let (json, surface) = asset(data, root, spec, &cram, &format!("{name}.png"), true)?;
        assets.insert(*name, json);
        surfaces.push(surface);
    }
    let (background, pieces, transfer, background_surface, transfer_surface) =
        background_surfaces(data, root, &cram)?;
    let (cycle, frames) = boot::derive(data)?;
    let mut order: Vec<&Surface> = vec![&background_surface, &transfer_surface];
    order.extend(surfaces.iter());
    let replay = replay(root, cycle, &frames, &order)?;
    let layout = Json::object()
        .with("format_version", 1u8)
        .with("kind", "psiv_title_pack")
        .with(
            "screen",
            Json::object()
                .with("width_pixels", SCREEN.0)
                .with("height_pixels", SCREEN.1),
        )
        .with(
            "palette",
            Json::object().with("name", PALETTE_NAME).with(
                "cram_lines",
                (0..4)
                    .map(|line| rgb_lists(&cram[line * 16..(line + 1) * 16]))
                    .collect::<Vec<_>>(),
            ),
        )
        .with("assets", assets)
        .with("background", background)
        .with("background_pieces", pieces)
        .with("background_transfer", transfer)
        .with("palette_cycle", replay)
        .with(
            "placement_contract",
            Json::object()
                .with("sega_logo", cell(12, 11, 17, 5))
                .with("title_logo", cell(11, 3, 17, 13))
                .with("subtitle", cell(5, 17, 29, 3))
                .with("press_start", cell(11, 22, 18, 1))
                .with("copyright", cell(12, 25, 17, 1)),
        );
    crate::write_json(root, "title/layout.json", &layout).map(drop)
}
