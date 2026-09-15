use super::*;
use crate::battle::{PartyMember, SliceRolls, fixtures, status};

fn id(n: u8) -> FighterId {
    FighterId::new(n).unwrap()
}

fn setup(effect: u8) -> (Roster, BattleData) {
    let mut enemy = fixtures::monster_fly();
    enemy.attack_status = effect;
    enemy.strength = 10;
    enemy.dexterity = 10;
    enemy.attack = 1;
    let data = fixtures::data().with_enemies([enemy.clone()]);
    let mut roster = Roster::new();
    let mut member = PartyMember::seat(&fixtures::chaz(), &data).unwrap();
    member.stats.curr_hp = 100;
    member.stats.max_hp = 100;
    member.stats.strength.battle = 10;
    member.stats.agility.battle = 1;
    member.stats.defence.battle = 999;
    roster.add_party_member(member.character, member.name, member.stats);
    roster.add_enemy(1, &enemy);
    (roster, data)
}

#[test]
fn attack_poison_and_paralysis_roll_after_damage_at_the_retail_threshold() {
    for (effect, bit) in [(27, status::POISONED), (28, status::PARALYZED)] {
        for (draw, applied) in [(56, false), (57, true)] {
            let (mut roster, data) = setup(effect);
            let stats = &mut roster.get_mut(id(1)).unwrap().stats;
            stats.status = status::ASLEEP | status::ASLEEP_2 | status::TECH_SEALED;
            let mut stream = [0; 18];
            stream[17] = draw;
            let mut rolls = SliceRolls::new(&stream);
            let mut events = Vec::new();
            resolve_attack(
                &mut roster,
                id(6),
                Some(id(1)),
                &data,
                &mut rolls,
                &mut events,
            )
            .unwrap();
            assert_eq!(
                rolls.drawn(),
                18,
                "hit, 16 damage draws, then one status draw"
            );
            let stats = &roster.get(id(1)).unwrap().stats;
            assert_eq!(stats.curr_hp, 99);
            assert_eq!(stats.status & bit != 0, applied);
            if applied {
                assert_eq!(
                    events.last(),
                    Some(&BattleEvent::StatusInflicted {
                        actor: id(6),
                        target: id(1),
                        status: bit
                    })
                );
                assert_ne!(stats.status & status::TECH_SEALED, 0);
                assert_ne!(stats.status & status::ASLEEP_2, 0);
                if effect == 28 {
                    assert_eq!(stats.status & status::ASLEEP, 0);
                    assert_eq!((stats.agility.battle, stats.dexterity.battle), (1, 1));
                }
            } else {
                assert_eq!(events.len(), 2);
            }
        }
    }
}

#[test]
fn miss_death_and_existing_ailment_skip_the_effect_roll_but_immunity_does_not() {
    for case in ["miss", "death", "existing", "immune"] {
        let (mut roster, data) = setup(27);
        let target = &mut roster.get_mut(id(1)).unwrap().stats;
        match case {
            "miss" => target.agility.battle = 255,
            "death" => {
                target.curr_hp = 1;
                target.profession = 5;
                target.status = status::ASLEEP | status::TECH_SEALED;
            }
            "existing" => target.status = status::POISONED,
            "immune" => target.element_props[12] = 0,
            _ => unreachable!(),
        }
        let mut rolls = SliceRolls::new(&[63]);
        let mut events = Vec::new();
        resolve_attack(
            &mut roster,
            id(6),
            Some(id(1)),
            &data,
            &mut rolls,
            &mut events,
        )
        .unwrap();
        assert_eq!(
            rolls.drawn(),
            match case {
                "miss" => 1,
                "immune" => 18,
                _ => 17,
            }
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, BattleEvent::StatusInflicted { .. }))
        );
        if case == "death" {
            assert_eq!(
                roster.get(id(1)).unwrap().stats.status,
                status::ANDROID_DEAD | status::TECH_SEALED
            );
        }
    }
}
