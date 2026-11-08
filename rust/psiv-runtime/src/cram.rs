//! The palette state: `Palette_Table_Buffer` (`$FFFFFB00`, the 64 words `VInt`
//! uploads to CRAM) and its copy `Palette_Table_Buffer_2` (`$FFFFFB80`).
//!
//! [`Cram`] is the one writer. A map load fills both buffers from the record's
//! 64-word shadow (`loc_53F14` and the field load's `Pal_FadeIn` copy); the map
//! update programs write single words; and every palette record a scene emits
//! is applied here, through the arithmetic in [`psiv_core::palette`]. A scene
//! loop that spends frames (a tone, a restore, a flash, a cycle, a fade) is a
//! running program the runtime steps once per tick with that tick's
//! `Main_Frame_Count`, starting on the tick the scene emits it: the frame the
//! cartridge's loop body first runs. Words a scene reads from the ROM come from
//! the pack (`presentation/panels.json`); a palette the pack lacks is listed in
//! [`CramView::missing`] rather than guessed.
//!
//! Deviations, recorded in `docs/scenes/SCENE_PRESENTATION.md` ("CRAM state"):
//! `Pal_FadeIn`/`PalFadeOut_ClrSpriteTbl` (and `InitVRAMAndCRAM`'s fade) write
//! only their second-buffer copy here, because the scene does not spend their
//! frames and the shell's cover draws the ramp; the live words keep the
//! pre-fade palette instead of reaching black.

use psiv_core::palette::{self, PALETTE_WORDS};
use psiv_core::{PaletteBuffer, PaletteHold, PaletteSource, PresentationOp, SceneOp};
use psiv_data::{MapRecord, ScenePalettes};

/// `Pal_Init` (`$09F2BC`), the 64 words `InitVRAMAndCRAM` loads (`$05A68A`).
const PAL_INIT: u32 = 0x0009_F2BC;
/// `Palette_Table_Buffer`'s work-RAM address.
const PALETTE_TABLE_BUFFER: u32 = 0xFFFF_FB00;

/// What the renderer reads: the live palette and a counter that changes
/// whenever any word of it does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CramView {
    /// `Palette_Table_Buffer`, the words `VInt` uploads to CRAM.
    pub words: [u16; PALETTE_WORDS],
    /// Changes to `words`, for renderer upload suppression. Never reset: a
    /// map load continues the count.
    pub revision: u64,
    /// ROM palettes a scene asked for that the pack does not carry, by
    /// address, in the order they were asked for.
    pub missing: Vec<u32>,
    /// Whether `words` are the cartridge's: the map record carries the
    /// 64-word shadow a load fills them from. A legacy record has none, and
    /// the renderer draws the colours baked into the pack's pictures.
    pub live: bool,
}

impl Default for CramView {
    fn default() -> Self {
        Self {
            words: [0; PALETTE_WORDS],
            revision: 0,
            missing: Vec::new(),
            live: false,
        }
    }
}

/// A scene loop that spends frames.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Program {
    /// `Pal_IncreaseTone`, then the hold's `trap #1`, every loop frame.
    Tone {
        left: u16,
        hold: Option<(usize, Vec<u16>)>,
    },
    /// `loc_58666` every loop frame.
    Restore { left: u16 },
    /// `loc_6D3BA` or `loc_6D40A` over the first `words` words.
    Flash {
        brighten: bool,
        words: usize,
        left: u16,
    },
    /// Literal writes at frame offsets from the start: `(offset, first word,
    /// words)`, in offset order.
    Writes {
        elapsed: u16,
        writes: Vec<(u16, usize, Vec<u16>)>,
    },
    /// Whole-palette passes: the first at `first`, then every `every`
    /// frames, `count` in all.
    Passes {
        kind: Pass,
        elapsed: u16,
        first: u16,
        every: u16,
        count: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pass {
    VariableOut,
    VariableIn,
    ToRed,
    FromRed,
}

/// The palette buffers and the scene loop stepping them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Cram {
    view: CramView,
    saved: [u16; PALETTE_WORDS],
    program: Option<Program>,
}

impl Default for Cram {
    fn default() -> Self {
        Self {
            view: CramView::default(),
            saved: [0; PALETTE_WORDS],
            program: None,
        }
    }
}

fn words_of(slice: &[u16]) -> [u16; PALETTE_WORDS] {
    let mut words = [0; PALETTE_WORDS];
    for (word, &value) in words.iter_mut().zip(slice) {
        *word = value;
    }
    words
}

impl Cram {
    /// A map load: both buffers hold the record's 64-word shadow (lines 0,
    /// 1 and 3 from `Map_Palettes_Addr`, line 2 `Pal_Init_Line_3`). A legacy
    /// record without a 64-word one loads black and is not [`CramView::live`].
    pub(crate) fn loaded(record: &MapRecord) -> Self {
        let shadow = record
            .map_update_palette
            .as_deref()
            .filter(|words| words.len() == PALETTE_WORDS);
        let words = shadow.map(words_of).unwrap_or([0; PALETTE_WORDS]);
        Self {
            view: CramView {
                words,
                live: shadow.is_some(),
                ..CramView::default()
            },
            saved: words,
            program: None,
        }
    }

    /// A `MapDataManager` routine's palette copy (`move.l (a0)+, (a1)+` into
    /// `Palette_Table_Buffer`), made after `loc_53F14` fills the buffer and
    /// before the load's `Pal_FadeIn` copies it to `Palette_Table_Buffer_2`
    /// (`ps4.asm:107579-107630`). So both buffers hold it.
    pub(crate) fn map_load_copy(&mut self, start: usize, values: &[u16]) {
        self.write_run(start, values);
        self.saved = self.view.words;
    }

    /// Continues `previous`'s revision count past this load, so a renderer
    /// that cached the old palette uploads the new one.
    pub(crate) fn follow(&mut self, previous: &Cram) {
        self.view.revision = previous.view.revision.wrapping_add(1);
    }

    pub(crate) fn view(&self) -> &CramView {
        &self.view
    }

    /// `Palette_Table_Buffer_2`.
    #[cfg(test)]
    pub(crate) fn saved(&self) -> &[u16; PALETTE_WORDS] {
        &self.saved
    }

    /// Whether a scene loop is still stepping the palette.
    #[cfg(test)]
    pub(crate) fn running(&self) -> bool {
        self.program.is_some()
    }

    /// One word of the live buffer, as a map update program writes it.
    pub(crate) fn write(&mut self, slot: usize, word: u16) {
        if let Some(current) = self.view.words.get_mut(slot)
            && *current != word
        {
            *current = word;
            self.view.revision = self.view.revision.wrapping_add(1);
        }
    }

    fn set_live(&mut self, words: [u16; PALETTE_WORDS]) {
        if self.view.words != words {
            self.view.words = words;
            self.view.revision = self.view.revision.wrapping_add(1);
        }
    }

    fn write_run(&mut self, start: usize, values: &[u16]) {
        let mut words = self.view.words;
        for (offset, &value) in values.iter().enumerate() {
            if let Some(word) = words.get_mut(start + offset) {
                *word = value;
            }
        }
        self.set_live(words);
    }

    fn rom<'a>(&mut self, palettes: &'a ScenePalettes, at: u32, words: usize) -> Option<&'a [u16]> {
        match palettes.words(at) {
            Some(found) if found.len() >= words => Some(&found[..words]),
            _ => {
                self.view.missing.push(at);
                None
            }
        }
    }

    /// Steps the running program by one frame; the runtime calls it once at
    /// the top of every tick.
    pub(crate) fn tick(&mut self, main_frame_count: u16) {
        let Some(program) = self.program.take() else {
            return;
        };
        self.program = self.step(program, main_frame_count);
    }

    /// Starts `program` and runs its first frame now: the scene emitted it on
    /// this tick, which is the loop's first frame.
    fn start(&mut self, program: Program, main_frame_count: u16) {
        self.program = self.step(program, main_frame_count);
    }

    fn step(&mut self, program: Program, frame: u16) -> Option<Program> {
        let phase = palette::tone_phase(frame);
        match program {
            Program::Tone { left, hold } => {
                if left == 0 {
                    return None;
                }
                let mut words = self.view.words;
                if phase {
                    for word in &mut words {
                        *word = palette::increase_tone(*word);
                    }
                }
                if let Some((start, held)) = &hold {
                    for (offset, &value) in held.iter().enumerate() {
                        if let Some(word) = words.get_mut(start + offset) {
                            *word = value;
                        }
                    }
                }
                self.set_live(words);
                (left > 1).then_some(Program::Tone {
                    left: left - 1,
                    hold,
                })
            }
            Program::Restore { left } => {
                if left == 0 {
                    return None;
                }
                if phase {
                    let mut words = self.view.words;
                    for (word, &target) in words.iter_mut().zip(&self.saved) {
                        *word = palette::restore_tone(*word, target);
                    }
                    self.set_live(words);
                }
                (left > 1).then_some(Program::Restore { left: left - 1 })
            }
            Program::Flash {
                brighten,
                words: count,
                left,
            } => {
                if left == 0 {
                    return None;
                }
                if phase {
                    let mut words = self.view.words;
                    for (word, &target) in words.iter_mut().zip(&self.saved).take(count) {
                        *word = if brighten {
                            palette::flash_brighten(*word)
                        } else {
                            palette::flash_restore(*word, target)
                        };
                    }
                    self.set_live(words);
                }
                (left > 1).then_some(Program::Flash {
                    brighten,
                    words: count,
                    left: left - 1,
                })
            }
            Program::Writes { elapsed, writes } => {
                for (_, start, values) in writes.iter().filter(|(at, ..)| *at == elapsed) {
                    self.write_run(*start, values);
                }
                let last = writes.last().map_or(0, |(at, ..)| *at);
                (elapsed < last).then_some(Program::Writes {
                    elapsed: elapsed + 1,
                    writes,
                })
            }
            Program::Passes {
                kind,
                elapsed,
                first,
                every,
                count,
            } => {
                let last = first + every * count.saturating_sub(1);
                if elapsed >= first && (elapsed - first).is_multiple_of(every) && elapsed <= last {
                    let mut words = self.view.words;
                    for (word, &target) in words.iter_mut().zip(&self.saved) {
                        *word = match kind {
                            Pass::VariableOut => palette::fade_out_step(*word),
                            Pass::VariableIn => palette::fade_in_step(*word, target).0,
                            Pass::ToRed => palette::fade_to_red_step(*word),
                            Pass::FromRed => palette::fade_from_red_step(*word, target),
                        };
                    }
                    self.set_live(words);
                }
                (elapsed < last).then_some(Program::Passes {
                    kind,
                    elapsed: elapsed + 1,
                    first,
                    every,
                    count,
                })
            }
        }
    }

    fn copy(
        &mut self,
        source: PaletteSource,
        destination: PaletteBuffer,
        start: usize,
        count: usize,
        palettes: &ScenePalettes,
    ) {
        let values: Vec<u16> = match source {
            PaletteSource::Buffer(PaletteBuffer::Live) => self
                .view
                .words
                .iter()
                .skip(start)
                .take(count)
                .copied()
                .collect(),
            PaletteSource::Buffer(PaletteBuffer::Saved) => {
                self.saved.iter().skip(start).take(count).copied().collect()
            }
            PaletteSource::Rom(at) => match self.rom(palettes, at, count) {
                Some(found) => found.to_vec(),
                None => return,
            },
            PaletteSource::Fill(value) => vec![value; count],
        };
        match destination {
            PaletteBuffer::Live => self.write_run(start, &values),
            PaletteBuffer::Saved => {
                for (offset, value) in values.into_iter().enumerate() {
                    if let Some(word) = self.saved.get_mut(start + offset) {
                        *word = value;
                    }
                }
            }
        }
    }

    /// Applies one scene record on the tick the scene emits it. `map` is the
    /// current map's record, for `ReloadMapPalette`. Returns the frames a
    /// [`PresentationOp::VariablePaletteFade`] spends, which the scene waits
    /// for; `None` for every other record.
    pub(crate) fn apply(
        &mut self,
        op: &SceneOp,
        main_frame_count: u16,
        palettes: &ScenePalettes,
        map: Option<&MapRecord>,
    ) -> Option<u16> {
        match *op {
            // `InitVRAMAndCRAM`: `PalFadeOut_ClrSpriteTbl` copies the
            // buffer to the second one, then 32 longwords of `Pal_Init` (`$05A68A`).
            SceneOp::InitVramAndCram => {
                self.saved = self.view.words;
                if let Some(init) = self.rom(palettes, PAL_INIT, PALETTE_WORDS) {
                    let init = words_of(init);
                    self.set_live(init);
                }
            }
            // Both copy the buffer to the second one first (`trap #1`, 64
            // words); the ramp itself is the shell's cover (module docs).
            SceneOp::FadeIn | SceneOp::FadeOut => self.saved = self.view.words,
            SceneOp::LoadPalette {
                rom_addr,
                words,
                start,
            } => {
                if let Some(found) = self.rom(palettes, rom_addr, usize::from(words)) {
                    let found = found.to_vec();
                    self.write_run(usize::from(start), &found);
                }
            }
            // `loc_53F14`: the map's 64-word shadow into the live buffer.
            SceneOp::ReloadMapPalette => {
                if let Some(words) = map.and_then(|record| record.map_update_palette.as_deref()) {
                    self.set_live(words_of(words));
                }
            }
            // `Panel_Draw` (`ps4.asm:144392-144401`): each drawn layer's
            // sixteen palette words at its art tile's line.
            SceneOp::PanelCreate { id } => {
                let layers = palettes.panel(id).map(<[_]>::to_vec).unwrap_or_default();
                for layer in layers {
                    if let Some(found) = self.rom(palettes, layer.rom_addr, 16) {
                        let found = found.to_vec();
                        self.write_run(layer.start(), &found);
                    }
                }
            }
            SceneOp::Presentation { op } => {
                return self.presentation(op, main_frame_count, palettes);
            }
            _ => {}
        }
        None
    }

    /// `Panel_Create` for a dialogue `$F2` action's panel: the same writes as
    /// a scene's.
    pub(crate) fn panel(&mut self, id: u16, palettes: &ScenePalettes) {
        let _ = self.apply(&SceneOp::PanelCreate { id }, 0, palettes, None);
    }

    fn presentation(
        &mut self,
        op: PresentationOp,
        frame: u16,
        palettes: &ScenePalettes,
    ) -> Option<u16> {
        match op {
            PresentationOp::SetPaletteWords {
                offset,
                first,
                second,
            } => self.write_run(usize::from(offset), &[first, second]),
            // `addi.w #add, (Palette_Table_Buffer + 2*offset)`.
            PresentationOp::AddPaletteWord { offset, add } => {
                let slot = usize::from(offset);
                if let Some(&word) = self.view.words.get(slot) {
                    self.write(slot, word.wrapping_add(add));
                }
            }
            PresentationOp::PaletteCopy {
                source,
                destination,
                start,
                words,
            } => self.copy(
                source,
                destination,
                usize::from(start),
                usize::from(words),
                palettes,
            ),
            PresentationOp::PaletteIncreaseTone { frames, hold } => {
                let hold = hold.and_then(
                    |PaletteHold {
                         source_rom_addr,
                         destination_ram,
                         words,
                     }| {
                        let start = destination_ram.checked_sub(PALETTE_TABLE_BUFFER)? / 2;
                        let found = self.rom(palettes, source_rom_addr, usize::from(words))?;
                        Some((start as usize, found.to_vec()))
                    },
                );
                self.start(Program::Tone { left: frames, hold }, frame);
            }
            PresentationOp::PaletteRestoreTone { frames } => {
                self.start(Program::Restore { left: frames }, frame);
            }
            PresentationOp::PaletteToneFlash {
                brighten,
                words,
                frames,
            } => self.start(
                Program::Flash {
                    brighten,
                    words: usize::from(words),
                    left: frames,
                },
                frame,
            ),
            // `loc_78296`: nine words a frame, then the frame's `dbf` wait.
            PresentationOp::RykrosPaletteCycle {
                source_rom_addr,
                destination_ram,
                words_per_frame,
                delays,
            } => {
                let per = usize::from(words_per_frame);
                let start = destination_ram.checked_sub(PALETTE_TABLE_BUFFER)?;
                let table = self.rom(palettes, source_rom_addr, per * delays.len())?;
                let mut writes = Vec::with_capacity(delays.len());
                let mut at = 0u16;
                for (index, &delay) in delays.iter().enumerate() {
                    writes.push((at, start as usize / 2, table[index * per..][..per].to_vec()));
                    at += u16::from(delay);
                }
                self.start(Program::Writes { elapsed: 0, writes }, frame);
            }
            // `loc_6ED4E`: three words of line `destination_ram`'s words 1-3,
            // then `frame_delay` frames, `steps` times.
            PresentationOp::PaletteRampFromTable {
                source_rom_addr,
                destination_ram,
                steps,
                frame_delay,
            } => {
                let start = destination_ram.checked_sub(PALETTE_TABLE_BUFFER)? as usize / 2 + 1;
                let table = self.rom(palettes, source_rom_addr, usize::from(steps) + 14)?;
                let rows = palette::palette_ramp_writes(table, steps)?;
                let writes = rows
                    .into_iter()
                    .enumerate()
                    .map(|(n, row)| (n as u16 * frame_delay, start, row.to_vec()))
                    .collect();
                self.start(Program::Writes { elapsed: 0, writes }, frame);
            }
            // `loc_6E842`: two words a pass, then `frame_delay` frames.
            PresentationOp::PaletteCycleFromTable {
                source_rom_addr,
                offset,
                second_skip,
                steps,
                frame_delay,
            } => {
                let table = match palettes.words(source_rom_addr) {
                    Some(table) => table,
                    None => {
                        self.view.missing.push(source_rom_addr);
                        return None;
                    }
                };
                let Some(pairs) = palette::palette_cycle_writes(table, second_skip, steps) else {
                    self.view.missing.push(source_rom_addr);
                    return None;
                };
                let writes = pairs
                    .into_iter()
                    .enumerate()
                    .map(|(n, (first, second))| {
                        (
                            n as u16 * frame_delay,
                            usize::from(offset),
                            vec![first, second],
                        )
                    })
                    .collect();
                self.start(Program::Writes { elapsed: 0, writes }, frame);
            }
            // `Pal_VariableFadeOut`/`In` (`$05B168`, `$05B1B0`).
            PresentationOp::VariablePaletteFade { fade_in, mode } => {
                self.saved = self.view.words;
                let every = palette::dbf_frames(mode);
                let (kind, first, count, frames) = if fade_in {
                    // `trap #0` clears the buffer, `VDP_EnableDisplay` spends
                    // one frame, then the passes.
                    self.set_live([0; PALETTE_WORDS]);
                    let count = palette::variable_fade_in_passes(&self.saved);
                    let frames = palette::variable_fade_in_frames(&self.saved, mode);
                    (Pass::VariableIn, 1, count, frames)
                } else {
                    let count = palette::variable_fade_out_passes(&self.view.words);
                    let frames = palette::variable_fade_out_frames(&self.view.words, mode);
                    (Pass::VariableOut, 0, count, frames)
                };
                self.start(
                    Program::Passes {
                        kind,
                        elapsed: 0,
                        first,
                        every,
                        count,
                    },
                    frame,
                );
                return Some(frames);
            }
            // `Pal_VariableFadeToRed` copies the buffer first; both run eight
            // stages of `delay + 1` frames.
            PresentationOp::FadeToRed { lines } | PresentationOp::FadeFromRed { lines } => {
                let to_red = matches!(op, PresentationOp::FadeToRed { .. });
                if to_red {
                    self.saved = self.view.words;
                }
                self.start(
                    Program::Passes {
                        kind: if to_red { Pass::ToRed } else { Pass::FromRed },
                        elapsed: 0,
                        first: 0,
                        every: palette::dbf_frames(lines),
                        count: palette::RED_FADE_STAGES,
                    },
                    frame,
                );
            }
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests;
