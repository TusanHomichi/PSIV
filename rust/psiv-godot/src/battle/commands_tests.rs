use super::*;
use psiv_core::{CharId, GameState, RetailLocation, RetailSave, StepFrames};

fn id(n: u8) -> FighterId {
    FighterId::new(n).unwrap()
}

fn menu() -> Option<CommandsMenu> {
    let pack = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack"));
    if !pack.join("manifest.json").is_file() {
        eprintln!("runtime pack not present; skipping");
        return None;
    }
    let files = psiv_data::BattleFiles::load(pack).unwrap();
    let mut game = GameState::new();
    game.set_party([
        Some(CharId(0)),
        Some(CharId(1)),
        Some(CharId(2)),
        None,
        None,
    ]);
    let mut runtime = psiv_runtime::Runtime::from_save(
        psiv_data::GameData::load(pack).unwrap(),
        RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index_2: 0,
                map_index: 0x13,
                char_x: 48 * 16,
                char_y: 19 * 16,
            },
        },
        StepFrames::default(),
    )
    .unwrap();
    runtime.enable_battles(&files).unwrap();
    runtime.start_battle(0x8A, runtime.battle_party()).unwrap();
    Some(CommandsMenu::new(
        runtime.battle_roster().unwrap().clone(),
        runtime
            .battle_techniques()
            .map(|t| (t.id, t.clone()))
            .collect(),
        runtime.battle_skills().map(|s| (s.id, s.clone())).collect(),
        runtime.battle_armed_fighters(),
        runtime
            .battle_items()
            .map(|item| (item.id, item.clone()))
            .collect(),
        runtime.game().inventory().clone(),
    ))
}

#[test]
fn individual_commands_keep_fighter_slots_and_selected_targets() {
    let Some(mut menu) = menu() else {
        return;
    };
    menu.move_cursor(1);
    menu.accept(); // Chaz TECH
    menu.accept(); // RES
    menu.move_cursor(1);
    menu.accept(); // Alys recipient
    menu.move_cursor(4);
    menu.accept(); // Alys DEFEND
    menu.accept(); // Hahn ATTACK
    menu.move_cursor(1);
    let orders = menu.accept().unwrap();
    assert_eq!(
        orders,
        RoundOrders::Commands(vec![
            Command::Technique {
                technique: 24,
                target: Some(id(2))
            },
            Command::Defend,
            Command::AttackTarget(id(7)),
            Command::Defend,
            Command::Defend
        ])
    );
    assert_eq!(
        menu.roster.get(id(1)).unwrap().stats.curr_tp,
        10,
        "menu never spends TP"
    );
}

#[test]
fn cancel_unwinds_targets_techniques_and_prior_character_without_submitting() {
    let Some(mut menu) = menu() else {
        return;
    };
    menu.move_cursor(1);
    menu.accept();
    menu.accept();
    menu.cancel();
    assert_eq!(menu.page, Page::Techniques);
    menu.cancel();
    assert_eq!(menu.page, Page::Actions);
    menu.move_cursor(4);
    menu.accept();
    assert_eq!(menu.actor_id(), Some(id(2)));
    menu.cancel();
    assert_eq!(menu.actor_id(), Some(id(1)));
    menu.cancel();
    assert!(!menu.open);
}

#[test]
fn insufficient_tp_disables_accept_and_leaves_selection_open() {
    let Some(mut menu) = menu() else {
        return;
    };
    menu.roster.get_mut(id(1)).unwrap().stats.curr_tp = 2;
    menu.move_cursor(1);
    menu.accept();
    assert!(!menu.rows()[0].1);
    assert_eq!(menu.accept(), None);
    assert_eq!(menu.page, Page::Techniques);
    assert!(menu.open);
}

#[test]
fn incapacitated_slots_are_skipped_and_group_tech_needs_no_target_cursor() {
    let Some(menu) = menu() else {
        return;
    };
    let mut roster = menu.roster;
    roster.get_mut(id(1)).unwrap().stats.status = status::PARALYZED;
    roster.get_mut(id(3)).unwrap().stats.status = status::DEAD;
    let mut menu = CommandsMenu::new(
        roster,
        menu.techniques,
        menu.skills,
        menu.armed_fighters,
        menu.items,
        menu.inventory,
    );
    assert_eq!(menu.actor_id(), Some(id(2)));
    menu.move_cursor(1);
    menu.accept();
    assert!(
        menu.rows()[0].0.starts_with("SANER"),
        "retail lists newest learned first"
    );
    let RoundOrders::Commands(orders) = menu.accept().unwrap() else {
        panic!("commands")
    };
    assert_eq!(
        orders[1],
        Command::Technique {
            technique: 31,
            target: None
        }
    );
    assert_eq!(orders[0], Command::Defend);
}

#[test]
fn skills_keep_ids_and_targets_without_spending_uses_in_the_menu() {
    let Some(mut menu) = menu() else {
        return;
    };
    for (actor, name, target) in [
        (1, "EARTH 3 OF 3", Some(0)),
        (2, "VORTEX 5 OF 5", Some(1)),
        (3, "VISION 5 OF 5", None),
    ] {
        assert_eq!(menu.actor_id(), Some(id(actor)));
        menu.move_cursor(2);
        menu.accept();
        assert_eq!(menu.rows(), vec![(name.into(), true)]);
        let mut result = menu.accept();
        if let Some(cursor) = target {
            menu.move_cursor(cursor);
            result = menu.accept();
        }
        if actor == 3 {
            assert_eq!(
                result,
                Some(RoundOrders::Commands(vec![
                    Command::Skill {
                        skill: 31,
                        target: Some(id(6))
                    },
                    Command::Skill {
                        skill: 6,
                        target: Some(id(7))
                    },
                    Command::Skill {
                        skill: 47,
                        target: None
                    },
                    Command::Defend,
                    Command::Defend,
                ]))
            );
        } else {
            assert!(result.is_none());
        }
    }
    assert_eq!(menu.roster.get(id(1)).unwrap().stats.curr_skill_uses[0], 3);
    assert_eq!(menu.roster.get(id(2)).unwrap().stats.curr_skill_uses[0], 5);
    assert_eq!(menu.roster.get(id(3)).unwrap().stats.curr_skill_uses[0], 5);
}

#[test]
fn skills_reject_exhausted_unarmed_and_unsupported_but_work_when_tech_sealed() {
    let Some(mut menu) = menu() else {
        return;
    };
    menu.roster.get_mut(id(1)).unwrap().stats.status = status::TECH_SEALED;
    menu.roster.get_mut(id(1)).unwrap().stats.curr_tp = 0;
    menu.move_cursor(2);
    menu.accept();
    assert!(menu.rows()[0].1);
    menu.roster.get_mut(id(1)).unwrap().stats.curr_skill_uses[0] = 0;
    assert_eq!(menu.rows()[0], ("EARTH 0 OF 3".into(), false));
    assert_eq!(menu.accept(), None);
    assert_eq!(menu.page, Page::Skills);
    menu.roster.get_mut(id(1)).unwrap().stats.curr_skill_uses[0] = 3;
    menu.armed_fighters.clear();
    assert!(!menu.rows()[0].1);
    menu.armed_fighters.push(id(1));
    menu.skills.get_mut(&31).unwrap().effect = 2;
    assert!(!menu.rows()[0].1);
}

#[test]
fn cancel_returns_to_the_selected_skill_without_spending_a_use() {
    let Some(mut menu) = menu() else {
        return;
    };
    menu.move_cursor(2);
    menu.accept();
    menu.accept();
    assert_eq!(menu.page, Page::Targets(TargetAction::Skill(31)));
    menu.cancel();
    assert_eq!(menu.page, Page::Skills);
    assert_eq!(menu.cursor(), 0);
    assert_eq!(menu.rows()[0].0, "EARTH 3 OF 3");
    menu.cancel();
    assert_eq!(menu.page, Page::Actions);
    menu.cancel();
    assert!(!menu.open);
}

fn select_item(menu: &mut CommandsMenu, source: ItemSource) {
    menu.move_cursor(3);
    menu.accept();
    assert_eq!(menu.page, Page::Items);
    let row = menu
        .known_items()
        .iter()
        .position(|(s, _)| *s == source)
        .unwrap();
    menu.move_cursor(row as isize);
}

#[test]
fn item_reservations_distinguish_copies_and_keep_inventory_holes() {
    let Some(mut menu) = menu() else {
        return;
    };
    let mut slots = [0; 40];
    slots[0] = 125;
    slots[7] = 125;
    menu.inventory = psiv_core::Inventory::from_slots(slots);
    select_item(&mut menu, ItemSource::Inventory(7));
    menu.accept();
    menu.accept(); // Chaz -> Monomate in slot 7 -> Chaz
    select_item(&mut menu, ItemSource::Inventory(7));
    assert!(!menu.rows()[menu.cursor()].1);
    assert_eq!(menu.accept(), None);
    assert_eq!(menu.page, Page::Items);
    let free = menu
        .known_items()
        .iter()
        .position(|(s, _)| *s == ItemSource::Inventory(0))
        .unwrap();
    menu.cursor = free;
    assert!(menu.rows()[free].1);
    menu.accept();
    menu.move_cursor(2);
    menu.accept(); // Alys -> slot 0 -> Hahn
    menu.move_cursor(4);
    let RoundOrders::Commands(orders) = menu.accept().unwrap() else {
        panic!("orders");
    };
    assert_eq!(
        orders[0],
        Command::Item {
            item: 125,
            source: ItemSource::Inventory(7),
            target: Some(id(1))
        }
    );
    assert_eq!(
        orders[1],
        Command::Item {
            item: 125,
            source: ItemSource::Inventory(0),
            target: Some(id(3))
        }
    );
    assert_eq!(
        menu.inventory.occupied(),
        2,
        "selection never consumes items"
    );
}

#[test]
fn backing_up_and_replacing_an_order_releases_its_item_reservation() {
    let Some(mut menu) = menu() else {
        return;
    };
    menu.inventory.add(125).unwrap();
    select_item(&mut menu, ItemSource::Inventory(0));
    menu.accept();
    menu.cancel();
    assert_eq!(menu.page, Page::Items);
    assert_eq!(
        menu.known_items()[menu.cursor()].0,
        ItemSource::Inventory(0)
    );
    menu.accept();
    menu.accept(); // commit Chaz
    menu.cancel(); // back from Alys to Chaz
    menu.move_cursor(4);
    menu.accept(); // replace Chaz with Defend
    select_item(&mut menu, ItemSource::Inventory(0));
    assert!(menu.rows()[menu.cursor()].1);
    assert_eq!(menu.inventory.get(0), Some(125));
}

#[test]
fn item_targets_include_downed_allies_and_unusable_items_stay_disabled() {
    let Some(mut menu) = menu() else {
        return;
    };
    menu.inventory.add(130).unwrap();
    menu.inventory.add(132).unwrap();
    menu.roster.get_mut(id(3)).unwrap().stats.status = status::DEAD;
    select_item(&mut menu, ItemSource::Inventory(0));
    menu.accept();
    assert_eq!(
        menu.targets(TargetAction::Item {
            item: 130,
            source: ItemSource::Inventory(0)
        }),
        vec![id(1), id(2), id(3)]
    );
    menu.move_cursor(2);
    menu.accept();
    assert_eq!(
        menu.orders[0],
        Command::Item {
            item: 130,
            source: ItemSource::Inventory(0),
            target: Some(id(3))
        }
    );
    select_item(&mut menu, ItemSource::Inventory(1));
    assert!(!menu.rows()[menu.cursor()].1);
    assert_eq!(menu.accept(), None);
}
