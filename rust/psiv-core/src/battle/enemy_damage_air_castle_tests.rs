//! Constructed controls for lane A5's damage arms (`routes/air_castle.rs`).
//! No retail stat or ability record is transcribed here: the records carry the
//! route's own effect byte and otherwise chosen numbers.

use super::tests::id;
use super::*;
use crate::battle::{EnemySkill, PartyMember, SliceRolls, fixtures, status};

fn data(enemy: u16, ability: u8, target: u8) -> BattleData {
    let mut record = fixtures::zoran_bult();
    record.id = enemy;
    record.hp = 900;
    record.mental = 40;
    record.strength = 40;
    record.attack = 40;
    fixtures::data()
        .with_enemies([record])
        .with_enemy_skills([EnemySkill {
            id: ability,
            name: "constructed".into(),
            effect: 0x01,
            power_stat: 0x82,
            target,
            power: 10,
            resistance: 7,
            element: 3,
        }])
}

/// Two living party members and a fallen third; the carrier in enemy slot 1
/// and two more of it beside it.
fn roster(data: &BattleData, enemy: u16) -> Roster {
    let mut roster = Roster::new();
    for _ in 0..3 {
        let mut stats = PartyMember::seat(&fixtures::alys(), data).unwrap().stats;
        stats.curr_hp = 999;
        stats.max_hp = 999;
        stats.mental_defence.battle = 7;
        stats.element_props = [4; crate::battle::ELEMENT_SLOTS];
        roster.add_party_member(0, "constructed".into(), stats);
    }
    roster.get_mut(id(3)).unwrap().mark_defeated();
    for slot in 1..=3 {
        roster.add_enemy(slot, data.enemy(enemy).unwrap());
    }
    roster
}

fn run(
    enemy: u16,
    ability: u8,
    target: u8,
    draws: &[u16],
) -> (bool, Roster, Vec<BattleEvent>, usize) {
    let data = data(enemy, ability, target);
    let mut roster = roster(&data, enemy);
    let mut rolls = SliceRolls::new(draws);
    let mut events = Vec::new();
    let resolved = resolve_damage_skill(
        &mut roster,
        id(6),
        ability,
        Some(id(2)),
        &data,
        &mut rolls,
        &mut events,
    );
    (resolved, roster, events, rolls.drawn())
}

fn damaged(events: &[BattleEvent]) -> Vec<FighterId> {
    events
        .iter()
        .filter_map(|event| match event {
            BattleEvent::Resolved { target, .. } => Some(*target),
            _ => None,
        })
        .collect()
}

#[test]
fn every_a5_carrier_takes_its_cited_request_shape() {
    // (carriers, ability, all party), enumerated from the cited arms rather
    // than from the registry.
    let families: &[(&[u16], u8, bool)] = &[
        (&[70, 71, 72], 0x2C, true),
        (&[77, 101, 122, 123, 124, 125], 0x35, true),
        (&[128], 0x5F, true),
        (&[131], 0x64, false),
        (&[131], 0x65, true),
    ];
    for &(carriers, ability, all_party) in families {
        for &enemy in carriers {
            let (resolved, _, events, drawn) = run(enemy, ability, 9, &[0]);
            assert!(resolved, "{enemy}/{ability:#x}");
            let expected = if all_party {
                vec![id(1), id(2)]
            } else {
                vec![id(2)]
            };
            assert_eq!(damaged(&events), expected, "{enemy}/{ability:#x}");
            assert_eq!(
                drawn,
                if all_party { 32 } else { 16 },
                "{enemy}/{ability:#x}"
            );
        }
    }
}

#[test]
fn gra_takes_its_sparks_before_the_two_living_members_damage_runs() {
    for (enemy, ability) in [(73, 0x31), (74, 0x31), (74, 0x32)] {
        let (resolved, _, events, drawn) = run(enemy, ability, 9, &[0]);
        assert!(resolved);
        assert_eq!(damaged(&events), vec![id(1), id(2)]);
        // Two sparked members at countdown 6 take twelve calls
        // (`sparks::draws`), then sixteen per living member.
        let mut sparks = SliceRolls::new(&[0]);
        assert_eq!(super::sparks::draws(2, &mut sparks), 12);
        assert_eq!(drawn, 12 + 32);
    }
}

#[test]
fn another_gate_takes_its_236_calls_before_the_damage_runs() {
    let (resolved, _, events, drawn) = run(128, 0x61, 9, &[0]);
    assert!(resolved);
    assert_eq!(damaged(&events), vec![id(1), id(2)]);
    assert_eq!(drawn, 59 * 4 + 32);
}

#[test]
fn thunder_blast_paralyzes_the_three_xe_a_thouls_and_strikes_the_party() {
    let (resolved, roster, events, drawn) = run(123, 0x5C, 9, &[0]);
    assert!(resolved);
    assert_eq!(drawn, 32);
    assert_eq!(damaged(&events), vec![id(1), id(2)]);
    for slot in 6..=8 {
        assert_ne!(
            roster.get(id(slot)).unwrap().stats.status & status::PARALYZED,
            0,
            "enemy slot {slot}"
        );
        assert!(events.contains(&BattleEvent::StatusInflicted {
            actor: id(6),
            target: id(slot),
            status: status::PARALYZED,
        }));
    }
}

#[test]
fn a_sealed_gizan_caster_fizzles_with_no_call() {
    for enemy in [77, 101, 122, 123, 124, 125] {
        sealed_gizan(enemy);
    }
}

fn sealed_gizan(enemy: u16) {
    let data = data(enemy, 0x35, 9);
    let mut roster = roster(&data, enemy);
    roster.get_mut(id(6)).unwrap().stats.status |= status::TECH_SEALED;
    let mut rolls = SliceRolls::new(&[0]);
    let mut events = Vec::new();
    assert!(resolve_damage_skill(
        &mut roster,
        id(6),
        0x35,
        Some(id(2)),
        &data,
        &mut rolls,
        &mut events
    ));
    assert_eq!(rolls.drawn(), 0);
    assert!(matches!(
        events.as_slice(),
        [BattleEvent::EnemySkillUsed { skill: 0x35, .. }]
    ));
    assert_eq!(roster.get(id(1)).unwrap().stats.curr_hp, 999);
}

#[test]
fn the_unsealable_routes_ignore_the_caster_s_seal() {
    // Only the sixteen objects that test it fizzle: AIRSLASH's `$2A0` does not.
    let data = data(71, 0x2C, 9);
    let mut roster = roster(&data, 71);
    roster.get_mut(id(6)).unwrap().stats.status |= status::TECH_SEALED;
    let mut rolls = SliceRolls::new(&[0]);
    let mut events = Vec::new();
    assert!(resolve_damage_skill(
        &mut roster,
        id(6),
        0x2C,
        None,
        &data,
        &mut rolls,
        &mut events
    ));
    assert_eq!(damaged(&events), vec![id(1), id(2)]);
}

#[test]
fn the_status_arms_of_the_same_bosses_are_not_damage_routes() {
    // POSESSION, REINFORCE and EVIL EYE make no `move.w #$C` request; their
    // handlers are `enemy_effect`'s.
    for (enemy, ability) in [(128, 0x60), (128, 0x62), (131, 0x4C)] {
        let (resolved, _, events, drawn) = run(enemy, ability, 8, &[0]);
        assert!(!resolved, "{enemy}/{ability:#x}");
        assert!(events.is_empty());
        assert_eq!(drawn, 0);
    }
}

#[test]
fn exactly_the_routes_whose_object_tests_the_caster_seal_are_sealable() {
    // Read from the sixteen `btst #4, $16(a1)` sites in object code
    // (`ps4.asm:38047` ... `56505`) and the objects that reach them, through
    // their shared caster preludes (`docs/battle/ENEMY_ABILITIES_AIR_CASTLE.md`).
    let mut expected = vec![
        (71, 0x2E),
        (77, 0x2E),
        (101, 0x2E),
        (122, 0x2E),
        (123, 0x2E),
        (99, 0x40),
        (100, 0x40),
        (114, 0x40),
        (99, 0x44),
        (100, 0x44),
        (114, 0x44),
        (100, 0x47),
        (114, 0x47),
        (114, 0x56),
        (115, 0x56),
        (116, 0x56),
        (77, 0x35),
        (101, 0x35),
        (122, 0x35),
        (123, 0x35),
        (124, 0x35),
        (125, 0x35),
        // Lane A6: GIFOI's two objects test it too - TechUser's prelude
        // `loc_21C30` (45257) for 101, DElmLars's `loc_291D2` (54387) for 124.
        (101, 0x48),
        (124, 0x48),
    ];
    expected.sort_unstable();
    let mut sealable: Vec<(u16, u8)> = all()
        .filter(|route| route.sealable)
        .map(|route| (route.enemy_id, route.ability))
        .collect();
    sealable.sort_unstable();
    assert_eq!(sealable, expected);
}

#[test]
fn a_sealed_caster_s_foi_fizzles_but_hew_gilla_s_giwat_does_not_check() {
    // TechUser's shared prelude `loc_21C30` (`ps4.asm:45255-45258`) versus
    // HewGilla's `$390`, which has no seal test.
    for (enemy, ability, fizzles) in [(99, 0x44, true), (91, 0x2E, false)] {
        let data = data(enemy, ability, 8);
        let mut roster = roster(&data, enemy);
        roster.get_mut(id(6)).unwrap().stats.status |= status::TECH_SEALED;
        let mut rolls = SliceRolls::new(&[0]);
        let mut events = Vec::new();
        assert!(resolve_damage_skill(
            &mut roster,
            id(6),
            ability,
            Some(id(2)),
            &data,
            &mut rolls,
            &mut events
        ));
        assert_eq!(damaged(&events).is_empty(), fizzles, "{enemy}/{ability:#x}");
    }
}
