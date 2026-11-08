//! How a shop or inn visit ends, through ordinary pad input (#49).
//!
//! A greeting's wait takes Cancel as it takes Speak (`loc_65F74`). Cancel at
//! the BUY/SELL menu (`Win_ShopBuySell`'s `loc_65014`), and Cancel or NO at
//! the inn's bill (`Win_RestConfirm`'s `loc_660B6`), say the counter's farewell
//! and close the window only when it is acknowledged; an inn's result line
//! closes it too. Every line compared here is the pack's (`shops.json`'s
//! `lines`), never a copy in the test.

use super::*;

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
