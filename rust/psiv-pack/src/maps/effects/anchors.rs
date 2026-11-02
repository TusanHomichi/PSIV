//! Where the mechanism's routines, flag banks and buffers are in the image:
//! `psiv_tools/map_effects/anchors.py`, which owns the citations. Each is
//! derived from the retail image, and a disagreement with the shape the
//! mechanism assumes fails.

use std::collections::HashMap;

use crate::PackError;

/// `records.py`'s bound on a `MapDataManager` index.
pub const MAP_DATA_MANAGER_ROUTINES: usize = 0xA0;
const FLAG_TEST_BLOCK: usize = 0x057624;
const FLAG_CLEAR_BLOCK: usize = 0x0576A8;
const FLAG_TEST_STRIDE: usize = 0x0A;
/// The clear block is one door short: nothing clears a town flag.
const FLAG_CLEAR_DOORS: usize = 3;
const FLAG_BANKS: [(u16, &str); 4] = [
    (0xF100, "event_flags"),
    (0xF120, "extended_event_flags"),
    (0xF140, "chest_flags"),
    (0xF160, "town_flags"),
];
pub const GET_MAP_LAYOUT_OFFSET: i64 = 0x053514;
pub const FIELD_OBJ_SECONDARY: i64 = 0xC300;
pub const FIELD_OBJ_STRIDE: i64 = 0x40;
pub const FIELD_OBJ_LAST: i64 = 0xCFC0;
pub const OBJECT_DIALOGUE_OFFSET: i64 = 0x14;
/// `curr_x_pos` within an object.
pub const OBJECT_X_OFFSET: i64 = 0x30;
/// `curr_y_pos` within an object.
pub const OBJECT_Y_OFFSET: i64 = 0x34;
/// The two layout buffers, exactly a buffer apart.
pub const MAP_LAYOUT_FG: i64 = 0xA000;
pub const MAP_LAYOUT_BG: i64 = 0xB000;
pub const MAP_LAYOUT_BYTES: i64 = 0x1000;

/// A flag bank's address, by name.
pub fn bank_address(bank: &str) -> u16 {
    FLAG_BANKS
        .iter()
        .find(|(_, name)| *name == bank)
        .map_or(0, |(address, _)| *address)
}

fn word(rom: &[u8], at: usize) -> Option<u16> {
    let bytes = rom.get(at..at + 2)?;
    Some(u16::from_be_bytes([bytes[0], bytes[1]]))
}

/// Non-overlapping left-to-right matches of `pattern`, where `None` is any
/// byte, as `re.finditer` finds them.
fn matches(rom: &[u8], pattern: &[Option<&[u8]>]) -> Vec<usize> {
    let mut hits = Vec::new();
    let mut at = 0;
    while at + pattern.len() <= rom.len() {
        let hit = pattern
            .iter()
            .zip(&rom[at..])
            .all(|(want, byte)| want.is_none_or(|bytes| bytes.contains(byte)));
        if hit {
            hits.push(at);
            at += pattern.len();
        } else {
            at += 1;
        }
    }
    hits
}

fn long_after(rom: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([rom[at], rom[at + 1], rom[at + 2], rom[at + 3]])
}

/// Exact bytes then a run of wildcards, as a [`matches`] pattern.
fn exact(bytes: &'static [u8]) -> impl Iterator<Item = Option<&'static [u8]>> {
    bytes.iter().map(std::slice::from_ref).map(Some)
}

/// `dispatch_table`: `MapDataManagerJmpTbl`, from the dispatcher that names it.
pub fn dispatch_table(rom: &[u8]) -> Result<usize, PackError> {
    let pattern: Vec<Option<&[u8]>> = exact(&[0x30, 0x10, 0x0C, 0x40, 0xFF, 0xFF, 0x67])
        .chain([None])
        .chain(exact(&[0x43, 0xF9]))
        .chain([None; 4])
        .chain(exact(&[
            0xD0, 0x40, 0xD0, 0x40, 0x2F, 0x08, 0x4E, 0xB1, 0x00, 0x00,
        ]))
        .collect();
    let hits = matches(rom, &pattern);
    let [hit] = hits.as_slice() else {
        return Err(PackError(format!(
            "the MapDataManager dispatcher matches {} times, not once",
            hits.len()
        )));
    };
    Ok(long_after(rom, hit + 10) as usize)
}

/// `routine_address`: one `bra.w` entry, resolved.
pub fn routine_address(rom: &[u8], table: usize, index: usize) -> Result<usize, PackError> {
    if index >= MAP_DATA_MANAGER_ROUTINES {
        return Err(PackError(format!(
            "entry {index} is outside the jump table"
        )));
    }
    let entry = table + index * 4;
    if word(rom, entry) != Some(0x6000) {
        return Err(PackError(format!(
            "jump table entry {index} is not a `bra.w`"
        )));
    }
    let displacement = word(rom, entry + 2).unwrap_or(0) as i16;
    Ok((entry as i64 + 2 + i64::from(displacement)) as usize)
}

/// One flag block: `movem.l` then the `lea` that picks the bank, per door.
fn flag_block(
    rom: &[u8],
    base: usize,
    what: &str,
    expected: usize,
) -> Result<HashMap<i64, &'static str>, PackError> {
    let mut out = HashMap::new();
    for i in 0..FLAG_BANKS.len() {
        let address = base + i * FLAG_TEST_STRIDE;
        if rom.get(address..address + 4) != Some(&[0x48, 0xE7, 0xE0, 0x80])
            || rom.get(address + 4..address + 6) != Some(&[0x41, 0xF8])
        {
            break;
        }
        let bank = word(rom, address + 6);
        let Some((_, name)) = FLAG_BANKS.iter().find(|(at, _)| Some(*at) == bank) else {
            break;
        };
        out.insert(address as i64, *name);
    }
    if out.len() != expected {
        return Err(PackError(format!(
            "the flag {what} block at 0x{base:06X} has {} doors, not {expected}",
            out.len()
        )));
    }
    Ok(out)
}

/// `flag_tests`: the four flag-test entry points, with the bank each reads.
pub fn flag_tests(rom: &[u8]) -> Result<HashMap<i64, &'static str>, PackError> {
    flag_block(rom, FLAG_TEST_BLOCK, "test", FLAG_BANKS.len())
}

/// `flag_clears`: the flag-clearing entry points.
pub fn flag_clears(rom: &[u8]) -> Result<HashMap<i64, &'static str>, PackError> {
    flag_block(rom, FLAG_CLEAR_BLOCK, "clear", FLAG_CLEAR_DOORS)
}

/// `_kos_decomp`: `KosDecomp`, by the call the layout-replace routines share.
/// One other site loads $A000 for its own reasons; the routines are the
/// several that agree.
pub fn kos_decomp(rom: &[u8]) -> Result<i64, PackError> {
    let buffers: &[u8] = &[0xA0, 0xB0];
    let pattern: Vec<Option<&[u8]>> = exact(&[0x41, 0xF9])
        .chain([None; 4])
        .chain(exact(&[0x43, 0xF8]))
        .chain([Some(buffers)])
        .chain(exact(&[0x00, 0x4E, 0xB9]))
        .chain([None; 4])
        .collect();
    let mut counts: HashMap<u32, usize> = HashMap::new();
    for hit in matches(rom, &pattern) {
        *counts.entry(long_after(rom, hit + 12)).or_default() += 1;
    }
    let agreed: Vec<u32> = counts
        .into_iter()
        .filter(|&(_, count)| count > 1)
        .map(|(target, _)| target)
        .collect();
    let [target] = agreed.as_slice() else {
        return Err(PackError(format!(
            "{} routines are called by more than one layout-replace site; KosDecomp cannot be \
             identified",
            agreed.len()
        )));
    };
    Ok(i64::from(*target))
}

/// `map_layout_bases`: read both buffers back out of `GetMapLayoutOffset`,
/// because the distance between them is what makes a cross-plane displacement
/// resolvable at all.
pub fn check_map_layout_bases(rom: &[u8]) -> Result<(), PackError> {
    let start = GET_MAP_LAYOUT_OFFSET as usize;
    let (mut fg, mut bg) = (false, false);
    for probe in (start..start + 0x20).step_by(2) {
        if word(rom, probe) != Some(0x43F8) {
            continue;
        }
        let address = i64::from(word(rom, probe + 2).unwrap_or(0));
        match address {
            MAP_LAYOUT_FG => fg = true,
            MAP_LAYOUT_BG => bg = true,
            _ => {
                return Err(PackError(format!(
                    "0x{probe:06X}: GetMapLayoutOffset loads an unknown layout buffer \
                     0xFFFF{address:04X}"
                )));
            }
        }
    }
    if !(fg && bg) {
        return Err(PackError(
            "GetMapLayoutOffset does not name both layout buffers".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scan_resumes_after_a_match_and_honours_wildcards() {
        let a: &[u8] = &[1];
        let pattern = [Some(a), None, Some(a)];
        assert_eq!(matches(&[1, 9, 1, 9, 1, 1, 0, 1], &pattern), [0, 5]);
    }

    #[test]
    fn a_rom_without_the_anchors_is_refused() {
        let rom = vec![0u8; 0x60000];
        assert!(dispatch_table(&rom).unwrap_err().0.contains("0 times"));
        assert!(
            flag_tests(&rom)
                .unwrap_err()
                .0
                .contains("has 0 doors, not 4")
        );
        assert!(
            flag_clears(&rom)
                .unwrap_err()
                .0
                .contains("has 0 doors, not 3")
        );
        assert!(kos_decomp(&rom).unwrap_err().0.starts_with("0 routines"));
        assert!(check_map_layout_bases(&rom).is_err());
    }

    #[test]
    fn an_entry_past_the_table_or_not_a_branch_is_refused() {
        let rom = [0x60, 0x00, 0x00, 0x10, 0x4E, 0x75];
        assert_eq!(routine_address(&rom, 0, 0).unwrap(), 0x12);
        assert!(
            routine_address(&rom, 0, 1)
                .unwrap_err()
                .0
                .contains("not a `bra.w`")
        );
        let error = routine_address(&rom, 0, MAP_DATA_MANAGER_ROUTINES).unwrap_err();
        assert!(error.0.contains("outside the jump table"));
    }
}
