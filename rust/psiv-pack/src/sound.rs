//! The `sound` section: raw sound records, the port of `psiv_tools/sound.py`.
//!
//! The cartridge uses a modified SMPS 68k driver. The section keeps that
//! format as found (records, relative pointers, command bytes, voices,
//! envelopes and the Z80 DAC tables); the static track walk is a verifier
//! and census, not an interpreter. Before any pointer is trusted, the byte
//! anchors and region hashes of [`crate::sound_defs`] pin the driver.
//!
//! The files are `psiv-data`'s [`schema`] types, the shape its loader reads.
//! `map_usage_count` counts the field maps that select each music record,
//! from [`crate::field_maps::music_ids`].

use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::path::Path;

use psiv_data::sound::schema::{
    self, Census, Dac, DacBank, DacSample, DacTableEntry, Driver, DriverFile, Envelope, Envelopes,
    FileEntry, FileRef, IndexFile, MusicFile, MusicHeader, RawRecord, Record, RecordGroup, Region,
    RomStamp, SfxFile, SfxGroups, SfxHeader, Track, TrackDecode, TrackFamily, TrackInitial,
    TrackInstruments, TrackSource, Voice,
};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use crate::png::hex;
use crate::sound_defs::{
    ANCHORS, COMMAND_SPECS, CommandSpec, EXPECTED_SHORT_BYTES, KNOWN_REGIONS, META_PAN_ANIM,
    META_PAN_ANIM_LENGTH, META_PAN_ANIM_PAYLOAD, META_PAN_ANIM_TYPE, MUSIC_NAMES, SFX_NAMES,
    SPECIAL_NAMES,
};
use crate::{PackError, Rom};

const SOUND_DIRECTORY: &str = "sound";
const VERSION: u32 = schema::SOUND_FORMAT_VERSION;

const MUSIC_PTRS: usize = 0xD1C40;
const MUSIC_COUNT: usize = 52;
const MUSIC_END: usize = 0xDE4B6;
const SFX_PTRS: usize = 0xDE4B6;
const SFX_COUNT: usize = 67;
const SFX_END: usize = 0xDF68A;
const SPECIAL_PTRS: usize = 0xDE5C2;
const SPECIAL_COUNT: usize = 3;
const SPECIAL_END: usize = 0xDF78C;
/// A static walk refuses a deeper `F8` nesting.
const MAX_CALL_DEPTH: usize = 32;
const VOICE_SIZE: usize = 25;
/// Bank, ROM base and sample count of each DAC bank.
const DAC_BANKS: [(u8, usize, usize); 4] = [
    (0, 0xE0000, 10),
    (1, 0xE8000, 2),
    (2, 0xF0000, 4),
    (3, 0xF8000, 1),
];

fn error(message: impl Into<String>) -> PackError {
    PackError(message.into())
}

fn hx(value: usize, width: usize) -> String {
    format!("0x{value:0width$X}")
}

fn rom_offset(value: usize) -> String {
    hx(value, 6)
}

fn sha(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn be16(data: &[u8], offset: usize) -> Result<u16, PackError> {
    data.get(offset..offset + 2)
        .map(|b| u16::from_be_bytes([b[0], b[1]]))
        .ok_or_else(|| error(format!("read past ROM at {}", rom_offset(offset))))
}

fn be32(data: &[u8], offset: usize) -> Result<usize, PackError> {
    data.get(offset..offset + 4)
        .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize)
        .ok_or_else(|| error(format!("read past ROM at {}", rom_offset(offset))))
}

/// `base + signed(word at word_at)`, a relative branch's target, which can
/// fall below zero.
fn relative(rom: &[u8], base: usize, word_at: usize) -> Result<isize, PackError> {
    let displacement = be16(rom, word_at)? as i16;
    Ok(base as isize + isize::from(displacement))
}

/// [`rom_offset`] of a signed position, as Python formats a negative one.
fn signed_offset(value: isize) -> String {
    if value < 0 {
        format!("0x-{:05X}", value.unsigned_abs())
    } else {
        rom_offset(value as usize)
    }
}

fn range<'a>(data: &'a [u8], start: usize, end: usize, label: &str) -> Result<&'a [u8], PackError> {
    if end < start || end > data.len() {
        return Err(error(format!(
            "{label} range {}:{} is outside the ROM",
            rom_offset(start),
            rom_offset(end)
        )));
    }
    Ok(&data[start..end])
}

/// `bytes.fromhex`: pairs of hex digits, spaces between them ignored.
fn from_hex(text: &str) -> Vec<u8> {
    let digits: Vec<u8> = text.bytes().filter(|b| *b != b' ').collect();
    digits
        .chunks(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn hex_list(values: impl IntoIterator<Item = u8>) -> Vec<String> {
    values.into_iter().map(|v| hx(usize::from(v), 2)).collect()
}

fn counts(counts: &BTreeMap<u8, u32>) -> BTreeMap<String, u32> {
    counts
        .iter()
        .map(|(k, v)| (format!("{k:02X}"), *v))
        .collect()
}

/// The ROM hash is [`Rom`]'s check; these pin the driver itself.
fn verify_provenance(rom: &[u8]) -> Result<(), PackError> {
    for (offset, expected) in ANCHORS {
        let expected = from_hex(expected);
        let actual = range(rom, *offset, offset + expected.len(), "anchor")?;
        if actual != expected.as_slice() {
            return Err(error(format!(
                "sound provenance anchor mismatch at {}: expected {}, got {}",
                rom_offset(*offset),
                hex(&expected),
                hex(actual)
            )));
        }
    }
    for (name, label, start, end, expected_sha) in KNOWN_REGIONS {
        let actual = range(rom, *start, *end, label)?;
        if sha(actual) != *expected_sha {
            return Err(error(format!(
                "sound region {label} ({name}) changed at {}:{}",
                rom_offset(*start),
                rom_offset(*end)
            )));
        }
        let short = EXPECTED_SHORT_BYTES.iter().find(|(n, _)| n == name);
        if let Some((_, bytes)) = short
            && actual != from_hex(bytes).as_slice()
        {
            return Err(error(format!(
                "sound constant {label} bytes do not match the disassembly"
            )));
        }
    }
    Ok(())
}

fn read_pointer_table(
    rom: &[u8],
    offset: usize,
    count: usize,
    end_limit: usize,
    label: &str,
) -> Result<Vec<usize>, PackError> {
    let raw = range(rom, offset, offset + count * 4, label)?;
    let pointers = (0..count)
        .map(|i| be32(raw, i * 4))
        .collect::<Result<Vec<_>, _>>()?;
    if pointers
        .iter()
        .any(|&p| p < offset + count * 4 || p >= end_limit)
    {
        return Err(error(format!(
            "{label} contains a pointer outside its data region"
        )));
    }
    if pointers.windows(2).any(|w| w[0] >= w[1]) {
        return Err(error(format!(
            "{label} pointers are not strictly increasing"
        )));
    }
    Ok(pointers)
}

/// The raw files the section writes, by `sound/`-relative path.
#[derive(Default)]
struct RawFiles(BTreeMap<String, Vec<u8>>);

impl RawFiles {
    fn add(&mut self, path: String, data: &[u8]) -> Result<(), PackError> {
        if self.0.contains_key(&path) {
            return Err(error(format!("duplicate raw output path {path}")));
        }
        self.0.insert(path, data.to_vec());
        Ok(())
    }
}

fn region(
    rom: &[u8],
    raw_files: &mut RawFiles,
    label: &str,
    start: usize,
    end: usize,
    raw_path: String,
) -> Result<Region, PackError> {
    let data = range(rom, start, end, label)?;
    raw_files.add(raw_path.clone(), data)?;
    Ok(Region {
        source_label: label.to_owned(),
        rom_offset: rom_offset(start),
        size_bytes: data.len() as u64,
        raw_file: raw_path,
        raw_sha256: sha(data),
        raw_hex: hex(data),
    })
}

/// An envelope's span through its terminal control byte, and that byte.
/// `84` is a two-byte modulation command, so it cannot end a modulation
/// envelope; every other control byte ends the stream.
fn envelope_span(
    rom: &[u8],
    start: usize,
    limit: usize,
    kind: &str,
    index: usize,
) -> Result<(usize, usize, u8), PackError> {
    let mut pos = start;
    while pos < limit {
        let value = rom[pos];
        if kind == "modulation" && value == 0x84 {
            pos += 2;
            if pos > limit {
                return Err(error(format!(
                    "modulation envelope {index} truncates CHG_MULT"
                )));
            }
            continue;
        }
        if (0x80..=0x83).contains(&value) {
            return Ok((start, pos + 1, value));
        }
        pos += 1;
    }
    Err(error(format!(
        "{kind} envelope {index} has no terminal control byte"
    )))
}

/// What the static walk of one track saw.
#[derive(Default)]
struct Decoded {
    command_counts: BTreeMap<u8, u32>,
    meta_counts: BTreeMap<u8, u32>,
    fm_instruments: BTreeSet<u8>,
    psg_instruments: BTreeSet<u8>,
    covered: HashSet<usize>,
    byte_values: BTreeSet<u8>,
    states: u32,
    max_call_depth: usize,
    has_track_end: bool,
}

impl Decoded {
    fn json(&self) -> TrackDecode {
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
fn decode_track(
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

fn voice(rom: &[u8], start: usize, index: usize) -> Result<Voice, PackError> {
    let at = start + index * VOICE_SIZE;
    let raw = range(rom, at, at + VOICE_SIZE, "FM voice")?;
    Ok(Voice {
        index: index as u32,
        rom_offset: rom_offset(at),
        size_bytes: VOICE_SIZE as u32,
        raw_hex: hex(raw),
        algorithm_feedback: raw[0],
        operator_registers: raw[1..21].to_vec(),
        operator_total_levels: raw[21..25].to_vec(),
    })
}

/// Every voice in the table and the zero padding after the last whole one.
fn voice_table(
    rom: &[u8],
    voice_start: usize,
    record_end: usize,
    label: &str,
    decoded: &[Decoded],
) -> Result<(u32, u32, Vec<Voice>), PackError> {
    let voice_bytes = record_end - voice_start;
    let (count, padding) = (voice_bytes / VOICE_SIZE, voice_bytes % VOICE_SIZE);
    if padding != 0
        && rom[record_end - padding..record_end]
            .iter()
            .any(|b| *b != 0)
    {
        return Err(error(format!(
            "{label}: nonzero bytes after the FM voice table"
        )));
    }
    let highest = decoded
        .iter()
        .filter_map(|d| d.fm_instruments.last())
        .max()
        .map(|v| usize::from(*v));
    if highest.is_some_and(|h| h >= count) {
        return Err(error(format!(
            "{label}: used FM voice index exceeds the full voice table"
        )));
    }
    let voices = (0..count)
        .map(|i| voice(rom, voice_start, i))
        .collect::<Result<_, _>>()?;
    Ok((count as u32, padding as u32, voices))
}

/// Where a track is, its descriptor and its initial values.
struct TrackSpec {
    start: usize,
    index: u8,
    kind: TrackFamily,
    id: String,
    initial: TrackInitial,
    descriptor: std::ops::Range<usize>,
}

fn track(
    rom: &[u8],
    record: std::ops::Range<usize>,
    sequence_start: usize,
    spec: TrackSpec,
) -> Result<(Track, Decoded), PackError> {
    let decoded = decode_track(rom, spec.start, record.end, &spec.id, sequence_start)?;
    let track = Track {
        track_id: spec.id,
        kind: spec.kind,
        index: spec.index,
        source: TrackSource {
            rom_offset: rom_offset(spec.start),
            record_relative_offset: (spec.start - record.start) as u32,
            record_end_exclusive: rom_offset(record.end),
            raw_descriptor_hex: hex(&rom[spec.descriptor]),
        },
        initial: spec.initial,
        instruments_used: decoded.fm_instruments.iter().copied().collect(),
        psg_instruments_used: decoded.psg_instruments.iter().copied().collect(),
        decode: decoded.json(),
    };
    Ok((track, decoded))
}

fn raw_record(
    rom: &[u8],
    raw_files: &mut RawFiles,
    prefix: &str,
    sound_id: u8,
    name: &str,
    span: std::ops::Range<usize>,
    label: &str,
) -> Result<RawRecord, PackError> {
    let (start, end) = (span.start, span.end);
    let raw_path = format!("raw/{prefix}/{sound_id:02X}_{name}.bin");
    let raw = range(rom, start, end, label)?;
    raw_files.add(raw_path.clone(), raw)?;
    Ok(RawRecord {
        rom_offset: rom_offset(start),
        end_offset_exclusive: rom_offset(end),
        size_bytes: raw.len() as u64,
        raw_file: raw_path,
        raw_sha256: sha(raw),
    })
}

fn music_record(
    rom: &[u8],
    raw_files: &mut RawFiles,
    sound_id: u8,
    name: &str,
    start: usize,
    end: usize,
    map_usage_count: u32,
) -> Result<(Record<MusicHeader>, Vec<Decoded>), PackError> {
    let label = format!("music {} {name}", hx(usize::from(sound_id), 2));
    if start + 6 > end {
        return Err(error(format!("{label}: header is truncated")));
    }
    let voice_relative = be16(rom, start)?;
    let voice_start = start + usize::from(voice_relative);
    let fm_count = rom[start + 2];
    let psg_count = rom[start + 3];
    let tick_multiplier = rom[start + 4];
    let tempo_reload = rom[start + 5];
    if fm_count == 0 && psg_count == 0 {
        return Err(error(format!("{label}: no channels")));
    }
    let descriptor_end = start + 6 + usize::from(fm_count) * 4 + usize::from(psg_count) * 6;
    if descriptor_end > end {
        return Err(error(format!("{label}: channel descriptors exceed record")));
    }
    if voice_start < descriptor_end || voice_start > end {
        return Err(error(format!(
            "{label}: voice table pointer is not after the header"
        )));
    }

    let mut tracks = Vec::new();
    let mut decoded_tracks = Vec::new();
    let mut pos = start + 6;
    for index in 0..fm_count {
        let track_start = start + usize::from(be16(rom, pos)?);
        let initial_word = be16(rom, pos + 2)?;
        if track_start < descriptor_end || track_start >= end {
            return Err(error(format!(
                "{label}: FM track {index} pointer leaves record"
            )));
        }
        let spec = TrackSpec {
            start: track_start,
            index,
            kind: TrackFamily::Fm,
            id: format!("music:{sound_id:02X}:fm:{index}"),
            initial: TrackInitial {
                channel_word: None,
                channel_byte: None,
                transpose: (initial_word >> 8) as u8,
                volume: (initial_word & 0xFF) as u8,
                modulation_envelope: None,
                volume_envelope: None,
            },
            descriptor: pos..pos + 4,
        };
        let (track, decoded) = track(rom, start..end, descriptor_end, spec)?;
        tracks.push(track);
        decoded_tracks.push(decoded);
        pos += 4;
    }
    for index in 0..psg_count {
        let track_start = start + usize::from(be16(rom, pos)?);
        let initial_word = be16(rom, pos + 2)?;
        if track_start < descriptor_end || track_start >= end {
            return Err(error(format!(
                "{label}: PSG track {index} pointer leaves record"
            )));
        }
        let spec = TrackSpec {
            start: track_start,
            index,
            kind: TrackFamily::Psg,
            id: format!("music:{sound_id:02X}:psg:{index}"),
            initial: TrackInitial {
                channel_word: None,
                channel_byte: None,
                transpose: (initial_word >> 8) as u8,
                volume: (initial_word & 0xFF) as u8,
                modulation_envelope: Some(rom[pos + 4]),
                volume_envelope: Some(rom[pos + 5]),
            },
            descriptor: pos..pos + 6,
        };
        let (track, decoded) = track(rom, start..end, descriptor_end, spec)?;
        tracks.push(track);
        decoded_tracks.push(decoded);
        pos += 6;
    }

    let (voice_count, voice_padding, voices) =
        voice_table(rom, voice_start, end, &label, &decoded_tracks)?;
    let record = raw_record(rom, raw_files, "music", sound_id, name, start..end, &label)?;
    Ok((
        Record {
            id: sound_id,
            id_hex: hx(usize::from(sound_id), 2),
            symbol: name.to_owned(),
            record,
            header: MusicHeader {
                voice_table_relative: voice_relative,
                voice_table_rom_offset: rom_offset(voice_start),
                fm_channel_count: fm_count,
                psg_channel_count: psg_count,
                tick_multiplier,
                tempo_reload,
                header_size_bytes: (descriptor_end - start) as u32,
                voice_count,
                voice_padding_bytes: voice_padding,
            },
            map_usage_count: Some(map_usage_count),
            voices,
            tracks,
        },
        decoded_tracks,
    ))
}

fn sfx_record(
    rom: &[u8],
    raw_files: &mut RawFiles,
    sound_id: u8,
    name: &str,
    start: usize,
    end: usize,
    special: bool,
) -> Result<(Record<SfxHeader>, Vec<Decoded>), PackError> {
    let label = format!(
        "{}SFX {} {name}",
        if special { "special " } else { "" },
        hx(usize::from(sound_id), 2)
    );
    if start + 4 > end {
        return Err(error(format!("{label}: header is truncated")));
    }
    let voice_relative = be16(rom, start)?;
    let voice_start = start + usize::from(voice_relative);
    if voice_start > end {
        return Err(error(format!("{label}: voice table pointer leaves record")));
    }
    let tick_multiplier = rom[start + 2];
    let track_count = rom[start + 3];
    if track_count == 0 {
        return Err(error(format!("{label}: zero track count")));
    }
    let descriptor_end = start + 4 + usize::from(track_count) * 6;
    if descriptor_end > end || voice_start < descriptor_end {
        return Err(error(format!(
            "{label}: descriptor/header layout is invalid"
        )));
    }

    let mut tracks = Vec::new();
    let mut decoded_tracks = Vec::new();
    let mut pos = start + 4;
    for index in 0..track_count {
        let channel_word = be16(rom, pos)?;
        let track_start = start + usize::from(be16(rom, pos + 2)?);
        let initial_word = be16(rom, pos + 4)?;
        if track_start < descriptor_end || track_start >= end {
            return Err(error(format!(
                "{label}: track {index} pointer leaves record"
            )));
        }
        let spec = TrackSpec {
            start: track_start,
            index,
            kind: TrackFamily::Sfx,
            id: format!("sfx:{sound_id:02X}:{index}"),
            initial: TrackInitial {
                channel_word: Some(channel_word),
                channel_byte: Some((channel_word & 0xFF) as u8),
                transpose: (initial_word >> 8) as u8,
                volume: (initial_word & 0xFF) as u8,
                modulation_envelope: None,
                volume_envelope: None,
            },
            descriptor: pos..pos + 6,
        };
        let (track, decoded) = track(rom, start..end, descriptor_end, spec)?;
        tracks.push(track);
        decoded_tracks.push(decoded);
        pos += 6;
    }

    let (voice_count, voice_padding, voices) =
        voice_table(rom, voice_start, end, &label, &decoded_tracks)?;
    let prefix = if special { "special" } else { "sfx" };
    let record = raw_record(rom, raw_files, prefix, sound_id, name, start..end, &label)?;
    Ok((
        Record {
            id: sound_id,
            id_hex: hx(usize::from(sound_id), 2),
            symbol: name.to_owned(),
            record,
            header: SfxHeader {
                voice_table_relative: voice_relative,
                voice_table_rom_offset: rom_offset(voice_start),
                tick_multiplier,
                track_count,
                header_size_bytes: (descriptor_end - start) as u32,
                voice_count,
                voice_padding_bytes: voice_padding,
            },
            map_usage_count: None,
            voices,
            tracks,
        },
        decoded_tracks,
    ))
}

fn envelopes(
    rom: &[u8],
    raw_files: &mut RawFiles,
    kind: &str,
    table: usize,
    count: usize,
) -> Result<Vec<Envelope>, PackError> {
    const LIMIT: usize = 0xD1C40;
    (0..count)
        .map(|index| {
            let pointer = be32(rom, table + index * 4)?;
            let (start, end, terminal) = envelope_span(rom, pointer, LIMIT, kind, index)?;
            let path = format!("raw/driver/{kind}_envelope_{index:02}.bin");
            let data = range(rom, start, end, &format!("{kind} envelope"))?;
            raw_files.add(path.clone(), data)?;
            Ok(Envelope {
                index: index as u32,
                rom_offset: rom_offset(start),
                size_bytes: data.len() as u64,
                terminal: hx(usize::from(terminal), 2),
                raw_file: path,
                raw_sha256: sha(data),
                raw_hex: hex(data),
            })
        })
        .collect()
}

fn command_vocabulary() -> Value {
    let primary: serde_json::Map<String, Value> = COMMAND_SPECS
        .iter()
        .map(|s| {
            let mut entry = json!({
                "opcode": s.opcode,
                "type": s.kind,
                "subtype": s.subtype,
                "length": s.length,
            });
            if let Some(note) = s.note {
                entry["note"] = json!(note);
            }
            if let Some(at) = s.relative_word_at {
                entry["relative_word_at"] = json!(at);
            }
            if s.variable {
                entry["variable"] = json!(true);
            }
            (hx(usize::from(s.opcode), 2), entry)
        })
        .collect();
    json!({
        "primary": primary,
        "meta": {
            hx(usize::from(META_PAN_ANIM), 2): {
                "opcode": META_PAN_ANIM,
                "type": META_PAN_ANIM_TYPE,
                "length": META_PAN_ANIM_LENGTH,
                "payload": META_PAN_ANIM_PAYLOAD,
            },
        },
        "primary_count": COMMAND_SPECS.len(),
        "meta_count": 1,
        "total_documented_entries": COMMAND_SPECS.len() + 1,
    })
}

fn fixed_constants(rom: &[u8]) -> Result<Value, PackError> {
    let bytes = |s: usize, e: usize| rom[s..e].to_vec();
    let frequencies = (0..12)
        .map(|i| be16(rom, 0xD0DE2 + i * 2))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({
        "fm_init_bytes": bytes(0xD07D2, 0xD07D9),
        "psg_init_bytes": bytes(0xD07DA, 0xD07DD),
        "spc_fm3_registers": bytes(0xD0490, 0xD0498),
        "fm_algorithm_operator_masks": bytes(0xD1290, 0xD1298),
        "fm_operator_registers": bytes(0xD1308, 0xD131C),
        "fm_volume_registers": bytes(0xD131C, 0xD1320),
        "fm3_frequency_values": bytes(0xD1510, 0xD1518),
        "fm_frequencies_big_endian_words": frequencies,
        "psg_frequency_table_raw_hex": hex(&rom[0xD0FA2..0xD102E]),
        "pan_animation_sequences": {
            "0": hex(&rom[0xD053C..0xD053E]),
            "1": hex(&rom[0xD053E..0xD0541]),
            "2": hex(&rom[0xD0541..0xD0545]),
        },
    }))
}

fn driver(rom: &[u8], raw_files: &mut RawFiles) -> Result<Driver, PackError> {
    let mut regions = BTreeMap::new();
    for (name, label, start, end, _) in KNOWN_REGIONS {
        let path = format!("raw/driver/{name}.bin");
        regions.insert(
            (*name).to_owned(),
            region(rom, raw_files, label, *start, *end, path)?,
        );
    }

    let general_start = 0xD1A60;
    let general = (0..8)
        .map(|i| be32(rom, general_start + i * 4).map(rom_offset))
        .collect::<Result<Vec<_>, _>>()?;
    let modulation = envelopes(rom, raw_files, "modulation", 0xD1A80, 8)?;
    let volume = envelopes(rom, raw_files, "volume", 0xD1B44, 10)?;

    let dac_raw = range(rom, 0xD1A3E, 0xD1A60, "zBankTbl")?;
    let bank_table_entries = dac_raw
        .as_chunks::<2>()
        .0
        .iter()
        .zip(0x81u8..)
        .map(|(pair, sound_id)| DacTableEntry {
            sound_id,
            bank: pair[0],
            sample: pair[1],
        })
        .collect();

    let control = |pairs: &[(&str, &str)]| -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    };
    Ok(Driver {
        provenance: json!({
            "rom_sha256": sha(rom),
            "rom_size_bytes": rom.len(),
            "source_tree": "reference/ps4disasm/sound",
            "source_files": [
                "DefCFlag.txt", "DefDrv.txt", "Notes.txt", "Pointers.txt",
                "ps4.sound_driver.asm", "ps4.dac_driver.asm",
            ],
            "clone_drift_guard": "ROM hash, structural anchors, and fixed region hashes are verified before extraction",
        }),
        timing: json!({
            "update_sound_rom_offset": rom_offset(0xD0008),
            "global_tempo": {
                "music_header_byte": 5,
                "reload_register": "$2(a6)",
                "counter_register": "$1(a6)",
                "semantics": "Each UpdateSound call decrements the counter; zero reloads it from the header byte and increments $E on each active channel.",
                "zero_behavior": "A zero reload disables the global tempo tick path.",
            },
            "channel_tick_multiplier": {
                "music_header_byte": 4,
                "sfx_header_byte": 2,
                "register": "$2(a5)",
                "semantics": "TickMultiplier multiplies a note/delay byte by this value before loading channel $E/$F.",
            },
            "note_tokens": {
                "delay": "bytes 0x00-0x7F consume one byte and load that delay",
                "rest_or_note": "0x80 is rest; 0x81-0xDF are notes; a following byte below 0x80 is consumed as delay, while a high-bit byte is the next token",
            },
        }),
        command_vocabulary: command_vocabulary(),
        loop_commands: json!({
            "F6": "signed big-endian relative word at opcode+1; target is opcode+2+offset",
            "F7": "index byte, count byte, signed big-endian relative word at opcode+3; taken target is opcode+4+offset and exhausted fallthrough is opcode+5",
            "F8": "signed big-endian relative word at opcode+1; target is opcode+2+offset and return address is opcode+3",
            "F9": "pop the most recent F8 return address; an empty stack is undecodable",
        }),
        fixed_constants: fixed_constants(rom)?,
        envelopes: Envelopes {
            modulation_control_bytes: control(&[
                ("0x80", "RESET"),
                ("0x81", "HOLD"),
                ("0x82", "LOOP"),
                ("0x83", "STOP"),
                ("0x84", "CHG_MULT then one multiplier byte"),
            ]),
            volume_control_bytes: control(&[
                ("0x80", "RESET"),
                ("0x81", "HOLD"),
                ("0x82", "JUMP2IDX then one index byte"),
                ("0x83", "OFF"),
            ]),
            modulation,
            volume,
        },
        pointer_tables: json!({
            "driver_block": {
                "rom_offset": rom_offset(general_start),
                "pointers": general,
                "meaning": [
                    "SndPriorities", "SpcSFXPtrs", "MusicPtrs", "SFXPtrs",
                    "ModEnvPtrs", "VolEnvPtrs", "regular SFX id base", "UpdateSound",
                ],
            },
            "music": {"rom_offset": rom_offset(MUSIC_PTRS), "id_first": 0x81, "count": MUSIC_COUNT},
            "sfx": {"rom_offset": rom_offset(SFX_PTRS), "id_first": 0xB5, "count": SFX_COUNT},
            "special_sfx": {"rom_offset": rom_offset(SPECIAL_PTRS), "id_first": 0xF8, "count": SPECIAL_COUNT},
        }),
        dac: Dac {
            z80_driver: regions["z80_dac_driver"].clone(),
            bank_table: regions["dac_bank_table"].clone(),
            bank_table_entries,
            bank_rom_bases: DAC_BANKS.iter().map(|b| rom_offset(b.1)).collect(),
            table_entry_size_bytes: 8,
            table_word_encoding: "ROM stores each Z80 little-endian word as the two bytes read by ld e,(hl); ld d,(hl+1); swap bytes when presenting the address/length.".to_owned(),
            banks: Vec::new(),
        },
        regions,
    })
}

fn dac_banks(rom: &[u8], raw_files: &mut RawFiles, dac: &mut Dac) -> Result<(), PackError> {
    let mut first_entry = 0;
    for (bank, base, count) in DAC_BANKS {
        let table_path = format!("raw/dac/bank_{bank:02X}_table.bin");
        raw_files.add(
            table_path.clone(),
            range(rom, base, base + count * 8, "DAC bank table")?,
        )?;
        let mut samples = Vec::new();
        for local in 0..count {
            let sound_id = dac.bank_table_entries[first_entry + local].sound_id;
            let entry_offset = base + local * 8;
            let z80_start = usize::from(be16(rom, entry_offset)?.swap_bytes());
            let length = usize::from(be16(rom, entry_offset + 2)?.swap_bytes());
            if z80_start < 0x8000 || length == 0 {
                return Err(error(format!(
                    "DAC sample {} has invalid Z80 range",
                    hx(usize::from(sound_id), 2)
                )));
            }
            let sample_offset = base + (z80_start - 0x8000);
            let sample_end = sample_offset + length;
            if sample_end > base + 0x8000 {
                return Err(error(format!(
                    "DAC sample {} leaves bank {}",
                    hx(usize::from(sound_id), 2),
                    hx(usize::from(bank), 2)
                )));
            }
            let raw_path = format!("raw/dac/bank_{bank:02X}_sample_{local:02X}.bin");
            let raw = range(rom, sample_offset, sample_end, "DAC sample")?;
            raw_files.add(raw_path.clone(), raw)?;
            samples.push(DacSample {
                sound_id,
                bank,
                sample: local as u8,
                table_rom_offset: rom_offset(entry_offset),
                z80_start: hx(z80_start, 4),
                length_bytes: length as u32,
                rom_offset: rom_offset(sample_offset),
                raw_file: raw_path,
                raw_sha256: sha(raw),
            });
        }
        dac.banks.push(DacBank {
            bank,
            rom_base: rom_offset(base),
            entry_count: count as u32,
            table_raw_file: table_path,
            samples,
        });
        first_entry += count;
    }
    Ok(())
}

fn instruments_of<H>(records: &[Record<H>], into: &mut Vec<TrackInstruments>) {
    for track in records.iter().flat_map(|r| &r.tracks) {
        into.push(TrackInstruments {
            track_id: track.track_id.clone(),
            instruments_used: track.instruments_used.clone(),
            psg_instruments_used: track.psg_instruments_used.clone(),
        });
    }
}

/// The extracted files' contents and the raw files they name.
pub struct Extracted {
    pub driver: DriverFile,
    pub music: MusicFile,
    pub sfx: SfxFile,
    pub census: Census,
    raw_files: RawFiles,
}

/// Extract every sound record, verified, without writing files.
/// `map_music_ids` is each field map's music byte, for `map_usage_count`.
pub fn extract(rom: &[u8], map_music_ids: &[u8]) -> Result<Extracted, PackError> {
    verify_provenance(rom)?;
    let mut raw_files = RawFiles::default();
    let mut driver = driver(rom, &mut raw_files)?;
    dac_banks(rom, &mut raw_files, &mut driver.dac)?;

    let music_ptrs = read_pointer_table(rom, MUSIC_PTRS, MUSIC_COUNT, MUSIC_END, "MusicPtrs")?;
    let sfx_ptrs = read_pointer_table(rom, SFX_PTRS, SFX_COUNT, SFX_END, "SFXPtrs")?;
    let special_ptrs =
        read_pointer_table(rom, SPECIAL_PTRS, SPECIAL_COUNT, SPECIAL_END, "SpcSFXPtrs")?;
    let mut map_usage: BTreeMap<u8, u32> = BTreeMap::new();
    for id in map_music_ids {
        *map_usage.entry(*id).or_default() += 1;
    }
    let ends =
        |ptrs: &[usize], index: usize, last: usize| ptrs.get(index + 1).copied().unwrap_or(last);

    let mut music = Vec::new();
    let mut music_decoded = Vec::new();
    for (index, &start) in music_ptrs.iter().enumerate() {
        let id = 0x81 + index as u8;
        let (record, decoded) = music_record(
            rom,
            &mut raw_files,
            id,
            MUSIC_NAMES[index],
            start,
            ends(&music_ptrs, index, MUSIC_END),
            map_usage.get(&id).copied().unwrap_or(0),
        )?;
        music.push(record);
        music_decoded.extend(decoded);
    }
    let mut sfx_group = |ptrs: &[usize], first: u8, names: &[&str], last: usize, special: bool| {
        let mut records = Vec::new();
        let mut decoded_all = Vec::new();
        for (index, &start) in ptrs.iter().enumerate() {
            let (record, decoded) = sfx_record(
                rom,
                &mut raw_files,
                first + index as u8,
                names[index],
                start,
                ends(ptrs, index, last),
                special,
            )?;
            records.push(record);
            decoded_all.extend(decoded);
        }
        Ok::<_, PackError>((records, decoded_all))
    };
    let (regular, regular_decoded) = sfx_group(&sfx_ptrs, 0xB5, &SFX_NAMES, SFX_END, false)?;
    let (special, special_decoded) =
        sfx_group(&special_ptrs, 0xF8, &SPECIAL_NAMES, SPECIAL_END, true)?;

    let mut instruments = Vec::new();
    instruments_of(&music, &mut instruments);
    instruments_of(&regular, &mut instruments);
    instruments_of(&special, &mut instruments);
    let all = music_decoded
        .iter()
        .chain(&regular_decoded)
        .chain(&special_decoded);
    let mut command_counts: BTreeMap<u8, u32> = BTreeMap::new();
    let mut meta_counts: BTreeMap<u8, u32> = BTreeMap::new();
    let mut sequence_values = BTreeSet::new();
    let mut max_depth = 0;
    let mut no_end = 0;
    for decoded in all {
        for (k, v) in &decoded.command_counts {
            *command_counts.entry(*k).or_default() += v;
        }
        for (k, v) in &decoded.meta_counts {
            *meta_counts.entry(*k).or_default() += v;
        }
        sequence_values.extend(&decoded.byte_values);
        max_depth = max_depth.max(decoded.max_call_depth);
        no_end += u32::from(!decoded.has_track_end);
    }
    let track_count = music_decoded.len() + regular_decoded.len() + special_decoded.len();
    let census = Census {
        music_record_count: music.len() as u32,
        regular_sfx_record_count: regular.len() as u32,
        special_sfx_record_count: special.len() as u32,
        music_track_count: music_decoded.len() as u32,
        regular_sfx_track_count: regular_decoded.len() as u32,
        special_sfx_track_count: special_decoded.len() as u32,
        track_count: track_count as u32,
        command_opcodes_seen: hex_list(command_counts.keys().copied()),
        command_vocabulary_size: command_counts.len() as u32,
        documented_primary_command_count: COMMAND_SPECS.len() as u32,
        meta_opcodes_seen: hex_list(meta_counts.keys().copied()),
        command_counts: counts(&command_counts),
        meta_counts: counts(&meta_counts),
        sequence_byte_values_seen: hex_list(sequence_values),
        max_gosub_call_depth: max_depth as u32,
        tracks_without_static_track_end: no_end,
        instruments_used_per_track: instruments,
        unresolved: Vec::new(),
    };
    let stamp = RomStamp {
        sha256: sha(rom),
        size_bytes: rom.len() as u64,
    };
    Ok(Extracted {
        driver: DriverFile {
            format_version: VERSION,
            kind: "sound_driver".to_owned(),
            rom: stamp.clone(),
            driver,
        },
        music: MusicFile {
            format_version: VERSION,
            kind: "music_records".to_owned(),
            rom: stamp.clone(),
            records: RecordGroup {
                id_first: 0x81,
                records: music,
            },
        },
        sfx: SfxFile {
            format_version: VERSION,
            kind: "sfx_records".to_owned(),
            rom: stamp,
            records: SfxGroups {
                regular: RecordGroup {
                    id_first: 0xB5,
                    records: regular,
                },
                special: RecordGroup {
                    id_first: 0xF8,
                    records: special,
                },
            },
        },
        census,
        raw_files,
    })
}

/// Write `value` as a pack JSON file; its SHA-256 and size.
fn write_typed(
    root: &Path,
    relative: &str,
    value: &impl serde::Serialize,
) -> Result<FileEntry, PackError> {
    let value = serde_json::to_value(value)
        .map_err(|e| error(format!("cannot serialize {relative}: {e}")))?;
    let bytes = crate::json::pack_bytes(&crate::json::Json::from(&value));
    crate::write_file(root, relative, &bytes)?;
    Ok(FileEntry {
        file: relative.to_owned(),
        size_bytes: bytes.len() as u64,
        sha256: sha(&bytes),
    })
}

/// Write `sound/` under `root`: the raw files, the three record files and
/// the index that lists them all.
pub fn emit(rom: &[u8], map_music_ids: &[u8], root: &Path) -> Result<(), PackError> {
    let extracted = extract(rom, map_music_ids)?;
    let mut files = Vec::new();
    for (relative, data) in &extracted.raw_files.0 {
        let file = format!("{SOUND_DIRECTORY}/{relative}");
        crate::write_file(root, &file, data)?;
        files.push(FileEntry {
            file,
            size_bytes: data.len() as u64,
            sha256: sha(data),
        });
    }
    let driver = write_typed(root, "sound/driver.json", &extracted.driver)?;
    let music = write_typed(root, "sound/music.json", &extracted.music)?;
    let sfx = write_typed(root, "sound/sfx.json", &extracted.sfx)?;
    let reference = |entry: &FileEntry| FileRef {
        file: entry.file.clone(),
        sha256: entry.sha256.clone(),
    };
    let index = IndexFile {
        format_version: VERSION,
        kind: "sound_index".to_owned(),
        rom: extracted.driver.rom.clone(),
        driver: reference(&driver),
        music: reference(&music),
        sfx: reference(&sfx),
        census: extracted.census,
        unresolved: Vec::new(),
        files: Vec::new(),
    };
    files.extend([driver, music, sfx]);
    files.sort_by(|a, b| a.file.cmp(&b.file));
    let index = IndexFile { files, ..index };
    write_typed(root, "sound/index.json", &index)?;
    Ok(())
}

/// The section: every field map's music id, then `sound/`.
pub fn build(rom: &Rom, root: &Path) -> Result<(), PackError> {
    let ids = crate::field_maps::music_ids(rom.bytes())?;
    emit(rom.bytes(), &ids, root)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn walk(track: &[u8]) -> Result<Decoded, PackError> {
        decode_track(track, 0, track.len(), "t", 0)
    }

    #[test]
    fn notes_take_a_delay_only_below_0x80() {
        // note+delay, note, note+delay, end
        let decoded = walk(&[0x81, 0x10, 0x82, 0x83, 0x04, 0xF2]).unwrap();
        assert_eq!(decoded.states, 4);
        assert_eq!(decoded.covered.len(), 6);
        assert!(decoded.has_track_end);
        assert_eq!(decoded.command_counts, BTreeMap::from([(0xF2, 1)]));
    }

    #[test]
    fn a_loop_walks_both_ways_and_a_call_returns() {
        let track = [
            0xF7, 0x00, 0x02, 0xFF, 0xFC, // 0: loop to 4-4 = 0, or on to 5
            0xF8, 0x00, 0x02, // 5: gosub 7+2 = 9, returning to 8
            0xF2, // 8: end
            0xF9, // 9: return
        ];
        let decoded = walk(&track).unwrap();
        assert_eq!(decoded.states, 4);
        assert_eq!(decoded.max_call_depth, 1);
        assert_eq!(
            decoded.command_counts,
            BTreeMap::from([(0xF2, 1), (0xF7, 1), (0xF8, 1), (0xF9, 1)])
        );
        assert!(decoded.has_track_end);
    }

    #[test]
    fn the_walk_refuses_what_the_driver_cannot_play() {
        for (track, message) in [
            (&[0xF9][..], "t: RETURN without GOSUB at 0x000000"),
            (
                &[0xFF, 0x01, 0x00],
                "t: undocumented FF meta 0x01 at 0x000001",
            ),
            (
                &[0xF6, 0x00, 0x10],
                "t: 0xF6 at 0x000000 targets outside track at 0x000012",
            ),
            (
                &[0xF6, 0xFF, 0xF0],
                "t: 0xF6 at 0x000000 targets outside track at 0x-0000E",
            ),
            (
                &[0xF0, 0x00, 0x00],
                "t: 0xF0 at 0x000000 truncates at 0x000003",
            ),
            (&[0x81], "t: note at 0x000000 lacks lookahead"),
            (&[0xEF], "t: EF at 0x000000 lacks instrument"),
            (
                &[0xF8, 0xFF, 0xFE],
                "t: GOSUB stack exceeds static safety limit",
            ),
        ] {
            assert_eq!(walk(track).err().map(|e| e.0).as_deref(), Some(message));
        }
    }

    #[test]
    fn envelopes_end_at_a_control_byte_but_not_inside_chg_mult() {
        assert_eq!(
            envelope_span(&[1, 0x84, 0x81, 2, 0x83], 0, 5, "modulation", 0).unwrap(),
            (0, 5, 0x83)
        );
        assert_eq!(
            envelope_span(&[1, 0x84, 0x81], 0, 3, "volume", 0).unwrap(),
            (0, 3, 0x81)
        );
        assert_eq!(
            envelope_span(&[1, 2], 0, 2, "volume", 4).unwrap_err().0,
            "volume envelope 4 has no terminal control byte"
        );
        assert_eq!(
            envelope_span(&[0x84], 0, 1, "modulation", 1).unwrap_err().0,
            "modulation envelope 1 truncates CHG_MULT"
        );
    }

    #[test]
    fn a_voice_table_pads_with_zeros_and_covers_every_used_voice() {
        let mut rom = vec![7; 2 * VOICE_SIZE];
        rom.extend([0, 0]);
        let (count, padding, voices) = voice_table(&rom, 0, rom.len(), "r", &[]).unwrap();
        assert_eq!((count, padding, voices.len()), (2, 2, 2));
        assert_eq!(voices[1].rom_offset, "0x000019");

        let used = Decoded {
            fm_instruments: BTreeSet::from([2]),
            ..Decoded::default()
        };
        assert_eq!(
            voice_table(&rom, 0, rom.len(), "r", &[used]).unwrap_err().0,
            "r: used FM voice index exceeds the full voice table"
        );
        *rom.last_mut().unwrap() = 1;
        assert_eq!(
            voice_table(&rom, 0, rom.len(), "r", &[]).unwrap_err().0,
            "r: nonzero bytes after the FM voice table"
        );
    }

    #[test]
    fn another_image_fails_the_first_anchor() {
        let error = verify_provenance(&vec![0; 0x100000]).unwrap_err().0;
        assert!(
            error.starts_with("sound provenance anchor mismatch at 0x0D0008: expected "),
            "{error}"
        );
    }

    #[test]
    fn the_vocabulary_documents_every_primary_command_once() {
        let opcodes: Vec<u8> = COMMAND_SPECS.iter().map(|s| s.opcode).collect();
        assert_eq!(opcodes, (0xE0..=0xFF).collect::<Vec<u8>>());
        let vocabulary = command_vocabulary();
        assert_eq!(vocabulary["primary"]["0xF7"]["relative_word_at"], 3);
        assert_eq!(vocabulary["primary"]["0xFF"]["variable"], true);
        assert_eq!(vocabulary["primary"]["0xE0"].get("variable"), None);
        assert_eq!(vocabulary["total_documented_entries"], 33);
    }
}
