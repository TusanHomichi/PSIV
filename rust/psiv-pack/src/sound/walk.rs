//! The static track walk: every driver path a track can take, as a
//! verifier and census.

use super::*;

/// What the static walk of one track saw.
#[derive(Default)]
pub(super) struct Decoded {
    pub(super) command_counts: BTreeMap<u8, u32>,
    pub(super) meta_counts: BTreeMap<u8, u32>,
    pub(super) fm_instruments: BTreeSet<u8>,
    pub(super) psg_instruments: BTreeSet<u8>,
    pub(super) covered: HashSet<usize>,
    pub(super) byte_values: BTreeSet<u8>,
    pub(super) states: u32,
    pub(super) max_call_depth: usize,
    pub(super) has_track_end: bool,
}

impl Decoded {
    pub(super) fn json(&self) -> TrackDecode {
        TrackDecode {
            reachable_instruction_states: self.states,
            reachable_byte_count: self.covered.len() as u32,
            command_opcodes_seen: hex_list(self.command_counts.keys().copied()),
            command_counts: counts(&self.command_counts),
            meta_opcodes_seen: hex_list(self.meta_counts.keys().copied()),
            meta_counts: counts(&self.meta_counts),
            sequence_byte_values_seen: hex_list(self.byte_values.iter().copied()),
            max_call_depth: self.max_call_depth as u32,
            has_track_end: self.has_track_end,
        }
    }
}

fn command_spec(opcode: u8) -> Option<&'static CommandSpec> {
    COMMAND_SPECS.iter().find(|s| s.opcode == opcode)
}

/// Walk every statically reachable driver path of the track at `start`,
/// inside `floor..end`.
///
/// A state is a position and its `F8` return stack. `F7` takes both its
/// branch and its fall-through, since its counter is runtime state, so every
/// command byte in the record a later interpreter can meet is seen.
pub(super) fn decode_track(
    rom: &[u8],
    start: usize,
    end: usize,
    track_id: &str,
    floor: usize,
) -> Result<Decoded, PackError> {
    let at = rom_offset;
    if !(floor <= start && start < end) {
        return Err(error(format!("{track_id}: empty or inverted track range")));
    }
    let mut result = Decoded::default();
    let mut work: VecDeque<(usize, Vec<usize>)> = VecDeque::from([(start, Vec::new())]);
    let mut visited: HashSet<(usize, Vec<usize>)> = HashSet::new();
    while let Some((pos, call_stack)) = work.pop_front() {
        if !visited.insert((pos, call_stack.clone())) {
            continue;
        }
        result.states += 1;
        result.max_call_depth = result.max_call_depth.max(call_stack.len());
        if call_stack.len() > MAX_CALL_DEPTH {
            return Err(error(format!(
                "{track_id}: GOSUB stack exceeds static safety limit at {}",
                at(pos)
            )));
        }
        if pos < floor || pos >= end {
            return Err(error(format!(
                "{track_id}: control flow leaves track at {}",
                at(pos)
            )));
        }

        let opcode = rom[pos];
        let mut successors: Vec<(isize, Vec<usize>)> = Vec::new();
        let size;
        if opcode < 0x80 {
            size = 1;
        } else if opcode < 0xE0 {
            // A note or rest takes a delay only when the next byte has bit 7
            // clear; a high byte is the next token, and the driver backs its
            // pointer up after looking at it.
            if pos + 1 >= end {
                return Err(error(format!(
                    "{track_id}: note at {} lacks lookahead",
                    at(pos)
                )));
            }
            size = if rom[pos + 1] < 0x80 { 2 } else { 1 };
            successors.push(((pos + size) as isize, call_stack.clone()));
        } else if opcode == 0xFF {
            *result.command_counts.entry(opcode).or_default() += 1;
            if pos + 1 >= end {
                return Err(error(format!(
                    "{track_id}: FF at {} lacks meta opcode",
                    at(pos)
                )));
            }
            let meta = rom[pos + 1];
            if meta != META_PAN_ANIM {
                return Err(error(format!(
                    "{track_id}: undocumented FF meta {} at {}",
                    hx(usize::from(meta), 2),
                    at(pos + 1)
                )));
            }
            *result.meta_counts.entry(meta).or_default() += 1;
            if pos + 2 >= end {
                return Err(error(format!(
                    "{track_id}: FF 00 at {} lacks payload",
                    at(pos)
                )));
            }
            size = if rom[pos + 2] == 0 { 3 } else { 7 };
            successors.push(((pos + size) as isize, call_stack.clone()));
        } else {
            let Some(spec) = command_spec(opcode) else {
                return Err(error(format!(
                    "{track_id}: undocumented command {} at {}",
                    hx(usize::from(opcode), 2),
                    at(pos)
                )));
            };
            *result.command_counts.entry(opcode).or_default() += 1;
            size = spec.length;
            if opcode == 0xEF || opcode == 0xF5 {
                if pos + 1 >= end {
                    let what = if opcode == 0xEF { "EF" } else { "F5" };
                    return Err(error(format!(
                        "{track_id}: {what} at {} lacks instrument",
                        at(pos)
                    )));
                }
                if opcode == 0xEF {
                    result.fm_instruments.insert(rom[pos + 1]);
                } else {
                    result.psg_instruments.insert(rom[pos + 1]);
                }
            }
            match opcode {
                0xF2 => result.has_track_end = true,
                0xF6 => {
                    let target = relative(rom, pos + 2, pos + 1)?;
                    successors.push((target, call_stack.clone()));
                }
                0xF7 => {
                    let target = relative(rom, pos + 4, pos + 3)?;
                    successors.push((target, call_stack.clone()));
                    successors.push(((pos + size) as isize, call_stack.clone()));
                }
                0xF8 => {
                    if call_stack.len() >= MAX_CALL_DEPTH {
                        return Err(error(format!(
                            "{track_id}: GOSUB stack exceeds static safety limit"
                        )));
                    }
                    let target = relative(rom, pos + 2, pos + 1)?;
                    let mut pushed = call_stack.clone();
                    pushed.push(pos + size);
                    successors.push((target, pushed));
                }
                0xF9 => {
                    let mut popped = call_stack.clone();
                    let Some(back) = popped.pop() else {
                        return Err(error(format!(
                            "{track_id}: RETURN without GOSUB at {}",
                            at(pos)
                        )));
                    };
                    successors.push((back as isize, popped));
                }
                _ => successors.push(((pos + size) as isize, call_stack.clone())),
            }
        }

        if pos + size > end {
            return Err(error(format!(
                "{track_id}: {} at {} truncates at {}",
                hx(usize::from(opcode), 2),
                at(pos),
                at(end)
            )));
        }
        result.covered.extend(pos..pos + size);
        result.byte_values.extend(&rom[pos..pos + size]);
        for (successor, stack) in successors {
            if successor < floor as isize || successor >= end as isize {
                return Err(error(format!(
                    "{track_id}: {} at {} targets outside track at {}",
                    hx(usize::from(opcode), 2),
                    at(pos),
                    signed_offset(successor)
                )));
            }
            work.push_back((successor as usize, stack));
        }
    }
    Ok(result)
}
