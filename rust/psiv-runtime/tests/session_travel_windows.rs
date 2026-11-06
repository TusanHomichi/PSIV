//! The TELEPIPE town list and its message, through `Session::frame` with pads
//! only (#128).
//!
//! Retail's list (`Win_TelepipePlaceList`, `ps4.asm:123998-124213`) shows the
//! visited towns five to a page, each page its own window. Up and Down wrap
//! within the page; Right *held* opens the next page and Left *held* closes
//! the current one, each leaving the cursor on the first row. The confirmed
//! town opens `Win_ItemTeleportMsg`'s first line, and a refused TELEPIPE,
//! ESCAPIPE, RYUKA or HINAS shows the cartridge's refusal: each window line is
//! the pack's (`travel.json`'s `messages`), never a copy in the runtime.

use std::path::Path;

use psiv_core::{CharId, Flag, GameState, RetailLocation, RetailSave};
use psiv_data::{BattleFiles, GameData, TravelMessages};
use psiv_runtime::{
    Button, CampPage, ESCAPIPE, HINAS, Pad, RYUKA, Session, TELEPIPE, TOWN_PAGE_ROWS,
};

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");
/// The Academy corridor the camp fixtures use: no scene, no counter.
const FIELD: (u16, u16, u16) = (0x13, 48, 19);

fn session() -> Option<Session> {
    session_with(&[TELEPIPE], 0)
}

/// Chaz alone in the corridor with `items` in the pack, RYUKA and HINAS
/// known and `tp` to cast them.
fn session_with(items: &[u8], tp: u16) -> Option<Session> {
    let path = Path::new(PACK);
    if !path.join("manifest.json").is_file() {
        eprintln!("runtime pack not present; skipping");
        return None;
    }
    let data = GameData::load(path).expect("pack loads");
    let files = BattleFiles::load(path).expect("battle files load");
    let initial = Session::start(data.clone())
        .with_battles(files.clone())
        .new_game()
        .expect("START");
    let mut game = GameState::from_snapshot(&initial.runtime().game().snapshot());
    game.set_party([Some(CharId(0)), None, None, None, None]);
    // Every Motavia town visited, the story past the Piata guards.
    for town in 0..16 {
        game.set(Flag::town(town)).unwrap();
    }
    game.set(Flag::event(0x0C)).unwrap();
    while game.inventory().slots().iter().any(|id| *id != 0) {
        let slot = game
            .inventory()
            .slots()
            .iter()
            .position(|id| *id != 0)
            .unwrap();
        game.inventory_mut().remove_in_field(slot);
    }
    for &item in items {
        game.inventory_mut().add(item).unwrap();
    }
    let chaz = game.roster_mut().get_mut(CharId(0)).unwrap();
    chaz.techniques = [RYUKA, HINAS, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    chaz.curr_tp = tp;
    chaz.status = 0;
    let (map, x, y) = FIELD;
    Some(
        Session::start(data)
            .with_battles(files)
            .from_save(RetailSave {
                snapshot: game.snapshot(),
                location: RetailLocation {
                    world_index: 0,
                    map_index_2: 0,
                    map_index: map,
                    char_x: x * 16,
                    char_y: y * 16,
                },
            })
            .expect("the session starts"),
    )
}

fn press(session: &mut Session, button: Button) {
    session.frame(Pad::new(button));
    session.frame(Pad::NEUTRAL);
}

fn page(session: &Session) -> CampPage {
    session.camp_view().expect("the camp is up").page
}

fn selection(session: &Session) -> usize {
    session.camp_view().unwrap().travel_selection
}

/// Camp, ITEM, the TELEPIPE, and whatever the item path asks for until the
/// town list is up.
fn open_town_list(session: &mut Session) {
    press(session, Button::Camp);
    press(session, Button::Speak); // ITEM
    for _ in 0..4 {
        if page(session) == CampPage::TravelTowns {
            return;
        }
        press(session, Button::Speak);
    }
    panic!("the town list never opened: {:?}", page(session));
}

#[test]
fn the_town_list_pages_with_left_and_right_and_wraps_within_a_page() {
    let Some(mut session) = session() else { return };
    open_town_list(&mut session);
    let count = session.camp_view().unwrap().travel_towns.len();
    assert!(count > 2 * TOWN_PAGE_ROWS, "{count} towns span three pages");

    // Down five times comes back to the first row: the cursor wraps within
    // the page instead of running onto the next one.
    for _ in 0..TOWN_PAGE_ROWS {
        press(&mut session, Button::Down);
    }
    assert_eq!(selection(&session), 0);
    press(&mut session, Button::Up);
    assert_eq!(
        selection(&session),
        TOWN_PAGE_ROWS - 1,
        "Up wraps to the page's last row"
    );

    // Right held opens the next page with the cursor on its first row; held
    // on the next frame it opens the one after.
    session.frame(Pad::new(Button::Right));
    assert_eq!(selection(&session), TOWN_PAGE_ROWS);
    session.frame(Pad::new(Button::Right));
    assert_eq!(selection(&session), 2 * TOWN_PAGE_ROWS);
    session.frame(Pad::NEUTRAL);
    // Left held closes a page.
    session.frame(Pad::new(Button::Left));
    assert_eq!(selection(&session), TOWN_PAGE_ROWS);
    session.frame(Pad::NEUTRAL);

    // The last page wraps over its own rows only.
    let last = (count - 1) / TOWN_PAGE_ROWS;
    for _ in 0..last {
        session.frame(Pad::new(Button::Right));
    }
    session.frame(Pad::NEUTRAL);
    assert_eq!(selection(&session), last * TOWN_PAGE_ROWS);
    let rows = count - last * TOWN_PAGE_ROWS;
    for _ in 0..rows {
        press(&mut session, Button::Down);
    }
    assert_eq!(selection(&session), last * TOWN_PAGE_ROWS);
    session.frame(Pad::new(Button::Right));
    assert_eq!(
        selection(&session),
        last * TOWN_PAGE_ROWS,
        "no page past the last"
    );
    session.frame(Pad::NEUTRAL);
}

/// The pack's teleport lines: every window line below is compared with them.
fn messages(session: &Session) -> TravelMessages {
    session
        .runtime()
        .data()
        .travel()
        .expect("the pack has travel tables")
        .messages
        .clone()
}

fn message(session: &Session) -> String {
    session.camp_view().unwrap().message.clone()
}

/// Camp, TECH, Chaz, then the technique `id`.
fn cast(session: &mut Session, id: u8) {
    press(session, Button::Camp);
    press(session, Button::Down); // TECH
    assert_eq!(session.camp_view().unwrap().root_selection, 1);
    press(session, Button::Speak);
    assert_eq!(page(session), CampPage::AbilityCharacters);
    press(session, Button::Speak); // Chaz
    assert_eq!(page(session), CampPage::AbilityList);
    let at = session
        .camp_view()
        .unwrap()
        .ability_options
        .iter()
        .position(|ability| ability.id == id)
        .expect("Chaz knows the technique");
    for _ in 0..at {
        press(session, Button::Down);
    }
    press(session, Button::Speak);
}

#[test]
fn a_confirmed_town_shows_the_cartridges_line() {
    let Some(mut session) = session() else { return };
    let lines = messages(&session);
    // The ready line is two lines split at the cartridge's `$FC`.
    assert_eq!(lines.teleported.split('\n').count(), 2);
    open_town_list(&mut session);
    assert_eq!(
        session.camp_view().unwrap().travel_towns_label,
        lines.towns_label
    );
    press(&mut session, Button::Down);
    press(&mut session, Button::Speak);
    assert_eq!(page(&session), CampPage::TravelReady);
    assert_eq!(message(&session), lines.teleported);
}

#[test]
fn ryuka_lists_towns_under_the_packs_label_and_refuses_without_tp() {
    let Some(mut session) = session_with(&[], 50) else {
        return;
    };
    let lines = messages(&session);
    cast(&mut session, RYUKA);
    assert_eq!(page(&session), CampPage::TravelTowns);
    assert_eq!(
        session.camp_view().unwrap().travel_towns_label,
        lines.towns_label
    );
    press(&mut session, Button::Speak);
    assert_eq!(page(&session), CampPage::TravelReady);
    assert_eq!(message(&session), lines.teleported);

    let Some(mut session) = session_with(&[], 0) else {
        return;
    };
    cast(&mut session, RYUKA);
    assert_eq!(page(&session), CampPage::AbilityResult);
    assert_eq!(message(&session), lines.not_enough_tp);
}

#[test]
fn hinas_and_escapipe_outside_a_dungeon_show_the_cartridges_refusal() {
    let Some(mut session) = session_with(&[ESCAPIPE], 50) else {
        return;
    };
    let lines = messages(&session);
    assert_ne!(lines.cannot_be_used, lines.not_enough_tp);
    cast(&mut session, HINAS);
    assert_eq!(page(&session), CampPage::AbilityResult);
    assert_eq!(message(&session), lines.cannot_be_used);

    let Some(mut session) = session_with(&[ESCAPIPE], 50) else {
        return;
    };
    press(&mut session, Button::Camp);
    press(&mut session, Button::Speak); // ITEM
    for _ in 0..4 {
        if page(&session) == CampPage::ItemResult {
            break;
        }
        press(&mut session, Button::Speak);
    }
    assert_eq!(page(&session), CampPage::ItemResult);
    assert_eq!(message(&session), lines.cannot_be_used);
}
