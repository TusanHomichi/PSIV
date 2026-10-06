//! Lane A5's engine paths: Dark Force 2's latch, Lashiec's REINFORCE latch and
//! COMBINE's reload. Constructed records; the inline formation is a stand-in
//! with the shape `psiv_tools.formations` decodes.
use super::*;
use crate::battle::{EnemyRecord, EnemySkill, FormationEnemy, InlineFormation};

fn skill(id: u8, effect: u8, target: u8) -> EnemySkill {
    EnemySkill {
        id,
        name: format!("constructed {id:#04X}"),
        effect,
        power_stat: 0,
        target,
        power: 0,
        resistance: 0,
        element: 1,
    }
}

fn formation(enemies: &[(u8, u16)], run_chance: u8) -> FormationRecord {
    FormationRecord {
        id: 999,
        ambush_chance: 0,
        run_chance,
        drop_rate: 0,
        drop_item: None,
        enemies: enemies
            .iter()
            .map(|&(slot, enemy_id)| FormationEnemy {
                slot,
                enemy_id,
                position: 0,
            })
            .collect(),
    }
}

fn enemy(id: u16, regular: u8, conditions: [u8; 4], conditional: [u8; 4]) -> EnemyRecord {
    let mut record = fixtures::zoran_bult();
    record.id = id;
    record.name = format!("ENEMY{id}");
    record.hp = 999;
    record.regular_abilities = [regular; 8];
    record.condition_ids = conditions;
    record.conditional_abilities = conditional;
    record
}

#[test]
fn dark_force_2_spends_its_first_turn_on_the_reveal_and_lowers_the_latch() {
    let data = fixtures::data()
        .with_enemies([enemy(131, 0x64, [0; 4], [0; 4])])
        .with_enemy_skills([skill(0x64, 1, 8)]);
    let (mut battle, started) = Battle::start(
        &formation(&[(1, 131)], 0xFE),
        basement_party(&data),
        &data,
        true,
        &mut SliceRolls::new(&[20]),
    )
    .unwrap();
    assert!(matches!(
        started[0],
        BattleEvent::Started {
            priority: Priority::Ambush,
            ..
        }
    ));
    let mut rolls = SliceRolls::new(&[1]);
    let mut events = Vec::new();
    assert!(
        battle
            .roll_enemy_ability(id(6), Some(id(2)), &data, &mut rolls, &mut events)
            .unwrap()
    );
    assert_eq!(
        rolls.drawn(),
        1,
        "the ordinary roll and nothing from `$83C`"
    );
    assert_eq!(
        events,
        [BattleEvent::FirstZioAction {
            actor: id(6),
            action: crate::battle::FirstZioAction::DarkForceReveal,
            target: None,
        }]
    );
    assert!(!battle.scripted_latch);
    // With the latch down the same roll reaches SHDWBREATH's single request.
    events.clear();
    let mut rolls = SliceRolls::new(&[2]);
    assert!(
        battle
            .roll_enemy_ability(id(6), Some(id(2)), &data, &mut rolls, &mut events)
            .unwrap()
    );
    assert_eq!(rolls.drawn(), 1 + 16);
}

#[test]
fn reinforce_fires_once_a_battle_at_a_quarter_hp() {
    let data = fixtures::data()
        .with_enemies([enemy(128, 0x5F, [0x11; 4], [0x62; 4])])
        .with_enemy_skills([skill(0x5F, 1, 9), skill(0x62, 0x2B, 3)]);
    let (mut battle, _) = Battle::start(
        &formation(&[(1, 128)], 0xFE),
        basement_party(&data),
        &data,
        true,
        &mut SliceRolls::new(&[20]),
    )
    .unwrap();
    battle.roster.get_mut(id(6)).unwrap().stats.curr_hp = 999 / 4;
    let agility = battle.roster.get(id(6)).unwrap().stats.agility.battle;
    let mut events = Vec::new();
    assert!(
        battle
            .roll_enemy_ability(
                id(6),
                Some(id(2)),
                &data,
                &mut SliceRolls::new(&[1]),
                &mut events
            )
            .unwrap()
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, BattleEvent::EnemySkillUsed { skill: 0x62, .. }))
    );
    assert_eq!(
        battle.roster.get(id(6)).unwrap().stats.agility.battle,
        agility + 20
    );
    assert!(battle.ai_flags.reinforced);
    // Still at a quarter, but the latch holds the arm off: the rolled
    // THNDHALBRT runs instead.
    events.clear();
    assert!(
        battle
            .roll_enemy_ability(
                id(6),
                Some(id(2)),
                &data,
                &mut SliceRolls::new(&[2]),
                &mut events
            )
            .unwrap()
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, BattleEvent::EnemySkillUsed { skill: 0x5F, .. }))
    );
}

#[test]
fn combine_reloads_the_side_from_the_inline_record_and_takes_its_run_byte() {
    let data = fixtures::data()
        .with_enemies([
            enemy(84, 0x21, [0x0D; 4], [0x3A; 4]),
            enemy(86, 0x33, [0x0E; 4], [0x3B; 4]),
            enemy(87, 0, [0; 4], [0; 4]),
        ])
        .with_enemy_skills([skill(0x3A, 0x1F, 6), skill(0x3B, 0x1F, 4)])
        .with_inline_formations([InlineFormation {
            label: "loc_23D00".into(),
            run_chance: 7,
            enemies: vec![FormationEnemy {
                slot: 1,
                enemy_id: 87,
                position: 0x14,
            }],
        }]);
    let (mut battle, _) = Battle::start(
        &formation(&[(1, 84), (2, 86)], 0x3C),
        basement_party(&data),
        &data,
        false,
        &mut SliceRolls::new(&[20]),
    )
    .unwrap();
    let mut rolls = SliceRolls::new(&[1]);
    let mut events = Vec::new();
    assert!(
        battle
            .roll_enemy_ability(id(6), None, &data, &mut rolls, &mut events)
            .unwrap()
    );
    assert_eq!(rolls.drawn(), 1, "no call between the roll and the reload");
    let [
        BattleEvent::EnemiesFused {
            removed,
            fighter,
            enemy_id,
            position,
            ..
        },
    ] = events.as_slice()
    else {
        panic!("one reload event, got {events:?}");
    };
    assert_eq!(removed, &vec![id(6), id(7)]);
    assert_eq!((*fighter, *enemy_id, *position), (id(6), 87, 0x14));
    assert_eq!(battle.roster.side(Side::Enemy).count(), 1);
    assert_eq!(
        battle.run_chance,
        Some(7),
        "Enemy_Run_Chance is the record's"
    );

    // Without the record in the data the reload is an error, not a guess.
    let bare = fixtures::data()
        .with_enemies([
            enemy(84, 0x21, [0x0D; 4], [0x3A; 4]),
            enemy(86, 0x33, [0x0E; 4], [0x3B; 4]),
        ])
        .with_enemy_skills([skill(0x3A, 0x1F, 6)]);
    let (mut battle, _) = Battle::start(
        &formation(&[(1, 84), (2, 86)], 0x3C),
        basement_party(&bare),
        &bare,
        false,
        &mut SliceRolls::new(&[20]),
    )
    .unwrap();
    assert_eq!(
        battle.roll_enemy_ability(
            id(6),
            None,
            &bare,
            &mut SliceRolls::new(&[1]),
            &mut Vec::new()
        ),
        Err(BattleDataError::UnknownInlineFormation("loc_23D00"))
    );
}

#[test]
fn a_lone_blade_right_breathes_fire_instead_of_combining() {
    // The corrected `$0D` arm: no HakenLeft, no COMBINE.
    let data = fixtures::data()
        .with_enemies([enemy(84, 0x21, [0x0D; 4], [0x3A; 4])])
        .with_enemy_skills([skill(0x21, 1, 8), skill(0x3A, 0x1F, 6)]);
    let (mut battle, _) = Battle::start(
        &formation(&[(1, 84)], 0x3C),
        basement_party(&data),
        &data,
        false,
        &mut SliceRolls::new(&[20]),
    )
    .unwrap();
    let mut events = Vec::new();
    assert!(
        battle
            .roll_enemy_ability(
                id(6),
                Some(id(2)),
                &data,
                &mut SliceRolls::new(&[1]),
                &mut events
            )
            .unwrap()
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, BattleEvent::EnemySkillUsed { skill: 0x21, .. }))
    );
}
