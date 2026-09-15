//! Teleport selection, paid message and ordinary map-change delivery.

use psiv_runtime::{CampTravelMenu, Runtime};

use super::{CampMenu, Mode};

impl CampMenu {
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
                self.mode = Mode::TravelTowns;
            }
            Ok(CampTravelMenu::Ready { name }) => {
                self.message = format!("{name}: READY TO TELEPORT");
                // Pipes only play the teleport sound when the message is
                // acknowledged (Win_ItemTeleportMsg), never the TECH cast.
                if self.travel_item_slot.is_none() {
                    self.sound_request = Some(0xBD);
                }
                self.mode = Mode::TravelReady;
            }
            Err(reason) => {
                self.message = reason;
                self.mode = if self.travel_item_slot.is_some() {
                    Mode::ItemResult
                } else {
                    Mode::AbilityResult
                };
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
                self.mode = if self.travel_item_slot.is_some() {
                    Mode::ItemResult
                } else {
                    Mode::AbilityResult
                };
            }
        }
    }
}
