//! Palette writes of the flight helpers, before their ordinary `Pal_FadeIn`.
//! US loc_641A2 copies the ship's sixteen words to line 3; loc_643B8
//! copies lines 2/4 and loc_644AE selects line 1 through loc_64550.

use psiv_data::ScenePalettes;

use super::Cram;

impl Cram {
    fn flight_line(&mut self, palettes: &ScenePalettes, address: u32, start: usize) {
        if let Some(words) = self.rom(palettes, address, 16) {
            self.write_run(start, words);
        }
    }

    /// loc_641A2 ($0641F6/$06421E), after RefreshMap on either flight leg.
    /// EventFlag_Snowstorm chooses the alternate ship; it is not a world test.
    pub(crate) fn flight_ship(&mut self, palettes: &ScenePalettes, snowstorm: bool) {
        self.flight_line(palettes, if snowstorm { 0x1D_E0D8 } else { 0x1D_E0F8 }, 32);
    }

    /// loc_643B8 and loc_644AE, after InitVRAMAndCRAM. Only these three
    /// lines change; line 3 remains Pal_Init, with the caption word cleared
    /// by the route immediately before Pal_FadeIn ($064C84).
    pub(crate) fn flight_planet(&mut self, palettes: &ScenePalettes, world: u8) {
        const WORLDS: [u32; 6] = [
            0x1D_E1D8, 0x1D_E1F8, 0x1D_E218, 0x1D_E198, 0x1D_E1B8, 0x1D_E238,
        ];
        self.flight_line(palettes, 0x1D_E138, 48);
        self.flight_line(palettes, 0x1D_E118, 16);
        if let Some(&address) = WORLDS.get(usize::from(world)) {
            self.flight_line(palettes, address, 0);
        }
    }
}
