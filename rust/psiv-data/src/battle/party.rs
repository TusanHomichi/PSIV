//! `battle/characters.json` and `battle/equipment.json` — the seating path.
//!
//! Together these are everything the engine needs to put a party on the
//! field: the 66-byte `InitialCharStats` records, the inventory records whose
//! type byte and stat bonuses the derivation passes read, and the pack's own
//! `initialized` conformance vector for the result.

use super::{LevelStats, NamedId, Property};
use serde::{Deserialize, Serialize};

/// `battle/characters.json` — the eleven 66-byte `InitialCharStats` records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharactersFile {
    /// The pack format that wrote this.
    pub format_version: u32,
    /// How many records the file declares.
    pub count: u32,
    /// The records, in `Character_Stats` order.
    pub characters: Vec<Character>,
}

/// One starting character.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Character {
    /// Index into `Character_Stats`, and the key into `levels.json`.
    pub character_id: u8,
    /// The disassembly's identifier.
    pub symbol: String,
    /// The cartridge's own name.
    #[serde(default)]
    pub display_name: Option<String>,
    /// `ProfessionID_*`.
    pub profession: NamedId,
    /// Starting level.
    pub level: u16,
    /// Starting experience. Eight of the eleven join with some already.
    pub experience: u32,
    /// Current HP.
    pub hp: u16,
    /// Maximum HP — the record mirrors one value into both.
    pub max_hp: u16,
    /// Current TP.
    pub tp: u16,
    /// Maximum TP.
    pub max_tp: u16,
    /// The four base stats, before equipment.
    pub stats: LevelStats,
    /// Innate element resistances, keyed by slot name. These land in the *low*
    /// halves of the property words; [`Initialized::element_props`] is what the
    /// damage pipeline reads.
    pub properties: std::collections::BTreeMap<String, Property>,
    /// What the character starts wearing.
    pub equipment: Loadout,
    /// What `InitializeCharStats` leaves in RAM — the conformance vector.
    pub initialized: Initialized,
}

/// The four equipment slots. A slot the character starts with empty is `null`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Loadout {
    /// `$4C`.
    #[serde(default)]
    pub right_hand: Option<Equipped>,
    /// `$4D`.
    #[serde(default)]
    pub left_hand: Option<Equipped>,
    /// `$4E`.
    #[serde(default)]
    pub head: Option<Equipped>,
    /// `$4F`.
    #[serde(default)]
    pub body: Option<Equipped>,
}

impl Loadout {
    /// The four slots in record order, named.
    #[must_use]
    pub fn slots(&self) -> [(&'static str, Option<&Equipped>); 4] {
        [
            ("right_hand", self.right_hand.as_ref()),
            ("left_hand", self.left_hand.as_ref()),
            ("head", self.head.as_ref()),
            ("body", self.body.as_ref()),
        ]
    }

    /// The four item ids in record order, `0` for an empty slot — the shape
    /// `Stats::equipment` wants.
    #[must_use]
    pub fn item_ids(&self) -> [u8; 4] {
        let mut ids = [0u8; 4];
        for (index, (_, filled)) in self.slots().iter().enumerate() {
            ids[index] = filled.map_or(0, |item| item.item_id);
        }
        ids
    }
}

/// One filled equipment slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Equipped {
    /// Index into `InventoryData`.
    pub item_id: u8,
    /// The disassembly's identifier, so a loadout reads without a lookup.
    pub symbol: String,
    /// The cartridge's own name.
    #[serde(default)]
    pub display_name: Option<String>,
    /// The item's type byte, repeated here for the same reason.
    #[serde(rename = "type")]
    pub kind: u8,
}

/// What the two derivation passes must produce.
///
/// `InitializeCharStats` (`$0044652`) finishes every character with
/// `UpdateCharModStats` (`$0005F754`) and `UpdateCharElems` (`$0005FD2A`); this
/// is the result. `psiv-core` reproduces all eleven from the record alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Initialized {
    /// Each base stat with its equipment-modified value.
    pub stats: std::collections::BTreeMap<String, DerivedStat>,
    /// `atk_pow` — base strength plus every item's strength and attack bonus.
    pub atk_pow: u16,
    /// `dfs_pow` — base agility plus every item's agility and defence bonus.
    pub dfs_pow: u16,
    /// `magic_dfs` — base mental plus every item's mental and magic-defence
    /// bonus.
    pub magic_dfs: u16,
    /// The finished element properties, after armour grants and the fallback
    /// to the record's own bytes.
    pub element_props: std::collections::BTreeMap<String, Property>,
    /// `$50` and `$51`: what element each hand's weapon swings with.
    pub weapon_elements: WeaponElements,
}

/// One stat before and after equipment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DerivedStat {
    /// What the record holds.
    pub base: u8,
    /// What `UpdateCharModStats` computed.
    #[serde(rename = "mod")]
    pub modified: u8,
}

/// The two cached weapon elements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaponElements {
    /// `$50`.
    pub right_hand: NamedId,
    /// `$51`.
    pub left_hand: NamedId,
}

/// `battle/equipment.json` — the 160 `InventoryData` records and the decoded
/// type table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquipmentFile {
    /// The pack format that wrote this.
    pub format_version: u32,
    /// How many records the file declares.
    pub count: u32,
    /// `Equip_Item`'s jump table (`$05F6AE`), read out of the ROM.
    pub types: Vec<EquipmentType>,
    /// The records, in id order.
    pub items: Vec<Equipment>,
}

impl EquipmentFile {
    /// Looks a type up by its byte.
    #[must_use]
    pub fn kind(&self, byte: u8) -> Option<&EquipmentType> {
        self.types.iter().find(|entry| entry.kind == byte)
    }

    /// Looks an item up by id.
    #[must_use]
    pub fn item(&self, id: u8) -> Option<&Equipment> {
        self.items.iter().find(|item| item.id == id)
    }
}

/// One entry of the decoded type table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquipmentType {
    /// The record's type byte (`$A`).
    #[serde(rename = "type")]
    pub kind: u8,
    /// The extractor's name for it.
    pub name: String,
    /// Which slot `Equip_Item` puts it in, or `None` for the three types it
    /// refuses.
    #[serde(default)]
    pub slot: Option<String>,
    /// Whether equipping it clears the other hand.
    pub two_handed: bool,
    /// Whether it can swing.
    pub is_weapon: bool,
    /// Whether a swing reaches every enemy.
    pub multi_target: bool,
    /// Whether it can be equipped at all.
    pub equippable: bool,
    /// What its element byte means: `attack_element`, `resistance_granted`, or
    /// nothing.
    #[serde(default)]
    pub element_role: Option<String>,
}

/// One 22-byte `InventoryData` record, in the parts battle reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Equipment {
    /// Index into `InventoryData`, one-based.
    pub id: u8,
    /// The disassembly's identifier.
    pub symbol: String,
    /// The cartridge's own name.
    #[serde(default)]
    pub display_name: Option<String>,
    /// Record byte `$A`.
    #[serde(rename = "type")]
    pub kind: EquipmentKindRef,
    /// Record bytes `$B`..`$11`. Signed: retail bonuses run from `-10` to
    /// `+127`.
    pub bonuses: EquipmentBonuses,
    /// Record byte `$12`, with the role its type gives it.
    pub element: ElementRef,
    /// Whether it can swing.
    pub is_weapon: bool,
    /// Whether a swing reaches every enemy.
    pub multi_target: bool,
    /// Whether equipping it clears the other hand.
    pub two_handed: bool,
    /// Record byte `$13`: what a hit with it inflicts. Tier 2.
    #[serde(default)]
    pub post_attack_effect_id: Option<u8>,
    /// Record bytes `$08..$09`, decoded as the character usability bitmask.
    ///
    /// This stays as the pack's hexadecimal spelling at the schema boundary;
    /// the runtime parses it fail-closed when it builds the camp command seam.
    #[serde(default)]
    pub equippable_by_mask: String,
}

/// An item's type, by byte and name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquipmentKindRef {
    /// The type byte.
    pub id: u8,
    /// The extractor's name.
    pub name: String,
}

/// An item's element byte and what it does with it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElementRef {
    /// The element id, `0` for none.
    pub id: u8,
    /// The element's name, when it has one.
    #[serde(default)]
    pub name: Option<String>,
    /// `attack_element` on a weapon, `resistance_granted` on armour, absent on
    /// anything unequippable.
    #[serde(default)]
    pub role: Option<String>,
}

/// The seven stat bonuses at record offsets `$B`..`$11`.
///
/// Signed. `AddItemBonusToCharStats2` sign-extends them for the three derived
/// words; `AddItemBonusToCharStats` does not for the four small stats, and
/// retail really does carry negative bonuses (agility down to `-5`, mental and
/// dexterity to `-10`), so the difference between the two adders is live rather
/// than theoretical.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquipmentBonuses {
    pub strength: i8,
    pub mental: i8,
    pub agility: i8,
    pub dexterity: i8,
    pub attack: i8,
    pub defense: i8,
    pub magic_defense: i8,
}

impl Equipment {
    /// Parses the decoded usable-by mask, or `None` for absent/malformed data.
    ///
    /// A missing mask must not turn into an all-party permission. Keeping the
    /// parser here makes that rule reusable without changing the core item
    /// record or its shared `Stats` interface.
    #[must_use]
    pub fn usable_by_mask(&self) -> Option<u16> {
        let raw = self
            .equippable_by_mask
            .strip_prefix("0x")
            .or_else(|| self.equippable_by_mask.strip_prefix("0X"))?;
        u16::from_str_radix(raw, 16).ok()
    }
}
