//! Runtime-owned shop transactions.
//!
//! The Godot shop window owns selection and presentation. This module owns
//! the irreversible part: every purse and inventory change goes through the
//! [`GameState`] APIs held by [`Runtime`].

use psiv_core::{Flag, GameState};

use crate::Runtime;

/// The result of attempting to buy one item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShopBuyResult {
    /// The item was added to the first free slot and the price was paid.
    Bought {
        /// The item id written to the inventory.
        item: u8,
        /// The inventory slot used.
        slot: usize,
        /// The amount paid.
        price: u32,
    },
    /// Retail checks the forty occupied slots before checking money.
    InventoryFull {
        /// The requested item.
        item: u8,
    },
    /// The purse could not cover the unchanged buy price.
    InsufficientFunds {
        /// The requested item.
        item: u8,
        /// The unchanged price.
        price: u32,
        /// Money still held.
        money: u32,
    },
}

/// The result of attempting to sell one inventory slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShopSellResult {
    /// The item was removed and its buy price was shifted right once.
    Sold {
        /// The item removed.
        item: u8,
        /// The exact floor-half amount paid.
        price: u32,
    },
    /// No item occupied the requested slot.
    EmptySlot,
}

/// The result of asking an inn for a night.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InnResult {
    /// The party paid and the modeled character fields were recovered.
    Stayed {
        /// `rate * party_slots`.
        cost: u32,
        /// Occupied party slots billed and recovered.
        party_slots: usize,
    },
    /// The party cannot afford the bill; state is unchanged.
    InsufficientFunds {
        /// `rate * party_slots`.
        cost: u32,
        /// Money still held.
        money: u32,
    },
    /// Aiedo selector 6 enters the scripted event path instead of an
    /// ordinary night. The event runner is not owned by the shop lane yet.
    AiedoEventPending {
        /// The bill the event must settle.
        cost: u32,
        /// Slots recovered before the event handoff, matching retail's
        /// `RecoverStats` ordering.
        party_slots: usize,
    },
}

/// Applies a retail buy to a game state.
pub fn buy(game: &mut GameState, item: u8, price: u32) -> ShopBuyResult {
    if game.inventory().is_full() {
        return ShopBuyResult::InventoryFull { item };
    }
    if game.money() < price {
        return ShopBuyResult::InsufficientFunds {
            item,
            price,
            money: game.money(),
        };
    }
    let Ok(slot) = game.inventory_mut().add(item) else {
        // The count check above and `Inventory::add` share the same forty-slot
        // source of truth. Keep the defensive branch explicit if that API
        // ever gains another refusal reason.
        return ShopBuyResult::InventoryFull { item };
    };
    game.set_money(game.money() - price);
    ShopBuyResult::Bought { item, slot, price }
}

/// Applies a retail sell. The input is the item's buy-price word; retail
/// computes the sell price with `lsr.w #1`, so odd prices round down.
pub fn sell(game: &mut GameState, slot: usize, buy_price: u32) -> ShopSellResult {
    let Some(item) = game.inventory_mut().remove(slot) else {
        return ShopSellResult::EmptySlot;
    };
    let price = buy_price / 2;
    game.add_money(price);
    ShopSellResult::Sold { item, price }
}

/// Applies an inn bill and the modeled part of `RecoverStats`.
pub fn stay(game: &mut GameState, rate: u32, selector: usize) -> InnResult {
    let party_slots = game.party_len();
    let cost = rate.saturating_mul(party_slots as u32);
    if game.money() < cost {
        return InnResult::InsufficientFunds {
            cost,
            money: game.money(),
        };
    }

    recover_party(game, party_slots);
    if selector == 6 && game.is_clear(Flag::event(0x42)) && game.is_clear(Flag::event(0x46)) {
        return InnResult::AiedoEventPending { cost, party_slots };
    }

    game.set_money(game.money() - cost);
    InnResult::Stayed { cost, party_slots }
}

fn recover_party(game: &mut GameState, party_slots: usize) {
    let party = game.party();
    for id in party.into_iter().take(party_slots).flatten() {
        if let Some(stats) = game.roster_mut().get_mut(id) {
            stats.curr_hp = stats.max_hp;
            stats.curr_tp = stats.max_tp;
            stats.status = 0;
        }
    }
}

impl Runtime {
    /// Runs one shop purchase against the persistent game state.
    pub fn shop_buy(&mut self, item: u8, price: u32) -> ShopBuyResult {
        buy(&mut self.game, item, price)
    }

    /// Runs one shop sale against the persistent game state.
    pub fn shop_sell(&mut self, slot: usize, buy_price: u32) -> ShopSellResult {
        sell(&mut self.game, slot, buy_price)
    }

    /// Runs one inn stay against the persistent game state.
    pub fn shop_stay(&mut self, rate: u32, selector: usize) -> InnResult {
        stay(&mut self.game, rate, selector)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use psiv_core::CharId;

    #[test]
    fn buy_checks_capacity_then_money_and_writes_through_state() {
        let mut game = GameState::new();
        game.set_money(20);
        assert_eq!(
            buy(&mut game, 125, 20),
            ShopBuyResult::Bought {
                item: 125,
                slot: 0,
                price: 20
            }
        );
        assert_eq!(game.money(), 0);
        assert_eq!(game.inventory().get(0), Some(125));
        assert_eq!(
            buy(&mut game, 1, 1),
            ShopBuyResult::InsufficientFunds {
                item: 1,
                price: 1,
                money: 0
            }
        );
    }

    #[test]
    fn full_inventory_refusal_wins_over_affordability() {
        let mut game = GameState::new();
        for slot in 0..40 {
            game.inventory_mut().add(slot as u8 + 1).unwrap();
        }
        game.set_money(999);
        assert_eq!(
            buy(&mut game, 125, 1),
            ShopBuyResult::InventoryFull { item: 125 }
        );
    }

    #[test]
    fn sell_is_floor_half_and_empty_slots_do_not_pay() {
        let mut game = GameState::new();
        game.inventory_mut().add(125).unwrap();
        assert_eq!(
            sell(&mut game, 0, 11),
            ShopSellResult::Sold {
                item: 125,
                price: 5
            }
        );
        assert_eq!(game.money(), 5);
        assert_eq!(sell(&mut game, 0, 20), ShopSellResult::EmptySlot);
    }

    #[test]
    fn inn_bills_occupied_party_slots_and_recovers_modeled_stats() {
        let mut game = GameState::new();
        game.set_money(10);
        game.set_party_slot(0, Some(CharId(0))).unwrap();
        assert_eq!(
            stay(&mut game, 5, 0),
            InnResult::Stayed {
                cost: 5,
                party_slots: 1
            }
        );
        assert_eq!(game.money(), 5);
    }

    #[test]
    fn aiedo_special_is_flagged_before_the_bill_is_deducted() {
        let mut game = GameState::new();
        game.set_money(5);
        game.set_party_slot(0, Some(CharId(0))).unwrap();
        assert_eq!(
            stay(&mut game, 5, 6),
            InnResult::AiedoEventPending {
                cost: 5,
                party_slots: 1
            }
        );
        assert_eq!(game.money(), 5);
    }
}
