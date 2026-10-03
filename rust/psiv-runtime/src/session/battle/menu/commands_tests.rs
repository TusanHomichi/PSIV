//! The command window's own tests, ported from the Godot crate's
//! `battle/commands_tests.rs` with the S3 node plus the pad-mapping tests the
//! move made possible.

use super::*;
use crate::pad::Button;
use crate::session::battle::menu::{TopChoice, confirm_pressed, top_input, vehicle_slots};
use psiv_core::{CharId, GameState, RetailLocation, RetailSave, StepFrames};

/// Formation `$8A`, the Academy Basement party the shell fixtures use.
const FORMATION: u16 = 0x8A;

fn id(n: u8) -> FighterId {
    FighterId::new(n).unwrap()
}

fn pack() -> &'static std::path::Path {
    std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack"))
}

fn battle() -> Option<Runtime> {
    let pack = pack();
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
    let mut runtime = Runtime::from_save(
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
    runtime
        .start_battle(FORMATION, runtime.battle_party())
        .unwrap();
    Some(runtime)
}

fn menu() -> Option<(Runtime, CommandsMenu)> {
    let runtime = battle()?;
    let menu = CommandsMenu::new(&runtime);
    Some((runtime, menu))
}

#[test]
fn individual_commands_keep_fighter_slots_and_selected_targets() {
    let Some((_runtime, mut menu)) = menu() else {
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
    let Some((_runtime, mut menu)) = menu() else {
        return;
    };
    menu.move_cursor(1);
    menu.accept();
    menu.accept();
    menu.cancel();
    assert_eq!(menu.page, MenuPage::Techniques);
    menu.cancel();
    assert_eq!(menu.page, MenuPage::Actions);
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
    let Some((_runtime, mut menu)) = menu() else {
        return;
    };
    menu.roster.get_mut(id(1)).unwrap().stats.curr_tp = 2;
    menu.move_cursor(1);
    menu.accept();
    assert!(!menu.rows()[0].1);
    assert_eq!(menu.accept(), None);
    assert_eq!(menu.page, MenuPage::Techniques);
    assert!(menu.open);
}

#[test]
fn incapacitated_slots_are_skipped_and_group_tech_needs_no_target_cursor() {
    let Some((_runtime, menu)) = menu() else {
        return;
    };
    let mut roster = menu.roster;
    roster.get_mut(id(1)).unwrap().stats.status = status::PARALYZED;
    roster.get_mut(id(3)).unwrap().stats.status = status::DEAD;
    let mut menu = CommandsMenu {
        roster,
        techniques: menu.techniques,
        skills: menu.skills,
        armed_fighters: menu.armed_fighters,
        items: menu.items,
        inventory: menu.inventory,
        actors: Vec::new(),
        actor: 0,
        page: MenuPage::Actions,
        cursor: 0,
        orders: vec![Command::Defend; 5],
        age: 0,
        bytes: [0; 5],
        open: true,
    };
    menu.actors = menu
        .roster
        .side(Side::Party)
        .filter(|f| f.is_alive() && f.stats.can_act())
        .map(|f| f.id)
        .collect();
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
fn an_alive_but_incapacitated_party_has_no_second_comd_press() {
    let Some((_runtime, mut menu)) = menu() else {
        return;
    };
    for who in [id(1), id(2), id(3)] {
        menu.roster.get_mut(who).unwrap().stats.status = status::PARALYZED;
    }
    menu.actors = menu
        .roster
        .side(Side::Party)
        .filter(|fighter| fighter.is_alive() && fighter.stats.can_act())
        .map(|fighter| fighter.id)
        .collect();
    assert_eq!(menu.roster.living(Side::Party).count(), 3);
    assert!(menu.actors.is_empty(), "`$6E` skips every party actor");
    assert_eq!(
        menu.no_actor_orders(),
        Some(RoundOrders::Commands(vec![Command::Defend; 5]))
    );
}

#[test]
fn skills_keep_ids_and_targets_without_spending_uses_in_the_menu() {
    let Some((_runtime, mut menu)) = menu() else {
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
    let Some((_runtime, mut menu)) = menu() else {
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
    assert_eq!(menu.page, MenuPage::Skills);
    menu.roster.get_mut(id(1)).unwrap().stats.curr_skill_uses[0] = 3;
    menu.armed_fighters.clear();
    assert!(!menu.rows()[0].1);
    menu.armed_fighters.push(id(1));
    menu.skills.get_mut(&31).unwrap().effect = 2;
    assert!(!menu.rows()[0].1);
}

#[test]
fn cancel_returns_to_the_selected_skill_without_spending_a_use() {
    let Some((_runtime, mut menu)) = menu() else {
        return;
    };
    menu.move_cursor(2);
    menu.accept();
    menu.accept();
    assert_eq!(menu.page, MenuPage::Targets(TargetKind::Skill(31)));
    menu.cancel();
    assert_eq!(menu.page, MenuPage::Skills);
    assert_eq!(menu.cursor, 0);
    assert_eq!(menu.rows()[0].0, "EARTH 3 OF 3");
    menu.cancel();
    assert_eq!(menu.page, MenuPage::Actions);
    menu.cancel();
    assert!(!menu.open);
}

fn select_item(menu: &mut CommandsMenu, source: ItemSource) {
    menu.move_cursor(3);
    menu.accept();
    assert_eq!(menu.page, MenuPage::Items);
    let row = menu
        .known_items()
        .iter()
        .position(|(s, _)| *s == source)
        .unwrap();
    menu.move_cursor(row as isize);
}

#[test]
fn item_reservations_distinguish_copies_and_keep_inventory_holes() {
    let Some((_runtime, mut menu)) = menu() else {
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
    assert!(!menu.rows()[menu.cursor].1);
    assert_eq!(menu.accept(), None);
    assert_eq!(menu.page, MenuPage::Items);
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
    let Some((_runtime, mut menu)) = menu() else {
        return;
    };
    menu.inventory.add(125).unwrap();
    select_item(&mut menu, ItemSource::Inventory(0));
    menu.accept();
    menu.cancel();
    assert_eq!(menu.page, MenuPage::Items);
    assert_eq!(menu.known_items()[menu.cursor].0, ItemSource::Inventory(0));
    menu.accept();
    menu.accept(); // commit Chaz
    menu.cancel(); // back from Alys to Chaz
    menu.move_cursor(4);
    menu.accept(); // replace Chaz with Defend
    select_item(&mut menu, ItemSource::Inventory(0));
    assert!(menu.rows()[menu.cursor].1);
    assert_eq!(menu.inventory.get(0), Some(125));
}

#[test]
fn item_targets_include_downed_allies_and_unusable_items_stay_disabled() {
    let Some((_runtime, mut menu)) = menu() else {
        return;
    };
    menu.inventory.add(130).unwrap();
    menu.inventory.add(132).unwrap();
    menu.roster.get_mut(id(3)).unwrap().stats.status = status::DEAD;
    select_item(&mut menu, ItemSource::Inventory(0));
    menu.accept();
    assert_eq!(
        menu.targets(TargetKind::Item {
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
    assert!(!menu.rows()[menu.cursor].1);
    assert_eq!(menu.accept(), None);
}

/// The main options are the cartridge's own window: Up/Down wrap
/// (`Battle_UpdateRedCursor2`, `ps4.asm:1572`, from `Battle_MainOptions`
/// `ps4.asm:1916`), Left/Right do nothing there, and `ButtonSpeak|ButtonCamp`
/// accepts (`ps4.asm:1926`).
#[test]
fn the_main_options_wrap_on_up_and_down_and_ignore_left_right() {
    let mut cursor = 0;
    let none = Pad::NEUTRAL;
    assert_eq!(
        top_input(&mut cursor, false, none, Pad::new(Button::Left)),
        TopChoice::Nothing
    );
    assert_eq!(cursor, 0, "the main options have no Left rule");
    assert_eq!(
        top_input(&mut cursor, false, none, Pad::new(Button::Right)),
        TopChoice::Nothing
    );
    assert_eq!(cursor, 0);
    top_input(&mut cursor, false, none, Pad::new(Button::Up));
    assert_eq!(cursor, 2, "Up from the first row wraps to the last");
    top_input(&mut cursor, false, none, Pad::new(Button::Down));
    assert_eq!(cursor, 0, "Down from the last row wraps to the first");
    // The press edge, not the level: a held button is one step.
    let held = Pad::new(Button::Down);
    top_input(&mut cursor, false, held, held);
    assert_eq!(cursor, 0);
    assert_eq!(
        top_input(&mut cursor, false, none, held),
        TopChoice::Nothing,
        "the second row (MACR) chooses nothing"
    );
    for (row, choice) in [
        (0, TopChoice::Commands),
        (1, TopChoice::Nothing),
        (2, TopChoice::Run),
    ] {
        let mut cursor = row;
        assert_eq!(
            top_input(&mut cursor, false, none, Pad::new(Button::Speak)),
            choice,
            "row {row}"
        );
        let mut cursor = row;
        assert_eq!(
            top_input(&mut cursor, false, none, Pad::new(Button::Camp)),
            choice,
            "row {row} takes Genesis A too"
        );
    }
}

/// The mounted surface's main options are ATTAC / OPTIN / RUN
/// (`Battle_VehMainOptions`, `ps4.asm:2003`; the routine table at
/// `ps4.asm:2071`).
#[test]
fn the_mounted_main_options_open_the_skill_window_on_the_middle_row() {
    let none = Pad::NEUTRAL;
    let mut cursor = 0;
    assert_eq!(
        top_input(&mut cursor, true, none, Pad::new(Button::Speak)),
        TopChoice::AttackAll
    );
    cursor = 1;
    assert_eq!(
        top_input(&mut cursor, true, none, Pad::new(Button::Speak)),
        TopChoice::VehicleSkills
    );
    cursor = 2;
    assert_eq!(
        top_input(&mut cursor, true, none, Pad::new(Button::Speak)),
        TopChoice::Run
    );
}

/// The post-battle pages take any face button: `Battle_VictoryMessage`
/// (`ps4.asm:4706`) and `Battle_LastMessage` (`ps4.asm:6351`) both mask
/// `ButtonCancel|ButtonSpeak|ButtonCamp`.
#[test]
fn the_post_battle_pages_confirm_on_any_face_button() {
    let none = Pad::NEUTRAL;
    for button in [Button::Cancel, Button::Speak, Button::Camp] {
        assert!(
            confirm_pressed(none, Pad::new(button)),
            "{button} continues the results pages"
        );
    }
    for button in [Button::Up, Button::Down, Button::Left, Button::Right] {
        assert!(!confirm_pressed(none, Pad::new(button)));
    }
    // A held button is one press.
    let held = Pad::new(Button::Speak);
    assert!(!confirm_pressed(held, held));
}

/// The strip is the cartridge's `Battle_CharCommand` (`ps4.asm:2192`): the
/// cursor goes through `Battle_UpdateCursor` (`ps4.asm:70646`), which tests
/// Left and Right only and wraps over the five icons (`d1 = 4`).
#[test]
fn the_strip_moves_on_left_and_right_only_and_wraps() {
    let Some((_runtime, mut menu)) = menu() else {
        return;
    };
    let none = Pad::NEUTRAL;
    for button in [Button::Up, Button::Down] {
        menu.input(none, Pad::new(button));
        assert_eq!(menu.cursor, 0, "{button} is not tested by the strip");
    }
    menu.input(none, Pad::new(Button::Right));
    assert_eq!(menu.cursor, 1);
    menu.input(none, Pad::new(Button::Left));
    menu.input(none, Pad::new(Button::Left));
    assert_eq!(menu.cursor, 4, "Left from the first icon wraps to the last");
    menu.input(none, Pad::new(Button::Right));
    assert_eq!(
        menu.cursor, 0,
        "Right from the last icon wraps to the first"
    );
    // Accept and cancel are the cartridge's too.
    assert_eq!(menu.input(none, Pad::new(Button::Speak)), None);
    assert_eq!(menu.page, MenuPage::Targets(TargetKind::Attack));
    menu.input(none, Pad::new(Button::Cancel));
    assert_eq!(menu.page, MenuPage::Actions);
}

/// The list windows run `Battle_UpdateRedCursor2` (`ps4.asm:1572`) over a
/// page's four rows: Up and Down wrap inside the page, whether or not the row
/// holds an entry, and only Right (when a next page exists) and Left (when a
/// previous one does) change page (`Battle_TechWindow`, `ps4.asm:2593`).
#[test]
fn a_list_window_wraps_inside_its_page_and_flips_pages_on_left_and_right() {
    let Some((_runtime, mut menu)) = menu() else {
        return;
    };
    let usable: Vec<u8> = menu
        .techniques
        .values()
        .filter(|tech| tech.targeting & 0x10 != 0)
        .map(|tech| tech.id)
        .take(6)
        .collect();
    assert_eq!(usable.len(), 6, "the pack has six in-battle techniques");
    let actor = menu.actor_id().unwrap();
    {
        let stats = &mut menu.roster.get_mut(actor).unwrap().stats;
        stats.techniques = [0; 16];
        for (slot, id) in usable.iter().enumerate() {
            stats.techniques[slot] = *id;
        }
    }
    menu.page = MenuPage::Techniques;
    menu.cursor = 0;
    let none = Pad::NEUTRAL;
    let press = |menu: &mut CommandsMenu, button| {
        menu.input(none, Pad::new(button));
    };
    press(&mut menu, Button::Up);
    assert_eq!(
        menu.cursor, 3,
        "Up from the first row wraps within the page"
    );
    press(&mut menu, Button::Down);
    assert_eq!(
        menu.cursor, 0,
        "Down from the fourth row wraps within the page"
    );
    press(&mut menu, Button::Left);
    assert_eq!(menu.cursor, 0, "no previous page: Left does nothing");
    press(&mut menu, Button::Right);
    assert_eq!(menu.cursor, 4, "Right flips to the next page's first row");
    let view = menu.view().list.expect("a list window");
    assert_eq!((view.page, view.entries.len()), (1, 2));
    assert!(view.more_before && !view.more_after);
    for expected in [5, 6, 7, 4] {
        press(&mut menu, Button::Down);
        assert_eq!(
            menu.cursor, expected,
            "rows past the entries still hold the cursor"
        );
    }
    press(&mut menu, Button::Right);
    assert_eq!(menu.cursor, 4, "no next page: Right does nothing");
    press(&mut menu, Button::Left);
    assert_eq!(menu.cursor, 0, "Left flips back to the first page");
    // An empty row ignores accept; leaving the list returns to the strip on
    // the icon that opened it.
    menu.cursor = 7;
    assert_eq!(menu.accept(), None);
    assert_eq!(menu.page, MenuPage::Techniques);
    menu.cancel();
    assert_eq!((menu.page, menu.cursor), (MenuPage::Actions, 1));
}

/// The pane icon follows the command byte the way the cartridge writes it:
/// 2 as the technique window opens, back to 0 when the window is left, 5 on
/// defend, and the next record is cleared as a strip opens
/// (`ps4.asm:1476`, `2777`).
#[test]
fn command_bytes_follow_the_cartridge_writes() {
    let Some((_runtime, mut menu)) = menu() else {
        return;
    };
    menu.bytes = [4, 4, 4, 4, 4];
    menu.open_actor();
    assert_eq!(
        menu.bytes,
        [4, 0, 4, 4, 4],
        "opening a strip clears the next record"
    );
    menu.cursor = 1;
    menu.accept();
    assert_eq!(menu.bytes[menu.command_slot().unwrap()], 2);
    menu.cancel();
    assert_eq!(menu.bytes[menu.command_slot().unwrap()], 0);
    menu.cursor = 4;
    menu.accept();
    assert_eq!(menu.bytes[0], 5, "defend writes its own byte");
}

/// The mounted window reads the record, not a copy: the engine subtracts the
/// use when the command resolves (`psiv-core/src/battle/engine.rs`,
/// `Command::VehicleSkill`), and the window shows the same number the record
/// carries.
#[test]
fn mounted_skill_slots_read_the_live_battle_record() {
    let pack = pack();
    if !pack.join("manifest.json").is_file() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let files = psiv_data::BattleFiles::load(pack).unwrap();
    let mut runtime = Runtime::new(
        psiv_data::GameData::load(pack).unwrap(),
        0x13,
        psiv_core::Cell { x: 48, y: 19 },
        psiv_core::Direction::Down,
        StepFrames::default(),
    )
    .unwrap();
    runtime.enable_battles(&files).unwrap();
    runtime.set_vehicle_index(1).unwrap();
    let party = runtime.battle_party();
    assert_eq!(party.len(), 1, "a mounted battle seats the vehicle alone");
    let skills: Vec<u8> = party[0]
        .stats
        .skills
        .iter()
        .copied()
        .filter(|id| *id != 0)
        .collect();
    if skills.is_empty() {
        eprintln!("the saved vehicle record carries no skill; skipping");
        return;
    }
    runtime.start_battle(FORMATION, party).unwrap();
    let before = vehicle_slots(&runtime);
    assert_eq!(before.len(), skills.len());
    assert_eq!(before[0].id, skills[0]);
    let used = before[0].current;
    let orders = RoundOrders::Commands(vec![Command::VehicleSkill(before[0].id)]);
    runtime.battle_round(&orders).unwrap();
    let after = vehicle_slots(&runtime);
    assert_eq!(
        after[0].current,
        used.saturating_sub(1),
        "the window reads the record the fight spent"
    );
}
