//! Native-only common-header extensions.
//!
//! Retail payload bytes and the payload checksum stay untouched: these fields
//! live in common-header space the cartridge does not address, and old or
//! retail slots simply omit them.

use super::{
    CharId, GAME_CLEARED_OFFSET, PARTY_SLOTS, RETAIL_SLOT_FILE_BYTES, RUST_DUNGEON_EXIT_OFFSET,
    RUST_SAVED_PARTY_OFFSET, RetailSlot,
};

impl RetailSlot {
    /// Preserve the live dungeon exit in the native header extension. Retail
    /// payload bytes and their checksum stay unchanged; old slots omit it.
    pub fn with_dungeon_exit(mut self, index: u8) -> Self {
        let offset = RUST_DUNGEON_EXIT_OFFSET * 2 + 1;
        for (i, byte) in [b'T', b'R', index, !index].into_iter().enumerate() {
            self.bytes[offset + i * 2] = byte;
        }
        self
    }

    /// Preserve retail's transient `Saved_Char_ID_Mem_1..5` bridge — the five
    /// party slots a scene parked while it shows another party — in the native
    /// header extension, so a SAVE taken between the two scenes and CONTINUEd
    /// in a fresh process restores the party. `None` writes nothing (the
    /// retail zeroes), as every save outside those scenes holds.
    #[must_use]
    pub fn with_saved_party_slots(mut self, slots: Option<[Option<CharId>; PARTY_SLOTS]>) -> Self {
        let Some(slots) = slots else {
            return self;
        };
        if slots.iter().all(Option::is_none) {
            return self;
        }
        let offset = RUST_SAVED_PARTY_OFFSET * 2 + 1;
        let bytes = slots.map(|slot| slot.map_or(CharId::EMPTY, |id| id.0));
        self.bytes[offset] = b'P';
        self.bytes[offset + 2] = b'S';
        for (index, byte) in bytes.iter().enumerate() {
            self.bytes[offset + 4 + index * 2] = *byte;
        }
        self.bytes[offset + 14] = !bytes.iter().fold(0_u8, |sum, byte| sum.wrapping_add(*byte));
        self
    }

    /// Read the native saved-party bridge. Missing or damaged metadata reads
    /// `None`: the restore then faults rather than inventing a party, and a
    /// retail save (whose zeroed header has no marker) does the same.
    #[must_use]
    pub fn saved_party_slots(&self) -> Option<[Option<CharId>; PARTY_SLOTS]> {
        let offset = RUST_SAVED_PARTY_OFFSET * 2 + 1;
        if self.bytes[offset] != b'P' || self.bytes[offset + 2] != b'S' {
            return None;
        }
        let mut slots = [None; PARTY_SLOTS];
        let mut sum = 0_u8;
        for (index, slot) in slots.iter_mut().enumerate() {
            let byte = self.bytes[offset + 4 + index * 2];
            sum = sum.wrapping_add(byte);
            *slot = (byte != CharId::EMPTY).then_some(CharId(byte));
        }
        if self.bytes[offset + 14] != !sum || slots.iter().all(Option::is_none) {
            return None;
        }
        Some(slots)
    }

    /// Read a valid native dungeon-exit extension. Missing or damaged metadata
    /// falls back to the ordinary map-entry table; it never invents an exit.
    #[must_use]
    pub fn dungeon_exit(&self) -> Option<u8> {
        let offset = RUST_DUNGEON_EXIT_OFFSET * 2 + 1;
        let index = self.bytes[offset + 4];
        (self.bytes[offset] == b'T'
            && self.bytes[offset + 2] == b'R'
            && self.bytes[offset + 6] == !index
            && index < 0x80)
            .then_some(index)
    }

    /// Sets retail's `Game_Cleared_Flag` in the common header (`$200035`).
    /// The retail byte is sticky — the only write is `ps4.asm:159099`, and only
    /// `InitSRAM` zeroes the header — so this never writes 0.
    #[must_use]
    pub fn mark_game_cleared(mut self) -> Self {
        self.bytes[GAME_CLEARED_OFFSET * 2 + 1] |= 1;
        self
    }

    /// Whether a slot file's header carries `Game_Cleared_Flag`, without
    /// validating the payload: the store's sticky check over other slots.
    #[must_use]
    pub fn cleared_in_bytes(bytes: &[u8]) -> bool {
        bytes.len() == RETAIL_SLOT_FILE_BYTES && bytes[GAME_CLEARED_OFFSET * 2 + 1] & 1 == 1
    }

    /// Reads retail's `Game_Cleared_Flag` from the common header (`$200035`).
    /// A slot written before the flag existed reads false, which is what the
    /// cartridge's zeroed common header holds.
    #[must_use]
    pub fn game_cleared(&self) -> bool {
        self.bytes[GAME_CLEARED_OFFSET * 2 + 1] & 1 != 0
    }
}
