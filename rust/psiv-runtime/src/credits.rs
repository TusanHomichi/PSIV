//! `Cutscene_Ending`'s three-stage credits renderer (`$079D0C..$07A0A5`).
//!
//! Owns the retail counters (`TextCounter`, `$FFFFED92`, `FadeControl`,
//! `CredText_A`, `CredText_B`, `$FFFFECF2`), fixed-point cameras
//! (`Camera_Y_Pos_FG`/`BG`), scratch-RAM mappings (`$FFFF1000..$FFFF2380`),
//! staged and visible Plane A/B nametables, `RunText2` VRAM glyph assignments,
//! `CreditCommands1`/`2` execution and `CreditsFadeInOut` CRAM writes. The
//! Godot shell only draws the visible planes from [`EndingCreditsView`] and
//! live [`crate::CramView`].

use psiv_core::{CreditsPlane, PresentationOp};
use psiv_data::EndingCreditsAssets;

use crate::cram::Cram;

const PLANE_WORDS: usize = 64 * 32;
const PLANE_A_BASE: u32 = 0xFFFF_8000;
const SCRATCH_BASE: u32 = 0xFFFF_1000;
/// Covers `$FFFF1000..$FFFF2400`: the five Enigma mappings plus the four
/// zeroed rows stages 2 and 3 read past `$FFFF2380` (`row_counter` 28..31).
const SCRATCH_WORDS: usize = 0x1400 / 2;
/// `Palette_Line_4` (`$FFFFFB60`): words 48..64.
const PALETTE_LINE_4_START: usize = 48;
/// `Palette_Line_4 + $04` (`$FFFFFB64`): the five header fade words.
const PALETTE_HEADER_START: usize = 50;
/// `Palette_Table_Buffer + $5E` (`$FFFFFB5E`): the `RunText2` name colour.
const PALETTE_TEXT_SLOT: usize = 47;
/// Initial `Camera_Y_Pos_BG` (`move.w #$FF20, (Camera_Y_Pos_BG).w`, `$079D30`).
const INITIAL_BG_Y: u32 = 0xFF20_0000;
/// `LoadWindowTiles` base tile offset (`addi.w #$680, d2`, `ps4.asm:141874`).
const WINDOW_TILE_BASE: u16 = 0x0680;
/// `LoadWindowTiles` palette/priority word (`move.w #$E000, d2`, `$07A33A`).
const HEADER_TILE_ATTR: u16 = 0xE000;

/// What the renderer reads: the visible Plane A/B nametables, vertical scroll
/// state, 2-cell column scroll mode and `RunText2` VRAM glyph slots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndingCreditsView {
    /// Visible Plane A nametable (`64×32` words, `$FFFF8000`).
    pub plane_a: Box<[u16; PLANE_WORDS]>,
    /// Visible Plane B nametable (`64×32` words, `$FFFF9000`).
    pub plane_b: Box<[u16; PLANE_WORDS]>,
    /// `RunText2` VRAM tile slots (`0..2048`): `Some((font_byte, half))` where
    /// `half` is `0` for the top 8×8 tile and `1` for the bottom 8×8 tile.
    pub runtext2_vram: Box<[Option<(u8, u8)>; 2048]>,
    /// Visible Plane A vertical scroll in pixels (`Camera_Y_Pos_FG >> 16`).
    pub fg_y_scroll: u16,
    /// Visible Plane B vertical scroll in pixels (`Camera_Y_Pos_BG >> 16`).
    pub bg_y_scroll: u16,
    /// VDP reg 11 `$8B04` 2-cell column vertical scroll: columns `0..16` of
    /// Plane A scroll by `fg_y_scroll`; columns `16..40` stay at `0`.
    pub column_scroll: bool,
    /// Active stage (`0` before stage 1, `1..=3` during stages, `3` after).
    pub stage: u8,
    /// Retail `TextCounter` (`$FFFFED90`).
    pub text_counter: u16,
    /// Retail row counter (`$FFFFED92`).
    pub row_counter: u16,
    /// Retail `FadeControl` (`$FFFFED95`).
    pub fade_control: u8,
    /// Retail `CredText_A` (`$FFFFED96`).
    pub cred_text_a: u16,
    /// Retail `CredText_B` (`$FFFFED98`).
    pub cred_text_b: u16,
    /// Retail `$FFFFECF2` header fade step (`0..8`).
    pub fade_step: u8,
    /// Increments whenever visible planes, scroll, column mode or `RunText2`
    /// VRAM changes, for renderer upload suppression.
    pub revision: u64,
}

impl EndingCreditsView {
    /// The `RunText2` glyph byte and half (`0` top, `1` bottom) uploaded at
    /// VRAM tile index `tile & 0x7FF`, if any.
    #[must_use]
    pub fn runtext2_tile(&self, tile: u16) -> Option<(u8, u8)> {
        self.runtext2_vram[usize::from(tile & 0x7FF)]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StageConfig {
    stage: u8,
    scroll_delay: u16,
    foreground_step: u32,
    background_step: u32,
    source_ram: u32,
    plane: CreditsPlane,
    commands_rom_addr: u32,
}

/// Retail credits state machine owned by [`crate::Runtime`].
#[derive(Debug, Clone)]
pub(crate) struct EndingCredits {
    view: EndingCreditsView,
    scratch_ram: Vec<u16>,
    plane_a_staged: Box<[u16; PLANE_WORDS]>,
    plane_b_staged: Box<[u16; PLANE_WORDS]>,
    camera_y_fg: u32,
    camera_y_bg: u32,
    stage: Option<StageConfig>,
    /// True after a header command staged its tiles into `plane_a_staged` and
    /// paused for `LoadWindowTiles`'s `DMAPlane_A_VInt` (`loc_69AFA`).
    pending_header_dma: bool,
}

impl EndingCredits {
    /// Initializes `$079D12..$079E6F`: zeroes counters, seats `Camera_Y_Pos_BG`
    /// at `$FF20_0000`, unpacks the five mappings into scratch RAM, copies
    /// `$FFFF1A80` (`16×28`) to Plane B and transfers it (`$079E6A`).
    pub(crate) fn new(assets: &EndingCreditsAssets) -> Self {
        let mut scratch_ram = vec![0u16; SCRATCH_WORDS];
        for mapping in &assets.mappings {
            for &dest in &mapping.ram_destinations {
                if let Some(byte_off) = dest.checked_sub(SCRATCH_BASE) {
                    let start = (byte_off / 2) as usize;
                    let end = (start + mapping.words.len()).min(scratch_ram.len());
                    let count = end.saturating_sub(start);
                    scratch_ram[start..end].copy_from_slice(&mapping.words[..count]);
                }
            }
        }
        let plane_a_staged = Box::new([0u16; PLANE_WORDS]);
        let mut plane_b_staged = Box::new([0u16; PLANE_WORDS]);
        // `$079E54..$079E6A`: `PlaneMapToRAM($FFFF1A80 -> Plane_B_Buffer, 16, 28)`
        // followed by `DMAPlanes_VInt`.
        let init_start = ((0xFFFF_1A80u32 - SCRATCH_BASE) / 2) as usize;
        for row in 0..28 {
            let src = &scratch_ram[init_start + row * 16..init_start + (row + 1) * 16];
            plane_b_staged[row * 64..row * 64 + 16].copy_from_slice(src);
        }
        let view = EndingCreditsView {
            plane_a: Box::new([0u16; PLANE_WORDS]),
            plane_b: plane_b_staged.clone(),
            runtext2_vram: Box::new([None; 2048]),
            fg_y_scroll: 0,
            bg_y_scroll: (INITIAL_BG_Y >> 16) as u16,
            column_scroll: false,
            stage: 0,
            text_counter: 0,
            row_counter: 0,
            fade_control: 0,
            cred_text_a: 0,
            cred_text_b: 0,
            fade_step: 0,
            revision: 1,
        };
        Self {
            view,
            scratch_ram,
            plane_a_staged,
            plane_b_staged,
            camera_y_fg: 0,
            camera_y_bg: INITIAL_BG_Y,
            stage: None,
            pending_header_dma: false,
        }
    }

    /// Read-only view for the renderer and diagnostics.
    pub(crate) fn view(&self) -> &EndingCreditsView {
        &self.view
    }

    /// Starts one `EndingCreditsStage` and runs its first tick immediately.
    /// Returns `true` if the stage finished on entry (already at `scroll_delay`).
    pub(crate) fn begin_stage(
        &mut self,
        op: PresentationOp,
        main_frame_count: u16,
        assets: &EndingCreditsAssets,
        cram: &mut Cram,
        cycle_index: &mut u8,
    ) -> bool {
        let PresentationOp::EndingCreditsStage {
            stage,
            scroll_delay,
            foreground_step,
            background_step,
            source_ram,
            plane,
            commands_rom_addr,
        } = op
        else {
            return true;
        };
        if stage == 1 {
            // `$079E76..$079E88`: right after `Pal_FadeIn`, clear `Palette_Line_4`
            // (16 words), `Palette_Table_Buffer+$5E` (word 47) and `$FFFFECF2.w`.
            cram.write_slice(PALETTE_LINE_4_START, &[0u16; 16]);
            cram.write(PALETTE_TEXT_SLOT, 0);
            self.view.fade_step = 0;
            *cycle_index = 0;
        } else if stage == 2 {
            // `$079F26`: `move.w #$8B04, (a6)` (2-cell column VScroll) and
            // `clr.w ($FFFFED92).w` (row counter).
            self.view.column_scroll = true;
            self.view.row_counter = 0;
            self.bump_revision();
        }
        self.view.stage = stage;
        self.stage = Some(StageConfig {
            stage,
            scroll_delay,
            foreground_step,
            background_step,
            source_ram,
            plane,
            commands_rom_addr,
        });
        self.tick(main_frame_count, assets, cram)
    }

    /// Advances the active stage by one VBlank frame. Returns `true` without
    /// consuming a frame once `TextCounter == scroll_delay` at the loop head.
    pub(crate) fn tick(
        &mut self,
        main_frame_count: u16,
        assets: &EndingCreditsAssets,
        cram: &mut Cram,
    ) -> bool {
        let Some(stage) = self.stage else {
            return true;
        };
        if self.pending_header_dma {
            self.pending_header_dma = false;
            self.dma_plane_a();
            self.view.cred_text_a = self.view.cred_text_a.wrapping_add(1);
            self.view.fade_control |= 1 << 0;
            self.credits_fade_in_out(main_frame_count, assets, cram);
            return false;
        }
        if self.view.text_counter == stage.scroll_delay {
            self.stage = None;
            return true;
        }

        // 1. Advance cameras, TextCounter (bit 16) and row replacement (bit 19).
        let diff = if stage.stage == 1 {
            let old = self.camera_y_bg;
            self.camera_y_bg = self.camera_y_bg.wrapping_add(stage.background_step);
            self.camera_y_bg ^ old
        } else {
            let old = self.camera_y_fg;
            self.camera_y_fg = self.camera_y_fg.wrapping_add(stage.foreground_step);
            self.camera_y_bg = self.camera_y_bg.wrapping_add(stage.background_step);
            self.camera_y_fg ^ old
        };
        if diff & (1 << 16) != 0 {
            self.view.text_counter = self.view.text_counter.wrapping_add(1);
        }
        if diff & (1 << 19) != 0 {
            // Stage 1 adds 1 before indexing ($079EB6: `addq.w #1, d0`);
            // Stages 2 and 3 use `row_counter` directly ($079F7A, $07A032).
            let row_idx = if stage.stage == 1 {
                self.view.row_counter.wrapping_add(1)
            } else {
                self.view.row_counter
            };
            self.copy_scratch_row(stage.source_ram, row_idx, stage.plane);
            match stage.plane {
                CreditsPlane::A => self.view.fade_control |= 1 << 0,
                CreditsPlane::B => self.view.fade_control |= 1 << 1,
            }
            self.view.row_counter = self.view.row_counter.wrapping_add(1);
        }

        // 2. VInt / DMA branch (`loc_79EEE`, `loc_79FAE`, `loc_7A066`).
        if self.view.fade_control & (1 << 0) != 0 {
            self.view.fade_control &= !(1 << 0);
            self.dma_plane_a();
        } else if self.view.fade_control & (1 << 1) != 0 {
            self.view.fade_control &= !(1 << 1);
            self.dma_plane_b();
        }
        let fg_y = (self.camera_y_fg >> 16) as u16;
        let bg_y = (self.camera_y_bg >> 16) as u16;
        if self.view.fg_y_scroll != fg_y || self.view.bg_y_scroll != bg_y {
            self.view.fg_y_scroll = fg_y;
            self.view.bg_y_scroll = bg_y;
            self.bump_revision();
        }

        // 3. Stage 3 skips `CreditsLoop` and `CreditsFadeInOut` once bit 7 latches.
        if stage.stage == 3 && self.view.fade_control & (1 << 7) != 0 {
            return false;
        }

        // 4. `CreditsLoop` (`$07A2B4`). Header commands pause before `loc_69AFA`'s
        // `DMAPlane_A_VInt`, so `CreditsFadeInOut` runs after that frame completes.
        if self.credits_loop(stage.commands_rom_addr, assets) {
            self.pending_header_dma = true;
            return false;
        }

        // 5. `CreditsFadeInOut` (`$07A55E`).
        self.credits_fade_in_out(main_frame_count, assets, cram);
        false
    }

    fn bump_revision(&mut self) {
        self.view.revision = self.view.revision.wrapping_add(1);
    }

    fn dma_plane_a(&mut self) {
        if *self.view.plane_a != *self.plane_a_staged {
            self.view.plane_a.copy_from_slice(&*self.plane_a_staged);
            self.bump_revision();
        }
    }

    fn dma_plane_b(&mut self) {
        if *self.view.plane_b != *self.plane_b_staged {
            self.view.plane_b.copy_from_slice(&*self.plane_b_staged);
            self.bump_revision();
        }
    }

    fn copy_scratch_row(&mut self, source_ram: u32, row_idx: u16, plane: CreditsPlane) {
        let Some(base_bytes) = source_ram.checked_sub(SCRATCH_BASE) else {
            return;
        };
        let src_start = (base_bytes / 2) as usize + usize::from(row_idx) * 16;
        let dst_row = usize::from(row_idx & 0x1F) * 64;
        let dst = match plane {
            CreditsPlane::A => &mut self.plane_a_staged[dst_row..dst_row + 16],
            CreditsPlane::B => &mut self.plane_b_staged[dst_row..dst_row + 16],
        };
        for (i, slot) in dst.iter_mut().enumerate() {
            *slot = self.scratch_ram.get(src_start + i).copied().unwrap_or(0);
        }
    }

    /// `trap #0` over `Plane_A_Buffer`: clears `(d7 + 1) * 2` words starting at
    /// `ram_addr`, wrapping inside the 4 KiB Plane A buffer.
    fn clear_plane_a_longs(&mut self, ram_addr: u32, d7: usize) {
        let Some(byte_off) = ram_addr.checked_sub(PLANE_A_BASE) else {
            return;
        };
        let start = (byte_off / 2) as usize;
        let words = (d7 + 1) * 2;
        for i in 0..words {
            self.plane_a_staged[(start + i) % PLANE_WORDS] = 0;
        }
    }

    /// `LoadWindowTiles` (`ps4.asm:141835`) with `Camera_X/Y_Pos_FG == 0`,
    /// `d2 = $E000` (`HEADER_TILE_ATTR`), `d4 = 1`.
    fn load_window_tiles(&mut self, col: usize, row: usize, bytes: &[u8]) {
        let row_base = (row & 0x1F) * 64;
        let mut x = col & 0x3F;
        for &byte in bytes {
            if byte == 0xFF {
                break;
            }
            let mut tile = u16::from(byte);
            if byte & 0x80 != 0 {
                tile = tile.wrapping_add(0xC0);
            }
            let word = HEADER_TILE_ATTR
                .wrapping_add(WINDOW_TILE_BASE)
                .wrapping_add(tile);
            self.plane_a_staged[row_base + x] = word;
            x = (x + 1) & 0x3F;
        }
    }

    /// `RunText2` (`ps4.asm:143694`) with `d4 = 1`: clears 32 cells one row
    /// above, maps 32 vertical pairs of tiles at `tile`, blanks the 64 VRAM
    /// tile slots and uploads each glyph byte as top/bottom 8×8 halves.
    fn run_text2(&mut self, plane_ram: u32, tile: u16, bytes: &[u8]) {
        let Some(byte_off) = plane_ram.checked_sub(PLANE_A_BASE) else {
            return;
        };
        let origin = ((byte_off / 2) as usize) % PLANE_WORDS;
        let row = origin / 64;
        let col = origin % 64;
        let cell = |dy: usize, dx: usize| ((row + dy) % 32) * 64 + (col + dx) % 64;
        for x in 0..32 {
            self.plane_a_staged[cell(31, x)] = 0;
            for half in 0..2 {
                self.plane_a_staged[cell(half, x)] = tile.wrapping_add((x * 2 + half) as u16);
            }
        }
        let base = tile & 0x7FF;
        let mut vram_changed = false;
        for offset in 0..64u16 {
            let slot = usize::from((base + offset) & 0x7FF);
            if self.view.runtext2_vram[slot].is_some() {
                self.view.runtext2_vram[slot] = None;
                vram_changed = true;
            }
        }
        for (x, &byte) in bytes
            .iter()
            .take_while(|&&b| b != 0xFF)
            .take(32)
            .enumerate()
        {
            for half in 0..2u8 {
                let slot = usize::from((base + (x * 2 + usize::from(half)) as u16) & 0x7FF);
                if self.view.runtext2_vram[slot] != Some((byte, half)) {
                    self.view.runtext2_vram[slot] = Some((byte, half));
                    vram_changed = true;
                }
            }
        }
        if vram_changed {
            self.bump_revision();
        }
    }

    /// `CreditsLoop` (`$07A2B4`). Returns `true` when a header command ran and
    /// needs `LoadWindowTiles`'s `DMAPlane_A_VInt` frame before finishing.
    fn credits_loop(&mut self, commands_rom_addr: u32, assets: &EndingCreditsAssets) -> bool {
        let Some(table) = assets
            .commands
            .iter()
            .find(|table| table.rom_addr == commands_rom_addr)
        else {
            return false;
        };
        let counter = (self.view.text_counter & 0x3F) as u8;
        let (pairs, _) = table.bytes.as_chunks::<2>();
        let mut opcode = None;
        for &[at, cmd] in pairs {
            if at == counter {
                opcode = Some(cmd);
                break;
            }
            if at == 0xFF {
                break;
            }
        }
        let Some(cmd) = opcode else {
            return false;
        };
        let header_idx = usize::from(self.view.cred_text_a >> 3);
        let name_base = usize::from(self.view.cred_text_b >> 3) * 4;
        match cmd {
            0 => {
                // `CreditsShowHeaderText` (`$07A320`)
                self.clear_plane_a_longs(0xFFFF_82A2, 0x17);
                if let Some(header) = assets.headers.get(header_idx) {
                    self.load_window_tiles(0x11, 0x05, &header.bytes);
                }
                true
            }
            1 => {
                // `CreditsShowNamesLine1` (`$07A436`)
                if let Some(name) = assets.names.get(name_base) {
                    self.run_text2(0xFFFF_84A8, 0x4300, &name.bytes);
                }
                self.view.fade_control |= 1 << 0;
                false
            }
            2 => {
                // `CreditsShowNamesLine2` (`$07A458`)
                if let Some(name) = assets.names.get(name_base + 1) {
                    self.run_text2(0xFFFF_86A8, 0x4380, &name.bytes);
                }
                self.view.fade_control |= 1 << 0;
                false
            }
            3 => {
                // `CreditsShowNamesLine3` (`$07A47C`)
                if let Some(name) = assets.names.get(name_base + 2) {
                    self.run_text2(0xFFFF_88A8, 0x4400, &name.bytes);
                }
                self.view.fade_control |= 1 << 0;
                false
            }
            4 => {
                // `CreditsShowNamesLine4` (`$07A4A0`)
                if let Some(name) = assets.names.get(name_base + 3) {
                    self.run_text2(0xFFFF_8AA8, 0x4480, &name.bytes);
                }
                self.view.cred_text_b = self.view.cred_text_b.wrapping_add(1);
                self.view.fade_control |= 1 << 0;
                false
            }
            5 => {
                // `CreditsFadeInFirstBlock` (`$07A4C8`)
                self.view.fade_control |= 1 << 2;
                self.view.fade_step = 0;
                false
            }
            6 => {
                // `CreditsFadeInSecondBlock` (`$07A4D4`)
                self.view.fade_control |= 1 << 3;
                false
            }
            7 => {
                // `CreditsFadeOutFirstBlock` (`$07A4DC`)
                self.view.fade_control |= 1 << 4;
                self.view.fade_step = 0;
                false
            }
            8 => {
                // `CreditsFadeOutSecondBlock` (`$07A4E8`)
                self.view.fade_control |= 1 << 5;
                false
            }
            9 => {
                // `CreditsShowHeaderOffset` (`$07A360`)
                self.clear_plane_a_longs(0xFFFF_82A2, 0x17);
                self.clear_plane_a_longs(0xFFFF_86A2, 0x17);
                if let Some(header) = assets.headers.get(header_idx) {
                    self.load_window_tiles(0x11, 0x0D, &header.bytes);
                }
                true
            }
            10 => {
                // `CreditsShowHeaderAsSecondLine` (`$07A3AA`)
                self.clear_plane_a_longs(0xFFFF_86A2, 0x17);
                self.clear_plane_a_longs(0xFFFF_8A26, 0x13);
                if let Some(header) = assets.headers.get(header_idx) {
                    self.load_window_tiles(0x13, 0x14, &header.bytes);
                }
                true
            }
            11 => {
                // `CreditsShowHeaderAsThirdLine` (`$07A3F4`)
                self.clear_plane_a_longs(0xFFFF_8B34, 9);
                if let Some(header) = assets.headers.get(header_idx) {
                    self.load_window_tiles(0x1A, 0x16, &header.bytes);
                }
                true
            }
            12 => {
                // `CreditsEndCreditsSequence` (`$07A4F0`)
                self.view.fade_control |= 1 << 6;
                false
            }
            _ => false,
        }
    }

    /// `CreditsFadeInOut` (`$07A55E..$07A789`): tests bits 2..6 of the initial
    /// `FadeControl` snapshot in order.
    fn credits_fade_in_out(
        &mut self,
        main_frame_count: u16,
        assets: &EndingCreditsAssets,
        cram: &mut Cram,
    ) {
        let bits = self.view.fade_control;
        let low = main_frame_count & 0xFF;
        // Bit 2 (`loc_7A58E`): header fade in, every 4th frame.
        if bits & (1 << 2) != 0 && low & 3 == 0 {
            let row = usize::from(self.view.fade_step);
            if let Some(slice) = assets.fade.words.get(row * 5..row * 5 + 5) {
                cram.write_slice(PALETTE_HEADER_START, slice);
            }
            self.view.fade_step = self.view.fade_step.wrapping_add(1) & 7;
            if self.view.fade_step == 0 {
                self.view.fade_control &= !(1 << 2);
            }
        }
        // Bit 3 (`loc_7A5E4`): names fade in (`+$222`), every 8th frame.
        if bits & (1 << 3) != 0 && low & 7 == 0 {
            let next = cram.view().words[PALETTE_TEXT_SLOT].wrapping_add(0x222);
            if next & (1 << 12) != 0 {
                self.view.fade_control &= !(1 << 3);
            } else {
                cram.write(PALETTE_TEXT_SLOT, next);
            }
        }
        // Bit 4 (`loc_7A60A`): header fade out, every 2nd frame.
        if bits & (1 << 4) != 0 && low & 1 == 0 {
            let row = usize::from(7u8.wrapping_sub(self.view.fade_step));
            if let Some(slice) = assets.fade.words.get(row * 5..row * 5 + 5) {
                cram.write_slice(PALETTE_HEADER_START, slice);
            }
            self.view.fade_step = self.view.fade_step.wrapping_add(1) & 7;
            if self.view.fade_step == 0 {
                self.view.fade_control &= !(1 << 4);
                self.clear_plane_a_longs(0xFFFF_82A2, 0x17);
                self.view.fade_control |= 1 << 0;
            }
        }
        // Bit 5 (`loc_7A672`): names fade out (`-$222`), every 2nd frame.
        if bits & (1 << 5) != 0 && low & 1 == 0 {
            let current = cram.view().words[PALETTE_TEXT_SLOT];
            if current == 0 {
                self.view.fade_control &= !(1 << 5);
                for addr in [
                    0xFFFF_84A8,
                    0xFFFF_8528,
                    0xFFFF_86A8,
                    0xFFFF_8728,
                    0xFFFF_88A8,
                    0xFFFF_8928,
                    0xFFFF_8AA8,
                    0xFFFF_8B28,
                ] {
                    self.clear_plane_a_longs(addr, 0x14);
                }
                self.view.fade_control |= 1 << 0;
            } else {
                cram.write(PALETTE_TEXT_SLOT, current.wrapping_sub(0x222));
            }
        }
        // Bit 6 (`loc_7A72E`): final header fade in, latches bit 7 on wrap.
        if bits & (1 << 6) != 0 && low & 3 == 0 {
            let row = usize::from(self.view.fade_step);
            if let Some(slice) = assets.fade.words.get(row * 5..row * 5 + 5) {
                cram.write_slice(PALETTE_HEADER_START, slice);
            }
            self.view.fade_step = self.view.fade_step.wrapping_add(1) & 7;
            if self.view.fade_step == 0 {
                self.view.fade_control &= !(1 << 2);
                self.view.fade_control |= 1 << 7;
            }
        }
    }
}

#[cfg(test)]
mod tests;
