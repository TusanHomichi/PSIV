//! `dialogue/`: the port of `psiv_tools/dialogue_pack/`, whose package docs
//! walk the proof of every number (#216, slice 7b). This part writes the
//! dialogue font and its strip, the menu font, the waiting arrow, the window
//! chrome, the portraits and the trees.
//!
//! Nothing is written until every routine the package transcribes is the
//! retail one (`check_signatures`). The opcode runs are pinned here by
//! length and SHA-256, not by their bytes, so the source carries no cartridge
//! content (#461).

mod art;
mod census;
mod chrome;
mod trees;

use std::collections::BTreeSet;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::json::Json;
use crate::{PackError, Rom, gfx};

/// `DIALOGUE_FORMAT_VERSION`, which tracks the pack's.
const FORMAT_VERSION: u8 = 2;

/// `Pal_Init_Line_3`, and line 2 of `Pal_Init`, which it must copy.
const PAL_INIT_LINE_3: usize = 0x296300;
const PAL_INIT: usize = 0x09F2BC;
const PAL_INIT_MIRRORED_LINE: usize = 2;
const PALETTE_LINE_SIZE: usize = 32;
/// `TextBufferPlaneMaps`: palette bits %10.
const DIALOGUE_CRAM_LINE: u8 = 2;

/// `SIGNATURES`: (name, offset, length, SHA-256, what the run proves).
const SIGNATURES: [(&str, usize, usize, &str, &str); 24] = [
    (
        "chars_per_line",
        0x06A0CE,
        8,
        "e3ed5f81c926ccab19b96f2cba6c337335aeb2988fabaad5831f27d0458ae0f6",
        "RunText_CharacterLoop: addq.w #1,d1 / andi.w #$1F,d1 / bne",
    ),
    (
        "glyph_colors",
        0x06A938,
        12,
        "fc8c6e401057418b7719ed5a94ec7f90e68f57326e307aeb5f2a06e1ff83b0d2",
        "ParseText: move.w #$F,d0 (text) / #$E,d1 (background) / #$F,d7 (16 rows)",
    ),
    (
        "glyph_stride",
        0x06A8F8,
        26,
        "2b0600cc095e857ad50e85f28f8931d27c505c35fa39ec47c222aefae09418b5",
        "GetFontGraphics: andi.w #$7F,d1 / lsl.w #4,d1 / lea (Art_DialogueFont).l,a1",
    ),
    (
        "interaction_event",
        0x058BBC,
        10,
        "f1ef9be63f83e0c9661b55682457e9cfb3a0a8fac771fd34c4f3f76982a8ee40",
        "Interaction_ProcessDialogueTree: $F6 hands off to GetEventFromDialogue",
    ),
    (
        "interaction_facing",
        0x058BC6,
        14,
        "9f42e8433e35bc6b4a057e9f001bdbc7b263d9708f5dbdd5fbab3c65d69e2fd9",
        "Interaction_ProcessDialogueTree: $F3 skips Interaction_UpdateObj",
    ),
    (
        "interaction_preamble",
        0x058B9A,
        22,
        "bd3ff6c11039c9d993649aff35d7887c85b20a58977dc039f68c637371d85c18",
        "Interaction_ProcessDialogueTree: the $FA loop, with no $FB branch",
    ),
    (
        "lines_per_window",
        0x06A0E6,
        16,
        "196390ea67359672fb813a5fc3cd69d5420ba04c4e7e734101a1876571c2dbb4",
        "RunText_CharacterLoop: swallow a following $FC, then andi.w #1,d2 / beq",
    ),
    (
        "null_is_rts",
        0x06A162,
        4,
        "7253dc34e10ccc9570456553e51e929f7c78f23c05afac503ce7f642bf3d3ef8",
        "TextCtrlCodesJmpTbl entry $FB branches to TextCtrlCode_Null at 0x06A176",
    ),
    (
        "portrait_plane_map",
        0x06A27A,
        12,
        "e7497b5bde14b49388c5a5c5330a1eb1a3befc3de569e63831bde0b90737c181",
        "TextCtrlCode_Portrait: lea (0x2A2B36).l,a0 / 6 x 6 tiles",
    ),
    (
        "portrait_position",
        0x06A238,
        8,
        "2b3032e01a531343f5e4a542ab4a6dc088a4467f88053874dc48586c51be0c3b",
        "TextCtrlCode_Portrait: move.w #5,d3 / move.w #$D,d4",
    ),
    (
        "portrait_table",
        0x06A1BA,
        6,
        "91313a39fc51fe29119fd6f849d4922bc67caee0e87f58039b3367d9a532fc41",
        "TextCtrlCode_Portrait: lea (DialoguePortraitArtPtrs).l,a3",
    ),
    (
        "portrait_talk_slot",
        0x06A26A,
        10,
        "4b822094885a7c3a7612223fdb4e5c26235c5e9f177891ef5daa5d47d0040fa1",
        "TextCtrlCode_Portrait: mulu.w #$C,d5 / add.w d5,d3 in Talk mode",
    ),
    (
        "portrait_vram_tile",
        0x06A1CC,
        4,
        "2732521c4f27af857641a439fc2dc8c210962b3ce9ef3944cddda92796cb2bb1",
        "TextCtrlCode_Portrait: move.w #$55C,d0 before NemDecomp",
    ),
    (
        "scroll_arrow",
        0x06A400,
        16,
        "cd6a582c4d7e10240ea59eeaa78b582e1dfa2c92b1835ded1041fda8c7c5e14c",
        "TextCtrlCode_Interrupt: Text_Scroll_Arrow at ($188, $14A)",
    ),
    (
        "system_messages",
        0x058CE8,
        26,
        "007092cfb46725cae93b7ba98770d712bb249d7f67d29a7dc6e5d3d05d8955e6",
        "Interaction_DoPlayerNothingMsg: Current_Party_Slots indexes WinTiles_PlayerNothingMsg through GetOffsetByID, then RunText",
    ),
    (
        "window_art_vram_tile",
        0x0429CE,
        20,
        "5f19a88cbe36d0aacfd8dbaba46585eb7dc1c9940214cac00d19c0440bb3ceb0",
        "Title_ArtPtrs: ArtNem_WindowTiles at $680, ArtNem_Font at $7C0 and $681",
    ),
    (
        "window_fill",
        0x06A7A8,
        14,
        "c0ee8f63f94b86bf7c4cd8fc38a7491ee379ff4fab8551bb3e35992d1405253f",
        "FillTextBackground: VRAM $B000, $EEEE, $800 words",
    ),
    (
        "window_frame_bottom",
        0x068778,
        34,
        "8e907f530ce5b9ce359a93a64ed13f9c6e5bed71feca517af295c5ee1593a93c",
        "loc_68704: the bottom row, the top row's three tiles with V-flip set",
    ),
    (
        "window_frame_middle",
        0x068746,
        32,
        "44d616a897a75d4b99c1948e13b4c3ab323bfa58338e6cc314e1f08dd7d48fdf",
        "loc_68704: a middle row, $C6F3 edge / $C680 fill / $CEF3 flipped edge",
    ),
    (
        "window_frame_top",
        0x068714,
        34,
        "3c1ce966c5ff2ab617210d6cf6ed9f20ceb49214ce4398c24f961e36d957091f",
        "loc_68704: the top row, $C6E9 corner / $C6EA edge / $CEE9 flipped corner",
    ),
    (
        "window_plane_map",
        0x06A7F8,
        16,
        "f7ce10d743d3ec530bdb41129b98c9f0c8de9bcc943777c2d7e5e43726d251b0",
        "TextBufferPlaneMaps: priority, palette line 2, base tile $580",
    ),
    (
        "window_position",
        0x06A7D8,
        8,
        "140231b01a5ea1d5f177ada5d360851866d27bbd9543e8893090de5f7a24a36c",
        "TextBufferToPlane: move.w #4,d3 / move.w #$15,d4",
    ),
    (
        "window_records",
        0x069380,
        32,
        "be3a6d647692f4c24fd23686c980ba7a3419bad9099e21c5488d37418fe34e3a",
        "WinGroup_Dialogue: four width/height/x/y records, dialogue is 34x6 at (3, 20)",
    ),
    (
        "window_size",
        0x06A7EA,
        8,
        "e81f664a2d4520de134e6dadeb50196821a60378916c2401c590b39edfb1995b",
        "TextBufferToPlane: move.w #$20,d1 / move.w #4,d2",
    ),
];

fn refuse<T>(message: String) -> Result<T, PackError> {
    Err(PackError(message))
}

fn slice<'a>(rom: &'a [u8], at: usize, len: usize, what: &str) -> Result<&'a [u8], PackError> {
    rom.get(at..at + len).ok_or_else(|| {
        PackError(format!(
            "{what} at 0x{at:06X} (+{len}) runs past the end of the ROM"
        ))
    })
}

fn sha256(bytes: &[u8]) -> String {
    crate::png::hex(&Sha256::digest(bytes))
}

/// `check_signatures`: every transcribed routine is the retail one.
/// `check_signatures`: each run, as the manifest records it.
fn check_signatures(rom: &[u8]) -> Result<Vec<Json>, PackError> {
    let mut checked = Vec::with_capacity(SIGNATURES.len());
    for (name, at, len, digest, note) in SIGNATURES {
        let actual = slice(rom, at, len, name)?;
        if sha256(actual) != digest {
            return refuse(format!(
                "{name}: the {len} bytes at 0x{at:06X} ({note}) are not the retail ones: the \
                 image holds {}; this is not the supported retail build",
                crate::png::hex(actual)
            ));
        }
        checked.push(
            Json::object()
                .with("name", name)
                .with("rom_offset", format!("0x{at:06X}"))
                .with("opcodes", crate::png::hex(actual))
                .with("proves", note),
        );
    }
    Ok(checked)
}

/// `dialogue_palette`: `Pal_Init_Line_3`, re-proved as a copy of `Pal_Init`'s
/// line 2.
fn palette(rom: &[u8]) -> Result<Vec<[u8; 3]>, PackError> {
    let mirror = slice(rom, PAL_INIT_LINE_3, PALETTE_LINE_SIZE, "Pal_Init_Line_3")?;
    let inside = slice(
        rom,
        PAL_INIT + PAL_INIT_MIRRORED_LINE * PALETTE_LINE_SIZE,
        PALETTE_LINE_SIZE,
        "Pal_Init",
    )?;
    if mirror != inside {
        return refuse(format!(
            "Pal_Init_Line_3 at 0x{PAL_INIT_LINE_3:06X} is not a copy of line \
             {PAL_INIT_MIRRORED_LINE} of Pal_Init at 0x{PAL_INIT:06X}"
        ));
    }
    Ok(gfx::palette_rgb(&gfx::decode_palette(mirror)?))
}

fn colours(palette: &[[u8; 3]]) -> Json {
    Json::from(palette.iter().map(|c| c.to_vec()).collect::<Vec<_>>())
}

/// The dialogue section's files so far.
/// The dialogue section, and `emit_dialogue`'s manifest fragment.
pub fn build(rom: &Rom, root: &Path) -> Result<Json, PackError> {
    let data = rom.bytes();
    let signatures = check_signatures(data)?;
    let palette = palette(data)?;
    let (font, font_sha) = art::font(data, root, &palette)?;
    let (menu_sha, menu_tiles) = art::menu_font(data, root, &palette)?;
    let scroll_arrow = art::scroll_arrow(data, root, &palette)?;
    let (window, window_sha) = chrome::write(data, root, &palette, &scroll_arrow)?;
    let (trees, trees_sha) = trees::write(data, root, &scroll_arrow)?;
    let (portraits, portraits_sha, portrait_bytes) = art::portraits(data, root, &palette)?;

    let mut census = census::Census::default();
    for tree in trees.at(&["trees"])?.items()? {
        let id = tree.at(&["tree"])?.int()?;
        for entry in tree.at(&["entries"])?.items()? {
            census.add_entry(id, entry)?;
        }
    }
    let portrait_ids = portraits
        .at(&["portraits"])?
        .items()?
        .iter()
        .map(|p| p.at(&["id"])?.int())
        .collect::<Result<BTreeSet<_>, _>>()?;
    let mut blank = BTreeSet::new();
    for glyph in font.at(&["glyphs"])?.items()? {
        if glyph.at(&["blank"])? == &Json::Bool(true) {
            blank.insert(glyph.at(&["byte"])?.int()? as u8);
        }
    }
    let Json::Object(roles) = window.at(&["roles"])? else {
        return refuse("window roles are not an object".into());
    };
    let mut role_tiles = Json::object();
    for (name, role) in roles {
        role_tiles.insert(name.clone(), role.at(&["tile"])?.clone());
    }
    let fragment = Json::object()
        .with("format_version", FORMAT_VERSION)
        .with("directory", "dialogue")
        .with(
            "trees",
            Json::object()
                .with("path", "dialogue/trees.json")
                .with("sha256", trees_sha)
                .with("tree_count", trees.at(&["tree_count"])?.clone())
                .with("entry_count", trees.at(&["entry_count"])?.clone())
                .with(
                    "system_message_count",
                    trees.at(&["system_messages", "count"])?.clone(),
                ),
        )
        .with(
            "font",
            Json::object()
                .with("path", "dialogue/font.json")
                .with("sha256", font_sha)
                .with("png", "dialogue/font.png")
                .with("png_sha256", font.at(&["png_sha256"])?.clone())
                .with("glyph_count", art::GLYPH_COUNT)
                .with("glyph_size", vec![art::GLYPH_WIDTH, art::GLYPH_HEIGHT]),
        )
        .with(
            "chrome",
            Json::object()
                .with("path", "dialogue/window.json")
                .with("sha256", window_sha)
                .with("png", "dialogue/window.png")
                .with("png_sha256", window.at(&["png_sha256"])?.clone())
                .with("tile_count", window.at(&["tile", "count"])?.clone())
                .with("roles", role_tiles)
                .with("rect", window.at(&["text_window", "rect"])?.clone())
                .with("scroll_arrow", scroll_arrow.clone()),
        )
        .with(
            "menu_font",
            Json::object()
                .with("path", "dialogue/menu_font.png")
                .with("png_sha256", menu_sha)
                .with("tile_count", menu_tiles),
        )
        .with(
            "portraits",
            Json::object()
                .with("path", "dialogue/portraits.json")
                .with("sha256", portraits_sha)
                .with("directory", "dialogue/portraits")
                .with("count", portraits.at(&["count"])?.clone())
                .with("distinct_art", portraits.at(&["distinct_art"])?.clone())
                .with("bytes", portrait_bytes),
        )
        .with("window", trees::window_record(&scroll_arrow))
        .with("signatures", signatures)
        .with(
            "census",
            census.to_json(&portrait_ids, &blank, &trees::control_codes()),
        );
    Ok(Json::object().with("dialogue", fragment))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_signature_is_named_once_and_fits_a_digest() {
        let mut names: Vec<&str> = SIGNATURES.iter().map(|s| s.0).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), SIGNATURES.len());
        assert!(SIGNATURES.iter().all(|s| s.3.len() == 64 && s.2 > 0));
    }

    #[test]
    fn a_blank_image_is_refused_before_anything_is_written() {
        let rom = vec![0u8; 0x300000];
        let error = check_signatures(&rom).unwrap_err();
        assert!(
            error.0.contains("not the supported retail build"),
            "{}",
            error.0
        );
    }
}
