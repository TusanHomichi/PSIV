//! Cartridge records used across the battle tests.
//!
//! Every number here is copied from `generated/*.json`, which the extractor
//! proved against the retail image, and the party's derived stats are the ones
//! the oracle logged out of live RAM (`oracle/README.md`, "Battle ground
//! truth"). Hand-built rather than pack-loaded because `psiv-core` never reads
//! a file — and because a fixture that fails to match the oracle is exactly the
//! signal a test suite is for.

use super::records::{
    BattleData, Bonuses, CharacterRecord, ELEMENT_SLOTS, EnemyRecord, FormationEnemy,
    FormationRecord, ItemKind, ItemRecord, LevelRecord, LevelTable,
};

/// `generated/items.json`, the entries the opening party wears.
#[must_use]
pub fn items() -> Vec<ItemRecord> {
    let weapon = |id, name: &str, kind, attack| ItemRecord {
        id,
        name: name.into(),
        kind,
        bonuses: Bonuses {
            attack,
            ..Bonuses::default()
        },
        element: 1,
    };
    let armour = |id, name: &str, kind, defence| ItemRecord {
        id,
        name: name.into(),
        kind,
        bonuses: Bonuses {
            defence,
            ..Bonuses::default()
        },
        element: 0,
    };
    vec![
        weapon(1, "DAGGER", ItemKind::OneHandedSingleTarget, 2),
        weapon(2, "HUNT-KNIFE", ItemKind::OneHandedSingleTarget, 5),
        weapon(3, "BOOMERANG", ItemKind::OneHandedMultiTarget, 1),
        armour(4, "LTHR-CLOTH", ItemKind::Body, 2),
        armour(5, "LTHR-HELM", ItemKind::Headwear, 1),
        armour(6, "LTHR-CROWN", ItemKind::Headwear, 1),
        armour(7, "LTHR-BAND", ItemKind::Headwear, 1),
        armour(10, "LTHR-SHIELD", ItemKind::Shield, 2),
    ]
}

fn character(
    id: u8,
    name: &str,
    level: u16,
    hp: u16,
    tp: u16,
    stats: (u8, u8, u8, u8),
    equipment: [u8; 4],
) -> CharacterRecord {
    let (strength, mental, agility, dexterity) = stats;
    CharacterRecord {
        id,
        name: name.into(),
        profession: 0,
        level,
        experience: 0,
        hp,
        max_hp: hp,
        tp,
        max_tp: tp,
        strength,
        mental,
        agility,
        dexterity,
        properties: chaz_properties(),
        equipment,
    }
}

/// Chaz's element resistances: normal to everything except holyword and
/// mechanical (immune) and brose (resistant).
fn chaz_properties() -> [u8; ELEMENT_SLOTS] {
    let mut props = [2u8; ELEMENT_SLOTS];
    props[7] = 0; // holyword
    props[8] = 1; // brose
    props[11] = 0; // mechanical
    props
}

/// Chaz at level 1, as the new-game initialiser writes him.
///
/// Derives to attack 18, defence 10 — the oracle's RAM values.
#[must_use]
pub fn chaz() -> CharacterRecord {
    character(0, "CHAZ", 1, 25, 10, (8, 6, 7, 5), [2, 2, 5, 4])
}

/// Alys at level 7, holding the type-2 Boomerang that makes her attacks hit
/// every enemy. Derives to attack 13, defence 18.
#[must_use]
pub fn alys() -> CharacterRecord {
    character(1, "ALYS", 7, 53, 40, (12, 12, 15, 13), [3, 0, 6, 4])
}

/// Hahn at level 1. Derives to attack 8, defence 9.
#[must_use]
pub fn hahn() -> CharacterRecord {
    character(2, "HAHN", 1, 21, 25, (6, 8, 4, 5), [1, 10, 7, 4])
}

fn enemy(
    id: u16,
    name: &str,
    hp: u16,
    stats: (u8, u8, u8, u8),
    attack: u16,
    defence: u16,
    rewards: (u16, u16),
) -> EnemyRecord {
    let (strength, mental, agility, dexterity) = stats;
    let (experience, meseta) = rewards;
    EnemyRecord {
        id,
        name: name.into(),
        hp,
        strength,
        mental,
        agility,
        dexterity,
        attack,
        defence,
        mental_defence: 0,
        attack_element: 1,
        attack_status: 0,
        properties: [2; ELEMENT_SLOTS],
        regular_abilities: [0; 8],
        condition_ids: [0; 4],
        conditional_abilities: [0; 4],
        experience,
        meseta,
    }
}

/// Enemy 10, the Academy Basement patrol both oracle tapes fought.
#[must_use]
pub fn zoran_bult() -> EnemyRecord {
    enemy(10, "ZORAN-BULT", 25, (18, 4, 6, 8), 16, 2, (12, 3))
}

/// Enemy 9, tape 09's other combatant.
#[must_use]
pub fn xanafalgue() -> EnemyRecord {
    enemy(9, "XANAFALGUE", 16, (6, 0, 5, 7), 13, 0, (9, 2))
}

/// Enemy 1, the scout's worked example.
#[must_use]
pub fn monster_fly() -> EnemyRecord {
    let mut record = enemy(1, "MONSTER-FLY", 20, (4, 1, 12, 8), 14, 0, (27, 8));
    // Its plain attack carries poison — the `max_tp` slot. Tier 2 acts on it.
    record.attack_status = 0x1B;
    record
}

/// Formation 0 of block 1: two MonsterFly, ambush chance `$10`, freely
/// escapable, an 8-in-128 Antidote.
#[must_use]
pub fn formation_two_monster_flies() -> FormationRecord {
    FormationRecord {
        id: 0,
        ambush_chance: 0x10,
        run_chance: 0,
        drop_rate: 8,
        drop_item: Some(0x80),
        enemies: vec![
            FormationEnemy {
                slot: 1,
                enemy_id: 1,
                position: 14,
            },
            FormationEnemy {
                slot: 2,
                enemy_id: 1,
                position: 26,
            },
        ],
    }
}

/// Tape 07's formation: two ZoranBult.
#[must_use]
pub fn formation_two_zoran_bults() -> FormationRecord {
    FormationRecord {
        id: 100,
        ambush_chance: 0x10,
        run_chance: 0,
        drop_rate: 0,
        drop_item: None,
        enemies: vec![
            FormationEnemy {
                slot: 1,
                enemy_id: 10,
                position: 14,
            },
            FormationEnemy {
                slot: 2,
                enemy_id: 10,
                position: 26,
            },
        ],
    }
}

/// Tape 09's formation: Xanafalgue then ZoranBult.
#[must_use]
pub fn formation_xanafalgue_and_zoran_bult() -> FormationRecord {
    FormationRecord {
        id: 101,
        ambush_chance: 0x10,
        run_chance: 0,
        drop_rate: 0,
        drop_item: None,
        enemies: vec![
            FormationEnemy {
                slot: 1,
                enemy_id: 9,
                position: 14,
            },
            FormationEnemy {
                slot: 2,
                enemy_id: 10,
                position: 26,
            },
        ],
    }
}

/// Chaz's level table, from `generated/progression.json`, far enough to cover
/// the level-up tests.
#[must_use]
pub fn chaz_levels() -> LevelTable {
    LevelTable {
        starting_level: 1,
        levels: vec![
            LevelRecord {
                level: 2,
                experience_required: 21,
                hp: 31,
                tp: 13,
                strength: 9,
                mental: 7,
                agility: 8,
                dexterity: 6,
            },
            LevelRecord {
                level: 3,
                experience_required: 109,
                hp: 34,
                tp: 17,
                strength: 11,
                mental: 8,
                agility: 9,
                dexterity: 7,
            },
        ],
    }
}

/// A short table for Alys, who starts at 7 — the case that proves the
/// pointer-table entry's starting level is subtracted before indexing.
#[must_use]
pub fn alys_levels() -> LevelTable {
    LevelTable {
        starting_level: 7,
        levels: vec![LevelRecord {
            level: 8,
            experience_required: 30,
            hp: 60,
            tp: 44,
            strength: 13,
            mental: 13,
            agility: 16,
            dexterity: 14,
        }],
    }
}

/// Everything the engine looks up, wired together.
#[must_use]
pub fn data() -> BattleData {
    BattleData::new()
        .with_enemies([zoran_bult(), xanafalgue(), monster_fly()])
        .with_items(items())
        .with_level_table(0, chaz_levels())
        .with_level_table(1, alys_levels())
        .with_level_table(2, chaz_levels())
}
