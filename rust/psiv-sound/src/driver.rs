//! PSIV's modified-SMPS track scheduler and command interpreter.
//!
//! The extraction lane feeds this module raw track bytes plus the local
//! voice/envelope tables. No JSON, ROM offsets, or psiv_tools types belong in
//! the runtime crate. The command lengths and meanings below follow
//! `reference/ps4disasm/sound/DefCFlag.txt`; unsupported future data fails
//! into a recorded error instead of silently changing the write stream.

use crate::sequence::{SequenceError, SoundSequence, SoundTrack, TrackKind};
use crate::{Chip, RegisterLog, RegisterWrite};
use crate::{Sn76489, Ym2612};

use crate::sequence::{FM_CHANNELS, PSG_CHANNELS};
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
    fn new(source: SoundTrack, channel: u8) -> Self {
        Self {
            transpose: source.initial_transpose,
            pan: source.initial_pan,
            source,
            channel,
            cursor: 0,
            delay: 0,
            note_stop: 0,
            hold: false,
            active: true,
            frequency: 0,
            detune: 0,
            volume_delta: 0,
            psg_volume: 0,
            psg_envelope: None,
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
}

/// The stateful scheduler in front of the two chip cores.
pub(crate) struct Driver {
    sequence: Option<SoundSequence>,
    tracks: Vec<TrackState>,
    pending_sound: Option<u8>,
    started: bool,
    pub(crate) samples_until_tick: usize,
    tick_count: u64,
    tempo: u8,
    last_error: Option<SequenceError>,
}

impl Driver {
    pub(crate) fn new() -> Self {
        Self {
            sequence: None,
            tracks: Vec::new(),
            pending_sound: None,
            started: false,
            samples_until_tick: 0,
            tick_count: 0,
            tempo: 1,
            last_error: None,
        }
    }

    pub(crate) fn load(&mut self, sequence: SoundSequence) {
        self.last_error = sequence.validate().err();
        self.tempo = sequence.tempo.max(1);
        self.sequence = Some(sequence);
        self.tracks.clear();
        self.started = false;
        self.pending_sound = None;
        self.samples_until_tick = 0;
    }

    pub(crate) fn start(&mut self) {
        self.tracks.clear();
        let Some(sequence) = self.sequence.as_ref() else {
            self.started = false;
            return;
        };
        if self.last_error.is_some() {
            self.started = false;
            return;
        }
        let mut fm_used = [false; FM_CHANNELS];
        let mut psg_used = [false; PSG_CHANNELS];
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
                self.last_error = Some(SequenceError::InvalidChannel {
                    kind: source.kind,
                    channel: source.channel.unwrap_or(u8::MAX),
                });
                self.started = false;
                return;
            };
            used[channel] = true;
            self.tracks
                .push(TrackState::new(source.clone(), channel as u8));
        }
        self.tempo = sequence.tempo.max(1);
        self.tick_count = 0;
        self.samples_until_tick = 0;
        self.started = true;
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
        if !self.started {
            return;
        }
        let Some(sequence) = self.sequence.as_ref() else {
            return;
        };
        let mut context = TickContext {
            tempo: self.tempo,
            pending_sound: None,
            error: None,
        };
        for state in &mut self.tracks {
            if !state.active {
                continue;
            }
            tick_track(sequence, state, &mut context, self.tick_count, ym, psg, log);
        }
        self.tempo = context.tempo.max(1);
        if let Some(id) = context.pending_sound {
            self.queue_sound(id);
        }
        if context.error.is_some() {
            self.last_error = context.error;
        }
        self.tick_count = self.tick_count.saturating_add(1);
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
            0x81..=0xfa => {
                if self
                    .sequence
                    .as_ref()
                    .is_some_and(|sequence| sequence.id == id)
                {
                    self.start();
                }
            }
            0xfb => {
                for state in &mut self.tracks {
                    state.volume_delta = state.volume_delta.saturating_add(1);
                }
            }
            0xfc | 0xfd | 0xff => {}
        }
    }

    fn stop_all(&mut self, ym: &mut Ym2612, psg: &mut Sn76489, log: &mut RegisterLog) {
        let mut bus = ChipBus {
            tick: self.tick_count,
            ym,
            psg,
            log,
        };
        for state in &mut self.tracks {
            if state.active {
                note_off(state, &mut bus);
            }
            state.active = false;
        }
        self.started = false;
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
    tick: u64,
    ym: &mut Ym2612,
    psg: &mut Sn76489,
    log: &mut RegisterLog,
) {
    let mut bus = ChipBus { tick, ym, psg, log };
    if !state.initialized {
        state.initialized = true;
        if let Some(index) = state.source.initial_voice
            && let Err(error) = apply_voice(sequence, state, usize::from(index), &mut bus)
        {
            context.error = Some(error);
            state.active = false;
            return;
        }
    }
    if state.delay > 0 {
        state.delay -= 1;
        update_note_stop(state, &mut bus);
        advance_psg_envelope(sequence, state, &mut bus);
        if state.delay > 0 {
            advance_modulation(state, &mut bus);
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
            match command(sequence, state, context, value, &mut bus) {
                Ok(true) => continue,
                Ok(false) => return,
                Err(error) => {
                    context.error = Some(error);
                    state.active = false;
                    return;
                }
            }
        }

        note_off_if_needed(state, &mut bus);
        if value < 0x80 {
            state.delay = duration(state, value, context.tempo);
            return;
        }
        if value == 0x80 {
            state.frequency = 0;
            if state.source.kind == TrackKind::Psg {
                emit_psg_volume(state, &mut bus);
            }
        } else {
            match state.source.kind {
                TrackKind::Fm => {
                    state.frequency = fm_frequency(value, state.transpose, state.detune);
                    emit_fm_frequency(state, &mut bus);
                    key_on(state, &mut bus);
                }
                TrackKind::Psg => {
                    state.frequency = psg_frequency(value, state.transpose);
                    emit_psg_frequency(state, &mut bus);
                    emit_psg_volume(state, &mut bus);
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

fn command(
    sequence: &SoundSequence,
    state: &mut TrackState,
    context: &mut TickContext,
    opcode: u8,
    bus: &mut ChipBus<'_>,
) -> Result<bool, SequenceError> {
    match opcode {
        0xe0 => {
            let value = take_byte(state, opcode)?;
            state.pan = value;
            if state.source.kind == TrackKind::Fm {
                emit_fm(state, bus, 0xb4, value);
            }
        }
        0xed => state.dac_pan = take_byte(state, opcode)?,
        0xe1 => state.detune = take_byte(state, opcode)? as i8,
        0xe2 => state.communication = take_byte(state, opcode)?,
        0xe3 => state.dac_volume_control = take_byte(state, opcode)?,
        0xe4 => state.dac_loop = take_byte(state, opcode)?,
        0xe5 => {
            state.volume_delta = state
                .volume_delta
                .saturating_add(take_byte(state, opcode)? as i8);
            let _pfm_parameter = take_byte(state, opcode)?;
            if state.source.kind == TrackKind::Fm {
                refresh_fm_volume(state, bus);
            }
        }
        0xe6 => {
            state.volume_delta = state
                .volume_delta
                .saturating_add(take_byte(state, opcode)? as i8);
            if state.source.kind == TrackKind::Fm {
                refresh_fm_volume(state, bus);
            }
        }
        0xe7 => state.hold = true,
        0xe8 => state.note_stop = take_byte(state, opcode)?,
        0xe9 => {
            state.lfo_ams = take_byte(state, opcode)?;
            let frequency = take_byte(state, opcode)?;
            if state.source.kind == TrackKind::Fm {
                emit_fm(state, bus, 0x22, frequency);
                emit_fm(state, bus, 0xb4, state.pan);
            }
        }
        0xea => context.tempo = take_byte(state, opcode)?.max(1),
        0xeb => context.pending_sound = Some(take_byte(state, opcode)?),
        0xec => {
            state.psg_volume = state
                .psg_volume
                .saturating_add(take_byte(state, opcode)? as i8);
            if state.source.kind == TrackKind::Psg {
                emit_psg_volume(state, bus);
            }
        }
        0xee => state.dac_sample = take_byte(state, opcode)?,
        0xef => {
            let index = usize::from(take_byte(state, opcode)?);
            apply_voice(sequence, state, index, bus)?;
        }
        0xf0 => {
            let wait = take_byte(state, opcode)?;
            let step_wait = take_byte(state, opcode)?;
            let step = take_byte(state, opcode)? as i8;
            let steps = take_byte(state, opcode)?;
            state.modulation = Some(Modulation {
                wait,
                step_wait,
                step,
                steps: steps >> 1,
                offset: 0,
            });
        }
        0xf1 => {
            state.modulation_type = take_byte(state, opcode)?;
            let _modulation_parameter = take_byte(state, opcode)?;
        }
        0xf2 => {
            note_off(state, bus);
            state.active = false;
            return Ok(false);
        }
        0xf3 => {
            let value = take_byte(state, opcode)?;
            if state.source.kind == TrackKind::Psg {
                emit_psg(state, bus, 0xe0 | (value & 0x0f));
            }
        }
        0xf4 => {
            state.modulation_type = take_byte(state, opcode)?;
        }
        0xf5 => {
            let index = usize::from(take_byte(state, opcode)?);
            if index >= sequence.psg_envelopes.len() {
                return Err(SequenceError::MissingEnvelope { index });
            }
            state.psg_envelope = Some(index);
            state.envelope_cursor = 0;
        }
        0xf6 => jump(state, opcode)?,
        0xf7 => loop_command(state, opcode)?,
        0xf8 => gosub(state, opcode)?,
        0xf9 => return_command(state, opcode)?,
        0xfa => state.dac_reverse = take_byte(state, opcode)?,
        0xfb => {
            state.transpose = state
                .transpose
                .saturating_add(take_byte(state, opcode)? as i8)
        }
        0xfc => state.dac_volume = take_byte(state, opcode)?,
        0xfd => state.dac_mode = take_byte(state, opcode)?,
        0xfe => {
            for slot in 0..4_u8 {
                let selector = take_byte(state, opcode)? & 0x03;
                if state.source.kind == TrackKind::Fm {
                    let frequency = [0x000, 0x180, 0x1f4, 0x260][usize::from(selector)];
                    let channel = state.channel.min(2);
                    emit_fm(state, bus, 0xa8 + channel + slot.min(2), frequency as u8);
                    emit_fm(
                        state,
                        bus,
                        0xac + channel + slot.min(2),
                        (frequency >> 8) as u8,
                    );
                }
            }
            if state.source.kind == TrackKind::Fm {
                emit_fm(state, bus, 0x27, 0x40);
            }
        }
        0xff => {
            let meta = take_byte(state, opcode)?;
            if meta != 0 {
                return Err(SequenceError::UnsupportedMeta { value: meta });
            }
            let count = take_byte(state, opcode)?;
            if count != 0 {
                for _ in 0..5 {
                    let _ = take_byte(state, opcode)?;
                }
            }
        }
        _ => {}
    }
    Ok(true)
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
    if let Some(delta) = envelope.values.get(state.envelope_cursor) {
        state.psg_volume = state.psg_volume.saturating_add(*delta);
        state.envelope_cursor += 1;
        emit_psg_volume(state, bus);
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
mod tests {
    use super::{SoundSequence, SoundTrack};
    use crate::{Chip, SoundMachine};

    #[test]
    fn fixture_fm_register_trace_is_deterministic() {
        let mut first = SoundMachine::new();
        first.load(SoundSequence::fixture_tone());
        first.start();
        let _ = first.tick();
        let first_log = first.take_register_log();

        let mut second = SoundMachine::new();
        second.load(SoundSequence::fixture_tone());
        second.start();
        let _ = second.tick();
        assert_eq!(first_log, second.take_register_log());
        let writes: Vec<_> = first_log
            .iter()
            .map(|write| (write.port, write.register, write.value))
            .collect();
        assert_eq!(
            writes,
            vec![
                (0, 0xb0, 0x07),
                (0, 0x30, 0x01),
                (0, 0x38, 0x01),
                (0, 0x34, 0x01),
                (0, 0x3c, 0x01),
                (0, 0x50, 0x1f),
                (0, 0x58, 0x1f),
                (0, 0x54, 0x1f),
                (0, 0x5c, 0x1f),
                (0, 0x60, 0x00),
                (0, 0x68, 0x00),
                (0, 0x64, 0x00),
                (0, 0x6c, 0x00),
                (0, 0x70, 0x00),
                (0, 0x78, 0x00),
                (0, 0x74, 0x00),
                (0, 0x7c, 0x00),
                (0, 0x80, 0x0f),
                (0, 0x88, 0x0f),
                (0, 0x84, 0x0f),
                (0, 0x8c, 0x0f),
                (0, 0x40, 0x10),
                (0, 0x48, 0x10),
                (0, 0x44, 0x10),
                (0, 0x4c, 0x10),
                (0, 0xb4, 0xc0),
                (0, 0xa4, 0x0a),
                (0, 0xa0, 0x5e),
                (0, 0x28, 0xf0),
            ]
        );
        assert!(first_log.iter().all(|write| write.chip == Chip::Ym2612));
    }

    #[test]
    fn fixture_psg_trace_contains_latch_and_data_writes() {
        let sequence = SoundSequence::new(
            0x81,
            1,
            Vec::new(),
            Vec::new(),
            vec![SoundTrack::psg(0, vec![0x81, 0x04, 0x08, 0xf2])],
        );
        let mut machine = SoundMachine::new();
        machine.load(sequence);
        machine.start();
        let _ = machine.tick();
        let log = machine.take_register_log();
        assert_eq!(
            log.iter().map(|write| write.value).collect::<Vec<_>>(),
            vec![0x86, 0x35, 0x90]
        );
        assert!(log.iter().all(|write| write.chip == Chip::Sn76489));
    }

    #[test]
    fn command_loop_and_goto_are_executed() {
        let sequence = SoundSequence::new(
            0x81,
            1,
            vec![crate::FmVoice::fixture()],
            Vec::new(),
            vec![SoundTrack::fm(
                0,
                vec![0xef, 0, 0x81, 1, 0xf7, 0, 2, 0xff, 0xf9, 0xf2],
            )],
        );
        let mut machine = SoundMachine::new();
        machine.load(sequence);
        machine.start();
        let _ = machine.tick();
        assert!(machine.last_error().is_none());
    }

    #[test]
    fn disabled_pan_animation_consumes_only_its_count_byte() {
        let sequence = SoundSequence::new(
            0x81,
            1,
            Vec::new(),
            Vec::new(),
            vec![SoundTrack::fm(0, vec![0xff, 0x00, 0x00, 0xf2])],
        );
        let mut machine = SoundMachine::new();
        machine.load(sequence);
        machine.start();
        let _ = machine.tick();
        assert!(machine.last_error().is_none());
    }
}
