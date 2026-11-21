use super::*;
use crate::battle::combo::ComboRecord;
use crate::battle::engine::{Battle, PartyMember, RoundOrders};
use crate::battle::event::BattleEvent;
use crate::battle::fixtures;
use crate::battle::item::BattleItem;
use crate::battle::records::{BattleData, FormationEnemy, FormationRecord};
use crate::battle::rng::SliceRolls;
use crate::battle::skill::Skill;
use crate::battle::stats::{PROFESSION_ANDROID, Stats, status};
use crate::battle::technique::Technique;
use crate::{Inventory, MacroCommand, MacroRecord};

fn id(v: u8) -> FighterId {
    FighterId::new(v).expect("valid fighter id")
}

fn macro_from(cmds: &[(u8, u8, u8)]) -> MacroRecord {
    let mut rec = MacroRecord::default();
    for (i, &(character_id, command_index, ability_id)) in cmds.iter().enumerate() {
        rec.commands[i] = MacroCommand {
            character_id,
            command_index,
            ability_id,
            reserved: 0,
        };
    }
    rec
}

fn member(
    character: u8,
    agility: u8,
    hp: u16,
    max_hp: u16,
    techs: &[u8],
    skills: &[u8],
) -> PartyMember {
    let mut rec = fixtures::alys();
    rec.id = character;
    rec.name = format!("P{}", character + 1);
    rec.agility = agility;
    rec.mental = 25;
    rec.hp = max_hp;
    rec.tp = 40;
    rec.equipment = [3, 0, 0, 0];
    rec.techniques = [0; 16];
    for (i, &t) in techs.iter().enumerate() {
        rec.techniques[i] = t;
    }
    rec.skills = [0; 8];
    rec.skill_uses = [0; 8];
    for (i, &s) in skills.iter().enumerate() {
        rec.skills[i] = s;
        rec.skill_uses[i] = 3;
    }
    let items = fixtures::items();
    let mut stats = Stats::from_character(&rec, |item_id| {
        items.iter().find(|i| i.id == item_id).cloned()
    });
    stats.curr_hp = hp;
    PartyMember {
        character,
        name: rec.name,
        stats,
    }
}

fn test_data() -> BattleData {
    let mut enemy = fixtures::zoran_bult();
    enemy.hp = 400;
    enemy.agility = 1;
    enemy.mental_defence = 5;
    enemy.properties = [2; 14];

    BattleData::new()
        .with_enemies([enemy])
        .with_items(fixtures::items())
        .with_techniques([
            Technique {
                id: 4,
                name: "T_WAT".into(),
                effect: 1,
                cost: 3,
                targeting: 0x11,
                power: 12,
                resistance: 7,
                element: 5,
            },
            Technique {
                id: 24,
                name: "T_RES".into(),
                effect: 18,
                cost: 3,
                targeting: 0x34,
                power: 30,
                resistance: 0,
                element: 0,
            },
            Technique {
                id: 34,
                name: "T_ANTI".into(),
                effect: 19,
                cost: 2,
                targeting: 0x34,
                power: 0,
                resistance: 0,
                element: 0,
            },
            Technique {
                id: 36,
                name: "T_REVER".into(),
                effect: 21,
                cost: 10,
                targeting: 0x34,
                power: 0,
                resistance: 0,
                element: 0,
            },
        ])
        .with_skills([Skill {
            id: 11,
            name: "S_TANDLE".into(),
            effect: 1,
            power_stat: 2,
            requires_weapon: false,
            targeting: 0x12,
            power: 22,
            resistance: 7,
            element: 7,
        }])
        .with_battle_items([BattleItem {
            id: 144,
            name: "I_REPAIR".into(),
            effect: 22,
            actor_power: 10,
            targeting: 6,
            power: 50,
            resistance: 0,
            element: 0,
            object: 23,
            consumable: true,
        }])
        .with_combos([ComboRecord {
            id: 4,
            name: "SYNTH_ORD".into(),
            effect: 1,
            range: 2,
            power: 0,
            resistance: 7,
            element: 7,
            participants: 2,
            ordered: true,
            setups: vec![vec![4, 0x80 | 11]],
        }])
}

fn formation(count: usize) -> FormationRecord {
    FormationRecord {
        id: 1,
        ambush_chance: 0,
        run_chance: 0,
        drop_rate: 0,
        drop_item: None,
        enemies: (0..count)
            .map(|i| FormationEnemy {
                slot: (i + 1) as u8,
                enemy_id: 10,
                position: 0,
            })
            .collect(),
    }
}

#[test]
fn macro_reorders_faster_second_actor_so_ordered_combo_fires() {
    let data = test_data();
    // Character 0 (WAT) is SLOWER (agi 10) than Character 1 (TANDLE, agi 40).
    // Under COMD, Character 1 would go first and ordered combo 4 (WAT -> TANDLE) would NOT fire.
    // Under MACR [char 0 WAT, char 1 TANDLE], `loc_55DE` rotates char 1 after char 0 so combo 4 fires!
    let party = vec![
        member(0, 10, 100, 100, &[4], &[]),
        member(1, 40, 100, 100, &[], &[11]),
    ];
    let mut rolls = SliceRolls::new(&[0; 256]);
    let (mut battle, _) = Battle::start(&formation(2), party, &data, true, &mut rolls).unwrap();
    let rec = macro_from(&[(0, 2, 4), (1, 3, 11)]);
    let events = battle
        .round(&RoundOrders::Macro(rec), &data, &mut rolls)
        .unwrap();

    // Queue order placed fighter 1 (char 0) immediately before fighter 2 (char 1).
    let order = events
        .iter()
        .find_map(|e| match e {
            BattleEvent::RoundBegan { order, .. } => Some(order.clone()),
            _ => None,
        })
        .unwrap();
    let pos1 = order.iter().position(|&f| f == id(1)).unwrap();
    let pos2 = order.iter().position(|&f| f == id(2)).unwrap();
    assert!(
        pos1 < pos2,
        "macro must order char 0 before char 1: {order:?}"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, BattleEvent::ComboUsed { combo: 4, .. })),
        "macro ordering must trigger ordered combo 4: {events:?}"
    );
}

#[test]
fn macro_special_targeting_heals_most_injured_human_cures_poison_revives_dead_and_repairs_android()
{
    let data = test_data();

    // 1. RES targets living human with largest HP deficit (skipping full-HP and android).
    let mut p0 = member(0, 40, 100, 100, &[24], &[]);
    let p1 = member(1, 30, 40, 100, &[], &[]); // deficit 60
    let mut p2 = member(2, 20, 10, 100, &[], &[]); // deficit 90, but android!
    p2.stats.profession = PROFESSION_ANDROID;
    p0.stats.curr_hp = 90; // deficit 10
    let mut rolls = SliceRolls::new(&[0; 256]);
    let (mut b1, _) =
        Battle::start(&formation(1), vec![p0, p1, p2], &data, true, &mut rolls).unwrap();
    let ev1 = b1
        .round(
            &RoundOrders::Macro(macro_from(&[(0, 2, 24), (1, 5, 0), (2, 5, 0)])),
            &data,
            &mut rolls,
        )
        .unwrap();
    assert!(
        ev1.iter()
            .any(|e| matches!(e, BattleEvent::Healed { target, .. } if *target == id(2))),
        "RES must heal most-injured living human (slot 2), not android: {ev1:?}"
    );

    // 2. ANTI targets poisoned ally; REVER targets dead ally; RepairKit (144) targets injured android.
    let c0 = member(0, 50, 100, 100, &[34], &[]);
    let mut c1 = member(1, 40, 100, 100, &[36], &[]);
    c1.stats.status |= status::POISONED;
    let mut c2 = member(2, 30, 0, 100, &[], &[]);
    c2.stats.status |= status::DEAD;
    let mut c3 = member(3, 20, 20, 100, &[], &[]);
    c3.stats.profession = PROFESSION_ANDROID;
    let mut inv = Inventory::new();
    inv.add(144).expect("inventory space");
    let mut rolls2 = SliceRolls::new(&[0; 256]);
    let (mut b2, _) = Battle::start(
        &formation(1),
        vec![c0, c1, c2, c3],
        &data,
        true,
        &mut rolls2,
    )
    .unwrap();
    let ev2 = b2
        .round_with_inventory(
            &RoundOrders::Macro(macro_from(&[
                (0, 2, 34),
                (1, 2, 36),
                (2, 5, 0),
                (3, 4, 144),
            ])),
            &data,
            &mut inv,
            &mut rolls2,
        )
        .unwrap();
    assert!(
        ev2.iter().any(|e| matches!(
            e,
            BattleEvent::StatusRestored { target, removed, .. }
                if *target == id(2) && (*removed & status::POISONED != 0)
        )),
        "ANTI must cure poisoned fighter 2: {ev2:?}"
    );
    assert!(
        ev2.iter()
            .any(|e| matches!(e, BattleEvent::Revived { target, .. } if *target == id(3))),
        "REVER must revive dead fighter 3: {ev2:?}"
    );
    assert!(
        ev2.iter()
            .any(|e| matches!(e, BattleEvent::Healed { target, .. } if *target == id(4))),
        "RepairKit must heal injured android fighter 4: {ev2:?}"
    );
}
