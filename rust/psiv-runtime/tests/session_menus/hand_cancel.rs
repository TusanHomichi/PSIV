//! The equipment hand selector's Cancel, through ordinary pad input (#121).

use super::*;

/// Cancel on the hand selector changes nothing (#121). The cartridge's
/// selector (`loc_5F93A`, `ps4.asm:128025`) previews the chosen hand on each
/// cursor move from the saved record and commits only on Speak or Camp
/// (`loc_5FA58`, `:128105`, into `loc_5FAF2`); Cancel (`loc_5FAA2`,
/// `ps4.asm:128125-128145`) redraws the windows and returns to the item list
/// without reaching the commit.
#[test]
fn cancelling_the_hand_selector_changes_neither_equipment_nor_the_pack() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    // An empty left hand, so LEFT opens the item list.
    let mut game = camp_game();
    game.roster_mut().get_mut(CharId(0)).unwrap().equipment[1] = 0;
    let mut player = Player::new(FIELD, &game);
    let before = player.runtime().game().snapshot().to_retail_payload();

    player.press(Button::Camp);
    player.down_to(|s| s.camp_view().unwrap().root_selection, 3); // EQUIP
    player.press(Button::Speak);
    player.press(Button::Speak); // Chaz
    assert_eq!(camp_page(&player), CampPage::EquipStats);
    // The cursor order is head, right, left, body: LEFT is row 2, and an
    // empty slot opens the item list.
    player.down_to(|s| s.camp_view().unwrap().equipment_slot_selection, 2);
    player.press(Button::Speak);
    assert_eq!(camp_page(&player), CampPage::EquipItems);
    let dagger = player
        .session
        .camp_view()
        .unwrap()
        .equipment_options
        .iter()
        .position(|item| item.id == 1)
        .expect("the Dagger is a candidate");
    player.down_to(|s| s.camp_view().unwrap().equipment_item_selection, dagger);
    player.press(Button::Speak);
    assert_eq!(
        camp_page(&player),
        CampPage::EquipHands,
        "a one-handed weapon asks which hand"
    );
    // Move the hand cursor, so the cancelled choice is not the default one.
    player.press(Button::Down);
    assert_eq!(
        player.session.camp_view().unwrap().equipment_hand_selection,
        1
    );

    player.press(Button::Cancel);
    assert_eq!(camp_page(&player), CampPage::EquipItems, "back to the list");
    assert_eq!(
        player.runtime().game().snapshot().to_retail_payload(),
        before,
        "equipment and the pack are byte-identical"
    );
    player.press(Button::Cancel);
    assert_eq!(camp_page(&player), CampPage::EquipStats);
    assert_eq!(
        player.runtime().game().snapshot().to_retail_payload(),
        before
    );

    // The control: confirming the same choice does change the record.
    player.press(Button::Speak);
    player.down_to(|s| s.camp_view().unwrap().equipment_item_selection, dagger);
    player.press(Button::Speak);
    assert_eq!(camp_page(&player), CampPage::EquipHands);
    player.press(Button::Down);
    player.press(Button::Speak);
    assert_eq!(camp_page(&player), CampPage::EquipResult);
    assert_ne!(
        player.runtime().game().snapshot().to_retail_payload(),
        before
    );
    assert_eq!(
        player
            .runtime()
            .game()
            .roster()
            .get(CharId(0))
            .unwrap()
            .equipment[1],
        1,
        "the Dagger went into the left hand"
    );
}
