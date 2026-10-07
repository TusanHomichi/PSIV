//! `$21` FIREBREATH: the five carriers whose arm is proven, through
//! [`resolve_damage_skill`] and through `roll_enemy_ability`, and the carrier
//! the table leaves out.
//!
//! Every number below is pinned twice — once as the literal the record and the
//! carrier's own strength give it, and once through [`record_damage`], the
//! formula written out — so a change to either the fixtures or the cartridge
//! reading shows up as a failing pair.

use super::tests::*;
use super::*;
use crate::battle::{
    Battle, Command, ELEMENT_SLOTS, EnemySkill, FormationEnemy, FormationRecord, PartyMember,
    RoundOrders, SliceRolls, fixtures,
};

/// The five carriers of the FIREBREATH family, with the stat line
/// `generated/enemies.json` gives them. `$21` selects record byte 1 = `$01`,
/// strength, which `Enemy_DamageCharacter` (`ps4.asm:3775`) reads as a **byte**.
#[derive(Clone, Copy)]
struct Carrier {
    enemy_id: u16,
    symbol: &'static str,
    hp: u16,
    strength: u8,
    abilities: [u8; 8],
}

const FLAME_NEWT: Carrier = Carrier {
    enemy_id: 58,
    symbol: "FlameNewt",
    hp: 189,
    strength: 74,
    abilities: [
        FIREBREATH, FIREBREATH, FIREBREATH, FIREBREATH, FIREBREATH, FIREBREATH, 0x24, 0x24,
    ],
};
const STONE_HEADS: Carrier = Carrier {
    enemy_id: 59,
    symbol: "StoneHeads",
    hp: 220,
    strength: 69,
    abilities: [0, 0, 0, 0, FIREBREATH, FIREBREATH, FIREBREATH, FIREBREATH],
};
const RIPPER: Carrier = Carrier {
    enemy_id: 83,
    symbol: "Ripper",
    hp: 112,
    strength: 40,
    abilities: [0, 0, 0, 0, 0, FIREBREATH, FIREBREATH, FIREBREATH],
};
const BLADE_RIGHT: Carrier = Carrier {
    enemy_id: 84,
    symbol: "BladeRight",
    hp: 232,
    strength: 90,
    abilities: [0, 0, 0, 0, 0, FIREBREATH, FIREBREATH, FIREBREATH],
};
const GY_LAGUIAH: Carrier = Carrier {
    enemy_id: 117,
    symbol: "GyLaguiah",
    hp: 2580,
    strength: 100,
    abilities: [0, 0, 0, 0, FIREBREATH, FIREBREATH, FIREBREATH, FIREBREATH],
};
const FIREBREATH_CARRIERS: [Carrier; 5] =
    [FLAME_NEWT, STONE_HEADS, RIPPER, BLADE_RIGHT, GY_LAGUIAH];

/// `$21` FIREBREATH: the record `generated/enemy_skills.json` decodes, as
/// `enemy_damage_tests::motavia_record` builds it.
fn firebreath_record() -> EnemySkill {
    motavia_record(FIREBREATH)
}

fn carrier_record(carrier: &Carrier) -> crate::battle::EnemyRecord {
    let mut record = fixtures::zoran_bult();
    record.id = carrier.enemy_id;
    record.name = carrier.symbol.into();
    record.hp = carrier.hp;
    record.strength = carrier.strength;
    record.mental = 1;
    record.attack = 1;
    record.agility = 100;
    record.regular_abilities = carrier.abilities;
    record.condition_ids = [0; 4];
    record
}

/// Carrier records plus the record the pair under test runs.
fn firebreath_data(carriers: &[Carrier]) -> BattleData {
    fixtures::data()
        .with_enemies(carriers.iter().map(carrier_record))
        .with_enemy_skills([firebreath_record()])
}

/// Three party members with defense 7, no magic defense and every element
/// factor 2, so a number below depends only on the carrier's strength.
fn firebreath_roster(data: &BattleData, carrier: &Carrier) -> Roster {
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

/// Resolves one listed pair once and pins the request it makes: one damage run,
/// an `EnemySkillUsed` and a single `Resolved` against the chosen target.
fn resolve_pair(carrier: &Carrier) -> u16 {
    let data = firebreath_data(&[*carrier]);
    let mut r = firebreath_roster(&data, carrier);
    let mut rolls = SliceRolls::new(&[0]);
    let mut events = Vec::new();
    assert!(
        resolve_damage_skill(
            &mut r,
            id(6),
            FIREBREATH,
            Some(id(2)),
            &data,
            &mut rolls,
            &mut events
        ),
        "{} is a listed route",
        carrier.symbol
    );
    assert_eq!(
        rolls.drawn(),
        16,
        "{}: one request, one run of sixteen",
        carrier.symbol
    );
    assert_eq!(
        events.len(),
        2,
        "{}: ability and damage only: {events:?}",
        carrier.symbol
    );
    assert!(
        matches!(
            events[0],
            BattleEvent::EnemySkillUsed { actor, skill, .. }
                if actor == id(6) && skill == FIREBREATH
        ),
        "{}: {events:?}",
        carrier.symbol
    );
    let BattleEvent::Resolved {
        actor,
        target,
        damage: Some(damage),
        remaining_hp,
        ..
    } = events[1]
    else {
        panic!("{}: {events:?}", carrier.symbol);
    };
    assert_eq!(actor, id(6), "{}", carrier.symbol);
    assert_eq!(target, id(2), "{}: the chosen target alone", carrier.symbol);
    assert_eq!(remaining_hp, 400 - damage, "{}", carrier.symbol);
    assert_eq!(
        r.get(id(1)).unwrap().stats.curr_hp,
        400,
        "{}: not the whole party",
        carrier.symbol
    );
    assert_eq!(
        r.get(id(3)).unwrap().stats.curr_hp,
        400,
        "{}",
        carrier.symbol
    );
    damage
}

/// `$21` FIREBREATH: selector 1 is `strength`, a byte, and the record's
/// resistance selector is `$07` (magic defense, 0 on this fixture), element `3`
/// (fire, factor 2). Each carrier's own strength decides the number, and the
/// request lands on the drawn target only.
#[test]
fn firebreath_reads_strength_on_every_carrier() {
    assert_eq!(firebreath_record().power_stat, 0x01);
    assert_eq!(firebreath_record().power, 32);
    assert_eq!(firebreath_record().resistance, 0x07);
    assert_eq!(firebreath_record().element, 3);
    for (carrier, expected) in [
        (FLAME_NEWT, 73u16),
        (STONE_HEADS, 70),
        (RIPPER, 54),
        (BLADE_RIGHT, 82),
        (GY_LAGUIAH, 88),
    ] {
        assert_eq!(resolve_pair(&carrier), expected, "{}", carrier.symbol);
        assert_eq!(
            expected,
            record_damage(u16::from(carrier.strength), 0, 2, 32),
            "{}: the formula's own terms",
            carrier.symbol
        );
        // Selector 1 is a byte read that the formula shifts right twice, so a
        // neighbouring stat byte would not show up in every case; eight points
        // of it do, on every carrier here.
        assert_ne!(
            expected,
            record_damage(u16::from(carrier.strength) + 8, 0, 2, 32),
            "{}: the number is this carrier's own strength",
            carrier.symbol
        );
    }
}

/// One round against one carrier, driven through the engine's own ability roll
/// on a fully specified stream: nine ordering draws, the four enemy-target
/// draws, the ability index (1, and every slot holds FIREBREATH) and the 16
/// damage draws — 30 in all, with no swing.
fn firebreath_round(carrier: &Carrier) -> (Vec<BattleEvent>, usize) {
    // Every regular slot holds FIREBREATH, so the index roll lands on it. The
    // real lists above are what the gate is proven against; a zero slot here
    // would dispatch the plain attack instead.
    let carrier = &Carrier {
        abilities: [FIREBREATH; 8],
        ..*carrier
    };
    let data = firebreath_data(&[*carrier]);
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
    stream.extend([0; 16]);
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

/// Every FIREBREATH row is reachable through the engine's own ability roll, not
/// just through a direct call: the ability resolves against the drawn target,
/// the caster never swings, and no other ability of the carrier's list gets in
/// the way.
#[test]
fn every_firebreath_route_resolves_in_an_ordinary_round() {
    for carrier in FIREBREATH_CARRIERS {
        let (events, drawn) = firebreath_round(&carrier);
        assert_eq!(drawn, 30, "{}: one request, no swing", carrier.symbol);
        assert!(
            events.iter().any(|e| matches!(
                e,
                BattleEvent::EnemySkillUsed { actor, skill, .. }
                    if *actor == id(6) && *skill == FIREBREATH
            )),
            "{}: {events:?}",
            carrier.symbol
        );
        let hits: Vec<FighterId> = events
            .iter()
            .filter_map(|event| match event {
                BattleEvent::Resolved { target, .. } => Some(*target),
                _ => None,
            })
            .collect();
        assert_eq!(
            hits.len(),
            1,
            "{}: the single-target class resolves once: {events:?}",
            carrier.symbol
        );
        assert!(
            !events.iter().any(|e| matches!(
                e,
                BattleEvent::Attacked { .. } | BattleEvent::UnsupportedAbility { .. }
            )),
            "{}: dispatched, so nothing falls back: {events:?}",
            carrier.symbol
        );
    }
}

/// 133 ProfoundDarkness1 carries `$21` in three of its eight regular slots, and
/// `EnemyAttack_ProfoundDarkness1` (`ps4.asm:19823`) tests the scripted-battle
/// flag `$FFFFEE87` before it dispatches anything: with the flag set it clears
/// `$24(a4)` and loads object `$864` instead. The port models no battle objects,
/// so it cannot tell which arm the cartridge takes and leaves the pair out of
/// the table — the `$21` arm below the gate stays for a lane that models the
/// flag. A row here would resolve FIREBREATH on a turn the cartridge spends on
/// `$864`.
#[test]
fn the_gated_firebreath_carrier_is_refused() {
    let carrier = Carrier {
        enemy_id: 133,
        symbol: "ProfoundDarkness1",
        hp: 8240,
        strength: 194,
        abilities: [
            FIREBREATH, FIREBREATH, FIREBREATH, 0x22, 0x22, 0x64, 0x64, 0x64,
        ],
    };
    // Lane A6 modelled the latch (`scripted_flag::first_action`), so the
    // engine spends the first turn on `$864` before the roll reaches this
    // table (`engine_tests_endgame.rs`), and the row below the gate - `$870`,
    // one request through `$874`'s `loc_24A6C` - is a route like the others.
    let route = super::all()
        .find(|route| route.enemy_id == 133 && route.ability == FIREBREATH)
        .expect("133 ProfoundDarkness1's FIREBREATH is a route");
    assert_eq!(route.class, DamageClass::Single);
    let data = firebreath_data(&[carrier]);
    let mut r = firebreath_roster(&data, &carrier);
    let mut rolls = SliceRolls::new(&[0]);
    let mut events = Vec::new();
    assert!(resolve_damage_skill(
        &mut r,
        id(6),
        FIREBREATH,
        Some(id(2)),
        &data,
        &mut rolls,
        &mut events
    ));
    assert_eq!(rolls.drawn(), 16, "one request, no object calls");
}
