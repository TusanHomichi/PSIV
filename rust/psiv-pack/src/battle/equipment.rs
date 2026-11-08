//! `battle/equipment.json`: the port of `extract_items` in
//! `psiv_tools/core.py` (its battle half) and `build_equipment` in
//! `psiv_tools/battle_records.py`: every `InventoryData` record, with the
//! rules [`super::rules`] decodes giving the type, bonus and element bytes
//! their meaning.
//!
//! The file is `psiv_data::EquipmentFile`. Each record's offset, slot, owners
//! and raw bytes, the types' clears and handlers, the source, the rules and
//! the census have no typed home yet and are laid over it.

use std::collections::BTreeMap;

use psiv_data::EquipmentType;
use psiv_data::{ElementRef, Equipment, EquipmentBonuses, EquipmentFile, EquipmentKindRef};

use super::names::{CHARACTERS, Display, ITEM_TYPES, display, element};
use super::rules::{BONUS_FIELDS, ITEM_BYTES, ITEM_TABLE, Rules, TypeRule};
use super::{FORMAT_VERSION, serialize};
use crate::PackError;
use crate::json::{Json, overlay};
use crate::map_symbols::ITEM_SYMBOLS;
use crate::png::hex;

const NOTE: &str = "bytes 0x00-0x07 are the item's battle effect and are emitted in \
abilities.json as item_effects; bytes 0x14-0x15 are the shop price, which the shops section \
carries";

fn hex6(value: usize) -> String {
    format!("0x{value:06X}")
}

/// One 22-byte `InventoryData` record.
pub struct Item {
    pub id: u8,
    pub symbol: &'static str,
    pub offset: usize,
    pub raw: [u8; ITEM_BYTES],
}

impl Item {
    pub fn kind(&self) -> u8 {
        self.raw[10]
    }

    pub fn used_by_mask(&self) -> u16 {
        u16::from_be_bytes([self.raw[8], self.raw[9]])
    }

    pub fn element(&self) -> u8 {
        self.raw[18]
    }

    /// The seven signed bonus bytes, `0x0B`..`0x11`.
    pub fn bonus(&self, field: &str) -> i8 {
        let (offset, _) = BONUS_FIELDS
            .iter()
            .find(|(_, name)| *name == field)
            .expect("a bonus field");
        self.raw[usize::from(*offset)] as i8
    }

    fn bonuses(&self) -> EquipmentBonuses {
        EquipmentBonuses {
            strength: self.bonus("strength"),
            mental: self.bonus("mental"),
            agility: self.bonus("agility"),
            dexterity: self.bonus("dexterity"),
            attack: self.bonus("attack"),
            defense: self.bonus("defense"),
            magic_defense: self.bonus("magic_defense"),
        }
    }
}

/// `extract_items`: all 160 records, by id.
pub fn read_items(rom: &[u8]) -> Result<Vec<Item>, PackError> {
    ITEM_SYMBOLS
        .iter()
        .enumerate()
        .map(|(index, &symbol)| {
            let offset = ITEM_TABLE + index * ITEM_BYTES;
            let raw = rom
                .get(offset..offset + ITEM_BYTES)
                .and_then(|r| r.try_into().ok())
                .ok_or_else(|| {
                    PackError(format!("Item record {} runs past end of ROM", index + 1))
                })?;
            Ok(Item {
                id: index as u8 + 1,
                symbol,
                offset,
                raw,
            })
        })
        .collect()
}

/// `_characters`: the party members a mask names.
fn owners(mask: u16) -> Vec<Json> {
    CHARACTERS
        .iter()
        .enumerate()
        .filter(|(index, _)| mask & (1 << index) != 0)
        .map(|(index, &name)| {
            Json::object()
                .with("character_id", index)
                .with("symbol", name)
        })
        .collect()
}

fn mask_hex(mask: u16) -> String {
    format!("0x{mask:04X}")
}

/// What `_equipment_census` reads of a record.
struct Row<'a> {
    item: &'a Item,
    rule: &'a TypeRule,
    display_name: Option<String>,
}

fn census(rows: &[Row], rules: &Rules) -> Json {
    let mut counts: BTreeMap<u8, usize> = BTreeMap::new();
    for row in rows {
        *counts.entry(row.item.kind()).or_default() += 1;
    }
    let mut distribution = Json::object();
    for (kind, count) in &counts {
        distribution.insert(kind.to_string(), *count);
    }
    let count = |test: &dyn Fn(&Row) -> bool| rows.iter().filter(|r| test(r)).count();
    let mut fields: Vec<&str> = BONUS_FIELDS.iter().map(|(_, name)| *name).collect();
    fields.sort_unstable();
    let mut bonus_range = Json::object();
    for field in fields {
        let values = rows.iter().map(|r| r.item.bonus(field));
        bonus_range.insert(
            field,
            Json::object()
                .with("min", values.clone().min())
                .with("max", values.max()),
        );
    }
    let mut by_character = Json::object();
    for (index, name) in CHARACTERS.iter().enumerate() {
        by_character.insert(
            *name,
            count(&|r| r.rule.equippable && r.item.used_by_mask() & (1 << index) != 0),
        );
    }
    let elements: std::collections::BTreeSet<u8> = rows
        .iter()
        .map(|r| r.item.element())
        .filter(|&e| e != 0)
        .collect();
    Json::object()
        .with("types_present", counts.keys().copied().collect::<Vec<_>>())
        .with("type_distribution", distribution)
        .with(
            "types_declared_unused",
            (0..ITEM_TYPES.len() as u8)
                .filter(|kind| !counts.contains_key(kind))
                .collect::<Vec<_>>(),
        )
        .with("equippable", count(&|r| r.rule.equippable))
        .with("weapons", count(&|r| r.rule.is_weapon))
        .with("multi_target_weapons", count(&|r| r.rule.multi_target))
        .with("two_handed_weapons", count(&|r| r.rule.two_handed))
        .with("with_element", count(&|r| r.item.element() != 0))
        .with("elements_used", elements.into_iter().collect::<Vec<_>>())
        .with("with_post_attack_effect", count(&|r| r.item.raw[19] != 0))
        .with(
            "with_any_bonus",
            count(&|r| BONUS_FIELDS.iter().any(|(_, f)| r.item.bonus(f) != 0)),
        )
        .with("bonus_range", bonus_range)
        .with("equippable_by_character", by_character)
        .with(
            "equip_mask_on_unequippable",
            rows.iter()
                .filter(|r| !r.rule.equippable && r.item.used_by_mask() != 0)
                .map(|r| {
                    Json::object()
                        .with("id", r.item.id)
                        .with("symbol", r.item.symbol)
                        .with("display_name", r.display_name.clone())
                        .with("type", r.item.kind())
                        .with("mask", mask_hex(r.item.used_by_mask()))
                })
                .collect::<Vec<_>>(),
        )
        .with(
            "equippable_by_nobody",
            rows.iter()
                .filter(|r| r.rule.equippable && r.item.used_by_mask() == 0)
                .map(|r| r.item.id)
                .collect::<Vec<_>>(),
        )
        .with(
            "mask_bits_above_party",
            rows.iter()
                .filter(|r| r.item.used_by_mask() >> CHARACTERS.len() != 0)
                .map(|r| r.item.id)
                .collect::<Vec<_>>(),
        )
        .with("max_equippable_type", rules.equip.max_type)
}

/// The typed record of one item type.
fn equipment_type(rule: &TypeRule) -> EquipmentType {
    EquipmentType {
        kind: rule.item_type,
        name: rule.name.to_owned(),
        slot: rule.slot.map(str::to_owned),
        two_handed: rule.two_handed,
        is_weapon: rule.is_weapon,
        multi_target: rule.multi_target,
        equippable: rule.equippable,
        element_role: rule.element_role.map(str::to_owned),
    }
}

/// The whole file.
pub fn build(
    items: &[Item],
    rules: &Rules,
    types: &BTreeMap<u8, TypeRule>,
    names: &Display,
) -> Result<Json, PackError> {
    let mut rows = Vec::new();
    let mut records = Vec::new();
    let mut extra = Vec::new();
    for item in items {
        let rule = types.get(&item.kind()).ok_or_else(|| {
            PackError(format!(
                "item {} has type {}, which no rule covers",
                item.id,
                item.kind()
            ))
        })?;
        let display_name = display(names, u16::from(item.id));
        records.push(Equipment {
            id: item.id,
            symbol: item.symbol.to_owned(),
            display_name: display_name.clone(),
            meseta_cost: Some(u16::from_be_bytes([item.raw[20], item.raw[21]])),
            kind: EquipmentKindRef {
                id: item.kind(),
                name: rule.name.to_owned(),
            },
            bonuses: item.bonuses(),
            element: ElementRef {
                id: item.element(),
                name: element(item.element()).map(str::to_owned),
                role: rule.element_role.map(str::to_owned),
            },
            is_weapon: rule.is_weapon,
            multi_target: rule.multi_target,
            two_handed: rule.two_handed,
            post_attack_effect_id: Some(item.raw[19]),
            equippable_by_mask: mask_hex(item.used_by_mask()),
        });
        extra.push(
            Json::object()
                .with("rom_offset", hex6(item.offset))
                .with("equippable", rule.equippable)
                .with("slot", rule.slot)
                .with("equippable_by", owners(item.used_by_mask()))
                .with("raw_hex", hex(&item.raw)),
        );
        rows.push(Row {
            item,
            rule,
            display_name,
        });
    }
    let census = census(&rows, rules);
    let file = EquipmentFile {
        format_version: FORMAT_VERSION,
        count: records.len() as u32,
        types: types.values().map(equipment_type).collect(),
        items: records,
    };
    let mut out = serialize(&file, "the equipment")?;
    let fields = Json::object()
        .with("usable_by", format!("0x{:02X}", rules.equip.usable_by))
        .with("type", format!("0x{:02X}", rules.equip.type_offset))
        .with("bonuses", "0x0B-0x11")
        .with(
            "element",
            format!("0x{:02X}", rules.elements.element_offset),
        )
        .with("post_attack_effect", "0x13");
    let over = Json::object()
        .with("kind", "battle_equipment")
        .with(
            "source",
            Json::object()
                .with("label", "InventoryData")
                .with("rom_offset", hex6(ITEM_TABLE))
                .with("record_bytes", ITEM_BYTES)
                .with("fields", fields)
                .with("note", NOTE),
        )
        .with("rules", rules.to_json())
        .with(
            "types",
            types
                .values()
                .map(|r| r.research.clone())
                .collect::<Vec<_>>(),
        )
        .with("items", extra)
        .with("census", census);
    overlay(&mut out, &over);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(raw: [u8; ITEM_BYTES]) -> Item {
        Item {
            id: 1,
            symbol: "Dagger",
            offset: ITEM_TABLE,
            raw,
        }
    }

    #[test]
    fn a_record_reads_its_mask_type_signed_bonuses_and_element() {
        let mut raw = [0u8; ITEM_BYTES];
        raw[8..10].copy_from_slice(&0x0405u16.to_be_bytes());
        raw[10] = 6;
        raw[11] = 0xFE;
        raw[17] = 9;
        raw[18] = 3;
        let item = item(raw);
        assert_eq!(item.used_by_mask(), 0x0405);
        assert_eq!(item.kind(), 6);
        let bonuses = item.bonuses();
        assert_eq!((bonuses.strength, bonuses.magic_defense), (-2, 9));
        assert_eq!(item.element(), 3);
    }

    #[test]
    fn owners_are_the_mask_bits_within_the_party() {
        let named = owners(0b1000_0000_0101);
        let ids: Vec<_> = named
            .iter()
            .map(|o| serde_json::Value::from(o)["character_id"].as_u64().unwrap())
            .collect();
        assert_eq!(ids, [0, 2]);
        assert_eq!(mask_hex(0x0800), "0x0800");
    }
}
