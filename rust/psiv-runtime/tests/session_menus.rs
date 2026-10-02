//! Shops, inns, the camp menu and the chest window, through `Session::frame`
//! with pads only.
//!
//! Nothing here calls a runtime mutator: the party stands where a player would,
//! presses Speak at a shopkeeper, and walks the menus with the d-pad. The state
//! that results is read back from the runtime. The negative controls sit beside
//! the cases they guard: a purchase the purse cannot cover changes nothing, and
//! the Camp button does not open the menu while a scene runs.

use std::path::Path;
use std::sync::OnceLock;

use psiv_core::{CharId, GameState, RetailLocation, RetailSave, StepFrames};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::{
    Button, CampPage, FrameMode, Pad, Routed, Runtime, RuntimeEvent, SaveStore, Session, ShopPage,
};

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

/// Piata's item shop (`PiataItemShop`, map `$1B`): the party stands two cells
/// below the shopkeeper at (33,30) with the counter between them.
const ITEM_SHOP: (u16, u16, u16) = (0x1B, 33, 32);
/// Piata's inn (`PiataInn`, map `$19`): shopkeeper at (41,30).
const INN: (u16, u16, u16) = (0x19, 41, 32);
/// A quiet field map with no scene and no counter (the Academy corridor the
/// camp fixtures use).
const FIELD: (u16, u16, u16) = (0x13, 48, 19);

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

/// A session standing at `(map, x, y)` with `game`'s party, battle data loaded
/// (the camp needs the item and ability tables).
fn session_at(place: (u16, u16, u16), game: &GameState) -> Session {
    let (data, files) = pack();
    let data = data.clone();
    let (map, x, y) = place;
    let mut runtime = Runtime::from_save(
        data,
        RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index_2: 0,
                map_index: map,
                char_x: x * 16,
                char_y: y * 16,
            },
        },
        StepFrames::default(),
    )
    .expect("runtime builds");
    runtime.enable_battles(files).expect("battles enable");
    Session::new(runtime)
}

/// A new game's state: the roster the title's initializer builds.
fn fresh_game() -> GameState {
    let (data, files) = pack();
    let mut initial = Runtime::new_game(data.clone(), StepFrames::default()).expect("START");
    initial.enable_battles(files).expect("battles enable");
    GameState::from_snapshot(&initial.game().snapshot())
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
    fn tick(&mut self, pad: Pad) -> psiv_runtime::Frame {
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
    fn press(&mut self, button: Button) -> psiv_runtime::Frame {
        let frame = self.tick(Pad::new(button));
        self.tick(Pad::NEUTRAL);
        frame
    }

    fn runtime(&self) -> &Runtime {
        self.session.runtime()
    }

    /// Faces the shopkeeper across the counter and presses Speak; returns the
    /// frame the window opened on.
    fn talk_up(&mut self) -> psiv_runtime::Frame {
        // The counter is solid: walking into it turns the party to face it.
        for _ in 0..4 {
            self.tick(Pad::new(Button::Up));
        }
        self.tick(Pad::NEUTRAL);
        self.press(Button::Speak)
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

/// From the greeting to the buy confirmation for the list's first item.
fn open_buy_confirm(player: &mut Player) {
    assert_eq!(shop_page(player), ShopPage::Greeting);
    player.press(Button::Speak);
    assert_eq!(shop_page(player), ShopPage::Root);
    player.press(Button::Speak); // BUY
    assert_eq!(shop_page(player), ShopPage::BuyList);
    player.press(Button::Speak); // the first item
    assert_eq!(shop_page(player), ShopPage::BuyConfirm);
}

#[test]
fn buying_at_a_piata_counter_spends_the_price_and_fills_a_slot() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let mut player = Player::new(ITEM_SHOP, &chaz_alone(100));
    let opened = player.talk_up();
    assert_eq!(opened.mode, FrameMode::Field);
    assert!(
        opened
            .routed
            .iter()
            .any(|routed| matches!(routed, Routed::ShopOpened { counter: 12 })),
        "the Piata item shop is counter row 12: {:?}",
        opened.routed
    );
    // The press that opened the window is still down for its first frame; the
    // window waits for it to be released before it listens.
    player.tick(Pad::NEUTRAL);
    open_buy_confirm(&mut player);

    let stock = player.session.shop_view().unwrap().stock.clone();
    assert_eq!(stock.len(), 1, "Piata sells one thing");
    assert_eq!(stock[0].item_id, 125, "Monomate");
    assert_eq!(stock[0].price, 20);

    player.press(Button::Speak); // YES
    assert_eq!(shop_page(&player), ShopPage::Message);
    assert_eq!(player.runtime().game().money(), 80, "100 - 20");
    assert_eq!(
        player.runtime().game().inventory().get(0),
        Some(125),
        "the first free slot holds the Monomate"
    );
    assert!(
        player
            .session
            .shop_view()
            .unwrap()
            .message
            .starts_with("Thank you")
    );

    // Back out: the message returns to the menu, the menu to the field.
    player.press(Button::Speak);
    assert_eq!(shop_page(&player), ShopPage::Root);
    player.press(Button::Cancel);
    assert!(player.session.shop_view().is_none(), "the window closed");
}

/// The negative control for the purchase: a purse that cannot cover the price
/// is refused and nothing changes.
#[test]
fn a_purchase_the_purse_cannot_cover_is_refused_with_no_state_change() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let mut player = Player::new(ITEM_SHOP, &chaz_alone(19));
    player.talk_up();
    player.tick(Pad::NEUTRAL);
    open_buy_confirm(&mut player);
    let before = player.runtime().game().snapshot();

    player.press(Button::Speak); // YES
    assert_eq!(shop_page(&player), ShopPage::Message);
    assert_eq!(
        player.session.shop_view().unwrap().message,
        "You don't have enough money!"
    );
    assert_eq!(player.runtime().game().money(), 19);
    assert_eq!(player.runtime().game().inventory().get(0), None);
    assert_eq!(
        player.runtime().game().snapshot(),
        before,
        "a refused purchase changes nothing"
    );
}

#[test]
fn selling_pays_half_the_record_price_for_any_item() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    // A Monomate (price word 20) and a Dagger (price word 40, which no Piata
    // shop stocks): the cartridge reads the item record for either
    // (`ps4.asm:135570`), halves it, and pays it.
    let mut game = chaz_alone(0);
    game.inventory_mut().add(125).unwrap();
    game.inventory_mut().add(1).unwrap();
    let mut player = Player::new(ITEM_SHOP, &game);
    player.talk_up();
    player.tick(Pad::NEUTRAL);
    player.press(Button::Speak); // greeting
    player.press(Button::Down); // SELL
    player.press(Button::Speak);
    assert_eq!(shop_page(&player), ShopPage::SellList);
    let items = player.session.shop_view().unwrap().items.clone();
    assert_eq!(items.len(), 2);
    assert_eq!((items[0].id, items[0].sell_price), (125, 10));
    assert_eq!((items[1].id, items[1].sell_price), (1, 20));

    player.press(Button::Down); // the Dagger
    player.press(Button::Speak);
    assert_eq!(shop_page(&player), ShopPage::SellConfirm);
    player.press(Button::Speak); // YES
    assert_eq!(shop_page(&player), ShopPage::Message);
    assert_eq!(player.runtime().game().money(), 20, "half of 40");
    assert_eq!(player.runtime().game().inventory().get(0), Some(125));
    assert_eq!(
        player.runtime().game().inventory().get(1),
        None,
        "the Dagger is gone"
    );
}

/// Leaving the sell list by Cancel zeroes the shared list cursor
/// (`ps4.asm:135508`), so the buy list does not open on a stale row.
#[test]
fn the_buy_list_opens_on_its_first_row_after_a_sell_visit() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let mut game = chaz_alone(100);
    game.inventory_mut().add(125).unwrap();
    game.inventory_mut().add(1).unwrap();
    let mut player = Player::new(ITEM_SHOP, &game);
    player.talk_up();
    player.tick(Pad::NEUTRAL);
    player.press(Button::Speak); // greeting
    player.press(Button::Down); // SELL
    player.press(Button::Speak);
    player.press(Button::Down); // the second item
    assert_eq!(player.session.shop_view().unwrap().item_selection, 1);
    player.press(Button::Cancel);
    assert_eq!(shop_page(&player), ShopPage::Root);
    player.press(Button::Up); // BUY
    player.press(Button::Speak);
    assert_eq!(shop_page(&player), ShopPage::BuyList);
    assert_eq!(player.session.shop_view().unwrap().item_selection, 0);
    player.press(Button::Speak);
    assert_eq!(shop_page(&player), ShopPage::BuyConfirm);
}

#[test]
fn an_empty_pack_has_nothing_to_sell() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let mut player = Player::new(ITEM_SHOP, &chaz_alone(5));
    player.talk_up();
    player.tick(Pad::NEUTRAL);
    player.press(Button::Speak);
    player.press(Button::Down); // SELL
    player.press(Button::Speak);
    assert_eq!(shop_page(&player), ShopPage::Message);
    assert_eq!(
        player.session.shop_view().unwrap().message,
        "You don't have anything to sell."
    );
}

#[test]
fn a_night_at_the_inn_restores_the_party_and_charges_per_member() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let mut game = fresh_game();
    game.set_party([Some(CharId(0)), Some(CharId(2)), None, None, None]);
    game.set_money(100);
    for id in [0, 2] {
        let stats = game.roster_mut().get_mut(CharId(id)).unwrap();
        stats.max_hp = 200;
        stats.max_tp = 50;
        stats.curr_hp = 7;
        stats.curr_tp = 1;
        stats.status = 2;
    }
    let mut player = Player::new(INN, &game);
    let opened = player.talk_up();
    assert!(
        opened
            .routed
            .iter()
            .any(|routed| matches!(routed, Routed::ShopOpened { counter: 0 })),
        "the Piata inn is counter row 0: {:?}",
        opened.routed
    );
    player.tick(Pad::NEUTRAL);
    assert_eq!(shop_page(&player), ShopPage::InnGreeting);
    player.press(Button::Speak);
    assert_eq!(shop_page(&player), ShopPage::InnConfirm);
    assert_eq!(
        player.session.shop_view().unwrap().inn_cost,
        10,
        "5 meseta a head, two heads"
    );
    player.press(Button::Speak); // YES
    assert_eq!(shop_page(&player), ShopPage::Message);
    assert_eq!(player.runtime().game().money(), 90);
    for id in [0, 2] {
        let stats = player.runtime().game().roster().get(CharId(id)).unwrap();
        assert_eq!((stats.curr_hp, stats.curr_tp), (200, 50), "member {id}");
        assert_eq!(stats.status, 0, "member {id}");
    }
}

/// An inn the party cannot afford sends them away as they came.
#[test]
fn an_inn_bill_the_purse_cannot_cover_changes_nothing() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let mut game = chaz_alone(4);
    game.roster_mut().get_mut(CharId(0)).unwrap().curr_hp = 3;
    let mut player = Player::new(INN, &game);
    player.talk_up();
    player.tick(Pad::NEUTRAL);
    player.press(Button::Speak);
    let before = player.runtime().game().snapshot();
    player.press(Button::Speak); // YES
    assert_eq!(
        player.session.shop_view().unwrap().message,
        "You don't have enough money!"
    );
    assert_eq!(player.runtime().game().snapshot(), before);
}

/// Three-member field fixture: Chaz hurt with the heal technique, Hahn and
/// Demi beside him.
fn camp_game() -> GameState {
    let mut game = fresh_game();
    game.set_party([
        Some(CharId(0)),
        Some(CharId(2)),
        Some(CharId(6)),
        None,
        None,
    ]);
    game.set_money(300);
    for (id, hp) in [(0, 10), (2, 40), (6, 90)] {
        let stats = game.roster_mut().get_mut(CharId(id)).unwrap();
        stats.curr_hp = hp;
        stats.max_hp = 100;
        stats.status = 0;
        stats.curr_tp = 60;
        stats.max_tp = 60;
        stats.mental.modified = 31;
    }
    let chaz = game.roster_mut().get_mut(CharId(0)).unwrap();
    chaz.techniques = [1, 34, 24, 27, 36, 37, 35, 25, 26, 28, 29, 39, 40, 0, 0, 0];
    game.inventory_mut().add(1).unwrap(); // a Dagger
    game
}

#[test]
fn the_camp_menu_casts_a_healing_technique_on_a_party_member() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let mut player = Player::new(FIELD, &camp_game());
    player.press(Button::Camp);
    assert_eq!(camp_page(&player), CampPage::Root);
    assert!(
        player.runtime().field_suspended(),
        "the field waits under the menu"
    );
    player.down_to(|s| s.camp_view().unwrap().root_selection, 1); // TECH
    player.press(Button::Speak);
    assert_eq!(camp_page(&player), CampPage::AbilityCharacters);
    player.press(Button::Speak); // Chaz
    assert_eq!(camp_page(&player), CampPage::AbilityList);
    let heal = player
        .session
        .camp_view()
        .unwrap()
        .ability_options
        .iter()
        .position(|ability| ability.id == 24)
        .expect("Chaz knows technique 24");
    player.down_to(|s| s.camp_view().unwrap().ability_selection, heal);
    player.press(Button::Speak);
    assert_eq!(camp_page(&player), CampPage::AbilityTarget);
    player.down_to(|s| s.camp_view().unwrap().target_selection, 1); // Hahn
    let tp_before = player
        .runtime()
        .game()
        .roster()
        .get(CharId(0))
        .unwrap()
        .curr_tp;
    player.press(Button::Speak);
    assert_eq!(camp_page(&player), CampPage::AbilityResult);

    let hahn = player.runtime().game().roster().get(CharId(2)).unwrap();
    assert!(hahn.curr_hp > 40, "Hahn was healed: {}", hahn.curr_hp);
    let message = player.session.camp_view().unwrap().message.clone();
    assert!(
        message.ends_with(&format!("Hahn {} HP", hahn.curr_hp - 40)),
        "the result line names the target and the amount: {message}"
    );
    assert!(
        player
            .runtime()
            .game()
            .roster()
            .get(CharId(0))
            .unwrap()
            .curr_tp
            < tp_before,
        "the cast cost TP"
    );
    // Cancel all the way out: result, list, casters, root, field.
    for _ in 0..4 {
        player.press(Button::Cancel);
    }
    assert!(player.session.camp_view().is_none());
    assert!(!player.runtime().field_suspended());
}

#[test]
fn the_camp_menu_equips_and_unequips_by_item_data() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let mut player = Player::new(FIELD, &camp_game());
    let weapon = player
        .runtime()
        .game()
        .roster()
        .get(CharId(0))
        .unwrap()
        .equipment[0];
    assert_ne!(weapon, 0, "Chaz starts with a right-hand weapon");
    let packed = player
        .runtime()
        .game()
        .inventory()
        .slots()
        .iter()
        .filter(|id| **id != 0)
        .count();

    player.press(Button::Camp);
    player.down_to(|s| s.camp_view().unwrap().root_selection, 3); // EQUIP
    player.press(Button::Speak);
    assert_eq!(camp_page(&player), CampPage::EquipCharacters);
    player.press(Button::Speak); // Chaz
    assert_eq!(camp_page(&player), CampPage::EquipStats);
    // The cursor order is head, right, left, body: RIGHT is row 1.
    player.down_to(|s| s.camp_view().unwrap().equipment_slot_selection, 1);

    // A slot holding an item takes it off: the equipment byte decides.
    player.press(Button::Speak);
    assert_eq!(camp_page(&player), CampPage::EquipResult);
    assert!(
        player
            .session
            .camp_view()
            .unwrap()
            .message
            .starts_with("REMOVED"),
        "{}",
        player.session.camp_view().unwrap().message
    );
    assert_eq!(
        player
            .runtime()
            .game()
            .roster()
            .get(CharId(0))
            .unwrap()
            .equipment[0],
        0
    );
    let unpacked = player
        .runtime()
        .game()
        .inventory()
        .slots()
        .iter()
        .filter(|id| **id != 0)
        .count();
    assert_eq!(unpacked, packed + 1, "the weapon went back into the pack");

    // The same slot, now empty, opens the item list; equipping from it puts
    // the weapon back.
    player.press(Button::Speak); // result -> stats
    assert_eq!(camp_page(&player), CampPage::EquipStats);
    player.press(Button::Speak);
    assert_eq!(
        camp_page(&player),
        CampPage::EquipItems,
        "an empty slot opens the item list"
    );
    let choice = player
        .session
        .camp_view()
        .unwrap()
        .equipment_options
        .iter()
        .position(|item| item.id == weapon)
        .expect("the weapon is a candidate again");
    player.down_to(|s| s.camp_view().unwrap().equipment_item_selection, choice);
    player.press(Button::Speak);
    // A weapon offers the hand choice when its type says so; take whichever
    // page the item data asked for.
    if camp_page(&player) == CampPage::EquipHands {
        player.press(Button::Speak); // RIGHT HAND
    }
    assert_eq!(camp_page(&player), CampPage::EquipResult);
    assert!(
        player
            .session
            .camp_view()
            .unwrap()
            .message
            .starts_with("EQUIPPED")
    );
    assert_eq!(
        player
            .runtime()
            .game()
            .roster()
            .get(CharId(0))
            .unwrap()
            .equipment[0],
        weapon,
        "the weapon is back in the right hand"
    );
    let repacked = player
        .runtime()
        .game()
        .inventory()
        .slots()
        .iter()
        .filter(|id| **id != 0)
        .count();
    assert_eq!(repacked, packed);
}

#[test]
fn the_camp_menu_reorders_the_party() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let mut player = Player::new(FIELD, &camp_game());
    assert_eq!(
        player.runtime().game().party_members(),
        vec![CharId(0), CharId(2), CharId(6)]
    );
    player.press(Button::Camp);
    player.down_to(|s| s.camp_view().unwrap().root_selection, 4); // STATE
    player.press(Button::Speak);
    assert_eq!(camp_page(&player), CampPage::State);
    player.down_to(|s| s.camp_view().unwrap().state_selection, 1); // ORDER
    player.press(Button::Speak);
    assert_eq!(camp_page(&player), CampPage::Order);
    // Demi first; the cursor walks the members still to be placed.
    player.down_to(|s| s.camp_view().unwrap().order_draft.cursor, 2);
    player.press(Button::Speak);
    // Chaz next: the last member, Hahn, is placed automatically.
    player.press(Button::Speak);
    assert_eq!(camp_page(&player), CampPage::OrderDone);
    assert_eq!(
        player.runtime().game().party_members(),
        vec![CharId(6), CharId(0), CharId(2)]
    );
    // The committed page closes on its own after the cartridge's wait.
    for _ in 0..70 {
        player.tick(Pad::NEUTRAL);
    }
    assert_eq!(camp_page(&player), CampPage::Root);
}

/// SAVE completes inside the session: the store writes the picked slot and the
/// page's result line is that write's answer. Negative control beside it: a
/// session with no run directory refuses and the line says so, instead of
/// falling back to a default path.
#[test]
fn the_camp_menu_saves_through_the_session_store() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let directory = std::env::temp_dir().join(format!("psiv-camp-save-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    let mut player = Player::new(FIELD, &camp_game());
    player.press(Button::Camp);
    player.down_to(|s| s.camp_view().unwrap().root_selection, 4); // STATE
    player.press(Button::Speak);
    player.down_to(|s| s.camp_view().unwrap().state_selection, 2); // SAVE
    player.press(Button::Speak);
    assert_eq!(camp_page(&player), CampPage::SaveSlots);
    player.down_to(|s| s.camp_view().unwrap().save_selection, 1);
    // A session without a store refuses the row: the shell resolved no
    // directory, and a runtime never guesses one.
    player.session.set_save_store(SaveStore::new(&directory));
    let frame = player.tick(Pad::new(Button::Speak));
    assert_eq!(frame.mode, FrameMode::Camp);
    assert_eq!(camp_page(&player), CampPage::SaveResult);
    assert_eq!(player.session.camp_view().unwrap().message, "FILE SAVED");
    assert!(
        directory.join("slot_2.sram").is_file(),
        "the picked slot was written"
    );
    assert!(
        !directory.join("slot_1.sram").exists(),
        "only the picked slot was written"
    );
    // The written slot is the state the session is in: a fresh session over the
    // same store loads the same party and purse.
    let (data, _) = pack();
    let loaded = SaveStore::new(&directory)
        .load(data.clone(), 1, StepFrames::default())
        .expect("the slot loads");
    assert_eq!(
        loaded.game().snapshot(),
        player.runtime().game().snapshot(),
        "camp SAVE wrote the live state"
    );
    std::fs::remove_dir_all(&directory).expect("the run directory is ours");
    player.tick(Pad::NEUTRAL);
    player.press(Button::Speak);
    assert_eq!(camp_page(&player), CampPage::State);
}

/// Negative control: a session whose run named no directory refuses the camp's
/// SAVE with the store's own message and writes nothing.
#[test]
fn a_save_without_a_run_directory_is_refused() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let mut player = Player::new(FIELD, &camp_game());
    player.press(Button::Camp);
    player.down_to(|s| s.camp_view().unwrap().root_selection, 4);
    player.press(Button::Speak);
    player.down_to(|s| s.camp_view().unwrap().state_selection, 2);
    player.press(Button::Speak);
    player.down_to(|s| s.camp_view().unwrap().save_selection, 0);
    assert!(player.session.save_store().is_none());
    assert_eq!(player.session.save_slots(), [false; 3]);
    assert_eq!(
        player
            .session
            .save_slot(0)
            .err()
            .map(|error| error.to_string()),
        Some("the session has no save directory for this run".to_owned())
    );
    player.tick(Pad::new(Button::Speak));
    assert_eq!(
        player.session.camp_view().unwrap().message,
        "SAVE ERROR: the session has no save directory for this run"
    );
}

/// The cartridge's menus confirm on Speak or Camp (`ps4.asm:134986`,
/// `117260-117281`): Camp advances a shop greeting and picks a camp option.
#[test]
fn the_camp_button_confirms_in_shops_and_the_camp() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let mut player = Player::new(ITEM_SHOP, &chaz_alone(100));
    player.talk_up();
    player.tick(Pad::NEUTRAL);
    player.press(Button::Camp);
    assert_eq!(
        shop_page(&player),
        ShopPage::Root,
        "Camp confirms the greeting"
    );
    player.press(Button::Cancel);
    assert!(player.session.shop_view().is_none());

    player.press(Button::Camp);
    assert_eq!(camp_page(&player), CampPage::Root, "Camp opens the menu");
    player.press(Button::Down); // TECH
    player.press(Button::Camp);
    assert_eq!(
        camp_page(&player),
        CampPage::AbilityCharacters,
        "Camp picks the highlighted option"
    );
}

/// Start closes the whole camp from a cursor page and dismisses a result line
/// like any other button (`ps4.asm:117260`, `122511`, `123286`).
#[test]
fn start_closes_the_camp_from_a_cursor_page_and_dismisses_a_result() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let mut player = Player::new(ITEM_SHOP, &camp_game());
    player.press(Button::Camp);
    player.down_to(|s| s.camp_view().unwrap().root_selection, 4); // STATE
    player.press(Button::Speak);
    assert_eq!(camp_page(&player), CampPage::State);
    player.press(Button::Start);
    assert!(
        player.session.camp_view().is_none(),
        "Start closed every page"
    );

    // A result line: ITEM on a pack with nothing usable reports "NOT USABLE".
    player.press(Button::Camp);
    player.press(Button::Speak); // ITEM
    assert_eq!(camp_page(&player), CampPage::ItemList);
    player.press(Button::Speak); // the Dagger
    assert_eq!(camp_page(&player), CampPage::ItemResult);
    player.press(Button::Start);
    assert!(
        player.session.camp_view().is_some(),
        "Start dismissed the result, not the menu"
    );
    assert_ne!(camp_page(&player), CampPage::ItemResult);
}

/// The negative control for the Camp button: while a scene runs, the button
/// does not open the menu.
#[test]
fn camp_pressed_while_a_scene_runs_does_not_open_the_menu() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let data = pack().0.clone();
    let event = data.new_game().expect("title initializer").event_index;
    let mut runtime = Runtime::new_game(data, StepFrames::default()).expect("START");
    assert!(runtime.start_event(event), "the opening event starts");
    let mut player = Player {
        session: Session::new(runtime),
    };
    assert!(player.runtime().scene_active());
    for _ in 0..6 {
        player.press(Button::Camp);
    }
    assert!(
        player.session.camp_view().is_none(),
        "the Camp button is not a menu while the story owns the party"
    );
    assert!(
        player.runtime().scene_active(),
        "the scene is still running"
    );
}

#[test]
fn camp_opens_on_the_pad_edge_and_not_on_a_held_button() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    // Held across the frame the menu closes on, the button must not reopen it:
    // an edge opens the menu, a level does not.
    let mut player = Player::new(ITEM_SHOP, &camp_game());
    player.tick(Pad::new(Button::Camp));
    assert!(player.session.camp_view().is_some());
    player.tick(Pad::of(&[Button::Camp, Button::Cancel]));
    assert!(player.session.camp_view().is_none(), "Cancel closed it");
    for _ in 0..5 {
        player.tick(Pad::of(&[Button::Camp, Button::Cancel]));
    }
    assert!(
        player.session.camp_view().is_none(),
        "a held button is not a new press"
    );
    player.tick(Pad::NEUTRAL);
    player.tick(Pad::new(Button::Camp));
    assert!(
        player.session.camp_view().is_some(),
        "a fresh press opens it again"
    );
}

/// The chest in map `$47` at (17,17), read from below: it holds a Monomate
/// (`src/loot_tests.rs`, `chest_item_normal_input_grants_once...`).
const CHEST: (u16, u16, u16) = (0x47, 17, 18);

/// Forty Daggers: no free slot.
fn full_pack() -> GameState {
    let mut game = chaz_alone(0);
    for _ in 0..40 {
        game.inventory_mut().add(1).unwrap();
    }
    game
}

#[test]
fn a_chest_opens_into_the_pack_and_its_window_closes_on_confirm() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let mut player = Player::new(CHEST, &chaz_alone(0));
    player.talk_up();
    // The frame after the open, the field has parked and the window is up.
    assert_eq!(camp_page(&player), CampPage::LootMessage);
    let view = player.session.camp_view().unwrap();
    assert!(view.is_loot());
    assert_eq!(
        view.message,
        "Chest has been opened!\nMONOMATE is procured!"
    );
    assert_eq!(player.runtime().game().inventory().get(0), Some(125));
    assert!(player.runtime().loot_state().is_some());

    player.press(Button::Speak);
    assert!(player.session.camp_view().is_none(), "the window closed");
    assert!(player.runtime().loot_state().is_none());
    assert_eq!(player.runtime().game().inventory().get(0), Some(125));
}

#[test]
fn a_full_pack_can_give_the_chest_up_and_keeps_every_item() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let mut player = Player::new(CHEST, &full_pack());
    player.talk_up();
    assert_eq!(camp_page(&player), CampPage::LootMessage);
    assert!(
        player
            .session
            .camp_view()
            .unwrap()
            .message
            .contains("Itempack is full!")
    );
    player.press(Button::Speak); // the message -> the pack list
    assert_eq!(camp_page(&player), CampPage::LootItems);
    player.press(Button::Cancel); // give it up?
    assert_eq!(camp_page(&player), CampPage::LootReturnConfirm);
    assert_eq!(
        player.session.camp_view().unwrap().confirm_selection,
        1,
        "NO is the cartridge's default"
    );
    player.press(Button::Down); // YES
    player.press(Button::Speak);
    assert!(player.session.camp_view().is_none());
    assert!(player.runtime().loot_state().is_none());
    assert!(
        player
            .runtime()
            .game()
            .inventory()
            .slots()
            .iter()
            .all(|id| *id == 1),
        "nothing was discarded or added"
    );
}

#[test]
fn a_full_pack_can_discard_an_item_to_take_the_chest() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let mut player = Player::new(CHEST, &full_pack());
    player.talk_up();
    player.press(Button::Speak); // the pack list
    assert_eq!(camp_page(&player), CampPage::LootItems);
    player.press(Button::Speak); // discard the first Dagger?
    assert_eq!(camp_page(&player), CampPage::LootDiscardConfirm);
    // NO is the default: confirming it only returns to the list.
    player.press(Button::Speak);
    assert_eq!(camp_page(&player), CampPage::LootItems);
    player.press(Button::Speak);
    player.press(Button::Down); // YES
    player.press(Button::Speak);
    assert_eq!(camp_page(&player), CampPage::LootMessage);
    assert_eq!(
        player.session.camp_view().unwrap().message,
        "DAGGER discarded\nMONOMATE is procured."
    );
    let slots = player.runtime().game().inventory().slots().to_vec();
    assert_eq!(slots.iter().filter(|id| **id == 1).count(), 39);
    assert_eq!(slots.iter().filter(|id| **id == 125).count(), 1);
    player.press(Button::Speak);
    assert!(player.session.camp_view().is_none());
}
