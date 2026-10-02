//! The shop and inn window: the counter's state machine, one frame at a time.
//!
//! A shopkeeper across a counter opens this mode (`Session::route`); from then
//! on the pad belongs to it until the party leaves. It owns every rule the
//! Godot window used to carry: which counter, what it stocks, the "nothing to
//! sell" refusal, what a sale pays, the bill an inn presents, and the order of
//! the pages. The window the shell draws is the [`ShopView`].
//!
//! Buttons are the shell's mapping from before this moved, kept as it was: the
//! d-pad moves the cursor, `Speak` confirms, `Cancel` backs out. The cartridge
//! also confirms on `Camp` (`ps4.asm:135157`, `andi.b #ButtonSpeak_Mask|ButtonCamp_Mask`);
//! the shell binds the camp key to the same physical keys as cancel, so reading
//! `Camp` as a confirm here would make one key both answers.
//!
//! The money and inventory changes themselves are `shop.rs`'s transactions
//! (`ps4.asm:135153` buy, `ps4.asm:135570` sell): the price word spent
//! unchanged, and halved with `lsr.w #1`.

use psiv_data::{ShopCounter, ShopData};

use crate::pad::{Button, Pad};
use crate::{InnResult, Runtime, ShopBuyResult, ShopSellResult};

/// The page the shop window shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShopPage {
    /// A shopkeeper's greeting, before the buy/sell menu.
    Greeting,
    /// An innkeeper's greeting.
    InnGreeting,
    /// BUY / SELL.
    Root,
    /// The stock list.
    BuyList,
    /// "Is this what you want?"
    BuyConfirm,
    /// The party's items.
    SellList,
    /// "What would you like to sell?" confirm.
    SellConfirm,
    /// The bill and "Would you care to stay?".
    InnConfirm,
    /// A transaction's result, until acknowledged.
    Message,
}

/// One item on the sell list: an occupied inventory slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShopOwnedItem {
    /// The raw inventory slot, the key a sale removes.
    pub slot: usize,
    /// The item id.
    pub id: u8,
    /// The cartridge's name for the item.
    pub name: String,
    /// The record's price word (`InventoryData` `$14`).
    pub price: u32,
    /// What a sale pays: the price word halved, rounding down.
    pub sell_price: u32,
}

/// One item on the buy list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShopStock {
    /// The item id.
    pub item_id: u8,
    /// The cartridge's name for the item.
    pub name: String,
    /// The disassembly's symbol, which the window keys its description on.
    pub symbol: String,
    /// The price, spent unchanged.
    pub price: u32,
}

/// The counter the window is open at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShopCounterView {
    /// The counter row's id.
    pub id: usize,
    /// Whether it is an inn.
    pub is_inn: bool,
    /// The shopkeeper's portrait art address.
    pub portrait: String,
    /// The pack path of the portrait image, when the pack has one.
    pub portrait_png: Option<String>,
    /// The greeting's trade noun: `0` weapon, `1` armor, anything else item.
    pub trade_fragment: u8,
}

/// Everything the shop window draws, and the cursors it draws them at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShopView {
    /// The page.
    pub page: ShopPage,
    /// BUY (0) or SELL (1).
    pub root_selection: usize,
    /// The cursor on whichever item list is up.
    pub item_selection: usize,
    /// YES (0) or NO (1).
    pub confirm_selection: usize,
    /// Meseta held.
    pub money: u32,
    /// The party's items, in slot order.
    pub items: Vec<ShopOwnedItem>,
    /// What the counter sells. Empty for an inn.
    pub stock: Vec<ShopStock>,
    /// What a night costs the party as it stands: `rate * occupied slots`.
    pub inn_cost: u32,
    /// The result page's text.
    pub message: String,
    /// Where the party is shopping.
    pub counter: ShopCounterView,
    accept_blocked: bool,
    inn_selector: Option<usize>,
}

/// What a frame of the shop did to the mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShopOutcome {
    /// The window stays up.
    Open,
    /// The party left the counter.
    Closed,
}

impl ShopView {
    /// Opens the window at `counter`.
    pub(crate) fn open(counter: &ShopCounter, shops: &ShopData, runtime: &Runtime) -> ShopView {
        let mut view = ShopView {
            page: if counter.is_inn() {
                ShopPage::InnGreeting
            } else {
                ShopPage::Greeting
            },
            root_selection: 0,
            item_selection: 0,
            confirm_selection: 0,
            money: 0,
            items: Vec::new(),
            stock: counter
                .shop_inventory_index
                .and_then(|index| shops.inventory(index))
                .map_or_else(Vec::new, |inventory| {
                    inventory
                        .items
                        .iter()
                        .map(|item| ShopStock {
                            item_id: item.item_id,
                            name: item.display_name.clone(),
                            symbol: item.symbol.clone(),
                            price: item.buy_price,
                        })
                        .collect()
                }),
            inn_cost: 0,
            message: String::new(),
            counter: ShopCounterView {
                id: counter.id,
                is_inn: counter.is_inn(),
                portrait: counter.portrait.clone(),
                portrait_png: shops.portrait_png(&counter.portrait).map(str::to_owned),
                trade_fragment: counter.greeting.as_ref().map_or(2, |g| g.trade_fragment),
            },
            accept_blocked: true,
            inn_selector: counter.inn_index,
        };
        view.refresh(runtime);
        view
    }

    /// Re-reads the purse, the party's items and the inn's bill.
    fn refresh(&mut self, runtime: &Runtime) {
        let game = runtime.game();
        self.money = game.money();
        self.inn_cost = self
            .inn_selector
            .and_then(|index| runtime.data().shops()?.inn(index))
            .map_or(0, |inn| {
                inn.rate_per_character
                    .saturating_mul(game.party_len() as u32)
            });
        self.items = game
            .inventory()
            .slots()
            .iter()
            .enumerate()
            .filter(|&(_, &id)| id != 0)
            .map(|(slot, &id)| {
                let price = price_word(runtime, id);
                ShopOwnedItem {
                    slot,
                    id,
                    name: runtime.item_name(id),
                    price,
                    sell_price: price / 2,
                }
            })
            .collect();
    }

    /// One frame of the window. `pressed` is this frame's edges.
    pub(crate) fn frame(&mut self, runtime: &mut Runtime, pad: Pad, pressed: Pad) -> ShopOutcome {
        if pressed.held(Button::Cancel) {
            if self.go_back() {
                return ShopOutcome::Closed;
            }
            self.refresh(runtime);
            return ShopOutcome::Open;
        }
        if self.accept_blocked {
            // The press that opened the window must be released first.
            if !pad.held(Button::Speak) {
                self.accept_blocked = false;
            }
            self.refresh(runtime);
            return ShopOutcome::Open;
        }
        let up = pressed.held(Button::Up);
        let down = pressed.held(Button::Down);
        let accept = pressed.held(Button::Speak);
        match self.page {
            ShopPage::Greeting => {
                if accept {
                    self.page = ShopPage::Root;
                }
            }
            ShopPage::InnGreeting => {
                if accept {
                    self.page = ShopPage::InnConfirm;
                    self.confirm_selection = 0;
                }
            }
            ShopPage::Root => {
                if up {
                    self.root_selection = wrap(self.root_selection, 2, false);
                } else if down {
                    self.root_selection = wrap(self.root_selection, 2, true);
                } else if accept {
                    if self.root_selection == 0 {
                        self.page = ShopPage::BuyList;
                    } else if self.items.is_empty() {
                        self.message = "You don't have anything to sell.".to_owned();
                        self.page = ShopPage::Message;
                    } else {
                        self.item_selection = 0;
                        self.page = ShopPage::SellList;
                    }
                }
            }
            ShopPage::BuyList => {
                let count = self.stock.len();
                if up {
                    self.item_selection = wrap(self.item_selection, count, false);
                } else if down {
                    self.item_selection = wrap(self.item_selection, count, true);
                } else if accept && count > 0 {
                    self.confirm_selection = 0;
                    self.page = ShopPage::BuyConfirm;
                }
            }
            ShopPage::BuyConfirm => {
                if up || down {
                    self.confirm_selection = 1usize.saturating_sub(self.confirm_selection);
                } else if accept {
                    if self.confirm_selection == 0 {
                        self.confirm_buy(runtime);
                    } else {
                        self.page = ShopPage::BuyList;
                    }
                }
            }
            ShopPage::SellList => {
                let count = self.items.len();
                if up {
                    self.item_selection = wrap(self.item_selection, count, false);
                } else if down {
                    self.item_selection = wrap(self.item_selection, count, true);
                } else if accept && count > 0 {
                    self.confirm_selection = 0;
                    self.page = ShopPage::SellConfirm;
                }
            }
            ShopPage::SellConfirm => {
                if up || down {
                    self.confirm_selection = 1usize.saturating_sub(self.confirm_selection);
                } else if accept {
                    if self.confirm_selection == 0 {
                        self.confirm_sell(runtime);
                    } else {
                        self.page = ShopPage::SellList;
                    }
                }
            }
            ShopPage::InnConfirm => {
                if up || down {
                    self.confirm_selection = 1usize.saturating_sub(self.confirm_selection);
                } else if accept {
                    if self.confirm_selection == 0 {
                        self.confirm_inn(runtime);
                    } else {
                        self.page = ShopPage::InnGreeting;
                    }
                }
            }
            ShopPage::Message => {
                if accept {
                    self.page = self.resting_page();
                }
            }
        }
        self.refresh(runtime);
        ShopOutcome::Open
    }

    /// The page a result returns to.
    fn resting_page(&self) -> ShopPage {
        if self.counter.is_inn {
            ShopPage::InnGreeting
        } else {
            ShopPage::Root
        }
    }

    /// Backs one page out; `true` when there is nowhere left and the window
    /// closes.
    fn go_back(&mut self) -> bool {
        match self.page {
            ShopPage::Greeting | ShopPage::InnGreeting | ShopPage::Root => return true,
            ShopPage::BuyList | ShopPage::SellList => self.page = ShopPage::Root,
            ShopPage::BuyConfirm => self.page = ShopPage::BuyList,
            ShopPage::SellConfirm => self.page = ShopPage::SellList,
            ShopPage::InnConfirm => self.page = ShopPage::InnGreeting,
            ShopPage::Message => self.page = self.resting_page(),
        }
        false
    }

    fn confirm_buy(&mut self, runtime: &mut Runtime) {
        let Some(item) = self.stock.get(self.item_selection) else {
            self.page = ShopPage::BuyList;
            return;
        };
        self.message = match runtime.shop_buy(item.item_id, item.price) {
            ShopBuyResult::Bought { .. } => "Thank you very much.\nAnything else?".to_owned(),
            ShopBuyResult::InventoryFull { .. } => "You can't carry anything else.".to_owned(),
            ShopBuyResult::InsufficientFunds { .. } => "You don't have enough money!".to_owned(),
        };
        self.page = ShopPage::Message;
    }

    fn confirm_sell(&mut self, runtime: &mut Runtime) {
        let Some(item) = self.items.get(self.item_selection) else {
            self.page = ShopPage::SellList;
            return;
        };
        self.message = match runtime.shop_sell(item.slot, item.price) {
            ShopSellResult::Sold { .. } => "Thank you very much.\nAnything else?".to_owned(),
            ShopSellResult::EmptySlot => "That item is no longer there.".to_owned(),
        };
        self.page = ShopPage::Message;
    }

    fn confirm_inn(&mut self, runtime: &mut Runtime) {
        let Some(index) = self.inn_selector else {
            self.page = ShopPage::InnGreeting;
            return;
        };
        let Some(rate) = runtime
            .data()
            .shops()
            .and_then(|shops| shops.inn(index))
            .map(|inn| inn.rate_per_character)
        else {
            self.page = ShopPage::InnGreeting;
            return;
        };
        self.message = match runtime.shop_stay(rate, index) {
            InnResult::Stayed { .. } => "Thank you very much.\nPlease come again.".to_owned(),
            InnResult::InsufficientFunds { .. } => "You don't have enough money!".to_owned(),
            InnResult::AiedoEventPending { .. } => "Aiedo rest event pending.".to_owned(),
        };
        self.page = ShopPage::Message;
    }
}

/// An item's price word (`ps4.asm:135570` reads `InventoryData` `$14` for a
/// sale, whatever the item). A runtime without battle data falls back to the
/// stock lists' price for the item, and an item with neither is worth nothing.
fn price_word(runtime: &Runtime, item: u8) -> u32 {
    runtime
        .item_price(item)
        .or_else(|| runtime.data().shops()?.listed_price(item))
        .unwrap_or(0)
}

pub(crate) fn wrap(current: usize, count: usize, forward: bool) -> usize {
    if count == 0 {
        return 0;
    }
    if forward {
        (current + 1) % count
    } else if current == 0 {
        count - 1
    } else {
        current - 1
    }
}
