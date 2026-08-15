//! Persistent game state: event flags and party composition.
//!
//! This is the save file's shape. It is deliberately serde-free — the core has
//! no dependencies — but every field is reachable as plain arrays and integers
//! through [`GameState::snapshot`] / [`GameState::from_snapshot`], so
//! `psiv-data` or any other crate can serialise it without this module knowing
//! how.
//!
//! # The cartridge's flag storage
//!
//! `EventFlags_Set`/`_Test`/`_Clear` (`ps4.asm:116626-116730`) are one shared
//! body reached through per-bank entry points that differ only in which base
//! address they load:
//!
//! ```text
//!     andi.w  #$FF, d0        ; flag id, masked to a byte
//!     move.w  d0, d2
//!     lsr.w   #3, d0          ; byte offset = id >> 3
//!     andi.w  #7, d2
//!     move.w  #7, d1
//!     sub.w   d2, d1          ; bit = 7 - (id & 7), MSB first
//!     btst    d1, (a0,d0.w)
//! ```
//!
//! So each bank is a bit array addressed **most-significant-bit first within
//! each byte**, and a bank holds at most 256 flags because of that `andi.w
//! #$FF`. Ids at or above `$100` are the *extended* bank:
//! `ExtendedEventFlags_Test` subtracts `$100` before running the same body, so
//! the event-flag id space is `$000..=$1FF` split across two banks.
//!
//! Bank sizes come straight from the RAM map (`ps4.constants.asm:2362-2367`),
//! each bank running up to the next symbol:
//!
//! | bank | address | bytes | flags |
//! |---|---|---:|---:|
//! | `Event_Flags` | `$F100` | 32 | 256 |
//! | `Extended_Event_Flags` | `$F120` | 32 | 256 |
//! | `Chest_Flags` | `$F140` | 22 | 176 |
//! | `Temp_Event_Flags` | `$F156` | 10 | 80 |
//! | `Town_Flags` | `$F160` | 16 | 128 |
//!
//! The design doc's "~174 flags" is the count of *named* `EventFlag_*`
//! constants, not the addressable space; the trigger table's highest observed
//! event flag is `$E8` and the whole retail set fits the base bank.

use crate::error::MapError;

/// Which bit array a flag lives in.
///
/// Separate banks, not one id space: the cartridge reaches each through its
/// own entry point, and a chest flag `$08` and an event flag `$08` are
/// unrelated bits in different arrays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FlagBank {
    /// `Event_Flags` plus `Extended_Event_Flags`, one id space `$000..=$1FF`.
    Event,
    /// `Chest_Flags` — one bit per treasure chest.
    Chest,
    /// `Temp_Event_Flags` — see [`GameState`] on their lifetime.
    Temp,
    /// `Town_Flags`.
    Town,
}

impl FlagBank {
    /// How many flags the bank can hold, from the RAM map.
    #[must_use]
    pub const fn capacity(self) -> u16 {
        match self {
            // 32 bytes at $F100 plus 32 more at $F120, reached by id >= $100.
            FlagBank::Event => 512,
            FlagBank::Chest => 176,
            FlagBank::Temp => 80,
            FlagBank::Town => 128,
        }
    }

    const fn bytes(self) -> usize {
        self.capacity() as usize / 8
    }
}

/// One flag: a bank and an id within it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Flag {
    /// Which bit array.
    pub bank: FlagBank,
    /// The id within that bank.
    pub id: u16,
}

impl Flag {
    /// An `EventFlag_*` id, including the extended `$100..` range.
    #[must_use]
    pub const fn event(id: u16) -> Flag {
        Flag {
            bank: FlagBank::Event,
            id,
        }
    }

    /// A `ChestFlag_*` id.
    #[must_use]
    pub const fn chest(id: u16) -> Flag {
        Flag {
            bank: FlagBank::Chest,
            id,
        }
    }

    /// A `TempEveFlag_*` id.
    #[must_use]
    pub const fn temp(id: u16) -> Flag {
        Flag {
            bank: FlagBank::Temp,
            id,
        }
    }

    /// A `TownFlag_*` id.
    #[must_use]
    pub const fn town(id: u16) -> Flag {
        Flag {
            bank: FlagBank::Town,
            id,
        }
    }
}

/// A character id, as stored in a party slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CharId(pub u8);

impl CharId {
    /// The byte an unused slot holds. `CalcPartyNumber` (`ps4.asm:120885`)
    /// counts until it sees one.
    pub const EMPTY: u8 = 0xFF;
}

/// How many field-party slots the cartridge has: `Current_Party_Slot_1..5` at
/// `$F40A..$F40E`, with `Char_ID_Mem_End` immediately after.
pub const PARTY_SLOTS: usize = 5;

/// A plain-data copy of the whole state, for saving.
///
/// Deliberately nothing but arrays and integers: another crate serialises it
/// without this one growing a dependency, and the layout mirrors the
/// cartridge's SRAM buffer closely enough to be recognisable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateSnapshot {
    /// `Event_Flags` + `Extended_Event_Flags`, MSB-first per byte.
    pub event_flags: [u8; 64],
    /// `Chest_Flags`.
    pub chest_flags: [u8; 22],
    /// `Temp_Event_Flags`.
    pub temp_flags: [u8; 10],
    /// `Town_Flags`.
    pub town_flags: [u8; 16],
    /// `Current_Party_Slots`, `$FF` for an empty slot.
    pub party: [u8; PARTY_SLOTS],
    /// `Current_Money`.
    pub money: u32,
}

/// The persistent state the event engine reads and writes.
///
/// # Temp flags are not automatically cleared
///
/// The name suggests a per-map or per-scene lifetime. The cartridge has no
/// such rule: `Temp_Event_Flags` is referenced in exactly four places in the
/// whole disassembly — `TempEveFlags_Test`, `_Set`, `_Clear` and `_Toggle` —
/// and nothing bulk-clears it, not at map load, not on save/load (it sits
/// inside `SRAM_Buffer`, so it is saved with everything else).
///
/// What makes them "temp" is that events clear them explicitly, in pairs. The
/// trigger table shows the idiom plainly: index `$26`
/// `RunEvent_ChazHouseRest` fires while `TempEveFlag_ChazHouse` is clear and
/// its event sets it; index `$27` `RunEvent_ClrChazHouseRest` fires while it
/// is set and its event clears it. Same for `$79`/`$7A` and the stripper
/// dance. So [`GameState`] treats temp flags exactly like any other bank and
/// leaves the lifetime to the scenes, which is what the cartridge does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameState {
    event: Vec<u8>,
    chest: Vec<u8>,
    temp: Vec<u8>,
    town: Vec<u8>,
    party: [u8; PARTY_SLOTS],
    money: u32,
}

impl Default for GameState {
    fn default() -> GameState {
        GameState::new()
    }
}

impl GameState {
    /// A fresh state: every flag clear, every party slot empty.
    #[must_use]
    pub fn new() -> GameState {
        GameState {
            event: vec![0; FlagBank::Event.bytes()],
            chest: vec![0; FlagBank::Chest.bytes()],
            temp: vec![0; FlagBank::Temp.bytes()],
            town: vec![0; FlagBank::Town.bytes()],
            party: [CharId::EMPTY; PARTY_SLOTS],
            money: 0,
        }
    }

    /// `Current_Money` (`$F438`, longword).
    #[must_use]
    pub const fn money(&self) -> u32 {
        self.money
    }

    /// Adds to the purse, saturating rather than wrapping. Scenes use this:
    /// `Event_MeetingHahn` adds 100, `Event_PrincipalConfession` 300.
    pub const fn add_money(&mut self, amount: u32) {
        self.money = self.money.saturating_add(amount);
    }

    /// Sets the purse outright.
    pub const fn set_money(&mut self, amount: u32) {
        self.money = amount;
    }

    fn bank(&self, bank: FlagBank) -> &[u8] {
        match bank {
            FlagBank::Event => &self.event,
            FlagBank::Chest => &self.chest,
            FlagBank::Temp => &self.temp,
            FlagBank::Town => &self.town,
        }
    }

    fn bank_mut(&mut self, bank: FlagBank) -> &mut [u8] {
        match bank {
            FlagBank::Event => &mut self.event,
            FlagBank::Chest => &mut self.chest,
            FlagBank::Temp => &mut self.temp,
            FlagBank::Town => &mut self.town,
        }
    }

    /// Byte index and bit number for `flag`, or `None` past the bank's end.
    ///
    /// The bit number is `7 - (id & 7)` — the cartridge numbers bits from the
    /// most significant end of each byte.
    fn locate(flag: Flag) -> Option<(usize, u8)> {
        if flag.id >= flag.bank.capacity() {
            return None;
        }
        let byte = usize::from(flag.id >> 3);
        let bit = 7 - (flag.id & 7) as u8;
        Some((byte, bit))
    }

    /// Whether `flag` is set. An id past the end of its bank reads as clear
    /// rather than panicking — a bad id is a data problem, not a crash.
    #[must_use]
    pub fn is_set(&self, flag: Flag) -> bool {
        match GameState::locate(flag) {
            Some((byte, bit)) => self.bank(flag.bank)[byte] & (1 << bit) != 0,
            None => false,
        }
    }

    /// Whether `flag` is clear.
    #[must_use]
    pub fn is_clear(&self, flag: Flag) -> bool {
        !self.is_set(flag)
    }

    /// Sets `flag`.
    ///
    /// # Errors
    ///
    /// [`MapError::FlagOutOfRange`] when the id is past its bank's capacity.
    pub fn set(&mut self, flag: Flag) -> Result<(), MapError> {
        self.write(flag, true)
    }

    /// Clears `flag`.
    ///
    /// # Errors
    ///
    /// [`MapError::FlagOutOfRange`] when the id is past its bank's capacity.
    pub fn clear(&mut self, flag: Flag) -> Result<(), MapError> {
        self.write(flag, false)
    }

    /// Sets or clears `flag`.
    ///
    /// # Errors
    ///
    /// [`MapError::FlagOutOfRange`] when the id is past its bank's capacity.
    pub fn write(&mut self, flag: Flag, value: bool) -> Result<(), MapError> {
        let Some((byte, bit)) = GameState::locate(flag) else {
            return Err(MapError::FlagOutOfRange {
                bank: flag.bank,
                id: flag.id,
                capacity: flag.bank.capacity(),
            });
        };
        let mask = 1 << bit;
        let cell = &mut self.bank_mut(flag.bank)[byte];
        if value {
            *cell |= mask;
        } else {
            *cell &= !mask;
        }
        Ok(())
    }

    /// The five party slots, `None` where the cartridge stores `$FF`.
    #[must_use]
    pub fn party(&self) -> [Option<CharId>; PARTY_SLOTS] {
        let mut out = [None; PARTY_SLOTS];
        for (slot, raw) in self.party.iter().enumerate() {
            if *raw != CharId::EMPTY {
                out[slot] = Some(CharId(*raw));
            }
        }
        out
    }

    /// How many members the party has, counting to the first empty slot.
    ///
    /// `CalcPartyNumber` (`ps4.asm:120885`) stops at the first `$FF` rather
    /// than scanning past gaps, so a hole truncates the party — reproduced
    /// here rather than "fixed".
    #[must_use]
    pub fn party_len(&self) -> usize {
        self.party
            .iter()
            .position(|&raw| raw == CharId::EMPTY)
            .unwrap_or(PARTY_SLOTS)
    }

    /// Reads one slot.
    #[must_use]
    pub fn party_slot(&self, slot: usize) -> Option<CharId> {
        match self.party.get(slot) {
            Some(&raw) if raw != CharId::EMPTY => Some(CharId(raw)),
            _ => None,
        }
    }

    /// Writes one slot. `None` empties it.
    ///
    /// # Errors
    ///
    /// [`MapError::PartySlotOutOfRange`] past [`PARTY_SLOTS`].
    pub fn set_party_slot(&mut self, slot: usize, who: Option<CharId>) -> Result<(), MapError> {
        if slot >= PARTY_SLOTS {
            return Err(MapError::PartySlotOutOfRange {
                slot,
                slots: PARTY_SLOTS,
            });
        }
        self.party[slot] = who.map_or(CharId::EMPTY, |c| c.0);
        Ok(())
    }

    /// Replaces the whole party at once.
    ///
    /// This is the shape scenes actually use: `Event_AlysFound` writes
    /// `(CharID_Alys << 8) | CharID_Chaz` as a *word* to `Current_Party_Slots`,
    /// which is simply slots 1 and 2 big-endian — Alys leading, Chaz behind.
    /// Bigger parties use the same trick wider: `move.l d0,(Current_Party_Slots)`
    /// fills slots 1-4 and `move.b d1,(Current_Party_Slot_5)` the fifth
    /// (`ps4.asm:88061-88063`). There is no packed word format to decode; the
    /// slots are five plain bytes.
    pub fn set_party(&mut self, slots: [Option<CharId>; PARTY_SLOTS]) {
        for (slot, who) in slots.iter().enumerate() {
            self.party[slot] = who.map_or(CharId::EMPTY, |c| c.0);
        }
    }

    /// Puts `who` in `slot`, shifting nobody. Returns who was displaced.
    ///
    /// # Errors
    ///
    /// [`MapError::PartySlotOutOfRange`] past [`PARTY_SLOTS`].
    pub fn join_party(&mut self, slot: usize, who: CharId) -> Result<Option<CharId>, MapError> {
        let displaced = self.party_slot(slot);
        self.set_party_slot(slot, Some(who))?;
        Ok(displaced)
    }

    /// A plain-data copy, for saving.
    #[must_use]
    pub fn snapshot(&self) -> StateSnapshot {
        let mut snapshot = StateSnapshot {
            event_flags: [0; 64],
            chest_flags: [0; 22],
            temp_flags: [0; 10],
            town_flags: [0; 16],
            party: self.party,
            money: self.money,
        };
        snapshot.event_flags.copy_from_slice(&self.event);
        snapshot.chest_flags.copy_from_slice(&self.chest);
        snapshot.temp_flags.copy_from_slice(&self.temp);
        snapshot.town_flags.copy_from_slice(&self.town);
        snapshot
    }

    /// Rebuilds a state from a snapshot. Total — every byte pattern is a valid
    /// flag bank, and party bytes are validated only when read.
    #[must_use]
    pub fn from_snapshot(snapshot: &StateSnapshot) -> GameState {
        GameState {
            event: snapshot.event_flags.to_vec(),
            chest: snapshot.chest_flags.to_vec(),
            temp: snapshot.temp_flags.to_vec(),
            town: snapshot.town_flags.to_vec(),
            party: snapshot.party,
            money: snapshot.money,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bit_order_is_most_significant_first() {
        // The cartridge's `7 - (id & 7)`: flag 0 is bit 7 of byte 0.
        let mut state = GameState::new();
        state.set(Flag::event(0)).unwrap();
        assert_eq!(state.snapshot().event_flags[0], 0b1000_0000);

        let mut state = GameState::new();
        state.set(Flag::event(7)).unwrap();
        assert_eq!(state.snapshot().event_flags[0], 0b0000_0001);

        let mut state = GameState::new();
        state.set(Flag::event(8)).unwrap();
        assert_eq!(state.snapshot().event_flags[1], 0b1000_0000);
    }

    #[test]
    fn banks_are_independent() {
        let mut state = GameState::new();
        state.set(Flag::chest(8)).unwrap();
        assert!(state.is_set(Flag::chest(8)));
        assert!(state.is_clear(Flag::event(8)));
        assert!(state.is_clear(Flag::temp(8)));
        assert!(state.is_clear(Flag::town(8)));
    }

    #[test]
    fn the_event_bank_spans_the_extended_range() {
        let mut state = GameState::new();
        // $100 is the first extended id; the cartridge subtracts $100 and uses
        // a second 32-byte array, which is contiguous with the first here.
        state.set(Flag::event(0x100)).unwrap();
        assert!(state.is_set(Flag::event(0x100)));
        assert!(state.is_clear(Flag::event(0)));
        assert_eq!(state.snapshot().event_flags[32], 0b1000_0000);
    }

    #[test]
    fn ids_past_a_banks_capacity_are_rejected_and_read_clear() {
        let mut state = GameState::new();
        assert!(matches!(
            state.set(Flag::temp(80)),
            Err(MapError::FlagOutOfRange { .. })
        ));
        assert!(state.is_clear(Flag::temp(80)));
        assert!(state.set(Flag::temp(79)).is_ok());
    }

    #[test]
    fn setting_and_clearing_round_trips() {
        let mut state = GameState::new();
        for id in 0..64 {
            state.set(Flag::event(id)).unwrap();
        }
        for id in 0..64 {
            assert!(state.is_set(Flag::event(id)), "flag {id}");
        }
        for id in (0..64).step_by(2) {
            state.clear(Flag::event(id)).unwrap();
        }
        for id in 0..64 {
            assert_eq!(state.is_set(Flag::event(id)), id % 2 == 1, "flag {id}");
        }
    }

    #[test]
    fn party_counts_to_the_first_empty_slot() {
        let mut state = GameState::new();
        assert_eq!(state.party_len(), 0);

        state.set_party([
            Some(CharId(1)),
            Some(CharId(2)),
            Some(CharId(3)),
            None,
            None,
        ]);
        assert_eq!(state.party_len(), 3);

        // A hole truncates, exactly as `CalcPartyNumber` does.
        state.set_party([Some(CharId(1)), None, Some(CharId(3)), None, None]);
        assert_eq!(state.party_len(), 1);
        assert_eq!(state.party_slot(2), Some(CharId(3)));
    }

    #[test]
    fn the_alys_found_write_puts_alys_in_front() {
        // Event_AlysFound writes (CharID_Alys << 8) | CharID_Chaz as a word to
        // Current_Party_Slots: big-endian, so the high byte lands in slot 1.
        const CHAZ: u8 = 0;
        const ALYS: u8 = 1;
        let word: u16 = (u16::from(ALYS) << 8) | u16::from(CHAZ);

        let mut state = GameState::new();
        state
            .set_party_slot(0, Some(CharId((word >> 8) as u8)))
            .unwrap();
        state.set_party_slot(1, Some(CharId(word as u8))).unwrap();

        assert_eq!(state.party_slot(0), Some(CharId(ALYS)), "Alys leads");
        assert_eq!(state.party_slot(1), Some(CharId(CHAZ)));
        assert_eq!(state.party_len(), 2);
    }

    #[test]
    fn snapshots_round_trip() {
        let mut state = GameState::new();
        state.set(Flag::event(0x42)).unwrap();
        state.set(Flag::chest(0x0D)).unwrap();
        state.set(Flag::temp(0x1B)).unwrap();
        state.set(Flag::town(3)).unwrap();
        state.set_party([Some(CharId(1)), Some(CharId(0)), None, None, None]);
        state.add_money(400);

        let restored = GameState::from_snapshot(&state.snapshot());
        assert_eq!(restored.money(), 400);
        assert_eq!(restored, state);
    }
}
