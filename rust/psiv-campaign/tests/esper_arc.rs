//! The Dezolis late arc's seams, driven with pads from hand-built saves (lane
//! C7): the Ice Digger boarded from the ITEM menu, a trigger cell the
//! carnivorous trees fire on, the Esper Mansion's door guards, the Air Castle's
//! scene-opened destination menu and a doorway whose warp sits one row on.
//! They need the repository pack and skip with a message without one.

mod common;

use common::pack;
use psiv_campaign::driver::Driver;
use psiv_campaign::halt::HaltKind;
use psiv_campaign::route::NameOrId;
use psiv_core::{Cell, CharId, Flag, GameState, RetailLocation, RetailSave};
use psiv_runtime::Session;

const DEZOLIS: u16 = 0x001;
const SPACEPORT: u16 = 0x0D4;
const ENTRANCE: u16 = 0x166;
const HALL: u16 = 0x167;
const AIR_CASTLE: u16 = 0x171;
const AIR_CASTLE_F1: u16 = 0x181;
const AIR_CASTLE_PART6: u16 = 0x170;

const ITEM_ICE_DIGGER: u8 = 0x97;

/// A Chaz/Rika/Rune/Kyra party on `map` at `(x, y)` with `extra` event flags
/// and, when asked, the Ice Digger in the pack.
fn party_at(map: u16, x: u16, y: u16, extra: &[u16], ice_digger: bool) -> Option<Session> {
    let set = pack()?;
    let initial = Session::start(set.data.clone())
        .with_battles(set.battle.clone())
        .field()
        .expect("the pack boots");
    let mut game = GameState::from_snapshot(&initial.runtime().game().snapshot());
    game.set_party([
        Some(CharId(0)),
        Some(CharId(5)),
        Some(CharId(3)),
        Some(CharId(9)),
        None,
    ]);
    // The story the party arrives with: Zio and Wren, the Chaos Sorcerer, the
    // crash landing, the snowstorm talk and Gyuna (so none of their triggers
    // fires on the first frame), plus the test's own flags.
    for flag in [
        0x08, 0x66, 0x68, 0x70, 0x71, 0x72, 0x80, 0x81, 0x84, 0x85, 0x88,
    ]
    .iter()
    .chain(extra)
    {
        game.set(Flag::event(*flag)).expect("flag in range");
    }
    if ice_digger {
        game.inventory_mut()
            .add(ITEM_ICE_DIGGER)
            .expect("room for the Ice Digger");
    }
    Some(
        Session::start(set.data.clone())
            .with_battles(set.battle.clone())
            .from_save(RetailSave {
                snapshot: game.snapshot(),
                location: RetailLocation {
                    world_index: if map == AIR_CASTLE || map == AIR_CASTLE_F1 {
                        5
                    } else {
                        1
                    },
                    map_index_2: 0,
                    map_index: map,
                    char_x: x * 16,
                    char_y: y * 16,
                },
            })
            .expect("the map loads"),
    )
}

fn flag(driver: &Driver, id: u16) -> bool {
    driver.runtime().game().is_set(Flag::event(id))
}

fn name(text: &str) -> NameOrId {
    NameOrId::Name(text.to_owned())
}

/// ITEM, ICE-DIGGER on plain ground runs `Event_BoardingIceDigger` (`$0A`): the
/// menu closes on that very press and the field runs the scene. On the
/// map-change tile of the spaceport door (raw collision 1) the action is
/// refused and nothing boards.
#[test]
fn the_ice_digger_boards_from_the_item_menu_on_plain_ground_only() {
    let Some(session) = party_at(DEZOLIS, 10, 73, &[0x82, 0x89], true) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver
        .use_item(&name("ICE-DIGGER"), &name("Chaz"))
        .expect("the boarding scene runs out");
    assert_eq!(
        driver.runtime().game().vehicle_index(),
        2,
        "Vehicle_Index 2"
    );

    // Negative control: the door tile (12,73).
    let Some(session) = party_at(DEZOLIS, 12, 73, &[0x82, 0x89], true) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver
        .use_item(&name("ICE-DIGGER"), &name("Chaz"))
        .expect("a refusal is a result line, not a halt");
    assert_eq!(
        driver.runtime().game().vehicle_index(),
        0,
        "no machine boards on a door"
    );
}

/// `RunEvent_CarnivorousTrees` fires on the corridor cell (186,13): with Raja
/// sick (`$94`) and the trees clear it is `Event_SavingKyra` (`$95` set before
/// event battle 10), without it `Event_CarnivorousTrees` (the same battle, no
/// flag). `step_onto` takes the one step and is done when the scene has run; a
/// cell where no trigger fires is a halt naming it.
#[test]
fn step_onto_a_plain_trigger_cell_fires_the_trees_arm_the_flags_pick() {
    let Some(session) = party_at(DEZOLIS, 186, 15, &[0x94], false) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    let halt = driver
        .step_onto(DEZOLIS, Cell::new(186, 13))
        .expect_err("the event battle starts with no policy to fight it");
    assert_eq!(halt.kind, HaltKind::UnexpectedState);
    assert!(halt.detail.contains("no policy"), "{halt:?}");
    assert!(flag(&driver, 0x95), "Event_SavingKyra set $95");
    let battle = driver.battles().last().expect("a battle began");
    assert_eq!((battle.kind, battle.id), ("event", 10));

    // Negative control: Raja well, the other arm: the battle without the flag.
    let Some(session) = party_at(DEZOLIS, 186, 15, &[], false) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    let halt = driver
        .step_onto(DEZOLIS, Cell::new(186, 13))
        .expect_err("the trees fight");
    assert!(halt.detail.contains("no policy"), "{halt:?}");
    assert!(!flag(&driver, 0x95), "Event_CarnivorousTrees sets nothing");
    assert_eq!(driver.battles().last().map(|b| b.id), Some(10));

    // A cell where nothing fires is a halt, not a loop.
    let Some(session) = party_at(DEZOLIS, 186, 15, &[0x94], false) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    let halt = driver
        .step_onto(DEZOLIS, Cell::new(186, 16))
        .expect_err("(186,16) fires nothing");
    assert_eq!(halt.kind, HaltKind::UnexpectedState);
    assert!(halt.detail.contains("no scene ran"), "{halt:?}");
}

/// `Event_EsperGuardPermission`: with the trees fought the door guards speak
/// entry 1, take dialogue id 1 and the temp flag opens the door, and once they
/// have walked off the party reaches the hall. Without the trees (and no Dark
/// Force 2) they turn the party away, the flag stays clear and the door stays
/// shut.
#[test]
fn the_door_guards_step_aside_for_the_party_that_fought_the_trees() {
    let Some(session) = party_at(ENTRANCE, 32, 37, &[0x94, 0x95], false) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver.talk(0).expect("the guard speaks");
    assert_eq!(driver.runtime().npc_dialogue_id(0), Some(1));
    assert_eq!(driver.runtime().npc_dialogue_id(1), Some(1));
    assert!(driver.runtime().game().is_set(Flag::temp(0x1A)));
    driver.neutral(90).expect("the guards walk");
    driver
        .go_to_map(HALL, None)
        .expect("the door row is free: the party reaches the hall");

    // Negative control: no trees fought.
    let Some(session) = party_at(ENTRANCE, 32, 37, &[0x94], false) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver.talk(0).expect("the guard speaks");
    assert!(
        !driver.runtime().game().is_set(Flag::temp(0x1A)),
        "no permission"
    );
    assert_ne!(driver.runtime().npc_dialogue_id(0), Some(1));
    driver.neutral(90).expect("frames run");
    let halt = driver
        .go_to_map(HALL, None)
        .expect_err("the guards still stand on the door row");
    assert_eq!(halt.kind, HaltKind::Unreachable);
}

/// `RunEvent_FindingAirCastle` fires on the Dezo spaceport's first frame with
/// the torch stolen (`$98`) and the Air Castle not found (`$99`), and its scene
/// ends in the ship's menu: `go_to_map` stops there, `board` with no step
/// answers it and the flight lands on the Air Castle. Without `$98` the same
/// arrival opens nothing.
#[test]
fn a_scene_opens_the_air_castle_menu_and_board_answers_it() {
    let Some(session) = party_at(DEZOLIS, 13, 75, &[0x82, 0x89, 0x98], false) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver
        .go_to_map(SPACEPORT, None)
        .expect("the arrival stops at the menu the scene opens");
    assert!(
        driver.session().destination_view().is_some(),
        "the menu is open"
    );
    driver
        .board_from_scene(5)
        .expect("the Air Castle is listed");
    assert_eq!(driver.map(), AIR_CASTLE);
    assert!(flag(&driver, 0x99), "the scene set $99");

    // Negative control: the torch is still on its altar.
    let Some(session) = party_at(DEZOLIS, 13, 75, &[0x82, 0x89], false) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver
        .go_to_map(SPACEPORT, None)
        .expect("an ordinary arrival");
    assert!(driver.session().destination_view().is_none());
    let halt = driver
        .board_from_scene(5)
        .expect_err("no scene opens a menu");
    assert_eq!(halt.kind, HaltKind::UnexpectedState);
}

/// `AirCastle_F1`'s warp 0 is in the normal-ground table on row 44; the doorway
/// row above it (43) is collision type 1 with no map-change entry. The walk
/// onto row 43 reports an unmapped cell and goes on to row 44, which fires the
/// warp: that report is no fault beside a warp footprint.
#[test]
fn a_doorway_row_above_its_warp_is_walked_through() {
    let Some(session) = party_at(AIR_CASTLE_F1, 34, 41, &[0x98, 0x99, 0x9f], false) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver
        .go_to_map(AIR_CASTLE_PART6, Some(0))
        .expect("the unmapped doorway row is not a halt");
    assert_eq!(driver.map(), AIR_CASTLE_PART6);
}
