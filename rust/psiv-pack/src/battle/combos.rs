//! Combination attacks (`ComboData`, `ComboSetupData`, `ComboNames`).
//!
//! Every pointer and table length is opcode-pinned from the US retail ROM:
//! - `0x0028DE`: `lea (ComboData).l, a2` (`45 F9`, damage combo `loc_2864`)
//! - `0x006032`: `lea (ComboData).l, a2` (`45 F9`, instant-death combo `loc_5FB4`)
//! - `0x00AF78`: `lea (ComboData).l, a0` (`41 F9`, target-range check `loc_AF74`)
//! - `0x27DF78`: `lea (ComboSetupData).l, a1` (`43 F9`, matcher `loc_27DE7C`)
//! - `0x27DF7E`: `moveq #$0D, d7` (`7E 0D`, 14 combos)
//! - `0x27DE4E`: `lea (ComboNames).l, a0` (`41 F9`, `$FF`-terminated `wincharset` names)

use psiv_data::Combo;

use super::names::{element, named_byte, stat};
use crate::PackError;
use crate::text::window_charset;

const COMBO_DATA_LEA_A2_DAMAGE: usize = 0x0028DE;
const COMBO_DATA_LEA_A2_CHANCE: usize = 0x006032;
const COMBO_DATA_LEA_A0_RANGE: usize = 0x00AF78;
const COMBO_SETUP_LEA_A1: usize = 0x27DF78;
const COMBO_COUNT_MOVEQ_D7: usize = 0x27DF7E;
const COMBO_NAMES_LEA_A0: usize = 0x27DE4E;
const COMBO_RECORD_BYTES: usize = 8;

fn read_lea_long(rom: &[u8], at: usize, opcode: [u8; 2], label: &str) -> Result<usize, PackError> {
    let bytes = rom
        .get(at..at + 6)
        .ok_or_else(|| PackError(format!("{label} at 0x{at:06X} runs past end of ROM")))?;
    if bytes[0..2] != opcode {
        return Err(PackError(format!(
            "{label} at 0x{at:06X} expected opcode {:02X}{:02X}, found {:02X}{:02X}",
            opcode[0], opcode[1], bytes[0], bytes[1]
        )));
    }
    let addr = u32::from_be_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]) as usize;
    if addr >= rom.len() {
        return Err(PackError(format!(
            "{label} pointer 0x{addr:06X} runs past end of ROM"
        )));
    }
    Ok(addr)
}

fn decode_wincharset_ff(rom: &[u8], start: usize) -> Result<(String, usize), PackError> {
    let charset = window_charset();
    let mut pos = start;
    let mut out = String::new();
    while let Some(&byte) = rom.get(pos) {
        pos += 1;
        if byte == 0xFF {
            return Ok((out, pos));
        }
        let ch = charset.glyph(byte).ok_or_else(|| {
            PackError(format!(
                "ComboNames byte 0x{byte:02X} at 0x{:06X} is not in wincharset",
                pos - 1
            ))
        })?;
        out.push(ch);
    }
    Err(PackError(format!(
        "ComboNames string at 0x{start:06X} ran past end of ROM without 0xFF terminator"
    )))
}

/// Extracts all retail combination attacks directly from the ROM.
pub fn extract(rom: &[u8]) -> Result<Vec<Combo>, PackError> {
    let data_damage = read_lea_long(
        rom,
        COMBO_DATA_LEA_A2_DAMAGE,
        [0x45, 0xF9],
        "ComboData (damage)",
    )?;
    let data_chance = read_lea_long(
        rom,
        COMBO_DATA_LEA_A2_CHANCE,
        [0x45, 0xF9],
        "ComboData (chance)",
    )?;
    let data_range = read_lea_long(
        rom,
        COMBO_DATA_LEA_A0_RANGE,
        [0x41, 0xF9],
        "ComboData (range)",
    )?;
    if data_damage != data_chance || data_damage != data_range {
        return Err(PackError(format!(
            "ComboData lea pointers disagree: 0x{data_damage:06X}, 0x{data_chance:06X}, 0x{data_range:06X}"
        )));
    }
    let setup_base = read_lea_long(rom, COMBO_SETUP_LEA_A1, [0x43, 0xF9], "ComboSetupData")?;
    let names_base = read_lea_long(rom, COMBO_NAMES_LEA_A0, [0x41, 0xF9], "ComboNames")?;

    let count_op = rom
        .get(COMBO_COUNT_MOVEQ_D7..COMBO_COUNT_MOVEQ_D7 + 2)
        .ok_or_else(|| PackError("Combo count moveq runs past end of ROM".into()))?;
    if count_op[0] != 0x7E {
        return Err(PackError(format!(
            "Combo count at 0x{COMBO_COUNT_MOVEQ_D7:06X} expected moveq #imm, d7 (0x7E), found 0x{:02X}",
            count_op[0]
        )));
    }
    let count = usize::from(count_op[1]) + 1;

    // Entry 0 is `Combo_None`, eight zero bytes.
    let none = rom
        .get(data_damage..data_damage + COMBO_RECORD_BYTES)
        .ok_or_else(|| PackError("Combo_None runs past end of ROM".into()))?;
    if none != [0u8; COMBO_RECORD_BYTES] {
        return Err(PackError(format!(
            "Combo_None at 0x{data_damage:06X} is not all zeros"
        )));
    }

    let mut setup_pos = setup_base;
    let mut names_pos = names_base;
    let mut combos = Vec::with_capacity(count);
    for idx in 1..=count {
        let off = data_damage + idx * COMBO_RECORD_BYTES;
        let raw = rom
            .get(off..off + COMBO_RECORD_BYTES)
            .ok_or_else(|| PackError(format!("ComboData entry {idx} runs past end of ROM")))?;
        let header = *rom
            .get(setup_pos)
            .ok_or_else(|| PackError(format!("ComboSetupData entry {idx} runs past end of ROM")))?;
        setup_pos += 1;
        let ordered = header & 0x80 != 0;
        let participants = header & 0x7F;
        if !(2..=4).contains(&participants) {
            return Err(PackError(format!(
                "ComboSetupData entry {idx} has invalid participant count {participants} (header 0x{header:02X})"
            )));
        }
        let width = usize::from(participants);
        let mut setups = Vec::new();
        loop {
            let lead = *rom.get(setup_pos).ok_or_else(|| {
                PackError(format!(
                    "ComboSetupData entry {idx} ran past end of ROM without 0xFF terminator"
                ))
            })?;
            if lead == 0xFF {
                setup_pos += 1;
                break;
            }
            let seq = rom
                .get(setup_pos..setup_pos + width)
                .ok_or_else(|| {
                    PackError(format!(
                        "ComboSetupData entry {idx} tuple runs past end of ROM"
                    ))
                })?
                .to_vec();
            if seq.contains(&0xFF) {
                return Err(PackError(format!(
                    "ComboSetupData entry {idx} tuple contains premature 0xFF terminator"
                )));
            }
            setups.push(seq);
            setup_pos += width;
        }
        if setups.is_empty() {
            return Err(PackError(format!(
                "ComboSetupData entry {idx} has no valid ability setups"
            )));
        }
        let (display_name, next_name) = decode_wincharset_ff(rom, names_pos)?;
        names_pos = next_name;

        combos.push(Combo {
            id: idx as u8,
            display_name,
            effect_id: raw[0],
            range: raw[2],
            power_or_hit_chance: raw[3],
            resistance_stat: named_byte(raw[4], stat),
            element: named_byte(raw[5], element),
            participants,
            ordered,
            setups,
        });
    }

    Ok(combos)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_rom() -> Vec<u8> {
        let mut rom = vec![0u8; 0x286000];
        let data_addr: u32 = 0x285424;
        let setup_addr: u32 = 0x28549C;
        let names_addr: u32 = 0x28554C;

        for (at, op, addr) in [
            (COMBO_DATA_LEA_A2_DAMAGE, [0x45, 0xF9], data_addr),
            (COMBO_DATA_LEA_A2_CHANCE, [0x45, 0xF9], data_addr),
            (COMBO_DATA_LEA_A0_RANGE, [0x41, 0xF9], data_addr),
            (COMBO_SETUP_LEA_A1, [0x43, 0xF9], setup_addr),
            (COMBO_NAMES_LEA_A0, [0x41, 0xF9], names_addr),
        ] {
            rom[at..at + 2].copy_from_slice(&op);
            rom[at + 2..at + 6].copy_from_slice(&addr.to_be_bytes());
        }
        // Two combos (dbf init = 1).
        rom[COMBO_COUNT_MOVEQ_D7..COMBO_COUNT_MOVEQ_D7 + 2].copy_from_slice(&[0x7E, 0x01]);
        // ComboData: index 0 = None, index 1 = damage, index 2 = death.
        let d = data_addr as usize;
        rom[d + 8..d + 16].copy_from_slice(&[1, 0, 2, 0, 7, 3, 0, 0]);
        rom[d + 16..d + 24].copy_from_slice(&[2, 0, 2, 0x40, 2, 10, 0, 0]);
        // ComboSetupData: combo 1 unordered 2, combo 2 ordered 2.
        let s = setup_addr as usize;
        let setups = [0x02, 0x01, 0x0A, 0xFF, 0x82, 0x13, 0x96, 0xFF];
        rom[s..s + setups.len()].copy_from_slice(&setups);
        // ComboNames: "AB", "CD" in wincharset (A=1, B=2, C=3, D=4).
        let n = names_addr as usize;
        let names = [0x01, 0x02, 0xFF, 0x03, 0x04, 0xFF];
        rom[n..n + names.len()].copy_from_slice(&names);
        rom
    }

    #[test]
    fn extracts_combos_and_rejects_corrupted_opcodes_or_tables() {
        let rom = synthetic_rom();
        let combos = extract(&rom).expect("synthetic combo ROM extracts");
        assert_eq!(combos.len(), 2);
        assert_eq!(combos[0].display_name, "AB");
        assert!(!combos[0].ordered);
        assert_eq!(combos[0].setups, vec![vec![0x01, 0x0A]]);
        assert_eq!(combos[1].display_name, "CD");
        assert!(combos[1].ordered);
        assert_eq!(combos[1].effect_id, 2);
        assert_eq!(combos[1].power_or_hit_chance, 0x40);

        // Negative control 1: corrupt ComboData opcode.
        let mut bad_op = rom.clone();
        bad_op[COMBO_DATA_LEA_A2_DAMAGE] = 0x4E;
        assert!(extract(&bad_op).is_err());

        // Negative control 2: disagreeing ComboData pointers.
        let mut mismatch = rom.clone();
        mismatch[COMBO_DATA_LEA_A2_CHANCE + 5] ^= 0x08;
        assert!(extract(&mismatch).is_err());

        // Negative control 3: invalid participant count in ComboSetupData.
        let mut bad_count = rom.clone();
        bad_count[0x28549C] = 0x05;
        assert!(extract(&bad_count).is_err());

        // Negative control 4: non-zero Combo_None sentinel.
        let mut bad_none = rom.clone();
        bad_none[0x285424] = 1;
        assert!(extract(&bad_none).is_err());
    }
}
