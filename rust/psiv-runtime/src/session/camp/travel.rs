//! Teleport selection, paid message and ordinary map-change delivery.

use crate::{CampTravelMenu, Runtime};

use super::{CampPage, CampView, MenuInput, wrap};

impl CampView {
    pub(super) fn begin_selected_travel(&mut self, runtime: &mut Runtime) {
        self.travel_item_slot = None;
        let Some(caster) = self.snapshot.party.get(self.ability_character_selection) else {
            return;
        };
        let Some(ability) = self.ability_options.get(self.ability_selection) else {
            return;
        };
        let result = runtime.begin_camp_travel(caster.party_slot, ability.id);
        self.show_travel_result(result);
    }

    pub(super) fn begin_selected_item_travel(&mut self, runtime: &mut Runtime) {
        let Some(item) = self.snapshot.inventory.get(self.item_selection) else {
            return;
        };
        self.travel_item_slot = Some(item.slot);
        let result = runtime.begin_camp_item_travel(item.slot);
        self.show_travel_result(result);
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
        self.show_travel_result(result);
    }

    fn show_travel_result(&mut self, result: Result<CampTravelMenu, String>) {
        match result {
            Ok(CampTravelMenu::Towns(towns)) => {
                self.travel_towns = towns;
                self.travel_selection = 0;
                self.page = CampPage::TravelTowns;
            }
            Ok(CampTravelMenu::Ready { .. }) => {
                self.message = TRAVEL_READY_MESSAGE.to_owned();
                // Pipes only play the teleport sound when the message is
                // acknowledged (Win_ItemTeleportMsg), never the TECH cast.
                if self.travel_item_slot.is_none() {
                    self.sound_request = Some(0xBD);
                }
                self.page = CampPage::TravelReady;
            }
            Err(reason) => {
                self.message = retail_travel_refusal(reason);
                self.page = self.travel_failure_page();
            }
        }
    }

    pub(super) fn complete_travel(&mut self, runtime: &mut Runtime) {
        match runtime.complete_camp_travel() {
            Ok(events) => {
                self.runtime_events.extend(events);
                self.sound_request = Some(0xDE);
                self.close();
            }
            Err(error) => {
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

/// The three lines `Win_ItemTeleportMsg` and `Win_TechTeleportMsg` choose
/// from by `$FFFFE284` (`loc_5CB66`, `loc_60B26`): `loc_2AA896`,
/// `loc_2AA8BC` and `loc_2AA8D2`. `\n` is the cartridge's `$FC`.
const TRAVEL_READY_MESSAGE: &str = "Somehow the body\nhas become lighter.";
const TRAVEL_CANNOT_MESSAGE: &str = "Cannot be used here!";
const TRAVEL_TP_MESSAGE: &str = "Not enough TP!";

/// The cartridge's line for a refused teleport: the place forbids it, or the
/// caster lacks the TP. Any other refusal is the port's own and keeps its
/// wording.
fn retail_travel_refusal(reason: String) -> String {
    match reason.as_str() {
        crate::travel::CANNOT_TELEPORT_HERE => TRAVEL_CANNOT_MESSAGE.to_owned(),
        crate::travel::NOT_ENOUGH_TP => TRAVEL_TP_MESSAGE.to_owned(),
        _ => reason,
    }
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
