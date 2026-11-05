//! #51: the per-member cursor words of the battle command windows.
//!
//! `Battle_Char_Comd_Index`, `Battle_Tech_Index`, `Battle_Skill_Index` and
//! `Battle_Item_Index` are one word per party slot (`ps4.asm:2195`, `2598`,
//! `2938`, `3330`). Nothing clears the first three during a battle, and
//! `loc_5BAE` clears the item words at each round's end
//! (`ps4.asm:8422-8425`). The window's own use of them is pinned against a
//! live battle in the crate's `commands_tests.rs`; this needs no pack.

use psiv_runtime::{CursorMemory, MenuPage, TargetKind};

#[test]
fn each_slot_keeps_its_own_word_per_window() {
    let mut memory = CursorMemory::default();
    for page in [
        MenuPage::Actions,
        MenuPage::Techniques,
        MenuPage::Skills,
        MenuPage::Items,
    ] {
        assert_eq!(memory.cursor(0, page), 0, "a battle opens at zero");
    }
    memory.remember(0, MenuPage::Actions, 4);
    memory.remember(1, MenuPage::Actions, 1);
    memory.remember(1, MenuPage::Techniques, 5);
    memory.remember(2, MenuPage::Skills, 3);
    memory.remember(1, MenuPage::Items, 2);
    assert_eq!(memory.cursor(0, MenuPage::Actions), 4);
    assert_eq!(memory.cursor(1, MenuPage::Actions), 1);
    assert_eq!(memory.cursor(1, MenuPage::Techniques), 5, "a row on page 2");
    assert_eq!(
        memory.cursor(0, MenuPage::Techniques),
        0,
        "slot 0's own word"
    );
    assert_eq!(memory.cursor(2, MenuPage::Skills), 3);
    assert_eq!(memory.cursor(1, MenuPage::Items), 2);
}

#[test]
fn the_round_end_clears_the_item_words_only() {
    let mut memory = CursorMemory::default();
    memory.remember(3, MenuPage::Actions, 3);
    memory.remember(3, MenuPage::Techniques, 2);
    memory.remember(3, MenuPage::Skills, 1);
    memory.remember(3, MenuPage::Items, 6);
    memory.end_round();
    assert_eq!(memory.cursor(3, MenuPage::Items), 0);
    assert_eq!(memory.cursor(3, MenuPage::Actions), 3);
    assert_eq!(memory.cursor(3, MenuPage::Techniques), 2);
    assert_eq!(memory.cursor(3, MenuPage::Skills), 1);
}

#[test]
fn target_pickers_and_slots_past_five_keep_no_word() {
    let mut memory = CursorMemory::default();
    let picker = MenuPage::Targets(TargetKind::Attack);
    memory.remember(0, picker, 2);
    memory.remember(5, MenuPage::Actions, 2);
    assert_eq!(memory.cursor(0, picker), 0);
    assert_eq!(memory.cursor(5, MenuPage::Actions), 0);
    assert_eq!(memory, CursorMemory::default());
}
