//! Fixtures shared by the damage-skill route tests.
//!
//! This file used to hold every test of [`resolve_damage_skill`] in one module.
//! At 1,504 lines it broke the repository's 1,000-line rule, so the tests moved
//! verbatim into one child module per group of pairs, each registered with
//! `#[path]` from `enemy_damage.rs` (the all-party group already had its own
//! module; it is listed here for the map, not as a destination of this split):
//!
//! | module | what it covers |
//! |---|---|
//! | `acid_tests` | `$33` ACIDBREATH's four carriers, their death and ailment behaviour, and one engine-driven round |
//! | `flame_tests` | `$02` FLAME BOLT's two carriers, the record's arithmetic, and one engine-driven round |
//! | `gate_tests` | the gate itself: the effect-`$01` requirement and what replaced the byte-2 check |
//! | `motavia_tests` | the Motavia single-target pairs and the stat widths they read |
//! | `all_party_tests` | `$08` SPIRAL BLD and `$38` EARTHQUAKE, the `AllParty` class |
//!
//! What stays here is what more than one of them uses: [`id`], [`kill`],
//! [`record_damage`], and the Motavia carrier stat lines with the record, data
//! and roster builders around them.

use super::*;
use crate::battle::{ELEMENT_SLOTS, EnemySkill, PartyMember, fixtures, status};

pub(super) fn id(n: u8) -> FighterId {
    FighterId::new(n).unwrap()
}

pub(super) fn kill(r: &mut Roster, n: u8) {
    let f = r.get_mut(id(n)).unwrap();
    f.stats.curr_hp = 0;
    f.stats.status = status::DEAD;
}

/// `Battle_CalculateDamage` (`ps4.asm:17374`) as `Enemy_DamageCharacter`
/// (`ps4.asm:3775`) feeds it, at 16 zero draws (so `S + 8` is 8):
/// `(((8 * power) >> 6) + power + 2 * bonus) * element >> 2 - resistance`,
/// every step a 16-bit word like the cartridge's.
pub(super) fn record_damage(power: u16, resistance: u16, element_factor: u8, bonus: u8) -> u16 {
    let mut t = 8u16.wrapping_mul(power);
    t >>= 6;
    t = t.wrapping_add(power);
    t = t.wrapping_add(u16::from(bonus).wrapping_mul(2));
    t = t.wrapping_mul(u16::from(element_factor));
    t >>= 2;
    crate::battle::clamp_damage(t.wrapping_sub(resistance) as i16)
}

/// The carriers of `docs/battle/ENEMY_DAMAGE_ROUTES.md` §3's single-target pairs, with
/// the stat line `generated/enemies.json` gives them. `Enemy_DamageCharacter`
/// (`ps4.asm:3775`) reads the caster's stat through record byte 1, so each
/// expected number below is that carrier's own stat.
#[derive(Clone, Copy)]
pub(super) struct Carrier {
    pub(super) enemy_id: u16,
    pub(super) symbol: &'static str,
    pub(super) hp: u16,
    pub(super) strength: u8,
    pub(super) mental: u8,
    pub(super) attack: u16,
    pub(super) abilities: [u8; 8],
}

pub(super) const FROST_SABER: Carrier = Carrier {
    enemy_id: 71,
    symbol: "FrostSaber",
    hp: 211,
    strength: 70,
    mental: 45,
    attack: 175,
    abilities: [0, 0, 0, 0, 0, GIWAT, GIWAT, GIWAT],
};
pub(super) const TECH_PLANT: Carrier = Carrier {
    enemy_id: 77,
    symbol: "TechPlant",
    hp: 124,
    strength: 19,
    mental: 27,
    attack: 141,
    abilities: [0, 0, 42, GIWAT, GIWAT, GIWAT, 53, 53],
};
pub(super) const HEW_GILLA: Carrier = Carrier {
    enemy_id: 91,
    symbol: "HewGilla",
    hp: 165,
    strength: 62,
    mental: 79,
    attack: 154,
    abilities: [
        GIWAT, GIWAT, GIWAT, WAT, WAT, FLODBREATH, FLODBREATH, FLODBREATH,
    ],
};
pub(super) const DARK_WITCH: Carrier = Carrier {
    enemy_id: 101,
    symbol: "DarkWitch",
    hp: 253,
    strength: 40,
    mental: 76,
    attack: 124,
    abilities: [GIWAT, GIWAT, GIWAT, 53, 53, 72, 72, 72],
};
pub(super) const DELM_LARS: Carrier = Carrier {
    enemy_id: 122,
    symbol: "DElmLars",
    hp: 777,
    strength: 50,
    mental: 45,
    attack: 185,
    abilities: [0, 0, 0, 0, GIWAT, GIWAT, GIWAT, 53],
};
pub(super) const XE_ATHOUL: Carrier = Carrier {
    enemy_id: 123,
    symbol: "XeAThoul",
    hp: 1520,
    strength: 40,
    mental: 118,
    attack: 162,
    abilities: [0, 0, 0, GIWAT, GIWAT, GIWAT, 53, 53],
};
pub(super) const DESRT_LEACH: Carrier = Carrier {
    enemy_id: 81,
    symbol: "DesrtLeach",
    hp: 1040,
    strength: 68,
    mental: 1,
    attack: 286,
    abilities: [0, 0, 0, 0, 0, SAND_STORM, SAND_STORM, SAND_STORM],
};
pub(super) const LEVIATHAN: Carrier = Carrier {
    enemy_id: 82,
    symbol: "Leviathan",
    hp: 1240,
    strength: 84,
    mental: 1,
    attack: 252,
    abilities: [0, 0, 0, 0, 0, MAELSTROM, MAELSTROM, MAELSTROM],
};
pub(super) const DEPCEN: Carrier = Carrier {
    enemy_id: 90,
    symbol: "Depcen",
    hp: 155,
    strength: 30,
    mental: 19,
    attack: 101,
    abilities: [0, 0, 0, 0, 0, FLODBREATH, FLODBREATH, FLODBREATH],
};
pub(super) const ELMELEW: Carrier = Carrier {
    enemy_id: 92,
    symbol: "Elmelew",
    hp: 362,
    strength: 72,
    mental: 59,
    attack: 184,
    abilities: [
        WAT, WAT, WAT, WAT, FLODBREATH, FLODBREATH, FLODBREATH, FLODBREATH,
    ],
};
pub(super) const TECH_USER: Carrier = Carrier {
    enemy_id: 99,
    symbol: "TechUser",
    hp: 80,
    strength: 10,
    mental: 25,
    attack: 42,
    abilities: [WAT, WAT, WAT, WAT, FOI, FOI, FOI, FOI],
};
pub(super) const TECH_MASTER: Carrier = Carrier {
    enemy_id: 100,
    symbol: "TechMaster",
    hp: 120,
    strength: 21,
    mental: 38,
    attack: 85,
    abilities: [62, WAT, WAT, WAT, FOI, FOI, FOI, 71],
};
pub(super) const JUZA: Carrier = Carrier {
    enemy_id: 114,
    symbol: "Juza",
    hp: 1523,
    strength: 28,
    mental: 30,
    attack: 92,
    abilities: [WAT, WAT, FOI, FOI, 71, 71, 86, 86],
};
pub(super) const RAPPY: Carrier = Carrier {
    enemy_id: 147,
    symbol: "Rappy",
    hp: 65,
    strength: 34,
    mental: 23,
    attack: 173,
    abilities: [0, 0, 0, 0, ROUND_EYES, ROUND_EYES, ROUND_EYES, ROUND_EYES],
};
pub(super) const BLUE_RAPPY: Carrier = Carrier {
    enemy_id: 148,
    symbol: "BlueRappy",
    hp: 130,
    strength: 54,
    mental: 35,
    attack: 184,
    abilities: [0, 0, 0, 0, LOVEL_EYES, LOVEL_EYES, LOVEL_EYES, LOVEL_EYES],
};

/// The one `EnemySkillData` record each ability's pair runs, as
/// `generated/enemy_skills.json` decodes it. Byte 1 stays raw — `$82` is the
/// masked-selector case.
pub(super) fn motavia_record(ability: u8) -> EnemySkill {
    let (name, power_stat, target, power, resistance, element) = match ability {
        GIWAT => ("GIWAT", 0x82, 8, 88, 7, 5),
        SAND_STORM => ("SAND STORM", 0x05, 9, 96, 6, 1),
        MAELSTROM => ("MAELSTROM", 0x05, 9, 32, 6, 1),
        FLODBREATH => ("FLODBREATH", 0x05, 8, 20, 6, 1),
        WAT => ("WAT", 0x82, 8, 24, 7, 5),
        FOI => ("FOI", 0x82, 8, 20, 7, 3),
        ROUND_EYES => ("ROUND EYES", 0x01, 8, 0, 6, 1),
        LOVEL_EYES => ("LOVEL EYES", 0x05, 8, 32, 6, 1),
        other => panic!("no Motavia record for {other:#04X}"),
    };
    EnemySkill {
        id: ability,
        name: name.into(),
        effect: 1,
        power_stat,
        target,
        power,
        resistance,
        element,
    }
}

/// Carrier records plus the records the pairs under test run.
pub(super) fn motavia_data(carriers: &[Carrier], abilities: &[u8]) -> BattleData {
    let enemies = carriers.iter().map(|carrier| {
        let mut record = fixtures::zoran_bult();
        record.id = carrier.enemy_id;
        record.name = carrier.symbol.into();
        record.hp = carrier.hp;
        record.strength = carrier.strength;
        record.mental = carrier.mental;
        record.attack = carrier.attack;
        record.agility = 100;
        record.regular_abilities = carrier.abilities;
        record.condition_ids = [0; 4];
        record
    });
    fixtures::data()
        .with_enemies(enemies)
        .with_enemy_skills(abilities.iter().map(|ability| motavia_record(*ability)))
}

/// Three party members with defense 7, no magic defense and every element
/// factor 2 (normal), so a number below depends only on the carrier's stat and
/// the record's bytes.
pub(super) fn motavia_roster(data: &BattleData, carrier: &Carrier) -> Roster {
    let mut r = Roster::new();
    for record in [fixtures::alys(), fixtures::chaz(), fixtures::hahn()] {
        let mut member = PartyMember::seat(&record, data).unwrap();
        member.stats.curr_hp = 400;
        member.stats.max_hp = 400;
        member.stats.agility.battle = 255;
        member.stats.defence.battle = 7;
        member.stats.mental_defence.battle = 0;
        member.stats.element_props = [2; ELEMENT_SLOTS];
        r.add_party_member(member.character, member.name, member.stats);
    }
    r.add_enemy(1, data.enemy(carrier.enemy_id).unwrap());
    r
}
