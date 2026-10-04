//! Runtime-owned shop transactions.
//!
//! The Godot shop window owns selection and presentation. This module owns
//! the irreversible part: every purse and inventory change goes through the
//! [`GameState`] APIs held by [`Runtime`].
//!
//! An inn stay is two halves, because the cartridge's Aiedo counter hands the
//! party to `Event_GirlsSneakingOut` between them: [`inn_begin`] tests the bill
//! and runs `RecoverStats`, and [`inn_charge`] deducts it after the scene has
//! returned (`ps4.asm:136367-136414`; `docs/camp/SHOPS.md`, "Finding 3").

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

/// The inn's opening half.
///
/// Retail tests the bill against the purse and runs `RecoverStats` *before* the
/// Aiedo special case, and charges the bill only after it
/// (`ps4.asm:136367-136414`). Splitting the transaction at that seam is what
/// lets a rest hand the party to a scene with the recovery already done and the
/// bill still open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InnOpening {
    /// The bill is affordable and every occupied slot has been recovered.
    Resting {
        /// `rate * party_slots`.
        cost: u32,
        /// Occupied party slots billed and recovered.
        party_slots: usize,
    },
    /// The purse cannot cover the unchanged bill; nothing was recovered.
    InsufficientFunds {
        /// `rate * party_slots`.
        cost: u32,
        /// Money still held.
        money: u32,
    },
}

/// `EventFlag_Zio` (`ps4.constants.asm:1539`): set when the party first fights
/// Zio.
const EVENT_FLAG_ZIO: u16 = 0x42;
/// `EventFlag_GirlsCaught` (`ps4.constants.asm:1543`): set by the Aiedo rest
/// event itself (`docs/scenes/27_GirlsSneakingOut.md`).
const EVENT_FLAG_GIRLS_CAUGHT: u16 = 0x46;
/// Group 0's Aiedo counter: `$FFFFECD1` selector `6` (`$066148`).
const AIEDO_INN_SELECTOR: usize = 6;
/// `Event_GirlsSneakingOut`, EventPtrs[`$23`] (`ps4.asm:146949`).
const EVENT_GIRLS_SNEAKING_OUT: u16 = 0x23;

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

/// The inn's opening half: the affordability test and the modeled part of
/// `RecoverStats`, in the cartridge's order (`ps4.asm:136371-136388`).
pub fn inn_begin(game: &mut GameState, rate: u32) -> InnOpening {
    let party_slots = game.party_len();
    let cost = rate.saturating_mul(party_slots as u32);
    if game.money() < cost {
        return InnOpening::InsufficientFunds {
            cost,
            money: game.money(),
        };
    }
    game.recover_stats();
    InnOpening::Resting { cost, party_slots }
}

/// Charges a bill an [`inn_begin`] priced (`sub.l d0, (Current_Money).w`,
/// `ps4.asm:136414`) and returns the purse.
pub fn inn_charge(game: &mut GameState, cost: u32) -> u32 {
    game.set_money(game.money().saturating_sub(cost));
    game.money()
}

impl Runtime {
    /// Runs one shop purchase against the persistent game state.
    pub(crate) fn shop_buy(&mut self, item: u8, price: u32) -> ShopBuyResult {
        buy(&mut self.game, item, price)
    }

    /// Runs one shop sale against the persistent game state.
    pub(crate) fn shop_sell(&mut self, slot: usize, buy_price: u32) -> ShopSellResult {
        sell(&mut self.game, slot, buy_price)
    }

    /// The inn's opening half against the persistent game state.
    pub(crate) fn inn_begin(&mut self, rate: u32) -> InnOpening {
        inn_begin(&mut self.game, rate)
    }

    /// Charges the purse for a bill [`Runtime::inn_begin`] priced.
    pub(crate) fn inn_charge(&mut self, cost: u32) -> u32 {
        inn_charge(&mut self.game, cost)
    }

    /// The scene a rest at `selector` runs instead of an ordinary night, if
    /// any: `$066148` tests the inn selector for group 0's Aiedo counter (`6`)
    /// and `EventFlag_Zio` and `EventFlag_GirlsCaught` for clear, and then calls
    /// `Event_GirlsSneakingOut` with the bill, selector and text variant saved
    /// and restored around it (`ps4.asm:136391-136410`).
    #[must_use]
    pub(crate) fn rest_event(&self, selector: usize) -> Option<u16> {
        (selector == AIEDO_INN_SELECTOR
            && self.game.is_clear(Flag::event(EVENT_FLAG_ZIO))
            && self.game.is_clear(Flag::event(EVENT_FLAG_GIRLS_CAUGHT)))
        .then_some(EVENT_GIRLS_SNEAKING_OUT)
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
            inn_begin(&mut game, 5),
            InnOpening::Resting {
                cost: 5,
                party_slots: 1
            }
        );
        assert_eq!(game.money(), 10, "the opening half does not charge");
        assert_eq!(inn_charge(&mut game, 5), 5);
        assert_eq!(game.money(), 5);
    }

    /// The opening half prices the bill and recovers the party; the bill is
    /// charged only by [`inn_charge`] (`ps4.asm:136367-136414`). The pad-only
    /// Aiedo test in `tests/session_menu_scenes.rs` proves the same ordering
    /// with a real party: restored before the scene, charged after it.
    #[test]
    fn the_bill_stays_open_between_the_halves() {
        let mut game = GameState::new();
        game.set_money(5);
        game.set_party_slot(0, Some(CharId(0))).unwrap();
        assert_eq!(
            inn_begin(&mut game, 5),
            InnOpening::Resting {
                cost: 5,
                party_slots: 1
            }
        );
        assert_eq!(game.money(), 5, "the bill is charged after the rest");
        assert_eq!(inn_charge(&mut game, 5), 0);
    }

    /// The negative control: a bill the purse cannot cover changes nothing —
    /// not the purse, not the party.
    #[test]
    fn a_bill_the_purse_cannot_cover_recovers_nothing() {
        let mut game = GameState::new();
        game.set_money(4);
        game.set_party_slot(0, Some(CharId(0))).unwrap();
        let before = game.snapshot();
        assert_eq!(
            inn_begin(&mut game, 5),
            InnOpening::InsufficientFunds { cost: 5, money: 4 }
        );
        assert_eq!(game.snapshot(), before);
    }
}
