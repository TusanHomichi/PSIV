//! Shop and inn page builders: one window layout per mode.
//!
//! Every rectangle here is the retail decode in `layout`; this module only
//! decides which page the current mode shows and which values ride on it.

use godot::classes::ImageTexture;
use godot::prelude::*;

use super::chrome::{Quad, ShopChrome};
use super::layout::{
    BUY_LIST, BUY_QUANTITY, BUY_SELL_MENU, INN_CONFIRM, MAIN_MESSAGE, MONEY, PORTRAIT, SELL_ITEM,
    SELL_PANE,
};
use psiv_runtime::{ShopCounterView, ShopPage, ShopView};

use super::ShopWindow;

pub(super) type ShopDrawList = (Vec<Quad>, Option<(Gd<ImageTexture>, Rect2)>);

impl ShopWindow {
    pub(super) fn draw_list(&self) -> Option<ShopDrawList> {
        let chrome = self.chrome.as_ref()?;
        let view = self.view.as_ref()?;
        let counter = &view.counter;
        let mut quads = Vec::new();
        let mut portrait = None;
        let money = format!("{} MST", view.money);
        quads.extend(chrome.frame(MONEY));
        draw_text(chrome, &mut quads, &money, centered_money_cell(&money));

        if counter.is_inn {
            quads.extend(chrome.frame(PORTRAIT));
            if let Some(texture) = self.portrait.as_ref() {
                portrait = Some((
                    texture.clone(),
                    Rect2::new(Vector2::new(48.0, 56.0), Vector2::new(48.0, 48.0)),
                ));
            }
            match view.page {
                ShopPage::InnConfirm => draw_inn_confirm(chrome, view, &mut quads),
                ShopPage::Message => draw_message(chrome, &mut quads, &view.message),
                _ => draw_message(chrome, &mut quads, "Welcome! This is the inn."),
            }
        } else {
            quads.extend(chrome.frame(BUY_SELL_MENU));
            draw_text(chrome, &mut quads, "buy", (8, 15));
            draw_text(chrome, &mut quads, "sell", (8, 17));
            let cursor_y = 15 + (view.root_selection as i32 * 2);
            if let Some(cursor) = chrome.word(0x6E8, (6, cursor_y)) {
                quads.push(cursor);
            }
            match view.page {
                ShopPage::Greeting => draw_message(
                    chrome,
                    &mut quads,
                    &format!("Welcome! This is\nthe {} store.", trade_name(counter)),
                ),
                ShopPage::BuyList => draw_buy_list(chrome, view, &mut quads, false),
                ShopPage::BuyConfirm => draw_buy_list(chrome, view, &mut quads, true),
                ShopPage::SellList => draw_sell_list(chrome, view, &mut quads, false),
                ShopPage::SellConfirm => draw_sell_list(chrome, view, &mut quads, true),
                ShopPage::Message => draw_message(chrome, &mut quads, &view.message),
                _ => draw_message(chrome, &mut quads, "Do you need anything else?"),
            }
        }
        Some((quads, portrait))
    }
}

fn draw_buy_list(chrome: &ShopChrome, view: &ShopView, quads: &mut Vec<Quad>, confirm: bool) {
    quads.extend(chrome.frame(BUY_LIST));
    if let Some(item) = view.stock.get(view.item_selection) {
        draw_text(chrome, quads, &item.name, (17, 7));
        let price = item.price.to_string();
        draw_text(
            chrome,
            quads,
            &price,
            (32 - price.chars().count() as i32, 7),
        );
        let description = if item.symbol == "Monomate" {
            "A weak medicine that\nrestores HP."
        } else {
            &item.name
        };
        if confirm {
            quads.extend(chrome.frame(BUY_QUANTITY));
            draw_yes_no(chrome, quads, view.confirm_selection, (9, 15));
            draw_message(
                chrome,
                quads,
                &format!("{}.\nIs this what you want?", item.name),
            );
        } else {
            draw_message(chrome, quads, description);
        }
    }
}

fn draw_sell_list(chrome: &ShopChrome, view: &ShopView, quads: &mut Vec<Quad>, confirm: bool) {
    let Some(item) = view.items.get(view.item_selection) else {
        draw_message(chrome, quads, "You don't have anything to sell.");
        return;
    };
    quads.extend(chrome.frame(SELL_ITEM));
    draw_text(chrome, quads, &item.name, (21, 3));
    quads.extend(chrome.frame(SELL_PANE));
    if confirm {
        draw_yes_no(chrome, quads, view.confirm_selection, (10, 15));
        draw_message(
            chrome,
            quads,
            &format!("{}?\n{} meseta.", item.name, item.sell_price),
        );
    } else {
        if let Some(cursor) = chrome.word(0x6E8, (8, 14)) {
            quads.push(cursor);
        }
        draw_message(chrome, quads, "What would you like to sell?");
    }
}

fn draw_inn_confirm(chrome: &ShopChrome, view: &ShopView, quads: &mut Vec<Quad>) {
    quads.extend(chrome.frame(INN_CONFIRM));
    draw_yes_no(chrome, quads, view.confirm_selection, (9, 15));
    draw_message(
        chrome,
        quads,
        &format!("{} meseta.\nWould you care to stay?", view.inn_cost),
    );
}

fn draw_yes_no(chrome: &ShopChrome, quads: &mut Vec<Quad>, selected: usize, text_cell: (i32, i32)) {
    draw_text(chrome, quads, "YES", text_cell);
    draw_text(chrome, quads, "NO", (text_cell.0, text_cell.1 + 2));
    if let Some(cursor) = chrome.word(0x6E8, (text_cell.0 - 2, text_cell.1 + selected as i32 * 2)) {
        quads.push(cursor);
    }
}

fn draw_message(chrome: &ShopChrome, quads: &mut Vec<Quad>, message: &str) {
    quads.extend(chrome.frame(MAIN_MESSAGE));
    draw_text(chrome, quads, message, (4, 21));
}

fn draw_text(chrome: &ShopChrome, quads: &mut Vec<Quad>, text: &str, cell: (i32, i32)) {
    quads.extend(chrome.text(text, cell));
}

fn centered_money_cell(text: &str) -> (i32, i32) {
    let width = text.chars().count() as i32;
    (2 + ((13 - width).max(0) / 2), 3)
}

fn trade_name(counter: &ShopCounterView) -> &'static str {
    match counter.trade_fragment {
        0 => "weapon",
        1 => "armor",
        _ => "item",
    }
}
