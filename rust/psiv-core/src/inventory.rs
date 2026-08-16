//! The party's item list.
//!
//! One flat array of forty item ids at `Inventory` (`$FFFFF410`), running to
//! `Current_Money` (`$FFFFF438`) — forty bytes, one per slot, `0` for empty.
//! It is **party-wide**, not per character: the only per-character item bytes
//! are the four equipment slots at `$4C..$4F` of a character's stat record.
//!
//! Two loops in `FieldRoutine_ItemFound` (`ps4.asm:137246`) define everything
//! this type does.
//!
//! Counting, which is how "is it full" is decided:
//!
//! ```text
//!     lea     (Inventory).w, a0
//!     moveq   #0, d0
//!     moveq   #39, d1
//! -   tst.b   (a0)+
//!     beq.s   +
//!     addq.w  #1, d0          ; count the non-empty slots
//! +   dbf     d1, -
//!     cmpi.w  #40, d0
//!     bcs.w   loc_66D66       ; room: take the normal add path
//! ```
//!
//! And inserting, which is first-free rather than sorted or appended:
//!
//! ```text
//!     lea     (Inventory).w, a0
//!     moveq   #39, d0
//! -   tst.b   (a0)+
//!     beq.s   +
//!     dbf     d0, -
//! +   move.b  ($FFFFE3FF).w, -(a0)
//! ```
//!
//! Note what the second loop does when there is no free slot: it falls out of
//! the `dbf` with `a0` one past the end and writes to `-(a0)`, clobbering slot
//! 39. Retail never reaches that, because the count above it diverts a full
//! inventory to the swap path first. [`Inventory::add`] returns an error rather
//! than reproducing an overwrite the cartridge cannot actually perform.

use crate::error::MapError;

/// Slots in the party's item list. `Inventory` (`$F410`) to `Current_Money`
/// (`$F438`) is forty bytes.
pub const INVENTORY_SLOTS: usize = 40;

/// The id an empty slot holds.
pub const EMPTY: u8 = 0;

/// The party's forty item slots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inventory {
    slots: [u8; INVENTORY_SLOTS],
}

impl Default for Inventory {
    fn default() -> Inventory {
        Inventory::new()
    }
}

impl Inventory {
    /// An empty list, which is what a new game starts with — the initialiser
    /// writes zero to all forty slots.
    #[must_use]
    pub const fn new() -> Inventory {
        Inventory {
            slots: [EMPTY; INVENTORY_SLOTS],
        }
    }

    /// The raw slots, in cartridge order. `0` is empty.
    #[must_use]
    pub const fn slots(&self) -> &[u8; INVENTORY_SLOTS] {
        &self.slots
    }

    /// Rebuilds from raw slot bytes, for a save load.
    #[must_use]
    pub const fn from_slots(slots: [u8; INVENTORY_SLOTS]) -> Inventory {
        Inventory { slots }
    }

    /// The item in `slot`, or `None` for an empty or out-of-range slot.
    #[must_use]
    pub fn get(&self, slot: usize) -> Option<u8> {
        match self.slots.get(slot) {
            Some(&EMPTY) | None => None,
            Some(&id) => Some(id),
        }
    }

    /// How many slots hold something.
    ///
    /// Counts non-empty slots across the whole array rather than stopping at
    /// the first gap, so a list with holes in it counts correctly — which
    /// matters because [`Inventory::remove`] leaves holes.
    #[must_use]
    pub fn occupied(&self) -> usize {
        self.slots.iter().filter(|&&id| id != EMPTY).count()
    }

    /// Whether an `add` would have to go through the swap path.
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.occupied() >= INVENTORY_SLOTS
    }

    /// Whether the list holds at least one of `item`.
    #[must_use]
    pub fn contains(&self, item: u8) -> bool {
        item != EMPTY && self.slots.contains(&item)
    }

    /// Puts `item` in the first free slot and returns which one.
    ///
    /// # Errors
    ///
    /// [`MapError::InventoryFull`] when every slot is taken. The caller's job
    /// is then the cartridge's: offer the swap, and call
    /// [`Inventory::swap`] with whatever the player gives up.
    pub fn add(&mut self, item: u8) -> Result<usize, MapError> {
        if item == EMPTY {
            return Err(MapError::InventoryFull);
        }
        let slot = self
            .slots
            .iter()
            .position(|&id| id == EMPTY)
            .ok_or(MapError::InventoryFull)?;
        self.slots[slot] = item;
        Ok(slot)
    }

    /// Empties `slot`, returning what was there.
    ///
    /// The hole is left in place — the cartridge does not compact the array,
    /// and [`Inventory::add`] fills the first hole it finds.
    pub fn remove(&mut self, slot: usize) -> Option<u8> {
        let held = self.get(slot)?;
        self.slots[slot] = EMPTY;
        Some(held)
    }

    /// Closes holes from left to right.
    ///
    /// This is not the normal inventory-removal operation. It is the private
    /// `ReorderInventory` tail used by the equipment window when the selected
    /// item is replaced by an empty displaced slot; field item use continues
    /// to leave a hole in cartridge order.
    pub(crate) fn compact(&mut self) {
        let mut write = 0;
        for read in 0..INVENTORY_SLOTS {
            if self.slots[read] == EMPTY {
                continue;
            }
            self.slots.swap(write, read);
            write += 1;
        }
        self.slots[write..].fill(EMPTY);
    }

    /// The full-inventory path: gives up whatever is in `slot` and puts `item`
    /// there, returning the item dropped.
    ///
    /// `FieldRoutine_ItemFound` reaches this when the count says forty. The
    /// player picks a slot, that item is discarded, and the found item takes
    /// its place.
    ///
    /// # Errors
    ///
    /// [`MapError::FlagOutOfRange`] is not used here; an out-of-range slot or
    /// an empty `item` yields [`MapError::InventoryFull`], the same error the
    /// caller was already handling.
    pub fn swap(&mut self, slot: usize, item: u8) -> Result<u8, MapError> {
        if item == EMPTY || slot >= INVENTORY_SLOTS {
            return Err(MapError::InventoryFull);
        }
        let dropped = self.slots[slot];
        self.slots[slot] = item;
        Ok(dropped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_list_is_forty_empty_slots() {
        let inventory = Inventory::new();
        assert_eq!(inventory.slots().len(), 40, "$F410 to $F438");
        assert_eq!(inventory.occupied(), 0);
        assert!(!inventory.is_full());
        assert_eq!(inventory.get(0), None);
    }

    #[test]
    fn adding_takes_the_first_free_slot_not_the_end() {
        // The cartridge scans for the first zero, so a hole is refilled before
        // the tail is used.
        let mut inventory = Inventory::new();
        assert_eq!(inventory.add(0x7D).unwrap(), 0);
        assert_eq!(inventory.add(0x01).unwrap(), 1);
        assert_eq!(inventory.add(0x02).unwrap(), 2);

        assert_eq!(inventory.remove(1), Some(0x01));
        assert_eq!(inventory.add(0x03).unwrap(), 1, "the hole, not slot 3");
        assert_eq!(inventory.get(2), Some(0x02), "and nothing shuffled");
    }

    #[test]
    fn removing_leaves_a_hole_rather_than_compacting() {
        let mut inventory = Inventory::new();
        for id in 1..=5 {
            inventory.add(id).unwrap();
        }
        inventory.remove(0).unwrap();
        assert_eq!(inventory.get(0), None);
        assert_eq!(inventory.get(1), Some(2), "slot 1 did not slide down");
        assert_eq!(inventory.occupied(), 4);
    }

    #[test]
    fn a_full_list_refuses_the_add_instead_of_clobbering_slot_39() {
        // Retail's insert loop would write over the last slot if it ever ran
        // with no free space; the count check upstream is what stops it. This
        // returns the error that sends the caller to the swap path instead.
        let mut inventory = Inventory::new();
        for id in 1..=40 {
            inventory.add(id).unwrap();
        }
        assert!(inventory.is_full());
        assert_eq!(inventory.occupied(), 40);
        assert!(matches!(inventory.add(0x7D), Err(MapError::InventoryFull)));
        assert_eq!(inventory.get(39), Some(40), "slot 39 is untouched");
    }

    #[test]
    fn the_swap_path_drops_what_the_player_chose() {
        let mut inventory = Inventory::new();
        for id in 1..=40 {
            inventory.add(id).unwrap();
        }
        let dropped = inventory.swap(7, 0x7D).unwrap();
        assert_eq!(dropped, 8, "slot 7 held item 8");
        assert_eq!(inventory.get(7), Some(0x7D));
        assert!(inventory.is_full(), "still forty items");
    }

    #[test]
    fn slot_zero_is_empty_not_an_item() {
        // Item ids start at 1 (`ItemID_Dagger = 1`), so 0 is unambiguously the
        // empty marker and can never be a real item.
        let mut inventory = Inventory::new();
        assert!(matches!(inventory.add(EMPTY), Err(MapError::InventoryFull)));
        assert!(!inventory.contains(EMPTY));
        assert_eq!(inventory.occupied(), 0);
    }

    #[test]
    fn raw_slots_round_trip() {
        let mut slots = [0u8; INVENTORY_SLOTS];
        slots[0] = 0x7D;
        slots[39] = 0x01;
        let inventory = Inventory::from_slots(slots);
        assert_eq!(inventory.occupied(), 2);
        assert_eq!(*inventory.slots(), slots);
        assert!(inventory.contains(0x7D));
    }
}
