//! The cartridge's equipment transaction.
//!
//! `Equip_Item` is deliberately kept beside, rather than inside, [`Stats`].
//! The latter is the shared 128-byte character interface between field and
//! battle and its shape is locked. This module owns the command-level rules:
//! the inventory filter, the type-to-slot dispatch, displaced-item exchange,
//! and the derivation refresh at the command boundary.

use core::fmt;

use crate::inventory::{EMPTY, INVENTORY_SLOTS, Inventory};

use super::records::{EQUIPMENT_SLOTS, EquipSlot, ItemKind, ItemRecord};
use super::stats::Stats;

/// One legal entry in the equipment list, retaining the raw inventory slot.
///
/// The cartridge's list is a filtered view over all forty inventory bytes, so
/// the slot is part of the command identity. Item ids alone are insufficient
/// when the party carries duplicates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EquipmentCandidate {
    /// The raw party inventory slot.
    pub inventory_slot: usize,
    /// The one-based item id in that slot.
    pub item_id: u8,
}

/// Why an equip or unequip command was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EquipmentError {
    /// The command named a byte outside the forty-byte inventory.
    InventorySlotOutOfRange(usize),
    /// The named inventory byte is empty.
    EmptyInventorySlot(usize),
    /// The named item id is not in the battle data.
    UnknownItem(u8),
    /// The item type is not one of 1..=7.
    NotEquippable {
        /// The item id whose type cannot occupy an equipment slot.
        item_id: u8,
        /// The decoded item kind that was rejected.
        kind: ItemKind,
    },
    /// The item has no usable-by mask in the runtime pack. Missing or malformed
    /// masks are rejected rather than becoming an accidental all-party grant.
    MissingUsableByMask(u8),
    /// The character's bit is clear in the item mask.
    NotUsableBy {
        /// The item id whose usable-by mask excludes the character.
        item_id: u8,
        /// The character id whose bit was clear.
        character_id: u8,
    },
    /// An already equipped item could not be resolved while preparing the
    /// exchange. The transaction remains untouched.
    InvalidExistingItem(u8),
    /// An unequip needs one free inventory byte.
    InventoryFull,
    /// The requested equipment slot is not one of the four character bytes.
    EquipmentSlotOutOfRange(usize),
    /// The requested equipment slot is already empty.
    EmptyEquipmentSlot(EquipSlot),
}

impl fmt::Display for EquipmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EquipmentError::InventorySlotOutOfRange(slot) => {
                write!(f, "inventory slot {slot} is outside 0..{INVENTORY_SLOTS}")
            }
            EquipmentError::EmptyInventorySlot(slot) => write!(f, "inventory slot {slot} is empty"),
            EquipmentError::UnknownItem(id) => write!(f, "no item record for id {id}"),
            EquipmentError::NotEquippable { item_id, kind } => {
                write!(f, "item {item_id} has non-equippable type {kind:?}")
            }
            EquipmentError::MissingUsableByMask(id) => {
                write!(f, "item {id} has no usable-by mask")
            }
            EquipmentError::NotUsableBy {
                item_id,
                character_id,
            } => write!(
                f,
                "item {item_id} is not usable by character {character_id}"
            ),
            EquipmentError::InvalidExistingItem(id) => {
                write!(f, "equipped item {id} has no item record")
            }
            EquipmentError::InventoryFull => write!(f, "inventory is full"),
            EquipmentError::EquipmentSlotOutOfRange(slot) => {
                write!(f, "equipment slot {slot} is outside 0..{EQUIPMENT_SLOTS}")
            }
            EquipmentError::EmptyEquipmentSlot(slot) => {
                write!(f, "equipment slot {slot:?} is empty")
            }
        }
    }
}

impl core::error::Error for EquipmentError {}

/// Returns the cartridge's equipment-list candidates in inventory order.
///
/// The retail loop first rejects type bytes above seven, then tests the
/// selected character bit in record offset `$08`. Every failed lookup is
/// skipped: a missing item record or missing mask can never become an equip
/// command by accident.
#[must_use]
pub fn equipment_candidates(
    inventory: &Inventory,
    character_id: u8,
    item: &impl Fn(u8) -> Option<ItemRecord>,
    usable_by_mask: &impl Fn(u8) -> Option<u16>,
) -> Vec<EquipmentCandidate> {
    inventory
        .slots()
        .iter()
        .enumerate()
        .filter_map(|(inventory_slot, &item_id)| {
            if item_id == EMPTY {
                return None;
            }
            let record = item(item_id)?;
            if !record.kind.is_equippable() {
                return None;
            }
            let mask = usable_by_mask(item_id)?;
            if u32::from(character_id) >= u16::BITS {
                return None;
            }
            if mask & (1u16 << character_id) == 0 {
                return None;
            }
            Some(EquipmentCandidate {
                inventory_slot,
                item_id,
            })
        })
        .collect()
}

/// Equips the item in `inventory_slot`, applying the cartridge's type dispatch
/// and displaced-item exchange.
///
/// The caller supplies the pack's item table and the separate usable-by mask
/// table because `Stats` intentionally does not grow a data-pack field. The
/// transaction validates all records and capacity before mutating either
/// input. On success it immediately refreshes modified/battle stats and the
/// element caches, so the persistent record is coherent for the next battle.
pub fn equip_item(
    stats: &mut Stats,
    inventory: &mut Inventory,
    character_id: u8,
    inventory_slot: usize,
    item: &impl Fn(u8) -> Option<ItemRecord>,
    usable_by_mask: &impl Fn(u8) -> Option<u16>,
) -> Result<(), EquipmentError> {
    let Some(item_id) = inventory.slots().get(inventory_slot).copied() else {
        return Err(EquipmentError::InventorySlotOutOfRange(inventory_slot));
    };
    if item_id == EMPTY {
        return Err(EquipmentError::EmptyInventorySlot(inventory_slot));
    }
    let selected = item(item_id).ok_or(EquipmentError::UnknownItem(item_id))?;
    let slot = selected.kind.slot().ok_or(EquipmentError::NotEquippable {
        item_id,
        kind: selected.kind,
    })?;
    let mask = usable_by_mask(item_id).ok_or(EquipmentError::MissingUsableByMask(item_id))?;
    if u32::from(character_id) >= u16::BITS || mask & (1u16 << character_id) == 0 {
        return Err(EquipmentError::NotUsableBy {
            item_id,
            character_id,
        });
    }

    let (equipment, displaced) = prepare_equipment(stats, slot, &selected, item)?;
    validate_displaced(displaced, item)?;

    // E3FE replaces the selected inventory byte. E3FF is then inserted into
    // the first free byte. A full list is valid only when replacement itself
    // does not need the second displaced item.
    if displaced[1] != EMPTY && displaced[0] != EMPTY && inventory.is_full() {
        return Err(EquipmentError::InventoryFull);
    }

    if displaced[0] == EMPTY {
        let _ = inventory.remove(inventory_slot);
        inventory.compact();
    } else {
        inventory
            .swap(inventory_slot, displaced[0])
            .map_err(|_| EquipmentError::InventoryFull)?;
    }
    if displaced[1] != EMPTY {
        inventory
            .add(displaced[1])
            .map_err(|_| EquipmentError::InventoryFull)?;
    }

    stats.equipment = equipment;
    refresh(stats, item);
    Ok(())
}

/// Unequips one of the four raw character equipment slots.
///
/// The UI's order is head/right/left/body; the seam deliberately takes the
/// raw `$4C..$4F` index so callers cannot accidentally change the shared
/// `Stats` layout while translating presentation order.
pub fn unequip_item(
    stats: &mut Stats,
    inventory: &mut Inventory,
    equipment_slot: usize,
    item: &impl Fn(u8) -> Option<ItemRecord>,
) -> Result<(), EquipmentError> {
    let slot = match equipment_slot {
        0 => EquipSlot::RightHand,
        1 => EquipSlot::LeftHand,
        2 => EquipSlot::Head,
        3 => EquipSlot::Body,
        other => return Err(EquipmentError::EquipmentSlotOutOfRange(other)),
    };
    let item_id = stats.equipment[equipment_slot];
    if item_id == EMPTY {
        return Err(EquipmentError::EmptyEquipmentSlot(slot));
    }
    if item(item_id).is_none() {
        return Err(EquipmentError::InvalidExistingItem(item_id));
    }
    if inventory.is_full() {
        return Err(EquipmentError::InventoryFull);
    }

    inventory
        .add(item_id)
        .map_err(|_| EquipmentError::InventoryFull)?;
    stats.equipment[equipment_slot] = EMPTY;
    refresh(stats, item);
    Ok(())
}

fn prepare_equipment(
    stats: &Stats,
    slot: EquipSlot,
    selected: &ItemRecord,
    item: &impl Fn(u8) -> Option<ItemRecord>,
) -> Result<([u8; EQUIPMENT_SLOTS], [u8; 2]), EquipmentError> {
    let mut equipment = stats.equipment;
    let mut displaced = [EMPTY; 2];
    match slot {
        EquipSlot::RightHand => {
            displaced[0] = equipment[EquipSlot::RightHand.index()];
            equipment[EquipSlot::RightHand.index()] = selected.id;
            if selected.kind.is_two_handed() {
                displaced[1] = equipment[EquipSlot::LeftHand.index()];
                equipment[EquipSlot::LeftHand.index()] = EMPTY;
            }
        }
        EquipSlot::LeftHand => {
            let right = equipment[EquipSlot::RightHand.index()];
            let right_is_two_handed = if right == EMPTY {
                false
            } else {
                item(right)
                    .ok_or(EquipmentError::InvalidExistingItem(right))?
                    .kind
                    .is_two_handed()
            };
            if right_is_two_handed {
                // Retail's shield handler clears the two-handed right hand
                // and writes the shield into the left hand. It does not add a
                // separate left-hand displacement; a valid two-handed state
                // has that byte empty, and malformed states follow the same
                // overwrite semantics.
                displaced[0] = right;
                equipment[EquipSlot::RightHand.index()] = EMPTY;
            } else {
                displaced[0] = equipment[EquipSlot::LeftHand.index()];
            }
            equipment[EquipSlot::LeftHand.index()] = selected.id;
        }
        EquipSlot::Head | EquipSlot::Body => {
            let index = slot.index();
            displaced[0] = equipment[index];
            equipment[index] = selected.id;
        }
    }
    Ok((equipment, displaced))
}

fn validate_displaced(
    displaced: [u8; 2],
    item: &impl Fn(u8) -> Option<ItemRecord>,
) -> Result<(), EquipmentError> {
    for id in displaced.into_iter().filter(|&id| id != EMPTY) {
        if item(id).is_none() {
            return Err(EquipmentError::InvalidExistingItem(id));
        }
    }
    Ok(())
}

fn refresh(stats: &mut Stats, item: &impl Fn(u8) -> Option<ItemRecord>) {
    stats.update_mod_stats(item);
    stats.update_char_elems(item);
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::battle::engine::PartyMember;
    use crate::battle::party_fixtures;
    use crate::battle::records::BattleData;

    fn data() -> BattleData {
        BattleData::new().with_items(party_fixtures::equipment())
    }

    fn masks() -> impl Fn(u8) -> Option<u16> {
        |id| match id {
            // Dagger is Chaz/Hahn; Hunt-Knife is Chaz/Alys/Rika.
            1 => Some(0x0405),
            2 => Some(0x0405),
            // Boomerang is Alys and the later hunter loadouts in the pack.
            3 => Some(0x0202),
            10 => Some(0x073F),
            16 => Some(0x0001),
            118 => Some(0x03FF),
            _ => Some(0x0405),
        }
    }

    fn chaz() -> Stats {
        let data = data();
        PartyMember::seat(&party_fixtures::records()[0], &data)
            .expect("Chaz fixture seats")
            .stats
    }

    #[test]
    fn candidates_apply_type_then_mask_in_inventory_order() {
        let mut inventory = Inventory::new();
        inventory.add(1).unwrap(); // legal for Chaz
        inventory.add(118).unwrap(); // mask says yes, type 9 says no
        inventory.add(3).unwrap(); // mask says no for Chaz
        inventory.add(0xEE).unwrap(); // missing item record
        let data = data();
        let item = |id| {
            if id == 118 {
                Some(ItemRecord {
                    id,
                    name: "NOTHING2".into(),
                    kind: ItemKind::Plot,
                    bonuses: Default::default(),
                    element: 0,
                })
            } else {
                data.item(id).ok().cloned()
            }
        };

        assert_eq!(
            equipment_candidates(&inventory, 0, &item, &masks()),
            vec![EquipmentCandidate {
                inventory_slot: 0,
                item_id: 1
            }]
        );
    }

    #[test]
    fn legal_and_illegal_equip_are_fail_closed() {
        let data = data();
        let item = |id| {
            if id == 118 {
                Some(ItemRecord {
                    id,
                    name: "NOTHING2".into(),
                    kind: ItemKind::Plot,
                    bonuses: Default::default(),
                    element: 0,
                })
            } else {
                data.item(id).ok().cloned()
            }
        };
        let masks = masks();
        let mut legal_inventory = Inventory::new();
        legal_inventory.add(1).unwrap();
        let mut stats = chaz();
        equip_item(&mut stats, &mut legal_inventory, 0, 0, &item, &masks).unwrap();
        assert_eq!(stats.equipment[0], 1);

        let mut illegal_inventory = Inventory::new();
        illegal_inventory.add(3).unwrap();
        let before = chaz();
        let mut stats = before.clone();
        let error = equip_item(&mut stats, &mut illegal_inventory, 0, 0, &item, &masks);
        assert!(matches!(error, Err(EquipmentError::NotUsableBy { .. })));
        assert_eq!(stats, before);
        assert_eq!(illegal_inventory.get(0), Some(3));

        let mut disposable = Inventory::new();
        disposable.add(118).unwrap();
        let error = equip_item(&mut stats, &mut disposable, 0, 0, &item, &masks);
        assert!(matches!(error, Err(EquipmentError::NotEquippable { .. })));
    }

    #[test]
    fn equip_refreshes_modified_and_battle_stats_before_returning() {
        let data = data();
        let item = |id| data.item(id).ok().cloned();
        let masks = masks();
        let mut inventory = Inventory::new();
        inventory.add(1).unwrap();
        let mut stats = chaz();
        assert_eq!(stats.attack.derived, 18);
        equip_item(&mut stats, &mut inventory, 0, 0, &item, &masks).unwrap();
        assert_eq!(stats.attack.derived, 15);
        assert_eq!(stats.attack.battle, 15);
        assert_eq!(stats.strength.modified, stats.strength.battle);
        assert_eq!(stats.weapon_elements, [1, 1]);
    }

    #[test]
    fn two_handed_equip_clears_left_and_returns_both_displaced_items() {
        let data = data();
        let item = |id| data.item(id).ok().cloned();
        let masks = masks();
        let mut inventory = Inventory::new();
        inventory.add(16).unwrap();
        let mut stats = chaz();
        equip_item(&mut stats, &mut inventory, 0, 0, &item, &masks).unwrap();
        assert_eq!(stats.equipment[0], 16);
        assert_eq!(stats.equipment[1], 0);
        assert!(inventory.contains(2), "old right hand returned");
        assert!(inventory.contains(2), "old left hand returned");
        assert_eq!(inventory.occupied(), 2);
    }

    #[test]
    fn starting_left_hand_weapon_is_permanently_lost_from_the_left_slot() {
        let data = data();
        let item = |id| data.item(id).ok().cloned();
        let masks = masks();
        let mut inventory = Inventory::new();
        let mut stats = chaz();
        unequip_item(
            &mut stats,
            &mut inventory,
            EquipSlot::LeftHand.index(),
            &item,
        )
        .unwrap();
        assert_eq!(stats.equipment[1], 0);
        assert_eq!(inventory.get(0), Some(2));

        equip_item(&mut stats, &mut inventory, 0, 0, &item, &masks).unwrap();
        assert_eq!(stats.equipment[0], 2);
        assert_eq!(stats.equipment[1], 0);
    }

    #[test]
    fn equipment_round_trip_matches_party_member_seat() {
        let data = data();
        let item = |id| data.item(id).ok().cloned();
        let masks = masks();
        let record = party_fixtures::records()[0].clone();
        let mut stats = PartyMember::seat(&record, &data).unwrap().stats;
        let mut inventory = Inventory::new();
        inventory.add(1).unwrap();
        equip_item(&mut stats, &mut inventory, 0, 0, &item, &masks).unwrap();

        let mut expected_record = record;
        expected_record.equipment = stats.equipment;
        let expected = PartyMember::seat(&expected_record, &data).unwrap().stats;
        assert_eq!(stats.strength.modified, expected.strength.modified);
        assert_eq!(stats.mental.modified, expected.mental.modified);
        assert_eq!(stats.agility.modified, expected.agility.modified);
        assert_eq!(stats.dexterity.modified, expected.dexterity.modified);
        assert_eq!(stats.attack.derived, expected.attack.derived);
        assert_eq!(stats.defence.derived, expected.defence.derived);
        assert_eq!(
            stats.mental_defence.derived,
            expected.mental_defence.derived
        );
        assert_eq!(stats.element_props, expected.element_props);
        assert_eq!(stats.weapon_elements, expected.weapon_elements);
    }
}
