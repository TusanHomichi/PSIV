//! `guild/guild.json` and `fifth/fifth.json`: the tables two scenes read from
//! the pack, the port of `psiv_tools/guild_pack.py` and
//! `psiv_tools/fifth_pack.py`, whose module docs give the routines and why
//! the data belongs to the pack. Each table is found through the instruction
//! that names it, and each of those instructions is checked first.
//!
//! - `Event_HuntersGuild` keeps the job board's rows, money and names behind
//!   its code, and draws the list window in three colour lines.
//! - `Event_PickingFifthCharacter` keeps the five waiting friends' rows and
//!   the character records it refills.

use std::path::Path;

use sha2::{Digest, Sha256};

use crate::json::Json;
use crate::ship_menu::WIN_GROUP_EVENT;
use crate::text::{WINDOW, decode_name};
use crate::{PackError, Rom, gfx};

/// The opcodes the checks look for: `lea (xxx).l, a0`, `jsr (xxx).l` and
/// `move.b #imm, d0`. These are 68000 instruction encodings.
const LEA_ABS_A0: u16 = 0x41F9;
const JSR_ABS: u16 = 0x4EB9;
const MOVE_B_IMM_D0: u16 = 0x103C;

/// The two files keep their own format version, as the Python writes them.
const TABLE_FORMAT_VERSION: u8 = 1;

fn refuse<T>(message: String) -> Result<T, PackError> {
    Err(PackError(message))
}

fn hex6(value: usize) -> String {
    format!("0x{value:06X}")
}

fn word(rom: &[u8], at: usize) -> Result<u16, PackError> {
    Ok(crate::m68k::w(rom, at)?)
}

fn long(rom: &[u8], at: usize) -> Result<usize, PackError> {
    Ok(crate::m68k::l(rom, at)? as usize)
}

fn bytes(rom: &[u8], at: usize, len: usize) -> Result<&[u8], PackError> {
    rom.get(at..at + len)
        .ok_or_else(|| PackError(format!("0x{at:06X} (+{len}) runs past the end of the ROM")))
}

fn sha256(rom: &[u8]) -> String {
    crate::png::hex(&Sha256::digest(rom))
}

// ---------------------------------------------------------------------------
// The Hunter's Guild
// ---------------------------------------------------------------------------

/// `Event_HuntersGuild`, whose first instruction loads `HuntersGuildData`.
const GUILD_EVENT: usize = 0x0718E6;
const GUILD_DATA: usize = 0x071D74;
const GUILD_ROWS: usize = 8;
const GUILD_ROW_BYTES: usize = 8;
/// `HuntersGuildMoneyTbl`, then `HuntersGuildTextOffset`.
const GUILD_MONEY: usize = GUILD_DATA + GUILD_ROWS * GUILD_ROW_BYTES;
const GUILD_NAME_OFFSETS: usize = GUILD_MONEY + GUILD_ROWS;
/// The list loop's `lea (GuildText_ListingPending).l, a0`, and its target.
const PENDING_LEA: usize = 0x071AE2;
const PENDING: usize = 0x071E46;
/// `move.w #$B, d0` / `Window_Create`: the list window's record.
const GUILD_WINDOW: usize = 0x0B;
/// `HuntersGuild`'s map palette, named by its map record's pointer.
const MAP_PALETTE_POINTER: usize = 0x148BA4;
const MAP_PALETTE: usize = 0x12F510;
/// `Pal_Init_Line_3`, the colours the font's PNG is drawn in.
const PAL_INIT_LINE_3: usize = 0x296300;
const LINE_WORDS: usize = 16;

fn line(rom: &[u8], at: usize) -> Result<Json, PackError> {
    let colours = gfx::palette_rgb(&gfx::decode_palette(bytes(rom, at, LINE_WORDS * 2)?)?);
    Ok(Json::from(
        colours.iter().map(|c| c.to_vec()).collect::<Vec<_>>(),
    ))
}

/// A job name: window glyphs up to the `$FE` within 32 bytes.
fn job_name(rom: &[u8], at: usize) -> Result<String, PackError> {
    let window = rom.get(at..(at + 32).min(rom.len())).unwrap_or_default();
    let Some(end) = window.iter().position(|&b| b == 0xFE) else {
        return refuse(format!("no $FE ends the job name at 0x{at:06X}"));
    };
    Ok(decode_name(&window[..end], WINDOW)?)
}

/// `extract_guild`.
fn guild(rom: &[u8]) -> Result<Json, PackError> {
    if word(rom, GUILD_EVENT)? != LEA_ABS_A0 || long(rom, GUILD_EVENT + 2)? != GUILD_DATA {
        return refuse("Event_HuntersGuild does not open with its table's lea".into());
    }
    if word(rom, PENDING_LEA)? != LEA_ABS_A0 || long(rom, PENDING_LEA + 2)? != PENDING {
        return refuse("the list loop does not name GuildText_ListingPending".into());
    }
    if long(rom, MAP_PALETTE_POINTER)? != MAP_PALETTE {
        return refuse("HuntersGuild's map record does not name its palette".into());
    }
    let window_at = WIN_GROUP_EVENT + GUILD_WINDOW * 8;
    let [width, height, x, y] = *bytes(rom, window_at, 4)? else {
        unreachable!("four bytes")
    };
    let names = (0..GUILD_ROWS)
        .map(|row| {
            let offset = usize::from(word(rom, GUILD_NAME_OFFSETS + row * 2)?);
            job_name(rom, GUILD_NAME_OFFSETS + offset)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let rows = (0..GUILD_ROWS)
        .map(|row| Ok(bytes(rom, GUILD_DATA + row * GUILD_ROW_BYTES, GUILD_ROW_BYTES)?.to_vec()))
        .collect::<Result<Vec<_>, PackError>>()?;
    Ok(Json::object()
        .with("format_version", TABLE_FORMAT_VERSION)
        .with("kind", "guild")
        .with("rom_sha256", sha256(rom))
        .with(
            "window",
            Json::object()
                .with("id", GUILD_WINDOW)
                .with("width", u16::from(width) + 1)
                .with("height", u16::from(height) + 1)
                .with("x", x)
                .with("y", y)
                .with("rom_offset", hex6(window_at)),
        )
        .with("names", names)
        .with("pending", job_name(rom, PENDING)?)
        .with("rows", rows)
        .with("money", bytes(rom, GUILD_MONEY, GUILD_ROWS)?.to_vec())
        .with("font_line", line(rom, PAL_INIT_LINE_3)?)
        .with("done_line", line(rom, MAP_PALETTE + 0x20)?)
        .with("closed_line", line(rom, MAP_PALETTE + 0x40)?)
        .with(
            "provenance",
            Json::object()
                .with("event", hex6(GUILD_EVENT))
                .with("rows", hex6(GUILD_DATA))
                .with("money", hex6(GUILD_MONEY))
                .with("name_offsets", hex6(GUILD_NAME_OFFSETS))
                .with("pending", hex6(PENDING))
                .with("map_palette", hex6(MAP_PALETTE))
                .with("font_line", hex6(PAL_INIT_LINE_3)),
        ))
}

// ---------------------------------------------------------------------------
// The fifth member
// ---------------------------------------------------------------------------

/// `EventPtrs[$68]`'s routine, opening with `jsr (CalcPartyNumber).l`.
const FIFTH_EVENT: usize = 0x071E66;
const CALC_PARTY_NUMBER: usize = 0x05A6A4;
/// The `lea`s naming `loc_721B8` (the records) and `loc_72182` (the rows).
const RECORDS_LEA: usize = 0x071EA6;
const ROWS_LEA: usize = 0x071EEC;
const FIFTH_ROWS: usize = 5;
const FIFTH_ROW_BYTES: usize = 8;
/// `loc_720E8`: five `move.b #flag, d0` / `jsr (EventFlags_Clear).l` pairs.
const CLEARS: usize = 0x0720E8;
const EVENT_FLAGS_CLEAR: usize = 0x0576A8;
/// `Character_Stats`: record N is `$80` bytes on.
const CHARACTER_STATS: usize = 0xFFFF_F500;
const RECORD_BYTES: usize = 0x80;

fn lea_a0(rom: &[u8], at: usize, what: &str) -> Result<usize, PackError> {
    if word(rom, at)? != LEA_ABS_A0 {
        return refuse(format!("no `lea (...).l, a0` naming {what} at 0x{at:06X}"));
    }
    long(rom, at + 2)
}

/// `extract_fifth`.
fn fifth(rom: &[u8]) -> Result<Json, PackError> {
    if word(rom, FIFTH_EVENT)? != JSR_ABS || long(rom, FIFTH_EVENT + 2)? != CALC_PARTY_NUMBER {
        return refuse("Event_PickingFifthCharacter does not open with jsr CalcPartyNumber".into());
    }
    let rows_at = lea_a0(rom, ROWS_LEA, "loc_72182")?;
    let records_at = lea_a0(rom, RECORDS_LEA, "loc_721B8")?;
    let mut rows = Vec::new();
    let mut flags = Vec::new();
    for row in 0..FIFTH_ROWS {
        let at = rows_at + row * FIFTH_ROW_BYTES;
        let raw = bytes(rom, at, FIFTH_ROW_BYTES)?;
        flags.push(raw[6]);
        rows.push(
            Json::object()
                .with("x", word(rom, at)?)
                .with("y", word(rom, at + 2)?)
                .with("character", raw[4])
                .with("first_line", raw[5])
                .with("picked_flag", raw[6]),
        );
    }
    // The scene clears the rows' flags: the routine's own immediates must be
    // those five.
    let mut cleared = Vec::new();
    for n in 0..FIFTH_ROWS {
        let at = CLEARS + n * 10;
        if word(rom, at)? != MOVE_B_IMM_D0
            || word(rom, at + 4)? != JSR_ABS
            || long(rom, at + 6)? != EVENT_FLAGS_CLEAR
        {
            return refuse(format!(
                "no `move.b #flag, d0 / jsr EventFlags_Clear` at 0x{at:06X}"
            ));
        }
        cleared.push(bytes(rom, at + 3, 1)?[0]);
    }
    cleared.sort_unstable();
    flags.sort_unstable();
    if cleared != flags {
        return refuse("the routine clears flags other than its rows'".into());
    }
    let records = (0..FIFTH_ROWS)
        .map(|row| {
            let address = long(rom, records_at + row * 4)?;
            let offset = address.checked_sub(CHARACTER_STATS);
            match offset {
                Some(o) if o.is_multiple_of(RECORD_BYTES) && o / RECORD_BYTES < 16 => {
                    Ok(o / RECORD_BYTES)
                }
                _ => refuse(format!(
                    "loc_721B8 entry {row} is not a character record: 0x{address:08X}"
                )),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json::object()
        .with("format_version", TABLE_FORMAT_VERSION)
        .with("kind", "fifth")
        .with("rom_sha256", sha256(rom))
        .with("rows", rows)
        .with("records", records)
        .with(
            "provenance",
            Json::object()
                .with("event", hex6(FIFTH_EVENT))
                .with("rows", hex6(rows_at))
                .with("records", hex6(records_at))
                .with("clears", hex6(CLEARS)),
        ))
}

/// `guild/guild.json` and `fifth/fifth.json`.
pub fn build(rom: &Rom, root: &Path) -> Result<Json, PackError> {
    let data = rom.bytes();
    let guild_sha = crate::write_json(root, "guild/guild.json", &guild(data)?)?;
    let fifth_sha = crate::write_json(root, "fifth/fifth.json", &fifth(data)?)?;
    let mut part = crate::file_part("guild", "guild/guild.json", guild_sha);
    crate::json::overlay(
        &mut part,
        &crate::file_part("fifth", "fifth/fifth.json", fifth_sha),
    );
    Ok(part)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(json: &Json) -> serde_json::Value {
        serde_json::Value::from(json)
    }

    /// Ported from tests/test_guild_pack.py (#216).
    #[test]
    fn retail_guild_jobs_window_and_anchors() {
        let Some(rom) = crate::test_support::retail_rom("guild retail test") else {
            return;
        };
        let data = rom.bytes();
        let record = value(&guild(data).unwrap());
        for key in ["names", "rows", "money"] {
            assert_eq!(record[key].as_array().unwrap().len(), 8, "{key}");
        }
        assert!(
            record["names"]
                .as_array()
                .unwrap()
                .iter()
                .all(|n| !n.as_str().unwrap().is_empty())
        );
        for line in ["font_line", "done_line", "closed_line"] {
            assert_eq!(record[line].as_array().unwrap().len(), 16, "{line}");
        }
        // `InitWindowCursor` at x $108, y $98: the window's first box cell,
        // with the sprite plane's $80 origin.
        let window = &record["window"];
        assert_eq!((window["x"].as_u64().unwrap() + 1) * 8 + 0x80, 0x108);
        assert_eq!((window["y"].as_u64().unwrap() + 1) * 8 + 0x80, 0x98);
        for offset in [GUILD_EVENT + 2, PENDING_LEA + 2, MAP_PALETTE_POINTER] {
            let mut broken = data.to_vec();
            broken[offset] ^= 0xFF;
            assert!(guild(&broken).is_err(), "0x{offset:X}");
        }
    }

    /// Ported from tests/test_fifth_pack.py (#216).
    #[test]
    fn retail_fifth_rows_records_and_anchors() {
        let Some(rom) = crate::test_support::retail_rom("fifth retail test") else {
            return;
        };
        let data = rom.bytes();
        let record = value(&fifth(data).unwrap());
        let rows = record["rows"].as_array().unwrap();
        assert_eq!(rows.len(), FIFTH_ROWS);
        let mut characters: Vec<String> = rows.iter().map(|r| r["character"].to_string()).collect();
        characters.sort();
        let mut records: Vec<String> = match &record["records"] {
            serde_json::Value::Object(map) => map.keys().map(|k| format!("\"{k}\"")).collect(),
            serde_json::Value::Array(list) => list.iter().map(|r| r.to_string()).collect(),
            other => panic!("records: {other}"),
        };
        records.sort();
        assert_eq!(
            characters, records,
            "each row names a record the routine refills"
        );
        let distinct = |key: &str| {
            rows.iter()
                .map(|r| r[key].to_string())
                .collect::<std::collections::BTreeSet<_>>()
                .len()
        };
        let stands: std::collections::BTreeSet<(u64, u64)> = rows
            .iter()
            .map(|r| (r["x"].as_u64().unwrap(), r["y"].as_u64().unwrap()))
            .collect();
        assert_eq!(stands.len(), FIFTH_ROWS);
        assert!(stands.iter().all(|(x, y)| x % 16 == 0 && y % 16 == 0));
        assert_eq!(distinct("picked_flag"), FIFTH_ROWS);
        assert_eq!(distinct("first_line"), FIFTH_ROWS);
        // Each row's character, picked flag and first line, in table order,
        // pinned by digest rather than copied: `c,f,l` per row, `;` between.
        let triples: Vec<String> = rows
            .iter()
            .map(|r| {
                let n = |k: &str| r[k].as_u64().unwrap();
                format!(
                    "{},{},{}",
                    n("character"),
                    n("picked_flag"),
                    n("first_line")
                )
            })
            .collect();
        assert_eq!(
            crate::test_support::text_pin(&triples.join(";")).1,
            "e7b49c08a195e3adb63656cd4970bc609607bb0112e9f0379e7ad631508ce958"
        );
        // Compare each row in order with the local cartridge table at
        // loc_72182; no extracted table cells belong in tracked source.
        let table = &data[0x072182..0x072182 + FIFTH_ROWS * FIFTH_ROW_BYTES];
        let (raw_rows, remainder) = table.as_chunks::<FIFTH_ROW_BYTES>();
        assert!(remainder.is_empty());
        for (row, raw) in rows.iter().zip(raw_rows) {
            for (key, offset) in [("character", 4), ("first_line", 5), ("picked_flag", 6)] {
                assert_eq!(row[key].as_u64().unwrap(), u64::from(raw[offset]), "{key}");
            }
        }
        for offset in [FIFTH_EVENT + 2, ROWS_LEA, RECORDS_LEA, CLEARS, CLEARS + 3] {
            let mut broken = data.to_vec();
            broken[offset] ^= 0xFF;
            assert!(fifth(&broken).is_err(), "0x{offset:X}");
        }
        // A record that is no character is refused.
        let at = usize::from_str_radix(
            record["provenance"]["records"]
                .as_str()
                .unwrap()
                .trim_start_matches("0x"),
            16,
        )
        .unwrap();
        let mut broken = data.to_vec();
        broken[at + 3] ^= 0x01;
        assert!(fifth(&broken).is_err());
    }

    /// Ported from the two files' manifest-entry tests: each part names the
    /// file it wrote by its digest.
    #[test]
    fn retail_parts_name_the_files_they_wrote() {
        use sha2::Digest;
        let Some(rom) = crate::test_support::retail_rom("guild and fifth parts") else {
            return;
        };
        let root = crate::test_support::scratch("scene-tables");
        let part = value(&build(&rom, &root).unwrap());
        for (key, kind) in [("guild", "guild"), ("fifth", "fifth")] {
            let path = root.join(part[key]["path"].as_str().unwrap());
            let bytes = std::fs::read(&path).unwrap();
            assert_eq!(
                part[key]["sha256"].as_str().unwrap(),
                crate::png::hex(&sha2::Sha256::digest(&bytes))
            );
            let file: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(file["kind"], kind);
        }
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_job_name_ends_at_its_fe_within_32_bytes() {
        let mut rom = vec![0u8; 0x40];
        rom[0x20] = 0xFE;
        assert!(job_name(&rom, 0).is_err(), "no $FE in the first 32 bytes");
        assert_eq!(job_name(&rom, 0x20).unwrap(), "");
    }

    #[test]
    fn a_lea_must_be_a_lea() {
        let mut rom = vec![0u8; 8];
        rom[0..2].copy_from_slice(&LEA_ABS_A0.to_be_bytes());
        rom[2..6].copy_from_slice(&0x0001_2344u32.to_be_bytes());
        assert_eq!(lea_a0(&rom, 0, "x").unwrap(), 0x12344);
        rom[1] = 0xF8;
        assert!(lea_a0(&rom, 0, "x").is_err());
    }
}
