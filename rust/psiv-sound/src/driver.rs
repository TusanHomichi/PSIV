//! PSIV's modified-SMPS track scheduler and command interpreter.
//!
//! The extraction lane feeds this module raw track bytes plus the local
//! voice/envelope tables. No JSON, ROM offsets, or psiv_tools types belong in
//! the runtime crate. The command lengths and meanings below follow
//! `reference/ps4disasm/sound/DefCFlag.txt`; unsupported future data fails
//! into a recorded error instead of silently changing the write stream.

use std::collections::BTreeMap;

use crate::dac::DacPlayer;
use crate::sequence::{SequenceError, SoundBank, SoundSequence, SoundTrack, TrackKind};
use crate::{Chip, RegisterLog, RegisterWrite};
use crate::{Sn76489, Ym2612};

use crate::sequence::{FM_CHANNELS, PSG_CHANNELS};
#[path = "commands.rs"]
mod commands;
const CALL_STACK: usize = 4;
const COMMAND_BUDGET: usize = 128;
const SAMPLES_PER_TICK: usize = crate::SAMPLE_RATE as usize / crate::DRIVER_TICK_HZ as usize;

const FM_FREQS: [u16; 12] = [
    0x25e, 0x284, 0x2ab, 0x2d3, 0x2fe, 0x32d, 0x35c, 0x38f, 0x3c5, 0x3ff, 0x43c, 0x47c,
];
const PSG_FREQS: [u16; 96] = [
    0x356, 0x326, 0x2f9, 0x2ce, 0x2a5, 0x280, 0x25c, 0x23a, 0x21a, 0x1fb, 0x1df, 0x1c4, 0x1ab,
    0x193, 0x17d, 0x167, 0x153, 0x140, 0x12e, 0x11d, 0x10d, 0x0fe, 0x0ef, 0x0e2, 0x0d6, 0x0c9,
    0x0be, 0x0b4, 0x0a9, 0x0a0, 0x097, 0x08f, 0x087, 0x07f, 0x078, 0x071, 0x06b, 0x065, 0x05f,
    0x05a, 0x055, 0x050, 0x04b, 0x047, 0x043, 0x040, 0x03c, 0x039, 0x036, 0x033, 0x030, 0x02d,
    0x02b, 0x028, 0x026, 0x024, 0x022, 0x020, 0x01f, 0x01d, 0x01b, 0x01a, 0x018, 0x017, 0x016,
    0x015, 0x013, 0x012, 0x011, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000,
    0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000,
    0x000, 0x000, 0x000, 0x000, 0x000,
];
const FM_OPERATOR_REGS: [u8; 20] = [
    0x30, 0x38, 0x34, 0x3c, 0x50, 0x58, 0x54, 0x5c, 0x60, 0x68, 0x64, 0x6c, 0x70, 0x78, 0x74, 0x7c,
    0x80, 0x88, 0x84, 0x8c,
];
const FM_VOLUME_REGS: [u8; 4] = [0x40, 0x48, 0x44, 0x4c];

struct TrackState {
    source: SoundTrack,
    sequence_id: u8,
    is_sfx: bool,
    suppressed: bool,
    channel: u8,
    cursor: usize,
    delay: u16,
    note_stop: u8,
    hold: bool,
    active: bool,
    frequency: u16,
    transpose: i8,
    detune: i8,
    volume_delta: i8,
    pan: u8,
    psg_volume: i8,
    psg_envelope: Option<usize>,
    envelope_cursor: usize,
    fm_levels: [u8; 4],
    communication: u8,
    loop_counters: [u8; 8],
    call_stack: [usize; CALL_STACK],
    call_depth: usize,
    modulation: Option<Modulation>,
    initialized: bool,
    dac_volume_control: u8,
    dac_loop: u8,
    dac_pan: u8,
    dac_sample: u8,
    dac_reverse: u8,
    dac_volume: u8,
    dac_mode: u8,
    lfo_ams: u8,
    modulation_type: u8,
}

#[derive(Clone, Copy)]
struct Modulation {
    wait: u8,
    step_wait: u8,
    step: i8,
    steps: u8,
    offset: i16,
}

impl TrackState {
    fn new(source: SoundTrack, channel: u8, sequence_id: u8, is_sfx: bool) -> Self {
        let initial_volume = source.initial_volume;
        let psg_envelope = source.psg_envelope;
        let start_offset = source.start_offset;
        Self {
            sequence_id,
            is_sfx,
            suppressed: false,
            transpose: source.initial_transpose,
            pan: source.initial_pan,
            source,
            channel,
            cursor: start_offset,
            delay: 0,
            note_stop: 0,
            hold: false,
            active: true,
            frequency: 0,
            detune: 0,
            volume_delta: initial_volume,
            psg_volume: initial_volume,
            psg_envelope,
            envelope_cursor: 0,
            fm_levels: [0x10; 4],
            communication: 0,
            loop_counters: [0; 8],
            call_stack: [0; CALL_STACK],
            call_depth: 0,
            modulation: None,
            initialized: false,
            dac_volume_control: 0,
            dac_loop: 0,
            dac_pan: 0xc0,
            dac_sample: 0,
            dac_reverse: 0,
            dac_volume: 0x10,
            dac_mode: 0,
            lfo_ams: 0,
            modulation_type: 0,
        }
    }
}

struct TickContext {
    tempo: u8,
    pending_sound: Option<u8>,
    error: Option<SequenceError>,
}

struct ChipBus<'a> {
    tick: u64,
    ym: &'a mut Ym2612,
    psg: &'a mut Sn76489,
    log: &'a mut RegisterLog,
    dac: &'a mut DacPlayer,
}

/// The stateful scheduler in front of the two chip cores.
pub(crate) struct Driver {
    sequences: BTreeMap<u8, SoundSequence>,
    music_id: Option<u8>,
    tracks: Vec<TrackState>,
    pending_sound: Option<u8>,
    started: bool,
    pub(crate) samples_until_tick: usize,
    tick_count: u64,
    tempo: u8,
    last_error: Option<SequenceError>,
    dac: DacPlayer,
}

impl Driver {
    pub(crate) fn new() -> Self {
        Self {
            sequences: BTreeMap::new(),
            music_id: None,
            tracks: Vec::new(),
            pending_sound: None,
            started: false,
            samples_until_tick: 0,
            tick_count: 0,
            tempo: 1,
            last_error: None,
            dac: DacPlayer::new(),
        }
    }

    pub(crate) fn load(&mut self, sequence: SoundSequence) {
        self.last_error = sequence.validate().err();
        self.tempo = sequence.tempo.max(1);
        self.sequences.clear();
        self.music_id = Some(sequence.id);
        self.sequences.insert(sequence.id, sequence);
        self.reset_playback();
        self.dac.load_samples(std::iter::empty());
    }

    pub(crate) fn load_bank(&mut self, bank: SoundBank) {
        self.sequences.clear();
        self.music_id = None;
        self.last_error = None;
        for sequence in bank.sequences {
            if self.last_error.is_none() {
                self.last_error = sequence.validate().err();
            }
            self.sequences.insert(sequence.id, sequence);
        }
        self.reset_playback();
        self.dac.load_samples(
            bank.dac_samples
                .into_iter()
                .map(|sample| (sample.id, sample.bytes)),
        );
    }

    pub(crate) fn start(&mut self) {
        let Some(id) = self.music_id else {
            self.started = false;
            return;
        };
        self.install_music(id);
    }

    pub(crate) fn queue_sound(&mut self, id: u8) {
        if self
            .pending_sound
            .is_none_or(|old| sound_priority(id) >= sound_priority(old))
        {
            self.pending_sound = Some(id);
        }
    }

    pub(crate) fn tick(&mut self, ym: &mut Ym2612, psg: &mut Sn76489, log: &mut RegisterLog) {
        self.samples_until_tick = SAMPLES_PER_TICK;
        if let Some(id) = self.pending_sound.take() {
            self.dispatch(id, ym, psg, log);
        }
        self.update_suppression(ym, psg, log);
        if !self.started {
            return;
        }
        let mut context = TickContext {
            tempo: self.tempo,
            pending_sound: None,
            error: None,
        };
        let sequences = &self.sequences;
        let mut bus = ChipBus {
            tick: self.tick_count,
            ym,
            psg,
            log,
            dac: &mut self.dac,
        };
        for state in &mut self.tracks {
            if !state.active || state.suppressed {
                continue;
            }
            let Some(sequence) = sequences.get(&state.sequence_id) else {
                context.error = Some(SequenceError::MissingSequence {
                    id: state.sequence_id,
                });
                state.active = false;
                continue;
            };
            tick_track(sequence, state, &mut context, &mut bus);
        }
        self.tracks.retain(|state| !state.is_sfx || state.active);
        self.tempo = context.tempo.max(1);
        if let Some(id) = context.pending_sound {
            self.queue_sound(id);
        }
        if context.error.is_some() {
            self.last_error = context.error;
        }
        self.started = self.tracks.iter().any(|state| state.active);
        self.update_suppression(ym, psg, log);
        self.tick_count = self.tick_count.saturating_add(1);
    }

    pub(crate) fn render_dac(&mut self, ym: &mut Ym2612, log: &mut RegisterLog) {
        self.dac.render(ym, log, self.tick_count);
    }

    pub(crate) fn tick_count(&self) -> u64 {
        self.tick_count
    }

    pub(crate) fn last_error(&self) -> Option<&SequenceError> {
        self.last_error.as_ref()
    }

    fn dispatch(&mut self, id: u8, ym: &mut Ym2612, psg: &mut Sn76489, log: &mut RegisterLog) {
        match id {
            0x00..=0x7f | 0xfe => self.stop_all(ym, psg, log),
            0x80 => {}
            0x81..=0xb4 if self.sequences.contains_key(&id) => {
                self.stop_all(ym, psg, log);
                self.install_music(id);
            }
            0xb5..=0xfa if self.sequences.contains_key(&id) => {
                self.start_sfx(id, ym, psg, log);
            }
            0xfb => {
                for state in &mut self.tracks {
                    state.volume_delta = state.volume_delta.saturating_add(1);
                }
            }
            0xfc | 0xfd => self.stop_sfx(ym, psg, log),
            0xff => {}
            0x81..=0xfa => {}
        }
    }

    fn stop_all(&mut self, ym: &mut Ym2612, psg: &mut Sn76489, log: &mut RegisterLog) {
        let mut bus = ChipBus {
            tick: self.tick_count,
            ym,
            psg,
            log,
            dac: &mut self.dac,
        };
        for state in &mut self.tracks {
            if state.active {
                note_off(state, &mut bus);
            }
            state.active = false;
        }
        self.dac.stop();
        self.tracks.clear();
        self.music_id = None;
        self.started = false;
    }

    fn reset_playback(&mut self) {
        self.tracks.clear();
        self.started = false;
        self.pending_sound = None;
        self.samples_until_tick = 0;
        self.tick_count = 0;
    }

    fn install_music(&mut self, id: u8) {
        let Ok(tracks) = self.build_tracks(id, false) else {
            self.started = false;
            return;
        };
        let Some(sequence) = self.sequences.get(&id) else {
            return;
        };
        self.music_id = Some(id);
        self.tempo = sequence.tempo.max(1);
        self.tracks = tracks;
        self.tick_count = 0;
        self.samples_until_tick = 0;
        self.started = true;
    }

    fn start_sfx(&mut self, id: u8, ym: &mut Ym2612, psg: &mut Sn76489, log: &mut RegisterLog) {
        let Ok(tracks) = self.build_tracks(id, true) else {
            return;
        };
        self.stop_sfx(ym, psg, log);
        self.tracks.extend(tracks);
        self.started = true;
        self.update_suppression(ym, psg, log);
    }

    fn stop_sfx(&mut self, ym: &mut Ym2612, psg: &mut Sn76489, log: &mut RegisterLog) {
        let mut bus = ChipBus {
            tick: self.tick_count,
            ym,
            psg,
            log,
            dac: &mut self.dac,
        };
        for state in &mut self.tracks {
            if state.is_sfx && state.active {
                note_off(state, &mut bus);
                state.active = false;
            }
        }
        self.tracks.retain(|state| !state.is_sfx);
        self.started = self.tracks.iter().any(|state| state.active);
    }

    fn build_tracks(&self, id: u8, is_sfx: bool) -> Result<Vec<TrackState>, SequenceError> {
        let Some(sequence) = self.sequences.get(&id) else {
            return Err(SequenceError::MissingSequence { id });
        };
        sequence.validate()?;
        let mut fm_used = [false; FM_CHANNELS];
        let mut psg_used = [false; PSG_CHANNELS];
        let mut tracks = Vec::with_capacity(sequence.tracks.len());
        for source in &sequence.tracks {
            let (used, limit) = match source.kind {
                TrackKind::Fm => (&mut fm_used[..], FM_CHANNELS),
                TrackKind::Psg => (&mut psg_used[..], PSG_CHANNELS),
            };
            let requested = source.channel.map(usize::from);
            let channel = requested
                .filter(|channel| *channel < limit && !used[*channel])
                .or_else(|| used.iter().position(|occupied| !occupied));
            let Some(channel) = channel else {
                return Err(SequenceError::InvalidChannel {
                    kind: source.kind,
                    channel: source.channel.unwrap_or(u8::MAX),
                });
            };
            used[channel] = true;
            tracks.push(TrackState::new(source.clone(), channel as u8, id, is_sfx));
        }
        Ok(tracks)
    }

    fn update_suppression(&mut self, ym: &mut Ym2612, psg: &mut Sn76489, log: &mut RegisterLog) {
        let mut fm = [false; FM_CHANNELS];
        let mut psg_channels = [false; PSG_CHANNELS];
        for state in &self.tracks {
            if state.is_sfx && state.active {
                match state.source.kind {
                    TrackKind::Fm => fm[usize::from(state.channel)] = true,
                    TrackKind::Psg => psg_channels[usize::from(state.channel)] = true,
                }
            }
        }
        let mut bus = ChipBus {
            tick: self.tick_count,
            ym,
            psg,
            log,
            dac: &mut self.dac,
        };
        for state in &mut self.tracks {
            if state.is_sfx {
                continue;
            }
            let suppressed = match state.source.kind {
                TrackKind::Fm => fm[usize::from(state.channel)],
                TrackKind::Psg => psg_channels[usize::from(state.channel)],
            };
            if suppressed == state.suppressed {
                continue;
            }
            if suppressed {
                note_off(state, &mut bus);
            } else {
                resume_track(state, &mut bus);
            }
            state.suppressed = suppressed;
        }
    }
}

fn sound_priority(id: u8) -> u8 {
    match id {
        0xfe => 0xff,
        0xfb..=0xfd => 0xf0,
        0xb5..=0xfa => 0xd0,
        0x81..=0xb4 => 0xc0,
        _ => 0x80,
    }
}

fn tick_track(
    sequence: &SoundSequence,
    state: &mut TrackState,
    context: &mut TickContext,
    bus: &mut ChipBus<'_>,
) {
    if !state.initialized {
        state.initialized = true;
        if let Some(index) = state.source.initial_voice
            && let Err(error) = apply_voice(sequence, state, usize::from(index), bus)
        {
            context.error = Some(error);
            state.active = false;
            return;
        }
    }
    if state.delay > 0 {
        state.delay -= 1;
        update_note_stop(state, bus);
        advance_psg_envelope(sequence, state, bus);
        if state.delay > 0 {
            advance_modulation(state, bus);
            return;
        }
    }

    state.hold = false;
    for _ in 0..COMMAND_BUDGET {
        let Some(&value) = state.source.bytes.get(state.cursor) else {
            state.active = false;
            return;
        };
        state.cursor += 1;
        if value >= 0xe0 {
            match commands::command(sequence, state, context, value, bus) {
                Ok(true) => continue,
                Ok(false) => return,
                Err(error) => {
                    context.error = Some(error);
                    state.active = false;
                    return;
                }
            }
        }

        note_off_if_needed(state, bus);
        if value < 0x80 {
            state.delay = duration(state, value, context.tempo);
            return;
        }
        if value == 0x80 {
            state.frequency = 0;
            if state.source.kind == TrackKind::Psg {
                emit_psg_volume(state, bus);
            }
        } else {
            match state.source.kind {
                TrackKind::Fm => {
                    if state.source.dac {
                        bus.dac.set_pitch(value);
                    } else {
                        state.frequency = fm_frequency(value, state.transpose, state.detune);
                        emit_fm_frequency(state, bus);
                        key_on(state, bus);
                    }
                }
                TrackKind::Psg => {
                    state.frequency = psg_frequency(value, state.transpose);
                    emit_psg_frequency(state, bus);
                    emit_psg_volume(state, bus);
                }
            }
        }
        state.delay = duration_from_stream(state, context.tempo);
        return;
    }
    context.error = Some(SequenceError::CommandBudgetExceeded {
        cursor: state.cursor,
    });
    state.active = false;
}

fn duration(state: &TrackState, value: u8, tempo: u8) -> u16 {
    u16::from(value.max(1))
        .saturating_mul(u16::from(state.source.tick_multiplier.max(1)))
        .saturating_mul(u16::from(tempo.max(1)))
}

fn duration_from_stream(state: &mut TrackState, tempo: u8) -> u16 {
    let stream_duration = state
        .source
        .bytes
        .get(state.cursor)
        .copied()
        .filter(|value| *value < 0x80)
        .map(|value| {
            state.cursor += 1;
            duration(state, value, tempo)
        });
    stream_duration.unwrap_or_else(|| duration(state, 1, tempo))
}

fn take_byte(state: &mut TrackState, opcode: u8) -> Result<u8, SequenceError> {
    let Some(value) = state.source.bytes.get(state.cursor).copied() else {
        return Err(SequenceError::UnexpectedEnd {
            command: opcode,
            cursor: state.cursor,
        });
    };
    state.cursor += 1;
    Ok(value)
}

fn jump(state: &mut TrackState, opcode: u8) -> Result<(), SequenceError> {
    let offset = i16::from_be_bytes([take_byte(state, opcode)?, take_byte(state, opcode)?]);
    let target = state.cursor as isize + isize::from(offset);
    if target < 0 || target as usize >= state.source.bytes.len() {
        return Err(SequenceError::InvalidJump {
            cursor: state.cursor,
            target,
        });
    }
    state.cursor = target as usize;
    Ok(())
}

fn loop_command(state: &mut TrackState, opcode: u8) -> Result<(), SequenceError> {
    let index = usize::from(take_byte(state, opcode)? & 0x07);
    let count = take_byte(state, opcode)?;
    let offset = i16::from_be_bytes([take_byte(state, opcode)?, take_byte(state, opcode)?]);
    if state.loop_counters[index] == 0 {
        state.loop_counters[index] = count;
    }
    state.loop_counters[index] = state.loop_counters[index].saturating_sub(1);
    if state.loop_counters[index] != 0 {
        let target = state.cursor as isize + isize::from(offset);
        if target < 0 || target as usize >= state.source.bytes.len() {
            return Err(SequenceError::InvalidJump {
                cursor: state.cursor,
                target,
            });
        }
        state.cursor = target as usize;
    }
    Ok(())
}

fn gosub(state: &mut TrackState, opcode: u8) -> Result<(), SequenceError> {
    let offset = i16::from_be_bytes([take_byte(state, opcode)?, take_byte(state, opcode)?]);
    if state.call_depth >= CALL_STACK {
        return Err(SequenceError::CallStackOverflow);
    }
    state.call_stack[state.call_depth] = state.cursor;
    state.call_depth += 1;
    let target = state.cursor as isize + isize::from(offset);
    if target < 0 || target as usize >= state.source.bytes.len() {
        return Err(SequenceError::InvalidJump {
            cursor: state.cursor,
            target,
        });
    }
    state.cursor = target as usize;
    Ok(())
}

fn return_command(state: &mut TrackState, opcode: u8) -> Result<(), SequenceError> {
    let _ = opcode;
    if state.call_depth == 0 {
        return Err(SequenceError::CallStackUnderflow);
    }
    state.call_depth -= 1;
    state.cursor = state.call_stack[state.call_depth];
    Ok(())
}

fn apply_voice(
    sequence: &SoundSequence,
    state: &mut TrackState,
    index: usize,
    bus: &mut ChipBus<'_>,
) -> Result<(), SequenceError> {
    if state.source.kind != TrackKind::Fm {
        return Ok(());
    }
    let Some(voice) = sequence.fm_voices.get(index) else {
        return Err(SequenceError::MissingVoice { index });
    };
    state.frequency = 0;
    state.fm_levels = voice.levels;
    let channel = fm_port_channel(state.channel).1;
    emit_fm(state, bus, 0xb0 + channel, voice.algorithm);
    for (register, value) in FM_OPERATOR_REGS.into_iter().zip(voice.operators) {
        emit_fm(state, bus, register, value);
    }
    for (register, value) in FM_VOLUME_REGS.into_iter().zip(voice.levels) {
        emit_fm(
            state,
            bus,
            register,
            adjust_volume(value, state.volume_delta),
        );
    }
    emit_fm(state, bus, 0xb4 + channel, state.pan);
    Ok(())
}

fn refresh_fm_volume(state: &TrackState, bus: &mut ChipBus<'_>) {
    for (register, level) in FM_VOLUME_REGS.into_iter().zip(state.fm_levels) {
        emit_fm(
            state,
            bus,
            register,
            adjust_volume(level, state.volume_delta),
        );
    }
}

fn adjust_volume(value: u8, delta: i8) -> u8 {
    i16::from(value)
        .saturating_add(i16::from(delta))
        .clamp(0, 127) as u8
}

fn fm_frequency(note: u8, transpose: i8, detune: i8) -> u16 {
    let note = (i16::from(note & 0x7f) - 0x80 + i16::from(transpose)).rem_euclid(128) as usize;
    let octave = note / 12;
    let semitone = note % 12;
    let base = u32::from(FM_FREQS[semitone]) + ((octave as u32) << 11);
    (base as i32 + i32::from(detune)).clamp(0, 0x0fff) as u16
}

fn psg_frequency(note: u8, transpose: i8) -> u16 {
    let note = (i16::from(note) - 0x81 + i16::from(transpose)).clamp(0, 95) as usize;
    PSG_FREQS[note]
}

fn fm_port_channel(channel: u8) -> (u8, u8) {
    (channel / 3, channel % 3)
}

fn note_off_if_needed(state: &mut TrackState, bus: &mut ChipBus<'_>) {
    if !state.hold && state.frequency != 0 {
        note_off(state, bus);
    }
}

fn note_off(state: &TrackState, bus: &mut ChipBus<'_>) {
    if state.source.dac {
        bus.dac.stop();
        return;
    }
    if state.source.kind == TrackKind::Fm {
        let key_channel = if state.channel < 3 {
            state.channel
        } else {
            4 + state.channel - 3
        };
        emit_ym(bus, 0, 0x28, key_channel);
    } else {
        emit_psg(state, bus, 0x90 | (state.channel << 5) | 0x0f);
    }
}

fn resume_track(state: &TrackState, bus: &mut ChipBus<'_>) {
    if state.frequency == 0 || state.source.dac {
        return;
    }
    match state.source.kind {
        TrackKind::Fm => {
            emit_fm_frequency(state, bus);
            key_on(state, bus);
        }
        TrackKind::Psg => {
            emit_psg_frequency(state, bus);
            emit_psg_volume(state, bus);
        }
    }
}

fn update_note_stop(state: &mut TrackState, bus: &mut ChipBus<'_>) {
    if state.note_stop > 0 {
        state.note_stop -= 1;
        if state.note_stop == 0 {
            note_off(state, bus);
        }
    }
}

fn advance_modulation(state: &mut TrackState, bus: &mut ChipBus<'_>) {
    let Some(mut modulation) = state.modulation else {
        return;
    };
    if modulation.wait > 0 {
        modulation.wait -= 1;
    } else if modulation.steps > 0 {
        if modulation.step_wait > 0 {
            modulation.step_wait -= 1;
        } else {
            modulation.offset += i16::from(modulation.step);
            modulation.steps -= 1;
            modulation.step_wait = 1;
            if state.source.kind == TrackKind::Fm && state.frequency != 0 {
                let fnum = (i32::from(state.frequency) + i32::from(modulation.offset))
                    .clamp(0, 0x0fff) as u16;
                let (_, channel) = fm_port_channel(state.channel);
                emit_fm(state, bus, 0xa4 + channel, (fnum >> 8) as u8);
                emit_fm(state, bus, 0xa0 + channel, fnum as u8);
            }
        }
    }
    state.modulation = Some(modulation);
}

fn advance_psg_envelope(sequence: &SoundSequence, state: &mut TrackState, bus: &mut ChipBus<'_>) {
    let Some(index) = state.psg_envelope else {
        return;
    };
    let Some(envelope) = sequence.psg_envelopes.get(index) else {
        return;
    };
    for _ in 0..8 {
        let Some(&control) = envelope.controls.get(state.envelope_cursor) else {
            return;
        };
        state.envelope_cursor += 1;
        match control {
            value if value < 0x80 => {
                state.psg_volume = state.psg_volume.saturating_add(value as i8);
                emit_psg_volume(state, bus);
                return;
            }
            0x80 => state.envelope_cursor = 0,
            0x81 => return,
            0x82 => {
                let Some(&index) = envelope.controls.get(state.envelope_cursor) else {
                    return;
                };
                state.envelope_cursor = usize::from(index);
            }
            0x83 => {
                state.psg_volume = 0x0f;
                emit_psg_volume(state, bus);
                return;
            }
            _ => return,
        }
    }
}

fn key_on(state: &TrackState, bus: &mut ChipBus<'_>) {
    let key_channel = if state.channel < 3 {
        state.channel
    } else {
        4 + state.channel - 3
    };
    emit_ym(bus, 0, 0x28, 0xf0 | key_channel);
}

fn emit_fm(state: &TrackState, bus: &mut ChipBus<'_>, register: u8, value: u8) {
    let (port, channel) = fm_port_channel(state.channel);
    let register = if (0x30..=0x8f).contains(&register) {
        register + channel
    } else {
        register
    };
    emit_ym(bus, port, register, value);
}

fn emit_fm_frequency(state: &TrackState, bus: &mut ChipBus<'_>) {
    let (_, channel) = fm_port_channel(state.channel);
    emit_fm(state, bus, 0xa4 + channel, (state.frequency >> 8) as u8);
    emit_fm(state, bus, 0xa0 + channel, state.frequency as u8);
}

fn emit_psg_frequency(state: &TrackState, bus: &mut ChipBus<'_>) {
    let channel = state.channel << 5;
    let low = 0x80 | channel | (state.frequency as u8 & 0x0f);
    let high = (state.frequency >> 4) as u8 & 0x3f;
    emit_psg(state, bus, low);
    emit_psg(state, bus, high);
}

fn emit_psg_volume(state: &TrackState, bus: &mut ChipBus<'_>) {
    let attenuation = i16::from(state.psg_volume).clamp(0, 15) as u8;
    emit_psg(state, bus, 0x90 | (state.channel << 5) | attenuation);
}

fn emit_ym(bus: &mut ChipBus<'_>, port: u8, register: u8, value: u8) {
    bus.ym.write(port, register, value);
    bus.log.push(RegisterWrite {
        tick: bus.tick,
        chip: Chip::Ym2612,
        port,
        register,
        value,
    });
}

fn emit_psg(_state: &TrackState, bus: &mut ChipBus<'_>, value: u8) {
    bus.psg.write(value);
    bus.log.push(RegisterWrite {
        tick: bus.tick,
        chip: Chip::Sn76489,
        port: 0,
        register: 0,
        value,
    });
}

#[cfg(test)]
#[path = "driver_tests.rs"]
mod driver_tests;
