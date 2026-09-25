//! `battle/`: enemies, formations, level tables and ability records.
//!
//! Field names are `psiv_tools.battle_pack`'s. As in [`crate::Manifest`], this
//! models the parts the runtime consumes and lets the rest — census blocks,
//! ROM offsets, encounter position grids — pass by, so a new report added to
//! the pack never breaks a load.
//!
//! # The seating path
//!
//! [`CharactersFile`] and [`EquipmentFile`] together are everything the engine
//! needs to seat a party: the 66-byte `InitialCharStats` records, and the
//! inventory records whose type byte and seven stat bonuses the two derivation
//! passes read.
//!
//! [`Character::initialized`] is the pack's own conformance vector — what
//! `InitializeCharStats` (`$0044652`) leaves in RAM after running
//! `UpdateCharModStats` and `UpdateCharElems`. It is not input; it is the
//! answer the engine has to reproduce, and `psiv-core` pins all eleven.
//!
//! # Fail-closed
//!
//! Same discipline as the rest of the crate: a declared count that disagrees
//! with the list, a duplicate enemy id, or a formation naming an enemy that is
//! not in `enemies.json` all stop the load.
//!
//! An ability whose effect id runs past `AbilityEffectsOffs` is the one thing
//! that does **not**, because retail carries one — `BLACK WAVE`, effect `$2C`,
//! on the unreferenced enemy `Zio3` — and refusing the pack over it would
//! refuse every real pack. The ratified policy is to reject the *record*, and
//! [`AbilitiesFile::usable`] is where that happens: a consumer that iterates it
//! cannot hand the engine something the cartridge would have dispatched through
//! an unbounded `TRAP #2`. [`AbilitiesFile::rejected`] lists what was withheld.

use crate::error::DataError;
use crate::manifest::PACK_FORMAT_VERSION;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;

/// Where `battle/` sits inside a pack.
pub const BATTLE_DIRECTORY: &str = "battle";

/// How many element-resistance slots an enemy record carries.
pub const ELEMENT_SLOTS: usize = 14;

/// The seven `battle/` files, parsed and cross-checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BattleFiles {
    /// `battle/enemies.json`.
    pub enemies: EnemiesFile,
    /// `battle/formations.json`.
    pub formations: FormationsFile,
    /// `battle/levels.json`.
    pub levels: LevelsFile,
    /// `battle/abilities.json`.
    pub abilities: AbilitiesFile,
    /// `battle/characters.json`.
    pub characters: CharactersFile,
    /// `battle/equipment.json`.
    pub equipment: EquipmentFile,
    /// `battle/enemy_animations.json`.
    pub enemy_animations: EnemyAnimationsFile,
}

impl BattleFiles {
    /// Reads and validates every `battle/` file under `pack_dir`.
    ///
    /// # Errors
    /// [`DataError`] for an unreadable or malformed file, a format-version
    /// mismatch, or any of the cross-checks in [`BattleFiles::validate`].
    pub fn load(pack_dir: &Path) -> Result<BattleFiles, DataError> {
        let files = BattleFiles {
            enemies: read(pack_dir, "enemies.json")?,
            formations: read(pack_dir, "formations.json")?,
            levels: read(pack_dir, "levels.json")?,
            abilities: read(pack_dir, "abilities.json")?,
            characters: read(pack_dir, "characters.json")?,
            equipment: read(pack_dir, "equipment.json")?,
            enemy_animations: read(pack_dir, "enemy_animations.json")?,
        };
        files.validate(pack_dir)?;
        Ok(files)
    }

    /// Rejected abilities that a fielded enemy could actually roll.
    ///
    /// Returns `(enemy symbol, enemy skill id)` pairs. A rejected ability on an
    /// enemy no formation uses is harmless; one on an enemy the game puts in
    /// front of the player is a hole in the engine's coverage, and the caller
    /// should know which.
    ///
    /// Retail returns exactly one: **`Zio3` fields `BLACK WAVE`**, whose effect
    /// id `$2C` is one past `AbilityEffectsOffs`. Zio3 is boss formation
    /// `event_battle_index` 4 — the scripted Zio fight that sets
    /// `EventFlag_Zio` (`ps4.asm:154200`) — with 16383 HP, agility 255 and
    /// defence 255, so it is a battle the player is not meant to win. That is
    /// not the same as unreachable, and an earlier note in
    /// `docs/battle/BATTLE_SCOUT.md` §11 saying Zio3 appears in no formation is wrong:
    /// it searched the 504 normal formations and not the 27 boss ones.
    #[must_use]
    pub fn fielded_rejected_abilities(&self) -> Vec<(&str, u16)> {
        let rejected: BTreeSet<u16> = self
            .abilities
            .rejected()
            .filter(|record| record.kind == "enemy_skills")
            .map(|record| record.id)
            .collect();
        if rejected.is_empty() {
            return Vec::new();
        }
        let fielded: BTreeSet<u16> = self
            .formations
            .formations
            .iter()
            .chain(&self.formations.boss_formations)
            .flat_map(|formation| formation.enemies.iter().map(|slot| slot.enemy_id))
            .collect();
        let mut found = Vec::new();
        for enemy in self
            .enemies
            .enemies
            .iter()
            .filter(|e| fielded.contains(&e.id))
        {
            for id in enemy
                .ai
                .regular_ability_ids
                .iter()
                .chain(&enemy.ai.conditional_ability_ids)
            {
                let id = u16::from(*id);
                if rejected.contains(&id) && !found.contains(&(enemy.symbol.as_str(), id)) {
                    found.push((enemy.symbol.as_str(), id));
                }
            }
        }
        found
    }

    /// Every cross-check the four files owe each other.
    ///
    /// # Errors
    /// [`DataError::ManifestMismatch`] naming the file and the field that
    /// disagrees.
    pub fn validate(&self, pack_dir: &Path) -> Result<(), DataError> {
        let dir = pack_dir.join(BATTLE_DIRECTORY);

        let count_check = |file: &str, field: &'static str, declared: u32, found: usize| {
            if declared as usize == found {
                Ok(())
            } else {
                Err(DataError::ManifestMismatch {
                    path: dir.join(file),
                    field,
                    manifest: declared.to_string(),
                    record: found.to_string(),
                })
            }
        };
        count_check(
            "enemies.json",
            "count",
            self.enemies.count,
            self.enemies.enemies.len(),
        )?;
        count_check(
            "formations.json",
            "formation_count",
            self.formations.formation_count,
            self.formations.formations.len(),
        )?;
        count_check(
            "formations.json",
            "boss_formation_count",
            self.formations.boss_formation_count,
            self.formations.boss_formations.len(),
        )?;
        count_check(
            "levels.json",
            "character_count",
            self.levels.character_count,
            self.levels.characters.len(),
        )?;
        count_check(
            "enemy_animations.json",
            "count",
            self.enemy_animations.count,
            self.enemy_animations.animations.len(),
        )?;

        if self.enemy_animations.source.grand_cross != 0
            || self
                .enemy_animations
                .source
                .tables
                .iter()
                .any(|table| table.grand_cross != 0)
        {
            return Err(DataError::ManifestMismatch {
                path: dir.join("enemy_animations.json"),
                field: "grand_cross",
                manifest: "0".into(),
                record: "nonzero animation table provenance".into(),
            });
        }
        count_check(
            "characters.json",
            "count",
            self.characters.count,
            self.characters.characters.len(),
        )?;
        count_check(
            "equipment.json",
            "count",
            self.equipment.count,
            self.equipment.items.len(),
        )?;
        count_check(
            "levels.json",
            "total_records",
            self.levels.total_records,
            self.levels
                .characters
                .iter()
                .map(|c| c.levels.len())
                .sum::<usize>(),
        )?;

        let mut seen = BTreeSet::new();
        for enemy in &self.enemies.enemies {
            if !seen.insert(enemy.id) {
                return Err(DataError::ManifestMismatch {
                    path: dir.join("enemies.json"),
                    field: "id",
                    manifest: "unique".into(),
                    record: enemy.id.to_string(),
                });
            }
            if enemy.properties.len() != ELEMENT_SLOTS {
                return Err(DataError::ManifestMismatch {
                    path: dir.join("enemies.json"),
                    field: "properties",
                    manifest: ELEMENT_SLOTS.to_string(),
                    record: enemy.properties.len().to_string(),
                });
            }
        }

        let animation_ids: BTreeSet<u16> = self
            .enemy_animations
            .animations
            .iter()
            .map(|animation| animation.enemy_id)
            .collect();
        if animation_ids != seen {
            return Err(DataError::ManifestMismatch {
                path: dir.join("enemy_animations.json"),
                field: "enemy_id",
                manifest: format!("{} enemy ids", seen.len()),
                record: format!("{} animation ids", animation_ids.len()),
            });
        }
        for animation in &self.enemy_animations.animations {
            if !animation
                .sfx_writes
                .iter()
                .any(|write| write.dispatch == "Sound_Index" && write.sound_id == animation.sfx_id)
            {
                return Err(DataError::ManifestMismatch {
                    path: dir.join("enemy_animations.json"),
                    field: "sfx_id",
                    manifest: animation.sfx_id.to_string(),
                    record: animation.enemy_id.to_string(),
                });
            }
        }

        // A formation naming an enemy with no record would produce a fighter
        // with no stats, which is a crash waiting for a player to find it.
        for formation in self
            .formations
            .formations
            .iter()
            .chain(&self.formations.boss_formations)
        {
            for slot in &formation.enemies {
                if !seen.contains(&slot.enemy_id) {
                    return Err(DataError::ManifestMismatch {
                        path: dir.join("formations.json"),
                        field: "enemy_id",
                        manifest: formation.key(),
                        record: slot.enemy_id.to_string(),
                    });
                }
            }
        }

        // Every slot a character fills must name an item that exists, or the
        // seating pass would silently drop its bonuses and its element and the
        // derived stats would merely look plausible.
        let equipment: BTreeSet<u8> = self.equipment.items.iter().map(|item| item.id).collect();
        for character in &self.characters.characters {
            for (slot, filled) in character.equipment.slots() {
                let Some(filled) = filled else { continue };
                if !equipment.contains(&filled.item_id) {
                    return Err(DataError::ManifestMismatch {
                        path: dir.join("characters.json"),
                        field: "item_id",
                        manifest: format!("{} {slot}", character.symbol),
                        record: filled.item_id.to_string(),
                    });
                }
            }
        }

        // Out-of-range effect ids are *not* a load failure: retail carries
        // one, so refusing the pack over it would refuse every real pack. The
        // ratified policy is to reject the record, which
        // [`AbilitiesFile::usable`] does by construction, and
        // [`BattleFiles::fielded_rejected_abilities`] reports the ones an
        // enemy could actually reach.
        Ok(())
    }
}

fn read<T: for<'de> Deserialize<'de> + Versioned>(
    pack_dir: &Path,
    name: &str,
) -> Result<T, DataError> {
    let path = pack_dir.join(BATTLE_DIRECTORY).join(name);
    let text = std::fs::read_to_string(&path).map_err(|e| DataError::io(&path, e))?;
    let value: T = serde_json::from_str(&text).map_err(|e| DataError::json(&path, e))?;
    if value.format_version() != PACK_FORMAT_VERSION {
        return Err(DataError::FormatVersion {
            found: value.format_version(),
            expected: PACK_FORMAT_VERSION,
        });
    }
    Ok(value)
}

/// A `battle/` file that declares which pack format wrote it.
trait Versioned {
    fn format_version(&self) -> u32;
}

macro_rules! versioned {
    ($($ty:ty),+ $(,)?) => {
        $(impl Versioned for $ty {
            fn format_version(&self) -> u32 {
                self.format_version
            }
        })+
    };
}
versioned!(
    EnemiesFile,
    FormationsFile,
    LevelsFile,
    AbilitiesFile,
    CharactersFile,
    EquipmentFile,
    EnemyAnimationsFile
);

// ---------------------------------------------------------------------------
// Shared
// ---------------------------------------------------------------------------

/// One element resistance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Property {
    /// The raw byte, 0..=4.
    pub value: u8,
}

/// An id with the extractor's name beside it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedId {
    /// The raw id.
    pub id: u16,
    /// What it is called, when it is called anything.
    #[serde(default, alias = "display_name")]
    pub name: Option<String>,
}

mod abilities;
mod animations;
mod enemies;
mod formations;
mod levels;
mod party;

pub use abilities::{AbilitiesFile, Ability, EffectTable};
pub use animations::{
    EnemyAnimation, EnemyAnimationCensus, EnemyAnimationDispatch, EnemyAnimationEvidence,
    EnemyAnimationFrameSequence, EnemyAnimationSfxWrite, EnemyAnimationSource,
    EnemyAnimationTableSource, EnemyAnimationsFile,
};
pub use enemies::{EnemiesFile, Enemy, EnemyAi, EnemyAttack, EnemyStats, Rewards};
pub use formations::{
    EncounterGroup, EncounterGroups, Formation, FormationEnemy, FormationsFile, MapBinding,
    NamedMapRef, PositionGrid,
};
pub use levels::{Level, LevelStats, LevelTable, LevelsFile};
pub use party::{
    Character, CharactersFile, DerivedStat, ElementRef, Equipment, EquipmentBonuses, EquipmentFile,
    EquipmentKindRef, EquipmentType, Equipped, Initialized, Loadout, SkillSlots, StatusPortrait,
    TechniqueSlots, WeaponElements,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::PACK_FORMAT_VERSION;

    fn ability(kind: &str, id: u16, effect_id: u8, out_of_range: bool) -> Ability {
        Ability {
            id,
            kind: kind.into(),
            symbol: Some("BlackWave3".into()),
            name: None,
            display_name: Some("BLACK WAVE".into()),
            effect_id,
            tp_cost: None,
            targeting: None,
            target_id: None,
            parameter_2: None,
            battle_object_or_graphic_id: None,
            relevant_stat: None,
            requires_weapon: None,
            power_or_hit_chance: Some(1),
            targeting_or_parameter_3: None,
            resistance_stat: None,
            element: None,
            effect_out_of_range: out_of_range,
        }
    }

    fn abilities(records: Vec<Ability>) -> AbilitiesFile {
        AbilitiesFile {
            format_version: PACK_FORMAT_VERSION,
            effects: EffectTable {
                table: "0x0061BE".into(),
                count: 44,
                bounds_checked: false,
            },
            techniques: Vec::new(),
            skills: Vec::new(),
            enemy_skills: records,
            item_effects: Vec::new(),
        }
    }

    #[test]
    fn an_out_of_range_effect_is_withheld_rather_than_refused() {
        let clean = abilities(vec![ability("enemy_skills", 2, 1, false)]);
        assert_eq!(clean.rejected().count(), 0);
        assert_eq!(clean.usable().count(), 1);

        let dirty = abilities(vec![
            ability("enemy_skills", 2, 1, false),
            ability("enemy_skills", 112, 0x2C, true),
        ]);
        let offenders: Vec<u16> = dirty.rejected().map(|a| a.id).collect();
        assert_eq!(offenders, vec![112], "BLACK WAVE, and nothing else");
        let usable: Vec<u16> = dirty.usable().map(|a| a.id).collect();
        assert_eq!(usable, vec![2], "and it never reaches a consumer");
        assert_eq!(dirty.all().count(), 2, "but the file still holds both");
    }

    #[test]
    fn a_boss_formation_is_keyed_by_its_event_index() {
        let boss = r#"{
            "ambush_chance": 0, "run_chance": 254, "can_run": false,
            "drop_rate": 0, "drop_item": null, "count": 3,
            "count_matches_entries": true, "event_battle_index": 0,
            "enemies": [{"slot": 1, "enemy_id": 60, "position": 14}]
        }"#;
        let formation: Formation = serde_json::from_str(boss).expect("a boss formation");
        assert_eq!(formation.id, None, "boss records carry no formation id");
        assert_eq!(formation.event_battle_index, Some(0));
        assert_eq!(formation.key(), "boss formation 0");
        assert!(!formation.can_run, "run chance $FE is above $F0");
    }

    #[test]
    fn an_error_names_the_record_rather_than_its_index() {
        let record = ability("enemy_skills", 112, 0x2C, true);
        assert_eq!(record.identity(), "enemy_skills 112 (BLACK WAVE)");

        // Falling back through symbol to name to a placeholder.
        let mut bare = record.clone();
        bare.display_name = None;
        assert_eq!(bare.identity(), "enemy_skills 112 (BlackWave3)");
        bare.symbol = None;
        bare.name = Some("Whatever".into());
        assert_eq!(bare.identity(), "enemy_skills 112 (Whatever)");
        bare.name = None;
        assert_eq!(bare.identity(), "enemy_skills 112 (?)");
    }

    #[test]
    fn an_enemy_record_round_trips_through_serde() {
        // The shapes are what `psiv_tools.battle_pack.build_enemies` emits, so
        // a drift on either side shows up here rather than at load time.
        let json = r#"{
            "id": 10,
            "symbol": "ZoranBult",
            "display_name": "ZORAN-BULT",
            "rom_offset": "0x281A1C",
            "hp": 25,
            "stats": {"strength": 18, "mental": 4, "agility": 6, "dexterity": 8,
                      "attack": 16, "defense": 2, "magic_defense": 0},
            "attack": {"element": {"id": 1, "name": "physical"},
                       "status_effect": {"id": 0, "name": "none"}},
            "properties": {"physical": {"value": 2, "meaning": "normal"}},
            "ai": {"condition_ids": [0, 0, 0, 0],
                   "conditional_ability_ids": [0, 0, 0, 0],
                   "regular_ability_ids": [0, 0, 0, 0, 0, 0, 0, 0]},
            "rewards": {"experience": 12, "meseta": 3}
        }"#;
        let enemy: Enemy = serde_json::from_str(json).expect("the pack's shape");
        assert_eq!(enemy.id, 10);
        assert_eq!(enemy.hp, 25);
        assert_eq!(enemy.stats.attack, 16);
        assert_eq!(enemy.stats.defense, 2);
        assert_eq!(enemy.attack.element.id, 1);
        assert_eq!(enemy.properties["physical"].value, 2);
        assert_eq!(enemy.ai.regular_ability_ids.len(), 8);
        assert_eq!(enemy.rewards.experience, 12);
        // `rom_offset` and `meaning` are not modelled and must pass by.
    }

    #[test]
    fn a_formation_record_round_trips_through_serde() {
        let json = r#"{
            "id": 0,
            "block": "block_1",
            "index_in_block": 0,
            "ambush_chance": 16,
            "run_chance": 0,
            "can_run": true,
            "drop_rate": 8,
            "drop_item": 128,
            "count": 2,
            "count_matches_entries": true,
            "group_1_mask": "0x03",
            "group_2_mask": "0x00",
            "group_1_slots": [1, 2],
            "group_2_slots": [],
            "enemies": [
                {"slot": 1, "enemy_id": 1, "position": 14, "groups": [1]},
                {"slot": 2, "enemy_id": 1, "position": 26, "groups": [1]}
            ]
        }"#;
        let formation: Formation = serde_json::from_str(json).expect("the pack's shape");
        assert_eq!(formation.ambush_chance, 0x10);
        assert!(formation.can_run);
        assert_eq!(formation.drop_item, Some(128));
        assert_eq!(formation.enemies.len(), 2);
        assert_eq!(formation.enemies[0].enemy_id, 1);
        assert!(formation.count_matches_entries);

        // A formation with no drop leaves the field null rather than absent.
        let none = json.replace("\"drop_item\": 128", "\"drop_item\": null");
        let formation: Formation = serde_json::from_str(&none).expect("null is a drop of nothing");
        assert_eq!(formation.drop_item, None);
    }

    #[test]
    fn a_level_record_round_trips_through_serde() {
        let json = r#"{
            "level": 2,
            "experience_required": 21,
            "hp": 31,
            "tp": 13,
            "stats": {"strength": 9, "mental": 7, "agility": 8, "dexterity": 6},
            "new_technique": null,
            "new_skill": null,
            "skill_uses": [4, 0, 0, 0, 0, 0, 0, 0]
        }"#;
        let level: Level = serde_json::from_str(json).expect("the pack's shape");
        assert_eq!(level.level, 2);
        assert_eq!(level.experience_required, 21);
        assert_eq!(level.stats.strength, 9);
        assert_eq!(level.new_technique, None);
    }

    #[test]
    fn the_three_ability_vocabularies_all_parse() {
        // A technique carries a TP cost where a skill carries a stat selector,
        // so both spellings have to survive the same struct.
        let technique = r#"{
            "id": 1, "kind": "techniques", "name": "Foi", "display_name": "FOI",
            "tp_cost": 3, "effect_id": 1, "power_or_hit_chance": 24,
            "resistance_stat": {"id": 7, "name": "magic_defense"},
            "element": {"id": 3, "name": "fire"}, "effect_out_of_range": false
        }"#;
        let record: Ability = serde_json::from_str(technique).expect("a technique");
        assert_eq!(record.tp_cost, Some(3));
        assert_eq!(record.relevant_stat, None);
        assert_eq!(record.element.as_ref().map(|e| e.id), Some(3));

        let skill = r#"{
            "id": 1, "kind": "skills", "name": "Crosscut", "display_name": "CROSSCUT",
            "relevant_stat": {"id": 5, "name": "attack"}, "requires_weapon": true,
            "effect_id": 1, "power_or_hit_chance": 80,
            "resistance_stat": {"id": 6, "name": "defense"},
            "element": {"id": 16, "name": null}, "effect_out_of_range": false
        }"#;
        let record: Ability = serde_json::from_str(skill).expect("a skill");
        assert_eq!(record.tp_cost, None);
        assert_eq!(record.relevant_stat.as_ref().map(|s| s.id), Some(5));
        assert_eq!(record.requires_weapon, Some(true));
        assert_eq!(
            record.element.as_ref().map(|e| e.id),
            Some(16),
            "$10 and up defer to the weapon"
        );

        let item = r#"{
            "id": 2, "kind": "item_effects", "symbol": "HuntKnife",
            "display_name": "HUNT-KNIFE", "effect_id": 0, "power_or_hit_chance": 0,
            "resistance_stat": {"id": 0, "name": "none"},
            "element": {"id": 0, "name": "none"}, "effect_out_of_range": false
        }"#;
        let record: Ability = serde_json::from_str(item).expect("an item effect");
        assert_eq!(record.effect_id, 0, "a plain weapon has no effect at all");
    }

    #[test]
    fn the_element_slot_count_is_the_record_layout() {
        // `$30`..`$4B`, fourteen words.
        assert_eq!(ELEMENT_SLOTS, 14);
    }
}
