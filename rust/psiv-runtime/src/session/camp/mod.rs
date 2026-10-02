//! The camp menu and the chest window: the menu state machine, one frame at a
//! time.
//!
//! The camp opens on the Camp button (`Session::camp_frame`) and a chest the
//! field opened pre-empts it; from then on the pad belongs to this mode. The
//! [`CampView`] is both the machine's state and what the shell draws: pages,
//! cursors, the roster snapshot and the line a command answered with. Every
//! rule the Godot menu used to carry lives in the submodules, one concern each:
//!
//! ```text
//! input      the page dispatch, backing out, ITEM and the root's options
//! abilities  TECH and SKILL: the caster, the ability, its target
//! equipment  EQUIP: equip against unequip, the hand choice
//! order      STATE > ORDER: the pick and undo draft and its frame-counted close
//! travel     RYUKA/HINAS and the pipes: the town list, the paid message
//! loot       the chest windows: the discard and give-up confirmations
//! ```
//!
//! Buttons are the cartridge's. Its menu routines test `ButtonSpeak_Mask|ButtonCamp_Mask`
//! to confirm and `ButtonCancel` to back out (`Win_MenuOptionsMain`,
//! `ps4.asm:117255-117281`; `Win_ItemActionMain`, `ps4.asm:122507-122520`;
//! `Win_EquipCharListMain`, `ps4.asm:126658-126680`; the TECH and SKILL lists at
//! `ps4.asm:128511` and `ps4.asm:130572`), and `ButtonStart` on a cursor page
//! closes the whole menu (`DestroyAllWindows`, the same lines). A result line is
//! dismissed by any of `Cancel|Speak|Camp|Start` (`Win_ItemUsedMsgMain`,
//! `ps4.asm:123286`). The chest windows confirm on Speak or Camp and back out on
//! Cancel with no Start (`FieldRoutine_ItemFound`, `ps4.asm:137584-137599`; its
//! result lines `ps4.asm:137478`). The routines were read at those
//! representatives; every other camp page uses the same rule and is not
//! individually verified: STATUS, ORDER, SAVE, the town list, the ability
//! target and the hand choice. The `Camp` press that opens the menu is
//! `FieldControls_GetInput`'s (`ps4.asm:114890`, table `loc_56014`).

mod abilities;
mod equipment;
mod input;
mod loot;
mod order;
mod travel;

use psiv_core::Input;
use psiv_data::TownDestination;

use crate::{
    CampAbility, CampAbilityKind, CampItem, CampState, Pad, Runtime, RuntimeEvent, pad::Button,
};

use super::{Frame, FrameMode, Session};

pub use order::OrderDraft;

/// The page the camp window shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CampPage {
    /// A chest's result line.
    LootMessage,
    /// The pack, over a full-pack chest.
    LootItems,
    /// "Discard the selected item?"
    LootDiscardConfirm,
    /// "Give up this item?"
    LootReturnConfirm,
    /// "This item may become necessary."
    LootBlocked,
    /// ITEM, TECH, SKILL, EQUIP, STATE, MUMBL, MACRO.
    Root,
    /// ITEM with an empty pack.
    ItemEmpty,
    /// ITEM's pack list.
    ItemList,
    /// ITEM's target pick.
    ItemTarget,
    /// ITEM's result line.
    ItemResult,
    /// TECH/SKILL's caster pick.
    AbilityCharacters,
    /// TECH/SKILL's ability list.
    AbilityList,
    /// TECH/SKILL's target pick.
    AbilityTarget,
    /// TECH/SKILL's result line.
    AbilityResult,
    /// RYUKA/HINAS and the pipes' town list.
    TravelTowns,
    /// "Ready to teleport", until acknowledged.
    TravelReady,
    /// EQUIP's character pick.
    EquipCharacters,
    /// EQUIP's slot pick, over the stat sheet.
    EquipStats,
    /// EQUIP's item list.
    EquipItems,
    /// EQUIP's right/left hand choice.
    EquipHands,
    /// EQUIP's result line.
    EquipResult,
    /// STATE: STATUS, ORDER, SAVE.
    State,
    /// ORDER's pick.
    Order,
    /// ORDER's committed line, until acknowledged or timed out.
    OrderDone,
    /// ORDER with one member: "There's only one of you!"
    OrderAlone,
    /// SAVE's slot list.
    SaveSlots,
    /// SAVE's result line.
    SaveResult,
    /// STATUS.
    Status,
    /// An option the port does not have yet, or an order the party refused.
    Unsupported,
}

/// The seven root options, in menu order.
pub const ROOT_OPTIONS: [&str; 7] = ["ITEM", "TECH", "SKILL", "EQUIP", "STATE", "MUMBL", "MACRO"];

/// The camp menu: its state and what the shell draws from it.
#[derive(Clone, Debug)]
pub struct CampView {
    /// The page.
    pub page: CampPage,
    /// The cursor on the root options.
    pub root_selection: usize,
    /// The cursor on STATE's options.
    pub state_selection: usize,
    /// The cursor on SAVE's slots.
    pub save_selection: usize,
    /// The cursor on the pack list (ITEM, or a full-pack chest's).
    pub item_selection: usize,
    /// The cursor on a target pick.
    pub target_selection: usize,
    /// YES (0) or NO (1) on a chest's confirmations. Its own cursor: the
    /// target pick's is clamped to the party every frame, which would turn a
    /// lone member's default NO into YES.
    pub confirm_selection: usize,
    /// Whether the ability pages are TECH or SKILL.
    pub ability_kind: CampAbilityKind,
    /// The cursor on the caster pick.
    pub ability_character_selection: usize,
    /// The cursor on the ability list.
    pub ability_selection: usize,
    /// The caster's abilities.
    pub ability_options: Vec<CampAbility>,
    /// The town list RYUKA/HINAS or a pipe offered.
    pub travel_towns: Vec<TownDestination>,
    /// The cursor on the town list.
    pub travel_selection: usize,
    /// The pack slot of the pipe being used, when the travel is an item's.
    pub travel_item_slot: Option<usize>,
    /// The cursor on STATUS's members.
    pub status_selection: usize,
    /// Whether STATUS was entered from STATE (and backs out to it).
    pub status_from_state: bool,
    /// ORDER's draft.
    pub order_draft: OrderDraft,
    /// The cursor on EQUIP's members.
    pub equipment_character_selection: usize,
    /// The cursor on EQUIP's slots (head, right, left, body).
    pub equipment_slot_selection: usize,
    /// The cursor on EQUIP's item list.
    pub equipment_item_selection: usize,
    /// RIGHT HAND (0) or LEFT HAND (1).
    pub equipment_hand_selection: usize,
    /// The items the selected member can equip.
    pub equipment_options: Vec<CampItem>,
    /// The roster, the pack and the purse, refreshed every frame.
    pub snapshot: CampState,
    /// The result line of the last command.
    pub message: String,
    pub(crate) order_wait: u16,
    pub(crate) sound_request: Option<u8>,
    pub(crate) runtime_events: Vec<RuntimeEvent>,
    pub(crate) save_slot_request: Option<usize>,
    pub(crate) closed: bool,
}

/// One frame of the menu, as the session needs it.
pub(crate) struct CampFrame {
    pub(crate) events: Vec<RuntimeEvent>,
    pub(crate) sound: Option<u8>,
    pub(crate) closed: bool,
}

/// The pad's edges and levels the menu pages read.
#[derive(Clone, Copy)]
pub(crate) struct MenuInput {
    pub(crate) up: bool,
    pub(crate) down: bool,
    pub(crate) left: bool,
    pub(crate) right: bool,
    pub(crate) accept: bool,
    pub(crate) cancel: bool,
    pub(crate) start: bool,
}

impl MenuInput {
    pub(crate) fn of(pressed: Pad) -> MenuInput {
        MenuInput {
            up: pressed.held(Button::Up),
            down: pressed.held(Button::Down),
            left: pressed.held(Button::Left),
            right: pressed.held(Button::Right),
            accept: pressed.held(Button::Speak) || pressed.held(Button::Camp),
            cancel: pressed.held(Button::Cancel),
            start: pressed.held(Button::Start),
        }
    }
}

impl CampView {
    /// Opens the camp on its root page.
    pub(crate) fn open(runtime: &Runtime) -> CampView {
        CampView {
            page: CampPage::Root,
            root_selection: 0,
            state_selection: 0,
            save_selection: 0,
            item_selection: 0,
            target_selection: 0,
            confirm_selection: 1,
            ability_kind: CampAbilityKind::Technique,
            ability_character_selection: 0,
            ability_selection: 0,
            ability_options: Vec::new(),
            travel_towns: Vec::new(),
            travel_selection: 0,
            travel_item_slot: None,
            status_selection: 0,
            status_from_state: false,
            order_draft: OrderDraft::default(),
            equipment_character_selection: 0,
            equipment_slot_selection: 0,
            equipment_item_selection: 0,
            equipment_hand_selection: 0,
            equipment_options: runtime.camp_equipment(0),
            snapshot: runtime.camp_state(),
            message: String::new(),
            order_wait: 0,
            sound_request: None,
            runtime_events: Vec::new(),
            save_slot_request: None,
            closed: false,
        }
    }

    /// Whether the window is a chest's rather than the camp's.
    #[must_use]
    pub fn is_loot(&self) -> bool {
        matches!(
            self.page,
            CampPage::LootMessage
                | CampPage::LootItems
                | CampPage::LootDiscardConfirm
                | CampPage::LootReturnConfirm
                | CampPage::LootBlocked
        )
    }

    pub(super) fn close(&mut self) {
        self.closed = true;
        self.message.clear();
    }

    /// The save slot the player picked, once, for the session to write.
    pub(crate) fn take_save_request(&mut self) -> Option<usize> {
        self.save_slot_request.take()
    }

    /// Records how the save went: the result line SAVE shows.
    pub(crate) fn finish_save(&mut self, result: Result<(), String>) {
        self.message = match result {
            Ok(()) => "FILE SAVED".to_owned(),
            Err(error) => format!("SAVE ERROR: {error}"),
        };
    }

    /// Refreshes the snapshot and clamps every cursor to it.
    pub(crate) fn sync(&mut self, runtime: &Runtime) {
        if self.closed {
            return;
        }
        self.snapshot = runtime.camp_state();
        self.item_selection = self
            .item_selection
            .min(self.snapshot.inventory.len().saturating_sub(1));
        self.status_selection = self
            .status_selection
            .min(self.snapshot.party.len().saturating_sub(1));
        self.target_selection = self
            .target_selection
            .min(self.snapshot.party.len().saturating_sub(1));
        self.equipment_character_selection = self
            .equipment_character_selection
            .min(self.snapshot.party.len().saturating_sub(1));
        self.equipment_slot_selection = self.equipment_slot_selection.min(3);
        if let Some(character) = self.snapshot.party.get(self.ability_character_selection) {
            self.ability_options = runtime.camp_abilities(character.party_slot, self.ability_kind);
            self.ability_selection = self
                .ability_selection
                .min(self.ability_options.len().saturating_sub(1));
        }
        self.equipment_options = runtime.camp_equipment(self.equipment_character_selection);
        self.equipment_item_selection = self
            .equipment_item_selection
            .min(self.equipment_options.len().saturating_sub(1));
    }

    /// Drains what the frame produced for the session.
    pub(crate) fn finish_frame(&mut self) -> CampFrame {
        CampFrame {
            events: std::mem::take(&mut self.runtime_events),
            sound: self.sound_request.take(),
            closed: self.closed,
        }
    }
}

pub(crate) use crate::session::shop::wrap;

impl Session {
    /// One frame of the camp menu or a chest window; `None` when neither is up
    /// or opens, and the field has the frame.
    pub(crate) fn camp_frame(&mut self, pressed: Pad) -> Option<Frame> {
        match self.camp.as_mut() {
            None => {
                self.camp = Some(if self.runtime.loot_state().is_some() {
                    CampView::open_loot(&self.runtime)
                } else if self.runtime.scene_active() || !pressed.held(Button::Camp) {
                    return None;
                } else {
                    CampView::open(&self.runtime)
                });
            }
            Some(camp) => {
                camp.frame(&mut self.runtime, MenuInput::of(pressed));
            }
        }
        let camp = self.camp.as_mut().expect("the camp is up");
        // The camp's SAVE completes here: the session owns the store, so the
        // page's result line is written by the same frame that asked for it,
        // with no round trip through the shell.
        let mut save_error = None;
        if let Some(slot) = camp.take_save_request() {
            save_error = self.finish_camp_save(slot);
        }
        let camp = self.camp.as_mut().expect("the camp is up");
        let drained = camp.finish_frame();
        if drained.closed {
            self.camp = None;
            self.runtime.set_field_suspended(false);
            // The press that closed the menu must not read as a talk.
            self.accept_blocked = true;
            return Some(Frame {
                mode: FrameMode::Camp,
                menu_events: drained.events,
                sound: drained.sound,
                camp_save_error: save_error,
                ..Frame::default()
            });
        }
        self.runtime.set_field_suspended(true);
        let events = self.runtime.tick(Input::Neutral);
        let (events, routed) = self.route(events);
        if let Some(camp) = self.camp.as_mut() {
            camp.sync(&self.runtime);
        }
        Some(Frame {
            mode: FrameMode::Camp,
            events,
            routed,
            menu_events: drained.events,
            sound: drained.sound,
            camp_save_error: save_error,
            ..Frame::default()
        })
    }
}
