use super::*;
use crate::battle::{EnemyRecord, SliceRolls, fixtures};

fn id(n: u8) -> FighterId {
    FighterId::new(n).unwrap()
}

// Deliberately synthetic stats: the captured pack supplies retail records.
fn setup(enemy: u16) -> (BattleData, Roster) {
    let record = EnemyRecord {
        id: enemy,
        strength: 7,
        mental_defence: 10,
        ..fixtures::zoran_bult()
    };
    let data = fixtures::data()
        .with_enemies([record])
        .with_enemy_skills([EnemySkill {
            id: 0x1D,
            name: "synthetic barrier".into(),
            effect: 0x0B,
            power_stat: 1,
            target: 2,
            power: 0,
            resistance: 0,
            element: 0,
        }]);
    let mut roster = Roster::new();
    for slot in 1..=3 {
        roster.add_enemy(slot, data.enemy(enemy).unwrap()).unwrap();
    }
    (data, roster)
}

fn run(roster: &mut Roster, data: &BattleData) -> (EffectTurn, Vec<BattleEvent>) {
    let mut rolls = SliceRolls::new(&[]);
    let mut events = Vec::new();
    let turn = resolve_effect_skill(roster, id(6), 0x1D, None, data, &mut rolls, &mut events);
    assert_eq!(
        rolls.drawn(),
        0,
        "resistance selector zero draws no effect RNG"
    );
    (turn, events)
}

#[test]
fn barrier_sets_each_living_enemy_from_derived_mdef_without_stacking() {
    for enemy in [48, 49] {
        let (data, mut roster) = setup(enemy);
        roster.get_mut(id(7)).unwrap().stats.mental_defence.battle = 2;
        roster.get_mut(id(8)).unwrap().stats.status |= status::DEAD;
        let (turn, events) = run(&mut roster, &data);
        assert_eq!(turn, EffectTurn::Resolved);
        assert_eq!(roster.get(id(6)).unwrap().stats.mental_defence.battle, 17);
        assert_eq!(roster.get(id(7)).unwrap().stats.mental_defence.battle, 17);
        assert_eq!(roster.get(id(8)).unwrap().stats.mental_defence.battle, 10);
        assert_eq!(events.len(), 3, "used plus two living recipients");
        roster.get_mut(id(6)).unwrap().ability = 0x1D;
        let (turn, events) = run(&mut roster, &data);
        assert_eq!(turn, EffectTurn::Swing);
        assert!(events.is_empty());
        assert_eq!(roster.get(id(6)).unwrap().ability, 0);
        assert_eq!(roster.get(id(7)).unwrap().stats.mental_defence.battle, 17);
    }
}

#[test]
fn barrier_preserves_signed_guard_and_word_wrapping() {
    let (data, mut roster) = setup(48);
    let stats = &mut roster.get_mut(id(6)).unwrap().stats;
    stats.mental_defence.derived = 0x7FFF;
    stats.mental_defence.battle = 0x8000;
    assert_eq!(run(&mut roster, &data).0, EffectTurn::Resolved);
    assert_eq!(
        roster.get(id(6)).unwrap().stats.mental_defence.battle,
        0x8006
    );
    let stats = &mut roster.get_mut(id(6)).unwrap().stats;
    stats.mental_defence.derived = 0xFFFE;
    stats.mental_defence.battle = 0xFFFE;
    assert_eq!(run(&mut roster, &data).0, EffectTurn::Resolved);
    assert_eq!(roster.get(id(6)).unwrap().stats.mental_defence.battle, 5);
}

#[test]
fn wrong_record_or_untraced_enemy_cannot_enable_barrier() {
    let (mut data, mut roster) = setup(48);
    let mut wrong = data.enemy_skill(0x1D).unwrap().clone();
    wrong.effect = 0x0A;
    data = data.with_enemy_skills([wrong]);
    assert_eq!(run(&mut roster, &data).0, EffectTurn::NotMine);
    assert_eq!(roster.get(id(6)).unwrap().stats.mental_defence.battle, 10);
    let (data, mut roster) = setup(47);
    assert_eq!(run(&mut roster, &data).0, EffectTurn::NotMine);
}
