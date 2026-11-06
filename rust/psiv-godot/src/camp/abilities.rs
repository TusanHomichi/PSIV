//! Camp TECH/SKILL pages: the caster, ability and target lists, and the travel
//! overlay. The session owns every choice; this only draws it.
use super::draw::{draw_text, frame};
use super::layout::{
    CHILD_CURSOR_PATTERN, CellRect, SELECTED_CURSOR_PATTERN, TOWN_ARROW_PATTERN,
    TOWN_ARROW_TAIL_PATTERN, TOWN_LIST_LABEL_COLUMN, TOWN_LIST_ORIGIN, TOWN_LIST_WIDTH,
    TRAVEL_MESSAGE_LINE_STEP,
};
use super::{CampChrome, CampMenu, DrawList, ITEM_LIST, ITEM_TARGET};
use psiv_runtime::{CampAbilityKind, CampPage, CampView, TOWN_PAGE_ROWS, town_page_rows};

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
            draw_town_pages(view, chrome, list);
        }
        if matches!(view.page, CampPage::AbilityResult | CampPage::TravelReady) {
            frame(chrome, &mut list.quads, super::layout::ITEM_MESSAGE);
            draw_message(view, chrome, list);
        }
    }
}

/// The town list as the cartridge leaves it on screen: every page window up
/// to the current one, each one cell right of the last, the later over the
/// earlier (`Win_TelepipePlaceList`, `ps4.asm:123998-124160`).
fn draw_town_pages(view: &CampView, chrome: &CampChrome, list: &mut DrawList) {
    let count = view.travel_towns.len();
    if count == 0 {
        return;
    }
    let current = view.travel_selection / TOWN_PAGE_ROWS;
    let last = (count - 1) / TOWN_PAGE_ROWS;
    let (left, top) = TOWN_LIST_ORIGIN;
    for page in 0..=current {
        let rows = town_page_rows(count, page) as i32;
        let x = left + page as i32;
        frame(
            chrome,
            &mut list.quads,
            CellRect::new(x, top, TOWN_LIST_WIDTH, 2 * rows + 2),
        );
        let first = page * TOWN_PAGE_ROWS;
        for (row, town) in view.travel_towns[first..]
            .iter()
            .take(rows as usize)
            .enumerate()
        {
            let y = top + 1 + 2 * row as i32;
            if let Some(quad) = chrome.window_word(CHILD_CURSOR_PATTERN, (x + 1, y)) {
                list.quads.push(quad);
            }
            draw_text(chrome, &mut list.quads, &town.name, (x + 3, y));
        }
        draw_text(
            chrome,
            &mut list.quads,
            &view.travel_towns_label,
            (x + TOWN_LIST_LABEL_COLUMN, top + 1 + 2 * rows),
        );
        if page > 0 {
            for (pattern, at) in [(TOWN_ARROW_TAIL_PATTERN, x - 1), (TOWN_ARROW_PATTERN, x)] {
                if let Some(quad) = chrome.window_word(pattern, (at, top)) {
                    list.quads.push(quad);
                }
            }
        }
        if page != last
            && page < 3
            && let Some(quad) =
                chrome.window_word_flipped(TOWN_ARROW_PATTERN, true, (x + TOWN_LIST_WIDTH - 1, top))
        {
            list.quads.push(quad);
        }
    }
    let row = (view.travel_selection % TOWN_PAGE_ROWS) as i32;
    if let Some(quad) = chrome.window_word(
        SELECTED_CURSOR_PATTERN,
        (left + current as i32 + 1, top + 1 + 2 * row),
    ) {
        list.quads.push(quad);
    }
}

/// A result line. An explicit line break is the cartridge's `$FC`, two cells
/// down; a long line without one wraps within the 24-cell window.
fn draw_message(view: &CampView, chrome: &CampChrome, list: &mut DrawList) {
    let mut y = 22;
    for paragraph in view.message.split('\n') {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
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
        y += TRAVEL_MESSAGE_LINE_STEP;
    }
}

fn cursor(chrome: &CampChrome, list: &mut DrawList, at: (i32, i32)) {
    if let Some(quad) = chrome.window_word(CHILD_CURSOR_PATTERN, at) {
        list.quads.push(quad);
    }
}
