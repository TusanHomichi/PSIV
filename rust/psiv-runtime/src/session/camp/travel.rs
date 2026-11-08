//! Teleport selection, paid message and ordinary map-change delivery.

use psiv_data::TravelMessages;

use crate::{CampTravelMenu, Runtime};

use super::teardown::{self, WindowRecord, WindowTeardown};
use super::{CampPage, CampView, MenuInput, wrap};

/// The first town-list record for a TELEPIPE (`ps4.asm:140344`) and for
/// RYUKA (`ps4.asm:140488`).
const TELEPIPE_TOWNS: u8 = 0x36;
const RYUKA_TOWNS: u8 = 0x66;

impl CampView {
    pub(super) fn begin_selected_travel(&mut self, runtime: &mut Runtime) {
        self.travel_item_slot = None;
        let Some(caster) = self.snapshot.party.get(self.ability_character_selection) else {
            return;
        };
        let Some(ability) = self.ability_options.get(self.ability_selection) else {
            return;
        };
        // Root, stats and meseta (`Win_MenuOptions`, `ps4.asm:117201-117247`),
        // the caster list when there is more than one member
        // (`MenuOptions_Tech`, `ps4.asm:117360-117368`), and the technique
        // list's pages up to the chosen one.
        let members = self.snapshot.party.len();
        let mut windows = self.root_windows();
        if members > 1 {
            windows.push(teardown::tech_casters(members));
        }
        let count = self.ability_options.len();
        windows
            .extend((0..=self.ability_selection / 8).map(|page| teardown::tech_list(count, page)));
        self.travel_windows = windows;
        self.travel_towns.clear();
        let result = runtime.begin_camp_travel(caster.party_slot, ability.id);
        self.show_travel_result(runtime, result);
    }

    pub(super) fn begin_selected_item_travel(&mut self, runtime: &mut Runtime) {
        let Some(item) = self.snapshot.inventory.get(self.item_selection) else {
            return;
        };
        self.travel_item_slot = Some(item.slot);
        // Root, stats and meseta, the pack's pages up to the pipe's
        // (`MenuOptions_Item`, `ps4.asm:117321-117349`), and USE/LOOK/DSCD.
        let count = self.snapshot.inventory.len();
        let mut windows = self.root_windows();
        windows.extend((0..=self.item_selection / 8).map(|page| teardown::item_list(count, page)));
        windows.push(teardown::ITEM_ACTION);
        self.travel_windows = windows;
        // A town list from an earlier pipe or cast is not this one's.
        self.travel_towns.clear();
        let result = runtime.begin_camp_item_travel(item.slot);
        self.show_travel_result(runtime, result);
    }

    pub(super) fn select_travel_town(&mut self, runtime: &mut Runtime) {
        let Some(town) = self.travel_towns.get(self.travel_selection) else {
            return;
        };
        let result = if let Some(slot) = self.travel_item_slot {
            runtime.select_camp_item_town(slot, town.index)
        } else {
            let Some(caster) = self.snapshot.party.get(self.ability_character_selection) else {
                return;
            };
            runtime.select_camp_town(caster.party_slot, town.index)
        };
        self.show_travel_result(runtime, result);
    }

    /// The town list, or the window line the pack's `travel.json` holds for
    /// the outcome (`TravelMessages`).
    fn show_travel_result(&mut self, runtime: &Runtime, result: Result<CampTravelMenu, String>) {
        let messages = runtime.data().travel().map(|travel| &travel.messages);
        match result {
            Ok(CampTravelMenu::Towns(towns)) => {
                // A town list comes only from the pack's travel tables, so its
                // label is there too.
                self.travel_towns_label = messages
                    .map(|messages| messages.towns_label.clone())
                    .unwrap_or_default();
                self.travel_towns = towns;
                self.travel_selection = 0;
                self.page = CampPage::TravelTowns;
            }
            Ok(CampTravelMenu::Ready { .. }) => {
                // Every granted teleport reads a destination from the pack's
                // travel tables, so its line is there too.
                self.message = messages
                    .map(|messages| messages.teleported.clone())
                    .unwrap_or_default();
                // Pipes only play the teleport sound when the message is
                // acknowledged (Win_ItemTeleportMsg), never the TECH cast.
                if self.travel_item_slot.is_none() {
                    self.sound_request = Some(0xBD);
                }
                self.page = CampPage::TravelReady;
            }
            Err(reason) => {
                self.message = retail_travel_refusal(messages, reason);
                self.page = self.travel_failure_page();
            }
        }
    }

    /// The windows `Win_MenuOptions` opens with the menu and keeps: the root
    /// options, the party's stats, the meseta, in that order.
    fn root_windows(&self) -> Vec<WindowRecord> {
        vec![
            teardown::ROOT,
            teardown::party_stats(self.snapshot.party.len()),
            teardown::MESETA,
        ]
    }

    /// The acknowledged teleport message: the sound, then `DestroyAllWindows`
    /// over every window on the stack (`teardown.rs`). The teleport itself
    /// starts when the last has closed ([`Self::teardown_frame`]).
    pub(super) fn acknowledge_travel(&mut self) {
        let mut windows = self.travel_windows.clone();
        let count = self.travel_towns.len();
        if count > 0 {
            let first = if self.travel_item_slot.is_some() {
                TELEPIPE_TOWNS
            } else {
                RYUKA_TOWNS
            };
            windows.extend(
                (0..=self.travel_selection / TOWN_PAGE_ROWS)
                    .map(|page| teardown::town_page(first, page, town_page_rows(count, page))),
            );
        }
        windows.push(if self.travel_item_slot.is_some() {
            teardown::ITEM_TELEPORT_MESSAGE
        } else {
            teardown::TECH_TELEPORT_MESSAGE
        });
        self.teardown = Some(WindowTeardown::new(windows));
        self.sound_request = Some(0xDE);
    }

    /// One frame of the windows closing; on the frame the menu exits, the
    /// teleport begins and the camp is gone.
    pub(super) fn teardown_frame(&mut self, runtime: &mut Runtime) {
        let Some(teardown) = self.teardown.as_mut() else {
            return;
        };
        if !teardown.tick() {
            return;
        }
        match runtime.begin_camp_teleport() {
            Ok(events) => {
                self.runtime_events.extend(events);
                self.close();
            }
            Err(error) => {
                self.teardown = None;
                self.message = format!("TRAVEL ERROR: {error}");
                self.page = self.travel_failure_page();
            }
        }
    }

    /// Where a refused travel shows its reason: over the pack for a pipe, over
    /// the ability list for a spell.
    fn travel_failure_page(&self) -> CampPage {
        if self.travel_item_slot.is_some() {
            CampPage::ItemResult
        } else {
            CampPage::AbilityResult
        }
    }

    /// The town list, `Win_TelepipePlaceList` (`ps4.asm:123998`) and its
    /// copy `Win_RyukaPlaceList` (`:129219`). Towns come in pages of
    /// [`TOWN_PAGE_ROWS`], each its own window; the cursor wraps within the
    /// page. Right *held* opens the next page and Left *held* closes the
    /// current one, both with the cursor on the first row
    /// (`ps4.asm:124111-124158`), before Speak or Camp confirms.
    pub(super) fn travel_towns_frame(&mut self, runtime: &mut Runtime, input: MenuInput) {
        let count = self.travel_towns.len();
        if count == 0 {
            return;
        }
        let last_page = (count - 1) / TOWN_PAGE_ROWS;
        let page = self.travel_selection / TOWN_PAGE_ROWS;
        let rows = town_page_rows(count, page);
        if input.up || input.down {
            let row = wrap(self.travel_selection % TOWN_PAGE_ROWS, rows, input.down);
            self.travel_selection = page * TOWN_PAGE_ROWS + row;
        }
        if input.right_held && page < last_page {
            self.sound_request = Some(SFX_MOVING_CURSOR);
            self.travel_selection = (page + 1) * TOWN_PAGE_ROWS;
            return;
        }
        if input.left_held && page > 0 {
            self.sound_request = Some(SFX_MOVING_CURSOR);
            self.travel_selection = (page - 1) * TOWN_PAGE_ROWS;
            return;
        }
        if input.accept {
            self.select_travel_town(runtime);
        }
    }
}

/// The cartridge's line for a refused teleport, from the lines
/// `Win_ItemTeleportMsg` and `Win_TechTeleportMsg` choose from by
/// `$FFFFE284` (`loc_5CB66`, `loc_60B26`): the place forbids it, or the caster
/// lacks the TP. Any other refusal, or one from a pack without travel tables,
/// is the port's own and keeps its wording.
fn retail_travel_refusal(messages: Option<&TravelMessages>, reason: String) -> String {
    let line = messages.and_then(|messages| match reason.as_str() {
        crate::travel::CANNOT_TELEPORT_HERE => Some(&messages.cannot_be_used),
        crate::travel::NOT_ENOUGH_TP => Some(&messages.not_enough_tp),
        _ => None,
    });
    line.cloned().unwrap_or(reason)
}

/// Rows in one town-list window: `$FFFFE270` holds the towns in groups of
/// five, one group per page window.
pub const TOWN_PAGE_ROWS: usize = 5;

/// `SFXID_MovingCursor` (`ps4.constants.asm:1038`).
const SFX_MOVING_CURSOR: u8 = 0xF2;

/// The rows on `page` of a list of `count` towns: five, except the last.
#[must_use]
pub fn town_page_rows(count: usize, page: usize) -> usize {
    count
        .saturating_sub(page * TOWN_PAGE_ROWS)
        .min(TOWN_PAGE_ROWS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refusal_shows_the_packs_line_or_keeps_the_ports_own_wording() {
        let messages = TravelMessages {
            towns_label: "label".into(),
            teleported: "ready".into(),
            cannot_be_used: "place".into(),
            not_enough_tp: "tp".into(),
        };
        for (reason, line) in [
            (crate::travel::CANNOT_TELEPORT_HERE, "place"),
            (crate::travel::NOT_ENOUGH_TP, "tp"),
            ("CASTER IS DOWN", "CASTER IS DOWN"),
        ] {
            assert_eq!(retail_travel_refusal(Some(&messages), reason.into()), line);
            // Without the pack's travel tables nothing is put in its place.
            assert_eq!(retail_travel_refusal(None, reason.into()), reason);
        }
    }
}
