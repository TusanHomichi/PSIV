//! Cartridge DAC playback.
//!
//! The PS4 driver feeds signed 8-bit samples to YM2612 register `$2A`. The
//! embedded Z80 uses the 29-entry pitch jump table from `ps4.dac_driver.asm`;
//! this renderer keeps that table's relative timing and performs the same
//! volume-control sign/magnitude transform before writing the YM register.

use std::collections::BTreeMap;

use crate::{Chip, RegisterLog, RegisterWrite, Ym2612};

const DAC_Z80_CLOCK_HZ: f64 = 3_579_545.0;
const PITCH_DELAYS: [u8; 29] = [
    24, 23, 22, 21, 20, 18, 18, 18, 17, 16, 15, 14, 13, 12, 11, 11, 10, 9, 9, 8, 8, 8, 6, 7, 6, 5,
    6, 5, 2,
];

struct ActiveSample {
    bytes: Vec<u8>,
    cursor: usize,
    phase: f64,
}

/// A YM2612 DAC stream driven by the extracted sample bank.
pub(crate) struct DacPlayer {
    samples: BTreeMap<u8, Vec<u8>>,
    active: Option<ActiveSample>,
    volume_control: u8,
    loop_enabled: bool,
    pan: u8,
    reverse: bool,
    volume: u8,
    pitch: u8,
}

impl DacPlayer {
    pub(crate) fn new() -> Self {
        Self {
            samples: BTreeMap::new(),
            active: None,
            volume_control: 0,
            loop_enabled: false,
            pan: 0xc0,
            reverse: false,
            volume: 0x10,
            pitch: 0x81,
        }
    }

    pub(crate) fn load_samples(&mut self, samples: impl IntoIterator<Item = (u8, Vec<u8>)>) {
        self.samples = samples.into_iter().collect();
        self.active = None;
    }

    pub(crate) fn set_volume_control(&mut self, value: u8) {
        self.volume_control = value;
    }

    pub(crate) fn set_loop(&mut self, value: u8) {
        self.loop_enabled = value != 0;
    }

    pub(crate) fn set_pan(&mut self, value: u8) {
        self.pan = value;
    }

    pub(crate) fn set_reverse(&mut self, value: u8) {
        self.reverse = value != 0;
    }

    pub(crate) fn set_volume(&mut self, value: u8) {
        self.volume = value;
    }

    pub(crate) fn set_pitch(&mut self, value: u8) {
        self.pitch = value;
    }

    pub(crate) fn trigger(
        &mut self,
        encoded_id: u8,
        ym: &mut Ym2612,
        log: &mut RegisterLog,
        tick: u64,
    ) {
        let id = if encoded_id < 0x80 {
            encoded_id | 0x80
        } else {
            encoded_id
        };
        let Some(bytes) = self.samples.get(&id).cloned() else {
            self.active = None;
            return;
        };
        if bytes.is_empty() {
            self.active = None;
            return;
        }
        let cursor = if self.reverse { bytes.len() - 1 } else { 0 };
        self.active = Some(ActiveSample {
            bytes,
            cursor,
            phase: 0.0,
        });
        write_ym(ym, log, tick, 0x2b, 0x80);
        write_ym(ym, log, tick, 0xb6, self.pan);
    }

    pub(crate) fn stop(&mut self) {
        self.active = None;
    }

    /// Emit zero or more PCM bytes for one native-rate output frame.
    pub(crate) fn render(&mut self, ym: &mut Ym2612, log: &mut RegisterLog, tick: u64) {
        let volume_control = self.volume_control;
        let volume = self.volume;
        let loop_enabled = self.loop_enabled;
        let reverse = self.reverse;
        let pitch = self.pitch;
        let Some(active) = self.active.as_mut() else {
            return;
        };
        let delay = PITCH_DELAYS
            .get(usize::from(pitch.saturating_sub(0x81)))
            .copied()
            .unwrap_or(PITCH_DELAYS[0]);
        let sample_hz = DAC_Z80_CLOCK_HZ / (75.0 + f64::from(delay) * 4.0);
        active.phase += sample_hz / f64::from(crate::SAMPLE_RATE);
        let mut finished = false;
        while active.phase >= 1.0 {
            active.phase -= 1.0;
            let raw = active.bytes[active.cursor];
            let value = transform(raw, volume_control, volume);
            write_ym(ym, log, tick, 0x2a, value);
            if !advance(active, reverse, loop_enabled) {
                finished = true;
                break;
            }
        }
        if finished {
            self.active = None;
        }
    }
}

fn transform(raw: u8, volume_control: u8, volume: u8) -> u8 {
    if volume_control & 0x10 != 0 {
        return raw;
    }
    let magnitude = (!raw) & 0x7f;
    let scaled = (u16::from(magnitude) * u16::from(volume & 0x0f) / 16) as u8;
    let centered = 0x80_u8.saturating_add(scaled);
    if raw & 0x80 == 0 { !centered } else { centered }
}

fn advance(active: &mut ActiveSample, reverse: bool, loop_enabled: bool) -> bool {
    if reverse {
        if active.cursor > 0 {
            active.cursor -= 1;
            return true;
        }
    } else if active.cursor + 1 < active.bytes.len() {
        active.cursor += 1;
        return true;
    }
    if loop_enabled {
        active.cursor = if reverse { active.bytes.len() - 1 } else { 0 };
        true
    } else {
        false
    }
}

fn write_ym(ym: &mut Ym2612, log: &mut RegisterLog, tick: u64, register: u8, value: u8) {
    ym.write(0, register, value);
    log.push(RegisterWrite {
        tick,
        chip: Chip::Ym2612,
        port: 0,
        register,
        value,
    });
}
