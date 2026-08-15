//! Random encounters and the battle data bridge.
//!
//! Three things live here, all transcribed from `docs/BATTLE_SCOUT.md` §9:
//!
//! - [`battle_data`] converts the pack's `battle/` files into the engine's
//!   in-memory [`BattleData`].
//! - [`EncounterTable`] answers "which formation fights here": per-map
//!   bindings, the 32-entry groups, and the overworld position grids.
//! - [`EncounterClock`] is `RunRandomBattles` (`$05784E`): ten free steps
//!   after a map load or a battle, then a 1-in-32 roll on every step.
//!
//! The encounter roll calls `UpdateRNGSeed` and reads the seed word — one
//! extra draw from the shared stream per rolling step, on top of the two
//! per-frame ticks. Interior maps whose binding mode is `none` bail before
//! the roll, which is why the oracle's tape-02 walk (Piata Academy, no
//! encounters) shows no extra calls while walking.

use std::collections::BTreeMap;

use psiv_core::battle::Bonuses;
use psiv_core::battle::{
    BattleData, CharacterRecord, ELEMENT_SLOTS, EnemyRecord, FormationEnemy, FormationRecord,
    ItemKind, ItemRecord, LevelRecord, LevelTable, Rolls,
};
use psiv_core::{Cell, CollisionType, FieldMap};
use psiv_data::BattleFiles;

use crate::BridgeError;

/// Steps granted after a map load or a battle before rolls begin.
///
/// `$FFFFECE4` is reset to 10 at both sites; once it reaches zero it is
/// re-armed to 1, so every later step rolls.
pub const GRACE_STEPS: u8 = 10;

/// The on-foot roll mask: `& $1F`, battle on zero — 1 in 32.
///
/// Vehicles mask `$7F` (1 in 128); Tier 3.
pub const FOOT_MASK: u16 = 0x1F;

/// The formation pick inside a group: `UpdateRNGSeed2 & $1F`, 32 entries.
pub const GROUP_MASK: u16 = 0x1F;

/// Converts the pack's battle files into the engine's [`BattleData`].
///
/// Items are absent until the pack emits `battle/equipment.json`; battles can
/// run (an empty equipment list is a bare-handed party), but seating the real
/// party waits on that file.
///
/// # Errors
/// [`BridgeError::Rejected`] when a record does not fit the engine's shape —
/// a property list that is not the 14 slots, an AI list of the wrong arity.
pub fn battle_data(files: &BattleFiles) -> Result<BattleData, BridgeError> {
    let mut enemies = Vec::with_capacity(files.enemies.enemies.len());
    for enemy in &files.enemies.enemies {
        enemies.push(enemy_record(&files.enemies.properties, enemy)?);
    }

    // Equipment: every record whose type byte names an equippable kind.
    // Unequippable kinds (plot items, consumable-only types) never appear in
    // an equipment slot; if a character record somehow names one anyway,
    // PartyMember::seat's own lookup fails closed and loudly.
    let items: Vec<ItemRecord> = files
        .equipment
        .items
        .iter()
        .filter_map(|item| {
            let kind = ItemKind::from_byte(item.kind.id)?;
            Some(ItemRecord {
                id: item.id,
                name: item
                    .display_name
                    .clone()
                    .unwrap_or_else(|| item.symbol.clone()),
                kind,
                bonuses: Bonuses {
                    strength: item.bonuses.strength,
                    mental: item.bonuses.mental,
                    agility: item.bonuses.agility,
                    dexterity: item.bonuses.dexterity,
                    attack: item.bonuses.attack,
                    defence: item.bonuses.defense,
                    mental_defence: item.bonuses.magic_defense,
                },
                element: item.element.id,
            })
        })
        .collect();
    let mut data = BattleData::new().with_enemies(enemies).with_items(items);
    for table in &files.levels.characters {
        data = data.with_level_table(table.character_id, level_table(table));
    }
    Ok(data)
}

fn enemy_record(
    property_order: &[String],
    enemy: &psiv_data::Enemy,
) -> Result<EnemyRecord, BridgeError> {
    let mut properties = [0u8; ELEMENT_SLOTS];
    if property_order.len() != ELEMENT_SLOTS {
        return Err(BridgeError::Rejected(format!(
            "enemy file declares {} element properties, engine wants {ELEMENT_SLOTS}",
            property_order.len()
        )));
    }
    for (slot, name) in property_order.iter().enumerate() {
        let property = enemy.properties.get(name).ok_or_else(|| {
            BridgeError::Rejected(format!("enemy {} missing property {name}", enemy.id))
        })?;
        properties[slot] = property.value;
    }

    Ok(EnemyRecord {
        id: enemy.id,
        name: enemy
            .display_name
            .clone()
            .unwrap_or_else(|| enemy.symbol.clone()),
        hp: enemy.hp,
        strength: enemy.stats.strength,
        mental: enemy.stats.mental,
        agility: enemy.stats.agility,
        dexterity: enemy.stats.dexterity,
        attack: enemy.stats.attack,
        defence: enemy.stats.defense,
        mental_defence: enemy.stats.magic_defense,
        attack_element: u8::try_from(enemy.attack.element.id)
            .map_err(|_| BridgeError::Rejected(format!("enemy {} element id", enemy.id)))?,
        attack_status: u8::try_from(enemy.attack.status_effect.id)
            .map_err(|_| BridgeError::Rejected(format!("enemy {} status id", enemy.id)))?,
        properties,
        regular_abilities: fixed(&enemy.ai.regular_ability_ids, enemy.id, "regular abilities")?,
        condition_ids: fixed(&enemy.ai.condition_ids, enemy.id, "condition ids")?,
        conditional_abilities: fixed(
            &enemy.ai.conditional_ability_ids,
            enemy.id,
            "conditional abilities",
        )?,
        experience: enemy.rewards.experience,
        meseta: enemy.rewards.meseta,
    })
}

fn fixed<const N: usize>(list: &[u8], enemy: u16, what: &str) -> Result<[u8; N], BridgeError> {
    <[u8; N]>::try_from(list).map_err(|_| {
        BridgeError::Rejected(format!(
            "enemy {enemy}: {what} has {} entries, engine wants {N}",
            list.len()
        ))
    })
}

fn level_table(table: &psiv_data::LevelTable) -> LevelTable {
    LevelTable {
        starting_level: table.starting_level,
        levels: table
            .levels
            .iter()
            .map(|level| LevelRecord {
                level: level.level,
                experience_required: level.experience_required,
                hp: level.hp,
                tp: level.tp,
                strength: level.stats.strength,
                mental: level.stats.mental,
                agility: level.stats.agility,
                dexterity: level.stats.dexterity,
            })
            .collect(),
    }
}

/// Converts one pack formation into the engine's record.
///
/// # Errors
/// [`BridgeError::Rejected`] for a formation with no id (boss formations are
/// keyed by `Event_Battle_Index` instead; pass those separately).
pub fn formation_record(formation: &psiv_data::Formation) -> Result<FormationRecord, BridgeError> {
    let id = formation
        .id
        .ok_or_else(|| BridgeError::Rejected("formation without an id".into()))?;
    Ok(FormationRecord {
        id,
        ambush_chance: formation.ambush_chance,
        run_chance: formation.run_chance,
        drop_rate: formation.drop_rate,
        drop_item: formation.drop_item,
        enemies: formation
            .enemies
            .iter()
            .map(|enemy| FormationEnemy {
                slot: enemy.slot,
                enemy_id: enemy.enemy_id,
                position: enemy.position,
            })
            .collect(),
    })
}

/// Which formation fights where: `Battle_SetupEnemyData` steps 2-5
/// (`ps4.asm:11813`), minus vehicles and bosses.
pub struct EncounterTable {
    /// map id -> encounter source.
    bindings: BTreeMap<u16, Source>,
    /// group -> the 32 formation ids an encounter roll picks between.
    groups: BTreeMap<u32, Vec<u16>>,
    /// grid name -> the overworld grid.
    grids: BTreeMap<String, Grid>,
    /// formation id -> engine record, converted once.
    formations: BTreeMap<u16, FormationRecord>,
}

enum Source {
    Group(u32),
    Grid(String),
}

struct Grid {
    columns: u32,
    cells: Vec<Vec<u8>>,
    fallback_group: u32,
}

impl EncounterTable {
    /// Builds the table from the pack's formations file.
    ///
    /// # Errors
    /// [`BridgeError::Rejected`] when a binding names a group or grid the
    /// file does not carry — the pack's own tests forbid it, so hitting this
    /// means a hand-edited pack.
    pub fn from_files(files: &BattleFiles) -> Result<EncounterTable, BridgeError> {
        let file = &files.formations;
        let mut groups = BTreeMap::new();
        if let Some(encounter_groups) = &file.encounter_groups {
            for group in &encounter_groups.groups {
                groups.insert(group.group, group.formation_ids.clone());
            }
        }
        let mut grids = BTreeMap::new();
        for grid in &file.position_grids {
            grids.insert(
                grid.name.clone(),
                Grid {
                    columns: grid.columns,
                    cells: grid.cells.clone(),
                    fallback_group: grid.fallback_group,
                },
            );
        }
        let mut bindings = BTreeMap::new();
        for binding in &file.map_bindings {
            let source = match binding.mode.as_str() {
                "group" => Source::Group(binding.group.ok_or_else(|| {
                    BridgeError::Rejected(format!(
                        "map {:#05x}: group mode, no group",
                        binding.map_id
                    ))
                })?),
                "position_grid" => {
                    Source::Grid(binding.position_grid.clone().ok_or_else(|| {
                        BridgeError::Rejected(format!(
                            "map {:#05x}: grid mode, no grid",
                            binding.map_id
                        ))
                    })?)
                }
                // `none` has no encounters; `outside_table` is the $1A0
                // anomaly, which retail also cannot fight on (battles are
                // disabled there) — both fall out of the map entirely.
                _ => continue,
            };
            bindings.insert(binding.map_id, source);
        }
        let mut formations = BTreeMap::new();
        for formation in &file.formations {
            let record = formation_record(formation)?;
            formations.insert(record.id, record);
        }
        Ok(EncounterTable {
            bindings,
            groups,
            grids,
            formations,
        })
    }

    /// Whether this map rolls encounters at all.
    #[must_use]
    pub fn enabled(&self, map: u16) -> bool {
        self.bindings.contains_key(&map)
    }

    /// A formation by id.
    #[must_use]
    pub fn formation(&self, id: u16) -> Option<&FormationRecord> {
        self.formations.get(&id)
    }

    /// Picks the formation for an encounter at `cell` on `map`.
    ///
    /// Draws exactly one roll (`UpdateRNGSeed2 & $1F`) for the group pick,
    /// after the grid lookup (which draws nothing). Returns `None` when the
    /// map has no encounters or the group id names no block — the latter is
    /// the pack's 416-byte-table anomaly class, surfaced not fought.
    pub fn select(&self, map: u16, cell: Cell, rolls: &mut impl Rolls) -> Option<&FormationRecord> {
        let source = self.bindings.get(&map)?;
        let group = match source {
            Source::Group(group) => *group,
            Source::Grid(name) => {
                let grid = self.grids.get(name)?;
                // `(character_y >> 6) * 64 + (character_x >> 6)` on world
                // pixels; our cells are 16px, so >> 2 on cells is the same
                // shift. A byte with the high bit set means the fallback.
                let gx = (u32::from(cell.x) >> 2).min(grid.columns.saturating_sub(1));
                let gy = u32::from(cell.y) >> 2;
                let byte = grid
                    .cells
                    .get(gy as usize)
                    .and_then(|row| row.get(gx as usize))
                    .copied()
                    .unwrap_or(0x80);
                if byte >= 0x80 {
                    grid.fallback_group
                } else {
                    u32::from(byte)
                }
            }
        };
        let block = self.groups.get(&group)?;
        let pick = usize::from(rolls.next_roll() & GROUP_MASK);
        let id = *block.get(pick)?;
        self.formations.get(&id)
    }
}

/// `RunRandomBattles`' step counter and gates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncounterClock {
    steps: u8,
}

impl Default for EncounterClock {
    fn default() -> EncounterClock {
        EncounterClock { steps: GRACE_STEPS }
    }
}

impl EncounterClock {
    /// A fresh clock: ten free steps.
    #[must_use]
    pub fn new() -> EncounterClock {
        EncounterClock::default()
    }

    /// Re-arms the grace period — map load and battle end both do.
    pub fn reset(&mut self) {
        self.steps = GRACE_STEPS;
    }

    /// One completed step on an encounter-enabled map. Returns whether the
    /// 1-in-32 roll is due this step.
    ///
    /// The cartridge decrements first and re-arms to 1 on reaching zero, so
    /// steps 1-9 are free, step 10 rolls, and every step after rolls.
    pub fn step(&mut self) -> bool {
        self.steps = self.steps.saturating_sub(1);
        if self.steps == 0 {
            self.steps = 1;
            return true;
        }
        false
    }

    /// The suppression gates that run before the counter is even touched:
    /// no battle standing on a map-change tile or a recovery floor, and none
    /// with a map-change tile in any of the eight surrounding cells (standing
    /// next to a town or dungeon entrance).
    #[must_use]
    pub fn suppressed(map: &FieldMap, cell: Cell) -> bool {
        let standing = map.collision_at(cell);
        if matches!(
            standing,
            Some(CollisionType::MapChange | CollisionType::Recovery)
        ) {
            return true;
        }
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let x = i32::from(cell.x) + dx;
                let y = i32::from(cell.y) + dy;
                let (Ok(x), Ok(y)) = (u16::try_from(x), u16::try_from(y)) else {
                    continue;
                };
                if map.collision_at(Cell::new(x, y)) == Some(CollisionType::MapChange) {
                    return true;
                }
            }
        }
        false
    }
}

/// Converts one pack character into the engine's `Character_Init` record.
///
/// `property_order` is the enemies file's 14-name list — the one canonical
/// element-slot ordering the whole pack shares.
///
/// # Errors
/// [`BridgeError::Rejected`] for a missing property or a wrong-arity list.
pub fn character_record(
    character: &psiv_data::Character,
    property_order: &[String],
) -> Result<CharacterRecord, BridgeError> {
    if property_order.len() != ELEMENT_SLOTS {
        return Err(BridgeError::Rejected(format!(
            "character file wants {ELEMENT_SLOTS} element properties, got {}",
            property_order.len()
        )));
    }
    let mut properties = [0u8; ELEMENT_SLOTS];
    for (slot, name) in property_order.iter().enumerate() {
        let property = character.properties.get(name).ok_or_else(|| {
            BridgeError::Rejected(format!(
                "character {} missing property {name}",
                character.character_id
            ))
        })?;
        properties[slot] = property.value;
    }
    Ok(CharacterRecord {
        id: character.character_id,
        name: character
            .display_name
            .clone()
            .unwrap_or_else(|| character.symbol.clone()),
        profession: character.profession.id,
        level: character.level,
        experience: character.experience,
        hp: character.hp,
        max_hp: character.max_hp,
        tp: character.tp,
        max_tp: character.max_tp,
        strength: character.stats.strength,
        mental: character.stats.mental,
        agility: character.stats.agility,
        dexterity: character.stats.dexterity,
        properties,
        equipment: character.equipment.item_ids(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use psiv_core::battle::SliceRolls;

    #[test]
    fn the_clock_gives_ten_free_steps_then_rolls_every_step() {
        let mut clock = EncounterClock::new();
        let due: Vec<bool> = (0..14).map(|_| clock.step()).collect();
        assert_eq!(&due[..9], &[false; 9], "nine silent decrements");
        assert!(due[9..].iter().all(|&d| d), "step ten and beyond all roll");
        clock.reset();
        assert!(!clock.step(), "reset re-arms the grace period");
    }

    #[test]
    fn grid_lookup_shifts_cells_by_two_and_falls_back_on_high_bytes() {
        let table = EncounterTable {
            bindings: BTreeMap::from([(0u16, Source::Grid("g".into()))]),
            groups: BTreeMap::from([(3u32, vec![7u16; 32]), (0u32, vec![9u16; 32])]),
            grids: BTreeMap::from([(
                "g".to_string(),
                Grid {
                    columns: 64,
                    cells: vec![vec![0x80; 64], vec![3; 64]],
                    fallback_group: 0,
                },
            )]),
            formations: BTreeMap::from([
                (
                    7u16,
                    FormationRecord {
                        id: 7,
                        ambush_chance: 0,
                        run_chance: 0,
                        drop_rate: 0,
                        drop_item: None,
                        enemies: Vec::new(),
                    },
                ),
                (
                    9u16,
                    FormationRecord {
                        id: 9,
                        ambush_chance: 0,
                        run_chance: 0,
                        drop_rate: 0,
                        drop_item: None,
                        enemies: Vec::new(),
                    },
                ),
            ]),
        };
        // Row 0 holds $80 bytes: the fallback group (0) -> formation 9.
        let mut rolls = SliceRolls::new(&[0]);
        let picked = table.select(0, Cell::new(10, 2), &mut rolls).unwrap();
        assert_eq!(picked.id, 9, "cell y=2 >> 2 = row 0, fallback");
        // Row 1 holds group 3 -> formation 7.
        let mut rolls = SliceRolls::new(&[31]);
        let picked = table.select(0, Cell::new(255, 7), &mut rolls).unwrap();
        assert_eq!(picked.id, 7, "cell y=7 >> 2 = row 1, group 3");
    }
}
