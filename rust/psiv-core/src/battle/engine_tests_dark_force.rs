//! DarkForce1's shared latch, separate from its post-intro damage routes.
use super::*;
use crate::battle::{EnemySkill, FormationEnemy};

#[test]
fn an_exploding_enemy_is_removed_without_adding_its_rewards() {
    for enemy_id in [44, 50] {
        let mut record = fixtures::zoran_bult();
        record.id = enemy_id;
        record.hp = 500;
        record.regular_abilities = [0x18; 8];
        record.condition_ids = [0; 4];
        // Nonzero synthetic rewards make an accidental death payout visible.
        record.experience = 123;
        record.meseta = 45;
        let data = fixtures::data()
            .with_enemies([record])
            .with_enemy_skills([EnemySkill {
                id: 0x18,
                name: "constructed".into(),
                effect: 0x23,
                power_stat: 2,
                target: 8,
                power: 1,
                resistance: 7,
                element: 1,
            }]);
        let formation = FormationRecord {
            id: 999,
            ambush_chance: 0,
            run_chance: 0,
            drop_rate: 0,
            drop_item: None,
            enemies: vec![FormationEnemy {
                slot: 1,
                enemy_id,
                position: 0,
            }],
        };
        let (mut battle, _) = Battle::start(
            &formation,
            basement_party(&data),
            &data,
            false,
            &mut SliceRolls::new(&[20]),
        )
        .unwrap();
        let events = battle
            .round(
                &RoundOrders::Commands(vec![Command::Defend; 3]),
                &data,
                &mut SliceRolls::new(&[1]),
            )
            .unwrap();
        assert_eq!(battle.outcome(), Some(Outcome::Victory));
        assert_eq!((battle.pools.experience, battle.pools.meseta), (0, 0));
        let removed = battle.roster.get(id(6)).unwrap();
        assert!(!removed.active);
        assert_eq!(removed.stats.curr_hp, 500);
        assert!(!events.iter().any(|event| matches!(
            event,
            BattleEvent::Rewarded {
                experience_total: 1..,
                ..
            }
        )));
    }
}

#[test]
fn dark_force_intro_spends_the_rolled_turn_without_damage_then_lowers_the_latch() {
    for ability in [0x1C, 0x20, 0x63] {
        let mut record = fixtures::zoran_bult();
        record.id = 130;
        record.hp = 999;
        record.regular_abilities = [ability; 8];
        record.condition_ids = [0; 4];
        let data = fixtures::data()
            .with_enemies([record])
            .with_enemy_skills([EnemySkill {
                id: ability,
                name: "constructed".into(),
                effect: 1,
                power_stat: 2,
                target: 9,
                power: 1,
                resistance: 7,
                element: 1,
            }]);
        let formation = FormationRecord {
            id: 999,
            ambush_chance: 0,
            run_chance: 0,
            drop_rate: 0,
            drop_item: None,
            enemies: vec![FormationEnemy {
                slot: 1,
                enemy_id: 130,
                position: 0,
            }],
        };
        let (mut battle, started) = Battle::start(
            &formation,
            basement_party(&data),
            &data,
            false,
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
        assert_eq!(rolls.drawn(), 1, "ordinary roll, no intro-object draws");
        assert_eq!(
            events,
            [BattleEvent::FirstZioAction {
                actor: id(6),
                action: crate::battle::FirstZioAction::DarkForceCharge,
                target: None,
            }],
            "the intro is a presentation-only turn: one charge-up event, nothing resolved"
        );
        assert_eq!(battle.roster.get(id(6)).unwrap().ability, 0);
        assert!(!battle.scripted_latch);
        // A different index avoids the ordinary repeat-index reroll. The
        // record still returns the same ability, now with its real requests.
        let mut rolls = SliceRolls::new(&[2]);
        assert!(
            battle
                .roll_enemy_ability(id(6), Some(id(2)), &data, &mut rolls, &mut events)
                .unwrap()
        );
        assert_eq!(rolls.drawn(), 1 + if ability == 0x1C { 16 } else { 48 });
        assert!(
            events.iter().any(
                |e| matches!(e, BattleEvent::EnemySkillUsed { skill, .. } if *skill == ability)
            )
        );
        // Negative control: raising the latch again suppresses the same arm.
        battle.scripted_latch = true;
        events.clear();
        let mut rolls = SliceRolls::new(&[3]);
        assert!(
            battle
                .roll_enemy_ability(id(6), Some(id(2)), &data, &mut rolls, &mut events)
                .unwrap()
        );
        assert_eq!(rolls.drawn(), 1);
        assert_eq!(events.len(), 1, "the raised latch spends the turn again");
        assert!(matches!(
            events[0],
            BattleEvent::FirstZioAction {
                action: crate::battle::FirstZioAction::DarkForceCharge,
                ..
            }
        ));
    }
}
