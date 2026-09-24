//! Ability dispatch: the enemy abilities the engine cannot run yet, and
//! the vehicle skill that goes through the retail dispatcher.
//!
//! Split out of `engine_tests.rs` under the repo's 1,000-line rule. A
//! child module of those tests, so the parent's harness — `id`,
//! `member`, `basement_party`, `start`, `play_out`, `rewarded` — and the
//! engine's private items stay in scope.

use super::*;

#[test]
fn an_unimplemented_ability_is_announced_rather_than_faked() {
    let mut record = fixtures::zoran_bult();
    record.regular_abilities = [7; 8];
    let data = fixtures::data().with_enemies([record]);

    let mut rolls = SliceRolls::new(&[20]);
    let mut battle = start(
        &fixtures::formation_two_zoran_bults(),
        basement_party(&data),
        &data,
        &mut rolls,
    );
    let draws: Vec<u16> = std::iter::repeat_n(0u16, FIGHTER_SLOTS + ENEMY_SLOTS)
        .chain(std::iter::repeat_n(30u16, 200))
        .collect();
    let mut rolls = SliceRolls::new(&draws);
    let events = battle
        .round(&RoundOrders::attack_all(), &data, &mut rolls)
        .expect("resolves");
    assert!(
        events
            .iter()
            .any(|e| matches!(e, BattleEvent::UnsupportedAbility { ability: 7, .. }))
    );
}

#[test]
fn the_ability_index_word_is_an_input_the_battle_hands_back() {
    // `$FFFFEEA8` is the session's cell, not the battle's: `Battle::start`
    // takes the word a battle load left there and `Battle::last_ability_index`
    // returns what the battle leaves in it. Every battle the cartridge loads
    // starts at zero (`ps4.asm:9992-9994`), so the parent harness's `start`
    // passes zero; this one hands over a mid-battle value to show that the word
    // the caller passes is the word the battle carries. What the reroll does
    // with a non-zero word is `choose_ability`'s own unit test's subject.
    let data = fixtures::data();
    let mut rolls = SliceRolls::new(&[0]);
    let (battle, _) = Battle::start(
        &fixtures::formation_two_zoran_bults(),
        basement_party(&data),
        &data,
        false,
        5,
        &mut rolls,
    )
    .expect("the fixture resolves");
    assert_eq!(battle.last_ability_index(), 5);
}

#[test]
fn a_vehicle_skill_consumes_a_use_and_runs_its_retail_dispatcher() {
    let data = fixtures::data();
    let vehicle = crate::vehicle::battle_member(
        1,
        VehicleRecord {
            current_hp: 500,
            max_hp: 500,
            skill_mask: 0x03,
            current_skill_uses: [1, 0, 0, 0, 0, 0, 0, 0],
            max_skill_uses: [1, 0, 0, 0, 0, 0, 0, 0],
            ..VehicleRecord::default()
        },
    )
    .expect("Land Rover battle member");
    let mut setup_rolls = SliceRolls::new(&[20]);
    let (mut battle, _) = Battle::start_vehicle(
        &fixtures::formation_two_zoran_bults(),
        vec![vehicle],
        &data,
        0,
        &mut setup_rolls,
    )
    .expect("vehicle battle starts");
    let draws: Vec<u16> = std::iter::repeat_n(0u16, FIGHTER_SLOTS + ENEMY_SLOTS)
        .chain(std::iter::repeat_n(30u16, 200))
        .collect();
    let mut rolls = SliceRolls::new(&draws);
    let events = battle
        .round(
            &RoundOrders::Commands(vec![Command::VehicleSkill(1)]),
            &data,
            &mut rolls,
        )
        .expect("vehicle round resolves");

    assert!(events.contains(&BattleEvent::VehicleSkillUsed {
        actor: id(1),
        skill: 1,
        remaining: 0,
    }));
    assert!(events.iter().any(|event| matches!(
        event,
        BattleEvent::VehicleSkillEffect {
            actor,
            skill: 1,
            effect: crate::battle::VehicleSkillEffectKind::Damage,
            ..
        } if *actor == id(1)
    )));
    assert!(!events.iter().any(|event| matches!(
        event,
        BattleEvent::VehicleSkillEffectUnavailable { actor, skill: 1 } if *actor == id(1)
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        BattleEvent::Attacked { actor, .. } if *actor == id(1)
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        BattleEvent::Resolved { actor, .. } if *actor == id(1)
    )));
    assert_eq!(
        battle
            .party_stats()
            .next()
            .expect("vehicle stats")
            .1
            .curr_skill_uses[0],
        0
    );
}
