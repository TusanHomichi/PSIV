//! Asset-independent regressions for list-to-strip closing transitions.
use super::*;
use crate::session::battle::blink::CursorBlink;
use psiv_core::battle::CharacterRecord;

fn fixture(page: MenuPage) -> CommandsMenu {
    let record = CharacterRecord {
        id: 0,
        name: "Actor".into(),
        profession: 0,
        level: 1,
        experience: 0,
        hp: 100,
        max_hp: 100,
        tp: 100,
        max_tp: 100,
        strength: 10,
        mental: 10,
        agility: 10,
        dexterity: 10,
        properties: [0; 14],
        equipment: [0; 4],
        techniques: [1; 16],
        skills: [31; 8],
        skill_uses: [10; 8],
    };
    let mut roster = Roster::new();
    let actor = roster
        .add_party_member(
            0,
            record.name.clone(),
            Stats::from_character(&record, |_| None),
        )
        .unwrap();
    let mut inventory = psiv_core::Inventory::new();
    for _ in 0..8 {
        inventory.add(1).unwrap();
    }
    let mut memory = CursorMemory::default();
    memory.remember(
        0,
        MenuPage::Actions,
        match page {
            MenuPage::Techniques => 1,
            MenuPage::Skills => 2,
            _ => 3,
        },
    );
    CommandsMenu {
        roster,
        techniques: [(
            1,
            Technique {
                id: 1,
                name: "Group".into(),
                effect: 7,
                cost: 0,
                targeting: 0x12,
                power: 1,
                resistance: 0,
                element: 0,
            },
        )]
        .into(),
        skills: [(
            31,
            Skill {
                id: 31,
                name: "Group".into(),
                effect: 7,
                power_stat: 0,
                requires_weapon: false,
                targeting: 0x12,
                power: 1,
                resistance: 0,
                element: 0,
            },
        )]
        .into(),
        items: [(
            1,
            BattleItem {
                id: 1,
                name: "Group".into(),
                effect: 7,
                actor_power: 1,
                targeting: 2,
                power: 1,
                resistance: 0,
                element: 0,
                object: 6,
                consumable: true,
            },
        )]
        .into(),
        armed_fighters: vec![],
        inventory,
        actors: vec![actor],
        actor: 0,
        page,
        cursor: 7,
        orders: vec![Command::Defend; 5],
        age: 42,
        bytes: [0; 5],
        memory,
        phase: None,
        pending: None,
        whole_unread: false,
        open: true,
    }
}

const LISTS: [MenuPage; 3] = [MenuPage::Techniques, MenuPage::Skills, MenuPage::Items];

#[test]
fn later_page_no_target_choices_restore_the_strip_cursor_through_every_close_frame() {
    for page in LISTS {
        let mut menu = fixture(page);
        assert!(menu.rows()[7].1, "{page:?}: later-page choice enabled");
        menu.input(Pad::NEUTRAL, Pad::new(Button::Speak));
        assert!(menu.pending.is_some(), "{page:?}: no target required");
        let expected = menu.memory.cursor(0, MenuPage::Actions);
        let mut strip_frames = 0;
        while menu.open {
            let view = menu.view();
            if view.page == MenuPage::Actions {
                strip_frames += 1;
                assert_eq!(view.cursor, expected, "{page:?}");
                assert!(view.cursor < 5);
                if let Some(strip) = view.strip {
                    assert_eq!(usize::from(strip.cursor), expected);
                }
            }
            menu.input(Pad::NEUTRAL, Pad::NEUTRAL);
        }
        assert_eq!(strip_frames, 8, "full strip, then seven narrowing frames");
        assert!(!matches!(menu.orders[0], Command::Defend));
    }
}

#[test]
fn all_list_closing_presses_keep_the_cursor_then_pause_input_and_blink() {
    for page in LISTS {
        for button in [Button::Speak, Button::Cancel, Button::Left] {
            let mut menu = fixture(page);
            let mut blink = CursorBlink::from_timers(10, 1);
            // The battle mode ticks before it reads the command, exactly once.
            assert!(menu.red_cursor_window());
            blink.tick(true, button == Button::Speak);
            menu.input(Pad::NEUTRAL, Pad::new(button));
            assert!(
                menu.list_cursor_shown(),
                "{page:?} {button:?}: full closing frame"
            );
            assert!(blink.red);
            assert!(menu.view().list.is_some());
            let frozen = blink;
            let cursor = menu.cursor;
            let age = menu.age;
            // Stop before the last close frame changes page/starts another phase.
            let frames = window_width(page).unwrap().div_ceil(2);
            for _ in 1..frames {
                assert!(!menu.red_cursor_window());
                if menu.red_cursor_window() {
                    blink.tick(true, false);
                }
                menu.input(Pad::NEUTRAL, Pad::new(Button::Down));
                assert_eq!(menu.cursor, cursor);
                assert_eq!(menu.age, age);
                assert_eq!(blink, frozen);
                assert!(!menu.list_cursor_shown());
                assert!(menu.view().list.is_none());
            }
        }
    }
}

#[test]
fn opening_lists_suppress_the_cursor_until_the_first_read_frame() {
    for page in LISTS {
        let mut menu = fixture(page);
        menu.start_growth();
        while menu.growing() {
            assert!(!menu.list_cursor_shown());
            assert!(!menu.red_cursor_window());
            menu.input(Pad::NEUTRAL, Pad::NEUTRAL);
        }
        assert!(!menu.list_cursor_shown(), "whole but unread");
        menu.input(Pad::NEUTRAL, Pad::NEUTRAL);
        assert!(menu.list_cursor_shown());
    }
}
