//! Menu actions that start scenes, with pads only: the ITEM menu's vehicle
//! actions and the Aiedo inn's rest.
//!
//! The class is the cartridge's own: a menu command that destroys its window
//! and hands the field to an event (`ps4.asm:123419-123431`, `117150-117169`),
//! and the map load that parks a machine unless `Map_Load_Flags` spares it
//! (`:107507-107517`). Every frame below goes through `Session::frame`; the
//! hand-built saves only place the party and fill the pack, exactly as
//! `session_menus.rs` builds its shops and inns.

use std::path::Path;
use std::sync::OnceLock;

use psiv_core::{Cell, CharId, GameState, HOME_X, HOME_Y, PixelPos, RetailLocation, RetailSave};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::{Button, CampPage, Frame, Pad, Runtime, RuntimeEvent, Session, ShopPage};

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

fn pack_present() -> bool {
    Path::new(PACK).join("manifest.json").is_file()
}

/// The pack, read once for the whole test binary.
fn pack() -> &'static (GameData, BattleFiles) {
    static PACK_FILES: OnceLock<(GameData, BattleFiles)> = OnceLock::new();
    PACK_FILES.get_or_init(|| {
        let path = Path::new(PACK);
        (
            GameData::load(path).expect("pack loads"),
            BattleFiles::load(path).expect("battle files load"),
        )
    })
}

/// A session standing at `(map, x, y)` with `game`'s state, battle data loaded
/// (the camp needs the item tables).
fn session_at(place: (u16, u16, u16), game: &GameState) -> Session {
    let (data, files) = pack();
    let (map, x, y) = place;
    Session::start(data.clone())
        .with_battles(files.clone())
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
        .expect("the session starts")
}

/// A new game's state: the roster the title's initializer builds.
fn fresh_game() -> GameState {
    let (data, files) = pack();
    let initial = Session::start(data.clone())
        .with_battles(files.clone())
        .new_game()
        .expect("START");
    GameState::from_snapshot(&initial.runtime().game().snapshot())
}

/// Chaz alone with `money` meseta.
fn chaz_alone(money: u32) -> GameState {
    let mut game = fresh_game();
    game.set_party([Some(CharId(0)), None, None, None, None]);
    game.set_money(money);
    game
}

/// A session and the frames it has been given.
struct Player {
    session: Session,
}

impl Player {
    fn new(place: (u16, u16, u16), game: &GameState) -> Player {
        Player {
            session: session_at(place, game),
        }
    }

    /// One frame; the frame's events are only checked for faults, since the
    /// session has already opened every window itself.
    fn tick(&mut self, pad: Pad) -> Frame {
        let frame = self.session.frame(pad);
        for event in &frame.events {
            assert!(
                !matches!(
                    event,
                    RuntimeEvent::SceneFaulted { .. }
                        | RuntimeEvent::MapRefreshFailed { .. }
                        | RuntimeEvent::EncounterRolled { .. }
                ),
                "unexpected {event:?}"
            );
        }
        frame
    }

    /// A fresh press: the button down for a frame, released for the next.
    fn press(&mut self, button: Button) -> Frame {
        let frame = self.tick(Pad::new(button));
        self.tick(Pad::NEUTRAL);
        frame
    }

    fn runtime(&self) -> &Runtime {
        self.session.runtime()
    }

    /// Faces the counter and presses Speak; the frames a talk takes.
    fn talk_up(&mut self) {
        for _ in 0..4 {
            self.tick(Pad::new(Button::Up));
        }
        self.tick(Pad::NEUTRAL);
        self.press(Button::Speak);
    }

    /// Moves the cursor of whichever menu is up with Down presses until `read`
    /// says it is on `target`; menus wrap, so it always arrives.
    fn down_to(&mut self, read: impl Fn(&Session) -> usize, target: usize) {
        for _ in 0..64 {
            if read(&self.session) == target {
                return;
            }
            self.press(Button::Down);
        }
        panic!("the cursor never reached {target}");
    }
}

fn shop_page(player: &Player) -> ShopPage {
    player.session.shop_view().expect("the shop is up").page
}

fn camp_page(player: &Player) -> CampPage {
    player.session.camp_view().expect("the camp is up").page
}

// ---------------------------------------------------------------------------
// Vehicles from the ITEM menu, and the Aiedo inn's rest scene.
//
// The classes are the cartridge's own: a menu command that destroys its window
// and hands the field to an event (`ps4.asm:123419-123431`, `117150-117169`),
// and the map load that parks a machine unless `Map_Load_Flags` spares it
// (`:107507-107517`). Every frame below goes through `Session::frame`; the
// hand-built saves only place the party and fill the pack, as the other tests
// here do.
// ---------------------------------------------------------------------------

/// Motavia, the Machine Center's doorway, where `Event_GettingLandRover` leaves
/// the party (`routes/main.json`, `machine-center-control-key`): open ground
/// (`VehicleBoardingFlags[0]`), one vehicle step above the warp the risen
/// Machine Center opens.
const MACHINE_CENTER_DOOR: (u16, u16, u16) = (0, 114, 177);
/// Motavia's open water: the four cells under and beside the party are all raw
/// `9`, so `Vehicle_Boarding_Flags` is `VehicleBoardingFlags[9]` = `$04` — the
/// Hydrofoil's bit and no other.
const MOTAVIA_WATER: (u16, u16, u16) = (0, 20, 20);
/// The Aiedo supermarket's inn counter (map `$63`, row 6): the shopkeeper is at
/// `(50,30)`, the party talks across the counter from `(50,32)`.
const AIEDO_INN: (u16, u16, u16) = (0x63, 50, 32);
/// Gryz's house in Tonoe: map `$43`'s only event is `RunEvent_Dorin`
/// (`$14` → `Cutscene_Dorin`, `$8004`), a flag-gated trigger, so a mounted
/// party can fire it.
const GRYZ_HOUSE: (u16, u16, u16) = (0x43, 30, 32);

/// The item ids the cartridge's field table names (`ps4.constants.asm`).
const ITEM_LAND_ROVER: u8 = 0x96;
/// See [`ITEM_LAND_ROVER`].
const ITEM_ICE_DIGGER: u8 = 0x97;
/// See [`ITEM_LAND_ROVER`].
const ITEM_HYDROFOIL: u8 = 0x98;

/// Chaz alone with `money`, `items` and `vehicle`, past the principal's
/// confession: trigger `$5B` `RunEvent_ReenterPiata` fires on Motavia while
/// `EventFlag_PrincipalConfession` (`$0C`) is clear, and every state below is
/// one a party holding a machine has long since left behind.
fn with_pack(money: u32, items: &[u8], vehicle: u16) -> GameState {
    let mut game = chaz_alone(money);
    for item in items {
        game.inventory_mut().add(*item).unwrap();
    }
    game.set_vehicle_index(vehicle);
    game.set(psiv_core::Flag::event(0x0C)).unwrap();
    game
}

fn with_two_party_members_and_item(item: u8) -> GameState {
    let mut game = fresh_game();
    game.set_party([Some(CharId(0)), Some(CharId(1)), None, None, None]);
    game.inventory_mut().add(item).unwrap();
    game.set(psiv_core::Flag::event(0x0C)).unwrap();
    game.set(psiv_core::Flag::event(0x43)).unwrap();
    game
}

impl Player {
    /// Uses `item` from the ITEM page, returning the frame the menu answered
    /// on: the cursor walks to the item's row and Speak accepts it.
    fn use_item(&mut self, item: u8) -> Frame {
        self.press(Button::Camp);
        assert_eq!(camp_page(self), CampPage::Root);
        self.press(Button::Speak); // ITEM
        assert_eq!(camp_page(self), CampPage::ItemList);
        let row = self
            .session
            .camp_view()
            .unwrap()
            .snapshot
            .inventory
            .iter()
            .position(|held| held.id == item)
            .unwrap_or_else(|| panic!("the pack holds no {item:#04x}"));
        self.down_to(|s| s.camp_view().unwrap().item_selection, row);
        self.press(Button::Speak)
    }

    /// Runs frames until the scene ends the way `psiv-campaign`'s `settle`
    /// does: a finished page is turned with Speak, everything else is waited
    /// out, and the pad is a player's.
    fn run_scene_out(&mut self) {
        for _ in 0..4000 {
            if !self.runtime().scene_active() {
                return;
            }
            let dismissable = self
                .runtime()
                .dialogue_view()
                .is_some_and(|view| view.choice.is_none() && view.dismissable);
            if dismissable {
                self.press(Button::Speak);
            } else {
                self.tick(Pad::NEUTRAL);
            }
        }
        panic!("the scene never finished");
    }
}

/// An isolated ordinary-pad route from hand-built save placement. The three
/// item actions must share the 32-pixel snap, live party sync, and camera gate;
/// this is not a connected New Game campaign milestone.
#[test]
fn vehicle_items_snap_off_grid_party_and_wait_for_camera_before_selector() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    for (item, selector, place, expected) in [
        (ITEM_LAND_ROVER, 1, (0, 115, 177), Cell::new(114, 177)),
        (ITEM_LAND_ROVER, 1, (0, 109, 178), Cell::new(108, 179)),
        (ITEM_ICE_DIGGER, 2, (0, 114, 178), Cell::new(114, 179)),
        (ITEM_HYDROFOIL, 3, (0, 20, 20), Cell::new(20, 21)),
    ] {
        let game = with_two_party_members_and_item(item);
        let mut player = Player::new(place, &game);
        let answered = player.use_item(item);
        assert_eq!(
            answered
                .scene_started
                .unwrap_or_else(|| panic!("item {item:#x} refused at {place:?}"))
                .event,
            selector + 8
        );
        let first = player.tick(Pad::NEUTRAL);
        assert!(player.runtime().scene_active());
        assert_eq!(player.runtime().vehicle_index(), None);
        let body = player
            .runtime()
            .vehicle_draw_state()
            .expect("body replaced before pan");
        assert_eq!((body.index(), body.cell()), (selector, expected));
        assert!(
            !first
                .events
                .iter()
                .any(|event| matches!(event, RuntimeEvent::VehicleChanged { .. }))
        );
        for slot in 0..2 {
            assert_eq!(
                player.runtime().scene_party_actor(slot).unwrap().cell,
                expected
            );
            assert_eq!(player.runtime().members()[slot].cell, expected);
        }
        let subject = PixelPos::from_cell(expected);
        let target = (subject.x - HOME_X, subject.y - HOME_Y);
        let camera_before_pan = player.runtime().camera().raw();
        let mut blocked_frames = 0;
        let mut arrived = false;
        for frame_index in 0..2000 {
            let frame = player.tick(Pad::NEUTRAL);
            if place == (0, 109, 178) && frame_index == 0 {
                let camera_after_first_x = player.runtime().camera().raw();
                assert_ne!(camera_after_first_x.0, camera_before_pan.0);
                assert_eq!(
                    camera_after_first_x.1, camera_before_pan.1,
                    "retail Event_MoveCamera finishes X before starting Y"
                );
            }
            if frame.events.iter().any(|event| {
                matches!(
                    event, RuntimeEvent::VehicleChanged { index } if *index == selector
                )
            }) {
                arrived = true;
                break;
            }
            assert_eq!(player.runtime().vehicle_index(), None, "selector must wait");
            blocked_frames += 1;
        }
        assert!(arrived, "boarding never passed its camera gate");
        assert!(
            blocked_frames > 0,
            "off-grid pan needs real camera progress"
        );
        let (x, y) = player.runtime().camera().raw();
        assert_eq!((x >> 16, y >> 16), target);
        assert_eq!(player.runtime().vehicle_cell(), Some(expected));
        assert_eq!(
            player.runtime().vehicle_draw_state().unwrap().cell(),
            expected
        );
        assert_eq!(player.runtime().state().cell(), expected);
        assert_eq!(player.runtime().members()[1].cell, expected);
    }
}

#[test]
fn aligned_vehicle_item_sets_selector_without_a_camera_gate() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let game = with_two_party_members_and_item(ITEM_LAND_ROVER);
    let mut player = Player::new(MACHINE_CENTER_DOOR, &game);
    player.use_item(ITEM_LAND_ROVER);
    let frame = player.tick(Pad::NEUTRAL);
    assert!(
        frame
            .events
            .iter()
            .any(|event| matches!(event, RuntimeEvent::VehicleChanged { index: 1 }))
    );
    assert_eq!(player.runtime().vehicle_cell(), Some(Cell::new(114, 177)));
    assert_eq!(player.runtime().members()[1].cell, Cell::new(114, 177));
}

/// Boarding the Land Rover from the ITEM menu, and the map load that parks it:
/// `ItemAction_LandRover` (`ps4.asm:123419-123431`) writes Event `$09`, the
/// boarding event writes `Vehicle_Index` (`:144950-145008`), and the warp into
/// the Machine Center runs `GameMode_LoadFieldMap` with no `Map_Load_Flags` bit
/// set, which clears the selector (`:107507-107517`).
#[test]
fn the_land_rover_item_boards_and_the_next_map_load_parks_it() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    // The Machine Center has risen (flag `$43`), which is what opens its door
    // cells for a freshly built map (`RUNNER_LOG.md` H19).
    let mut game = with_pack(0, &[ITEM_LAND_ROVER], 0);
    game.set(psiv_core::Flag::event(0x43)).unwrap();
    let mut player = Player::new(MACHINE_CENTER_DOOR, &game);
    assert_eq!(player.runtime().vehicle_index(), None, "on foot");

    let answered = player.use_item(ITEM_LAND_ROVER);
    assert!(
        player.session.camp_view().is_none(),
        "the accepted action destroys the menu (`DestroyAllWindows`)"
    );
    let started = answered
        .scene_started
        .expect("the action started its event");
    assert_eq!((started.event, started.started), (0x09, true));
    assert!(player.runtime().scene_active(), "the boarding event runs");

    player.run_scene_out();
    assert_eq!(
        player.runtime().vehicle_index(),
        Some(1),
        "Event_BoardingLandRover wrote Vehicle_Index 1"
    );

    // One vehicle step down onto the Machine Center's doorway.
    for frame in 0..16 {
        let pad = if frame < 12 {
            Pad::new(Button::Down)
        } else {
            Pad::NEUTRAL
        };
        player.tick(pad);
    }
    assert_eq!(
        player.runtime().map_id().0,
        0xB7,
        "the doorway warps into the Machine Center"
    );
    assert_eq!(
        player.runtime().vehicle_index(),
        None,
        "the load parked the machine: the party walks in on foot"
    );
}

/// The cartridge refuses the Land Rover item inside a town: `ItemAction_LandRover`
/// tests `Field_Map_Index & $FFF0` and answers "Cant get on it here!"
/// (`ps4.asm:123419-123424`, `loc_2AA2C8`).
#[test]
fn the_land_rover_item_is_refused_inside_a_town_with_the_cartridges_line() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let game = with_pack(0, &[ITEM_LAND_ROVER], 0);
    let mut player = Player::new(AIEDO_INN, &game);
    player.use_item(ITEM_LAND_ROVER);
    assert_eq!(camp_page(&player), CampPage::ItemResult);
    assert_eq!(
        player.session.camp_view().unwrap().message,
        "Can't get on it here!"
    );
    assert_eq!(player.runtime().vehicle_index(), None);
    assert!(!player.runtime().scene_active());
}

/// The boarding flag is the tile's, not the machine's: on Motavia's water
/// `VehicleBoardingFlags[9]` allows the Hydrofoil alone, so the Land Rover item
/// is refused there while the Hydrofoil item starts Event `$0B`
/// (`ps4.asm:123419-123465`, `:117189-117199`).
#[test]
fn the_standing_tile_decides_which_machine_may_board() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let mut game = with_pack(0, &[ITEM_LAND_ROVER, ITEM_HYDROFOIL], 0);
    // Water is out of the Land Rover's reach (`can_cross(1, 9)`), so a player
    // reaches it in the Hydrofoil; here the tile alone is under test.
    game.set(psiv_core::Flag::event(0x43)).unwrap();
    let mut refused = Player::new(MOTAVIA_WATER, &game);
    refused.use_item(ITEM_LAND_ROVER);
    assert_eq!(camp_page(&refused), CampPage::ItemResult);
    assert_eq!(
        refused.session.camp_view().unwrap().message,
        "Can't get on it here!",
        "VehicleBoardingFlags[9] has no Land Rover bit"
    );
    assert_eq!(refused.runtime().vehicle_index(), None);

    let mut boarded = Player::new(MOTAVIA_WATER, &game);
    let answered = boarded.use_item(ITEM_HYDROFOIL);
    let started = answered
        .scene_started
        .expect("the action started its event");
    assert_eq!((started.event, started.started), (0x0B, true));
    boarded.run_scene_out();
    assert_eq!(
        boarded.runtime().vehicle_index(),
        Some(3),
        "Event_BoardingHydrofoil wrote Vehicle_Index 3"
    );
}

/// The negative control for a machine the item table does not name: using a
/// Dagger destroys the menu like any "consumed" item (`loc_5C0B0` returns the
/// item id, `ps4.asm:123337-123421`) and changes nothing.
#[test]
fn a_pack_item_outside_the_action_table_closes_the_menu_without_a_scene() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let game = with_pack(0, &[1], 0);
    let mut player = Player::new(MACHINE_CENTER_DOOR, &game);
    let answered = player.use_item(1);
    assert!(player.session.camp_view().is_none());
    assert_eq!(answered.scene_started, None, "no event was written");
    assert!(!player.runtime().scene_active());
    assert_eq!(player.runtime().vehicle_index(), None);
    assert_eq!(
        player.runtime().game().inventory().get(0),
        Some(1),
        "the item is not consumed"
    );
}

/// `Cutscene_Dorin`'s own map reload keeps a mounted party: the scene sets
/// `Map_Load_Flags` bit 3 before its `LoadMap` (`ps4.asm:121767-121780`), so
/// `Vehicle_Index` survives the refresh.
#[test]
fn a_scene_that_sets_the_keep_bit_keeps_the_vehicle() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    // `RunEvent_Dorin` fires with `Dorin` set and `GryzJoined` clear.
    let mut game = with_pack(0, &[], 1);
    game.set(psiv_core::Flag::event(0x36)).unwrap();
    let mut player = Player::new(GRYZ_HOUSE, &game);
    assert_eq!(
        player.runtime().vehicle_index(),
        Some(1),
        "the save placed the party in the Land Rover"
    );

    let mut started = false;
    let mut loaded = false;
    for _ in 0..4000 {
        // The input is decided from the window's own state, so every frame's
        // events are inspected: `Player::press` spends two frames.
        let dismissable = player
            .runtime()
            .dialogue_view()
            .is_some_and(|view| view.choice.is_none() && view.dismissable);
        let frame = if dismissable {
            player.press(Button::Speak)
        } else {
            player.tick(Pad::NEUTRAL)
        };
        started |= frame
            .events
            .iter()
            .any(|event| matches!(event, RuntimeEvent::SceneStarted { .. }));
        if frame
            .events
            .iter()
            .any(|event| matches!(event, RuntimeEvent::MapChanged { .. }))
        {
            loaded = true;
            break;
        }
        if started && !player.runtime().scene_active() {
            break;
        }
    }
    assert!(started, "RunEvent_Dorin fired the scene");
    assert!(loaded, "the scene reached its load");
    assert_eq!(player.runtime().map_id().0, 0x43);
    assert_eq!(
        player.runtime().vehicle_index(),
        Some(1),
        "the scene's keep bit spared the machine through RefreshMap"
    );
}

/// The Aiedo inn's rest, when `EventFlag_Zio` and `EventFlag_GirlsCaught` are
/// clear: the bill is priced and `RecoverStats` runs, the scene plays instead
/// of a night, the bill is charged after it, and the window comes back on the
/// result line (`ps4.asm:136347-136466`).
#[test]
fn the_aiedo_inn_rest_runs_the_scene_between_recovery_and_the_bill() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    // Aiedo's rate is 50 a head (`shops.json` inn 6).
    let mut game = with_pack(500, &[], 0);
    game.set_party([Some(CharId(0)), Some(CharId(2)), None, None, None]);
    for id in [0, 2] {
        let stats = game.roster_mut().get_mut(CharId(id)).unwrap();
        stats.max_hp = 120;
        stats.max_tp = 40;
        stats.curr_hp = 5;
        stats.curr_tp = 2;
        stats.status = 2;
    }
    let mut player = Player::new(AIEDO_INN, &game);
    player.talk_up();
    player.tick(Pad::NEUTRAL);
    assert_eq!(shop_page(&player), ShopPage::InnGreeting);
    player.press(Button::Speak);
    assert_eq!(shop_page(&player), ShopPage::InnConfirm);
    assert_eq!(player.session.shop_view().unwrap().inn_cost, 100);

    let answered = player.press(Button::Speak); // YES
    // The window is destroyed for the scene; the field owns the frames.
    assert!(player.session.shop_view().is_none());
    assert!(player.runtime().scene_active(), "the rest event runs");

    // Recovery happened before the scene and the bill has not been charged.
    assert_eq!(player.runtime().game().money(), 500);
    for id in [0, 2] {
        let stats = player.runtime().game().roster().get(CharId(id)).unwrap();
        assert_eq!((stats.curr_hp, stats.curr_tp), (120, 40), "member {id}");
        assert_eq!(stats.status, 0, "member {id}");
    }

    player.run_scene_out();
    assert_eq!(
        player.runtime().game().money(),
        400,
        "the bill is charged after the scene returns"
    );
    assert!(
        player.runtime().game().is_set(psiv_core::Flag::event(0x46)),
        "Event_GirlsSneakingOut set EventFlag_GirlsCaught"
    );
    assert_eq!(
        player.session.shop_view().map(|view| view.page),
        Some(ShopPage::Message),
        "the window is rebuilt on the night's result line"
    );
    assert_eq!(
        player.session.shop_view().unwrap().message,
        "Thank you very much.\nPlease come again."
    );
    let _ = answered;
}

/// The negative control: with either flag set, the Aiedo counter is an
/// ordinary night — charged, restored, no scene (`ps4.asm:136389-136404`).
#[test]
fn the_aiedo_inn_is_an_ordinary_night_once_either_flag_is_set() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    for flag in [0x42u16, 0x46] {
        let mut game = with_pack(500, &[], 0);
        game.set_party([Some(CharId(0)), Some(CharId(2)), None, None, None]);
        game.set(psiv_core::Flag::event(flag)).unwrap();
        let mut player = Player::new(AIEDO_INN, &game);
        player.talk_up();
        player.tick(Pad::NEUTRAL);
        player.press(Button::Speak); // the greeting
        assert_eq!(shop_page(&player), ShopPage::InnConfirm);
        player.press(Button::Speak); // YES
        assert_eq!(shop_page(&player), ShopPage::Message, "flag {flag:#04x}");
        assert!(
            !player.runtime().scene_active(),
            "no scene with flag {flag:#04x}"
        );
        assert_eq!(player.runtime().game().money(), 400, "flag {flag:#04x}");
        assert_eq!(
            player.session.shop_view().unwrap().message,
            "Thank you very much.\nPlease come again."
        );
    }
}
