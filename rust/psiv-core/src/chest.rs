//! Treasure chests: what one holds, whether it is open, and what opening it
//! does.
//!
//! # A chest is a field object
//!
//! `LoadTreasureChests` (`ps4.asm:110966`) loads each chest through
//! `Field_LoadObject` into `Field_Obj_Secondary` — the *same* pool the map's
//! NPCs occupy — as object type `$A0` (`FieldObj_TreasureChest`) or `$1D4`
//! (`FieldObj_WhiteTreasureChest`, the ones hidden in plants). Both set bit 3
//! of `$2(a4)`, so they are solid *and* talkable by exactly the rule ordinary
//! NPCs follow, and both run `FieldObj_OnScreenTest`, so they freeze off screen
//! like any other gated type.
//!
//! `GameMode_LoadFieldMap` calls `LoadMapObjects` and then
//! `LoadTreasureChests`, and `Field_LoadObject` takes the first free slot — so
//! a map's chests occupy the object slots immediately after its NPCs. That
//! ordering is why [`Chest::object_slot`] exists: the oracle's `oNN_*` columns
//! index this shared pool, and a chest is one of the objects in it.
//!
//! # The record
//!
//! Four bytes per chest (`ps4.asm:111000-111004`):
//!
//! | byte | meaning |
//! |---|---|
//! | `0` | `$38(a4)`: zero for a normal chest, non-zero for a white one |
//! | `1` | `item_type` (`$39`): zero means an item, non-zero means meseta |
//! | `2` | `chest_flag` (`$3A`): the `$F140` flag id recording it as opened |
//! | `3` | `item_id` (`$3B`): the item, or the meseta amount in hundreds |
//!
//! # Opening one
//!
//! `Interaction_ChkIfTreasureChest` (`ps4.asm:118579`) runs on whatever object
//! the ordinary talk probe found, matches its id against `$A0`/`$1D4`, and if
//! it is a chest copies the record into the interaction globals and switches to
//! `FieldRoutine_ItemFound` (`ps4.asm:137246`). That routine:
//!
//! 1. tests the chest flag and bails out if it is already set;
//! 2. plays `SFXID_ChestOpened` and sets the object's animation to frame 4, the
//!    open lid;
//! 3. grants the contents — `money += item_id * 100` for meseta, or the
//!    first-free inventory insert for an item, diverting to the swap path when
//!    the count reaches forty;
//! 4. sets the chest flag, through `ChestFlags_Set` because
//!    `Interaction_Event_Type` is 1.
//!
//! Step 4 is why open-state needs no storage of its own: it is derived from the
//! flag at map build, exactly as `LoadTreasureChests` derives the sprite frame
//! by calling `ChestFlags_Test` as it loads each one.
//!
//! **Which bank.** Chest flags are `$F120` bits — the same array as the
//! extended event flags, reached through the same door. They are *not* `$F140`
//! temp flags, whatever the disassembly's `Chest_Flags` label says; `state.rs`
//! carries the byte-level proof and the oracle's confirmation.
//!
//! The sharing with extended event flags is deliberate rather than accidental:
//! story logic gates content on "has the player opened this chest" by testing
//! the chest's own flag. Every literal-id `$F120` test site in the ROM reads a
//! real chest's bit.
//!
//! And nothing ever clears one. The `$F120` clear door's only caller is dead
//! code, so a chest, once opened, stays open for the rest of the save.

use crate::geom::Cell;
use crate::state::Flag;

/// What a chest holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChestContents {
    /// An item id, granted into the party's inventory.
    Item(u8),
    /// Meseta. The record stores hundreds; this is the amount actually added,
    /// already multiplied — `mulu.w #100, d0` then `add.l d0, (Current_Money)`.
    Meseta(u32),
}

impl ChestContents {
    /// Builds contents from the two raw record bytes.
    ///
    /// `item_type` selects how `item_id` is read: zero means the byte is an
    /// item id, anything else means it is a meseta amount in hundreds.
    #[must_use]
    pub const fn from_record(item_type: u8, item_id: u8) -> ChestContents {
        if item_type == 0 {
            ChestContents::Item(item_id)
        } else {
            ChestContents::Meseta(item_id as u32 * 100)
        }
    }
}

/// One treasure chest on a map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chest {
    /// Where it stands.
    pub cell: Cell,
    /// The `$F140` flag id that records it as opened.
    pub flag: u8,
    /// What is inside.
    pub contents: ChestContents,
    /// A white chest (`$1D4`), the kind hidden in plants, rather than the
    /// ordinary `$A0`. Presentation only — both behave identically.
    pub white: bool,
    /// Index among this map's chests, which fixes its object slot.
    pub index: usize,
}

impl Chest {
    /// The flag this chest sets when opened.
    #[must_use]
    pub const fn chest_flag(&self) -> Flag {
        Flag::chest(self.flag as u16)
    }

    /// This chest's slot in the shared field-object pool.
    ///
    /// `GameMode_LoadFieldMap` loads the map's NPCs first and its chests
    /// second, and `Field_LoadObject` hands out the first free slot, so a
    /// chest sits at `npc_count + index`.
    #[must_use]
    pub const fn object_slot(&self, npc_count: usize) -> usize {
        npc_count + self.index
    }
}

/// What opening a chest did, for the layer that has to show it.
///
/// The cartridge opens a window naming the item or the amount, so the renderer
/// needs the contents rather than just "something happened".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChestOutcome {
    /// The item went into a free inventory slot.
    Took {
        /// The item id granted.
        item: u8,
        /// Which inventory slot it landed in.
        slot: usize,
    },
    /// Meseta was added to the purse.
    Meseta {
        /// How much, already in meseta rather than hundreds.
        amount: u32,
    },
    /// Every inventory slot is taken. Nothing has been granted and the chest
    /// flag is **not** set — the cartridge shows the "you have too much"
    /// message and offers a swap, and only the swap completes the open.
    Full {
        /// The item still waiting to be taken.
        item: u8,
    },
    /// The chest was already open. `FieldRoutine_ItemFound` tests the flag
    /// first and leaves without granting anything.
    AlreadyOpen,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chest(flag: u8, contents: ChestContents) -> Chest {
        Chest {
            cell: Cell::new(4, 4),
            flag,
            contents,
            white: false,
            index: 0,
        }
    }

    #[test]
    fn item_type_zero_means_an_item_and_anything_else_means_meseta() {
        assert_eq!(
            ChestContents::from_record(0, 0x7D),
            ChestContents::Item(0x7D)
        );
        // The record stores hundreds: `mulu.w #100, d0`.
        assert_eq!(ChestContents::from_record(1, 4), ChestContents::Meseta(400));
        // Any non-zero type selects meseta, not just 1.
        assert_eq!(
            ChestContents::from_record(0xFF, 10),
            ChestContents::Meseta(1000)
        );
    }

    #[test]
    fn the_largest_meseta_chest_does_not_overflow() {
        assert_eq!(
            ChestContents::from_record(1, 255),
            ChestContents::Meseta(25_500)
        );
    }

    #[test]
    fn a_chests_flag_is_an_f120_bit_not_an_f140_one() {
        // Oracle tape 20 measured `ChestFlag_PiataMonomate` (24) landing at
        // `$FFFFF123` bit 7 — byte 3 of `$F120`, exactly where
        // `bset 7-(id&7)` puts id 24 in that bank — with `$F140` untouched.
        let c = chest(24, ChestContents::Item(0x7D));
        assert_eq!(c.chest_flag(), Flag::chest(24));
        assert_eq!(c.chest_flag(), Flag::event(0x118), "the $F120 half");
        assert_ne!(c.chest_flag(), Flag::temp(24), "not the temp bank");
    }

    #[test]
    fn chests_take_the_object_slots_after_the_maps_npcs() {
        // LoadMapObjects runs before LoadTreasureChests and Field_LoadObject
        // takes the first free slot.
        let first = Chest {
            index: 0,
            ..chest(24, ChestContents::Item(1))
        };
        let second = Chest {
            index: 1,
            ..chest(25, ChestContents::Item(2))
        };
        assert_eq!(first.object_slot(8), 8);
        assert_eq!(second.object_slot(8), 9);
        // A map with no NPCs starts its chests at slot 0.
        assert_eq!(first.object_slot(0), 0);
    }
}
