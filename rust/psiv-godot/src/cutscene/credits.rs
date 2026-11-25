//! Indexed VDP compositor and Godot surface for `Cutscene_Ending`'s credits
//! planes (`$079D0C..$07A139`).
//!
//! `psiv-runtime` owns all counters, cameras, row copies, commands and CRAM
//! updates (`EndingCreditsView`, `CramView`). This module only decodes the
//! pack's 4bpp tiles and 8×16 font glyphs and composites Plane A over Plane B
//! onto a 320×224 RGBA8 texture using live CRAM.

use godot::classes::image::Format;
use godot::classes::{Image, ImageTexture};
use godot::prelude::*;
use psiv_data::color::cram_rgb;
use psiv_data::{EndingCreditsAssets, FontFile};
use psiv_runtime::EndingCreditsView;

use super::{CutsceneLayer, SCREEN};
use crate::cram_art::CramState;

const WIDTH: usize = 320;
const HEIGHT: usize = 224;
const PIXEL_BYTES: usize = WIDTH * HEIGHT * 4;
const VRAM_TILES: usize = 2048;

/// Pure-Rust Mega Drive VDP compositor for the ending credits planes.
/// Independent of Godot so unit tests can verify tile decoding, flipping,
/// 2-cell column scrolling, VDP priority occlusion and live CRAM colouring.
pub(crate) struct CreditsCompositor {
    tiles: Box<[[u8; 64]; VRAM_TILES]>,
    font_glyphs: Box<[([u8; 64], [u8; 64]); 256]>,
}

impl CreditsCompositor {
    /// Decodes the packed 4bpp tiles from `assets` and the 8×16 `RunText2`
    /// glyph strip from `font` + its decoded RGBA8 strip pixels.
    pub(crate) fn new(
        assets: &EndingCreditsAssets,
        font: &FontFile,
        font_rgba: &[u8],
        font_width: usize,
        font_height: usize,
    ) -> Self {
        let mut tiles = Box::new([[0u8; 64]; VRAM_TILES]);
        for art in &assets.art {
            let base = usize::from(art.vram_destination / 32);
            let (chunks, _) = art.tiles.as_chunks::<32>();
            for (offset, bytes) in chunks.iter().enumerate() {
                let Some(slot) = tiles.get_mut(base + offset) else {
                    break;
                };
                for (i, &byte) in bytes.iter().enumerate() {
                    slot[i * 2] = (byte >> 4) & 0x0F;
                    slot[i * 2 + 1] = byte & 0x0F;
                }
            }
        }
        let mut font_glyphs = Box::new([([0u8; 64], [0u8; 64]); 256]);
        for glyph in &font.glyphs {
            let gx = usize::try_from(glyph.x).unwrap_or(0);
            let gy = usize::try_from(glyph.y).unwrap_or(0);
            if gx + 8 > font_width || gy + 16 > font_height {
                continue;
            }
            let (top, bottom) = &mut font_glyphs[usize::from(glyph.byte)];
            for row in 0..16 {
                let dst = if row < 8 {
                    &mut top[row * 8..row * 8 + 8]
                } else {
                    &mut bottom[(row - 8) * 8..(row - 8) * 8 + 8]
                };
                let src_row = (gy + row) * font_width + gx;
                for (col, pixel) in dst.iter_mut().enumerate() {
                    let alpha = font_rgba.get((src_row + col) * 4 + 3).copied().unwrap_or(0);
                    *pixel = if alpha != 0 { 0x0F } else { 0 };
                }
            }
        }
        Self { tiles, font_glyphs }
    }

    fn sample(
        &self,
        plane: &[u16; 2048],
        x: usize,
        y: usize,
        runtext2: Option<&EndingCreditsView>,
    ) -> (bool, Option<usize>) {
        let word = plane[((y >> 3) & 31) * 64 + ((x >> 3) & 63)];
        let tile_idx = word & 0x7FF;
        if tile_idx == 0 {
            return (false, None);
        }
        let priority = word & 0x8000 != 0;
        let line = usize::from((word >> 13) & 3);
        let flip_v = word & 0x1000 != 0;
        let flip_h = word & 0x0800 != 0;
        let tx = if flip_h { 7 - (x & 7) } else { x & 7 };
        let ty = if flip_v { 7 - (y & 7) } else { y & 7 };
        let idx = if let Some((byte, half)) = runtext2.and_then(|v| v.runtext2_tile(tile_idx)) {
            let (top, bottom) = &self.font_glyphs[usize::from(byte)];
            if half == 0 {
                top[ty * 8 + tx]
            } else {
                bottom[ty * 8 + tx]
            }
        } else {
            self.tiles[usize::from(tile_idx)][ty * 8 + tx]
        };
        if idx == 0 {
            (priority, None)
        } else {
            (priority, Some(line * 16 + usize::from(idx)))
        }
    }

    /// Renders the 320×224 credits frame into `out` (RGBA8, `320 * 224 * 4`
    /// bytes) using live `cram` words and Mega Drive VDP priority order:
    /// High A > High B > Low A > Low B > Backdrop (CRAM word 0).
    pub(crate) fn compose(&self, view: &EndingCreditsView, cram: &[u16; 64], out: &mut [u8]) {
        debug_assert_eq!(out.len(), PIXEL_BYTES);
        let palette: [[u8; 3]; 64] = std::array::from_fn(|i| cram_rgb(cram[i]));
        let fg_scroll = usize::from(view.fg_y_scroll);
        let bg_scroll = usize::from(view.bg_y_scroll);
        let column_scroll = view.column_scroll;

        for sy in 0..HEIGHT {
            let by = (sy + bg_scroll) & 0xFF;
            let row_off = sy * WIDTH * 4;
            for sx in 0..WIDTH {
                let a_scroll = if column_scroll && sx >= 128 {
                    0
                } else {
                    fg_scroll
                };
                let ay = (sy + a_scroll) & 0xFF;
                let (prio_a, slot_a) = self.sample(&view.plane_a, sx, ay, Some(view));
                let (prio_b, slot_b) = self.sample(&view.plane_b, sx, by, None);
                let slot = if prio_a && slot_a.is_some() {
                    slot_a
                } else if prio_b && slot_b.is_some() {
                    slot_b
                } else if slot_a.is_some() {
                    slot_a
                } else if slot_b.is_some() {
                    slot_b
                } else {
                    Some(0)
                }
                .unwrap_or(0);
                let [r, g, b] = palette[slot];
                let dst = row_off + sx * 4;
                out[dst] = r;
                out[dst + 1] = g;
                out[dst + 2] = b;
                out[dst + 3] = 255;
            }
        }
    }
}

/// Godot-side texture buffer holding the latest composited credits frame.
pub(super) struct CreditsSurface {
    compositor: CreditsCompositor,
    rgba: Vec<u8>,
    texture: Gd<ImageTexture>,
    last_view_rev: Option<u64>,
    last_cram_rev: Option<u64>,
    visible: bool,
}

impl CreditsSurface {
    fn load(
        assets: &EndingCreditsAssets,
        pack: &crate::pack_assets::PackAssets,
        font: &FontFile,
    ) -> Option<Self> {
        let mut font_img = pack.load_image(&font.png)?;
        font_img.convert(Format::RGBA8);
        let font_w = usize::try_from(font_img.get_width()).ok()?;
        let font_h = usize::try_from(font_img.get_height()).ok()?;
        let font_data = font_img.get_data();
        let compositor = CreditsCompositor::new(assets, font, font_data.as_slice(), font_w, font_h);
        let rgba = vec![0u8; PIXEL_BYTES];
        let image = Image::create_from_data(
            WIDTH as i32,
            HEIGHT as i32,
            false,
            Format::RGBA8,
            &PackedByteArray::from(rgba.as_slice()),
        )?;
        let texture = ImageTexture::create_from_image(&image)?;
        Some(Self {
            compositor,
            rgba,
            texture,
            last_view_rev: None,
            last_cram_rev: None,
            visible: false,
        })
    }

    fn sync(&mut self, view: &EndingCreditsView, cram: &CramState) -> bool {
        let dirty = !self.visible
            || self.last_view_rev != Some(view.revision)
            || self.last_cram_rev != Some(cram.revision);
        if !dirty {
            return false;
        }
        self.compositor.compose(view, &cram.words, &mut self.rgba);
        if let Some(image) = Image::create_from_data(
            WIDTH as i32,
            HEIGHT as i32,
            false,
            Format::RGBA8,
            &PackedByteArray::from(self.rgba.as_slice()),
        ) {
            self.texture.update(&image);
        }
        self.last_view_rev = Some(view.revision);
        self.last_cram_rev = Some(cram.revision);
        self.visible = true;
        true
    }
}

impl CutsceneLayer {
    /// Initializes the credits tile compositor from the loaded pack.
    pub(crate) fn configure_credits(
        &mut self,
        assets: &EndingCreditsAssets,
        pack: &crate::pack_assets::PackAssets,
        font: &FontFile,
    ) {
        self.credits = CreditsSurface::load(assets, pack, font);
        if self.credits.is_none() {
            godot_error!("ending credits compositor failed to load dialogue font strip");
        }
    }

    pub(super) fn credits_visible(&self) -> bool {
        self.credits.as_ref().is_some_and(|surface| surface.visible)
    }

    /// Updates the credits surface from the runtime's live credits view and CRAM.
    pub(crate) fn sync_credits(&mut self, view: Option<&EndingCreditsView>) {
        let Some(cram) = self.cram.clone() else {
            return;
        };
        let changed = match (self.credits.as_mut(), view) {
            (Some(surface), Some(view)) => surface.sync(view, &cram.borrow()),
            (Some(surface), None) if surface.visible => {
                surface.visible = false;
                true
            }
            _ => false,
        };
        if changed {
            let visible = self.shows_anything();
            self.base_mut().set_visible(visible);
            self.base_mut().queue_redraw();
        }
    }

    pub(super) fn draw_credits(&mut self) {
        let Some(texture) = self
            .credits
            .as_ref()
            .filter(|surface| surface.visible)
            .map(|surface| surface.texture.clone())
        else {
            return;
        };
        self.base_mut()
            .draw_texture_rect(&texture, Rect2::new(Vector2::ZERO, SCREEN), false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use psiv_data::{
        CreditsArt, CreditsCommands, CreditsFontUpload, CreditsMapping, CreditsPalette,
        CreditsText, Glyph, GlyphMetrics,
    };

    fn test_assets() -> (EndingCreditsAssets, FontFile, Vec<u8>) {
        // Tile $100 (Plane B small planet): solid index 2.
        // Tile $200 (Plane A large planet): left half index 3, right half transparent (0).
        // Tile $7C0 (CreditFont): solid index 2.
        let mut small = vec![0u8; 87 * 32];
        small[..32].fill(0x22);
        let mut large = vec![0u8; 67 * 32];
        for row in 0..8 {
            large[row * 4] = 0x33;
            large[row * 4 + 1] = 0x33;
            large[row * 4 + 2] = 0x00;
            large[row * 4 + 3] = 0x00;
        }
        let mut cfont = vec![0u8; 29 * 32];
        cfont[..32].fill(0x22);
        let assets = EndingCreditsAssets {
            version: psiv_data::CREDITS_VERSION,
            source_rom_sha256: "0".repeat(64),
            art: vec![
                CreditsArt {
                    rom_addr: 0x1DE278,
                    compression: "kosinski".into(),
                    consumed: 1,
                    vram_destination: 0x2000,
                    tiles: small,
                },
                CreditsArt {
                    rom_addr: 0x1DEA48,
                    compression: "kosinski".into(),
                    consumed: 1,
                    vram_destination: 0x4000,
                    tiles: large,
                },
                CreditsArt {
                    rom_addr: 0x2A3350,
                    compression: "nemesis".into(),
                    consumed: 1,
                    vram_destination: 0xF800,
                    tiles: cfont,
                },
            ],
            font_upload: CreditsFontUpload {
                rom_addr: 0x7A26C,
                destination_tile: 0x7C0,
                art_rom_addr: 0x2A3350,
                terminator: 0xFFFF,
            },
            mappings: vec![
                CreditsMapping {
                    rom_addr: 0x1DF09C,
                    consumed: 1,
                    base_tile: 0x100,
                    ram_destinations: vec![0xFFFF_1000],
                    columns: 16,
                    rows: 28,
                    words: vec![0x0100; 448],
                },
                CreditsMapping {
                    rom_addr: 0x1DF17C,
                    consumed: 1,
                    base_tile: 0x100,
                    ram_destinations: vec![0xFFFF_1380],
                    columns: 16,
                    rows: 28,
                    words: vec![0x0100; 448],
                },
                CreditsMapping {
                    rom_addr: 0x1DF294,
                    consumed: 1,
                    base_tile: 0x100,
                    ram_destinations: vec![0xFFFF_1700],
                    columns: 16,
                    rows: 28,
                    words: vec![0x0100; 448],
                },
                CreditsMapping {
                    rom_addr: 0x1DF3E0,
                    consumed: 1,
                    base_tile: 0x100,
                    ram_destinations: vec![0xFFFF_1A80, 0xFFFF_1E00],
                    columns: 16,
                    rows: 28,
                    words: vec![0x0100; 448],
                },
                CreditsMapping {
                    rom_addr: 0x1DF4C2,
                    consumed: 1,
                    base_tile: 0x2200,
                    ram_destinations: vec![0xFFFF_2000],
                    columns: 16,
                    rows: 28,
                    words: vec![0x2200; 448],
                },
            ],
            palettes: vec![
                CreditsPalette {
                    rom_addr: 0x1DF53A,
                    start_word: 0,
                    words: vec![0; 32],
                },
                CreditsPalette {
                    rom_addr: 0x1DF57A,
                    start_word: 48,
                    words: vec![0; 16],
                },
            ],
            commands: vec![
                CreditsCommands {
                    rom_addr: 0x7A522,
                    bytes: vec![0xFF, 0x00],
                },
                CreditsCommands {
                    rom_addr: 0x7A548,
                    bytes: vec![0xFF, 0x00],
                },
            ],
            headers: vec![CreditsText {
                rom_addr: 0x1FEE26,
                bytes: vec![0x80, 0xFF],
            }],
            names: vec![CreditsText {
                rom_addr: 0x1FEB42,
                bytes: vec![0x01, 0xFF],
            }],
            fade: CreditsPalette {
                rom_addr: 0x7A6DE,
                start_word: 50,
                words: vec![0; 40],
            },
        };
        let font = FontFile {
            format_version: 2,
            png: "dialogue/font.png".into(),
            by_char: Default::default(),
            glyphs: vec![Glyph {
                byte: 1,
                char: Some('A'),
                x: 0,
                y: 0,
                width: 8,
                height: 16,
                blank: false,
            }],
            glyph: GlyphMetrics {
                count: 1,
                width: 8,
                height: 16,
            },
            unmapped_glyphs: Vec::new(),
        };
        let font_rgba = vec![255u8; 8 * 16 * 4];
        (assets, font, font_rgba)
    }

    fn pixel(rgba: &[u8], x: usize, y: usize) -> [u8; 3] {
        let i = (y * WIDTH + x) * 4;
        [rgba[i], rgba[i + 1], rgba[i + 2]]
    }

    #[test]
    fn compositor_respects_vdp_priority_column_scroll_and_live_cram() {
        let (assets, font, font_rgba) = test_assets();
        let comp = CreditsCompositor::new(&assets, &font, &font_rgba, 8, 16);
        let mut cram = [0u16; 64];
        cram[0] = 0x0222; // backdrop
        cram[2] = 0x0E00; // line 0 index 2 (Plane B sun)
        cram[16 + 3] = 0x00E0; // line 1 index 3 (Plane A planet)
        cram[48 + 2] = 0x000E; // line 3 index 2 (Header on Plane A right half)

        let mut plane_a = Box::new([0u16; 2048]);
        let mut plane_b = Box::new([0u16; 2048]);
        // Plane B tile $0100 (line 0, low prio) at (0, 0).
        plane_b[0] = 0x0100;
        // Plane A tile $2200 (line 1, low prio) at row 2 col 0 (y = 16).
        plane_a[2 * 64] = 0x2200;
        // Plane A header tile $E7C0 (line 3, high prio) at row 0 col 17 (x = 136).
        plane_a[17] = 0xE7C0;

        let mut view = EndingCreditsView {
            plane_a,
            plane_b,
            runtext2_vram: Box::new([None; 2048]),
            fg_y_scroll: 16, // scrolls left 16 columns up by 16px so row 2 lands at sy=0
            bg_y_scroll: 0,
            column_scroll: true,
            stage: 2,
            text_counter: 0x380,
            row_counter: 0,
            fade_control: 0,
            cred_text_a: 0,
            cred_text_b: 0,
            fade_step: 0,
            revision: 1,
        };
        let mut out = vec![0u8; PIXEL_BYTES];
        comp.compose(&view, &cram, &mut out);

        // At (0, 0): Plane A planet (index 3, green) occludes Plane B (blue).
        assert_eq!(pixel(&out, 0, 0), cram_rgb(0x00E0));
        // At (4, 0): Plane A planet pixel is transparent (0), revealing Plane B (blue).
        assert_eq!(pixel(&out, 4, 0), cram_rgb(0x0E00));
        // At (136, 0): column_scroll locks x >= 128 to vscroll 0, so header draws at sy=0 (red).
        assert_eq!(pixel(&out, 136, 0), cram_rgb(0x000E));
        // At (200, 0): empty cell shows backdrop word 0.
        assert_eq!(pixel(&out, 200, 0), cram_rgb(0x0222));

        // Negative control: turning column_scroll off scrolls the header away from sy=0.
        view.column_scroll = false;
        comp.compose(&view, &cram, &mut out);
        assert_eq!(pixel(&out, 136, 0), cram_rgb(0x0222));
    }
}
