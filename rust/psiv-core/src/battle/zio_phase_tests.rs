//! The Zio family's phase counter, driven through [`Battle::round`] so the
//! whole chain is exercised: the opening priority (`$FFFFEE87`), the ability
//! roll, the routine's scripted arm and the dispatch behind it.

use crate::Inventory;
use crate::battle::{
    Battle, BattleData, BattleEvent, BattleItem, Command, ELEMENT_SLOTS, EnemySkill, FighterId,
    FirstZioAction, FormationEnemy, FormationRecord, ItemSource, PartyMember, Priority,
    RoundOrders, SliceRolls, fixtures, status,
};

const MAGIC_BARRIER: FirstZioAction = FirstZioAction::MagicBarrier;
const PSYCHO_WAND: u8 = 0x39;
const CORRSION: u8 = 0x4D;
const HEWN: u8 = 0x4F;
const BLACK_WAVE2: u8 = 0x6C;

fn id(n: u8) -> FighterId {
    FighterId::new(n).unwrap()
}

fn skill(ability: u8, name: &str, (target, power): (u8, u8)) -> EnemySkill {
    EnemySkill {
        id: ability,
        name: name.into(),
        effect: 0x01,
        power_stat: 0x02,
        target,
        power,
        resistance: 0x07,
        element: 1,
    }
}

/// The three Zio records as the pack holds them (`enemies.json`), with the
/// regular list of Zio2 replaced so a test names the arm it drives.
fn data(zio2_list: [u8; 8]) -> BattleData {
    let mut zio = fixtures::zoran_bult();
    zio.id = 139;
    zio.name = "ZIO".into();
    zio.hp = 16383;
    zio.agility = 255;
    zio.regular_abilities = [84; 8];
    zio.condition_ids = [0; 4];
    zio.properties = [0; 14];
    let mut zio2 = zio.clone();
    zio2.id = 140;
    zio2.hp = 2889;
    zio2.agility = 100;
    zio2.mental = 40;
    zio2.regular_abilities = zio2_list;
    zio2.properties = [2; 14];
    let mut zio3 = zio.clone();
    zio3.id = 152;
    zio3.regular_abilities = [112; 8];
    fixtures::data()
        .with_enemies([zio, zio2, zio3])
        .with_enemy_skills([
            skill(CORRSION, "CORRSION", (9, 64)),
            skill(HEWN, "HEWN", (9, 56)),
            skill(BLACK_WAVE2, "BLACK WAVE", (8, 112)),
        ])
        .with_battle_items([BattleItem {
            id: PSYCHO_WAND,
            name: "PSYCO-WAND".into(),
            effect: 39,
            actor_power: 0,
            targeting: 2,
            power: 0,
            resistance: 0,
            element: 0,
            object: 2,
            consumable: false,
        }])
}

fn start(enemy_id: u16, data: &BattleData) -> (Battle, Vec<BattleEvent>) {
    let formation = FormationRecord {
        id: 0,
        ambush_chance: 0,
        run_chance: 254,
        drop_rate: 0,
        drop_item: None,
        enemies: vec![FormationEnemy {
            slot: 1,
            enemy_id,
            position: 20,
        }],
    };
    let party = [fixtures::alys(), fixtures::chaz(), fixtures::hahn()].map(|record| {
        let mut member = PartyMember::seat(&record, data).unwrap();
        member.stats.curr_hp = 400;
        member.stats.max_hp = 400;
        member.stats.agility.battle = 1;
        member.stats.element_props = [2; ELEMENT_SLOTS];
        member
    });
    // Boss battle: the chance roll is drawn and discarded, as `loc_B62A` does.
    Battle::start(
        &formation,
        party.to_vec(),
        data,
        true,
        &mut SliceRolls::new(&[0]),
    )
    .unwrap()
}

const STREAM: [u16; 11] = [3, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5];

fn round(battle: &mut Battle, data: &BattleData, orders: &RoundOrders) -> Vec<BattleEvent> {
    battle
        .round_with_inventory(
            orders,
            data,
            &mut Inventory::new(),
            &mut SliceRolls::new(&STREAM),
        )
        .unwrap()
}

fn zio_actions(events: &[BattleEvent]) -> Vec<FirstZioAction> {
    events
        .iter()
        .filter_map(|e| match e {
            BattleEvent::FirstZioAction { action, .. } => Some(*action),
            _ => None,
        })
        .collect()
}

fn defend() -> RoundOrders {
    RoundOrders::Commands(vec![Command::Defend; 3])
}

/// `loc_B62A` ends in `tst.b ($FFFFEE87).w / beq.s / st d0` (`ps4.asm:17448`):
/// every battle that holds a Zio, DarkForce1/2, ProfoundDarkness1 or a
/// CarnivorousTree opens as an enemy ambush, boss forcing and chance roll
/// notwithstanding. Nothing else does.
#[test]
fn the_scripted_flag_forces_the_opening_ambush() {
    let data = data([0; 8]);
    for enemy in [139, 140, 152] {
        let (_, events) = start(enemy, &data);
        assert!(
            matches!(
                events.first(),
                Some(BattleEvent::Started {
                    priority: Priority::Ambush,
                    ..
                })
            ),
            "enemy {enemy}: {events:?}"
        );
    }
    let plain = fixtures::data();
    let formation = fixtures::formation_two_zoran_bults();
    let party = vec![PartyMember::seat(&fixtures::alys(), &plain).unwrap()];
    let (_, events) =
        Battle::start(&formation, party, &plain, true, &mut SliceRolls::new(&[0])).unwrap();
    assert!(matches!(
        events.first(),
        Some(BattleEvent::Started {
            priority: Priority::Normal,
            ..
        })
    ));
}

/// `EnemyAttack_Zio` (`ps4.asm:19483`): Magic Barrier, Nightmare, then Black
/// Wave on every later turn, whatever the roll holds; Black Wave kills the
/// drawn target outright (`loc_25048`) and the fight goes on.
#[test]
fn zio_runs_barrier_nightmare_then_kills_one_member_a_turn() {
    let data = data([0; 8]);
    let (mut battle, _) = start(139, &data);
    // Round 1 is the ambush: the enemy acts alone and the party has no turn.
    let first = round(&mut battle, &data, &defend());
    assert_eq!(zio_actions(&first), [MAGIC_BARRIER]);
    let second = round(&mut battle, &data, &defend());
    assert_eq!(zio_actions(&second), [FirstZioAction::Nightmare]);
    let mut dead = 0;
    for turn in 0..3 {
        let events = round(&mut battle, &data, &defend());
        assert_eq!(
            zio_actions(&events),
            [FirstZioAction::BlackWave],
            "turn {turn}"
        );
        assert!(
            !events.iter().any(|e| matches!(
                e,
                BattleEvent::UnsupportedAbility { .. } | BattleEvent::Resolved { .. }
            )),
            "Black Wave is a kill, not a damage request: {events:?}"
        );
        dead += events
            .iter()
            .filter(|e| matches!(e, BattleEvent::Died { .. }))
            .count();
        if battle.outcome().is_some() {
            break;
        }
    }
    assert_eq!(dead, 3, "one member per turn, the third ends the fight");
    assert_eq!(battle.outcome(), Some(crate::battle::Outcome::Defeat));
    for member in battle.roster().side(crate::battle::Side::Party) {
        assert_eq!(member.stats.curr_hp, 0);
        assert_ne!(member.stats.status & status::OUT, 0);
    }
}

/// `EnemyAttack_Zio2` (`ps4.asm:19519`) with the counter at zero is the same
/// Magic Barrier; from the second action on it dispatches the roll, and the
/// three arms a Zio2 list holds each run their traced request.
#[test]
fn zio2_opens_with_the_barrier_then_dispatches_its_rolled_arms() {
    for (list, hits, name) in [
        ([CORRSION; 8], 3usize, "CORRSION"),
        ([HEWN; 8], 3, "HEWN"),
        ([BLACK_WAVE2; 8], 1, "BLACK WAVE"),
    ] {
        let data = data(list);
        let (mut battle, _) = start(140, &data);
        let first = round(&mut battle, &data, &defend());
        assert_eq!(zio_actions(&first), [MAGIC_BARRIER], "{name}");
        assert!(
            !first
                .iter()
                .any(|e| matches!(e, BattleEvent::Resolved { .. })),
            "{name}"
        );
        let second = round(&mut battle, &data, &defend());
        assert!(zio_actions(&second).is_empty(), "{name}: {second:?}");
        assert!(
            second
                .iter()
                .any(|e| matches!(e, BattleEvent::EnemySkillUsed { .. })),
            "{name}: {second:?}"
        );
        let resolved = second
            .iter()
            .filter(|e| matches!(e, BattleEvent::Resolved { .. }))
            .count();
        assert_eq!(resolved, hits, "{name}: {second:?}");
        assert!(
            !second.iter().any(|e| matches!(
                e,
                BattleEvent::UnsupportedAbility { .. } | BattleEvent::Attacked { .. }
            )),
            "{name}: nothing falls back: {second:?}"
        );
    }
}

/// Ability `0` in a Zio2 list is `loc_D3F0`'s first arm (`ps4.asm:19532`): the
/// ordinary attack object `$920`, whose chain ends in the single-target tail
/// like any swing. It must not be mistaken for the barrier or reported as an
/// unsupported ability.
#[test]
fn zio2_ability_zero_is_an_ordinary_swing() {
    let data = data([0; 8]);
    let (mut battle, _) = start(140, &data);
    let _ = round(&mut battle, &data, &defend());
    let second = round(&mut battle, &data, &defend());
    assert!(zio_actions(&second).is_empty());
    assert!(
        second
            .iter()
            .any(|e| matches!(e, BattleEvent::Attacked { .. })),
        "{second:?}"
    );
    assert!(
        !second
            .iter()
            .any(|e| matches!(e, BattleEvent::UnsupportedAbility { .. }))
    );
}

/// The Psycho Wand swaps 139 for 140 and runs no init routine
/// (`ps4.asm:79203-79215`, `loc_7F22`), so the counter Zio left at 2 survives:
/// the new Zio2's first action is not a second Magic Barrier.
#[test]
fn the_wand_keeps_the_counter_so_zio2_does_not_barrier_again() {
    let data = data([CORRSION; 8]);
    let (mut battle, _) = start(139, &data);
    assert_eq!(
        zio_actions(&round(&mut battle, &data, &defend())),
        [MAGIC_BARRIER]
    );
    let mut inventory = Inventory::new();
    inventory.add(PSYCHO_WAND).unwrap();
    let wand = RoundOrders::Commands(vec![
        Command::Item {
            item: PSYCHO_WAND,
            source: ItemSource::Inventory(0),
            target: None,
        },
        Command::Defend,
        Command::Defend,
    ]);
    let events = battle
        .round_with_inventory(&wand, &data, &mut inventory, &mut SliceRolls::new(&STREAM))
        .unwrap();
    // Round 2: Zio is first (agility 255) and runs Nightmare, then the wand.
    assert_eq!(zio_actions(&events), [FirstZioAction::Nightmare]);
    assert!(events.iter().any(|e| matches!(
        e,
        BattleEvent::EnemyStatsReloaded {
            enemy_id: 140,
            hp: 2889,
            ..
        }
    )));
    assert_eq!(battle.roster().get(id(6)).unwrap().stats.enemy_id, 140);
    let third = round(&mut battle, &data, &defend());
    assert!(zio_actions(&third).is_empty(), "{third:?}");
    assert_eq!(
        third
            .iter()
            .filter(|e| matches!(e, BattleEvent::Resolved { .. }))
            .count(),
        3,
        "the rolled CORRSION runs as the all-party request: {third:?}"
    );
}

/// Zio3 is the same counter, longer: the five-turn script and then an empty
/// turn forever, with Black Wave1 ending the encounter (`ScriptedExit`).
#[test]
fn zio3_is_generalised_not_copied() {
    let data = data([0; 8]);
    let (mut battle, _) = start(152, &data);
    let mut seen = Vec::new();
    for _ in 0..6 {
        seen.extend(zio_actions(&round(&mut battle, &data, &defend())));
        if battle.outcome().is_some() {
            break;
        }
    }
    assert_eq!(
        seen,
        [
            MAGIC_BARRIER,
            FirstZioAction::Invocation,
            FirstZioAction::Pause,
            FirstZioAction::Nightmare,
            FirstZioAction::BlackWave,
        ]
    );
    assert_eq!(battle.outcome(), Some(crate::battle::Outcome::ScriptedExit));
}
