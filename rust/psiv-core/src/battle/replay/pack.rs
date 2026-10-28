//! The pack records the captured battles need, beyond the hand-built fixtures.
//!
//! `crate::battle::fixtures::data` carries the three enemies the oracle tapes
//! happen to meet. Every other record a capture needs - the enemies a forced or
//! swept formation seats, the abilities they can roll, the inline formations
//! Fusion and COMBINE reload, and the party's equipment, techniques, skills and
//! battle items - is read from the local runtime pack's `battle/` files, the
//! records `psiv_runtime::battle_data` builds the game's own battles from. The
//! pack is built from the cartridge and never committed (#149), so without it
//! the replay tests skip, saying so, and the hosted check skips them by name
//! (`packTests` in `flake.nix`). The fixture supplies the *live* battle state
//! the RAM log shows, the pack supplies the records, and [`super::start`] is
//! where the two are held against each other.
//!
//! The three records below are transcribed for the forced captures' own tests:
//!
//! | capture | enemy | abilities | record |
//! |---|---|---|---|
//! | `$5E` two Helex | 0 HELEX | eight $02 FLAME BOLT | `battle/enemies.json` |
//! | `$37` two Fanbite | 15 FANBITE | six empty, two $08 SPIRAL BLD | same |
//! | `$53` one Desrt Leach | 81 DESRTLEACH | five empty, three $37 SAND STORM | same |

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Deserialize;

use crate::battle::fixtures;
use crate::battle::*;

/// The hand-built fixtures' data and the runtime pack's records, or `None`
/// (after saying why) when there is no pack to read.
///
/// The pack's records are added first so the three hand-transcribed enemies
/// below keep exactly the values their captures' tests were written against;
/// both describe the same cartridge records. Read once per test process.
pub(crate) fn data() -> Option<&'static BattleData> {
    static DATA: OnceLock<Option<BattleData>> = OnceLock::new();
    let data = DATA
        .get_or_init(|| {
            let dir = pack_dir();
            if !dir.join("battle/enemies.json").is_file() {
                return None;
            }
            let pack = read_pack(&dir);
            Some(
                fixtures::data()
                    .with_enemies(pack.enemies)
                    .with_enemy_skills(pack.enemy_skills)
                    .with_techniques(pack.techniques)
                    .with_skills(pack.skills)
                    .with_battle_items(pack.battle_items)
                    .with_items(pack.equipment)
                    .with_inline_formations(pack.inline_formations)
                    .with_enemies([helex(), fanbite(), desrt_leach()])
                    .with_enemy_skills([flame_bolt(), spiral_bld(), sand_storm()]),
            )
        })
        .as_ref();
    if data.is_none() {
        eprintln!(
            "skipping: no runtime pack at {} (build one with `python -m psiv_tools pack`, \
             or set PSIV_RUNTIME_PACK)",
            pack_dir().display()
        );
    }
    data
}

/// `PSIV_RUNTIME_PACK`, or the checkout's ignored `runtime-pack/`.
fn pack_dir() -> PathBuf {
    std::env::var_os("PSIV_RUNTIME_PACK").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime-pack"),
        PathBuf::from,
    )
}

fn read_json<T: serde::de::DeserializeOwned>(dir: &Path, name: &str) -> T {
    let path = dir.join("battle").join(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("{} does not parse: {error}", path.display()))
}

/// Every record of the four files, under the engine's field names.
fn read_pack(dir: &Path) -> Pack {
    let enemies: EnemiesJson = read_json(dir, "enemies.json");
    let abilities: AbilitiesJson = read_json(dir, "abilities.json");
    let equipment: EquipmentJson = read_json(dir, "equipment.json");
    let formations: FormationsJson = read_json(dir, "formations.json");
    let kinds: BTreeMap<u8, u8> = equipment
        .items
        .iter()
        .map(|item| (item.id, item.kind.id))
        .collect();
    Pack {
        enemies: enemies.enemies.into_iter().map(Into::into).collect(),
        enemy_skills: abilities.enemy_skills.into_iter().map(Into::into).collect(),
        techniques: abilities.techniques.into_iter().map(Into::into).collect(),
        skills: abilities.skills.into_iter().map(Into::into).collect(),
        battle_items: abilities
            .item_effects
            .into_iter()
            .map(|record| {
                let consumable = kinds.get(&record.id) == Some(&8);
                record.into_battle_item(consumable)
            })
            .collect(),
        equipment: equipment.items.into_iter().map(Into::into).collect(),
        inline_formations: formations
            .inline_formations
            .into_iter()
            .map(|j| InlineFormation {
                label: j.label,
                run_chance: j.run_chance,
                enemies: j
                    .enemies
                    .into_iter()
                    .map(|e| FormationEnemy {
                        slot: e.slot,
                        enemy_id: e.enemy_id,
                        position: e.position,
                    })
                    .collect(),
            })
            .collect(),
    }
}

struct Pack {
    enemies: Vec<EnemyRecord>,
    enemy_skills: Vec<EnemySkill>,
    techniques: Vec<Technique>,
    skills: Vec<Skill>,
    battle_items: Vec<BattleItem>,
    equipment: Vec<ItemRecord>,
    inline_formations: Vec<InlineFormation>,
}

/// The pack's `{"id": n, "name": ...}` references; only the id is read.
#[derive(Deserialize)]
struct IdRef {
    id: u8,
}

#[derive(Deserialize)]
struct FormationsJson {
    inline_formations: Vec<InlineJson>,
}

/// One record Fusion or COMBINE copies.
#[derive(Deserialize)]
struct InlineJson {
    label: String,
    run_chance: u8,
    enemies: Vec<InlineEnemyJson>,
}

#[derive(Deserialize)]
struct InlineEnemyJson {
    slot: u8,
    enemy_id: u16,
    position: u8,
}

#[derive(Deserialize)]
struct EquipmentJson {
    items: Vec<ItemJson>,
}

#[derive(Deserialize)]
struct ItemJson {
    id: u8,
    display_name: String,
    #[serde(rename = "type")]
    kind: IdRef,
    element: IdRef,
    bonuses: BonusesJson,
}

#[derive(Deserialize)]
struct BonusesJson {
    strength: i8,
    mental: i8,
    agility: i8,
    dexterity: i8,
    attack: i8,
    defense: i8,
    magic_defense: i8,
}

impl From<ItemJson> for ItemRecord {
    fn from(j: ItemJson) -> Self {
        Self {
            id: j.id,
            name: j.display_name,
            kind: ItemKind::from_byte(j.kind.id).expect("decoded equipment type"),
            element: j.element.id,
            bonuses: Bonuses {
                strength: j.bonuses.strength,
                mental: j.bonuses.mental,
                agility: j.bonuses.agility,
                dexterity: j.bonuses.dexterity,
                attack: j.bonuses.attack,
                defence: j.bonuses.defense,
                mental_defence: j.bonuses.magic_defense,
            },
        }
    }
}

#[derive(Deserialize)]
struct AbilitiesJson {
    enemy_skills: Vec<SkillJson>,
    techniques: Vec<TechniqueJson>,
    skills: Vec<PartySkillJson>,
    item_effects: Vec<ItemEffectJson>,
}

#[derive(Deserialize)]
struct TargetingJson {
    raw: u8,
}

#[derive(Deserialize)]
struct TechniqueJson {
    id: u8,
    display_name: String,
    effect_id: u8,
    targeting: TargetingJson,
    power_or_hit_chance: u8,
    resistance_stat: IdRef,
    element: IdRef,
    tp_cost: u8,
}

impl From<TechniqueJson> for Technique {
    fn from(j: TechniqueJson) -> Self {
        Self {
            id: j.id,
            name: j.display_name,
            effect: j.effect_id,
            cost: j.tp_cost,
            targeting: j.targeting.raw,
            power: j.power_or_hit_chance,
            resistance: j.resistance_stat.id,
            element: j.element.id,
        }
    }
}

#[derive(Deserialize)]
struct PartySkillJson {
    id: u8,
    display_name: String,
    effect_id: u8,
    relevant_stat: IdRef,
    requires_weapon: bool,
    targeting: TargetingJson,
    power_or_hit_chance: u8,
    resistance_stat: IdRef,
    element: IdRef,
}

impl From<PartySkillJson> for Skill {
    fn from(j: PartySkillJson) -> Self {
        Self {
            id: j.id,
            name: j.display_name,
            effect: j.effect_id,
            power_stat: j.relevant_stat.id,
            requires_weapon: j.requires_weapon,
            targeting: j.targeting.raw,
            power: j.power_or_hit_chance,
            resistance: j.resistance_stat.id,
            element: j.element.id,
        }
    }
}

#[derive(Deserialize)]
struct ItemEffectJson {
    id: u8,
    display_name: String,
    effect_id: u8,
    parameter_2: u8,
    targeting_or_parameter_3: u8,
    power_or_hit_chance: u8,
    resistance_stat: IdRef,
    element: IdRef,
    battle_object_or_graphic_id: u8,
}

impl ItemEffectJson {
    fn into_battle_item(self, consumable: bool) -> BattleItem {
        BattleItem {
            id: self.id,
            name: self.display_name,
            effect: self.effect_id,
            actor_power: self.parameter_2,
            targeting: self.targeting_or_parameter_3 & 15,
            power: self.power_or_hit_chance,
            resistance: self.resistance_stat.id,
            element: self.element.id,
            object: self.battle_object_or_graphic_id,
            consumable,
        }
    }
}

#[derive(Deserialize)]
struct EnemiesJson {
    enemies: Vec<EnemyJson>,
}

#[derive(Deserialize)]
struct EnemyJson {
    id: u16,
    symbol: String,
    hp: u16,
    stats: EnemyStatsJson,
    attack: EnemyAttackJson,
    properties: BTreeMap<String, PropertyJson>,
    ai: EnemyAiJson,
    rewards: RewardsJson,
}

#[derive(Deserialize)]
struct EnemyStatsJson {
    strength: u8,
    mental: u8,
    agility: u8,
    dexterity: u8,
    attack: u16,
    defense: u16,
    magic_defense: u16,
}

#[derive(Deserialize)]
struct EnemyAttackJson {
    element: IdRef,
    status_effect: IdRef,
}

#[derive(Deserialize)]
struct PropertyJson {
    value: u8,
}

#[derive(Deserialize)]
struct EnemyAiJson {
    regular_ability_ids: Vec<u8>,
    condition_ids: Vec<u8>,
    conditional_ability_ids: Vec<u8>,
}

#[derive(Deserialize)]
struct RewardsJson {
    experience: u16,
    meseta: u16,
}

#[derive(Deserialize)]
struct SkillJson {
    id: u8,
    display_name: String,
    effect_id: u8,
    relevant_stat: IdRef,
    target_id: u8,
    power_or_hit_chance: u8,
    resistance_stat: IdRef,
    element: IdRef,
}

impl From<EnemyJson> for EnemyRecord {
    fn from(json: EnemyJson) -> Self {
        let properties = ELEMENT_NAMES.map(|name| {
            json.properties
                .get(name)
                .unwrap_or_else(|| panic!("enemy {} has no {name} property", json.id))
                .value
        });
        EnemyRecord {
            id: json.id,
            // The symbol, as the fixtures' hand-built records name an enemy.
            name: json.symbol.to_uppercase(),
            hp: json.hp,
            strength: json.stats.strength,
            mental: json.stats.mental,
            agility: json.stats.agility,
            dexterity: json.stats.dexterity,
            attack: json.stats.attack,
            defence: json.stats.defense,
            mental_defence: json.stats.magic_defense,
            attack_element: json.attack.element.id,
            attack_status: json.attack.status_effect.id,
            properties,
            regular_abilities: json
                .ai
                .regular_ability_ids
                .as_slice()
                .try_into()
                .expect("eight regular abilities"),
            condition_ids: first_four(&json.ai.condition_ids, json.id, "condition ids"),
            conditional_abilities: first_four(
                &json.ai.conditional_ability_ids,
                json.id,
                "conditional abilities",
            ),
            experience: json.rewards.experience,
            meseta: json.rewards.meseta,
        }
    }
}

/// The four AI slots `EnemyRecord` holds (`AI_CONDITIONS`): the pack may
/// carry more condition bytes, and the engine reads the first four.
fn first_four(values: &[u8], enemy: u16, what: &str) -> [u8; AI_CONDITIONS] {
    values
        .get(..AI_CONDITIONS)
        .and_then(|four| four.try_into().ok())
        .unwrap_or_else(|| panic!("enemy {enemy}'s {what} is shorter than the record"))
}

impl From<SkillJson> for EnemySkill {
    fn from(json: SkillJson) -> Self {
        EnemySkill {
            id: json.id,
            name: json.display_name,
            effect: json.effect_id,
            power_stat: json.relevant_stat.id,
            target: json.target_id,
            power: json.power_or_hit_chance,
            resistance: json.resistance_stat.id,
            element: json.element.id,
        }
    }
}

fn enemy(record: EnemyRecord, attack_status: u8, regular_abilities: [u8; 8]) -> EnemyRecord {
    EnemyRecord {
        attack_status,
        regular_abilities,
        ..record
    }
}

/// Enemy 0, formation `$5E`: two of them, all eight ability slots FLAME BOLT.
pub(crate) fn helex() -> EnemyRecord {
    enemy(
        EnemyRecord {
            id: 0,
            name: "HELEX".into(),
            hp: 90,
            strength: 10,
            mental: 7,
            agility: 45,
            dexterity: 34,
            attack: 160,
            defence: 3,
            mental_defence: 0,
            attack_element: 1,
            attack_status: 0,
            properties: [2, 2, 0, 2, 4, 2, 2, 0, 1, 2, 2, 0, 0, 2],
            regular_abilities: [0; 8],
            condition_ids: [0; 4],
            conditional_abilities: [0; 4],
            experience: 1087,
            meseta: 281,
        },
        0,
        [2; 8],
    )
}

/// Enemy 15, formation `$37`: two Fanbite, whose list is six empty slots and
/// two SPIRAL BLD.
pub(crate) fn fanbite() -> EnemyRecord {
    enemy(
        EnemyRecord {
            id: 15,
            name: "FANBITE".into(),
            hp: 261,
            strength: 32,
            mental: 9,
            agility: 22,
            dexterity: 26,
            attack: 111,
            defence: 14,
            mental_defence: 2,
            attack_element: 1,
            attack_status: 0,
            properties: [2, 2, 2, 2, 3, 2, 2, 0, 1, 2, 2, 0, 0, 2],
            regular_abilities: [0; 8],
            condition_ids: [0; 4],
            conditional_abilities: [0; 4],
            experience: 496,
            meseta: 78,
        },
        // $1C, the paralyse its plain attack carries (`basic_attack`'s
        // `status_effect` 28).
        0x1C,
        [0, 0, 0, 0, 0, 0, 8, 8],
    )
}

/// Enemy 81, formation `$53`: one Desrt Leach, five empty slots and three
/// SAND STORM.
pub(crate) fn desrt_leach() -> EnemyRecord {
    enemy(
        EnemyRecord {
            id: 81,
            name: "DESRTLEACH".into(),
            hp: 1040,
            strength: 68,
            mental: 1,
            agility: 56,
            dexterity: 52,
            attack: 286,
            defence: 28,
            mental_defence: 19,
            attack_element: 1,
            attack_status: 0,
            properties: [2, 2, 2, 2, 4, 2, 2, 0, 1, 2, 0, 0, 0, 2],
            regular_abilities: [0; 8],
            condition_ids: [0; 4],
            conditional_abilities: [0; 4],
            experience: 1500,
            meseta: 1,
        },
        0,
        [0, 0, 0, 0, 0, 55, 55, 55],
    )
}

/// `EnemySkillData` `$02` FLAME BOLT, record `01 01 08 50 07 03 00 00`:
/// a single target, strength-powered, resisted by magic defence, fire.
pub(crate) fn flame_bolt() -> EnemySkill {
    EnemySkill {
        id: 2,
        name: "FLAME BOLT".into(),
        effect: 1,
        power_stat: 1,
        target: 8,
        power: 80,
        resistance: 7,
        element: 3,
    }
}

/// `EnemySkillData` `$08` SPIRAL BLD, record `01 05 09 00 06 01 00 00`: the
/// all-party route, attack-powered, resisted by defence, physical.
pub(crate) fn spiral_bld() -> EnemySkill {
    EnemySkill {
        id: 8,
        name: "SPIRAL BLD".into(),
        effect: 1,
        power_stat: 5,
        target: 9,
        power: 0,
        resistance: 6,
        element: 1,
    }
}

/// `EnemySkillData` `$37` SAND STORM, record `01 05 09 60 06 01 00 00`: the
/// vehicle route's single target, attack-powered, defence-resisted, physical.
pub(crate) fn sand_storm() -> EnemySkill {
    EnemySkill {
        id: 55,
        name: "SAND STORM".into(),
        effect: 1,
        power_stat: 5,
        target: 9,
        power: 96,
        resistance: 6,
        element: 1,
    }
}
