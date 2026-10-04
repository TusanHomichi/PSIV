//! The shop and inn window: the counter's state machine, one frame at a time.
//!
//! A shopkeeper across a counter opens this mode (`Session::route`); from then
//! on the pad belongs to it until the party leaves. It owns every rule the
//! Godot window used to carry: which counter, what it stocks, the "nothing to
//! sell" refusal, what a sale pays, the bill an inn presents, and the order of
//! the pages. The window the shell draws is the [`ShopView`].
//!
//! Buttons are the cartridge's. The shop routines confirm on
//! `ButtonSpeak_Mask|ButtonCamp_Mask` and back out on `ButtonCancel`: the
//! BUY/SELL menu (`ps4.asm:134986`, `135010`) and the inn's confirmation
//! (`ps4.asm:136334-136339`, which tests Cancel first); the buy list, sell list
//! and their confirmations carry the same two tests (`ps4.asm:135155-135695`,
//! read for the masks, not page by page). A result line and the greeting's
//! scroll arrow wait for any of `Cancel|Speak|Camp` (`ps4.asm:135029`,
//! `136246`). No shop routine reads `ButtonStart`. Two port page-graph
//! differences remain, listed in `docs/camp/SHOPS.md`: Cancel on a greeting
//! leaves where the cartridge advances, and Cancel at the BUY/SELL menu leaves
//! without the cartridge's farewell line.
//!
//! The money and inventory changes themselves are `shop.rs`'s transactions
//! (`ps4.asm:135153` buy, `ps4.asm:135570` sell): the price word spent
//! unchanged, and halved with `lsr.w #1`.

use psiv_data::{ShopCounter, ShopData};

use psiv_core::Input;

use crate::pad::{Button, Pad};
use crate::{InnOpening, Runtime, ShopBuyResult, ShopSellResult};

use super::menu_scene::MenuScene;
use super::{Frame, FrameMode, Session};

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
    /// A rest that handed the party to a scene: the session destroys this
    /// window, runs the scene, and rebuilds the window from here.
    pub(crate) rest_scene: Option<RestScene>,
}

/// The Aiedo rest's mid-transaction hand-off (`ps4.asm:136391-136414`): the
/// event the window is waiting on, the bill it has not charged, and the counter
/// it must rebuild itself at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RestScene {
    /// `Event_GirlsSneakingOut`.
    pub(crate) event: u16,
    /// The bill `RecoverStats` was priced at, charged after the scene.
    pub(crate) cost: u32,
    /// The counter row id, so the window can be rebuilt on the same view.
    pub(crate) counter: usize,
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
            rest_scene: None,
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
        let accept = pressed.held(Button::Speak) || pressed.held(Button::Camp);
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
            ShopPage::BuyList | ShopPage::SellList => {
                // Both lists share the cartridge's `Window_Option_Index_3`,
                // and leaving either by Cancel zeroes it (`ps4.asm:135182`
                // from the buy list, `ps4.asm:135508` from the sell list): the
                // next list opens on its first row.
                self.item_selection = 0;
                self.page = ShopPage::Root;
            }
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
        // The cartridge tests the bill and runs `RecoverStats` first
        // (`ps4.asm:136367-136388`), then, at the Aiedo counter only, hands the
        // party to `Event_GirlsSneakingOut` — with the bill, selector and text
        // variant saved around it — and charges the bill when it returns
        // (`:136391-136414`). An ordinary night charges it here.
        match runtime.inn_begin(rate) {
            InnOpening::InsufficientFunds { .. } => {
                self.message = "You don't have enough money!".to_owned();
                self.page = ShopPage::Message;
            }
            InnOpening::Resting { cost, .. } => match runtime.rest_event(index) {
                Some(event) => {
                    self.rest_scene = Some(RestScene {
                        event,
                        cost,
                        counter: self.counter.id,
                    });
                }
                None => {
                    runtime.inn_charge(cost);
                    self.message = "Thank you very much.\nPlease come again.".to_owned();
                    self.page = ShopPage::Message;
                }
            },
        }
    }

    /// Takes the rest's scene hand-off, once, for the session to run.
    pub(crate) fn take_rest_scene(&mut self) -> Option<RestScene> {
        self.rest_scene.take()
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

impl Session {
    /// One frame of the shop or inn window, and the scene a rest handed the
    /// party to.
    ///
    /// The cartridge destroys the windows before the inn's scene runs and
    /// rebuilds them after it (`ps4.asm:136391-136414`): this frame destroys
    /// them, [`MenuScene`] keeps the transaction open while the field runs the
    /// scene, and the resume rebuilds the window over the closed bill.
    pub(crate) fn shop_frame(&mut self, pad: Pad, pressed: Pad) -> Frame {
        let shop = self.shop.as_mut().expect("a shop frame has a shop");
        let outcome = shop.frame(&mut self.runtime, pad, pressed);
        let handoff = shop.take_rest_scene();
        if let Some(handoff) = handoff {
            // The window is gone for the scene's whole run, exactly as the
            // cartridge's three `Window_Destroy` calls leave it.
            self.shop = None;
            self.runtime.set_field_suspended(false);
            let counter = handoff.counter;
            let cost = handoff.cost;
            let handoff = MenuScene::AiedoInn {
                event: handoff.event,
                counter,
                cost,
            };
            if !self.start_menu_scene(handoff) {
                // A pack without the event transcribed still closes the
                // transaction: the bill was priced before the scene and must
                // not be lost to a missing one.
                self.finish_aiedo_stay(counter, cost);
            }
            return Frame {
                mode: FrameMode::Shop,
                ..Frame::default()
            };
        }
        if outcome == ShopOutcome::Closed {
            self.shop = None;
            self.runtime.set_field_suspended(false);
            return Frame {
                mode: FrameMode::Shop,
                ..Frame::default()
            };
        }
        self.runtime.set_field_suspended(true);
        let events = self.runtime.tick(Input::Neutral);
        let (events, routed) = self.route(events);
        Frame {
            mode: FrameMode::Shop,
            events,
            routed,
            ..Frame::default()
        }
    }
}
