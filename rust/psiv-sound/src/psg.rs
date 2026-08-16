//! Small SN76489-compatible PSG used by the PSIV driver.
//!
//! The PSG has no FM-sized accuracy dependency: its latch/data protocol,
//! divide-by-32 tone counters, attenuation table, and 15-bit noise LFSR are
//! compact enough to keep here. Register writes remain visible in the shared
//! trace before they reach this renderer.

use std::f32::consts::PI;

const PSG_CLOCK_HZ: f32 = 3_579_545.0;
const MAX_VOLUME: f32 = 0.18;

#[derive(Clone, Copy, Debug)]
struct ToneChannel {
    period: u16,
    attenuation: u8,
    phase: f32,
}

impl Default for ToneChannel {
    fn default() -> Self {
        Self {
            period: 1,
            attenuation: 15,
            phase: 0.0,
        }
    }
}

/// A direct SN76489 protocol/core implementation.
pub struct Sn76489 {
    sample_rate: f32,
    tones: [ToneChannel; 3],
    noise_attenuation: u8,
    noise_control: u8,
    noise_phase: f32,
    lfsr: u16,
    latch: usize,
}

impl Sn76489 {
    pub(crate) fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate: sample_rate as f32,
            tones: [ToneChannel::default(); 3],
            noise_attenuation: 15,
            noise_control: 0,
            noise_phase: 0.0,
            lfsr: 0x4000,
            latch: 0,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.tones = [ToneChannel::default(); 3];
        self.noise_attenuation = 15;
        self.noise_control = 0;
        self.noise_phase = 0.0;
        self.lfsr = 0x4000;
        self.latch = 0;
    }

    /// Consumes one SN76489 latch/data byte.
    pub(crate) fn write(&mut self, value: u8) {
        if value & 0x80 != 0 {
            self.latch = usize::from((value >> 5) & 0x03);
            let is_volume = value & 0x10 != 0;
            let data = value & 0x0f;
            if is_volume {
                self.set_volume(self.latch, data);
            } else if self.latch < 3 {
                self.tones[self.latch].period =
                    (self.tones[self.latch].period & 0x3f0) | u16::from(data);
            } else {
                self.noise_control = data & 0x07;
                self.lfsr = 0x4000;
                self.noise_phase = 0.0;
            }
            return;
        }

        let data = value & 0x3f;
        if self.latch < 3 {
            self.tones[self.latch].period =
                (self.tones[self.latch].period & 0x00f) | (u16::from(data) << 4);
        }
    }

    pub(crate) fn render_sample(&mut self) -> [f32; 2] {
        let mut output = 0.0;
        for channel in &mut self.tones {
            let frequency = PSG_CLOCK_HZ / (32.0 * f32::from(channel.period.max(1)));
            channel.phase = (channel.phase + frequency / self.sample_rate) % 1.0;
            let square = if channel.phase < 0.5 { 1.0 } else { -1.0 };
            output += square * attenuation(channel.attenuation);
        }

        let noise_period = match self.noise_control & 0x03 {
            0 => 16.0,
            1 => 32.0,
            2 => 64.0,
            _ => f32::from(self.tones[2].period.max(1)),
        };
        let noise_frequency = PSG_CLOCK_HZ / (32.0 * noise_period);
        self.noise_phase += noise_frequency / self.sample_rate;
        while self.noise_phase >= 1.0 {
            self.noise_phase -= 1.0;
            let feedback = if self.noise_control & 0x04 != 0 {
                (self.lfsr ^ (self.lfsr >> 1)) & 1
            } else {
                self.lfsr & 1
            };
            self.lfsr = (self.lfsr >> 1) | (feedback << 14);
        }
        let noise = if self.lfsr & 1 != 0 { 1.0 } else { -1.0 };
        output += noise * attenuation(self.noise_attenuation);

        let output = (output * PI.recip()).clamp(-1.0, 1.0);
        [output, output]
    }

    fn set_volume(&mut self, channel: usize, attenuation_value: u8) {
        if channel < 3 {
            self.tones[channel].attenuation = attenuation_value.min(15);
        } else {
            self.noise_attenuation = attenuation_value.min(15);
        }
    }
}

fn attenuation(value: u8) -> f32 {
    MAX_VOLUME * 10.0_f32.powf(-f32::from(value.min(15)) / 20.0)
}

#[cfg(test)]
mod tests {
    use super::Sn76489;

    #[test]
    fn tone_latch_produces_a_signal() {
        let mut psg = Sn76489::new(44_100);
        psg.write(0x80 | 0x02);
        psg.write(0x90);
        let mut peak = 0.0_f32;
        for _ in 0..1_000 {
            let [left, right] = psg.render_sample();
            peak = peak.max(left.abs()).max(right.abs());
        }
        assert!(peak > 0.01);
    }

    #[test]
    fn volume_latch_is_channel_local() {
        let mut psg = Sn76489::new(44_100);
        psg.write(0x90);
        psg.write(0xb0 | 0x0f);
        let loud = psg.render_sample()[0].abs();
        psg.write(0x9f);
        let quiet = psg.render_sample()[0].abs();
        assert!(loud >= quiet);
    }
}
