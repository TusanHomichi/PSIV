//! The port of `psiv_tools/battle_rules.py`: what five retail routines say an
//! equipment record means, decoded from their instructions rather than
//! transcribed. That module's docs own the table of routines and the reasons;
//! each signature here must occur exactly as often as retail carries it, and
//! every count and address the routines state is read back out of them and
//! held against the record tables.

mod char_init;
mod equip;
mod mod_stats;

use std::collections::BTreeSet;

pub use char_init::{CharInit, read_char_init};
pub use equip::{EquipRules, TypeRule, type_rules};
pub use mod_stats::ModStats;

use super::names::PROPERTIES;
use crate::PackError;
use crate::json::Json;
use crate::m68k::{expect, find_exactly, l, w};

/// `EQUIPMENT_SLOTS`, in record order.
pub const SLOTS: [&str; 4] = ["right_hand", "left_hand", "head", "body"];

/// `BONUS_FIELDS`: the bonus bytes, by record offset.
pub const BONUS_FIELDS: [(u16, &str); 7] = [
    (0x0B, "strength"),
    (0x0C, "mental"),
    (0x0D, "agility"),
    (0x0E, "dexterity"),
    (0x0F, "attack"),
    (0x10, "defense"),
    (0x11, "magic_defense"),
];

/// `TABLES["characters"]` and `TABLES["items"]`, which the routines must agree
/// with.
pub const CHARACTER_TABLE: usize = 0x2A8ACA;
pub const CHARACTER_COUNT: usize = 11;
pub const CHARACTER_BYTES: usize = 66;
pub const ITEM_TABLE: usize = 0x2A8E28;
pub const ITEM_BYTES: usize = 22;

/// `SIGNATURES`: each label's bytes and how often retail carries them.
const SIGNATURES: [(&str, &str, usize); 13] = [
    ("InitializeCharStats", "41f8f50043f9", 1),
    ("UpdateCharModStats", "7e0670002f0061", 1),
    ("AddItemBonusToCharStats", "2f0b760345f9", 1),
    ("AddItemBonusToCharStats2", "2f0b780345f9", 1),
    ("UpdateEquipment", "1028000a5340d040d040", 1),
    ("EquipListFilter", "1431100a0c020007", 2),
    ("Battle_AttackCommand", "1029004c67164eb9", 1),
    ("Battle_GetItemType", "4a00670661061028000a4e75", 1),
    ("UpdateCharElems", "7000323c000d343c0030", 1),
    ("CharElemsShieldTest", "0c2a0005000a", 2),
    ("CharElemsRightWeapon", "116a00120050", 1),
    ("CharElemsLeftWeapon", "116a00120051", 1),
    ("CharElemsResistance", "11bc00010030", 1),
];

const JUMP_TABLE_LIMIT: usize = 64;

const ATTACK_NOTE: &str = "the right hand is tried first and the left only if the right holds \
nothing that can swing; a character holding only shields has no Attack command at all";
const ELEMENT_NOTE: &str = "a hand holding anything but a shield sets that hand's attack \
element; a shield or either armour slot instead sets the named element's property to the \
resistance value. Element slots no item touched fall back to the character record's own byte.";

fn refuse<T>(message: String) -> Result<T, PackError> {
    Err(PackError(message))
}

fn hex2(value: impl Into<u64>) -> String {
    format!("0x{:02X}", value.into())
}

fn hex6(value: usize) -> String {
    format!("0x{value:06X}")
}

fn byte(rom: &[u8], at: usize) -> Result<u8, PackError> {
    rom.get(at)
        .copied()
        .ok_or_else(|| PackError(format!("0x{at:06X}: a read runs past the end of the image")))
}

/// `RAM_FIELDS`: the `Character_Stats` slot names, from `ps4.constants.asm`.
pub fn ram_field(offset: u16) -> Option<String> {
    let named = match offset {
        0x06 => "profession",
        0x08 => "level",
        0x0A => "experience",
        0x0E => "curr_hp",
        0x10 => "max_hp",
        0x12 => "curr_tp",
        0x14 => "max_tp",
        0x18 => "strength",
        0x1B => "mental",
        0x1E => "agility",
        0x21 => "dexterity",
        0x4C => "right_hand",
        0x4D => "left_hand",
        0x4E => "head",
        0x4F => "body",
        0x52 => "techniques",
        0x62 => "skills",
        0x6A => "curr_skill_uses",
        0x6B => "max_skill_uses",
        0x19 => "strength_mod",
        0x1C => "mental_mod",
        0x1F => "agility_mod",
        0x22 => "dexterity_mod",
        0x24 => "atk_pow",
        0x28 => "dfs_pow",
        0x2C => "magic_dfs",
        0x50 => "right_hand_element",
        0x51 => "left_hand_element",
        0x30..=0x4B => {
            let index = usize::from(offset - 0x30);
            let name = PROPERTIES[index / 2];
            return Some(if index % 2 == 0 {
                format!("{name}_prop")
            } else {
                format!("{name}_prop+1")
            });
        }
        _ => return None,
    };
    Some(named.to_owned())
}

fn named_ram(offset: u16) -> Result<String, PackError> {
    ram_field(offset)
        .ok_or_else(|| PackError(format!("no name for Character_Stats offset 0x{offset:02X}")))
}

/// `_sites`: every offset of `label`'s signature, at its retail count.
fn sites(rom: &[u8], label: &str) -> Result<Vec<usize>, PackError> {
    let (_, signature, expected) = SIGNATURES
        .iter()
        .find(|(name, _, _)| *name == label)
        .expect("a known signature label");
    Ok(find_exactly(rom, signature, *expected, label)?)
}

fn site(rom: &[u8], label: &str) -> Result<usize, PackError> {
    Ok(sites(rom, label)?[0])
}

fn site_of_mod_stats(rom: &[u8]) -> Result<usize, PackError> {
    site(rom, "UpdateCharModStats")
}

// ---------------------------------------------------------------------------
// Battle_AttackCommand: which types are weapons
// ---------------------------------------------------------------------------

/// `read_attack_rules`.
pub struct AttackRules {
    pub routine: usize,
    pub get_type: usize,
    pub weapon_types: BTreeSet<u16>,
    pub multi_target_types: BTreeSet<u16>,
}

pub fn read_attack_rules(rom: &[u8]) -> Result<AttackRules, PackError> {
    let site = site(rom, "Battle_AttackCommand")?;
    expect(rom, site + 6, 0x4EB9, "jsr Battle_GetItemType")?;
    let get_type = l(rom, site + 8)? as usize;
    if get_type != self::site(rom, "Battle_GetItemType")? {
        return refuse(format!(
            "Battle_AttackCommand calls 0x{get_type:06X}, not Battle_GetItemType"
        ));
    }
    expect(rom, site + 0x0E, 0x0C00, "cmpi.b #type, d0")?;
    expect(rom, site + 0x14, 0x0C00, "cmpi.b #type, d0")?;
    let multi: BTreeSet<u16> = [w(rom, site + 0x10)?, w(rom, site + 0x16)?].into();
    let bound = w(rom, site + 0x16)?;
    if byte(rom, site + 0x1A)? != 0x6D {
        return refuse(format!(
            "0x{:06X}: Battle_AttackCommand does not separate weapons with `blt.s`",
            site + 22
        ));
    }
    let weapons: BTreeSet<u16> = (1..=bound).collect();
    if !multi.is_subset(&weapons) {
        return refuse("a multi-target type is not a weapon type".into());
    }
    Ok(AttackRules {
        routine: site,
        get_type,
        weapon_types: weapons,
        multi_target_types: multi,
    })
}

// ---------------------------------------------------------------------------
// UpdateCharElems: what byte $12 means
// ---------------------------------------------------------------------------

/// `read_element_rules`.
pub struct ElementRules {
    pub routine: usize,
    pub element_offset: u16,
    pub type_offset: u16,
    pub shield_type: u16,
    pub elements: u16,
    pub props: u16,
    pub resistance_value: u16,
    pub right: u16,
    pub left: u16,
}

pub fn read_element_rules(rom: &[u8]) -> Result<ElementRules, PackError> {
    let site = site(rom, "UpdateCharElems")?;
    let elements = w(rom, site + 4)? + 1;
    let props = w(rom, site + 8)?;
    let shield = sites(rom, "CharElemsShieldTest")?[0];
    let shield_type = w(rom, shield + 2)?;
    let type_offset = w(rom, shield + 4)?;
    let right = self::site(rom, "CharElemsRightWeapon")?;
    let left = self::site(rom, "CharElemsLeftWeapon")?;
    let element_offset = w(rom, right + 2)?;
    if w(rom, left + 2)? != element_offset {
        return refuse("the two weapon-element writes read different bytes".into());
    }
    let resistance = self::site(rom, "CharElemsResistance")?;
    let value = w(rom, resistance + 2)?;
    if w(rom, resistance + 4)? != props {
        return refuse(
            "the resistance write and the clearing loop use different property bases".into(),
        );
    }
    if usize::from(elements) != PROPERTIES.len() {
        return refuse(format!(
            "UpdateCharElems clears {elements} element properties, not {}",
            PROPERTIES.len()
        ));
    }
    Ok(ElementRules {
        routine: site,
        element_offset,
        type_offset,
        shield_type,
        elements,
        props,
        resistance_value: value,
        right: w(rom, right + 4)?,
        left: w(rom, left + 4)?,
    })
}

/// `read_rules`: every rule the two files depend on.
pub struct Rules {
    pub equip: EquipRules,
    pub attack: AttackRules,
    pub derived: ModStats,
    pub elements: ElementRules,
}

pub fn read_rules(rom: &[u8]) -> Result<Rules, PackError> {
    Ok(Rules {
        equip: equip::read_equip_rules(rom)?,
        attack: read_attack_rules(rom)?,
        derived: mod_stats::read_mod_stats(rom)?,
        elements: read_element_rules(rom)?,
    })
}

impl Rules {
    /// The `rules` object `equipment.json` carries.
    pub fn to_json(&self) -> Json {
        let equip = &self.equip;
        let mut handlers = Json::object();
        for (item_type, handler) in &equip.handlers {
            handlers.insert(item_type.to_string(), handler.to_json());
        }
        let equip_json = Json::object()
            .with("routine", hex6(equip.routine))
            .with("jump_table", hex6(equip.jump_table))
            .with(
                "filters",
                equip.filters.iter().map(|&f| hex6(f)).collect::<Vec<_>>(),
            )
            .with("type_offset", hex2(equip.type_offset))
            .with("usable_by_offset", hex2(equip.usable_by))
            .with("max_equippable_type", equip.max_type)
            .with("slots", SLOTS.to_vec())
            .with("handlers", handlers);
        let attack = &self.attack;
        let attack_json = Json::object()
            .with("routine", hex6(attack.routine))
            .with(
                "item_type",
                Json::object()
                    .with("label", "Battle_GetItemType")
                    .with("rom_offset", hex6(attack.get_type)),
            )
            .with(
                "weapon_types",
                attack.weapon_types.iter().copied().collect::<Vec<_>>(),
            )
            .with(
                "multi_target_types",
                attack
                    .multi_target_types
                    .iter()
                    .copied()
                    .collect::<Vec<_>>(),
            )
            .with("note", ATTACK_NOTE);
        let mods = &self.derived;
        let adder = |label: &str, at: usize| {
            Json::object()
                .with("label", label)
                .with("rom_offset", hex6(at))
        };
        let derived_json = Json::object()
            .with("routine", hex6(mods.routine))
            .with("jump_table", hex6(mods.jump_table))
            .with("inventory_data", hex6(mods.inventory))
            .with("record_bytes", mods.stride)
            .with("equipment_slots", mods.slots)
            .with(
                "adders",
                Json::object()
                    .with("byte", adder("AddItemBonusToCharStats", mods.byte_adder))
                    .with("word", adder("AddItemBonusToCharStats2", mods.word_adder)),
            )
            .with(
                "passes",
                mods.passes
                    .iter()
                    .map(mod_stats::ModPass::to_json)
                    .collect::<Vec<_>>(),
            );
        let e = &self.elements;
        let elements_json = Json::object()
            .with("routine", hex6(e.routine))
            .with("element_offset", hex2(e.element_offset))
            .with("type_offset", hex2(e.type_offset))
            .with("shield_type", e.shield_type)
            .with("elements", e.elements)
            .with("property_base", hex2(e.props))
            .with("property_stride", 2u8)
            .with("resistance_value", e.resistance_value)
            .with(
                "weapon_element_slots",
                Json::object()
                    .with("right_hand", hex2(e.right))
                    .with("left_hand", hex2(e.left)),
            )
            .with("note", ELEMENT_NOTE);
        Json::object()
            .with("equip", equip_json)
            .with("attack", attack_json)
            .with("derived_stats", derived_json)
            .with("elements", elements_json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ram_fields_name_the_property_words_and_refuse_the_unknown() {
        assert_eq!(ram_field(0x30).as_deref(), Some("physical_prop"));
        assert_eq!(ram_field(0x31).as_deref(), Some("physical_prop+1"));
        assert_eq!(ram_field(0x4B).as_deref(), Some("destroy_prop+1"));
        assert_eq!(ram_field(0x24).as_deref(), Some("atk_pow"));
        assert!(ram_field(0x16).is_none());
        assert!(named_ram(0x7F).is_err());
    }
}
