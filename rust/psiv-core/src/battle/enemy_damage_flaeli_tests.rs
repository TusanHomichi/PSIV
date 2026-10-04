//! Synthetic arithmetic controls for every cited FLAELI carrier. Retail
//! records are deliberately loaded only by the pack-backed runtime suite.

use super::*;
use crate::battle::{EnemySkill, SliceRolls, fixtures};

#[test]
fn every_flaeli_carrier_makes_one_stored_target_request() {
    for enemy_id in [111, 112, 113, 138, 121, 125] {
        let mut record = fixtures::zoran_bult();
        record.id = enemy_id;
        record.mental = 40;
        record.strength = 1;
        record.attack = 1;
        let skill = EnemySkill {
            id: 0x5A,
            name: "synthetic".into(),
            effect: 1,
            power_stat: 2,
            target: 8,
            power: 10,
            resistance: 7,
            element: 3,
        };
        let data = fixtures::data()
            .with_enemies([record])
            .with_enemy_skills([skill]);
        let mut r = Roster::new();
        for _ in 0..2 {
            let mut stats = crate::battle::PartyMember::seat(&fixtures::alys(), &data)
                .unwrap()
                .stats;
            stats.curr_hp = 999;
            stats.max_hp = 999;
            stats.mental_defence.battle = 0;
            r.add_party_member(0, "control".into(), stats);
        }
        r.add_enemy(1, data.enemy(enemy_id).unwrap());
        let mut rolls = SliceRolls::new(&[0]);
        let mut events = Vec::new();
        assert!(resolve_damage_skill(
            &mut r,
            super::tests::id(6),
            0x5A,
            Some(super::tests::id(2)),
            &data,
            &mut rolls,
            &mut events
        ));
        assert_eq!(rolls.drawn(), 16, "carrier {enemy_id}");
        assert_eq!(r.get(super::tests::id(1)).unwrap().stats.curr_hp, 999);
        assert!(
            matches!(events.as_slice(), [BattleEvent::EnemySkillUsed { skill: 0x5A, .. },
            BattleEvent::Resolved { target, damage: Some(_), .. }] if *target == super::tests::id(2)),
            "carrier {enemy_id}: {events:?}"
        );
    }
}
