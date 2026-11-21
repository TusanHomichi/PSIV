//! Camp MACRO editor integration tests.

use super::{Button, CampPage, FIELD, Player, chaz_alone, fresh_game, pack_present};

#[test]
fn camp_macro_requires_two_members_and_builds_and_erases_macro_slots() {
    if !pack_present() {
        eprintln!("runtime pack not present; skipping");
        return;
    }
    // Solo Chaz sees MacroNotice instead of MacroList.
    let mut solo = Player::new(FIELD, &chaz_alone(0));
    solo.press(Button::Camp);
    for _ in 0..6 {
        solo.press(Button::Down);
    }
    solo.press(Button::Speak);
    assert_eq!(
        solo.session.camp_view().map(|v| v.page),
        Some(CampPage::MacroNotice),
        "solo party cannot edit macros"
    );
    solo.press(Button::Cancel);
    assert_eq!(
        solo.session.camp_view().map(|v| v.page),
        Some(CampPage::Root)
    );

    // Chaz + Alys opens MacroList and builds a new macro in slot B (index 1).
    let mut party = Player::new(FIELD, &fresh_game());
    party.press(Button::Camp);
    for _ in 0..6 {
        party.press(Button::Down);
    }
    party.press(Button::Speak);
    assert_eq!(
        party.session.camp_view().map(|v| v.page),
        Some(CampPage::MacroList)
    );
    // Slot A (index 0) is populated by default; move Down to empty slot B (index 1).
    party.press(Button::Down);
    assert_eq!(party.session.camp_view().unwrap().macro_draft.slot, 1);
    party.press(Button::Speak);
    assert_eq!(
        party.session.camp_view().map(|v| v.page),
        Some(CampPage::MacroPickMember)
    );
    // Pick first party member (Alys) -> TECH -> first learned technique.
    party.press(Button::Speak);
    assert_eq!(
        party.session.camp_view().map(|v| v.page),
        Some(CampPage::MacroPickAction)
    );
    party.press(Button::Down); // TECH
    party.press(Button::Speak);
    assert_eq!(
        party.session.camp_view().map(|v| v.page),
        Some(CampPage::MacroPickAbility)
    );
    party.press(Button::Speak); // picks first tech
    assert_eq!(
        party.session.camp_view().map(|v| v.page),
        Some(CampPage::MacroPickMember)
    );
    // Pick remaining member (Chaz) -> ATTACK -> commits macro and returns to MacroList.
    party.press(Button::Speak);
    assert_eq!(
        party.session.camp_view().map(|v| v.page),
        Some(CampPage::MacroPickAction)
    );
    party.press(Button::Speak); // ATTACK
    assert_eq!(
        party.session.camp_view().map(|v| v.page),
        Some(CampPage::MacroList)
    );
    let saved = party.runtime().game().macros()[1];
    assert_eq!(saved.commands[0].command_index, 2, "first entry is TECH");
    assert_ne!(saved.commands[0].ability_id, 0);
    assert_eq!(saved.commands[1].command_index, 1, "second entry is ATTACK");

    // Accepting an occupied slot prompts to erase: NO keeps it, YES erases it.
    party.press(Button::Speak);
    assert_eq!(
        party.session.camp_view().map(|v| v.page),
        Some(CampPage::MacroEraseConfirm)
    );
    party.press(Button::Speak); // default NO
    assert_eq!(
        party.session.camp_view().map(|v| v.page),
        Some(CampPage::MacroList)
    );
    assert_eq!(party.runtime().game().macros()[1], saved);

    party.press(Button::Speak);
    party.press(Button::Up); // YES
    party.press(Button::Speak);
    assert_eq!(
        party.session.camp_view().map(|v| v.page),
        Some(CampPage::MacroList)
    );
    assert_eq!(
        party.runtime().game().macros()[1].commands[0].command_index,
        0
    );
}
