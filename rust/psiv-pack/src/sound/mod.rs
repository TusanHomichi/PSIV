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
//! from [`crate::maps::extract_maps`].

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

mod driver;
mod records;
mod walk;

use driver::{dac_banks, driver};
use records::{envelopes, music_record, sfx_record};
use walk::{Decoded, decode_track};

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
    let decodes = crate::decode::Decodes::new(rom.bytes());
    let maps = crate::maps::extract_maps(rom.bytes(), &decodes)?;
    let ids: Vec<u8> = maps.real().map(|record| record.music_id).collect();
    emit(rom.bytes(), &ids, root)
}

#[cfg(test)]
mod tests;
