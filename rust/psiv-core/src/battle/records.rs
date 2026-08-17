//! The cartridge records battle reads, as plain in-memory types.
//!
//! `psiv-core` never depends on `psiv-data` — the dependency runs the other
//! way round the bridge — so the engine defines the shapes it needs and the
//! bridge fills them in from the pack. Everything here is data as the cartridge
//! stores it, with the field names the disassembly uses rather than the ones
//! the JSON happens to use.

use core::fmt;
use std::collections::BTreeMap;

/// How many element-resistance slots a fighter carries: `$30`..`$4B`.
pub const ELEMENT_SLOTS: usize = 14;

/// How many equipment slots a character has: right hand, left hand, head, body.
pub const EQUIPMENT_SLOTS: usize = 4;

/// How many technique ids a character record carries at `$52..$61`.
pub const TECHNIQUE_SLOTS: usize = 16;

/// How many skill ids and use-count pairs a character record carries.
pub const SKILL_SLOTS: usize = 8;

/// How many regular abilities an enemy's AI picks between (`$58`..`$5F`).
pub const REGULAR_ABILITIES: usize = 8;

/// How many conditional AI entries an enemy has (`$50`..`$53` paired with
/// `$54`..`$57`).
pub const AI_CONDITIONS: usize = 4;

/// A 48-byte `EnemyData` record, expanded.
///
/// Field names follow `Battle_FillEnemyStats` (`ps4.asm:11939`). Note which
/// struct slots it writes: the live values land in the `_battle` variants and
/// the base bytes stay zero, which is exactly what the oracle observed in RAM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnemyRecord {
    /// Index into `EnemyData`.
    pub id: u16,
    /// The cartridge's display name, for the event timeline.
    pub name: String,
    /// Starting and maximum HP — the record has one value for both.
    pub hp: u16,
    /// `strength_battle`.
    pub strength: u8,
    /// `mental_battle`.
    pub mental: u8,
    /// `agility_battle`, the turn-order key.
    pub agility: u8,
    /// `dexterity_battle`, the hit roll's actor stat.
    pub dexterity: u8,
    /// `atk_pow` / `atk_pow_battle`.
    pub attack: u16,
    /// `dfs_pow` / `dfs_pow_battle`, stored in the word's low byte.
    pub defence: u16,
    /// `magic_dfs` / `magic_dfs_battle`.
    pub mental_defence: u16,
    /// What element a plain attack from this enemy carries — the `curr_tp`
    /// slot, reused (`ps4.asm:11952`).
    pub attack_element: u8,
    /// What status a plain attack inflicts — the `max_tp` slot, reused. Tier 2.
    pub attack_status: u8,
    /// Resistance to each element, physical first. 0 immune, 2 normal, 4 very
    /// weak; the damage pipeline multiplies by this and divides by four.
    pub properties: [u8; ELEMENT_SLOTS],
    /// The eight abilities [`choose_ability`](crate::battle::choose_ability)
    /// picks between.
    pub regular_abilities: [u8; REGULAR_ABILITIES],
    /// Four AI condition ids, `0` terminating. Tier 2.
    pub condition_ids: [u8; AI_CONDITIONS],
    /// The ability each condition substitutes when it fires. Tier 2.
    pub conditional_abilities: [u8; AI_CONDITIONS],
    /// Added to the battle's experience pool on death.
    pub experience: u16,
    /// Added to the battle's meseta pool on death.
    pub meseta: u16,
}

/// Which equipment slot a type belongs in.
///
/// From the pack's decoded type table (`battle/equipment.json`, `types`),
/// which is `Equip_Item`'s jump table (`$05F6AE`) read out rather than guessed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EquipSlot {
    /// Types 1..=4.
    RightHand,
    /// Type 5.
    LeftHand,
    /// Type 6.
    Head,
    /// Type 7.
    Body,
}

impl EquipSlot {
    /// Its index into [`Stats::equipment`](crate::battle::Stats::equipment).
    #[must_use]
    pub const fn index(&self) -> usize {
        match self {
            EquipSlot::RightHand => 0,
            EquipSlot::LeftHand => 1,
            EquipSlot::Head => 2,
            EquipSlot::Body => 3,
        }
    }
}

/// What an item's element byte (`$12`) means for that item.
///
/// The byte has two jobs and the type decides which — the dual role
/// SOURCE_NOTES records. `UpdateCharElems` (`$0005FD2A`) is where the split
/// happens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ElementRole {
    /// Types 1..=4: the element this weapon's swings carry.
    AttackElement,
    /// Types 5..=7: the element this armour grants resistance to.
    ResistanceGranted,
}

/// An inventory record's type byte (`$A`).
///
/// `Battle_AttackCommand` (`ps4.asm:2235`) branches on exactly this: types 2
/// and 4 target every enemy at once, 1 and 3 target one, and anything above 4
/// is not a weapon at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ItemKind {
    /// Type 1.
    OneHandedSingleTarget,
    /// Type 2 — Boomerang, Slasher.
    OneHandedMultiTarget,
    /// Type 3.
    TwoHandedSingleTarget,
    /// Type 4 — the guns.
    TwoHandedMultiTarget,
    /// Type 5.
    Shield,
    /// Type 6.
    Headwear,
    /// Type 7.
    Body,
    /// Type 8.
    Disposable,
    /// Type 9.
    Plot,
    /// Type 10.
    FieldOnly,
}

impl ItemKind {
    /// Builds from the record's type byte.
    #[must_use]
    pub const fn from_byte(byte: u8) -> Option<ItemKind> {
        Some(match byte {
            1 => ItemKind::OneHandedSingleTarget,
            2 => ItemKind::OneHandedMultiTarget,
            3 => ItemKind::TwoHandedSingleTarget,
            4 => ItemKind::TwoHandedMultiTarget,
            5 => ItemKind::Shield,
            6 => ItemKind::Headwear,
            7 => ItemKind::Body,
            8 => ItemKind::Disposable,
            9 => ItemKind::Plot,
            10 => ItemKind::FieldOnly,
            _ => return None,
        })
    }

    /// Whether `Battle_AttackCommand` will let this swing at anything.
    ///
    /// Types 1..=4. A shield in hand contributes its bonuses but cannot attack,
    /// which is why a character holding only shields has no Attack command.
    #[must_use]
    pub const fn is_weapon(&self) -> bool {
        matches!(
            self,
            ItemKind::OneHandedSingleTarget
                | ItemKind::OneHandedMultiTarget
                | ItemKind::TwoHandedSingleTarget
                | ItemKind::TwoHandedMultiTarget
        )
    }

    /// Whether an attack with this weapon hits every enemy.
    #[must_use]
    pub const fn is_multi_target(&self) -> bool {
        matches!(
            self,
            ItemKind::OneHandedMultiTarget | ItemKind::TwoHandedMultiTarget
        )
    }

    /// Which slot `Equip_Item` puts this in, or `None` for the three types it
    /// refuses to equip at all (`max_equippable_type` is 7).
    #[must_use]
    pub const fn slot(&self) -> Option<EquipSlot> {
        Some(match self {
            ItemKind::OneHandedSingleTarget
            | ItemKind::OneHandedMultiTarget
            | ItemKind::TwoHandedSingleTarget
            | ItemKind::TwoHandedMultiTarget => EquipSlot::RightHand,
            ItemKind::Shield => EquipSlot::LeftHand,
            ItemKind::Headwear => EquipSlot::Head,
            ItemKind::Body => EquipSlot::Body,
            ItemKind::Disposable | ItemKind::Plot | ItemKind::FieldOnly => return None,
        })
    }

    /// Whether equipping this clears the other hand (types 3 and 4).
    #[must_use]
    pub const fn is_two_handed(&self) -> bool {
        matches!(
            self,
            ItemKind::TwoHandedSingleTarget | ItemKind::TwoHandedMultiTarget
        )
    }

    /// Whether this can be equipped at all.
    #[must_use]
    pub const fn is_equippable(&self) -> bool {
        self.slot().is_some()
    }

    /// What this type's element byte means.
    #[must_use]
    pub const fn element_role(&self) -> Option<ElementRole> {
        Some(match self {
            ItemKind::OneHandedSingleTarget
            | ItemKind::OneHandedMultiTarget
            | ItemKind::TwoHandedSingleTarget
            | ItemKind::TwoHandedMultiTarget => ElementRole::AttackElement,
            ItemKind::Shield | ItemKind::Headwear | ItemKind::Body => {
                ElementRole::ResistanceGranted
            }
            ItemKind::Disposable | ItemKind::Plot | ItemKind::FieldOnly => return None,
        })
    }
}

/// The seven stat bonuses at record offsets `$B`..`$11`.
///
/// Signed, because `AddItemBonusToCharStats2` sign-extends them (`ext.w d5`).
/// The four single-stat bonuses go through `AddItemBonusToCharStats`, which
/// uses `add.b` and does *not* sign-extend — a difference this type keeps
/// visible rather than smoothing over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Bonuses {
    /// Offset `$B`.
    pub strength: i8,
    /// Offset `$C`.
    pub mental: i8,
    /// Offset `$D`.
    pub agility: i8,
    /// Offset `$E`.
    pub dexterity: i8,
    /// Offset `$F`.
    pub attack: i8,
    /// Offset `$10`.
    pub defence: i8,
    /// Offset `$11`.
    pub mental_defence: i8,
}

/// A 22-byte `InventoryData` record, in the parts battle reads.
///
/// The embedded eight-byte ability record and the post-hit effect byte are
/// Tier 2; this tier needs the type, the bonuses and the attack element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemRecord {
    /// Index into `InventoryData`, one-based as the cartridge stores it.
    pub id: u8,
    /// The cartridge's display name.
    pub name: String,
    /// Record byte `$A`.
    pub kind: ItemKind,
    /// Record bytes `$B`..`$11`.
    pub bonuses: Bonuses,
    /// Record byte `$12`.
    ///
    /// Two jobs, decided by [`ItemKind::element_role`]: on a weapon it is the
    /// element the swing carries, read by `Battle_LoadWpnAttackElem`
    /// (`$0027DDD4`); on a shield or armour it names the element the wearer
    /// gains resistance to, applied by
    /// [`Stats::update_char_elems`](crate::battle::Stats::update_char_elems).
    pub element: u8,
}

/// One enemy slot of a formation record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormationEnemy {
    /// Which of the four enemy slots this fills, one-based.
    pub slot: u8,
    /// Index into `EnemyData`.
    pub enemy_id: u16,
    /// The on-screen position byte. Presentation only.
    pub position: u8,
}

/// A `$FF`-terminated battle formation.
///
/// The header is the record's first four bytes plus the count and the two group
/// masks; see `docs/BATTLE_SCOUT.md` §9.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormationRecord {
    /// The formation's index across all four data blocks.
    pub id: u16,
    /// Byte 0, compared against the party's highest agility to decide whether
    /// the battle opens with an ambush or a preemptive strike.
    pub ambush_chance: u8,
    /// Byte 1, the same comparison for escape.
    pub run_chance: u8,
    /// Byte 2, out of 128.
    pub drop_rate: u8,
    /// Byte 3. Tier 2 resolves the drop.
    pub drop_item: Option<u8>,
    /// The enemies, in slot order.
    pub enemies: Vec<FormationEnemy>,
}

/// `Enemy_Run_Chance` values at or above this forbid escape outright
/// (`ps4.asm:7707`).
pub const UNRUNNABLE: u8 = 0xF0;

impl FormationRecord {
    /// Whether escape is possible at all against this formation.
    #[must_use]
    pub fn can_run(&self) -> bool {
        self.run_chance < UNRUNNABLE
    }
}

/// An initial `Character_Init` record, in the parts battle reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterRecord {
    /// Index into `Character_Stats`, zero-based (Chaz is 0).
    pub id: u8,
    /// The cartridge's display name.
    pub name: String,
    /// `ProfessionID_*`. Androids behave differently from Tier 2 on.
    pub profession: u16,
    /// Starting level.
    pub level: u16,
    /// Starting experience.
    pub experience: u32,
    /// Current HP.
    pub hp: u16,
    /// Maximum HP. Equal to [`CharacterRecord::hp`] in every initial record.
    pub max_hp: u16,
    /// Current TP.
    pub tp: u16,
    /// Maximum TP.
    pub max_tp: u16,
    /// Base strength, before equipment.
    pub strength: u8,
    /// Base mental.
    pub mental: u8,
    /// Base agility.
    pub agility: u8,
    /// Base dexterity.
    pub dexterity: u8,
    /// Element resistances.
    pub properties: [u8; ELEMENT_SLOTS],
    /// Right hand, left hand, head, body. `0` means empty.
    pub equipment: [u8; EQUIPMENT_SLOTS],
    /// `$52..$61`, the sixteen initial technique ids.
    pub techniques: [u8; TECHNIQUE_SLOTS],
    /// `$62..$69`, the eight initial skill ids.
    pub skills: [u8; SKILL_SLOTS],
    /// `$6A..$79`, one initial use count for each skill slot.
    pub skill_uses: [u8; SKILL_SLOTS],
}

/// One 22-byte level record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelRecord {
    /// The level this record grants.
    pub level: u16,
    /// Total experience needed to reach it.
    pub experience_required: u32,
    /// New maximum HP.
    pub hp: u16,
    /// New maximum TP.
    pub tp: u16,
    /// New base strength.
    pub strength: u8,
    /// New base mental.
    pub mental: u8,
    /// New base agility.
    pub agility: u8,
    /// New base dexterity.
    pub dexterity: u8,
}

/// One character's level table, reached through `CharLevelTablePtrs`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelTable {
    /// The `[starting level .w]` half of the pointer-table entry, subtracted
    /// from the current level to index [`LevelTable::levels`].
    pub starting_level: u16,
    /// Records for `starting_level + 1` upwards, in order.
    pub levels: Vec<LevelRecord>,
}

impl LevelTable {
    /// The record that takes a character from `level` to `level + 1`.
    ///
    /// `ps4.asm:6007-6019`: index by `level - starting_level`, 22 bytes apiece.
    #[must_use]
    pub fn next_after(&self, level: u16) -> Option<&LevelRecord> {
        let index = level.checked_sub(self.starting_level)?;
        self.levels.get(usize::from(index))
    }
}

/// What went wrong looking a battle record up.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum BattleDataError {
    /// A formation named an enemy id with no record.
    UnknownEnemy(u16),
    /// A character had an equipment id with no record.
    UnknownItem(u8),
    /// A party member had no level table.
    UnknownLevelTable(u8),
    /// A formation had no enemies, which no retail record does.
    EmptyFormation(u16),
    /// A formation named more enemies than there are enemy slots.
    TooManyEnemies {
        /// The offending formation.
        formation: u16,
        /// How many it named.
        named: usize,
    },
}

impl fmt::Display for BattleDataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BattleDataError::UnknownEnemy(id) => write!(f, "no enemy record for id {id}"),
            BattleDataError::UnknownItem(id) => write!(f, "no item record for id {id}"),
            BattleDataError::UnknownLevelTable(id) => {
                write!(f, "no level table for character {id}")
            }
            BattleDataError::EmptyFormation(id) => write!(f, "formation {id} names no enemies"),
            BattleDataError::TooManyEnemies { formation, named } => write!(
                f,
                "formation {formation} names {named} enemies; the cartridge has four slots"
            ),
        }
    }
}

impl core::error::Error for BattleDataError {}

/// Everything the engine looks up by id.
///
/// Ordered maps, not hashed ones: iteration order must never leak into
/// behaviour (see the crate's determinism note).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BattleData {
    enemies: BTreeMap<u16, EnemyRecord>,
    items: BTreeMap<u8, ItemRecord>,
    level_tables: BTreeMap<u8, LevelTable>,
}

impl BattleData {
    /// An empty set, to be filled with the `with_*` builders.
    #[must_use]
    pub fn new() -> BattleData {
        BattleData::default()
    }

    /// Adds enemy records, keyed by id.
    #[must_use]
    pub fn with_enemies(mut self, records: impl IntoIterator<Item = EnemyRecord>) -> BattleData {
        for record in records {
            self.enemies.insert(record.id, record);
        }
        self
    }

    /// Adds item records, keyed by id.
    #[must_use]
    pub fn with_items(mut self, records: impl IntoIterator<Item = ItemRecord>) -> BattleData {
        for record in records {
            self.items.insert(record.id, record);
        }
        self
    }

    /// Adds a character's level table.
    #[must_use]
    pub fn with_level_table(mut self, character: u8, table: LevelTable) -> BattleData {
        self.level_tables.insert(character, table);
        self
    }

    /// Looks up an enemy.
    ///
    /// # Errors
    /// [`BattleDataError::UnknownEnemy`] when nothing is registered under `id`.
    pub fn enemy(&self, id: u16) -> Result<&EnemyRecord, BattleDataError> {
        self.enemies
            .get(&id)
            .ok_or(BattleDataError::UnknownEnemy(id))
    }

    /// Looks up an item.
    ///
    /// # Errors
    /// [`BattleDataError::UnknownItem`] when nothing is registered under `id`.
    pub fn item(&self, id: u8) -> Result<&ItemRecord, BattleDataError> {
        self.items.get(&id).ok_or(BattleDataError::UnknownItem(id))
    }

    /// Looks up a level table.
    ///
    /// # Errors
    /// [`BattleDataError::UnknownLevelTable`] when the character has none.
    pub fn level_table(&self, character: u8) -> Result<&LevelTable, BattleDataError> {
        self.level_tables
            .get(&character)
            .ok_or(BattleDataError::UnknownLevelTable(character))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_weapon_types_are_the_branch_battle_attack_command_takes() {
        // ps4.asm:2244-2251: == 2 or == 4 is multi-target, < 4 otherwise is
        // single, > 4 is not a weapon.
        for byte in 1..=4u8 {
            let kind = ItemKind::from_byte(byte).expect("a weapon type");
            assert!(kind.is_weapon(), "type {byte}");
            assert_eq!(
                kind.is_multi_target(),
                byte == 2 || byte == 4,
                "type {byte}"
            );
        }
        for byte in 5..=10u8 {
            let kind = ItemKind::from_byte(byte).expect("a known type");
            assert!(!kind.is_weapon(), "type {byte}");
        }
        assert_eq!(ItemKind::from_byte(0), None);
        assert_eq!(ItemKind::from_byte(11), None);
    }

    #[test]
    fn alys_carries_a_multi_target_weapon() {
        // The oracle saw Alys hit both enemies in one frame and called her
        // weapon a Slasher. Her starting record equips item 3, the Boomerang,
        // which is the same type-2 one-handed multi-target class.
        assert!(ItemKind::from_byte(2).expect("Boomerang").is_multi_target());
    }

    #[test]
    fn an_unrunnable_formation_says_so() {
        let mut formation = FormationRecord {
            id: 0,
            ambush_chance: 0x10,
            run_chance: 0,
            drop_rate: 8,
            drop_item: Some(0x80),
            enemies: Vec::new(),
        };
        assert!(formation.can_run());
        formation.run_chance = UNRUNNABLE - 1;
        assert!(formation.can_run());
        formation.run_chance = UNRUNNABLE;
        assert!(!formation.can_run(), "$F0 and up cannot be escaped");
        formation.run_chance = 0xFF;
        assert!(!formation.can_run());
    }

    #[test]
    fn a_level_table_indexes_from_its_own_starting_level() {
        // Alys starts at 7, so her table's first record is level 8 and a
        // level-7 Alys indexes it at 0.
        let table = LevelTable {
            starting_level: 7,
            levels: vec![
                LevelRecord {
                    level: 8,
                    experience_required: 100,
                    hp: 60,
                    tp: 30,
                    strength: 13,
                    mental: 13,
                    agility: 16,
                    dexterity: 14,
                },
                LevelRecord {
                    level: 9,
                    experience_required: 250,
                    hp: 66,
                    tp: 33,
                    strength: 14,
                    mental: 14,
                    agility: 17,
                    dexterity: 15,
                },
            ],
        };
        assert_eq!(table.next_after(7).map(|r| r.level), Some(8));
        assert_eq!(table.next_after(8).map(|r| r.level), Some(9));
        assert_eq!(table.next_after(9), None, "past the end of the table");
        assert_eq!(table.next_after(6), None, "below the starting level");
    }

    #[test]
    fn lookups_name_what_is_missing() {
        let data = BattleData::new();
        assert_eq!(data.enemy(10), Err(BattleDataError::UnknownEnemy(10)));
        assert_eq!(data.item(3), Err(BattleDataError::UnknownItem(3)));
        assert_eq!(
            data.level_table(1),
            Err(BattleDataError::UnknownLevelTable(1))
        );
    }
}
