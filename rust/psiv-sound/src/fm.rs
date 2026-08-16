//! Safe wrapper around the Nuked OPN2 YM2612-compatible core.
//!
//! The translation unit is already present in the repository's Genesis Plus
//! GX oracle source. `build.rs` compiles that upstream source with its
//! YM2612 mode enabled and links only this small ownership/rendering wrapper.
//! The Rust driver therefore emits the same register contract that a future
//! oracle trace will compare, while the oscillator itself is not a hand-wavy
//! sine approximation.

#[repr(C)]
struct PsivYm {
    _private: [u8; 0],
}

unsafe extern "C" {
    fn psiv_ym_new(sample_rate: u32) -> *mut PsivYm;
    fn psiv_ym_free(ym: *mut PsivYm);
    fn psiv_ym_reset(ym: *mut PsivYm);
    fn psiv_ym_write_register(ym: *mut PsivYm, port: u8, register: u8, value: u8);
    fn psiv_ym_render_sample(ym: *mut PsivYm, out: *mut i16);
}

/// A YM2612 register surface backed by Nuked OPN2 in YM2612 mode.
pub struct Ym2612 {
    chip: *mut PsivYm,
}

impl Ym2612 {
    pub(crate) fn new(_sample_rate: u32) -> Self {
        let chip = unsafe { psiv_ym_new(_sample_rate) };
        assert!(!chip.is_null(), "Nuked OPN2 allocation failed");
        Self { chip }
    }

    pub(crate) fn reset(&mut self) {
        unsafe { psiv_ym_reset(self.chip) };
    }

    pub(crate) fn write(&mut self, port: u8, register: u8, value: u8) {
        unsafe { psiv_ym_write_register(self.chip, port, register, value) };
    }

    pub(crate) fn render_sample(&mut self) -> [f32; 2] {
        let mut sample = [0_i16; 2];
        unsafe { psiv_ym_render_sample(self.chip, sample.as_mut_ptr()) };
        [
            f32::from(sample[0]) / 32_768.0,
            f32::from(sample[1]) / 32_768.0,
        ]
    }
}

impl Drop for Ym2612 {
    fn drop(&mut self) {
        unsafe { psiv_ym_free(self.chip) };
    }
}

#[cfg(test)]
mod tests {
    use super::Ym2612;

    #[test]
    fn nuked_core_produces_a_signal_after_key_on() {
        let mut ym = Ym2612::new(44_100);
        ym.write(0, 0xb0, 0x32);
        ym.write(0, 0xb4, 0xc0);
        ym.write(0, 0xa4, 0x22);
        ym.write(0, 0xa0, 0x1a);
        ym.write(0, 0x28, 0xf0);
        let mut peak = 0.0_f32;
        for _ in 0..2_000 {
            let [left, right] = ym.render_sample();
            peak = peak.max(left.abs()).max(right.abs());
        }
        assert!(peak > 0.001);
    }
}
