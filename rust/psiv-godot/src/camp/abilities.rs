//! Camp TECH/SKILL browse and confirmation. Runtime owns every state change.
use super::{
    CHILD_CURSOR_PATTERN, CampChrome, CampMenu, DrawList, ITEM_LIST, ITEM_TARGET, Mode, draw_text,
    frame, wrap,
};
use psiv_runtime::{CampAbilityKind, CampUseResult, Runtime};

impl CampMenu {
    pub(super) fn handle_ability_input(
        &mut self,
        runtime: &mut Runtime,
        up: bool,
        down: bool,
        accept: bool,
    ) {
        let (selection, count) = match self.mode {
            Mode::AbilityCharacters => (
                &mut self.ability_character_selection,
                self.snapshot.party.len(),
            ),
            Mode::AbilityList => (&mut self.ability_selection, self.ability_options.len()),
            Mode::AbilityTarget => (&mut self.target_selection, self.snapshot.party.len()),
            Mode::TravelTowns => (&mut self.travel_selection, self.travel_towns.len()),
            Mode::AbilityResult => {
                if accept {
                    self.mode = Mode::AbilityList;
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
        match self.mode {
            Mode::AbilityCharacters => {
                self.ability_selection = 0;
                self.mode = Mode::AbilityList;
            }
            Mode::AbilityList => {
                let ability = &self.ability_options[self.ability_selection];
                if self.ability_kind == CampAbilityKind::Technique
                    && matches!(ability.id, psiv_runtime::RYUKA | psiv_runtime::HINAS)
                {
                    self.begin_selected_travel(runtime);
                } else if !ability.supported || ability.remaining < u16::from(ability.cost) {
                    // Runtime gives the authoritative rejection without payment.
                    self.use_selected_ability(runtime);
                } else if ability.needs_target() && self.snapshot.party.len() > 1 {
                    self.target_selection = 0;
                    self.mode = Mode::AbilityTarget;
                } else {
                    self.target_selection = self.ability_character_selection;
                    self.use_selected_ability(runtime);
                }
            }
            Mode::AbilityTarget => self.use_selected_ability(runtime),
            Mode::TravelTowns => self.select_travel_town(runtime),
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
        self.mode = Mode::AbilityResult;
    }

    pub(super) fn draw_abilities(&self, chrome: &CampChrome, list: &mut DrawList) {
        self.draw_root(chrome, list, false);
        frame(chrome, &mut list.quads, ITEM_LIST);
        let kind = if self.ability_kind == CampAbilityKind::Technique {
            "TECH"
        } else {
            "SKILL"
        };
        if self.mode == Mode::AbilityCharacters {
            draw_text(chrome, &mut list.quads, &format!("{kind}: WHO?"), (16, 3));
            for (row, c) in self.snapshot.party.iter().enumerate() {
                draw_text(
                    chrome,
                    &mut list.quads,
                    &format!("{}  TP {}/{}", c.name, c.current_tp, c.max_tp),
                    (16, 6 + row as i32 * 2),
                );
            }
            cursor(
                chrome,
                list,
                (15, 6 + self.ability_character_selection as i32 * 2),
            );
            return;
        }
        let Some(caster) = self.snapshot.party.get(self.ability_character_selection) else {
            return;
        };
        draw_text(
            chrome,
            &mut list.quads,
            &format!("{} {kind}", caster.name),
            (16, 3),
        );
        if self.ability_kind == CampAbilityKind::Technique {
            draw_text(
                chrome,
                &mut list.quads,
                &format!("TP {}/{}", caster.current_tp, caster.max_tp),
                (16, 4),
            );
        }
        if self.ability_options.is_empty() {
            draw_text(chrome, &mut list.quads, "NONE LEARNED", (16, 7));
        } else {
            let first = self.ability_selection / 8 * 8;
            for (row, ability) in self.ability_options.iter().skip(first).take(8).enumerate() {
                let resource = if self.ability_kind == CampAbilityKind::Technique {
                    format!("{} TP", ability.cost)
                } else {
                    format!("{} LEFT", ability.remaining)
                };
                draw_text(
                    chrome,
                    &mut list.quads,
                    &ability.name,
                    (16, 6 + row as i32 * 2),
                );
                draw_text(chrome, &mut list.quads, &resource, (28, 6 + row as i32 * 2));
            }
            cursor(
                chrome,
                list,
                (15, 6 + (self.ability_selection - first) as i32 * 2),
            );
        }
        if self.mode == Mode::AbilityTarget {
            frame(chrome, &mut list.quads, ITEM_TARGET);
            for (row, c) in self.snapshot.party.iter().enumerate() {
                draw_text(
                    chrome,
                    &mut list.quads,
                    &format!("{} {}/{}", c.name, c.current_hp, c.max_hp),
                    (20, 7 + row as i32),
                );
            }
            cursor(chrome, list, (19, 7 + self.target_selection as i32));
        }
        self.draw_travel_overlay(chrome, list);
    }

    pub(super) fn draw_travel_overlay(&self, chrome: &CampChrome, list: &mut DrawList) {
        if self.mode == Mode::TravelTowns {
            frame(
                chrome,
                &mut list.quads,
                super::layout::CellRect::new(18, 6, 18, 12),
            );
            let first = self.travel_selection / 5 * 5;
            for (row, town) in self.travel_towns.iter().skip(first).take(5).enumerate() {
                draw_text(
                    chrome,
                    &mut list.quads,
                    &town.name,
                    (21, 7 + row as i32 * 2),
                );
            }
            cursor(
                chrome,
                list,
                (19, 7 + (self.travel_selection - first) as i32 * 2),
            );
        }
        if matches!(self.mode, Mode::AbilityResult | Mode::TravelReady) {
            frame(chrome, &mut list.quads, super::layout::ITEM_MESSAGE);
            // The retail font is fixed width. Wrap long resource/cure messages
            // within this 24-cell window instead of drawing across its border.
            let mut line = String::new();
            let mut y = 22;
            for word in self.message.split_whitespace() {
                if !line.is_empty() && line.len() + 1 + word.len() > 23 {
                    draw_text(chrome, &mut list.quads, &line, (8, y));
                    line.clear();
                    y += 1;
                }
                if !line.is_empty() {
                    line.push(' ');
                }
                line.push_str(word);
            }
            draw_text(chrome, &mut list.quads, &line, (8, y));
        }
    }
}

fn cursor(chrome: &CampChrome, list: &mut DrawList, at: (i32, i32)) {
    if let Some(quad) = chrome.window_word(CHILD_CURSOR_PATTERN, at) {
        list.quads.push(quad);
    }
}
