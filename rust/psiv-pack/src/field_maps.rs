//! Field maps: what the ported sections read of `FieldMapPtrs` so far.
//!
//! `psiv_tools/maps/records.py` walks the whole record grammar; slice 3 of
//! #216 ports that walk here. Until then, the sound section needs only each
//! real map's music id, the record's second byte (`GameMode_LoadFieldMap`),
//! and this module reads just that, behind the table's own checks.

use crate::PackError;

/// 417 longs of absolute 68000 addresses; the ROM maps at 0, so they are
/// file offsets. The table ends where its first entry points.
const FIELD_MAP_PTRS: usize = 0x100000;
const MAP_COUNT: usize = 417;
/// Every `PtrMap_Null*` entry points at `ErrorTrap`.
const ERROR_TRAP: usize = 0x000200;
/// The table's first 16 longs: entries 3 to $F are the first run of null
/// maps, so this pins both the address and the null convention.
const SIGNATURE: &str = "001006840010fea20011966400000200000002000000020000000200000002000000\
                         020000000200000002000000020000000200000002000000020000000200";

fn be32(data: &[u8], offset: usize) -> usize {
    u32::from_be_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]) as usize
}

fn hex6(value: usize) -> String {
    format!("0x{value:06X}")
}

/// Check `FieldMapPtrs` as `_check_pointer_table` does: its signature at
/// its offset, found nowhere else, and flush against the first record.
fn check_pointer_table(data: &[u8]) -> Result<(), PackError> {
    let signature: Vec<u8> = (0..SIGNATURE.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&SIGNATURE[i..i + 2], 16).expect("hex signature"))
        .collect();
    let start = FIELD_MAP_PTRS;
    let end = start + MAP_COUNT * 4;
    let actual = data.get(start..start + signature.len());
    if actual != Some(signature.as_slice()) {
        return Err(PackError(format!(
            "FieldMapPtrs signature mismatch at {}: expected {SIGNATURE}, got {}",
            hex6(start),
            crate::png::hex(actual.unwrap_or_default())
        )));
    }
    let occurrences = data
        .windows(signature.len())
        .filter(|w| *w == signature)
        .count();
    if occurrences != 1 {
        return Err(PackError(format!(
            "FieldMapPtrs signature occurs {occurrences} times in this ROM; it must be unique \
             for the offset to be self-verifying"
        )));
    }
    let first = be32(data, start);
    if first != end {
        return Err(PackError(format!(
            "FieldMapPtrs is {MAP_COUNT} longs ending at {}, but its first entry (Map_Motavia) \
             is {}; the table is supposed to be flush against the first map record",
            hex6(end),
            hex6(first)
        )));
    }
    Ok(())
}

/// Each real map's music id, in map order (0 keeps the current track).
pub fn music_ids(data: &[u8]) -> Result<Vec<u8>, PackError> {
    check_pointer_table(data)?;
    let mut ids = Vec::new();
    for map_id in 0..MAP_COUNT {
        let pointer = be32(data, FIELD_MAP_PTRS + map_id * 4);
        if pointer == ERROR_TRAP {
            continue;
        }
        let music = data.get(pointer + 1).ok_or_else(|| {
            PackError(format!(
                "map 0x{map_id:03X} points past the ROM at {}",
                hex6(pointer)
            ))
        })?;
        ids.push(*music);
    }
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rom_without_the_table_is_refused() {
        let error = music_ids(&vec![0; 0x200000]).unwrap_err();
        assert!(
            error
                .0
                .starts_with("FieldMapPtrs signature mismatch at 0x100000"),
            "{}",
            error.0
        );
        let error = music_ids(&[0; 16]).unwrap_err();
        assert!(error.0.ends_with(", got "), "{}", error.0);
    }
}
