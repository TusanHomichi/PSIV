//! Lane A6 at the engine: Profound Darkness's latch and form changes, the
//! trees' first action, the new heal targets, the reloads and the draw counts
//! of the new damage chains. Constructed records only.
use super::*;
use crate::battle::{EnemyRecord, EnemySkill, FirstZioAction, FormationEnemy};

fn record(
    enemy: u16,
    abilities: [u8; 8],
    conditions: [u8; 4],
    conditional: [u8; 4],
) -> EnemyRecord {
    let mut record = fixtures::zoran_bult();
    record.id = enemy;
    record.hp = 999;
    record.mental = 40;
    record.regular_abilities = abilities;
    record.condition_ids = conditions;
    record.conditional_abilities = conditional;
    record
}

fn skill(ability: u8, effect: u8, target: u8, power: u8) -> EnemySkill {
    EnemySkill {
        id: ability,
        name: format!("constructed {ability:#04X}"),
        effect,
        power_stat: 0x82,
        target,
        power,
        resistance: 0,
        element: 1,
    }
}

fn start(data: &BattleData, enemies: &[u16]) -> Battle {
    let formation = FormationRecord {
        id: 999,
        ambush_chance: 0,
        run_chance: 0,
        drop_rate: 0,
        drop_item: None,
        enemies: enemies
            .iter()
            .enumerate()
            .map(|(i, enemy_id)| FormationEnemy {
                slot: i as u8 + 1,
                enemy_id: *enemy_id,
                position: 0,
            })
            .collect(),
    };
    Battle::start(
        &formation,
        basement_party(data),
        data,
        false,
        &mut SliceRolls::new(&[20]),
    )
    .unwrap()
    .0
}

fn turn(
    battle: &mut Battle,
    data: &BattleData,
    actor: u8,
    draws: &[u16],
) -> (Vec<BattleEvent>, usize) {
    let mut rolls = SliceRolls::new(draws);
    let mut events = Vec::new();
    battle
        .roll_enemy_ability(id(actor), Some(id(2)), data, &mut rolls, &mut events)
        .unwrap();
    (events, rolls.drawn())
}

#[test]
fn profound_darkness_rises_then_changes_form_twice_keeping_its_ambush_bit() {
    let data = fixtures::data()
        .with_enemies([
            record(133, [0x21; 8], [2; 4], [0x67; 4]),
            record(134, [0x30; 8], [2; 4], [0x68; 4]),
            record(135, [0x4C; 8], [11; 4], [0x5E; 4]),
        ])
        .with_enemy_skills([
            skill(0x21, 1, 8, 32),
            skill(0x30, 1, 9, 128),
            skill(0x4C, 7, 8, 64),
            skill(0x5E, 1, 9, 112),
            skill(0x67, 0x1F, 0, 134),
            skill(0x68, 0x1F, 0, 135),
        ]);
    let mut battle = start(&data, &[133]);
    // `EnemyInit_ProfoundDarkness1` raised the latch: an enemy ambush that
    // set the Ambush reaction bit.
    assert!(battle.scripted_latch);
    let (events, drawn) = turn(&mut battle, &data, 6, &[1]);
    assert_eq!(drawn, 1, "the roll, nothing else");
    assert_eq!(
        events,
        [BattleEvent::FirstZioAction {
            actor: id(6),
            action: FirstZioAction::ProfoundDarknessRise,
            target: None,
        }]
    );
    // At half HP its condition writes `$67`: the form change, no draw.
    battle.roster.get_mut(id(6)).unwrap().stats.curr_hp = 400;
    let (events, drawn) = turn(&mut battle, &data, 6, &[2]);
    assert_eq!(drawn, 1);
    assert!(matches!(
        events[..],
        [BattleEvent::EnemyStatsReloaded {
            enemy_id: 134,
            hp: 999,
            ..
        }]
    ));
    let pd = battle.roster.get(id(6)).unwrap();
    assert_eq!(
        (pd.stats.enemy_id, pd.stats.curr_hp, pd.ability),
        (134, 999, 0)
    );
    battle.roster.get_mut(id(6)).unwrap().stats.curr_hp = 400;
    let (events, _) = turn(&mut battle, &data, 6, &[3]);
    assert!(matches!(
        events[..],
        [BattleEvent::EnemyStatsReloaded { enemy_id: 135, .. }]
    ));
    // The third form's first turn: arm `$0B` still sees the battle's Ambush
    // bit and writes MEGID.
    let (events, _) = turn(&mut battle, &data, 6, &[4]);
    assert!(matches!(
        events[0],
        BattleEvent::EnemySkillUsed { skill: 0x5E, .. }
    ));
    // The arm cleared the reaction byte: the next turn rolls EVIL EYE.
    assert_eq!(battle.roster.get(id(6)).unwrap().reaction_flags, 0);
}

#[test]
fn the_first_tree_to_act_roots_all_three_and_the_rest_swing() {
    let data = fixtures::data().with_enemies([record(129, [0; 8], [0; 4], [0; 4])]);
    let mut battle = start(&data, &[129, 129, 129]);
    assert!(battle.scripted_latch);
    let (events, drawn) = turn(&mut battle, &data, 7, &[1]);
    assert_eq!(drawn, 1);
    assert!(matches!(
        events[0],
        BattleEvent::FirstZioAction {
            action: FirstZioAction::TreesTakeRoot,
            ..
        }
    ));
    for slot in 6..=8 {
        assert_ne!(
            battle.roster.get(id(slot)).unwrap().stats.status & status::PARALYZED,
            0
        );
    }
    assert!(!battle.scripted_latch);
    // Negative control: with the latch down the next tree swings.
    let mut rolls = SliceRolls::new(&[1]);
    let mut events = Vec::new();
    assert!(
        !battle
            .roll_enemy_ability(id(8), Some(id(2)), &data, &mut rolls, &mut events)
            .unwrap()
    );
}

#[test]
fn gisar_heals_every_living_enemy_and_soldrfiends_gires_heals_itself() {
    let gisar = EnemySkill {
        id: 0x49,
        name: "GISAR".into(),
        effect: 0x12,
        power_stat: 0x82,
        target: 2,
        power: 16,
        resistance: 0,
        element: 0,
    };
    let gires = EnemySkill {
        id: 0x3E,
        target: 1,
        power: 64,
        name: "GIRES".into(),
        ..gisar.clone()
    };
    let data = fixtures::data()
        .with_enemies([
            record(116, [0x49; 8], [0; 4], [0; 4]),
            record(88, [0x3E; 8], [0; 4], [0; 4]),
        ])
        .with_enemy_skills([gisar, gires]);
    let mut battle = start(&data, &[116, 116, 88]);
    for slot in 6..=8 {
        battle.roster.get_mut(id(slot)).unwrap().stats.curr_hp = 10;
    }
    let (events, drawn) = turn(&mut battle, &data, 6, &[1]);
    let healed: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            BattleEvent::Healed { target, .. } => Some(target.get()),
            _ => None,
        })
        .collect();
    assert_eq!(healed, [6, 7, 8]);
    assert_eq!(drawn, 1 + 3 * 16);
    let (events, drawn) = turn(&mut battle, &data, 8, &[2]);
    let healed: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            BattleEvent::Healed { target, .. } => Some(target.get()),
            _ => None,
        })
        .collect();
    assert_eq!((healed.as_slice(), drawn), (&[8u8][..], 1 + 16));
}

#[test]
fn the_new_damage_chains_take_their_objects_calls_before_the_requests() {
    // (enemy, ability, the chain's own calls, requests on a three-member party)
    for (enemy, ability, calls, requests) in [
        (18u16, 0x09u8, 1usize, 3usize),
        (26, 0x0E, 8, 3),
        (135, 0x5E, 112, 3),
        (134, 0x61, 236, 3),
        (8, 0x05, 0, 1),
        (87, 0x3D, 0, 1),
    ] {
        let data = fixtures::data()
            .with_enemies([record(enemy, [ability; 8], [0; 4], [0; 4])])
            .with_enemy_skills([skill(ability, 1, 9, 10)]);
        let mut battle = start(&data, &[enemy]);
        battle.scripted_latch = false;
        let (events, drawn) = turn(&mut battle, &data, 6, &[1]);
        let hits = events
            .iter()
            .filter(|e| matches!(e, BattleEvent::Resolved { .. }))
            .count();
        assert_eq!(
            (hits, drawn),
            (requests, 1 + calls + 16 * requests),
            "{enemy}"
        );
    }
}

#[test]
fn the_new_reloads_seat_their_inline_records() {
    use crate::battle::InlineFormation;
    let seat = |slot: u8, enemy_id: u16| FormationEnemy {
        slot,
        enemy_id,
        position: 8 * slot,
    };
    for (carrier, ability, effect, label, seated) in [
        (23u16, 0x0Cu8, 0x1Fu8, "loc_1308C", vec![26u16]),
        (38, 0x1B, 0x25, "loc_1987E", vec![35, 35, 35, 35]),
        (94, 0x41, 0x1F, "loc_224E8", vec![80]),
    ] {
        let data = fixtures::data()
            .with_enemies([
                record(carrier, [ability; 8], [0; 4], [0; 4]),
                record(seated[0], [0; 8], [0; 4], [0; 4]),
            ])
            .with_enemy_skills([skill(ability, effect, 0, 0)])
            .with_inline_formations([InlineFormation {
                label: label.into(),
                run_chance: 0,
                enemies: seated
                    .iter()
                    .enumerate()
                    .map(|(i, e)| seat(i as u8 + 1, *e))
                    .collect(),
            }]);
        let mut battle = start(&data, &[carrier, carrier]);
        let (events, drawn) = turn(&mut battle, &data, 6, &[1]);
        assert_eq!(drawn, 1, "{carrier}");
        let fused = events
            .iter()
            .filter(|e| matches!(e, BattleEvent::EnemiesFused { .. }))
            .count();
        assert_eq!(fused, seated.len(), "{carrier}");
        let side: Vec<u16> = battle
            .roster
            .side(Side::Enemy)
            .map(|f| f.stats.enemy_id)
            .collect();
        assert_eq!(side, seated, "{carrier}");
    }
}
