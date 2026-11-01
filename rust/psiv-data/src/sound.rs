//! Typed loading for the extracted `runtime-pack/sound` records.
//!
//! This module stops at resolved cartridge data: raw track tails, local FM
//! voices, retail PSG envelope bytes, and DAC sample banks. The sound crate
//! owns interpretation and chip output; this crate owns the pack schema and
//! file provenance; [`schema`] is the files' shape, shared with the pack
//! builder.

use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub mod schema;

pub use schema::SOUND_FORMAT_VERSION;

/// The channel family named by an extracted track descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SoundTrackKind {
    /// YM2612 FM channel.
    Fm,
    /// SN76489 PSG channel.
    Psg,
}

/// One track with its raw command stream and resolved descriptor metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SoundTrackRecord {
    /// FM or PSG family.
    pub kind: SoundTrackKind,
    /// Physical channel, after SFX channel-byte decoding.
    pub channel: u8,
    /// Complete raw record bytes, retained so relative calls can reach shared
    /// subroutines before the track pointer.
    pub bytes: Vec<u8>,
    /// Record-relative command cursor from the extracted track pointer.
    pub start_offset: usize,
    /// First statically referenced FM voice, if the track has one.
    pub initial_voice: Option<u8>,
    /// Initial transpose byte interpreted as signed two's-complement.
    pub initial_transpose: i8,
    /// Initial volume/attenuation byte.
    pub initial_volume: i8,
    /// PSG volume-envelope table index, if this is a PSG track.
    pub psg_envelope: Option<usize>,
    /// Header tick multiplier.
    pub tick_multiplier: u8,
    /// True for the dedicated music DAC track.
    pub dac: bool,
}

/// One extracted music or SFX record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SoundRecord {
    /// Retail sound ID.
    pub id: u8,
    /// Retail symbol from the pointer table.
    pub symbol: String,
    /// Global tempo reload for music, or one for SFX.
    pub tempo: u8,
    /// Resolved local FM voice records.
    pub voices: Vec<Vec<u8>>,
    /// Resolved track descriptors and raw command tails.
    pub tracks: Vec<SoundTrackRecord>,
}

/// All extracted sound files in a runtime pack.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SoundFiles {
    music: BTreeMap<u8, SoundRecord>,
    sfx: BTreeMap<u8, SoundRecord>,
    volume_envelopes: Vec<Vec<u8>>,
    dac_samples: BTreeMap<u8, Vec<u8>>,
}

impl SoundFiles {
    /// Load `sound/` when present. Older synthetic packs without a sound
    /// directory remain valid and expose an empty sound set.
    pub fn load(pack_dir: &Path) -> Result<Self, crate::DataError> {
        let root = pack_dir.join("sound");
        let index_path = root.join("index.json");
        if !index_path.is_file() {
            if root.exists() {
                return Err(sound_error(index_path, "sound directory has no index.json"));
            }
            return Ok(Self::default());
        }
        let _index: schema::IndexFile = read_file(&index_path)?;

        let driver: schema::DriverFile = read_file(&root.join("driver.json"))?;
        let volume_envelopes = load_volume_envelopes(&root, &driver.driver.envelopes)?;
        let dac_samples = load_dac_samples(&root, &driver.driver.dac)?;

        let music_path = root.join("music.json");
        let music_file: schema::MusicFile = read_file(&music_path)?;
        let mut music = BTreeMap::new();
        for value in &music_file.records.records {
            let tempo = value.header.tempo_reload.max(1);
            let record = load_record(&root, value, value.header.tick_multiplier, tempo, false)?;
            insert_record(&mut music, record, &music_path)?;
        }

        let sfx_path = root.join("sfx.json");
        let sfx_file: schema::SfxFile = read_file(&sfx_path)?;
        let mut sfx = BTreeMap::new();
        for group in [&sfx_file.records.regular, &sfx_file.records.special] {
            for value in &group.records {
                let record = load_record(&root, value, value.header.tick_multiplier, 1, true)?;
                insert_record(&mut sfx, record, &sfx_path)?;
            }
        }

        Ok(Self {
            music,
            sfx,
            volume_envelopes,
            dac_samples,
        })
    }

    /// Find an extracted music record.
    pub fn music(&self, id: u8) -> Option<&SoundRecord> {
        self.music.get(&id)
    }

    /// Find an extracted regular or special SFX record.
    pub fn sfx(&self, id: u8) -> Option<&SoundRecord> {
        self.sfx.get(&id)
    }

    /// All music records in retail ID order.
    pub fn music_records(&self) -> impl Iterator<Item = &SoundRecord> {
        self.music.values()
    }

    /// All SFX records in retail ID order.
    pub fn sfx_records(&self) -> impl Iterator<Item = &SoundRecord> {
        self.sfx.values()
    }

    /// Raw volume-envelope streams, indexed by the retail `F5` selector.
    pub fn volume_envelopes(&self) -> &[Vec<u8>] {
        &self.volume_envelopes
    }

    /// Raw DAC sample streams keyed by their Z80-visible sound ID (`$81..$91`).
    pub fn dac_samples(&self) -> impl Iterator<Item = (u8, &[u8])> {
        self.dac_samples
            .iter()
            .map(|(id, bytes)| (*id, bytes.as_slice()))
    }

    /// Whether this pack has no extracted sound records.
    pub fn is_empty(&self) -> bool {
        self.music.is_empty() && self.sfx.is_empty()
    }
}

fn load_record<H>(
    root: &Path,
    value: &schema::Record<H>,
    tick_multiplier: u8,
    tempo: u8,
    is_sfx: bool,
) -> Result<SoundRecord, crate::DataError> {
    let tick_multiplier = tick_multiplier.max(1);
    let raw_path = root.join(&value.record.raw_file);
    let raw = std::fs::read(&raw_path).map_err(|error| crate::DataError::io(&raw_path, error))?;
    let voices = parse_voices(&value.voices, root)?;
    let tracks = value
        .tracks
        .iter()
        .map(|track| load_track(track, &raw, tick_multiplier, is_sfx, root))
        .collect::<Result<_, _>>()?;
    Ok(SoundRecord {
        id: value.id,
        symbol: value.symbol.clone(),
        tempo,
        voices,
        tracks,
    })
}

fn load_track(
    value: &schema::Track,
    raw: &[u8],
    tick_multiplier: u8,
    is_sfx: bool,
    root: &Path,
) -> Result<SoundTrackRecord, crate::DataError> {
    let initial = &value.initial;
    let offset = value.source.record_relative_offset as usize;
    if offset >= raw.len() {
        return Err(sound_error(
            root.to_owned(),
            format!("track pointer {offset} is outside its raw record"),
        ));
    }
    let (kind, channel) = if is_sfx {
        let channel_byte = initial.channel_byte.ok_or_else(|| {
            sound_error(root.to_owned(), "channel_byte is not an unsigned integer")
        })?;
        if channel_byte & 0x80 != 0 {
            let channel = (channel_byte >> 5).saturating_sub(4);
            (SoundTrackKind::Psg, channel)
        } else {
            (SoundTrackKind::Fm, channel_byte)
        }
    } else {
        match value.kind {
            schema::TrackFamily::Psg => (SoundTrackKind::Psg, value.index),
            schema::TrackFamily::Fm => (SoundTrackKind::Fm, value.index),
            schema::TrackFamily::Sfx => {
                return Err(sound_error(
                    root.to_owned(),
                    "unknown music track kind \"sfx\"",
                ));
            }
        }
    };
    let initial_voice = value.instruments_used.first().copied();
    let psg_envelope = initial
        .volume_envelope
        .and_then(|value| value.checked_sub(1))
        .map(usize::from);
    let dac = !is_sfx
        && kind == SoundTrackKind::Fm
        && value.decode.command_opcodes_seen.iter().any(|opcode| {
            matches!(
                opcode.as_str(),
                "0xE3" | "0xE4" | "0xED" | "0xEE" | "0xFA" | "0xFC"
            )
        });
    Ok(SoundTrackRecord {
        kind,
        channel,
        bytes: raw.to_vec(),
        start_offset: offset,
        initial_voice,
        initial_transpose: initial.transpose as i8,
        initial_volume: initial.volume as i8,
        psg_envelope,
        tick_multiplier,
        dac,
    })
}

fn parse_voices(voices: &[schema::Voice], root: &Path) -> Result<Vec<Vec<u8>>, crate::DataError> {
    voices
        .iter()
        .map(|voice| {
            let bytes = parse_hex(&voice.raw_hex)
                .map_err(|message| sound_error(root.to_owned(), message))?;
            if bytes.len() != 25 {
                return Err(sound_error(
                    root.to_owned(),
                    format!("FM voice has {} bytes, expected 25", bytes.len()),
                ));
            }
            Ok(bytes)
        })
        .collect()
}

fn load_volume_envelopes(
    root: &Path,
    envelopes: &schema::Envelopes,
) -> Result<Vec<Vec<u8>>, crate::DataError> {
    let values = &envelopes.volume;
    let mut loaded = vec![Vec::new(); values.len()];
    for value in values {
        let index = value.index as usize;
        let path = root.join(&value.raw_file);
        let bytes = std::fs::read(&path).map_err(|error| crate::DataError::io(&path, error))?;
        if index >= loaded.len() {
            return Err(sound_error(
                path,
                format!("envelope index {index} is out of order"),
            ));
        }
        loaded[index] = bytes;
    }
    if loaded.iter().any(Vec::is_empty) {
        return Err(sound_error(
            root.to_owned(),
            "volume-envelope table has a missing stream",
        ));
    }
    Ok(loaded)
}

fn load_dac_samples(
    root: &Path,
    dac: &schema::Dac,
) -> Result<BTreeMap<u8, Vec<u8>>, crate::DataError> {
    let mut samples = BTreeMap::new();
    for entry in dac.banks.iter().flat_map(|bank| &bank.samples) {
        let id = entry.sound_id;
        let path = root.join(&entry.raw_file);
        let bytes = std::fs::read(&path).map_err(|error| crate::DataError::io(&path, error))?;
        if samples.insert(id, bytes).is_some() {
            return Err(sound_error(
                path,
                format!("duplicate DAC sound id {id:#04x}"),
            ));
        }
    }
    Ok(samples)
}

fn insert_record(
    records: &mut BTreeMap<u8, SoundRecord>,
    record: SoundRecord,
    path: &Path,
) -> Result<(), crate::DataError> {
    if records.insert(record.id, record).is_some() {
        return Err(sound_error(path.to_owned(), "duplicate sound record id"));
    }
    Ok(())
}

/// Read a sound file: its `format_version` is checked before its shape, so
/// a pack from another version is refused as one.
fn read_file<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, crate::DataError> {
    let text = std::fs::read_to_string(path).map_err(|error| crate::DataError::io(path, error))?;
    let value: Value =
        serde_json::from_str(&text).map_err(|error| crate::DataError::json(path, error))?;
    let found = value
        .get("format_version")
        .and_then(Value::as_u64)
        .ok_or_else(|| sound_error(path.to_owned(), "format_version is missing"))?
        as u32;
    if found != SOUND_FORMAT_VERSION {
        return Err(crate::DataError::FormatVersion {
            found,
            expected: SOUND_FORMAT_VERSION,
        });
    }
    serde_json::from_value(value).map_err(|error| crate::DataError::json(path, error))
}

fn parse_hex(raw: &str) -> Result<Vec<u8>, String> {
    if !raw.len().is_multiple_of(2) {
        return Err("hex payload has odd length".into());
    }
    (0..raw.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&raw[index..index + 2], 16)
                .map_err(|_| format!("invalid hex byte at offset {index}"))
        })
        .collect()
}

fn sound_error(path: PathBuf, message: impl Into<String>) -> crate::DataError {
    crate::DataError::Sound {
        path,
        message: message.into(),
    }
}
