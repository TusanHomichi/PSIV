//! `battle/`: enemies, formations, level tables and ability records.
//!
//! Field names are `psiv_tools.battle_pack`'s. As in [`crate::Manifest`], this
//! models the parts the runtime consumes and lets the rest — census blocks,
//! ROM offsets, encounter position grids — pass by, so a new report added to
//! the pack never breaks a load.
//!
//! # Two records the pack does not carry yet
//!
//! `psiv-core`'s battle engine needs two things `battle/` does not emit:
//!
//! - **Inventory records.** `battle/abilities.json` carries an item's embedded
//!   eight-byte *effect* record but not its type byte (`$A`) or its seven stat
//!   bonuses (`$B`..`$11`). Without the type byte nothing can tell a
//!   single-target weapon from a multi-target one, which is what decides
//!   whether Alys's Boomerang hits one enemy or all of them; without the
//!   bonuses `UpdateCharModStats` cannot derive attack, defence or mental
//!   defence at all.
//! - **Initial character records.** `game_start.json` names the party but not
//!   the 66-byte `Character_Init` records — base stats, element resistances,
//!   starting equipment ids.
//!
//! Until both land, the bridge has to build the item and character halves of
//! `psiv_core::battle::BattleData` from somewhere else. Filed rather than
//! guessed at: inventing a schema here would just have to be undone.
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

/// The four `battle/` files, parsed and cross-checked.
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
        };
        files.validate(pack_dir)?;
        Ok(files)
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

        // Out-of-range effect ids are *not* a load failure: retail carries
        // one, so refusing the pack over it would refuse every real pack. The
        // ratified policy is to reject the record, which
        // [`AbilitiesFile::usable`] does by construction.
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
versioned!(EnemiesFile, FormationsFile, LevelsFile, AbilitiesFile);

// ---------------------------------------------------------------------------
// enemies.json
// ---------------------------------------------------------------------------

/// `battle/enemies.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnemiesFile {
    /// The pack format that wrote this.
    pub format_version: u32,
    /// How many records the file declares.
    pub count: u32,
    /// The element-property slot names, in record order.
    #[serde(default)]
    pub properties: Vec<String>,
    /// The records, in id order.
    pub enemies: Vec<Enemy>,
}

/// One 48-byte `EnemyData` record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Enemy {
    /// Index into `EnemyData`.
    pub id: u16,
    /// The disassembly's identifier, which disambiguates duplicate display
    /// names.
    pub symbol: String,
    /// The cartridge's own name, when it has one.
    #[serde(default)]
    pub display_name: Option<String>,
    /// Starting and maximum HP.
    pub hp: u16,
    /// The seven combat stats.
    pub stats: EnemyStats,
    /// What a plain attack from this enemy carries and inflicts.
    pub attack: EnemyAttack,
    /// Resistance to each element, keyed by slot name. Values are 0 (immune)
    /// through 4 (very weak); the damage pipeline multiplies by this and
    /// divides by four.
    pub properties: std::collections::BTreeMap<String, Property>,
    /// The AI's ability lists.
    pub ai: EnemyAi,
    /// What it leaves behind.
    pub rewards: Rewards,
}

/// An enemy's combat stats. `defense` and `magic_defense` keep the pack's
/// spelling.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnemyStats {
    pub strength: u8,
    pub mental: u8,
    pub agility: u8,
    pub dexterity: u8,
    pub attack: u16,
    pub defense: u16,
    pub magic_defense: u16,
}

/// The two bytes `Battle_SetupEnemyData` reuses the TP slots for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnemyAttack {
    /// What element a plain attack carries.
    pub element: NamedId,
    /// What status it inflicts, `0` for none.
    pub status_effect: NamedId,
}

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
    #[serde(default)]
    pub name: Option<String>,
}

/// `$50`..`$5F` of the in-battle stats struct.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnemyAi {
    /// Four condition ids, `0` terminating.
    pub condition_ids: Vec<u8>,
    /// The ability each condition substitutes when it fires.
    pub conditional_ability_ids: Vec<u8>,
    /// The eight the per-turn roll picks between.
    pub regular_ability_ids: Vec<u8>,
}

/// What an enemy contributes to the victory pools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rewards {
    /// Added to `$FFFF41CE`.
    pub experience: u16,
    /// Added to `$FFFF41D0`.
    pub meseta: u16,
}

// ---------------------------------------------------------------------------
// formations.json
// ---------------------------------------------------------------------------

/// `battle/formations.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormationsFile {
    /// The pack format that wrote this.
    pub format_version: u32,
    /// How many normal formations the file declares.
    pub formation_count: u32,
    /// How many boss formations it declares.
    pub boss_formation_count: u32,
    /// The normal formations, in id order.
    pub formations: Vec<Formation>,
    /// The boss formations, indexed by `Event_Battle_Index`.
    pub boss_formations: Vec<Formation>,
    /// The 32-entry groups an encounter roll indexes.
    #[serde(default)]
    pub encounter_groups: Option<EncounterGroups>,
}

/// One `$FF`-terminated formation record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Formation {
    /// Its index across the four normal data blocks.
    ///
    /// Absent on a boss formation, which is reached through
    /// [`Formation::event_battle_index`] instead — the two live in different
    /// index spaces because `Battle_SetupEnemyData` reaches them by different
    /// routes (`ps4.asm:11813`).
    #[serde(default)]
    pub id: Option<u16>,
    /// `Event_Battle_Index`, on a boss formation.
    #[serde(default)]
    pub event_battle_index: Option<u16>,
    /// Byte 0, compared against the party's highest agility.
    pub ambush_chance: u8,
    /// Byte 1, the same comparison for escape.
    pub run_chance: u8,
    /// Whether byte 1 is below `$F0`.
    pub can_run: bool,
    /// Byte 2, out of 128.
    pub drop_rate: u8,
    /// Byte 3, or `None` when the formation drops nothing.
    #[serde(default)]
    pub drop_item: Option<u8>,
    /// Byte 4. Formation `$177` declares four and lists three — the
    /// cartridge's own inconsistency, which is why
    /// [`Formation::count_matches_entries`] exists.
    pub count: u8,
    /// Whether byte 4 agrees with the number of entries.
    pub count_matches_entries: bool,
    /// The enemies, in slot order.
    pub enemies: Vec<FormationEnemy>,
}

impl Formation {
    /// How this formation is identified, for a message.
    #[must_use]
    pub fn key(&self) -> String {
        match (self.id, self.event_battle_index) {
            (Some(id), _) => format!("formation {id}"),
            (None, Some(index)) => format!("boss formation {index}"),
            (None, None) => "an unidentified formation".into(),
        }
    }
}

/// One enemy slot of a formation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormationEnemy {
    /// Which of the four enemy slots, one-based.
    pub slot: u8,
    /// Index into `EnemyData`.
    pub enemy_id: u16,
    /// The on-screen position byte. Presentation only.
    pub position: u8,
}

/// `Battle_FormationIndexes`, the tables a random encounter rolls into.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncounterGroups {
    /// How many groups there are.
    pub group_count: u32,
    /// How many formation ids each group holds — 32 in retail, which is why
    /// the encounter roll masks with `$1F`.
    pub entries_per_group: u32,
    /// The groups, in index order.
    pub groups: Vec<EncounterGroup>,
}

/// One encounter group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncounterGroup {
    /// Its index, as `Battle_EnemyFormationIndexes` stores it per map.
    pub group: u32,
    /// The formation ids it can roll.
    pub formation_ids: Vec<u16>,
}

// ---------------------------------------------------------------------------
// levels.json
// ---------------------------------------------------------------------------

/// `battle/levels.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelsFile {
    /// The pack format that wrote this.
    pub format_version: u32,
    /// How many characters have tables.
    pub character_count: u32,
    /// How many level records there are in total.
    pub total_records: u32,
    /// One table per character, in `CharLevelTablePtrs` order.
    pub characters: Vec<LevelTable>,
}

/// One character's level table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelTable {
    /// Index into `Character_Stats`.
    pub character_id: u8,
    /// The character's name.
    pub character: String,
    /// The `[starting level .w]` half of the pointer-table entry, subtracted
    /// from the current level before indexing [`LevelTable::levels`].
    pub starting_level: u16,
    /// The records, in level order.
    pub levels: Vec<Level>,
}

/// One 22-byte level record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Level {
    /// The level this record grants.
    pub level: u16,
    /// Total experience needed to reach it.
    pub experience_required: u32,
    /// New maximum HP.
    pub hp: u16,
    /// New maximum TP.
    pub tp: u16,
    /// New base stats.
    pub stats: LevelStats,
    /// A technique learned at this level, if any. Tier 2.
    #[serde(default)]
    pub new_technique: Option<NamedId>,
    /// A skill learned at this level, if any. Tier 2.
    #[serde(default)]
    pub new_skill: Option<NamedId>,
}

/// The four base stats a level record sets.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelStats {
    pub strength: u8,
    pub mental: u8,
    pub agility: u8,
    pub dexterity: u8,
}

// ---------------------------------------------------------------------------
// abilities.json
// ---------------------------------------------------------------------------

/// `battle/abilities.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbilitiesFile {
    /// The pack format that wrote this.
    pub format_version: u32,
    /// What the dispatch table looks like.
    pub effects: EffectTable,
    /// Player techniques.
    pub techniques: Vec<Ability>,
    /// Player skills.
    pub skills: Vec<Ability>,
    /// Enemy skills.
    pub enemy_skills: Vec<Ability>,
    /// The eight-byte records embedded in inventory entries.
    pub item_effects: Vec<Ability>,
}

impl AbilitiesFile {
    /// Every record across all four kinds.
    pub fn all(&self) -> impl Iterator<Item = &Ability> {
        self.techniques
            .iter()
            .chain(&self.skills)
            .chain(&self.enemy_skills)
            .chain(&self.item_effects)
    }

    /// Records safe to hand to the engine.
    ///
    /// Excludes anything whose effect id runs past `AbilityEffectsOffs`, which
    /// the cartridge would dispatch through an unbounded `TRAP #2` into
    /// whatever follows the table. `docs/RUNTIME_DESIGN.md` "Battle bug policy"
    /// ratifies rejecting those rather than reproducing the jump, and this is
    /// where the rejection happens — a consumer that iterates this cannot pass
    /// one on by accident.
    pub fn usable(&self) -> impl Iterator<Item = &Ability> {
        self.all().filter(|record| !record.effect_out_of_range)
    }

    /// Records the engine must not be given, with [`Ability::identity`] ready
    /// for a log line.
    ///
    /// Retail holds exactly one — `BLACK WAVE`, effect `$2C`, carried only by
    /// the enemy `Zio3`, which appears in none of the 531 formations.
    pub fn rejected(&self) -> impl Iterator<Item = &Ability> {
        self.all().filter(|record| record.effect_out_of_range)
    }
}

/// `AbilityEffectsOffs` and the warning that goes with it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectTable {
    /// The table's retail address, as a hex string.
    pub table: String,
    /// How many entries it really has: 44.
    pub count: u32,
    /// Always false — `TRAP #2` dispatches with no bound.
    pub bounds_checked: bool,
}

/// One eight-byte ability record, whichever table it came from.
///
/// The four kinds share a layout but not a vocabulary: a technique's byte 1 is
/// a TP cost while a skill's is a stat selector, so both spellings appear and
/// only one is populated per record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ability {
    /// Index into its own table.
    pub id: u16,
    /// Which table: `techniques`, `skills`, `enemy_skills` or `item_effects`.
    pub kind: String,
    /// The disassembly's identifier, for the tables that carry one.
    #[serde(default)]
    pub symbol: Option<String>,
    /// The extractor's name, for the tables that carry one.
    #[serde(default)]
    pub name: Option<String>,
    /// The cartridge's own name.
    #[serde(default)]
    pub display_name: Option<String>,
    /// Byte 0, the index into `AbilityEffectsOffs`.
    pub effect_id: u8,
    /// Byte 1, for a technique.
    #[serde(default)]
    pub tp_cost: Option<u8>,
    /// Byte 1's low seven bits, for a skill or enemy skill: which of the
    /// actor's stats supplies the attack power.
    #[serde(default)]
    pub relevant_stat: Option<NamedId>,
    /// Byte 1's high bit, for a skill.
    #[serde(default)]
    pub requires_weapon: Option<bool>,
    /// Byte 3.
    #[serde(default)]
    pub power_or_hit_chance: Option<u16>,
    /// Byte 4: which of the target's stats resists.
    #[serde(default)]
    pub resistance_stat: Option<NamedId>,
    /// Byte 5: which element the target resists it with. `$10` and above mean
    /// "use the attacker's weapon element instead".
    #[serde(default)]
    pub element: Option<NamedId>,
    /// Whether [`Ability::effect_id`] runs past the dispatch table. Retail has
    /// exactly one: `BLACK WAVE`, used only by the unreferenced enemy `Zio3`.
    pub effect_out_of_range: bool,
}

impl Ability {
    /// The best name this record has, for an error message.
    #[must_use]
    pub fn identity(&self) -> String {
        let name = self
            .display_name
            .as_deref()
            .or(self.symbol.as_deref())
            .or(self.name.as_deref())
            .unwrap_or("?");
        format!("{} {} ({name})", self.kind, self.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ability(kind: &str, id: u16, effect_id: u8, out_of_range: bool) -> Ability {
        Ability {
            id,
            kind: kind.into(),
            symbol: Some("BlackWave3".into()),
            name: None,
            display_name: Some("BLACK WAVE".into()),
            effect_id,
            tp_cost: None,
            relevant_stat: None,
            requires_weapon: None,
            power_or_hit_chance: Some(1),
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
