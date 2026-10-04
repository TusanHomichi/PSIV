//! The field ITEM menu's action table: what USING an item does when it is
//! neither a pipe nor a disposable.
//!
//! The cartridge's `Win_ItemActionMain` sends a recognized item down one of two
//! paths by its `InventoryData` type byte (`ps4.asm:122557-122600`): type `8`
//! (disposable) takes the heal path, equipment types `0..7` and the plot and
//! field-only types `9`/`$A` take the action table
//! ([`field_item_action`]) — `ItemActionPtrs`, `ps4.asm:123355-123400`.
//!
//! The table's own routines decide here and now, and answer with one of four
//! things:
//!
//! ```text
//! ItemAction_Nothing      refuse, with the item's own message   (:123498)
//! ItemAction_Map          the overworld map screen              (:123405)
//! LandRover/IceDigger/…   board, on the right tile, Event $09/$0A/$0B
//! Pennant/WoodCarvin      Event $97/$98 in Chaz's house         (:123470, :123484)
//! ```
//!
//! A boarding action accepts only on the overworld — `Field_Map_Index & $FFF0`
//! is zero, which is Motavia, Dezolis and Rykros and their sub-maps `0..$0F` —
//! and only when `Vehicle_Boarding_Flags` (`$FFFFEC7F`) holds the machine's bit
//! (`ps4.asm:123419-123465`). `Runtime::field_item_use` computes that flag from
//! the tile the party stands on, exactly as `FieldRoutine_Menu` does when the
//! menu opens (`ps4.asm:117126-117135`; see `psiv_core::boarding_flags`).
//!
//! An *accepted* action destroys every open window (`loc_5B7E4`,
//! `ps4.asm:122596-122600`) and sets
//! `Routine_Exit_Flags` bit 1, which `Field_MenuExit` turns into
//! `Game_Mode_Routine = $C` — the field's event routine — so the scene runs
//! with no menu over it (`ps4.asm:117150-117178`). A *refused* action opens the
//! message window instead, whose text is the per-item record below
//! (`loc_5C2EA`, `ps4.asm:123594-123642`), or the generic
//! `"<item> is used!" / "But nothing happened."` pair (`loc_2AA0FC`) for an item
//! the table does not name. Both halves of that answer are
//! [`ItemUseOutcome`]'s; the session decides what to do with them
//! (`session::MenuScene`).

use crate::Runtime;

/// `MapID_ChazHouse` (`ps4.constants.asm:1161`).
const CHAZ_HOUSE: u16 = 0x5E;

/// `ItemID_PsycoWand`.
const PSYCO_WAND: u8 = 0x39;
/// `ItemID_AlisSword`.
const ALIS_SWORD: u8 = 0x8A;
/// `ItemID_Dynamite`.
const DYNAMITE: u8 = 0x8B;
/// `ItemID_Nothing4`, the overworld map item.
const NOTHING_4: u8 = 0x8C;
/// `ItemID_Alshline`.
const ALSHLINE: u8 = 0x8D;
/// `ItemID_EclpsTorch`.
const ECLPS_TORCH: u8 = 0x8E;
/// `ItemID_AeroPrism`.
const AERO_PRISM: u8 = 0x8F;
/// `ItemID_Pennant`.
const PENNANT: u8 = 0x94;
/// `ItemID_WoodCarvin`.
const WOOD_CARVIN: u8 = 0x95;
/// `ItemID_LandRover`.
const LAND_ROVER: u8 = 0x96;
/// `ItemID_IceDigger`.
const ICE_DIGGER: u8 = 0x97;
/// `ItemID_HydroFoil`.
const HYDROFOIL: u8 = 0x98;

/// The `Event_Index` each boarding action writes (`ps4.asm:123429`, `:123446`,
/// `:123463`): `Event_BoardingLandRover`, `...IceDigger`, `...Hydrofoil`.
const EVENT_BOARDING_LAND_ROVER: u16 = 0x09;
/// See [`EVENT_BOARDING_LAND_ROVER`].
const EVENT_BOARDING_ICE_DIGGER: u16 = 0x0A;
/// See [`EVENT_BOARDING_LAND_ROVER`].
const EVENT_BOARDING_HYDROFOIL: u16 = 0x0B;

/// `ItemAction_Pennant`'s event (`ps4.asm:123473`).
const EVENT_PENNANT: u16 = 0x97;
/// `ItemAction_WoodCarvin`'s event (`ps4.asm:123487`).
const EVENT_WOOD_CARVIN: u16 = 0x98;

/// The cartridge's action table entry for one item
/// (`ItemActionPtrs`, `ps4.asm:123355-123400`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FieldItemAction {
    /// `ItemAction_Nothing` (`ps4.asm:123498-123501`): the item is named by the
    /// table and always refuses.
    Nothing,
    /// `ItemAction_Map` (`ps4.asm:123405-123414`): the overworld map screen, on
    /// map `0` or `1` only.
    MapScreen,
    /// A vehicle's boarding action (`ps4.asm:123419-123465`).
    Board {
        /// The selector the boarding event writes, `1`..`3`.
        selector: u16,
        /// The bit `Vehicle_Boarding_Flags` must hold, `0`..`2`.
        flag_bit: u8,
        /// The `Event_Index` the action writes when it accepts.
        event: u16,
    },
    /// `ItemAction_Pennant` (`ps4.asm:123470`) and `ItemAction_WoodCarvin`
    /// (`:123484`):
    /// accepted in Chaz's house only.
    Home {
        /// The only map the action accepts on.
        map: u16,
        /// The `Event_Index` the action writes when it accepts.
        event: u16,
    },
}

/// The action table, in the cartridge's own order: `ItemActionPtrs`
/// (`ps4.asm:123355-123400`), eleven entries and the `$FFFF` terminator.
///
/// `AlisSword` is *not* here although the refusal table names it: its message
/// record (`loc_2AA1A8`) is unreachable through USE, because a table miss
/// returns the item id and the caller's test reads that as "consumed"
/// (`ps4.asm:123337-123353`, `loc_5B7E4` `:122596-122600`).
const ITEM_ACTIONS: [(u8, FieldItemAction); 11] = [
    (PSYCO_WAND, FieldItemAction::Nothing),
    (DYNAMITE, FieldItemAction::Nothing),
    (NOTHING_4, FieldItemAction::MapScreen),
    (ALSHLINE, FieldItemAction::Nothing),
    (ECLPS_TORCH, FieldItemAction::Nothing),
    (AERO_PRISM, FieldItemAction::Nothing),
    (
        PENNANT,
        FieldItemAction::Home {
            map: CHAZ_HOUSE,
            event: EVENT_PENNANT,
        },
    ),
    (
        WOOD_CARVIN,
        FieldItemAction::Home {
            map: CHAZ_HOUSE,
            event: EVENT_WOOD_CARVIN,
        },
    ),
    (
        LAND_ROVER,
        FieldItemAction::Board {
            selector: 1,
            flag_bit: 0,
            event: EVENT_BOARDING_LAND_ROVER,
        },
    ),
    (
        ICE_DIGGER,
        FieldItemAction::Board {
            selector: 2,
            flag_bit: 1,
            event: EVENT_BOARDING_ICE_DIGGER,
        },
    ),
    (
        HYDROFOIL,
        FieldItemAction::Board {
            selector: 3,
            flag_bit: 2,
            event: EVENT_BOARDING_HYDROFOIL,
        },
    ),
];

/// The refusal window's text, keyed by item: `loc_5C2EA`'s records
/// (`ps4.asm:339273-339330`; the US text bank, whose entries the pointers of
/// `ps4.asm:123594-123642` select). It names twelve items — one more than
/// [`ITEM_ACTIONS`], the unreachable `AlisSword` record.
///
/// `$FC` is the cartridge's line break and `$FF` its terminator; the strings
/// below are the decoded text, with the line breaks as newlines.
const REFUSAL_MESSAGES: [(u8, &str); 12] = [
    (PSYCO_WAND, "PSYCO-WAND has no effect\nwhen used here!"),
    (ALIS_SWORD, "ALIS-SWORD has been\nraised.It's so light!"),
    (DYNAMITE, "It's dangerous to use\nin a place like this!!"),
    (NOTHING_4, "Cannot be used here!"),
    (ALSHLINE, "It's a waste to use it\nin a place like this!"),
    (ECLPS_TORCH, "It's a waste to use it\nin a place like this!"),
    (
        AERO_PRISM,
        "AERO-PRISM is raised up!\nBut nothing happened.",
    ),
    (PENNANT, "It's a waste\nto put it here!"),
    (WOOD_CARVIN, "It's a waste\nto put it here!"),
    // The three machines share one record, `loc_2AA2C8` (`ps4.asm:340515`).
    (LAND_ROVER, "Can't get on it here!"),
    (ICE_DIGGER, "Can't get on it here!"),
    (HYDROFOIL, "Can't get on it here!"),
];

/// The item's action, or `None` when `ItemActionPtrs` does not name it.
#[must_use]
pub(crate) fn field_item_action(item: u8) -> Option<FieldItemAction> {
    ITEM_ACTIONS
        .iter()
        .find(|(id, _)| *id == item)
        .map(|(_, action)| *action)
}

/// The cartridge's own refusal text for an item's action, when the table has
/// one. `None` means `loc_5C2EA` does not name the item either, and the window
/// shows `<name> is used!` / `But nothing happened.` instead.
#[must_use]
pub(crate) fn refusal_message(item: u8) -> Option<&'static str> {
    REFUSAL_MESSAGES
        .iter()
        .find(|(id, _)| *id == item)
        .map(|(_, text)| *text)
}

/// What USING an item from the ITEM menu does, in the cartridge's terms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ItemUseOutcome {
    /// The action accepted: every window is destroyed and this event runs
    /// (`ps4.asm:123419-123431`, `117150-117178`).
    Scene {
        /// The event the action wrote into `Event_Index`.
        event: u16,
    },
    /// The action accepted and asked for the overworld map screen
    /// (`Routine_Exit_Flags` bit 2, `ps4.asm:117170-117175`). The port has no
    /// map screen; the ITEM page says so instead of inventing a refusal.
    MapScreen,
    /// The action refused: the cartridge's own line for the item.
    Refused {
        /// The message the retail window shows.
        message: String,
    },
    /// `ItemActionPtrs` does not name the item: `loc_5C0B0` returns the item id
    /// (`ps4.asm:123337-123353`), the caller reads that as "consumed" and
    /// destroys the windows without running anything or showing a line.
    MenuClosed,
}

impl Runtime {
    /// What the ITEM menu's USE does with `item` on this map, at the tile the
    /// party stands on.
    #[must_use]
    pub(crate) fn field_item_use(&self, item: u8) -> ItemUseOutcome {
        let map = self.map.id().0;
        match field_item_action(item) {
            None => ItemUseOutcome::MenuClosed,
            Some(FieldItemAction::Nothing) => self.refusal(item),
            Some(FieldItemAction::MapScreen) => {
                // `andi.w #$FFFE`: the map screen opens on Motavia (`0`) and
                // Dezolis (`1`) and nowhere else.
                if map & 0xFFFE == 0 {
                    ItemUseOutcome::MapScreen
                } else {
                    self.refusal(item)
                }
            }
            Some(FieldItemAction::Home { map: home, event }) if map == home => {
                ItemUseOutcome::Scene { event }
            }
            Some(FieldItemAction::Home { .. }) => self.refusal(item),
            Some(FieldItemAction::Board {
                selector,
                flag_bit,
                event,
            }) => {
                let overworld = map & 0xFFF0 == 0;
                let flags = self.boarding_flags();
                if overworld && flags & (1 << flag_bit) != 0 {
                    // The action writes `Vehicle_Index` only indirectly, in the
                    // event it starts; the selector is carried for the test and
                    // for the log line.
                    let _ = selector;
                    ItemUseOutcome::Scene { event }
                } else {
                    self.refusal(item)
                }
            }
        }
    }

    /// `Vehicle_Boarding_Flags` (`$FFFFEC7F`): the mask the tile under the
    /// party allows. `FieldRoutine_Menu` latches it when the menu opens from
    /// the four-cell maximum raw collision (`ps4.asm:117132-117135`); nothing
    /// moves while a menu is up, so reading it here is the same value.
    #[must_use]
    pub(crate) fn boarding_flags(&self) -> u8 {
        let cell = self.party.leader().cell();
        psiv_core::boarding_flags(psiv_core::standing_collision(&self.map, cell))
    }

    /// The refusal line for `item`: its own message, or the generic pair.
    fn refusal(&self, item: u8) -> ItemUseOutcome {
        let message = refusal_message(item)
            .map(str::to_owned)
            .unwrap_or_else(|| format!("{} is used!\nBut nothing happened.", self.item_name(item)));
        ItemUseOutcome::Refused { message }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_names_the_cartridges_own_items() {
        assert_eq!(
            field_item_action(PSYCO_WAND),
            Some(FieldItemAction::Nothing)
        );
        assert_eq!(
            field_item_action(NOTHING_4),
            Some(FieldItemAction::MapScreen)
        );
        assert_eq!(
            field_item_action(LAND_ROVER),
            Some(FieldItemAction::Board {
                selector: 1,
                flag_bit: 0,
                event: 0x09
            })
        );
        assert_eq!(
            field_item_action(ICE_DIGGER),
            Some(FieldItemAction::Board {
                selector: 2,
                flag_bit: 1,
                event: 0x0A
            })
        );
        assert_eq!(
            field_item_action(HYDROFOIL),
            Some(FieldItemAction::Board {
                selector: 3,
                flag_bit: 2,
                event: 0x0B
            })
        );
        assert_eq!(
            field_item_action(PENNANT),
            Some(FieldItemAction::Home {
                map: CHAZ_HOUSE,
                event: 0x97
            })
        );
        assert_eq!(
            field_item_action(WOOD_CARVIN),
            Some(FieldItemAction::Home {
                map: CHAZ_HOUSE,
                event: 0x98
            })
        );
        // A Monomate, a Dagger and the Control Key are not in it.
        for item in [125, 1, 0x99, 118] {
            assert_eq!(field_item_action(item), None, "item {item}");
        }
    }

    #[test]
    fn every_action_item_has_its_own_refusal_message() {
        for (item, action) in ITEM_ACTIONS {
            assert!(
                refusal_message(item).is_some(),
                "item {item:#04X} ({action:?}) has a message record"
            );
        }
        assert_eq!(refusal_message(LAND_ROVER), Some("Can't get on it here!"));
        assert_eq!(refusal_message(ICE_DIGGER), Some("Can't get on it here!"));
        assert_eq!(refusal_message(HYDROFOIL), Some("Can't get on it here!"));
        assert_eq!(refusal_message(NOTHING_4), Some("Cannot be used here!"));
        assert_eq!(refusal_message(1), None, "a Dagger has no record");
    }
}
