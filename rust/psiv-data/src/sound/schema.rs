//! The files of `runtime-pack/sound`, as types: the one schema the pack
//! builder writes and [`super::SoundFiles::load`] reads (#216).
//!
//! `psiv-pack` fills these and serializes them as `json.dumps(...,
//! indent=2, sort_keys=True)` does; the loader deserializes the same types,
//! so a writer that disagrees with the reader fails to compile. Field names
//! and nesting are the Python builder's (`psiv_tools/sound.py`), which the
//! full gate's parity step holds both builders to until the switch.
//!
//! The driver's research blocks (provenance, timing notes, the command
//! vocabulary, the loop-command notes, fixed constants and pointer tables)
//! and the census's free-form `unresolved` list are kept as JSON values: the
//! game reads none of them, so they have no typed home yet.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// The version every sound file carries in `format_version`.
pub const SOUND_FORMAT_VERSION: u32 = 1;

/// The ROM a file was built from.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RomStamp {
    /// Lowercase SHA-256 of the image.
    pub sha256: String,
    /// Image size.
    pub size_bytes: u64,
}

/// A sound file named by the index, with its SHA-256.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FileRef {
    /// Pack-relative path.
    pub file: String,
    /// SHA-256 of the file's bytes.
    pub sha256: String,
}

/// One entry of the index's file inventory.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FileEntry {
    /// Pack-relative path.
    pub file: String,
    /// File size.
    pub size_bytes: u64,
    /// SHA-256 of the file's bytes.
    pub sha256: String,
}

/// `sound/index.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IndexFile {
    /// [`SOUND_FORMAT_VERSION`].
    pub format_version: u32,
    /// `"sound_index"`.
    pub kind: String,
    /// The source ROM.
    pub rom: RomStamp,
    /// `sound/driver.json`.
    pub driver: FileRef,
    /// `sound/music.json`.
    pub music: FileRef,
    /// `sound/sfx.json`.
    pub sfx: FileRef,
    /// Totals over every decoded track.
    pub census: Census,
    /// Open questions; empty for the retail image.
    pub unresolved: Vec<Value>,
    /// Every raw file and the three record files, sorted by path.
    pub files: Vec<FileEntry>,
}

/// The decode census over every track of every record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Census {
    /// Music records.
    pub music_record_count: u32,
    /// Regular SFX records.
    pub regular_sfx_record_count: u32,
    /// Special SFX records.
    pub special_sfx_record_count: u32,
    /// Tracks in music records.
    pub music_track_count: u32,
    /// Tracks in regular SFX records.
    pub regular_sfx_track_count: u32,
    /// Tracks in special SFX records.
    pub special_sfx_track_count: u32,
    /// All tracks.
    pub track_count: u32,
    /// Primary command opcodes met, as `0xNN`, ascending.
    pub command_opcodes_seen: Vec<String>,
    /// How many distinct primary commands were met.
    pub command_vocabulary_size: u32,
    /// How many primary commands the vocabulary documents.
    pub documented_primary_command_count: u32,
    /// `FF` meta opcodes met, as `0xNN`, ascending.
    pub meta_opcodes_seen: Vec<String>,
    /// Primary command counts keyed by `NN`.
    pub command_counts: BTreeMap<String, u32>,
    /// Meta command counts keyed by `NN`.
    pub meta_counts: BTreeMap<String, u32>,
    /// Every byte value on a reachable path, as `0xNN`, ascending.
    pub sequence_byte_values_seen: Vec<String>,
    /// Deepest `F8` nesting.
    pub max_gosub_call_depth: u32,
    /// Tracks with no reachable `F2`.
    pub tracks_without_static_track_end: u32,
    /// The FM and PSG instruments each track selects.
    pub instruments_used_per_track: Vec<TrackInstruments>,
    /// Open questions; empty for the retail image.
    pub unresolved: Vec<Value>,
}

/// One track's instrument use, in the census.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TrackInstruments {
    /// The track's id, as in [`Track::track_id`].
    pub track_id: String,
    /// FM voices selected by `EF`, ascending.
    pub instruments_used: Vec<u8>,
    /// PSG envelopes selected by `F5`, ascending.
    pub psg_instruments_used: Vec<u8>,
}

/// `sound/driver.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DriverFile {
    /// [`SOUND_FORMAT_VERSION`].
    pub format_version: u32,
    /// `"sound_driver"`.
    pub kind: String,
    /// The source ROM.
    pub rom: RomStamp,
    /// The driver's tables.
    pub driver: Driver,
}

/// The driver's tables and notes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Driver {
    /// Where the extraction's facts come from (research).
    pub provenance: Value,
    /// Tempo and tick notes (research).
    pub timing: Value,
    /// The documented command set (research).
    pub command_vocabulary: Value,
    /// Loop and call command layouts (research).
    pub loop_commands: Value,
    /// Fixed driver constants (research).
    pub fixed_constants: Value,
    /// Verified driver regions by name.
    pub regions: BTreeMap<String, Region>,
    /// Modulation and volume envelopes.
    pub envelopes: Envelopes,
    /// The driver's pointer tables (research).
    pub pointer_tables: Value,
    /// The Z80 DAC driver and its sample banks.
    pub dac: Dac,
}

/// A verified ROM region and its raw copy.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Region {
    /// Disassembly label.
    pub source_label: String,
    /// Start, `0xNNNNNN`.
    pub rom_offset: String,
    /// Length.
    pub size_bytes: u64,
    /// Raw copy, relative to `sound/`.
    pub raw_file: String,
    /// SHA-256 of the raw copy.
    pub raw_sha256: String,
    /// The bytes, as lowercase hex.
    pub raw_hex: String,
}

/// The envelope tables.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Envelopes {
    /// What each modulation control byte does, keyed by `0xNN`.
    pub modulation_control_bytes: BTreeMap<String, String>,
    /// What each volume control byte does, keyed by `0xNN`.
    pub volume_control_bytes: BTreeMap<String, String>,
    /// Modulation envelopes, by `ModEnvPtrs` index.
    pub modulation: Vec<Envelope>,
    /// Volume envelopes, by `VolEnvPtrs` index.
    pub volume: Vec<Envelope>,
}

/// One envelope stream, through its terminal control byte.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Envelope {
    /// Table index.
    pub index: u32,
    /// Start, `0xNNNNNN`.
    pub rom_offset: String,
    /// Length, the terminal byte included.
    pub size_bytes: u64,
    /// The terminal control byte, `0xNN`.
    pub terminal: String,
    /// Raw copy, relative to `sound/`.
    pub raw_file: String,
    /// SHA-256 of the raw copy.
    pub raw_sha256: String,
    /// The bytes, as lowercase hex.
    pub raw_hex: String,
}

/// The Z80 DAC driver, its bank table and its sample banks.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Dac {
    /// The Z80 driver program.
    pub z80_driver: Region,
    /// `zBankTbl`.
    pub bank_table: Region,
    /// `zBankTbl`, entry by entry.
    pub bank_table_entries: Vec<DacTableEntry>,
    /// Each bank's ROM base, `0xNNNNNN`.
    pub bank_rom_bases: Vec<String>,
    /// Size of one sample-table entry.
    pub table_entry_size_bytes: u32,
    /// How the table's Z80 words are stored.
    pub table_word_encoding: String,
    /// The banks and their samples.
    pub banks: Vec<DacBank>,
}

/// One `zBankTbl` entry.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DacTableEntry {
    /// The DAC sound id it serves.
    pub sound_id: u8,
    /// Bank number.
    pub bank: u8,
    /// Sample number within the bank.
    pub sample: u8,
}

/// One DAC bank.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DacBank {
    /// Bank number.
    pub bank: u8,
    /// ROM base, `0xNNNNNN`.
    pub rom_base: String,
    /// Samples in the bank.
    pub entry_count: u32,
    /// Raw copy of the bank's sample table, relative to `sound/`.
    pub table_raw_file: String,
    /// The bank's samples.
    pub samples: Vec<DacSample>,
}

/// One DAC sample.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DacSample {
    /// The DAC sound id that plays it.
    pub sound_id: u8,
    /// Bank number.
    pub bank: u8,
    /// Sample number within the bank.
    pub sample: u8,
    /// Its sample-table entry, `0xNNNNNN`.
    pub table_rom_offset: String,
    /// Z80 start address, `0xNNNN`.
    pub z80_start: String,
    /// Length.
    pub length_bytes: u32,
    /// Start, `0xNNNNNN`.
    pub rom_offset: String,
    /// Raw copy, relative to `sound/`.
    pub raw_file: String,
    /// SHA-256 of the raw copy.
    pub raw_sha256: String,
}

/// `sound/music.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MusicFile {
    /// [`SOUND_FORMAT_VERSION`].
    pub format_version: u32,
    /// `"music_records"`.
    pub kind: String,
    /// The source ROM.
    pub rom: RomStamp,
    /// The music records.
    pub records: RecordGroup<MusicHeader>,
}

/// `sound/sfx.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SfxFile {
    /// [`SOUND_FORMAT_VERSION`].
    pub format_version: u32,
    /// `"sfx_records"`.
    pub kind: String,
    /// The source ROM.
    pub rom: RomStamp,
    /// Regular and special SFX.
    pub records: SfxGroups,
}

/// The two SFX pointer tables.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SfxGroups {
    /// `SFXPtrs`.
    pub regular: RecordGroup<SfxHeader>,
    /// `SpcSFXPtrs`.
    pub special: RecordGroup<SfxHeader>,
}

/// One pointer table's records.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecordGroup<H> {
    /// The sound id of the table's first record.
    pub id_first: u8,
    /// The records, in table order.
    pub records: Vec<Record<H>>,
}

/// One music or SFX record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Record<H> {
    /// Retail sound id.
    pub id: u8,
    /// The id as `0xNN`.
    pub id_hex: String,
    /// Retail symbol.
    pub symbol: String,
    /// Where the record is and its raw copy.
    pub record: RawRecord,
    /// The record's header.
    pub header: H,
    /// Music only: how many field maps select this record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map_usage_count: Option<u32>,
    /// The record's whole FM voice table.
    pub voices: Vec<Voice>,
    /// The record's tracks, in descriptor order.
    pub tracks: Vec<Track>,
}

/// A record's place in the ROM and its raw copy.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RawRecord {
    /// Start, `0xNNNNNN`.
    pub rom_offset: String,
    /// End, exclusive: the next record's start.
    pub end_offset_exclusive: String,
    /// Length.
    pub size_bytes: u64,
    /// Raw copy, relative to `sound/`.
    pub raw_file: String,
    /// SHA-256 of the raw copy.
    pub raw_sha256: String,
}

/// A music record's header.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MusicHeader {
    /// The voice table's offset from the record start.
    pub voice_table_relative: u16,
    /// The voice table, `0xNNNNNN`.
    pub voice_table_rom_offset: String,
    /// FM channel descriptors.
    pub fm_channel_count: u8,
    /// PSG channel descriptors.
    pub psg_channel_count: u8,
    /// Header byte 4.
    pub tick_multiplier: u8,
    /// Header byte 5, the global tempo reload.
    pub tempo_reload: u8,
    /// Header and descriptors.
    pub header_size_bytes: u32,
    /// Whole 25-byte voices in the table.
    pub voice_count: u32,
    /// Zero bytes after the last voice.
    pub voice_padding_bytes: u32,
}

/// An SFX record's header.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SfxHeader {
    /// The voice table's offset from the record start.
    pub voice_table_relative: u16,
    /// The voice table, `0xNNNNNN`.
    pub voice_table_rom_offset: String,
    /// Header byte 2.
    pub tick_multiplier: u8,
    /// Header byte 3.
    pub track_count: u8,
    /// Header and descriptors.
    pub header_size_bytes: u32,
    /// Whole 25-byte voices in the table.
    pub voice_count: u32,
    /// Zero bytes after the last voice.
    pub voice_padding_bytes: u32,
}

/// One 25-byte FM voice.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Voice {
    /// Index in the record's voice table.
    pub index: u32,
    /// Start, `0xNNNNNN`.
    pub rom_offset: String,
    /// Always 25.
    pub size_bytes: u32,
    /// The voice, as lowercase hex.
    pub raw_hex: String,
    /// Byte 0.
    pub algorithm_feedback: u8,
    /// Bytes 1-20.
    pub operator_registers: Vec<u8>,
    /// Bytes 21-24.
    pub operator_total_levels: Vec<u8>,
}

/// A track descriptor's family.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrackFamily {
    /// A music FM descriptor.
    Fm,
    /// A music PSG descriptor.
    Psg,
    /// An SFX descriptor; its channel byte names the channel.
    Sfx,
}

/// One track and its static decode.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Track {
    /// `music:NN:fm:I`, `music:NN:psg:I` or `sfx:NN:I`.
    pub track_id: String,
    /// The descriptor's family.
    pub kind: TrackFamily,
    /// Index among the record's descriptors of this family.
    pub index: u8,
    /// Where the track starts.
    pub source: TrackSource,
    /// The descriptor's initial values.
    pub initial: TrackInitial,
    /// FM voices selected by `EF`, ascending.
    pub instruments_used: Vec<u8>,
    /// PSG envelopes selected by `F5`, ascending.
    pub psg_instruments_used: Vec<u8>,
    /// What the static walk saw.
    pub decode: TrackDecode,
}

/// A track's start.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TrackSource {
    /// Start, `0xNNNNNN`.
    pub rom_offset: String,
    /// Start, from the record start.
    pub record_relative_offset: u32,
    /// The record's end, `0xNNNNNN`.
    pub record_end_exclusive: String,
    /// The descriptor, as lowercase hex.
    pub raw_descriptor_hex: String,
}

/// A descriptor's initial values; which are present depends on the family.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TrackInitial {
    /// SFX: the descriptor's channel word.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_word: Option<u16>,
    /// SFX: its low byte, the channel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_byte: Option<u8>,
    /// Transpose byte, unsigned as stored.
    pub transpose: u8,
    /// Volume byte, unsigned as stored.
    pub volume: u8,
    /// Music PSG: the modulation envelope byte.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modulation_envelope: Option<u8>,
    /// Music PSG: the volume envelope byte.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume_envelope: Option<u8>,
}

/// What the static walk of one track saw.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TrackDecode {
    /// Distinct (position, call stack) states reached.
    pub reachable_instruction_states: u32,
    /// Distinct bytes on a reachable path.
    pub reachable_byte_count: u32,
    /// Primary command opcodes met, as `0xNN`, ascending.
    pub command_opcodes_seen: Vec<String>,
    /// Primary command counts keyed by `NN`.
    pub command_counts: BTreeMap<String, u32>,
    /// `FF` meta opcodes met, as `0xNN`, ascending.
    pub meta_opcodes_seen: Vec<String>,
    /// Meta command counts keyed by `NN`.
    pub meta_counts: BTreeMap<String, u32>,
    /// Every byte value on a reachable path, as `0xNN`, ascending.
    pub sequence_byte_values_seen: Vec<String>,
    /// Deepest `F8` nesting.
    pub max_call_depth: u32,
    /// Whether an `F2` is reachable.
    pub has_track_end: bool,
}
