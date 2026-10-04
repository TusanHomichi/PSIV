//! The Zio-arc all-party pairs: `$47` ZAN, `$56` FORCEFLASH and `$4D`
//! CORRSION, through [`resolve_damage_skill`] and through `roll_enemy_ability`.
//!
//! The class they prove is [`DamageClass::AllParty`]: every one of these arms
//! clears `Current_Target_Index` before it loads its object, and the object
//! either runs the five-slot loop itself or exits through `loc_24BB6`
//! (`ps4.asm:48562`). What each test pins is the part of that reading the
//! cartridge's own instructions decide — the slot set, the slot order, the
//! per-target arithmetic and the draw count of 16 per damaged slot and nothing
//! else (none of these chains touches `UpdateRNGSeed2`).
//!
//! The three abilities are also the shapes issue
//! [#58](https://github.com/TusanHomichi/PSIV/issues/58)'s campaign halts were
//! about: `docs/campaign/RUNNER_LOG.md` H16 stops at Juza on ZAN and
//! FORCEFLASH, and the Zio Fort's and Ladea Tower's Haunts and the Nurvus
//! TechMasters carry CORRSION and ZAN.

use super::routes::DamageRoute;
use super::tests::*;
use super::*;
use crate::battle::{
    Battle, Command, ELEMENT_SLOTS, FormationEnemy, FormationRecord, PartyMember, RoundOrders,
    SliceRolls, fixtures,
};

// The abilities the damage half deliberately does **not** run. Their chains
// make no `move.w #$C` request at all, so a row would claim damage the
// cartridge never deals, and their effect handlers are another lane's:
//
// | ability | record | effect |
// |---|---|---|
// | `$4C` EVIL EYE | `07 02 08 40 02 0B 00 00` at `$2835C4` | `$07` `AbilityEffect_SleepParalyze` (`ps4.asm:9154`) |
// | `$2A` RIMIT | `07 82 09 20 02 0B 00 00` at `$2834B4` | `$07` `AbilityEffect_SleepParalyze` |
// | `$2F` VOL | `02 82 08 50 02 0A 00 00` at `$2834DC` | `$02` `AbilityEffect_Death` (`ps4.asm:9098`) |
const EVIL_EYE: u8 = 0x4C;
const RIMIT: u8 = 0x2A;
const VOL: u8 = 0x2F;

/// The ten carriers of the Zio-arc family, with the stat line
/// `generated/enemies.json` gives them and their real regular lists — the
/// lists are what the engine-driven rounds below are checked against.
#[derive(Clone, Copy)]
struct Carrier {
    enemy_id: u16,
    symbol: &'static str,
    hp: u16,
    mental: u8,
    abilities: [u8; 8],
}

const TECH_MASTER: Carrier = Carrier {
    enemy_id: 100,
    symbol: "TechMaster",
    hp: 120,
    mental: 38,
    abilities: [0x3E, 0x40, 0x40, 0x40, 0x44, 0x44, 0x44, ZAN],
};
const JUZA: Carrier = Carrier {
    enemy_id: 114,
    symbol: "Juza",
    hp: 1523,
    mental: 30,
    abilities: [0x40, 0x40, 0x44, 0x44, ZAN, ZAN, FORCEFLASH, FORCEFLASH],
};
const GRENERIS: Carrier = Carrier {
    enemy_id: 115,
    symbol: "Greneris",
    hp: 360,
    mental: 31,
    abilities: [0x28, 0x29, RIMIT, VOL, VOL, 0x57, FORCEFLASH, FORCEFLASH],
};
const RADHIN: Carrier = Carrier {
    enemy_id: 116,
    symbol: "Radhin",
    hp: 286,
    mental: 65,
    abilities: [0x26, 0x27, 0x29, 0x2D, 0x27, 0x2D, FORCEFLASH, FORCEFLASH],
};
/// 77 TechPlant — off the arc maps, and the RIMIT carrier the §2 census lists beside
/// Greneris. Its own regular list is `$2A`/`$2E`/`$35`.
const TECH_PLANT: Carrier = Carrier {
    enemy_id: 77,
    symbol: "TechPlant",
    hp: 124,
    mental: 27,
    abilities: [0, 0, RIMIT, 0x2E, 0x2E, 0x2E, 0x35, 0x35],
};

const HAUNT: Carrier = Carrier {
    enemy_id: 106,
    symbol: "Haunt",
    hp: 3,
    mental: 1,
    abilities: [0, 0, 0, 0, 0, EVIL_EYE, EVIL_EYE, CORRSION],
};
const SPECTOR: Carrier = Carrier {
    enemy_id: 107,
    symbol: "Spector",
    hp: 180,
    mental: 35,
    abilities: [0, 0, 0x4E, 0x4E, EVIL_EYE, EVIL_EYE, CORRSION, CORRSION],
};
const CHAOS_SORCR: Carrier = Carrier {
    enemy_id: 111,
    symbol: "ChaosSorcr",
    hp: 460,
    mental: 49,
    abilities: [0, 0, 0x4B, CORRSION, 0x4F, 0x52, 0x5A, 0x5A],
};
const ILLUSIONST: Carrier = Carrier {
    enemy_id: 112,
    symbol: "Illusionst",
    hp: 480,
    mental: 65,
    abilities: [0, CORRSION, 0x5A, 0x4E, 0x4F, 0x51, 0x52, 0x52],
};
const IMAGIO_MAGE: Carrier = Carrier {
    enemy_id: 113,
    symbol: "ImagioMage",
    hp: 586,
    mental: 83,
    abilities: [0, CORRSION, 0x5A, 0x4E, 0x4F, 0x51, 0x52, 0x55],
};
const DARK_FORCE3: Carrier = Carrier {
    enemy_id: 132,
    symbol: "DarkForce3",
    hp: 8240,
    mental: 172,
    abilities: [0, 0, 0, 0x4B, CORRSION, CORRSION, CORRSION, 0x51],
};

fn carrier_record(carrier: &Carrier) -> crate::battle::EnemyRecord {
    let mut record = fixtures::zoran_bult();
    record.id = carrier.enemy_id;
    record.name = carrier.symbol.into();
    record.hp = carrier.hp;
    record.strength = 1;
    record.mental = carrier.mental;
    record.attack = 1;
    record.agility = 100;
    record.regular_abilities = carrier.abilities;
    // The real records' conditions are the conditional-ability lane's work:
    // TechMaster's `$F`/`$46` and Radhin's `$F`/`$49` replace the rolled id
    // before the arm runs, and no round below exercises them.
    record.condition_ids = [0; 4];
    record
}

/// One ability's record, as `generated/enemy_skills.json` decodes it. The four
/// abilities with a listed pair come from the shared builder; the three the
/// table deliberately leaves out are spelled here, effect byte and all, so the
/// refusal control below runs against the real records rather than a missing
/// one.
fn zio_record(ability: u8) -> crate::battle::EnemySkill {
    let (name, effect, power_stat, target, power, resistance, element) = match ability {
        EVIL_EYE => ("EVIL EYE", 0x07, 0x02, 8, 64, 0x02, 11),
        RIMIT => ("RIMIT", 0x07, 0x82, 9, 32, 0x02, 11),
        VOL => ("VOL", 0x02, 0x82, 8, 80, 0x02, 10),
        listed => return motavia_record(listed),
    };
    crate::battle::EnemySkill {
        id: ability,
        name: name.into(),
        effect,
        power_stat,
        target,
        power,
        resistance,
        element,
    }
}

/// Carrier records plus the records the pairs under test run.
fn zio_data(carriers: &[Carrier], abilities: &[u8]) -> BattleData {
    fixtures::data()
        .with_enemies(carriers.iter().map(carrier_record))
        .with_enemy_skills(abilities.iter().map(|ability| zio_record(*ability)))
}

/// Three party members with three different magic defenses and three different
/// physical element factors, so a number below can only come from the target's
/// own stats — the `move.b (a1,d2.w), d2` / `move.b (a1,d3.w), d3` reads of
/// `Enemy_DamageCharacter`'s enemy-skill branch (`ps4.asm:3775`), whose `a1` is
/// the *taking* fighter's stats. Every one of these three records resists with
/// selector `$07`, magic defense.
fn zio_roster(data: &BattleData, carrier: &Carrier) -> Roster {
    let mut r = Roster::new();
    for (record, magic_defence, physical) in [
        (fixtures::alys(), 0u16, 2u8),
        (fixtures::chaz(), 3, 2),
        (fixtures::hahn(), 7, 3),
    ] {
        let mut member = PartyMember::seat(&record, data).unwrap();
        member.stats.curr_hp = 400;
        member.stats.max_hp = 400;
        member.stats.agility.battle = 255;
        member.stats.defence.battle = 0;
        member.stats.mental_defence.battle = magic_defence;
        member.stats.element_props = [2; ELEMENT_SLOTS];
        member.stats.element_props[0] = physical;
        r.add_party_member(member.character, member.name, member.stats);
    }
    r.add_enemy(1, data.enemy(carrier.enemy_id).unwrap());
    r
}

/// One carrier's ability on a zero stream: the damage per living party slot, in
/// slot order, and the draws the run took.
fn resolve_on(carrier: &Carrier, ability: u8) -> (Vec<u16>, usize) {
    let data = zio_data(&[*carrier], &[ability]);
    let mut r = zio_roster(&data, carrier);
    let mut rolls = SliceRolls::new(&[0]);
    let mut events = Vec::new();
    assert!(
        resolve_damage_skill(
            &mut r,
            id(6),
            ability,
            Some(id(2)),
            &data,
            &mut rolls,
            &mut events
        ),
        "{} {ability:#04X} is a listed route",
        carrier.symbol
    );
    let damage = events
        .iter()
        .filter_map(|event| match event {
            BattleEvent::Resolved {
                damage: Some(d), ..
            } => Some(*d),
            _ => None,
        })
        .collect();
    (damage, rolls.drawn())
}

/// `$47` ZAN: record byte 1 is `$82` — masked to selector 2 (mental) by both
/// readers — byte 3 is 16, resistance `$07` (magic defense) and element `1`
/// (physical). Every carrier's own mental stat decides its three numbers, and
/// the drawn `intended` plays no part: the arm cleared `Current_Target_Index`
/// before it loaded the object, so all three slots are hit.
#[test]
fn zan_reads_the_masked_mental_selector_on_both_carriers() {
    assert_eq!(motavia_record(ZAN).power_stat, 0x82);
    assert_eq!(motavia_record(ZAN).target, 9, "the all-party nibble");
    for (carrier, expected) in [(TECH_MASTER, [37u16, 34, 48]), (JUZA, [32, 29, 41])] {
        let (damage, drawn) = resolve_on(&carrier, ZAN);
        assert_eq!(drawn, 48, "{}: three runs of sixteen", carrier.symbol);
        assert_eq!(damage, expected, "{}", carrier.symbol);
        assert_eq!(
            expected,
            [
                record_damage(u16::from(carrier.mental), 0, 2, 16),
                record_damage(u16::from(carrier.mental), 3, 2, 16),
                record_damage(u16::from(carrier.mental), 7, 3, 16),
            ],
            "{}: the formula's own terms",
            carrier.symbol
        );
        assert_eq!(
            damage.len(),
            3,
            "{}: one per living party slot",
            carrier.symbol
        );
    }
}

/// `$56` FORCEFLASH: stat `$02` (mental), byte 3 = 36, resistance `$07` and
/// element `2` (energy). All three carriers run the same object and share the
/// same `loc_2AFB6` request as ZAN, so the only thing that separates their
/// numbers is the mental stat the record selects.
#[test]
fn forcelflash_shares_zans_request_and_reads_mental() {
    assert_eq!(motavia_record(FORCEFLASH).power_stat, 0x02);
    assert_eq!(motavia_record(FORCEFLASH).element, 2);
    for (carrier, expected) in [
        (JUZA, [52u16, 49, 45]),
        (GRENERIS, [53, 50, 46]),
        (RADHIN, [72, 69, 65]),
    ] {
        let (damage, drawn) = resolve_on(&carrier, FORCEFLASH);
        assert_eq!(drawn, 48, "{}: three runs of sixteen", carrier.symbol);
        assert_eq!(damage, expected, "{}", carrier.symbol);
        assert_eq!(
            expected,
            [0u16, 3, 7].map(|mdef| record_damage(u16::from(carrier.mental), mdef, 2, 36)),
            "{}: the formula's own terms",
            carrier.symbol
        );
    }
}

/// `$4D` CORRSION: stat `$02` (mental), byte 3 = 64, resistance `$07` and
/// element `1` (physical). Six carriers, two object chains — Haunt's and
/// Spector's reach `loc_24BB6`, ChaosSorcr's, Illusionst's, ImagioMage's and
/// DarkForce3's their own loops — and one class and one arithmetic for all six.
#[test]
fn corrsion_reads_mental_for_every_carrier() {
    assert_eq!(motavia_record(CORRSION).power_stat, 0x02);
    assert_eq!(motavia_record(CORRSION).power, 64);
    for (carrier, expected) in [
        (HAUNT, [64u16, 61, 89]),
        (SPECTOR, [83, 80, 118]),
        (CHAOS_SORCR, [91, 88, 130]),
        (ILLUSIONST, [100, 97, 143]),
        (IMAGIO_MAGE, [110, 107, 158]),
        (DARK_FORCE3, [160, 157, 233]),
    ] {
        let (damage, drawn) = resolve_on(&carrier, CORRSION);
        assert_eq!(drawn, 48, "{}: three runs of sixteen", carrier.symbol);
        assert_eq!(damage, expected, "{}", carrier.symbol);
        assert_eq!(
            expected,
            [
                record_damage(u16::from(carrier.mental), 0, 2, 64),
                record_damage(u16::from(carrier.mental), 3, 2, 64),
                record_damage(u16::from(carrier.mental), 7, 3, 64),
            ],
            "{}: the formula's own terms",
            carrier.symbol
        );
    }
}

/// The `AllParty` walk resolves every living party slot in slot order and
/// nothing else: a dead slot draws nothing, an empty one is not walked, and the
/// skill event is emitted whatever the walk finds.
#[test]
fn the_all_party_walk_covers_living_slots_in_order() {
    for (dead, expected) in [(&[2u8][..], 2usize), (&[1, 2, 3][..], 0)] {
        let data = zio_data(&[JUZA], &[FORCEFLASH]);
        let mut r = zio_roster(&data, &JUZA);
        for slot in dead {
            kill(&mut r, *slot);
        }
        let mut rolls = SliceRolls::new(&[0]);
        let mut events = Vec::new();
        assert!(resolve_damage_skill(
            &mut r,
            id(6),
            FORCEFLASH,
            Some(id(1)),
            &data,
            &mut rolls,
            &mut events
        ));
        assert_eq!(
            rolls.drawn(),
            expected * 16,
            "dead slots {dead:?} draw nothing"
        );
        let hits: Vec<FighterId> = events
            .iter()
            .filter_map(|event| match event {
                BattleEvent::Resolved { target, .. } => Some(*target),
                _ => None,
            })
            .collect();
        let living: Vec<FighterId> = (1..=3u8)
            .filter(|slot| !dead.contains(slot))
            .map(id)
            .collect();
        assert_eq!(hits, living, "dead slots {dead:?}");
        assert!(matches!(
            events.first(),
            Some(BattleEvent::EnemySkillUsed { .. })
        ));
    }
}

/// One round against one carrier, driven through the engine's own ability roll
/// on a fully specified stream: nine ordering draws, the four enemy-target
/// draws, the ability index (1, and every slot holds the ability under test)
/// and three times 16 damage draws — 62 in all, with no swing. Party agility 1
/// keeps the enemy first in the queue, so Defend has raised nobody's
/// resistance yet.
fn zio_round(carrier: &Carrier, ability: u8) -> (Vec<BattleEvent>, usize) {
    let carrier = &Carrier {
        abilities: [ability; 8],
        ..*carrier
    };
    let data = zio_data(&[*carrier], &[ability]);
    let formation = FormationRecord {
        id: 0,
        ambush_chance: 0,
        run_chance: 254,
        drop_rate: 0,
        drop_item: None,
        enemies: vec![FormationEnemy {
            slot: 1,
            enemy_id: carrier.enemy_id,
            position: 20,
        }],
    };
    let party = [fixtures::alys(), fixtures::chaz(), fixtures::hahn()].map(|record| {
        let mut member = PartyMember::seat(&record, &data).unwrap();
        member.stats.curr_hp = 400;
        member.stats.max_hp = 400;
        member.stats.agility.battle = 1;
        member.stats.element_props = [2; ELEMENT_SLOTS];
        member
    });
    let (mut battle, _) = Battle::start(
        &formation,
        party.to_vec(),
        &data,
        false,
        &mut SliceRolls::new(&[0]),
    )
    .unwrap();
    let mut stream = vec![0; 13];
    stream.push(1);
    stream.extend([0; 48]);
    let mut rolls = SliceRolls::new(&stream);
    let events = battle
        .round(
            &RoundOrders::Commands(vec![Command::Defend; 3]),
            &data,
            &mut rolls,
        )
        .unwrap();
    (events, rolls.drawn())
}

/// Every added route is reachable through the engine's own ability roll, not
/// just through a direct call: the ability resolves against the whole party,
/// the caster never swings, and no other ability of the same carrier's list gets
/// in the way.
#[test]
fn every_zio_route_resolves_in_an_ordinary_round() {
    for (carrier, ability) in [
        (TECH_MASTER, ZAN),
        (JUZA, ZAN),
        (JUZA, FORCEFLASH),
        (GRENERIS, FORCEFLASH),
        (RADHIN, FORCEFLASH),
        (HAUNT, CORRSION),
        (SPECTOR, CORRSION),
        (CHAOS_SORCR, CORRSION),
        (ILLUSIONST, CORRSION),
        (IMAGIO_MAGE, CORRSION),
        (DARK_FORCE3, CORRSION),
    ] {
        let (events, drawn) = zio_round(&carrier, ability);
        assert_eq!(
            drawn, 62,
            "{} {ability:#04X}: three requests, no swing",
            carrier.symbol
        );
        assert!(
            events.iter().any(|e| matches!(
                e,
                BattleEvent::EnemySkillUsed { actor, skill, .. }
                    if *actor == id(6) && *skill == ability
            )),
            "{} {ability:#04X}: {events:?}",
            carrier.symbol
        );
        let hits: Vec<FighterId> = events
            .iter()
            .filter_map(|e| match e {
                BattleEvent::Resolved { target, .. } => Some(*target),
                _ => None,
            })
            .collect();
        assert_eq!(
            hits,
            [id(1), id(2), id(3)],
            "{} {ability:#04X}: the whole party, in slot order",
            carrier.symbol
        );
        assert!(
            !events.iter().any(|e| matches!(
                e,
                BattleEvent::Attacked { .. } | BattleEvent::UnsupportedAbility { .. }
            )),
            "{} {ability:#04X}: dispatched, so nothing falls back: {events:?}",
            carrier.symbol
        );
    }
}

/// The pairs the damage half leaves to later lanes, and why each one is not a
/// row. Every case here is refused with nothing drawn, nothing emitted and no
/// state moved — which is what keeps the caller on the explicit
/// [`BattleEvent::UnsupportedAbility`] path instead of resolving a turn the
/// chain reading does not support:
///
/// - **133 ProfoundDarkness1's `$21`**, covered in
///   `enemy_damage_firebreath_tests`.
/// - **`$4C` EVIL EYE, `$2A` RIMIT and `$2F` VOL**, whose chains make no
///   `move.w #$C` request on any carrier read so far: the effect handler
///   (`$07` SleepParalyze, `$02` Death) is the whole turn, and it belongs to
///   the status-effect lane. A row here would report damage the cartridge never
///   deals.
#[test]
fn the_deferred_pairs_are_refused() {
    for (carrier, ability) in [
        (HAUNT, EVIL_EYE),
        (SPECTOR, EVIL_EYE),
        (TECH_PLANT, RIMIT),
        (GRENERIS, RIMIT),
        (GRENERIS, VOL),
    ] {
        assert!(
            super::all()
                .all(|route| { route.enemy_id != carrier.enemy_id || route.ability != ability }),
            "{} {ability:#04X} must not be a listed route",
            carrier.symbol
        );
        let data = zio_data(&[carrier], &[ability]);
        let mut r = zio_roster(&data, &carrier);
        let before = r.clone();
        let mut rolls = SliceRolls::new(&[0]);
        let mut events = Vec::new();
        assert!(
            !resolve_damage_skill(
                &mut r,
                id(6),
                ability,
                Some(id(2)),
                &data,
                &mut rolls,
                &mut events
            ),
            "{} {ability:#04X}: the effect handler is the whole turn",
            carrier.symbol
        );
        assert_eq!(r, before, "{} {ability:#04X}", carrier.symbol);
        assert_eq!(rolls.drawn(), 0, "{} {ability:#04X}", carrier.symbol);
        assert!(events.is_empty(), "{} {ability:#04X}", carrier.symbol);
    }
}

/// The registry itself: every row of every family is reachable through
/// [`super::all`], no `(enemy, ability)` pair appears twice, and the four
/// ability ids this lane added are all present. A family a later lane appends
/// to has to keep both properties.
#[test]
fn every_row_is_listed_once_and_the_new_families_are_reachable() {
    let rows: Vec<&DamageRoute> = super::all().collect();
    assert_eq!(
        rows.len(),
        DAMAGE_SKILL_ROUTES
            .iter()
            .map(|family| family.len())
            .sum::<usize>(),
        "the registry is the whole table"
    );
    for (index, row) in rows.iter().enumerate() {
        assert!(
            !rows[..index]
                .iter()
                .any(|other| other.enemy_id == row.enemy_id && other.ability == row.ability),
            "({}, {:#04X}) is listed twice",
            row.enemy_id,
            row.ability
        );
    }
    for (enemy_id, ability) in [
        (58, FIREBREATH),
        (83, FIREBREATH),
        (100, ZAN),
        (114, ZAN),
        (115, FORCEFLASH),
        (116, FORCEFLASH),
        (106, CORRSION),
        (132, CORRSION),
    ] {
        assert!(
            rows.iter()
                .any(|row| row.enemy_id == enemy_id && row.ability == ability),
            "({enemy_id}, {ability:#04X}) is missing from the registry"
        );
    }
    assert!(DAMAGE_SKILL_ROUTES.iter().all(|family| !family.is_empty()));
}
