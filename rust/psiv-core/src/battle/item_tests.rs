use super::*;
use crate::battle::{Battle, Command, Outcome, PartyMember, RoundOrders, SliceRolls, fixtures};

fn id(n: u8) -> FighterId {
    FighterId::new(n).unwrap()
}
fn data() -> BattleData {
    let records = [
        (16, "WOOD-CANE", 18, 16, 4, 16, 0, 0, 1, false),
        (57, "PSYCO-WAND", 39, 0, 2, 0, 0, 0, 2, false),
        (81, "DREAM-ROD", 7, 40, 2, 64, 2, 11, 6, false),
        (100, "SWIFT-HELM", 12, 32, 3, 0, 0, 0, 9, false),
        (125, "MONOMATE", 18, 24, 4, 24, 0, 0, 1, true),
        (128, "ANTIDOTE", 19, 0, 4, 0, 0, 0, 15, true),
        (129, "CURE-PARAL", 20, 0, 4, 0, 0, 0, 16, true),
        (130, "MOON-DEW", 21, 0, 4, 0, 0, 0, 17, true),
        (131, "STAR-DEW", 18, 64, 5, 64, 0, 0, 18, true),
        (132, "TELEPIPE", 25, 0, 0, 0, 0, 0, 0, true),
        (134, "SOL-DEW", 22, 0, 4, 0, 0, 0, 19, true),
        (136, "MAHLAYSHLD", 10, 64, 3, 0, 0, 0, 20, false),
        (139, "DYNAMITE", 1, 64, 1, 64, 6, 3, 21, true),
        (144, "REPAIR-KIT", 22, 0, 6, 0, 0, 0, 23, true),
    ];
    fixtures::data().with_battle_items(records.into_iter().map(
        |(
            id,
            name,
            effect,
            actor_power,
            targeting,
            power,
            resistance,
            element,
            object,
            consumable,
        )| {
            BattleItem {
                id,
                name: name.into(),
                effect,
                actor_power,
                targeting,
                power,
                resistance,
                element,
                object,
                consumable,
            }
        },
    ))
}
fn roster(data: &BattleData) -> Roster {
    let mut roster = Roster::new();
    for record in [fixtures::chaz(), fixtures::alys(), fixtures::hahn()] {
        let member = PartyMember::seat(&record, data).unwrap();
        roster.add_party_member(member.character, member.name, member.stats);
    }
    roster.add_enemy(1, &fixtures::zoran_bult());
    roster.add_enemy(2, &fixtures::zoran_bult());
    roster
}

#[test]
fn monomate_uses_its_own_power_and_consumes_the_raw_slot_with_holes() {
    let data = data();
    let mut roster = roster(&data);
    roster.get_mut(id(1)).unwrap().stats.mental.battle = 255;
    let target = &mut roster.get_mut(id(3)).unwrap().stats;
    target.curr_hp = 1;
    target.max_hp = 200;
    let mut slots = [0; 40];
    slots[0] = 128;
    slots[7] = 125;
    let mut inventory = Inventory::from_slots(slots);
    let mut rolls = SliceRolls::new(&[0]);
    let mut events = Vec::new();
    resolve_item(
        &mut roster,
        &mut inventory,
        id(1),
        (125, ItemSource::Inventory(7), Some(id(3))),
        &data,
        &mut rolls,
        &mut events,
    );
    assert_eq!(rolls.drawn(), 16);
    assert_eq!(roster.get(id(3)).unwrap().stats.curr_hp, 38); // ((3+24+48)>>1) + 1
    assert_eq!(roster.get(id(1)).unwrap().stats.curr_tp, 10);
    assert_eq!(inventory.get(7), None);
    assert_eq!(inventory.get(0), Some(128));
    assert!(events.contains(&BattleEvent::Healed {
        actor: id(1),
        target: id(3),
        amount: 37,
        remaining_hp: 38
    }));
}

#[test]
fn a_full_hp_item_use_still_consumes_the_item_and_sixteen_rolls() {
    let data = data();
    let mut roster = roster(&data);
    let mut inventory = Inventory::new();
    inventory.add(125).unwrap();
    let mut rolls = SliceRolls::new(&[63]);
    let mut events = Vec::new();
    resolve_item(
        &mut roster,
        &mut inventory,
        id(1),
        (125, ItemSource::Inventory(0), Some(id(1))),
        &data,
        &mut rolls,
        &mut events,
    );
    assert_eq!(rolls.drawn(), 16);
    assert_eq!(inventory.occupied(), 0);
    assert_eq!(roster.get(id(1)).unwrap().stats.curr_hp, 25);
    assert!(events.contains(&BattleEvent::ItemIneffective {
        actor: id(1),
        target: id(1)
    }));
}

#[test]
fn invalid_sources_targets_and_telepipes_spend_nothing() {
    for (item, source, target, reason) in [
        (
            125,
            ItemSource::Inventory(1),
            Some(id(1)),
            ItemRejection::Missing,
        ),
        (
            125,
            ItemSource::Equipment(0),
            Some(id(1)),
            ItemRejection::Missing,
        ),
        (
            125,
            ItemSource::Inventory(0),
            Some(id(6)),
            ItemRejection::InvalidTarget,
        ),
        (
            132,
            ItemSource::Inventory(1),
            None,
            ItemRejection::Unavailable,
        ),
    ] {
        let data = data();
        let mut roster = roster(&data);
        let before = roster.clone();
        let mut inventory = Inventory::new();
        inventory.add(125).unwrap();
        inventory.add(132).unwrap();
        let before_items = inventory.clone();
        let mut rolls = SliceRolls::new(&[63]);
        let mut events = Vec::new();
        resolve_item(
            &mut roster,
            &mut inventory,
            id(1),
            (item, source, target),
            &data,
            &mut rolls,
            &mut events,
        );
        assert_eq!(roster, before);
        assert_eq!(inventory, before_items);
        assert_eq!(rolls.drawn(), 0);
        assert_eq!(
            events,
            vec![BattleEvent::ItemRejected {
                actor: id(1),
                item,
                reason
            }]
        );
    }
}

#[test]
fn two_commands_cannot_consume_one_copy_but_distinct_copies_remain_usable() {
    let data = data();
    let mut roster = roster(&data);
    let mut inventory = Inventory::new();
    inventory.add(125).unwrap();
    inventory.add(125).unwrap();
    let mut rolls = SliceRolls::new(&[0]);
    let mut events = Vec::new();
    for actor in [1, 2] {
        resolve_item(
            &mut roster,
            &mut inventory,
            id(actor),
            (125, ItemSource::Inventory(0), Some(id(actor))),
            &data,
            &mut rolls,
            &mut events,
        );
    }
    assert_eq!(rolls.drawn(), 16);
    assert!(events.contains(&BattleEvent::ItemRejected {
        actor: id(2),
        item: 125,
        reason: ItemRejection::Missing
    }));
    assert_eq!(inventory.get(1), Some(125));
    resolve_item(
        &mut roster,
        &mut inventory,
        id(2),
        (125, ItemSource::Inventory(1), Some(id(2))),
        &data,
        &mut rolls,
        &mut events,
    );
    assert_eq!(rolls.drawn(), 32);
    assert_eq!(inventory.occupied(), 0);
}

#[test]
fn group_healing_excludes_androids_and_dead_without_drawing_for_them() {
    let data = data();
    let mut roster = roster(&data);
    roster.get_mut(id(1)).unwrap().stats.curr_hp = 1;
    roster.get_mut(id(2)).unwrap().stats.profession = 5;
    roster.get_mut(id(2)).unwrap().stats.curr_hp = 1;
    roster.get_mut(id(3)).unwrap().stats.status = status::DEAD;
    roster.get_mut(id(3)).unwrap().stats.curr_hp = 0;
    let mut inventory = Inventory::new();
    inventory.add(131).unwrap();
    let mut rolls = SliceRolls::new(&[0]);
    resolve_item(
        &mut roster,
        &mut inventory,
        id(1),
        (131, ItemSource::Inventory(0), None),
        &data,
        &mut rolls,
        &mut Vec::new(),
    );
    assert_eq!(rolls.drawn(), 16);
    assert_eq!(roster.get(id(1)).unwrap().stats.curr_hp, 25);
    assert_eq!(roster.get(id(2)).unwrap().stats.curr_hp, 1);
    assert_eq!(roster.get(id(3)).unwrap().stats.curr_hp, 0);
}

#[test]
fn cures_remove_the_selected_status_and_restore_agi_dex_even_without_the_affliction() {
    for (item, bit) in [(128, status::POISONED), (129, status::PARALYZED)] {
        for afflicted in [true, false] {
            let data = data();
            let mut roster = roster(&data);
            let target = &mut roster.get_mut(id(3)).unwrap().stats;
            target.status = status::TECH_SEALED | if afflicted { bit } else { 0 };
            target.agility.battle = 1;
            target.dexterity.battle = 33;
            target.attack.battle = 50;
            let mut inventory = Inventory::new();
            inventory.add(item).unwrap();
            let mut rolls = SliceRolls::new(&[0]);
            resolve_item(
                &mut roster,
                &mut inventory,
                id(1),
                (item, ItemSource::Inventory(0), Some(id(3))),
                &data,
                &mut rolls,
                &mut Vec::new(),
            );
            let target = &roster.get(id(3)).unwrap().stats;
            assert_eq!(target.status, status::TECH_SEALED);
            assert_eq!(target.agility.battle, 4);
            assert_eq!(target.dexterity.battle, 5);
            assert_eq!(target.attack.battle, 50);
            assert_eq!(target.curr_tp, 25);
            assert_eq!(rolls.drawn(), 0);
        }
    }
}

#[test]
fn moon_dew_revives_to_a_quarter_and_preserves_tech_sealing() {
    let data = data();
    let mut roster = roster(&data);
    let target = &mut roster.get_mut(id(3)).unwrap().stats;
    target.status =
        status::DEAD | status::POISONED | status::ASLEEP | status::PARALYZED | status::TECH_SEALED;
    target.curr_hp = 0;
    target.curr_tp = 3;
    target.curr_skill_uses[0] = 2;
    let mut inventory = Inventory::new();
    inventory.add(130).unwrap();
    let mut rolls = SliceRolls::new(&[0]);
    let mut events = Vec::new();
    assert!(item_targets(&roster, id(1), data.battle_item(130).unwrap()).contains(&id(3)));
    resolve_item(
        &mut roster,
        &mut inventory,
        id(1),
        (130, ItemSource::Inventory(0), Some(id(3))),
        &data,
        &mut rolls,
        &mut events,
    );
    let target = &roster.get(id(3)).unwrap().stats;
    assert_eq!(target.curr_hp, 5);
    assert_eq!(target.status, status::TECH_SEALED);
    assert_eq!(target.curr_tp, 3);
    assert_eq!(target.curr_skill_uses[0], 2);
    assert_eq!(rolls.drawn(), 0);
    assert!(events.contains(&BattleEvent::Revived {
        actor: id(1),
        target: id(3),
        remaining_hp: 5
    }));
}

#[test]
fn sol_dew_and_repair_kit_keep_their_distinct_status_rules() {
    for (item, android, condition, remaining_status) in [
        (134, false, status::DEAD | status::POISONED, 0),
        (134, false, status::POISONED, status::POISONED),
        (
            144,
            true,
            status::ANDROID_DEAD | status::TECH_SEALED,
            status::TECH_SEALED,
        ),
    ] {
        let data = data();
        let mut roster = roster(&data);
        let target = &mut roster.get_mut(id(3)).unwrap().stats;
        if android {
            target.profession = 5;
        }
        target.status = condition;
        target.curr_hp = 0;
        target.curr_tp = 1;
        let mut inventory = Inventory::new();
        inventory.add(item).unwrap();
        resolve_item(
            &mut roster,
            &mut inventory,
            id(1),
            (item, ItemSource::Inventory(0), Some(id(3))),
            &data,
            &mut SliceRolls::new(&[0]),
            &mut Vec::new(),
        );
        let target = &roster.get(id(3)).unwrap().stats;
        assert_eq!(target.curr_hp, target.max_hp);
        assert_eq!(target.status, remaining_status);
        assert_eq!(target.curr_tp, 1);
        assert_eq!(inventory.occupied(), 0);
    }
}

#[test]
fn equipped_healing_is_reusable_and_does_not_consume_an_inventory_copy() {
    let data = data();
    let mut roster = roster(&data);
    roster.get_mut(id(1)).unwrap().stats.equipment[0] = 16;
    let mut inventory = Inventory::new();
    inventory.add(16).unwrap();
    let mut rolls = SliceRolls::new(&[0]);
    let mut events = Vec::new();
    resolve_item(
        &mut roster,
        &mut inventory,
        id(1),
        (16, ItemSource::Equipment(0), Some(id(1))),
        &data,
        &mut rolls,
        &mut events,
    );
    assert_eq!(rolls.drawn(), 16);
    assert_eq!(inventory.get(0), Some(16));
    assert_eq!(roster.get(id(1)).unwrap().stats.equipment[0], 16);
    assert!(events.iter().any(|e| matches!(
        e,
        BattleEvent::ItemUsed {
            consumed: false,
            ..
        }
    )));
}

#[test]
fn psyco_wand_restores_enemy_stats_and_resistances_without_curing_status() {
    let data = data();
    let mut roster = roster(&data);
    let target = &mut roster.get_mut(id(6)).unwrap().stats;
    target.attack.battle = 99;
    target.defence.battle = 99;
    target.agility.battle = 99;
    target.mental_defence.battle = 99;
    target.element_props = [0; 14];
    target.status = status::ASLEEP;
    let mut inventory = Inventory::new();
    inventory.add(57).unwrap();
    let mut rolls = SliceRolls::new(&[0]);
    resolve_item(
        &mut roster,
        &mut inventory,
        id(1),
        (57, ItemSource::Inventory(0), None),
        &data,
        &mut rolls,
        &mut Vec::new(),
    );
    let target = &roster.get(id(6)).unwrap().stats;
    assert_eq!(target.attack.battle, 16);
    assert_eq!(target.defence.battle, 2);
    assert_eq!(target.agility.battle, 6);
    assert_eq!(target.mental_defence.battle, 0);
    assert_eq!(target.element_props, [2; 14]);
    assert_eq!(target.status, status::ASLEEP);
    assert_eq!(rolls.drawn(), 0);
    assert_eq!(inventory.get(0), Some(57));
}

#[test]
fn dream_rod_uses_item_power_against_mental_and_leaves_agility_alone() {
    let data = data();
    let mut roster = roster(&data);
    roster.get_mut(id(7)).unwrap().stats.mental.battle = 60;
    let mut inventory = Inventory::new();
    inventory.add(81).unwrap();
    let mut rolls = SliceRolls::new(&[0]);
    resolve_item(
        &mut roster,
        &mut inventory,
        id(1),
        (81, ItemSource::Inventory(0), None),
        &data,
        &mut rolls,
        &mut Vec::new(),
    );
    assert_eq!(rolls.drawn(), 2);
    assert_eq!(roster.get(id(6)).unwrap().stats.status, status::ASLEEP);
    assert_eq!(roster.get(id(6)).unwrap().stats.agility.battle, 6);
    assert_eq!(roster.get(id(7)).unwrap().stats.status, 0);
}

#[test]
fn damaging_item_consumption_and_rewards_flow_through_a_real_round() {
    let data = data();
    let mut formation = fixtures::formation_two_zoran_bults();
    formation.enemies.truncate(1);
    let party = vec![PartyMember::seat(&fixtures::alys(), &data).unwrap()];
    let (mut battle, _) = Battle::start(
        &formation,
        party,
        &data,
        false,
        0,
        &mut SliceRolls::new(&[63]),
    )
    .unwrap();
    let mut inventory = Inventory::new();
    inventory.add(139).unwrap();
    let mut rolls = SliceRolls::new(&[0]);
    let events = battle
        .round_with_inventory(
            &RoundOrders::Commands(vec![Command::Item {
                item: 139,
                source: ItemSource::Inventory(0),
                target: Some(id(6)),
            }]),
            &data,
            &mut inventory,
            &mut rolls,
        )
        .unwrap();
    assert_eq!(inventory.occupied(), 0);
    assert_eq!(rolls.drawn(), 29); // nine queue + four enemy targets + sixteen damage, no physical hit roll
    assert!(events.contains(&BattleEvent::Ended {
        outcome: Outcome::Victory
    }));
    assert!(events.iter().any(|e| matches!(
        e,
        BattleEvent::Rewarded {
            meseta: 3,
            experience_total: 12,
            ..
        }
    )));
}

#[test]
fn psycho_wand_reloads_zio_and_formation_stats_once_without_revival_or_draws() {
    let mut invulnerable = fixtures::zoran_bult();
    invulnerable.id = 139;
    invulnerable.name = "ZIO".into();
    invulnerable.hp = 16383;
    invulnerable.properties = [0; 14];
    let mut vulnerable = invulnerable.clone();
    vulnerable.id = 140;
    vulnerable.hp = 2889;
    vulnerable.properties = [2; 14];
    let data = data().with_enemies([invulnerable.clone(), vulnerable.clone()]);
    let mut roster = roster(&data);
    let first = roster.get_mut(id(6)).unwrap();
    first.stats = Stats::from_enemy(&invulnerable);
    first.stats.status = status::ASLEEP;
    first.ability = 84;
    roster.get_mut(id(7)).unwrap().mark_defeated();
    let mut inventory = Inventory::new();
    inventory.add(57).unwrap();
    let mut rolls = SliceRolls::new(&[0]);
    let mut events = Vec::new();
    resolve_item(
        &mut roster,
        &mut inventory,
        id(1),
        (57, ItemSource::Inventory(0), None),
        &data,
        &mut rolls,
        &mut events,
    );
    let first = roster.get(id(6)).unwrap();
    assert_eq!(first.stats, Stats::from_enemy(&vulnerable));
    assert_eq!(first.ability, 84, "the object is not respawned");
    assert!(
        !roster.get(id(7)).unwrap().active,
        "a cleared fighter stays absent"
    );
    assert!(!events.iter().any(|e| matches!(
        e,
        BattleEvent::Revived { .. } | BattleEvent::EnemyReplenished { .. }
    )));
    assert!(events.iter().any(|e| matches!(
        e,
        BattleEvent::EnemyStatsReloaded {
            enemy_id: 140,
            hp: 2889,
            ..
        }
    )));
    assert_eq!(rolls.drawn(), 0);
    assert_eq!(inventory.get(0), Some(57));

    roster.get_mut(id(6)).unwrap().stats.curr_hp = 2000;
    events.clear();
    resolve_item(
        &mut roster,
        &mut inventory,
        id(1),
        (57, ItemSource::Inventory(0), None),
        &data,
        &mut rolls,
        &mut events,
    );
    assert_eq!(roster.get(id(6)).unwrap().stats.curr_hp, 2000);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, BattleEvent::EnemyStatsReloaded { .. }))
    );
}

#[test]
fn psycho_wand_does_not_transform_zio_in_a_later_formation_slot() {
    let mut enemy = fixtures::zoran_bult();
    enemy.id = 139;
    enemy.properties = [0; 14];
    let data = data().with_enemies([enemy.clone()]);
    let mut roster = roster(&data);
    roster.get_mut(id(7)).unwrap().stats = Stats::from_enemy(&enemy);
    let mut inventory = Inventory::new();
    inventory.add(57).unwrap();
    let mut events = Vec::new();
    resolve_item(
        &mut roster,
        &mut inventory,
        id(1),
        (57, ItemSource::Inventory(0), None),
        &data,
        &mut SliceRolls::new(&[0]),
        &mut events,
    );
    assert_eq!(roster.get(id(7)).unwrap().stats.enemy_id, 139);
    assert_eq!(roster.get(id(7)).unwrap().stats.element_props, [0; 14]);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, BattleEvent::EnemyStatsReloaded { .. }))
    );
}
