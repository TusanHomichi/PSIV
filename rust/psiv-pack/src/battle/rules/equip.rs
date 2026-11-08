//! `UpdateEquipment`: which slot a type equips to, and the item types as
//! the rules describe them.

use std::collections::{BTreeMap, BTreeSet};

use super::{JUMP_TABLE_LIMIT, Rules, SLOTS, byte, hex6, refuse, sites};
use crate::PackError;
use crate::battle::names::ITEM_TYPES;
use crate::json::Json;
use crate::m68k::{bra_target, expect, pc_relative_table, w};

/// One `EquipItemType_*` handler.
#[derive(Debug)]
pub struct Handler {
    pub rom_offset: usize,
    pub equips_to: BTreeSet<&'static str>,
    pub clears: BTreeSet<&'static str>,
    pub conditional: BTreeSet<u16>,
}

impl Handler {
    pub fn to_json(&self) -> Json {
        Json::object()
            .with("rom_offset", hex6(self.rom_offset))
            .with(
                "equips_to",
                self.equips_to.iter().copied().collect::<Vec<_>>(),
            )
            .with("clears", self.clears.iter().copied().collect::<Vec<_>>())
            .with(
                "conditional_on_other_hand_type",
                self.conditional.iter().copied().collect::<Vec<_>>(),
            )
    }
}

/// `read_equip_rules`.
pub struct EquipRules {
    pub routine: usize,
    pub jump_table: usize,
    pub filters: Vec<usize>,
    pub type_offset: u16,
    pub usable_by: u16,
    pub max_type: u16,
    pub handlers: BTreeMap<u16, Handler>,
}

pub fn read_equip_rules(rom: &[u8]) -> Result<EquipRules, PackError> {
    let filters = sites(rom, "EquipListFilter")?;
    let type_offset = w(rom, filters[0] + 2)? & 0xFF;
    let max_type = w(rom, filters[0] + 6)?;
    let mut usable_by = None;
    for &at in &filters {
        expect(rom, at + 0x0A, 0x3431, "move.w d8(a1,d1.w), d2")?;
        let offset = w(rom, at + 0x0C)? & 0xFF;
        if usable_by.is_some_and(|seen| seen != offset) {
            return refuse("the two equip-list filters read different words".into());
        }
        usable_by = Some(offset);
    }
    let site = super::site(rom, "UpdateEquipment")?;
    if w(rom, site + 2)? & 0xFF != type_offset {
        return refuse(
            "UpdateEquipment and the equip-list filter read different type bytes".into(),
        );
    }
    let table = pc_relative_table(rom, site + 0x16, 0x4EBB, "jsr EquipItemTypeJmpTbl")?;
    let count =
        crate::m68k::self_bounded_bra_table(rom, table, "EquipItemTypeJmpTbl", JUMP_TABLE_LIMIT)?;
    if count != usize::from(max_type) {
        return refuse(format!(
            "EquipItemTypeJmpTbl has {count} entries but the equip-list filter admits types up \
             to {max_type}"
        ));
    }
    let handlers = (1..=count as u16)
        .map(|item_type| {
            let at = bra_target(rom, table + usize::from(item_type - 1) * 4)?;
            Ok((item_type, read_equip_handler(rom, at)?))
        })
        .collect::<Result<BTreeMap<_, _>, PackError>>()?;
    Ok(EquipRules {
        routine: site,
        jump_table: table,
        filters,
        type_offset,
        usable_by: usable_by.expect("two filter sites"),
        max_type,
        handlers,
    })
}

/// `_read_equip_handler`: the slots a handler writes and clears, walking both
/// arms of the shield handler's two-handed test.
fn read_equip_handler(rom: &[u8], start: usize) -> Result<Handler, PackError> {
    let mut writes = Vec::new();
    let mut clears = Vec::new();
    let mut conditional = BTreeSet::new();
    let mut pending = vec![start];
    let mut seen = BTreeSet::new();
    while let Some(mut at) = pending.pop() {
        if !seen.insert(at) {
            continue;
        }
        loop {
            let opcode = w(rom, at)?;
            match opcode {
                0x4E75 => break,
                0x1778 => {
                    writes.push(w(rom, at + 4)?);
                    at += 6;
                }
                0x177C => {
                    if w(rom, at + 2)? != 0 {
                        return refuse(format!(
                            "0x{at:06X}: an equip handler writes a non-zero immediate"
                        ));
                    }
                    clears.push(w(rom, at + 4)?);
                    at += 6;
                }
                0x11E9 | 0x41F9 => at += 6,
                0x0C42 => {
                    conditional.insert(w(rom, at + 2)?);
                    if byte(rom, at + 4)? != 0x67 {
                        return refuse(format!(
                            "0x{:06X}: an equip handler's type test is not a `beq.s`",
                            at + 4
                        ));
                    }
                    pending.push(at + 6 + usize::from(byte(rom, at + 5)?));
                    at += 6;
                }
                0x7000 | 0x7400 | 0x5340 => at += 2,
                0x102B | 0xC0FC | 0x1430 => at += 4,
                _ => {
                    return refuse(format!(
                        "0x{at:06X}: unknown opcode 0x{opcode:04X} in an equip handler"
                    ));
                }
            }
            if at as i64 - start as i64 > 0x60 {
                return refuse(format!("0x{start:06X}: equip handler does not end"));
            }
        }
    }
    if writes.is_empty() {
        return refuse(format!("0x{start:06X}: equip handler writes no slot"));
    }
    let slot = |index: u16| {
        SLOTS
            .get(usize::from(index))
            .copied()
            .ok_or_else(|| PackError(format!("0x{start:06X}: equip handler touches slot {index}")))
    };
    let checked = |list: &[u16]| list.iter().map(|&i| slot(i)).collect::<Result<Vec<_>, _>>();
    let equips_to = checked(&writes)?;
    let cleared = checked(&clears)?;
    Ok(Handler {
        rom_offset: start,
        equips_to: equips_to.into_iter().collect(),
        clears: cleared.into_iter().collect(),
        conditional,
    })
}

/// `type_rules`: one entry per item type.
pub struct TypeRule {
    pub item_type: u8,
    pub name: &'static str,
    pub equippable: bool,
    pub is_weapon: bool,
    pub multi_target: bool,
    pub slot: Option<&'static str>,
    pub two_handed: bool,
    pub element_role: Option<&'static str>,
    /// What the schema doesn't carry: the clears and the handler.
    pub research: Json,
}

pub fn type_rules(rules: &Rules) -> Result<BTreeMap<u8, TypeRule>, PackError> {
    let mut out = BTreeMap::new();
    for (item_type, name) in ITEM_TYPES.iter().enumerate().skip(1) {
        let key = item_type as u16;
        let handler = rules.equip.handlers.get(&key);
        let is_weapon = rules.attack.weapon_types.contains(&key);
        let mut rule = TypeRule {
            item_type: item_type as u8,
            name,
            equippable: handler.is_some(),
            is_weapon,
            multi_target: rules.attack.multi_target_types.contains(&key),
            slot: None,
            two_handed: false,
            element_role: None,
            research: Json::object(),
        };
        let empty: Vec<&str> = Vec::new();
        match handler {
            None => {
                rule.research = Json::object()
                    .with("clears", empty.clone())
                    .with("clears_when_other_hand_holds", Vec::<u16>::new())
                    .with("handler", Json::Null);
            }
            Some(handler) => {
                if handler.equips_to.len() != 1 {
                    return refuse(format!(
                        "item type {item_type} equips to {:?}, not one slot",
                        handler.equips_to
                    ));
                }
                let conditional: Vec<u16> = handler.conditional.iter().copied().collect();
                let clears: Vec<&str> = handler.clears.iter().copied().collect();
                rule.slot = handler.equips_to.iter().next().copied();
                rule.two_handed = !clears.is_empty() && conditional.is_empty();
                rule.research = Json::object()
                    .with(
                        "clears",
                        if conditional.is_empty() {
                            clears.clone()
                        } else {
                            empty.clone()
                        },
                    )
                    .with("clears_when_other_hand_holds", conditional.clone())
                    .with("handler", hex6(handler.rom_offset));
                if !conditional.is_empty() {
                    rule.research.insert("clears_conditionally", clears);
                }
            }
        }
        rule.element_role = if rule.is_weapon {
            Some("attack_element")
        } else if rule.equippable {
            Some("resistance_granted")
        } else {
            None
        };
        out.insert(item_type as u8, rule);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_handler_walks_both_arms_and_names_its_slots() {
        // move.b (xxx).w,0(a3); cmpi.w #3,d2; beq.s +2; rts; then the arm the
        // branch reaches: move.b #0,1(a3); rts.
        let mut rom = vec![0u8; 0x40];
        let code: [u16; 13] = [
            0x1778, 0xF000, 0x0000, 0x0C42, 0x0003, 0x6702, 0x4E75, 0x177C, 0x0000, 0x0001, 0x4E75,
            0x0000, 0x0000,
        ];
        for (i, word) in code.iter().enumerate() {
            rom[i * 2..i * 2 + 2].copy_from_slice(&word.to_be_bytes());
        }
        let handler = read_equip_handler(&rom, 0).unwrap();
        assert_eq!(
            handler.equips_to.iter().copied().collect::<Vec<_>>(),
            ["right_hand"]
        );
        assert_eq!(
            handler.clears.iter().copied().collect::<Vec<_>>(),
            ["left_hand"]
        );
        assert_eq!(handler.conditional.iter().copied().collect::<Vec<_>>(), [3]);
    }

    #[test]
    fn a_handler_past_the_slots_or_writing_nothing_refuses() {
        let mut rom = vec![0u8; 0x20];
        for (i, word) in [0x1778u16, 0xF000, 0x0004, 0x4E75].iter().enumerate() {
            rom[i * 2..i * 2 + 2].copy_from_slice(&word.to_be_bytes());
        }
        assert!(
            read_equip_handler(&rom, 0)
                .unwrap_err()
                .0
                .contains("slot 4")
        );
        let mut empty = vec![0u8; 4];
        empty[0..2].copy_from_slice(&0x4E75u16.to_be_bytes());
        assert!(
            read_equip_handler(&empty, 0)
                .unwrap_err()
                .0
                .contains("no slot")
        );
    }
}
