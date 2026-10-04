//! The camp menu: the `equip`, `use_technique`, `use_item`, `reorder` and `save`
//! controllers.
//!
//! Each opens the camp with the Camp button, walks the root menu to its page
//! and the page's own lists with Up, Down and Speak, reads the result line the
//! runtime answered with, and backs out with Cancel. Every list, cursor and
//! result is read from [`psiv_runtime::CampView`]; which slot an item goes in,
//! what a cure heals and whether the order took are the runtime's.

use psiv_runtime::{Button, CampPage, CampView};

use crate::driver::Driver;
use crate::halt::{Halt, HaltKind, Res};
use crate::menu::{find_named, listing};
use crate::route::NameOrId;

const ROOT_ITEM: usize = 0;
const ROOT_TECH: usize = 1;
const ROOT_EQUIP: usize = 3;
const ROOT_STATE: usize = 4;
const STATE_ORDER: usize = 1;
const STATE_SAVE: usize = 2;

/// The camp's equipped-items cursor order is head, right hand, left hand,
/// body; the record's order (`CampCharacter::equipment_ids`) is right, left,
/// head, body.
const UI_TO_RECORD: [usize; 4] = [2, 0, 1, 3];

impl Driver {
    fn camp(&self) -> Res<&CampView> {
        self.session()
            .camp_view()
            .ok_or_else(|| Halt::new(HaltKind::UnexpectedState, "the camp is not open"))
    }

    fn camp_page(&self) -> Res<CampPage> {
        Ok(self.camp()?.page)
    }

    fn expect_camp_page(&self, wanted: CampPage) -> Res {
        let page = self.camp_page()?;
        if page == wanted {
            Ok(())
        } else {
            let message = self.camp()?.message.clone();
            Err(Halt::new(
                HaltKind::UnexpectedState,
                format!("the camp is on {page:?}, expected {wanted:?} ({message:?})"),
            ))
        }
    }

    /// Opens the camp on its root page with the Camp button.
    fn open_camp(&mut self) -> Res {
        self.settle(false)?;
        self.tap(Button::Camp)?;
        self.expect_camp_page(CampPage::Root)
    }

    /// Backs out of the camp by Cancel until it closes, then lets the field
    /// settle.
    fn close_camp(&mut self) -> Res {
        for _ in 0..16 {
            if self.session().camp_view().is_none() {
                return self.settle(false).map(|_| ());
            }
            self.tap(Button::Cancel)?;
        }
        Err(Halt::new(HaltKind::Stuck, "the camp would not close"))
    }

    fn camp_root(&mut self, row: usize) -> Res {
        self.cursor_to(
            "camp",
            |d| d.session().camp_view().map(|v| v.root_selection),
            row,
            Some(psiv_runtime::ROOT_OPTIONS.len()),
        )?;
        self.tap(Button::Speak)
    }

    /// The party member `key` names, as a row of the camp's roster.
    fn member_row(&self, key: &NameOrId) -> Res<usize> {
        let party = &self.camp()?.snapshot.party;
        find_named(party, key, |m| m.name.as_str(), |m| u32::from(m.id)).ok_or_else(|| {
            Halt::new(
                HaltKind::MenuEntryMissing,
                format!(
                    "{key} is not in the party; the party is: {}",
                    listing(party, |m| m.name.as_str())
                ),
            )
        })
    }

    /// Equips `item` on `member` through EQUIP.
    ///
    /// The EQUIP menu opens its item list from an empty slot and takes off what
    /// a full slot holds, so a member who wears something in every slot first
    /// has one cleared: the hand the item is for when it is a hand item (what
    /// it displaces goes to the pack either way), otherwise a head or body
    /// piece, which is put back on after.
    ///
    /// # Errors
    ///
    /// [`HaltKind::MenuEntryMissing`] when the member, the item or a free slot
    /// is not there; [`HaltKind::UnexpectedState`] when the runtime refuses.
    pub fn equip(&mut self, member: &NameOrId, item: &NameOrId) -> Res {
        self.open_camp()?;
        self.camp_root(ROOT_EQUIP)?;
        self.expect_camp_page(CampPage::EquipCharacters)?;
        let who = self.member_row(member)?;
        let rows = self.camp()?.snapshot.party.len();
        self.cursor_to(
            "equip member",
            |d| {
                d.session()
                    .camp_view()
                    .map(|v| v.equipment_character_selection)
            },
            who,
            Some(rows),
        )?;
        self.tap(Button::Speak)?;
        self.expect_camp_page(CampPage::EquipStats)?;
        let mut worn = self.camp()?.snapshot.party[who].equipment_ids;
        let mut restore = None;
        if worn.iter().all(|id| *id != 0) {
            restore = self.make_room(member, item, &worn)?;
            worn = self.camp()?.snapshot.party[who].equipment_ids;
        }
        self.equip_from_stats(member, item, &worn)?;
        if let Some((id, record)) = restore {
            // The new item took its own slot; what was taken off goes back on
            // only when its slot is still the empty one.
            let worn = self.camp()?.snapshot.party[who].equipment_ids;
            if worn[record] == 0 {
                self.equip_from_stats(member, &NameOrId::Id(u16::from(id)), &worn)?;
            }
        }
        self.close_camp()
    }

    /// Clears one slot of a member who wears something in all four, from the
    /// EQUIP slot page. Returns the item taken off and its record slot, to put
    /// back on afterwards if the new item did not take that slot.
    fn make_room(
        &mut self,
        member: &NameOrId,
        item: &NameOrId,
        worn: &[u8; 4],
    ) -> Res<Option<(u8, usize)>> {
        let (wanted_id, hand_item) = {
            let pack = &self.camp()?.snapshot.inventory;
            let row = find_named(pack, item, |i| i.name.as_str(), |i| u32::from(i.id)).ok_or_else(
                || {
                    Halt::new(
                        HaltKind::MenuEntryMissing,
                        format!(
                            "the pack holds no {item} for {member}; it holds: {}",
                            listing(pack, |i| i.name.as_str())
                        ),
                    )
                },
            )?;
            let slot = pack[row].slot;
            (
                pack[row].id,
                self.runtime().camp_equipment_hand_choice(slot),
            )
        };
        // UI order: head 0, right hand 1, left hand 2, body 3.
        let (ui, restore) = if hand_item {
            let ui = [1_usize, 2]
                .into_iter()
                .find(|ui| worn[UI_TO_RECORD[*ui]] != wanted_id)
                .unwrap_or(1);
            (ui, None)
        } else {
            let ui = [0_usize, 3]
                .into_iter()
                .find(|ui| worn[UI_TO_RECORD[*ui]] != wanted_id)
                .ok_or_else(|| {
                    Halt::new(
                        HaltKind::MenuEntryMissing,
                        format!("{member} already wears {item} in every head and body slot"),
                    )
                })?;
            (ui, Some((worn[UI_TO_RECORD[ui]], UI_TO_RECORD[ui])))
        };
        self.cursor_to(
            "equip slot",
            |d| d.session().camp_view().map(|v| v.equipment_slot_selection),
            ui,
            Some(4),
        )?;
        self.tap(Button::Speak)?;
        self.expect_camp_page(CampPage::EquipResult)?;
        let message = self.camp()?.message.clone();
        if !message.starts_with("REMOVED") {
            return Err(Halt::new(
                HaltKind::UnexpectedState,
                format!("taking off the slot for {member} refused: {message:?}"),
            ));
        }
        self.tap(Button::Speak)?;
        self.expect_camp_page(CampPage::EquipStats)?;
        Ok(restore)
    }

    /// From the EQUIP slot page: opens the item list from an empty slot and
    /// equips `item`.
    fn equip_from_stats(&mut self, member: &NameOrId, item: &NameOrId, worn: &[u8; 4]) -> Res {
        self.expect_camp_page(CampPage::EquipStats)?;
        let slot = (0..4)
            .find(|ui| worn[UI_TO_RECORD[*ui]] == 0)
            .ok_or_else(|| {
                Halt::new(
                    HaltKind::MenuEntryMissing,
                    format!("{member} has no empty equipment slot for {item}"),
                )
            })?;
        self.cursor_to(
            "equip slot",
            |d| d.session().camp_view().map(|v| v.equipment_slot_selection),
            slot,
            Some(4),
        )?;
        self.tap(Button::Speak)?;
        self.expect_camp_page(CampPage::EquipItems)?;
        let row = {
            let options = &self.camp()?.equipment_options;
            find_named(options, item, |o| o.name.as_str(), |o| u32::from(o.id)).ok_or_else(
                || {
                    Halt::new(
                        HaltKind::MenuEntryMissing,
                        format!(
                            "{member} cannot equip {item} from the pack; the list is: {}",
                            listing(options, |o| o.name.as_str())
                        ),
                    )
                },
            )?
        };
        let rows = self.camp()?.equipment_options.len();
        self.cursor_to(
            "equip item",
            |d| d.session().camp_view().map(|v| v.equipment_item_selection),
            row,
            Some(rows),
        )?;
        self.tap(Button::Speak)?;
        if self.camp_page()? == CampPage::EquipHands {
            // Right hand when it is free (or holds another item), the left
            // when the right already holds this item.
            let wanted = self.camp()?.equipment_options[row].id;
            let hand = usize::from(
                (worn[0] == wanted && worn[1] != wanted) || (worn[0] != 0 && worn[1] == 0),
            );
            self.cursor_to(
                "equip hand",
                |d| d.session().camp_view().map(|v| v.equipment_hand_selection),
                hand,
                Some(2),
            )?;
            self.tap(Button::Speak)?;
        }
        self.expect_camp_page(CampPage::EquipResult)?;
        let message = self.camp()?.message.clone();
        if !message.starts_with("EQUIPPED") {
            return Err(Halt::new(
                HaltKind::UnexpectedState,
                format!("equip refused: {message:?}"),
            ));
        }
        self.tap(Button::Speak)?;
        self.expect_camp_page(CampPage::EquipStats)
    }

    /// Casts `technique` from `caster` on `target` through TECH.
    ///
    /// # Errors
    ///
    /// [`HaltKind::MenuEntryMissing`] when the caster, the technique or the
    /// target is not there; [`HaltKind::UnexpectedState`] when it has no effect.
    pub fn use_technique(
        &mut self,
        caster: &NameOrId,
        technique: &NameOrId,
        target: &NameOrId,
    ) -> Res {
        self.open_camp()?;
        self.camp_root(ROOT_TECH)?;
        self.expect_camp_page(CampPage::AbilityCharacters)?;
        let who = self.member_row(caster)?;
        let rows = self.camp()?.snapshot.party.len();
        self.cursor_to(
            "technique caster",
            |d| {
                d.session()
                    .camp_view()
                    .map(|v| v.ability_character_selection)
            },
            who,
            Some(rows),
        )?;
        self.tap(Button::Speak)?;
        self.expect_camp_page(CampPage::AbilityList)?;
        let row = {
            let options = &self.camp()?.ability_options;
            find_named(options, technique, |o| o.name.as_str(), |o| u32::from(o.id)).ok_or_else(
                || {
                    Halt::new(
                        HaltKind::MenuEntryMissing,
                        format!(
                            "{caster} does not know {technique}; the list is: {}",
                            listing(options, |o| o.name.as_str())
                        ),
                    )
                },
            )?
        };
        let rows = self.camp()?.ability_options.len();
        self.cursor_to(
            "technique list",
            |d| d.session().camp_view().map(|v| v.ability_selection),
            row,
            Some(rows),
        )?;
        self.tap(Button::Speak)?;
        if self.camp_page()? == CampPage::AbilityTarget {
            let who = self.member_row(target)?;
            let rows = self.camp()?.snapshot.party.len();
            self.cursor_to(
                "technique target",
                |d| d.session().camp_view().map(|v| v.target_selection),
                who,
                Some(rows),
            )?;
            self.tap(Button::Speak)?;
        }
        self.expect_camp_page(CampPage::AbilityResult)?;
        let message = self.camp()?.message.clone();
        self.note(format!("camp technique: {message}"));
        if message.contains("NO EFFECT") {
            return Err(Halt::new(
                HaltKind::UnexpectedState,
                format!("the technique did nothing: {message:?}"),
            ));
        }
        self.close_camp()
    }

    /// Uses `item` from the pack on `target` through ITEM.
    ///
    /// # Errors
    ///
    /// [`HaltKind::MenuEntryMissing`] when the pack or party lacks it;
    /// [`HaltKind::UnexpectedState`] when the item cannot be used.
    pub fn use_item(&mut self, item: &NameOrId, target: &NameOrId) -> Res {
        self.open_camp()?;
        self.camp_root(ROOT_ITEM)?;
        if self.camp_page()? == CampPage::ItemEmpty {
            return Err(Halt::new(
                HaltKind::MenuEntryMissing,
                format!("the pack holds nothing, so there is no {item} to use"),
            ));
        }
        self.expect_camp_page(CampPage::ItemList)?;
        let row = {
            let pack = &self.camp()?.snapshot.inventory;
            find_named(pack, item, |i| i.name.as_str(), |i| u32::from(i.id)).ok_or_else(|| {
                Halt::new(
                    HaltKind::MenuEntryMissing,
                    format!(
                        "the pack holds no {item}; it holds: {}",
                        listing(pack, |i| i.name.as_str())
                    ),
                )
            })?
        };
        let rows = self.camp()?.snapshot.inventory.len();
        self.cursor_to(
            "item list",
            |d| d.session().camp_view().map(|v| v.item_selection),
            row,
            Some(rows),
        )?;
        self.tap(Button::Speak)?;
        if self.camp_page()? == CampPage::ItemTarget {
            let who = self.member_row(target)?;
            let rows = self.camp()?.snapshot.party.len();
            self.cursor_to(
                "item target",
                |d| d.session().camp_view().map(|v| v.target_selection),
                who,
                Some(rows),
            )?;
            self.tap(Button::Speak)?;
        }
        // The cartridge's ITEM use ends one of three ways
        // (`ps4.asm:122546-122605`, `123337-123431`): a result line for a
        // disposable or for a refused action, or the menu closed by an action
        // that ran — a boarding event, or the "consumed" branch an item outside
        // the action table takes. Only the last of those leaves nothing to read.
        match self.session().camp_view().map(|view| view.page) {
            Some(CampPage::ItemResult) => {
                let message = self.camp()?.message.clone();
                self.note(format!("camp item: {message}"));
            }
            other if self.runtime().scene_active() => {
                self.note(format!(
                    "camp item: the action started a scene ({other:?} page)"
                ));
            }
            other => {
                return Err(Halt::new(
                    HaltKind::UnexpectedState,
                    format!(
                        "using {item} answered nothing ({other:?} page): the action table took \
                         the menu without running anything"
                    ),
                ));
            }
        }
        self.close_camp()
    }

    /// Sets the party order, first to last, through STATE > ORDER.
    ///
    /// # Errors
    ///
    /// [`HaltKind::MenuEntryMissing`] for a member not in the party;
    /// [`HaltKind::UnexpectedState`] when the order is refused.
    pub fn reorder(&mut self, order: &[NameOrId]) -> Res {
        self.open_camp()?;
        self.camp_root(ROOT_STATE)?;
        self.expect_camp_page(CampPage::State)?;
        self.cursor_to(
            "state",
            |d| d.session().camp_view().map(|v| v.state_selection),
            STATE_ORDER,
            Some(3),
        )?;
        self.tap(Button::Speak)?;
        self.expect_camp_page(CampPage::Order)?;
        // Each pick removes the member from the draft's remaining list; the
        // last member is placed automatically.
        for key in &order[..order.len().saturating_sub(1)] {
            let (id, rows) = {
                let camp = self.camp()?;
                let party = &camp.snapshot.party;
                let row = find_named(party, key, |m| m.name.as_str(), |m| u32::from(m.id))
                    .ok_or_else(|| {
                        Halt::new(
                            HaltKind::MenuEntryMissing,
                            format!(
                                "{key} is not in the party; the party is: {}",
                                listing(party, |m| m.name.as_str())
                            ),
                        )
                    })?;
                (party[row].id, camp.order_draft.remaining.len())
            };
            let row = self
                .camp()?
                .order_draft
                .remaining
                .iter()
                .position(|r| *r == id)
                .ok_or_else(|| {
                    Halt::new(
                        HaltKind::MenuEntryMissing,
                        format!("{key} is named twice in the order"),
                    )
                })?;
            self.cursor_to(
                "order",
                |d| d.session().camp_view().map(|v| v.order_draft.cursor),
                row,
                Some(rows),
            )?;
            self.tap(Button::Speak)?;
        }
        let page = self.camp_page()?;
        if page != CampPage::OrderDone {
            let message = self.camp()?.message.clone();
            return Err(Halt::new(
                HaltKind::UnexpectedState,
                format!("the order did not complete: {page:?} {message:?}"),
            ));
        }
        self.tap(Button::Speak)?;
        self.close_camp()
    }

    /// Saves to `slot` through STATE > SAVE.
    ///
    /// # Errors
    ///
    /// [`HaltKind::SaveFailed`] when the file cannot be written.
    pub fn save_slot(&mut self, slot: u8) -> Res {
        self.open_camp()?;
        self.camp_root(ROOT_STATE)?;
        self.expect_camp_page(CampPage::State)?;
        self.cursor_to(
            "state",
            |d| d.session().camp_view().map(|v| v.state_selection),
            STATE_SAVE,
            Some(3),
        )?;
        self.tap(Button::Speak)?;
        self.expect_camp_page(CampPage::SaveSlots)?;
        self.cursor_to(
            "save slot",
            |d| d.session().camp_view().map(|v| v.save_selection),
            usize::from(slot),
            Some(3),
        )?;
        // The press reaches the session, which asks for the write; the driver
        // answers it in the same frame.
        self.tap(Button::Speak)?;
        self.expect_camp_page(CampPage::SaveResult)?;
        let message = self.camp()?.message.clone();
        if message != "FILE SAVED" {
            return Err(Halt::new(
                HaltKind::SaveFailed,
                format!("the camp's SAVE answered {message:?}"),
            ));
        }
        self.close_camp()
    }
}
