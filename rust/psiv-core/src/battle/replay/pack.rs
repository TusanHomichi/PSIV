//! The pack records the captured battles need, beyond the hand-built fixtures.
//!
//! `crate::battle::fixtures::data` carries the three enemies the oracle tapes
//! happen to meet. A forced capture meets the formation it asked for, whose
//! records - and the ability records its `eN_ability` byte names - come from
//! the project's own pack, `generated/enemies.json` and
//! `generated/enemy_skills.json`. They are transcribed here the way
//! `fixtures::zoran_bult()` and its siblings are: the fixture supplies the
//! *live* battle state the RAM log shows, this supplies the records, and
//! [`super::start`] is where the two are held against each other.
//!
//! | capture | enemy | abilities | record |
//! |---|---|---|---|
//! | `$5E` two Helex | 0 HELEX | eight $02 FLAME BOLT | `generated/enemies.json` |
//! | `$37` two Fanbite | 15 FANBITE | six empty, two $08 SPIRAL BLD | same |
//! | `$53` one Desrt Leach | 81 DESRTLEACH | five empty, three $37 SAND STORM | same |

use crate::battle::fixtures;
use crate::battle::*;

/// The hand-built fixtures' data plus the forced captures' records.
pub(crate) fn data() -> BattleData {
    fixtures::data()
        .with_enemies([helex(), fanbite(), desrt_leach()])
        .with_enemy_skills([flame_bolt(), spiral_bld(), sand_storm()])
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
