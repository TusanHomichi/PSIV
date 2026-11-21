//! `title/sound_test.json`: the post-clear Sound Test track table
//! (`Title_SoundTestLoop`, `SoundTestStrings`, `SoundTestData`,
//! `ps4.asm:88525-88645`, `346897-347010`).

use crate::PackError;
use crate::json::Json;

/// `Title_SoundTestLoop` ROM offsets in the US retail image.
pub(super) const TRACK_MAX_CMPI: usize = 0x044258;
pub(super) const PLAY_TABLE_LEA: usize = 0x044296;
pub(super) const REPEAT_CHECK_CMPI: usize = 0x0442A4;
pub(super) const REPEAT_OVERRIDE_MOVE: usize = 0x0442B8;
pub(super) const STRINGS_LEA: usize = 0x044386;
pub(super) const GAKI_CHAN_LEA: usize = 0x04439A;
pub(super) const COMPOSER_TABLE_LEA: usize = 0x0443A6;
pub(super) const IPPO_LEA: usize = 0x0443B2;

const LEA_ABS_A0: [u8; 2] = [0x41, 0xF9];
const LEA_ABS_A1: [u8; 2] = [0x43, 0xF9];

pub(super) const PATH: &str = "title/sound_test.json";

fn slice<'a>(rom: &'a [u8], at: usize, len: usize, what: &str) -> Result<&'a [u8], PackError> {
    rom.get(at..at + len)
        .ok_or_else(|| PackError(format!("{what} at 0x{at:06X} runs past the end of the ROM")))
}

fn read_lea(rom: &[u8], at: usize, opcode: [u8; 2], what: &str) -> Result<usize, PackError> {
    let bytes = slice(rom, at, 6, what)?;
    if bytes[..2] != opcode {
        return Err(PackError(format!(
            "{what} opcode mismatch at 0x{at:06X}: expected {:02X}{:02X}, got {:02X}{:02X}",
            opcode[0], opcode[1], bytes[0], bytes[1]
        )));
    }
    Ok(u32::from_be_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]) as usize)
}

/// Decodes one `$FE`-terminated string in the Sound Test window charset
/// (`ps4.asm:346891-346896`).
fn decode_sound_test_string(
    rom: &[u8],
    start: usize,
    what: &str,
) -> Result<(String, usize), PackError> {
    let mut at = start;
    let mut out = String::new();
    loop {
        let byte = *rom
            .get(at)
            .ok_or_else(|| PackError(format!("{what} at 0x{start:06X} missing $FE terminator")))?;
        at += 1;
        if byte == 0xFE {
            return Ok((out, at));
        }
        let ch = match byte {
            0x00 => ' ',
            0x80..=0x99 => (b'A' + (byte - 0x80)) as char,
            0xA4..=0xAD => (b'0' + (byte - 0xA4)) as char,
            0xB0 => '-',
            0xB1 => '!',
            other => {
                return Err(PackError(format!(
                    "{what} at 0x{:06X}: unexpected sound test byte 0x{other:02X}",
                    at - 1
                )));
            }
        };
        out.push(ch);
        if out.len() > 64 {
            return Err(PackError(format!(
                "{what} at 0x{start:06X} exceeded 64 chars without $FE"
            )));
        }
    }
}

/// Extracts `title/sound_test.json` from the US retail ROM.
pub(super) fn extract(rom: &[u8]) -> Result<Json, PackError> {
    // cmpi.b #$2F, ($E200).w -> 0C 38 00 <max> E2 00
    let cmpi = slice(rom, TRACK_MAX_CMPI, 6, "Title_SoundTestLoop track cap")?;
    if cmpi[0..3] != [0x0C, 0x38, 0x00] || cmpi[4..6] != [0xE2, 0x00] {
        return Err(PackError(format!(
            "Title_SoundTestLoop track cap opcode mismatch at 0x{TRACK_MAX_CMPI:06X}"
        )));
    }
    let track_count = usize::from(cmpi[3]) + 1;

    // Repeat override:
    // cmpi.b #<base>, ($FFFF500A).l at 0x0442A4 (0C 39 00 <base> FF FF 50 0A)
    // move.b #<cont>, ($FFFF500A).l at 0x0442B8 (13 FC 00 <cont> FF FF 50 0A)
    let rcmp = slice(
        rom,
        REPEAT_CHECK_CMPI,
        8,
        "Title_SoundTestLoop repeat check",
    )?;
    if rcmp[0..3] != [0x0C, 0x39, 0x00] || rcmp[4..8] != [0xFF, 0xFF, 0x50, 0x0A] {
        return Err(PackError(format!(
            "Title_SoundTestLoop repeat check opcode mismatch at 0x{REPEAT_CHECK_CMPI:06X}"
        )));
    }
    let rmov = slice(
        rom,
        REPEAT_OVERRIDE_MOVE,
        8,
        "Title_SoundTestLoop repeat override",
    )?;
    if rmov[0..3] != [0x13, 0xFC, 0x00] || rmov[4..8] != [0xFF, 0xFF, 0x50, 0x0A] {
        return Err(PackError(format!(
            "Title_SoundTestLoop repeat override opcode mismatch at 0x{REPEAT_OVERRIDE_MOVE:06X}"
        )));
    }
    let repeat_when_id = rcmp[3];
    let repeat_play_id = rmov[3];

    let data_offset = read_lea(rom, PLAY_TABLE_LEA, LEA_ABS_A0, "SoundTestData play lea")?;
    let composer_table_offset = read_lea(
        rom,
        COMPOSER_TABLE_LEA,
        LEA_ABS_A1,
        "SoundTestData composer lea",
    )?;
    if data_offset != composer_table_offset {
        return Err(PackError(format!(
            "SoundTestData lea mismatch: 0x{data_offset:06X} vs 0x{composer_table_offset:06X}"
        )));
    }
    let strings_offset = read_lea(rom, STRINGS_LEA, LEA_ABS_A0, "SoundTestStrings lea")?;
    let gaki_offset = read_lea(rom, GAKI_CHAN_LEA, LEA_ABS_A0, "GakiChanString lea")?;
    let ippo_offset = read_lea(rom, IPPO_LEA, LEA_ABS_A0, "IppoString lea")?;

    let (ippo_raw, _) = decode_sound_test_string(rom, ippo_offset, "IppoString")?;
    let (gaki_raw, _) = decode_sound_test_string(rom, gaki_offset, "GakiChanString")?;
    let ippo = ippo_raw.trim().to_owned();
    let gaki = gaki_raw.trim().to_owned();

    let table = slice(rom, data_offset, track_count * 2 + 1, "SoundTestData")?;
    if table[track_count * 2] != 0xFF {
        return Err(PackError(format!(
            "SoundTestData at 0x{data_offset:06X} missing $FF terminator after {track_count} entries"
        )));
    }

    let mut str_at = strings_offset;
    let mut tracks = Vec::with_capacity(track_count);
    for idx in 0..track_count {
        let (title, next_at) = decode_sound_test_string(rom, str_at, "SoundTestStrings")?;
        str_at = next_at;
        let sound_id = table[idx * 2];
        let composer_id = table[idx * 2 + 1];
        let composer = match composer_id {
            0 => ippo.as_str(),
            1 => gaki.as_str(),
            other => {
                return Err(PackError(format!(
                    "SoundTestData track {idx} has unknown composer id {other}"
                )));
            }
        };
        tracks.push(
            Json::object()
                .with("index", idx)
                .with("title", title)
                .with("sound_id", sound_id)
                .with("composer_id", composer_id)
                .with("composer", composer),
        );
    }
    let sentinel = *rom
        .get(str_at)
        .ok_or_else(|| PackError("SoundTestStrings missing trailing sentinel".into()))?;
    if sentinel != 0xFE {
        return Err(PackError(format!(
            "SoundTestStrings expected trailing $FE sentinel at 0x{str_at:06X}, got 0x{sentinel:02X}"
        )));
    }

    Ok(Json::object()
        .with("format_version", 1u8)
        .with("kind", "psiv_sound_test")
        .with("strings_offset", super::hex6(strings_offset))
        .with("data_offset", super::hex6(data_offset))
        .with("track_count", track_count)
        .with(
            "repeat_override",
            Json::object()
                .with("when_sound_id", repeat_when_id)
                .with("play_sound_id", repeat_play_id),
        )
        .with("tracks", tracks))
}

/// Writes `title/sound_test.json` and returns its manifest summary.
pub(super) fn write(rom: &[u8], root: &std::path::Path) -> Result<Json, PackError> {
    let doc = extract(rom)?;
    let count = match doc.get("track_count") {
        Some(Json::Int(n)) => *n as usize,
        _ => 0,
    };
    let sha = crate::write_json(root, PATH, &doc)?;
    Ok(Json::object()
        .with("path", PATH)
        .with("sha256", sha)
        .with("track_count", count))
}
