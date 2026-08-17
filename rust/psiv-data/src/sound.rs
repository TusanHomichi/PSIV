//! Typed loading for the extracted `runtime-pack/sound` records.
//!
//! This module stops at resolved cartridge data: raw track tails, local FM
//! voices, retail PSG envelope bytes, and DAC sample banks. The sound crate
//! owns interpretation and chip output; this crate owns the pack schema and
//! file provenance.

use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const SOUND_FORMAT_VERSION: u32 = 1;

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
        let index = read_json(&index_path)?;
        check_version(&index, &index_path)?;

        let driver_path = root.join("driver.json");
        let driver = read_json(&driver_path)?;
        check_version(&driver, &driver_path)?;
        let volume_envelopes = load_volume_envelopes(&root, &driver)?;
        let dac_samples = load_dac_samples(&root, &driver)?;

        let music_path = root.join("music.json");
        let music_json = read_json(&music_path)?;
        check_version(&music_json, &music_path)?;
        let mut music = BTreeMap::new();
        for value in records(&music_json, &music_path, None)? {
            let record = load_record(&root, value, false, &volume_envelopes)?;
            insert_record(&mut music, record, &music_path)?;
        }

        let sfx_path = root.join("sfx.json");
        let sfx_json = read_json(&sfx_path)?;
        check_version(&sfx_json, &sfx_path)?;
        let mut sfx = BTreeMap::new();
        for group in ["regular", "special"] {
            for value in records(&sfx_json, &sfx_path, Some(group))? {
                let record = load_record(&root, value, true, &volume_envelopes)?;
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

fn load_record(
    root: &Path,
    value: &Value,
    is_sfx: bool,
    _volume_envelopes: &[Vec<u8>],
) -> Result<SoundRecord, crate::DataError> {
    let id = u8_field(value, "id", root)?;
    let symbol = string_field(value, "symbol", root)?.to_owned();
    let header = object_field(value, "header", root)?;
    let tempo = if is_sfx {
        1
    } else {
        u8_field(header, "tempo_reload", root)?.max(1)
    };
    let tick_multiplier = u8_field(header, "tick_multiplier", root)?.max(1);
    let record = object_field(value, "record", root)?;
    let raw_file = string_field(record, "raw_file", root)?;
    let raw_path = root.join(raw_file);
    let raw = std::fs::read(&raw_path).map_err(|error| crate::DataError::io(&raw_path, error))?;
    let voices = parse_voices(value, root)?;
    let mut tracks = Vec::new();
    let tracks_value = value
        .get("tracks")
        .and_then(Value::as_array)
        .ok_or_else(|| sound_error(root.to_owned(), "record.tracks is not an array"))?;
    for track in tracks_value {
        tracks.push(load_track(track, &raw, tick_multiplier, is_sfx, root)?);
    }
    Ok(SoundRecord {
        id,
        symbol,
        tempo,
        voices,
        tracks,
    })
}

fn load_track(
    value: &Value,
    raw: &[u8],
    tick_multiplier: u8,
    is_sfx: bool,
    root: &Path,
) -> Result<SoundTrackRecord, crate::DataError> {
    let kind_name = string_field(value, "kind", root)?;
    let initial = object_field(value, "initial", root)?;
    let source = object_field(value, "source", root)?;
    let offset = usize_field(source, "record_relative_offset", root)?;
    if offset >= raw.len() {
        return Err(sound_error(
            root.to_owned(),
            format!("track pointer {offset} is outside its raw record"),
        ));
    }
    let index = u8_field(value, "index", root)?;
    let (kind, channel) = if is_sfx {
        let channel_byte = u8_field(initial, "channel_byte", root)?;
        if channel_byte & 0x80 != 0 {
            let channel = (channel_byte >> 5).saturating_sub(4);
            (SoundTrackKind::Psg, channel)
        } else {
            (SoundTrackKind::Fm, channel_byte)
        }
    } else if kind_name == "psg" {
        (SoundTrackKind::Psg, index)
    } else if kind_name == "fm" {
        (SoundTrackKind::Fm, index)
    } else {
        return Err(sound_error(
            root.to_owned(),
            format!("unknown music track kind {kind_name:?}"),
        ));
    };
    let initial_voice = value
        .get("instruments_used")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(Value::as_u64)
        .map(|value| value as u8);
    let psg_envelope = initial
        .get("volume_envelope")
        .and_then(Value::as_u64)
        .and_then(|value| value.checked_sub(1))
        .map(|value| value as usize);
    let transpose = u8_field(initial, "transpose", root)? as i8;
    let volume = u8_field(initial, "volume", root)? as i8;
    let opcodes = value
        .get("decode")
        .and_then(|decode| decode.get("command_opcodes_seen"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str);
    let dac = !is_sfx
        && kind == SoundTrackKind::Fm
        && opcodes
            .clone()
            .any(|opcode| matches!(opcode, "0xE3" | "0xE4" | "0xED" | "0xEE" | "0xFA" | "0xFC"));
    Ok(SoundTrackRecord {
        kind,
        channel,
        bytes: raw.to_vec(),
        start_offset: offset,
        initial_voice,
        initial_transpose: transpose,
        initial_volume: volume,
        psg_envelope,
        tick_multiplier,
        dac,
    })
}

fn parse_voices(value: &Value, root: &Path) -> Result<Vec<Vec<u8>>, crate::DataError> {
    let voices = value
        .get("voices")
        .and_then(Value::as_array)
        .ok_or_else(|| sound_error(root.to_owned(), "record.voices is not an array"))?;
    voices
        .iter()
        .map(|voice| {
            let raw_hex = string_field(voice, "raw_hex", root)?;
            let bytes =
                parse_hex(raw_hex).map_err(|message| sound_error(root.to_owned(), message))?;
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

fn load_volume_envelopes(root: &Path, driver: &Value) -> Result<Vec<Vec<u8>>, crate::DataError> {
    let values = driver
        .get("driver")
        .and_then(|value| value.get("envelopes"))
        .and_then(|value| value.get("volume"))
        .and_then(Value::as_array)
        .ok_or_else(|| sound_error(root.to_owned(), "driver.envelopes.volume is missing"))?;
    let mut envelopes = vec![Vec::new(); values.len()];
    for value in values {
        let index = usize_field(value, "index", root)?;
        let path = root.join(string_field(value, "raw_file", root)?);
        let bytes = std::fs::read(&path).map_err(|error| crate::DataError::io(&path, error))?;
        if index >= envelopes.len() {
            return Err(sound_error(
                path,
                format!("envelope index {index} is out of order"),
            ));
        }
        envelopes[index] = bytes;
    }
    if envelopes.iter().any(Vec::is_empty) {
        return Err(sound_error(
            root.to_owned(),
            "volume-envelope table has a missing stream",
        ));
    }
    Ok(envelopes)
}

fn load_dac_samples(
    root: &Path,
    driver: &Value,
) -> Result<BTreeMap<u8, Vec<u8>>, crate::DataError> {
    let banks = driver
        .get("driver")
        .and_then(|value| value.get("dac"))
        .and_then(|value| value.get("banks"))
        .and_then(Value::as_array)
        .ok_or_else(|| sound_error(root.to_owned(), "driver.dac.banks is missing"))?;
    let mut samples = BTreeMap::new();
    for bank in banks {
        let entries = bank
            .get("samples")
            .and_then(Value::as_array)
            .ok_or_else(|| sound_error(root.to_owned(), "DAC bank samples is not an array"))?;
        for entry in entries {
            let id = u8_field(entry, "sound_id", root)?;
            let path = root.join(string_field(entry, "raw_file", root)?);
            let bytes = std::fs::read(&path).map_err(|error| crate::DataError::io(&path, error))?;
            if samples.insert(id, bytes).is_some() {
                return Err(sound_error(
                    path,
                    format!("duplicate DAC sound id {id:#04x}"),
                ));
            }
        }
    }
    Ok(samples)
}

fn records<'a>(
    value: &'a Value,
    path: &Path,
    group: Option<&str>,
) -> Result<Vec<&'a Value>, crate::DataError> {
    let records = value
        .get("records")
        .and_then(Value::as_object)
        .ok_or_else(|| sound_error(path.to_owned(), "records is not an object"))?;
    let container = group
        .map(|name| records.get(name).and_then(|value| value.get("records")))
        .unwrap_or_else(|| records.get("records"));
    container
        .and_then(Value::as_array)
        .map(|items| items.iter().collect())
        .ok_or_else(|| sound_error(path.to_owned(), "record list is not an array"))
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

fn read_json(path: &Path) -> Result<Value, crate::DataError> {
    let text = std::fs::read_to_string(path).map_err(|error| crate::DataError::io(path, error))?;
    serde_json::from_str(&text).map_err(|error| crate::DataError::json(path, error))
}

fn check_version(value: &Value, path: &Path) -> Result<(), crate::DataError> {
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
    Ok(())
}

fn object_field<'a>(
    value: &'a Value,
    field: &str,
    path: &Path,
) -> Result<&'a Value, crate::DataError> {
    value
        .get(field)
        .filter(|value| value.is_object())
        .ok_or_else(|| sound_error(path.to_owned(), format!("{field} is not an object")))
}

fn string_field<'a>(
    value: &'a Value,
    field: &str,
    path: &Path,
) -> Result<&'a str, crate::DataError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| sound_error(path.to_owned(), format!("{field} is not a string")))
}

fn u8_field(value: &Value, field: &str, path: &Path) -> Result<u8, crate::DataError> {
    let value = value.get(field).and_then(Value::as_u64).ok_or_else(|| {
        sound_error(
            path.to_owned(),
            format!("{field} is not an unsigned integer"),
        )
    })?;
    u8::try_from(value)
        .map_err(|_| sound_error(path.to_owned(), format!("{field} does not fit in one byte")))
}

fn usize_field(value: &Value, field: &str, path: &Path) -> Result<usize, crate::DataError> {
    let value = value.get(field).and_then(Value::as_u64).ok_or_else(|| {
        sound_error(
            path.to_owned(),
            format!("{field} is not an unsigned integer"),
        )
    })?;
    usize::try_from(value)
        .map_err(|_| sound_error(path.to_owned(), format!("{field} does not fit in usize")))
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
