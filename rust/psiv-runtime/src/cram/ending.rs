//! `Cutscene_Ending`'s eighth-frame palette loops on the CRAM model: the
//! staff roll's word fade out and back in (`loc_79C8E`, `loc_79CCA`) and the
//! credits' table ramp (`loc_7A0A6`). The arithmetic and timing are
//! [`psiv_core::palette`]'s; here they become a [`Program::Writes`] started on
//! the op's frame, and the frames each answers release the scene.

use psiv_core::PresentationOp;
use psiv_core::palette;
use psiv_data::ScenePalettes;

use super::{Cram, PALETTE_TABLE_BUFFER, Program};

impl Cram {
    /// Starts a staff-roll word loop's writes; answers its frames.
    pub(super) fn word_loop(
        &mut self,
        slot: usize,
        fade: palette::WordLoop,
        frame: u16,
        cycle_index: &mut u8,
    ) -> u16 {
        let writes = fade
            .writes
            .into_iter()
            .map(|(at, word)| (at, slot, vec![word]))
            .collect();
        self.start(Program::Writes { elapsed: 0, writes }, frame, cycle_index);
        fade.frames
    }

    /// `loc_7A0A6`: the steps' copies from the pack's two tables on their
    /// frames. The frames it answers depend on the frame count alone, so a
    /// table the pack lacks (listed in [`super::CramView::missing`]) still releases
    /// the scene on the cartridge's frame, with no write.
    pub(super) fn credits_ramp(
        &mut self,
        op: PresentationOp,
        frame: u16,
        palettes: &ScenePalettes,
        cycle_index: &mut u8,
    ) -> u16 {
        let PresentationOp::EndingCreditsPaletteRamp {
            primary_rom_addr,
            primary_ram,
            primary_words,
            secondary_rom_addr,
            secondary_ram,
            secondary_words,
            secondary_from,
            steps,
        } = op
        else {
            return 0;
        };
        let (at, frames) = palette::credits_ramp_frames(frame, steps);
        let slot = |ram: u32| {
            ram.checked_sub(PALETTE_TABLE_BUFFER)
                .map(|b| b as usize / 2)
        };
        // The deepest reads: primary word `steps` on, and the last step's
        // secondary pair.
        let primary_need = usize::from(steps) + usize::from(primary_words);
        let secondary_need = usize::from(steps)
            .checked_sub(usize::from(secondary_from) + 1)
            .map_or(0, |late| 2 * late + usize::from(secondary_words));
        let primary = self.rom(palettes, primary_rom_addr, primary_need);
        let secondary = if secondary_need == 0 {
            Some(&[][..])
        } else {
            self.rom(palettes, secondary_rom_addr, secondary_need)
        };
        let (Some(primary), Some(secondary), Some(primary_slot), Some(secondary_slot)) =
            (primary, secondary, slot(primary_ram), slot(secondary_ram))
        else {
            return frames;
        };
        let Some(rows) = palette::credits_ramp_rows(
            primary,
            primary_words,
            secondary,
            secondary_words,
            secondary_from,
            steps,
        ) else {
            return frames;
        };
        let mut writes = Vec::new();
        for (row, at) in rows.into_iter().zip(at) {
            writes.push((at, primary_slot, row.primary));
            if let Some(late) = row.secondary {
                writes.push((at, secondary_slot, late));
            }
        }
        self.start(Program::Writes { elapsed: 0, writes }, frame, cycle_index);
        frames
    }
}
