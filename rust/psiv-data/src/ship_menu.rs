//! The ship's destination screen, as the pack carries it.
//!
//! `loc_63BC4` (`ps4.asm:133499`) stands its two windows on a planetary map it
//! draws itself. `psiv_tools/ship_menu_pack.py` replays that drawing from the
//! ROM; this module loads the result: the strings, the window records
//! (`WinGroup_Event`, `ps4.asm:141150`), the palette and one background per
//! cover combination, and turns a background and a palette state into pixels.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{DataError, RomHash};

/// Where the pack keeps the screen's JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShipMenuFile {
    /// Relative JSON path.
    pub path: String,
    /// Extractor's provenance digest.
    pub sha256: String,
}

/// The frame the screen is drawn in.
pub const SCREEN_WIDTH: usize = 320;
/// The frame the screen is drawn in.
pub const SCREEN_HEIGHT: usize = 224;
/// Cover bit: the Rykros marker's cover is drawn (`ps4.asm:133781-133872`).
pub const COVER_RYKROS: usize = 1;
/// Cover bit: the Kuran marker's cover is drawn (`ps4.asm:133880-133893`).
pub const COVER_KURAN: usize = 2;
/// Cover bit: the Air Castle marker's cover is drawn (`ps4.asm:133895-133911`).
pub const COVER_AIR_CASTLE: usize = 4;
/// The palette words `loc_64144` darkens when the Rykros marker is covered
/// (`Palette_Table_Buffer+$1C`, `ps4.asm:133903-133906`).
pub const DARK_WORDS: [usize; 2] = [14, 15];
/// The first palette word the cursor row's marker blinks through
/// (`addq.w #7, d1`, `ps4.asm:133921`); the world's low three bits are added.
pub const HIGHLIGHT_BASE_WORD: usize = 7;

/// One `WinGroup_Event` record: a window's size and position in cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct ShipWindow {
    /// The window id `Window_Create` is called with.
    pub id: u8,
    /// Width in cells, frame included.
    pub width: u8,
    /// Height in cells, frame included.
    pub height: u8,
    /// Left cell.
    pub x: u8,
    /// Top cell.
    pub y: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct PaletteEntry {
    rgb: [u8; 3],
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct Colors {
    highlight: [u8; 3],
    dark: [u8; 3],
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct BackgroundRecord {
    covers: usize,
    file: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct ShipMenuJson {
    format_version: u32,
    kind: String,
    rom_sha256: RomHash,
    palette: Vec<PaletteEntry>,
    colors: Colors,
    windows: Vec<ShipWindow>,
    prompt: String,
    confirm_suffix: Vec<String>,
    names: Vec<String>,
    #[serde(default)]
    flight_captions: Vec<FlightCaption>,
    #[serde(default)]
    place_windows: Vec<ShipWindow>,
    backgrounds: Vec<BackgroundRecord>,
}

/// `SpaceTravel_PlaceNamePtrs`: a complete RunText2 line (leading spaces kept).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct FlightCaption {
    world: u8,
    text: String,
    glyph_count: usize,
    instant: bool,
}

/// What the screen's palette does on a frame (`loc_64144`, `ps4.asm:133894-133925`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ShipPalette {
    /// Words 14 and 15 are dark blue: the Rykros marker is covered.
    pub dark: bool,
    /// The world whose marker is lit this frame, if the blink is on.
    pub highlight_world: Option<u8>,
}

/// The loaded screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShipMenu {
    palette: Vec<[u8; 3]>,
    highlight: [u8; 3],
    dark: [u8; 3],
    windows: Vec<ShipWindow>,
    prompt: String,
    confirm_suffix: Vec<String>,
    names: Vec<String>,
    flight_captions: Vec<FlightCaption>,
    place_windows: Vec<ShipWindow>,
    backgrounds: Vec<Vec<u8>>,
}

impl ShipMenu {
    pub(crate) fn load(
        directory: &Path,
        file: &ShipMenuFile,
        rom: &RomHash,
    ) -> Result<ShipMenu, DataError> {
        let path = directory.join(&file.path);
        let text = std::fs::read_to_string(&path).map_err(|e| DataError::io(&path, e))?;
        let json: ShipMenuJson =
            serde_json::from_str(&text).map_err(|e| DataError::json(&path, e))?;
        let mismatch = |message: String| DataError::ManifestMismatch {
            path: path.clone(),
            field: "ship_menu",
            manifest: "ship_menu/1 with matching ROM, 32 colours, six names, eight backgrounds"
                .into(),
            record: message,
        };
        if json.format_version != 1 || json.kind != "ship_menu" || json.rom_sha256 != *rom {
            return Err(mismatch("wrong version, kind or ROM hash".into()));
        }
        if json.palette.len() != 32 || json.names.len() != 6 || json.backgrounds.len() != 8 {
            return Err(mismatch("wrong palette, name or background count".into()));
        }
        // Older menu packs remain usable; their absent caption is exposed as
        // None. A present table must be complete, ordered and instant (d4=1).
        if !json.flight_captions.is_empty()
            && (json.flight_captions.len() != 6
                || json
                    .flight_captions
                    .iter()
                    .enumerate()
                    .any(|(world, caption)| {
                        usize::from(caption.world) != world
                            || !caption.instant
                            || caption.glyph_count == 0
                            || caption.glyph_count > 32
                            || caption.text.chars().count() != caption.glyph_count
                    }))
        {
            return Err(mismatch(
                "malformed flight captions; rebuild the pack".into(),
            ));
        }
        if !json.place_windows.is_empty()
            && (json.place_windows.len() != 9
                || json.place_windows.iter().enumerate().any(|(id, window)| {
                    usize::from(window.id) != id || window.width < 2 || window.height < 2
                }))
        {
            return Err(mismatch(
                "malformed place-name windows; rebuild the pack".into(),
            ));
        }
        let mut backgrounds = vec![Vec::new(); 8];
        for record in &json.backgrounds {
            let slot = backgrounds
                .get_mut(record.covers)
                .ok_or_else(|| mismatch(format!("cover set {} out of range", record.covers)))?;
            let file = directory.join(&record.file);
            let bytes = std::fs::read(&file).map_err(|e| DataError::io(&file, e))?;
            if bytes.len() != SCREEN_WIDTH * SCREEN_HEIGHT
                || bytes.iter().any(|&index| usize::from(index) >= 64)
            {
                return Err(mismatch(format!(
                    "background {} is malformed",
                    record.covers
                )));
            }
            *slot = bytes;
        }
        if backgrounds.iter().any(Vec::is_empty) {
            return Err(mismatch("a cover set has no background".into()));
        }
        Ok(ShipMenu {
            palette: json.palette.iter().map(|entry| entry.rgb).collect(),
            highlight: json.colors.highlight,
            dark: json.colors.dark,
            windows: json.windows,
            prompt: json.prompt,
            confirm_suffix: json.confirm_suffix,
            names: json.names,
            flight_captions: json.flight_captions,
            place_windows: json.place_windows,
            backgrounds,
        })
    }

    /// `loc_2AAA46`: "Where do you want to go?".
    #[must_use]
    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    /// `loc_2AAA60`: the two lines that follow the chosen name.
    #[must_use]
    pub fn confirm_suffix(&self) -> &[String] {
        &self.confirm_suffix
    }

    /// `loc_2AAA7A`: a world's name, by `World_Index`.
    #[must_use]
    pub fn name(&self, world: u8) -> Option<&str> {
        self.names.get(usize::from(world)).map(String::as_str)
    }

    /// The flight's planet/satellite caption, not the destination row name.
    /// `loc_64C4A` calls RunText2 with d4=1: all glyphs appear without waits.
    #[must_use]
    pub fn flight_caption(&self, world: u8) -> Option<&str> {
        self.flight_captions
            .get(usize::from(world))
            .map(|caption| caption.text.as_str())
    }

    /// The `WinGroup_Event` record for a window id.
    #[must_use]
    pub fn window(&self, id: u8) -> Option<ShipWindow> {
        self.windows.iter().copied().find(|window| window.id == id)
    }

    /// FieldRoutine_PlaceName indexes WinGroup_PlaceName by glyph count minus
    /// two (:136609-136615); older packs have no record to draw.
    #[must_use]
    pub fn place_window(&self, glyphs: usize) -> Option<ShipWindow> {
        self.place_windows.get(glyphs.checked_sub(2)?).copied()
    }

    /// The background for a cover set: a bit mask of the `COVER_*` bits.
    #[must_use]
    pub fn background(&self, covers: usize) -> Option<&[u8]> {
        self.backgrounds.get(covers).map(Vec::as_slice)
    }

    /// The screen's picture as RGBA, for a cover set and a palette state.
    ///
    /// A background byte is a global CRAM index; `0` is the backdrop, which
    /// the VDP draws as CRAM entry 0. The palette is the pack's 32 words, then
    /// `loc_64144`'s two rewrites: words 14 and 15 dark when the Rykros marker
    /// is covered, and the lit marker's word white (`ps4.asm:133915-133924`).
    /// CRAM lines 2 and 3 are not in the picture: the windows draw them.
    #[must_use]
    pub fn render(&self, covers: usize, state: ShipPalette) -> Option<Vec<u8>> {
        let background = self.background(covers)?;
        let mut palette = self.palette.clone();
        if state.dark {
            for word in DARK_WORDS {
                palette[word] = self.dark;
            }
        }
        if let Some(world) = state.highlight_world {
            palette[HIGHLIGHT_BASE_WORD + usize::from(world & 7)] = self.highlight;
        }
        let mut pixels = Vec::with_capacity(background.len() * 4);
        for &index in background {
            let [r, g, b] = palette
                .get(usize::from(index))
                .copied()
                .unwrap_or([0, 0, 0]);
            pixels.extend_from_slice(&[r, g, b, 255]);
        }
        Some(pixels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn menu() -> ShipMenu {
        let mut palette = vec![[1, 2, 3]; 32];
        palette[8] = [8, 8, 8];
        palette[14] = [14, 14, 14];
        let mut background = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT];
        background[0] = 8;
        background[1] = 14;
        background[2] = 5;
        ShipMenu {
            palette,
            highlight: [200, 200, 200],
            dark: [0, 0, 64],
            windows: vec![ShipWindow {
                id: 5,
                width: 26,
                height: 5,
                x: 7,
                y: 21,
            }],
            prompt: "P".into(),
            confirm_suffix: vec![],
            names: vec!["A".into(); 6],
            flight_captions: vec![],
            place_windows: vec![],
            backgrounds: vec![background; 8],
        }
    }

    #[test]
    fn the_blink_lights_the_worlds_word_and_the_dark_rewrite_takes_14_and_15() {
        let menu = menu();
        let plain = menu.render(0, ShipPalette::default()).unwrap();
        assert_eq!(&plain[0..4], &[8, 8, 8, 255]);
        assert_eq!(&plain[4..8], &[14, 14, 14, 255]);
        // World 1 lights word 8.
        let lit = menu
            .render(
                0,
                ShipPalette {
                    dark: false,
                    highlight_world: Some(1),
                },
            )
            .unwrap();
        assert_eq!(&lit[0..4], &[200, 200, 200, 255]);
        // The dark rewrite takes word 14, and a world's low three bits pick the word.
        let dark = menu
            .render(
                7,
                ShipPalette {
                    dark: true,
                    highlight_world: None,
                },
            )
            .unwrap();
        assert_eq!(&dark[4..8], &[0, 0, 64, 255]);
        assert!(menu.render(8, ShipPalette::default()).is_none());
    }

    /// The local pack's screen, when the pack has one (a pack built before the
    /// destination menu does not): the facts the cartridge states.
    #[test]
    fn the_local_pack_screen_holds_the_cartridges_strings_windows_and_pictures() {
        let root = std::env::var_os("PSIV_RUNTIME_PACK")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime-pack"));
        if !root.join("manifest.json").is_file() {
            eprintln!("runtime pack absent; skipping local ship screen");
            return;
        }
        let data = crate::GameData::load(&root).expect("local pack loads");
        let Some(menu) = data.ship_menu() else {
            eprintln!("ship screen absent; skipping local ship screen");
            return;
        };
        assert_eq!(menu.prompt(), "Where do you want to go?");
        assert_eq!(menu.confirm_suffix(), [" will be", "the destination."]);
        let names: Vec<_> = (0..6).filter_map(|world| menu.name(world)).collect();
        assert_eq!(
            names,
            ["MOTAVIA", "DEZOLIS", "RYKROS", "ZELAN", "KURAN", "A.CASTL"]
        );
        // `WinGroup_Event` records 5 and 6 (`ps4.asm:141166`, `:141169`).
        let prompt = menu.window(5).unwrap();
        assert_eq!(
            (prompt.x, prompt.y, prompt.width, prompt.height),
            (7, 21, 26, 5)
        );
        let list = menu.window(6).unwrap();
        assert_eq!((list.x, list.y, list.width, list.height), (26, 9, 11, 3));
        // Eight backgrounds, one per cover set, each a whole frame; a cover
        // changes the picture, and the lit marker changes the colours.
        for covers in 0..8 {
            assert_eq!(
                menu.background(covers).unwrap().len(),
                SCREEN_WIDTH * SCREEN_HEIGHT
            );
        }
        assert_ne!(menu.background(0), menu.background(COVER_RYKROS));
        assert_ne!(menu.background(0), menu.background(COVER_KURAN));
        assert_ne!(menu.background(0), menu.background(COVER_AIR_CASTLE));
        let plain = menu.render(7, ShipPalette::default()).unwrap();
        let lit = menu
            .render(
                7,
                ShipPalette {
                    dark: false,
                    highlight_world: Some(3),
                },
            )
            .unwrap();
        assert_eq!(plain.len(), SCREEN_WIDTH * SCREEN_HEIGHT * 4);
        assert_ne!(plain, lit, "world 3's marker lights");
        if menu.flight_caption(0).is_none() {
            eprintln!("flight captions absent in older pack; skipping caption checks");
            return;
        }
        for world in 0..6 {
            let caption = menu.flight_caption(world).expect("six captions");
            assert!(caption.ends_with(menu.name(world).unwrap()) || world == 5);
            assert!(!caption.trim().is_empty());
        }
        assert!(menu.flight_caption(6).is_none());
        assert_eq!(
            menu.place_window(2).map(|w| (w.width, w.height, w.x, w.y)),
            Some((6, 3, 17, 7))
        );
        assert!(menu.place_window(1).is_none());
        assert!(menu.place_window(11).is_none());
    }

    #[test]
    fn window_records_are_found_by_id() {
        assert_eq!(menu().window(5).map(|window| window.y), Some(21));
        assert!(menu().window(6).is_none());
    }

    #[test]
    fn caption_contract_rejects_waiting_lines_and_bad_window_records() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("psiv-ship-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let mut json = serde_json::json!({
            "format_version": 1, "kind": "ship_menu", "rom_sha256": "00".repeat(32),
            "palette": vec![serde_json::json!({"rgb": [0, 0, 0]}); 32],
            "colors": {"highlight": [255, 255, 255], "dark": [0, 0, 0]},
            "windows": [], "prompt": "a", "confirm_suffix": [], "names": vec!["a"; 6],
            "flight_captions": (0..6).map(|world| serde_json::json!({
                "world": world, "text": " a", "glyph_count": 2, "instant": true
            })).collect::<Vec<_>>(),
            "place_windows": (0..9).map(|id| serde_json::json!({
                "id": id, "width": 6 + id, "height": 3, "x": 1, "y": 2
            })).collect::<Vec<_>>(),
            "backgrounds": (0..8).map(|covers| serde_json::json!({
                "covers": covers, "file": format!("{covers}.idx")
            })).collect::<Vec<_>>()
        });
        for covers in 0..8 {
            std::fs::write(
                root.join(format!("{covers}.idx")),
                vec![0; SCREEN_WIDTH * SCREEN_HEIGHT],
            )
            .unwrap();
        }
        let file = ShipMenuFile {
            path: "menu.json".into(),
            sha256: String::new(),
        };
        let rom = serde_json::from_value(json["rom_sha256"].clone()).unwrap();
        let load = |json: &serde_json::Value| {
            std::fs::write(root.join(&file.path), serde_json::to_vec(json).unwrap()).unwrap();
            ShipMenu::load(&root, &file, &rom)
        };
        let valid = load(&json).unwrap();
        assert_eq!(valid.flight_caption(0), Some(" a"));
        assert_eq!(valid.place_window(2).unwrap().width, 6);
        // d4=1's contract must not silently accept per-character waits.
        json["flight_captions"][0]["instant"] = false.into();
        assert!(matches!(
            load(&json),
            Err(DataError::ManifestMismatch {
                field: "ship_menu",
                ..
            })
        ));
        json["flight_captions"][0]["instant"] = true.into();
        json["flight_captions"][0]["glyph_count"] = 3.into();
        assert!(load(&json).is_err());
        json["flight_captions"][0]["glyph_count"] = 2.into();
        json["place_windows"][0]["width"] = 0.into();
        assert!(load(&json).is_err());
        // Backward compatibility is absence, never a guessed caption.
        json.as_object_mut().unwrap().remove("flight_captions");
        json.as_object_mut().unwrap().remove("place_windows");
        assert_eq!(load(&json).unwrap().flight_caption(0), None);
        for covers in 0..8 {
            std::fs::remove_file(root.join(format!("{covers}.idx"))).unwrap();
        }
        std::fs::remove_file(root.join(&file.path)).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
