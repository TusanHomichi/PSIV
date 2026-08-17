use super::{
    ChipBus, Modulation, TickContext, TrackState, apply_voice, emit_fm, emit_psg, emit_psg_volume,
    gosub, jump, loop_command, note_off, refresh_fm_volume, return_command, take_byte,
};
use crate::sequence::{SequenceError, SoundSequence, TrackKind};

pub(super) fn command(
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
        0xed => {
            state.dac_pan = take_byte(state, opcode)?;
            bus.dac.set_pan(state.dac_pan);
        }
        0xe1 => state.detune = take_byte(state, opcode)? as i8,
        0xe2 => state.communication = take_byte(state, opcode)?,
        0xe3 => {
            state.dac_volume_control = take_byte(state, opcode)?;
            bus.dac.set_volume_control(state.dac_volume_control);
        }
        0xe4 => {
            state.dac_loop = take_byte(state, opcode)?;
            bus.dac.set_loop(state.dac_loop);
        }
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
        0xee => {
            state.dac_sample = take_byte(state, opcode)?;
            bus.dac.trigger(state.dac_sample, bus.ym, bus.log, bus.tick);
        }
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
        0xf4 => state.modulation_type = take_byte(state, opcode)?,
        0xf5 => {
            let selector = take_byte(state, opcode)?;
            let Some(index) = selector.checked_sub(1).map(usize::from) else {
                state.psg_envelope = None;
                state.envelope_cursor = 0;
                return Ok(true);
            };
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
        0xfa => {
            state.dac_reverse = take_byte(state, opcode)?;
            bus.dac.set_reverse(state.dac_reverse);
        }
        0xfb => {
            state.transpose = state
                .transpose
                .saturating_add(take_byte(state, opcode)? as i8)
        }
        0xfc => {
            state.dac_volume = take_byte(state, opcode)?;
            bus.dac.set_volume(state.dac_volume);
        }
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
