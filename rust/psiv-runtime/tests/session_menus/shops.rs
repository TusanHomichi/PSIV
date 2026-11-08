//! The shop and inn window through ordinary pad input: buying, selling, a
//! night at the inn, and how a visit ends (#49).
//!
//! A greeting's wait takes Cancel as it takes Speak (`loc_65F74`). Cancel at
//! the BUY/SELL menu (`Win_ShopBuySell`'s `loc_65014`), and Cancel or NO at
//! the inn's bill (`Win_RestConfirm`'s `loc_660B6`), say the counter's farewell
//! and close the window only when it is acknowledged; an inn's result line
//! closes it too. Every line compared here is the pack's (`shops.json`'s
//! `lines`), never a copy in the test.

use super::*;

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

    // Back out: the message returns to the menu, Cancel there says the
    // farewell, and its acknowledgement closes the window.
    player.press(Button::Speak);
    assert_eq!(shop_page(&player), ShopPage::Root);
    player.press(Button::Cancel);
    assert_eq!(shop_page(&player), ShopPage::Farewell);
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

/// The counter the window is open at, as the pack records it.
fn counter(player: &Player) -> psiv_data::ShopCounter {
    let id = player.session.shop_view().unwrap().counter.id;
    player.runtime().data().shops().unwrap().counters[id].clone()
}

fn lines(player: &Player) -> psiv_data::ShopLines {
    player.runtime().data().shops().unwrap().lines.clone()
}

fn message(player: &Player) -> String {
    player.session.shop_view().unwrap().message.clone()
}

#[test]
fn cancel_on_a_greeting_advances_it_as_speak_does() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let mut player = Player::new(ITEM_SHOP, &chaz_alone(100));
    player.talk_up();
    player.tick(Pad::NEUTRAL);
    assert_eq!(shop_page(&player), ShopPage::Greeting);
    player.press(Button::Cancel);
    assert_eq!(shop_page(&player), ShopPage::Root, "the shop's menu");

    let mut player = Player::new(INN, &chaz_alone(100));
    player.talk_up();
    player.tick(Pad::NEUTRAL);
    assert_eq!(shop_page(&player), ShopPage::InnGreeting);
    player.press(Button::Cancel);
    assert_eq!(shop_page(&player), ShopPage::InnConfirm, "the inn's bill");
}

#[test]
fn cancel_at_the_menu_says_the_shopkeepers_farewell_then_leaves() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    let mut player = Player::new(ITEM_SHOP, &chaz_alone(100));
    player.talk_up();
    player.tick(Pad::NEUTRAL);
    player.press(Button::Speak);
    assert_eq!(shop_page(&player), ShopPage::Root);
    let counter = counter(&player);
    assert_eq!(counter.group.id, 1, "an ordinary shop");
    let variant = usize::from(counter.greeting.unwrap().text_variant);

    player.press(Button::Cancel);
    assert_eq!(shop_page(&player), ShopPage::Farewell);
    let farewell = lines(&player).farewell[variant].clone();
    assert!(!farewell.is_empty());
    assert_eq!(message(&player), farewell, "variant {variant}'s line");
    // It waits: a direction does not close it.
    player.press(Button::Down);
    assert_eq!(shop_page(&player), ShopPage::Farewell);
    player.press(Button::Speak);
    assert!(player.session.shop_view().is_none(), "the window closed");
}

#[test]
fn cancel_or_no_at_the_inn_bill_says_the_innkeepers_farewell() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    for refuse in [Button::Cancel, Button::Down] {
        let mut player = Player::new(INN, &chaz_alone(100));
        player.talk_up();
        player.tick(Pad::NEUTRAL);
        player.press(Button::Speak);
        assert_eq!(shop_page(&player), ShopPage::InnConfirm);
        let before = player.runtime().game().snapshot();
        let index = counter(&player).inn_index.unwrap();
        let variant = usize::from(
            player
                .runtime()
                .data()
                .shops()
                .unwrap()
                .inn(index)
                .unwrap()
                .text_variant,
        );
        player.press(refuse);
        if refuse == Button::Down {
            player.press(Button::Speak); // NO
        }
        assert_eq!(shop_page(&player), ShopPage::Farewell, "{refuse:?}");
        assert_eq!(message(&player), lines(&player).inn_farewell[variant]);
        assert!(!message(&player).is_empty());
        player.press(Button::Cancel);
        assert!(player.session.shop_view().is_none(), "{refuse:?}");
        assert_eq!(player.runtime().game().snapshot(), before, "no charge");
    }
}

/// The morning's line and a refused bill's both end the visit.
#[test]
fn an_inns_result_line_closes_the_window() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    for money in [100, 0] {
        let mut player = Player::new(INN, &chaz_alone(money));
        player.talk_up();
        player.tick(Pad::NEUTRAL);
        player.press(Button::Speak);
        player.press(Button::Speak); // YES
        assert_eq!(shop_page(&player), ShopPage::Message, "{money}");
        player.press(Button::Speak);
        assert!(player.session.shop_view().is_none(), "{money}");
    }
}
