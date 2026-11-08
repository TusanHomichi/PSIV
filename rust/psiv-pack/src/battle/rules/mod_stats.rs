//! `UpdateCharModStats`: the seven derived-stat passes.

use super::{
    BONUS_FIELDS, ITEM_BYTES, ITEM_TABLE, JUMP_TABLE_LIMIT, SLOTS, byte, hex2, hex6, ram_field,
    refuse, site_of_mod_stats,
};
use crate::PackError;
use crate::json::Json;
use crate::m68k::{bra_target, expect, l, pc_relative_table, w};

/// One `ModStatsUpdate_*` handler.
pub struct ModPass {
    pub stat: String,
    pub base_stat: String,
    pub item_offsets: Vec<u16>,
    pub bonuses: Vec<&'static str>,
    pub result_bytes: u8,
    pub rom_offset: usize,
}

impl ModPass {
    pub fn to_json(&self) -> Json {
        Json::object()
            .with("stat", self.stat.as_str())
            .with("base_stat", self.base_stat.as_str())
            .with(
                "item_offsets",
                self.item_offsets
                    .iter()
                    .map(|&o| hex2(o))
                    .collect::<Vec<_>>(),
            )
            .with("bonuses", self.bonuses.clone())
            .with("result_bytes", self.result_bytes)
            .with(
                "arithmetic",
                if self.result_bytes == 1 {
                    "add.b"
                } else {
                    "ext.w + add.w"
                },
            )
            .with("signed", self.result_bytes == 2)
            .with("rom_offset", hex6(self.rom_offset))
    }
}

/// `read_mod_stats`.
pub struct ModStats {
    pub routine: usize,
    pub jump_table: usize,
    pub inventory: usize,
    pub stride: u16,
    pub slots: usize,
    pub byte_adder: usize,
    pub word_adder: usize,
    pub passes: Vec<ModPass>,
}

fn bonus_field(offset: u16) -> Result<&'static str, PackError> {
    BONUS_FIELDS
        .iter()
        .find(|(o, _)| *o == offset)
        .map(|(_, name)| *name)
        .ok_or_else(|| PackError(format!("no bonus field at item offset 0x{offset:02X}")))
}

pub fn read_mod_stats(rom: &[u8]) -> Result<ModStats, PackError> {
    let site = site_of_mod_stats(rom)?;
    let passes_expected = usize::from(byte(rom, site + 1)?) + 1;
    let table = pc_relative_table(rom, site + 0x16, 0x4EFB, "jmp UpdateCharModStatsJmpTbl")?;
    let count = crate::m68k::self_bounded_bra_table(
        rom,
        table,
        "UpdateCharModStatsJmpTbl",
        JUMP_TABLE_LIMIT,
    )?;
    if count != passes_expected {
        return refuse(format!(
            "UpdateCharModStats loops {passes_expected} times over a {count}-entry jump table"
        ));
    }
    let byte_adder = super::site(rom, "AddItemBonusToCharStats")?;
    let word_adder = super::site(rom, "AddItemBonusToCharStats2")?;
    let first = read_adder(rom, byte_adder, "AddItemBonusToCharStats")?;
    let second = read_adder(rom, word_adder, "AddItemBonusToCharStats2")?;
    if first != second {
        return refuse(format!(
            "the two bonus adders disagree about InventoryData: 0x{:06X}/{}/{} against \
             0x{:06X}/{}/{}",
            first.0, first.1, first.2, second.0, second.1, second.2
        ));
    }
    let (inventory, stride, slots) = first;
    let passes = (0..count)
        .map(|index| {
            let at = bra_target(rom, table + index * 4)?;
            read_mod_pass(rom, at, byte_adder, word_adder)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if (inventory, usize::from(stride)) != (ITEM_TABLE, ITEM_BYTES) {
        return refuse(format!(
            "the bonus adders read {stride}-byte records from 0x{inventory:06X}; \
             psiv_tools.core has {ITEM_BYTES} from 0x{ITEM_TABLE:06X}"
        ));
    }
    if slots != SLOTS.len() {
        return refuse(format!(
            "the bonus adders walk {slots} equipment slots, not {}",
            SLOTS.len()
        ));
    }
    Ok(ModStats {
        routine: site,
        jump_table: table,
        inventory,
        stride,
        slots,
        byte_adder,
        word_adder,
        passes,
    })
}

/// `_read_adder`: `InventoryData`, the record stride and the slot count.
fn read_adder(rom: &[u8], at: usize, label: &str) -> Result<(usize, u16, usize), PackError> {
    let slots = usize::from(byte(rom, at + 3)?) + 1;
    expect(
        rom,
        at + 4,
        0x45F9,
        &format!("{label}: lea (InventoryData).l, a2"),
    )?;
    let inventory = l(rom, at + 6)? as usize;
    for probe in (at..at + 0x20).step_by(2) {
        if w(rom, probe)? & 0xF1FF == 0xC0FC {
            return Ok((inventory, w(rom, probe + 2)?, slots));
        }
    }
    refuse(format!("{label}: no `mulu.w #stride` found"))
}

/// `_read_mod_pass`.
fn read_mod_pass(
    rom: &[u8],
    start: usize,
    byte_adder: usize,
    word_adder: usize,
) -> Result<ModPass, PackError> {
    let mut at = start;
    expect(rom, at, 0x7000, "moveq #0, d0")?;
    expect(rom, at + 2, 0x1028, "move.b d16(a0), d0")?;
    let base = w(rom, at + 4)?;
    expect(rom, at + 6, 0x43E8, "lea d16(a0), a1")?;
    let destination = w(rom, at + 8)?;
    expect(rom, at + 10, 0x323C, "move.w #offset, d1")?;
    let mut offsets = vec![w(rom, at + 12)?];
    at += 14;
    if w(rom, at)? == 0x343C {
        offsets.push(w(rom, at + 2)?);
        at += 4;
    }
    let target = bra_target(rom, at)?;
    let (arithmetic, width) = if target == byte_adder {
        ("add.b", 1u8)
    } else if target == word_adder {
        ("ext.w + add.w", 2)
    } else {
        return refuse(format!(
            "0x{at:06X}: a derived-stat pass tail-calls 0x{target:06X}, neither adder"
        ));
    };
    if offsets.len() != usize::from(width) {
        return refuse(format!(
            "0x{at:06X}: {} item offsets do not match a {arithmetic} pass",
            offsets.len()
        ));
    }
    let (Some(stat), Some(base_stat)) = (ram_field(destination), ram_field(base)) else {
        return refuse(format!(
            "0x{at:06X}: a derived-stat pass names an unknown stat slot"
        ));
    };
    Ok(ModPass {
        stat,
        base_stat,
        bonuses: offsets
            .iter()
            .map(|&o| bonus_field(o))
            .collect::<Result<_, _>>()?,
        item_offsets: offsets,
        result_bytes: width,
        rom_offset: start,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_two_word_pass_tail_calls_the_word_adder() {
        let mut rom = vec![0u8; 0x40];
        let code: [u16; 11] = [
            0x7000, 0x1028, 0x0018, 0x43E8, 0x0024, 0x323C, 0x000F, 0x343C, 0x000B, 0x6000, 0x0010,
        ];
        for (i, word) in code.iter().enumerate() {
            rom[i * 2..i * 2 + 2].copy_from_slice(&word.to_be_bytes());
        }
        // The bra.w at 0x12 lands on 0x14 + 0x10.
        let pass = read_mod_pass(&rom, 0, 0x30, 0x24).unwrap();
        assert_eq!(
            (pass.stat.as_str(), pass.base_stat.as_str()),
            ("atk_pow", "strength")
        );
        assert_eq!(pass.bonuses, ["attack", "strength"]);
        assert_eq!(pass.result_bytes, 2);
        let error = read_mod_pass(&rom, 0, 0x24, 0x30).err().unwrap();
        assert!(error.0.contains("do not match"), "{}", error.0);
    }
}
