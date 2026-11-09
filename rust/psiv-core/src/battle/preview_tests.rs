use super::*;
use crate::battle::{EnemySkill, PartyMember, SliceRolls, fixtures};

#[test]
fn support_and_enemy_damage_previews_share_real_rules_and_preserve_the_roster() {
    let mut enemy = fixtures::zoran_bult();
    enemy.id = 123;
    enemy.hp = 2000;
    enemy.mental = 100;
    enemy.dexterity = u8::MAX;
    let data = fixtures::data()
        .with_enemies([enemy])
        .with_enemy_skills([EnemySkill {
            id: 0x35,
            name: "constructed damage".into(),
            effect: 1,
            power_stat: 0x82,
            target: 9,
            power: 64,
            resistance: 7,
            element: 3,
        }]);
    let mut roster = Roster::new();
    let mut stats = PartyMember::seat(&fixtures::alys(), &data).unwrap().stats;
    stats.curr_hp = 999;
    stats.max_hp = 999;
    stats.mental_defence.derived = 20;
    stats.mental_defence.battle = 20;
    let patient = roster
        .add_party_member(1, "constructed".into(), stats)
        .unwrap();
    let enemy = roster.add_enemy(1, data.enemy(123).unwrap()).unwrap();
    let original = roster.clone();
    let raised = roster.preview_support(patient, 11, 100).unwrap();
    assert_eq!(
        raised.get(patient).unwrap().stats.mental_defence.battle,
        120
    );
    assert_eq!(
        raised
            .preview_support(patient, 11, 50)
            .unwrap()
            .get(patient)
            .unwrap()
            .stats
            .mental_defence
            .battle,
        70,
        "recasting replaces instead of stacking"
    );
    let amount = |r: &Roster, id| {
        r.preview_enemy_damage(enemy, id, patient, &data, &mut SliceRolls::new(&[3, 4]))
            .unwrap()
            .unwrap()
            .iter()
            .filter_map(|e| match e {
                BattleEvent::Resolved { target, damage, .. } if *target == patient => *damage,
                _ => None,
            })
            .sum::<u16>()
    };
    assert!(
        amount(&raised, 0x35) < amount(&roster, 0x35),
        "MDF reduces the actual damage request"
    );
    assert!(amount(&roster, 0) > 0, "the physical control must hit");
    assert_eq!(
        amount(&raised, 0),
        amount(&roster, 0),
        "MDF does not reduce physical ATTACK"
    );
    assert_eq!(roster, original, "both previews are read-only");
    let mut wide = roster.clone();
    wide.get_mut(patient).unwrap().stats.mental_defence.derived = u16::MAX;
    assert_eq!(
        wide.preview_support(patient, 11, 2)
            .unwrap()
            .get(patient)
            .unwrap()
            .stats
            .mental_defence
            .battle,
        1,
        "retail ADD.W wraps"
    );
}
