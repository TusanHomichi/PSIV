//! Camp TECH/SKILL pages: the caster, ability and target lists, and the travel
//! overlay. The session owns every choice; this only draws it.
use super::draw::{draw_text, frame};
use super::layout::CHILD_CURSOR_PATTERN;
use super::{CampChrome, CampMenu, DrawList, ITEM_LIST, ITEM_TARGET};
use psiv_runtime::{CampAbilityKind, CampPage, CampView};

impl CampMenu {
    pub(super) fn draw_abilities(&self, view: &CampView, chrome: &CampChrome, list: &mut DrawList) {
        self.draw_root(view, chrome, list, false);
        frame(chrome, &mut list.quads, ITEM_LIST);
        let kind = if view.ability_kind == CampAbilityKind::Technique {
            "TECH"
        } else {
            "SKILL"
        };
        if view.page == CampPage::AbilityCharacters {
            draw_text(chrome, &mut list.quads, &format!("{kind}: WHO?"), (16, 3));
            for (row, c) in view.snapshot.party.iter().enumerate() {
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
                (15, 6 + view.ability_character_selection as i32 * 2),
            );
            return;
        }
        let Some(caster) = view.snapshot.party.get(view.ability_character_selection) else {
            return;
        };
        draw_text(
            chrome,
            &mut list.quads,
            &format!("{} {kind}", caster.name),
            (16, 3),
        );
        if view.ability_kind == CampAbilityKind::Technique {
            draw_text(
                chrome,
                &mut list.quads,
                &format!("TP {}/{}", caster.current_tp, caster.max_tp),
                (16, 4),
            );
        }
        if view.ability_options.is_empty() {
            draw_text(chrome, &mut list.quads, "NONE LEARNED", (16, 7));
        } else {
            let first = view.ability_selection / 8 * 8;
            for (row, ability) in view.ability_options.iter().skip(first).take(8).enumerate() {
                let resource = if view.ability_kind == CampAbilityKind::Technique {
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
                (15, 6 + (view.ability_selection - first) as i32 * 2),
            );
        }
        if view.page == CampPage::AbilityTarget {
            frame(chrome, &mut list.quads, ITEM_TARGET);
            for (row, c) in view.snapshot.party.iter().enumerate() {
                draw_text(
                    chrome,
                    &mut list.quads,
                    &format!("{} {}/{}", c.name, c.current_hp, c.max_hp),
                    (20, 7 + row as i32),
                );
            }
            cursor(chrome, list, (19, 7 + view.target_selection as i32));
        }
        self.draw_travel_overlay(view, chrome, list);
    }

    pub(super) fn draw_travel_overlay(
        &self,
        view: &CampView,
        chrome: &CampChrome,
        list: &mut DrawList,
    ) {
        if view.page == CampPage::TravelTowns {
            frame(
                chrome,
                &mut list.quads,
                super::layout::CellRect::new(18, 6, 18, 12),
            );
            let first = view.travel_selection / 5 * 5;
            for (row, town) in view.travel_towns.iter().skip(first).take(5).enumerate() {
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
                (19, 7 + (view.travel_selection - first) as i32 * 2),
            );
        }
        if matches!(view.page, CampPage::AbilityResult | CampPage::TravelReady) {
            frame(chrome, &mut list.quads, super::layout::ITEM_MESSAGE);
            // The retail font is fixed width. Wrap long resource/cure messages
            // within this 24-cell window instead of drawing across its border.
            let mut line = String::new();
            let mut y = 22;
            for word in view.message.split_whitespace() {
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
