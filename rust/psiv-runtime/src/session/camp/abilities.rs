//! TECH and SKILL: the caster, the ability, its target. The runtime applies
//! every state change; this page only decides which command to send.

use crate::{CampAbilityKind, CampUseResult, Runtime};

use super::{CampPage, CampView, wrap};

impl CampView {
    pub(super) fn ability_frame(
        &mut self,
        runtime: &mut Runtime,
        up: bool,
        down: bool,
        accept: bool,
    ) {
        let (selection, count) = match self.page {
            CampPage::AbilityCharacters => (
                &mut self.ability_character_selection,
                self.snapshot.party.len(),
            ),
            CampPage::AbilityList => (&mut self.ability_selection, self.ability_options.len()),
            CampPage::AbilityTarget => (&mut self.target_selection, self.snapshot.party.len()),
            CampPage::TravelTowns => (&mut self.travel_selection, self.travel_towns.len()),
            CampPage::AbilityResult => {
                if accept {
                    self.page = CampPage::AbilityList;
                }
                return;
            }
            _ => return,
        };
        if up || down {
            *selection = wrap(*selection, count, down);
            return;
        }
        if !accept || count == 0 {
            return;
        }
        match self.page {
            CampPage::AbilityCharacters => {
                self.ability_selection = 0;
                self.page = CampPage::AbilityList;
            }
            CampPage::AbilityList => {
                let ability = &self.ability_options[self.ability_selection];
                if self.ability_kind == CampAbilityKind::Technique
                    && matches!(ability.id, crate::RYUKA | crate::HINAS)
                {
                    self.begin_selected_travel(runtime);
                } else if !ability.supported || ability.remaining < u16::from(ability.cost) {
                    // The runtime gives the authoritative rejection without
                    // payment.
                    self.use_selected_ability(runtime);
                } else if ability.needs_target() && self.snapshot.party.len() > 1 {
                    self.target_selection = 0;
                    self.page = CampPage::AbilityTarget;
                } else {
                    self.target_selection = self.ability_character_selection;
                    self.use_selected_ability(runtime);
                }
            }
            CampPage::AbilityTarget => self.use_selected_ability(runtime),
            CampPage::TravelTowns => self.select_travel_town(runtime),
            _ => {}
        }
    }

    fn use_selected_ability(&mut self, runtime: &mut Runtime) {
        let Some(ability) = self.ability_options.get(self.ability_selection) else {
            return;
        };
        let Some(caster) = self.snapshot.party.get(self.ability_character_selection) else {
            return;
        };
        let target = self
            .snapshot
            .party
            .get(self.target_selection)
            .map_or(caster.party_slot, |c| c.party_slot);
        let result =
            runtime.use_camp_ability(self.ability_kind, caster.party_slot, ability.id, target);
        if !matches!(result, CampUseResult::Unavailable { .. }) {
            // The cast's sound: the technique chime or the skill's.
            self.sound_request = Some(if self.ability_kind == CampAbilityKind::Technique {
                0xBD
            } else {
                0xCD
            });
        }
        self.message = match result {
            CampUseResult::Used {
                item_name,
                character_name,
                amount,
            } if amount > 0 => format!("{item_name}: {character_name} {amount} HP"),
            CampUseResult::Used {
                item_name,
                character_name,
                ..
            } => format!("{item_name}: {character_name} CURED"),
            CampUseResult::NoEffect { item_name, .. } => format!("{item_name}: NO EFFECT"),
            CampUseResult::Unavailable { reason } => reason,
        };
        self.page = CampPage::AbilityResult;
    }
}
