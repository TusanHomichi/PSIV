//! The box the text sits in: the port of `psiv_tools/dialogue_pack/chrome.py`.
//! `loc_68704` draws every window out of three tiles of `ArtNem_WindowTiles`
//! and their flip bits; `WinGroup_Dialogue` and `WinGroup_Menu` are the
//! cartridge's own window records. That module's docs give the reasons.

use std::path::Path;

use super::{DIALOGUE_CRAM_LINE, FORMAT_VERSION, PAL_INIT_LINE_3, colours, refuse, slice};
use crate::json::Json;
use crate::{PackError, gfx, png};

/// `ArtNem_WindowTiles` and the VRAM tile `Title_ArtPtrs` loads it at.
const ART: usize = 0x2A2B7E;
const ART_SIZE: usize = 1212;
const ART_TILES: usize = 128;
const ART_VRAM_TILE: u16 = 0x680;
const MENU_FONT_TILES: usize = 87;
const TILE_PIXELS: usize = 8;
const BACKGROUND_COLOR: u8 = 0xE;

/// The nine words `loc_68704` writes, in order.
const FRAME_WORDS: [(&str, u16); 9] = [
    ("corner_top_left", 0xC6E9),
    ("edge_top", 0xC6EA),
    ("corner_top_right", 0xCEE9),
    ("edge_left", 0xC6F3),
    ("fill", 0xC680),
    ("edge_right", 0xCEF3),
    ("corner_bottom_left", 0xD6E9),
    ("edge_bottom", 0xD6EA),
    ("corner_bottom_right", 0xDEE9),
];
const BORDER_CELLS: usize = 1;

/// `WinGroup_Dialogue`, `WinGroupOffsets` and the entries naming the groups.
const WIN_GROUP_DIALOGUE: usize = 0x069380;
const RECORD_SIZE: usize = 8;
const DIALOGUE_NAMES: [&str; 4] = ["controls", "dialogue", "portrait", "yes_no"];
const WIN_GROUP_OFFSETS: usize = 0x06899C;
const MENU_ENTRY: usize = 1;
const DIALOGUE_ENTRY: usize = 2;
const DIALOGUE_WINDOW: usize = 1;
const PORTRAIT_WINDOW: usize = 2;
const OPEN_STEP_CELLS: usize = 2;

/// `TextBufferToPlane`'s rectangle and `RunText`'s metrics.
const TEXT_X: usize = 4;
const TEXT_Y: usize = 0x15;
const TEXT_WIDTH: usize = 32;
const TEXT_HEIGHT: usize = 4;
const CHARS_PER_LINE: usize = 32;
const LINES_PER_WINDOW: usize = 2;
const GLYPH_WIDTH: usize = 8;
const GLYPH_HEIGHT: usize = 16;

fn hex6(value: usize) -> String {
    format!("0x{value:06X}")
}

/// `pattern_word`.
fn pattern_word(word: u16) -> Json {
    Json::object()
        .with("word", format!("0x{word:04X}"))
        .with("priority", word & 0x8000 != 0)
        .with("cram_line", (word >> 13) & 3)
        .with("flip_v", word & 0x1000 != 0)
        .with("flip_h", word & 0x0800 != 0)
        .with("vram_tile", word & 0x07FF)
}

/// `frame_roles`: each word as a tile of `window.png` and its flips.
fn frame_roles() -> Result<Json, PackError> {
    let mut roles = Json::object();
    for (name, word) in FRAME_WORDS {
        let vram = word & 0x07FF;
        let tile = usize::from(vram).checked_sub(usize::from(ART_VRAM_TILE));
        let Some(tile) = tile.filter(|&t| t < ART_TILES) else {
            return refuse(format!(
                "window frame role {name} names VRAM tile 0x{vram:03X}, outside the {ART_TILES} \
                 patterns loaded at 0x{ART_VRAM_TILE:03X}"
            ));
        };
        let line = (word >> 13) & 3;
        if line != u16::from(DIALOGUE_CRAM_LINE) {
            return refuse(format!(
                "window frame role {name} draws in CRAM line {line}, not the dialogue line \
                 {DIALOGUE_CRAM_LINE}"
            ));
        }
        let mut role = pattern_word(word);
        role.insert("tile", tile);
        role.insert("x", tile * TILE_PIXELS);
        role.insert("y", 0u8);
        role.insert("width", TILE_PIXELS);
        role.insert("height", TILE_PIXELS);
        roles.insert(name, role);
    }
    Ok(roles)
}

fn rect(x: usize, y: usize, width: usize, height: usize) -> Json {
    Json::object()
        .with("x", x * TILE_PIXELS)
        .with("y", y * TILE_PIXELS)
        .with("width", width * TILE_PIXELS)
        .with("height", height * TILE_PIXELS)
}

fn interior(x: usize, y: usize, width: usize, height: usize) -> Json {
    Json::object()
        .with("x_cell", x + BORDER_CELLS)
        .with("y_cell", y + BORDER_CELLS)
        .with("width_cells", width - 2 * BORDER_CELLS)
        .with("height_cells", height - 2 * BORDER_CELLS)
}

/// `window_records`: `WinGroup_Dialogue`'s four windows.
fn window_records(rom: &[u8]) -> Result<Vec<(Json, [usize; 4])>, PackError> {
    DIALOGUE_NAMES
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let at = WIN_GROUP_DIALOGUE + index * RECORD_SIZE;
            let raw = slice(rom, at, RECORD_SIZE, "WinGroup_Dialogue")?;
            let (width, height, x, y) = (
                usize::from(raw[0]) + 1,
                usize::from(raw[1]) + 1,
                usize::from(raw[2]),
                usize::from(raw[3]),
            );
            if width <= 2 || height <= 2 {
                return refuse(format!(
                    "window {index} ({name}) is {width}x{height} cells, which has no room for a \
                     border and an interior"
                ));
            }
            let pointer = u32::from_be_bytes([raw[4], raw[5], raw[6], raw[7]]) as usize;
            Ok((
                Json::object()
                    .with("index", index)
                    .with("name", *name)
                    .with("record_offset", hex6(at))
                    .with("record_hex", crate::png::hex(raw))
                    .with("width_cells", width)
                    .with("height_cells", height)
                    .with("x_cell", x)
                    .with("y_cell", y)
                    .with("rect", rect(x, y, width, height))
                    .with("interior", interior(x, y, width, height))
                    .with("content_pointer", hex6(pointer)),
                [x, y, width, height],
            ))
        })
        .collect()
}

fn group_offset(rom: &[u8], entry: usize) -> Result<usize, PackError> {
    let raw = slice(rom, WIN_GROUP_OFFSETS + 2 * entry, 2, "WinGroupOffsets")?;
    Ok(WIN_GROUP_OFFSETS + usize::from(u16::from_be_bytes([raw[0], raw[1]])))
}

/// `menu_window_records`: `WinGroup_Menu`, up to `WinGroup_Dialogue`.
fn menu_records(rom: &[u8]) -> Result<Vec<Json>, PackError> {
    let start = group_offset(rom, MENU_ENTRY)?;
    let end = group_offset(rom, DIALOGUE_ENTRY)?;
    if end != WIN_GROUP_DIALOGUE {
        return refuse(format!(
            "WinGroupOffsets names WinGroup_Dialogue at 0x{end:06X}, not \
             0x{WIN_GROUP_DIALOGUE:06X}"
        ));
    }
    if start >= end || !(end - start).is_multiple_of(RECORD_SIZE) {
        return refuse(format!(
            "WinGroup_Menu at 0x{start:06X} is not a whole number of records before \
             WinGroup_Dialogue"
        ));
    }
    (0..(end - start) / RECORD_SIZE)
        .map(|index| {
            let raw = slice(
                rom,
                start + index * RECORD_SIZE,
                RECORD_SIZE,
                "WinGroup_Menu",
            )?;
            Ok(Json::object()
                .with("index", index)
                .with("width_cells", u16::from(raw[0]) + 1)
                .with("height_cells", u16::from(raw[1]) + 1)
                .with("x_cell", raw[2])
                .with("y_cell", raw[3]))
        })
        .collect()
}

/// `emit_window`: `window.png` and `window.json`, with the waiting arrow's
/// record.
pub fn write(
    rom: &[u8],
    root: &Path,
    palette: &[[u8; 3]],
    scroll_arrow: &Json,
) -> Result<(), PackError> {
    let art = gfx::decompress_art(rom, ART, "ArtNem_WindowTiles", Some(ART_SIZE), None)?;
    let tiles = gfx::decode_tiles(&art.data)?;
    if tiles.len() != ART_TILES {
        return refuse(format!(
            "ArtNem_WindowTiles decodes to {} patterns, not {ART_TILES}",
            tiles.len()
        ));
    }
    let (width, height, pixels) = gfx::compose_sheet(&tiles, tiles.len())?;
    let image = png::encode_indexed(width as u32, height as u32, &pixels, palette, &[])?;
    crate::write_file(root, "dialogue/window.png", &image)?;

    let records = window_records(rom)?;
    let (_, [bx, by, bw, bh]) = &records[DIALOGUE_WINDOW];
    let box_interior = interior(*bx, *by, *bw, *bh);
    // The window's interior and TextBufferToPlane's rectangle are two
    // unrelated pieces of the cartridge describing the same area.
    let found = (
        bx + BORDER_CELLS,
        by + BORDER_CELLS,
        bw - 2 * BORDER_CELLS,
        bh - 2 * BORDER_CELLS,
    );
    let expected = (TEXT_X, TEXT_Y, TEXT_WIDTH, TEXT_HEIGHT);
    if found != expected {
        let cells =
            |(x, y, w, h): (usize, usize, usize, usize)| format!("{w}x{h} cells at ({x}, {y})");
        return refuse(format!(
            "WinGroup_Dialogue record {DIALOGUE_WINDOW} at {} has an interior of {}, but \
             TextBufferToPlane writes {}",
            hex6(WIN_GROUP_DIALOGUE + DIALOGUE_WINDOW * RECORD_SIZE),
            cells(found),
            cells(expected)
        ));
    }
    let [px, py, pw, ph] = records[PORTRAIT_WINDOW].1;
    let document = Json::object()
        .with("format_version", FORMAT_VERSION)
        .with("kind", "dialogue_window")
        .with("png", "dialogue/window.png")
        .with("png_sha256", png::image_sha256(&image)?)
        .with(
            "source",
            Json::object()
                .with("label", "ArtNem_WindowTiles")
                .with("rom_offset", hex6(ART))
                .with("compression", "nemesis")
                .with("compressed_size", art.compressed_size)
                .with("tile_count", art.tile_count)
                .with("decompressed_sha256", art.decompressed_sha256.as_str())
                .with("vram_tile", format!("0x{ART_VRAM_TILE:03X}"))
                .with("loader", "Title_ArtPtrs")
                .with(
                    "note",
                    format!(
                        "Title_ArtPtrs loads ArtNem_Font at $681 as well, so the menu font's \
                         {MENU_FONT_TILES} patterns cover indices 1..{MENU_FONT_TILES} of this \
                         blob for the whole game. The frame is untouched because it lives at \
                         either end: the fill at 0 and the border at 105..115."
                    ),
                ),
        )
        .with(
            "tile",
            Json::object()
                .with("width", TILE_PIXELS)
                .with("height", TILE_PIXELS)
                .with("count", art.tile_count)
                .with("note", "cell N of the strip is blob tile N, so x = N * 8"),
        )
        .with(
            "palette",
            Json::object()
                .with("source", "Pal_Init_Line_3")
                .with("rom_offset", hex6(PAL_INIT_LINE_3))
                .with("cram_line", DIALOGUE_CRAM_LINE)
                .with("colors", colours(palette))
                .with("fill_index", BACKGROUND_COLOR)
                .with("transparent_index", Json::Null)
                .with(
                    "note",
                    "No pixel of the window art is index 0, so nothing in it is transparent; \
                     the interior is a solid tile of index $E, the same colour \
                     FillTextBackground paints the text area with.",
                ),
        )
        .with("roles", frame_roles()?)
        .with(
            "geometry",
            Json::object()
                .with("border_cells", BORDER_CELLS)
                .with("cell_pixels", TILE_PIXELS)
                .with("shadow", false)
                .with(
                    "rule",
                    "A window of W x H cells is: corner_top_left, W-2 edge_top, \
                     corner_top_right; then H-2 rows of edge_left, W-2 fill, edge_right; then \
                     corner_bottom_left, W-2 edge_bottom, corner_bottom_right. Nothing else is \
                     drawn.",
                )
                .with(
                    "record_encoding",
                    "WinGroup records store width-1 and height-1; Window_Draw adds one to each \
                     before drawing.",
                )
                .with(
                    "open_animation",
                    Json::object()
                        .with("routine", "loc_68690")
                        .with("axis", "horizontal")
                        .with("from", "center")
                        .with("step_cells", OPEN_STEP_CELLS)
                        .with("frames_per_step", 1u8)
                        .with(
                            "note",
                            "The frame is redrawn two cells wider from the centre each step, \
                             with one DMA between steps, unless Window_Render_Mode bit 0 is set \
                             -- TextCtrlCode_Portrait sets it, so portrait windows appear \
                             instantly and the message box animates.",
                        ),
                ),
        )
        .with(
            "group",
            Json::object()
                .with("label", "WinGroup_Dialogue")
                .with("rom_offset", hex6(WIN_GROUP_DIALOGUE))
                .with("record_size", RECORD_SIZE)
                .with("selector", "LoadWindowGroup, by Game_Mode_Routine"),
        )
        .with(
            "windows",
            records.iter().map(|(j, _)| j.clone()).collect::<Vec<_>>(),
        )
        .with(
            "menu_group",
            Json::object()
                .with("label", "WinGroup_Menu")
                .with("rom_offset", hex6(group_offset(rom, MENU_ENTRY)?))
                .with("selector", "WinGroupOffsets entry 1"),
        )
        .with("menu_windows", menu_records(rom)?)
        .with(
            "text_window",
            Json::object()
                .with("window", DIALOGUE_WINDOW)
                .with("rect", rect(*bx, *by, *bw, *bh))
                .with("interior", box_interior)
                .with("chars_per_line", CHARS_PER_LINE)
                .with("lines_per_window", LINES_PER_WINDOW)
                .with("glyph_width", GLYPH_WIDTH)
                .with("glyph_height", GLYPH_HEIGHT)
                .with(
                    "note",
                    "The interior is the text area exactly: 32 glyphs of 8 pixels across, two \
                     lines of 16 pixels down.",
                ),
        )
        .with(
            "portrait_window",
            Json::object()
                .with("window", PORTRAIT_WINDOW)
                .with("rect", rect(px, py, pw, ph))
                .with(
                    "note",
                    "The portrait's 6x6 plane map is drawn at the same cell as this window and \
                     is the same size, so the art covers the frame completely -- portrait PNGs \
                     carry their own border. The window is created for the backup and restore \
                     Window_Destroy needs, not for the picture.",
                ),
        )
        .with("scroll_arrow", scroll_arrow.clone());
    crate::write_json(root, "dialogue/window.json", &document).map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pattern_word_splits_into_its_fields() {
        let word = serde_json::Value::from(&pattern_word(0xDEE9));
        assert_eq!(word["priority"], true);
        assert_eq!(word["cram_line"], 2);
        assert_eq!(
            (word["flip_v"].clone(), word["flip_h"].clone()),
            (true.into(), true.into())
        );
        assert_eq!(word["vram_tile"], 0x6E9);
    }

    #[test]
    fn the_frame_roles_are_tiles_of_the_window_strip() {
        let roles = serde_json::Value::from(&frame_roles().unwrap());
        assert_eq!(roles["fill"]["tile"], 0);
        assert_eq!(roles["corner_top_left"]["tile"], 0x69);
        assert_eq!(roles["corner_top_left"]["x"], 0x69 * 8);
    }
}
