//! Shops and inns: the `buy`, `sell` and `rest_inn` controllers.
//!
//! Each one faces the counter, presses Speak, and walks the window through the
//! session's [`ShopView`]: the page it is on, the rows it lists, the cursors it
//! reports. Prices, stock, the bill and what a sale pays are the runtime's; the
//! controller only reads the message the transaction answered with and halts
//! when it is not the thank-you the cartridge gives.

use psiv_core::Direction;
use psiv_runtime::{Button, ShopPage, ShopView};

use crate::driver::Driver;
use crate::halt::{Halt, HaltKind, Res};
use crate::menu::{find_named, listing};
use crate::route::NameOrId;

impl Driver {
    fn shop(&self) -> Res<&ShopView> {
        self.session()
            .shop_view()
            .ok_or_else(|| Halt::new(HaltKind::UnexpectedState, "the shop window is not open"))
    }

    fn shop_page(&self) -> Res<ShopPage> {
        Ok(self.shop()?.page)
    }

    fn expect_page(&self, wanted: ShopPage) -> Res {
        let page = self.shop_page()?;
        if page == wanted {
            Ok(())
        } else {
            Err(Halt::new(
                HaltKind::UnexpectedState,
                format!("the shop is on {page:?}, expected {wanted:?}"),
            ))
        }
    }

    /// A transaction's message page must be the cartridge's thank-you.
    fn expect_thanks(&self) -> Res {
        self.expect_page(ShopPage::Message)?;
        let message = self.shop()?.message.clone();
        if message.starts_with("Thank you") {
            Ok(())
        } else {
            Err(Halt::new(
                HaltKind::UnexpectedState,
                format!("the counter refused: {message:?}"),
            ))
        }
    }

    /// Backs out of the window by Cancel until it closes.
    fn leave_shop(&mut self) -> Res {
        for _ in 0..8 {
            if self.session().shop_view().is_none() {
                return Ok(());
            }
            self.tap(Button::Cancel)?;
        }
        Err(Halt::new(
            HaltKind::Stuck,
            "the shop window would not close",
        ))
    }

    /// Greeting to the BUY/SELL menu with the cursor on `row` (0 BUY, 1 SELL).
    fn open_trade(&mut self, row: usize) -> Res {
        self.expect_page(ShopPage::Greeting)?;
        self.tap(Button::Speak)?;
        self.expect_page(ShopPage::Root)?;
        self.root_row(row)
    }

    fn root_row(&mut self, row: usize) -> Res {
        self.cursor_to(
            "shop",
            |d| d.session().shop_view().map(|v| v.root_selection),
            row,
            Some(2),
        )?;
        self.tap(Button::Speak)
    }

    /// Buys `count` of `item` at the counter the party faces.
    ///
    /// # Errors
    ///
    /// [`HaltKind::MenuEntryMissing`] when the counter does not stock the
    /// item, [`HaltKind::UnexpectedState`] when it refuses the sale.
    pub fn buy(&mut self, item: &NameOrId, count: u16, face: Option<Direction>) -> Res {
        self.open_counter(face)?;
        self.open_trade(0)?;
        for n in 0..count {
            if n > 0 {
                self.expect_page(ShopPage::Root)?;
                self.root_row(0)?;
            }
            self.expect_page(ShopPage::BuyList)?;
            let row = {
                let stock = &self.shop()?.stock;
                find_named(stock, item, |s| s.name.as_str(), |s| u32::from(s.item_id)).ok_or_else(
                    || {
                        Halt::new(
                            HaltKind::MenuEntryMissing,
                            format!(
                                "the counter does not stock {item}; it sells: {}",
                                listing(stock, |s| s.name.as_str())
                            ),
                        )
                    },
                )?
            };
            let rows = self.shop()?.stock.len();
            self.cursor_to(
                "buy list",
                |d| d.session().shop_view().map(|v| v.item_selection),
                row,
                Some(rows),
            )?;
            self.tap(Button::Speak)?;
            self.expect_page(ShopPage::BuyConfirm)?;
            // The confirmation opens on YES.
            self.tap(Button::Speak)?;
            self.expect_thanks()?;
            self.tap(Button::Speak)?;
        }
        self.leave_shop()
    }

    /// Sells one `item` at the counter the party faces.
    ///
    /// # Errors
    ///
    /// [`HaltKind::MenuEntryMissing`] when the party holds no such item.
    pub fn sell(&mut self, item: &NameOrId, face: Option<Direction>) -> Res {
        self.open_counter(face)?;
        self.open_trade(1)?;
        self.expect_page(ShopPage::SellList)?;
        let row = {
            let items = &self.shop()?.items;
            find_named(items, item, |s| s.name.as_str(), |s| u32::from(s.id)).ok_or_else(|| {
                Halt::new(
                    HaltKind::MenuEntryMissing,
                    format!(
                        "the party holds no {item}; it holds: {}",
                        listing(items, |s| s.name.as_str())
                    ),
                )
            })?
        };
        let rows = self.shop()?.items.len();
        self.cursor_to(
            "sell list",
            |d| d.session().shop_view().map(|v| v.item_selection),
            row,
            Some(rows),
        )?;
        self.tap(Button::Speak)?;
        self.expect_page(ShopPage::SellConfirm)?;
        self.tap(Button::Speak)?;
        self.expect_thanks()?;
        self.tap(Button::Speak)?;
        self.leave_shop()
    }

    /// Pays for a night at the inn the party faces.
    ///
    /// # Errors
    ///
    /// [`HaltKind::UnexpectedState`] when the innkeeper refuses, or when the
    /// night starts the Aiedo rest event the port does not run mid-transaction.
    pub fn rest_inn(&mut self, face: Option<Direction>) -> Res {
        self.open_counter(face)?;
        self.expect_page(ShopPage::InnGreeting)?;
        self.tap(Button::Speak)?;
        self.expect_page(ShopPage::InnConfirm)?;
        self.tap(Button::Speak)?;
        self.expect_thanks()?;
        self.tap(Button::Speak)?;
        self.leave_shop()
    }
}
