//! Shop input routing: one frame of cursor and confirm input per mode, and the
//! transaction each confirm fires.
//!
//! The window owns cursors and messages; every mutation goes to the runtime.

use godot::classes::Input;
use godot::global::Key;
use godot::prelude::*;

use psiv_runtime::{InnResult, Runtime, ShopBuyResult, ShopSellResult};

use super::{Mode, ShopCounter, ShopWindow};

impl ShopWindow {
    pub(crate) fn handle_input(&mut self, runtime: &mut Runtime) {
        if self.cancel_requested() {
            if self.go_back() {
                self.close();
                return;
            }
            self.snapshot = self.snapshot(runtime);
            self.base_mut().queue_redraw();
            return;
        }
        let input = Input::singleton();
        if self.accept_blocked {
            if !input.is_action_pressed("ui_accept") {
                self.accept_blocked = false;
            }
            self.snapshot = self.snapshot(runtime);
            return;
        }
        let up = input.is_action_just_pressed("ui_up");
        let down = input.is_action_just_pressed("ui_down");
        let accept = input.is_action_just_pressed("ui_accept");
        match self.mode {
            Mode::Greeting => {
                if accept {
                    self.mode = Mode::Root;
                }
            }
            Mode::InnGreeting => {
                if accept {
                    self.mode = Mode::InnConfirm;
                    self.confirm_selection = 0;
                }
            }
            Mode::Root => {
                if up {
                    self.root_selection = wrap(self.root_selection, 2, false);
                } else if down {
                    self.root_selection = wrap(self.root_selection, 2, true);
                } else if accept {
                    if self.root_selection == 0 {
                        self.mode = Mode::BuyList;
                    } else if self.snapshot.items.is_empty() {
                        self.message = "You don't have anything to sell.".to_owned();
                        self.mode = Mode::Message;
                    } else {
                        self.item_selection = 0;
                        self.mode = Mode::SellList;
                    }
                }
            }
            Mode::BuyList => {
                let count = self.buy_items().len();
                if up {
                    self.item_selection = wrap(self.item_selection, count, false);
                } else if down {
                    self.item_selection = wrap(self.item_selection, count, true);
                } else if accept && count > 0 {
                    self.confirm_selection = 0;
                    self.mode = Mode::BuyConfirm;
                }
            }
            Mode::BuyConfirm => {
                if up || down {
                    self.confirm_selection = 1usize.saturating_sub(self.confirm_selection);
                } else if accept {
                    if self.confirm_selection == 0 {
                        self.confirm_buy(runtime);
                    } else {
                        self.mode = Mode::BuyList;
                    }
                }
            }
            Mode::SellList => {
                let count = self.snapshot.items.len();
                if up {
                    self.item_selection = wrap(self.item_selection, count, false);
                } else if down {
                    self.item_selection = wrap(self.item_selection, count, true);
                } else if accept && count > 0 {
                    self.confirm_selection = 0;
                    self.mode = Mode::SellConfirm;
                }
            }
            Mode::SellConfirm => {
                if up || down {
                    self.confirm_selection = 1usize.saturating_sub(self.confirm_selection);
                } else if accept {
                    if self.confirm_selection == 0 {
                        self.confirm_sell(runtime);
                    } else {
                        self.mode = Mode::SellList;
                    }
                }
            }
            Mode::InnConfirm => {
                if up || down {
                    self.confirm_selection = 1usize.saturating_sub(self.confirm_selection);
                } else if accept {
                    if self.confirm_selection == 0 {
                        self.confirm_inn(runtime);
                    } else {
                        self.mode = Mode::InnGreeting;
                    }
                }
            }
            Mode::Message => {
                if accept {
                    self.mode = if self.counter.as_ref().is_some_and(ShopCounter::is_inn) {
                        Mode::InnGreeting
                    } else {
                        Mode::Root
                    };
                }
            }
            Mode::Closed => {}
        }
        self.snapshot = self.snapshot(runtime);
        self.base_mut().queue_redraw();
    }

    fn cancel_requested(&mut self) -> bool {
        let input = Input::singleton();
        let x = input.is_physical_key_pressed(Key::X);
        let edge = x && !self.cancel_down;
        self.cancel_down = x;
        input.is_action_just_pressed("ui_cancel") || edge
    }

    fn go_back(&mut self) -> bool {
        match self.mode {
            Mode::Closed | Mode::Greeting | Mode::InnGreeting | Mode::Root => true,
            Mode::BuyList | Mode::SellList => {
                self.mode = Mode::Root;
                false
            }
            Mode::BuyConfirm => {
                self.mode = Mode::BuyList;
                false
            }
            Mode::SellConfirm => {
                self.mode = Mode::SellList;
                false
            }
            Mode::InnConfirm => {
                self.mode = Mode::InnGreeting;
                false
            }
            Mode::Message => {
                self.mode = if self.counter.as_ref().is_some_and(ShopCounter::is_inn) {
                    Mode::InnGreeting
                } else {
                    Mode::Root
                };
                false
            }
        }
    }

    fn confirm_buy(&mut self, runtime: &mut Runtime) {
        let Some(item) = self.buy_items().get(self.item_selection).cloned() else {
            self.mode = Mode::BuyList;
            return;
        };
        self.message = match runtime.shop_buy(item.item_id, item.buy_price) {
            ShopBuyResult::Bought { .. } => "Thank you very much.\nAnything else?".to_owned(),
            ShopBuyResult::InventoryFull { .. } => "You can't carry anything else.".to_owned(),
            ShopBuyResult::InsufficientFunds { .. } => "You don't have enough money!".to_owned(),
        };
        self.mode = Mode::Message;
    }

    fn confirm_sell(&mut self, runtime: &mut Runtime) {
        let Some(item) = self.snapshot.items.get(self.item_selection) else {
            self.mode = Mode::SellList;
            return;
        };
        let buy_price = self
            .catalog
            .as_ref()
            .and_then(|catalog| {
                catalog
                    .inventories
                    .iter()
                    .flat_map(|i| &i.items)
                    .find(|candidate| candidate.item_id == item.id)
            })
            .map_or(0, |candidate| candidate.buy_price);
        self.message = match runtime.shop_sell(item.slot, buy_price) {
            ShopSellResult::Sold { .. } => "Thank you very much.\nAnything else?".to_owned(),
            ShopSellResult::EmptySlot => "That item is no longer there.".to_owned(),
        };
        self.mode = Mode::Message;
    }

    fn confirm_inn(&mut self, runtime: &mut Runtime) {
        let Some(counter) = self.counter.as_ref() else {
            return;
        };
        let Some(index) = counter.inn_index else {
            self.mode = Mode::InnGreeting;
            return;
        };
        let Some(rate) = self
            .catalog
            .as_ref()
            .and_then(|catalog| catalog.inn(index))
            .map(|inn| inn.rate_per_character)
        else {
            self.mode = Mode::InnGreeting;
            return;
        };
        self.message = match runtime.shop_stay(rate, index) {
            InnResult::Stayed { .. } => "Thank you very much.\nPlease come again.".to_owned(),
            InnResult::InsufficientFunds { .. } => "You don't have enough money!".to_owned(),
            InnResult::AiedoEventPending { .. } => "Aiedo rest event pending.".to_owned(),
        };
        self.mode = Mode::Message;
    }
}

fn wrap(current: usize, count: usize, forward: bool) -> usize {
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
