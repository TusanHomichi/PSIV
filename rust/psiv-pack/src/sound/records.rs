//! Voices, tracks, the music and effect records and the envelopes.

use super::*;

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
pub(super) fn voice_table(
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

pub(super) fn music_record(
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

pub(super) fn sfx_record(
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

pub(super) fn envelopes(
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
