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
//! # Four banks, and two of the disassembly's labels are wrong
//!
//! The disassembly names five banks. Retail has **four**, and the clone
//! mislabels which addresses two of them live at. Both errors come from the
//! same place: the flag door block at `$057624` (test), `$057666` (set) and
//! `$0576A8` (clear), where the clone reconstructed one more entry point than
//! retail has and shifted every label below it.
//!
//! Decoding the set block straight out of the ROM gives four entries, ten
//! bytes apart, each a plain `movem.l` + `lea`:
//!
//! ```text
//! 0x057666:  lea $FFFFF100    event
//! 0x057670:  lea $FFFFF120    extended event -- and chest
//! 0x05767A:  lea $FFFFF140    temp event
//! 0x057684:  lea $FFFFF160    town
//! ```
//!
//! (Retail's `$F120` door has no `subi.w #$100, d0`; the clone shows one. The
//! door takes the raw id.)
//!
//! ## Chest flags are `$F120`, not `$F140`
//!
//! The whole chest system uses the `$F120` door. Three independent call sites,
//! read as bytes rather than labels:
//!
//! | site | ROM | target | bank |
//! |---|---|---|---|
//! | `LoadTreasureChests`, open-lid sprite at map load | `0x537E0` | `0x05762E` | `$F120` |
//! | `FieldRoutine_ItemFound`, "already open?" at entry | `0x66B2A` | `0x05762E` | `$F120` |
//! | `FieldRoutine_ItemFound`, the set on open | `0x66DE0` | `0x057670` | `$F120` |
//!
//! **So a chest flag and an extended event flag are the same bit**: `$F120` is
//! one bank carrying two name-spaces, and `Flag::chest(n)` is exactly
//! `Flag::event(0x100 + n)`.
//!
//! That is design rather than collision. Every one of the eleven literal-id
//! `$F120` test sites in the ROM reads a real chest's flag — the five tower
//! rings (`$A1`-`$A5`), plus `EclpsTorch`, `FradeMantl`, `Canceller`,
//! `PalmaRing`, `AeroPrism` and `RepairKit` at `$08`-`$0D`. Story logic gates
//! content on "has the player opened this chest" by testing the chest's own
//! flag.
//!
//! ## `$F140` is the temp bank, and there is no `$F156`
//!
//! The clone's `Temp_Event_Flags = $F156` is fiction — retail contains zero
//! instructions addressing it. The real temp bank is `$F140`, which the clone
//! labels `Chest_Flags`. Hardware confirms it: writing temp flag `$13` lands
//! at `$F142` bit 4, exactly where `bset 7-(id&7)` predicts.
//!
//! So temp flags and chest flags are **independent**. They never share a bit.
//!
//! ## Evidence chain
//!
//! Three independent lines, and the last one was blind:
//!
//! 1. the ROM door-block decode and the three chest call sites above;
//! 2. a pack census — all eleven ids the new-game initialiser pre-sets in
//!    `$F120` are real chests' flags, eleven for eleven;
//! 3. oracle tape 20, a whole-64KB RAM diff: `ChestFlag_PiataMonomate` (24)
//!    landing at `$FFFFF123` bit 7 simultaneously with the grant at
//!    `ItemFound+10`, with `$F140`-`$F17F` untouched — measured before the
//!    prediction reached them.
//!
//! `SOURCE_NOTES.md`'s "FLAG MODEL FINAL" entry is the record; entries it
//! supersedes are marked in place with their measurements preserved.
//!
//! ## Nothing ever clears a `$F120` bit
//!
//! The `$F120` clear door at `0x0576B2` has exactly one caller in the whole
//! ROM: `MapUpdate_ClrChestFlag`, which the disassembly itself annotates "Not
//! referenced" and which clears the unused id `$A9`. Dead code.
//!
//! A chest flag, once set, is permanent. One consequence is visible from the
//! first frame of a new game: the initialiser pre-sets eleven of them
//! (`$27 $28 $2A $34 $38 $50 $5B $5D $6B $78 $A7`), so those eleven chests
//! read as already opened and **can never be looted**. Seven of them hold a
//! HuntKnife across unrelated maps, which reads as placeholder records
//! deliberately switched off.
//!
//! ## Bank sizes
//!
//! From the RAM map (`ps4.constants.asm:2362-2367`), each running to the next
//! symbol retail actually addresses:
//!
//! | bank | address | bytes | flags |
//! |---|---|---:|---:|
//! | `Event_Flags` | `$F100` | 32 | 256 |
//! | `Extended_Event_Flags` (= chest) | `$F120` | 32 | 256 |
//! | temp event (`Chest_Flags` in the clone) | `$F140` | 32 | 256 |
//! | `Town_Flags` | `$F160` | 16 | 128 |
//!
//! The design doc's "~174 flags" is the count of *named* `EventFlag_*`
//! constants, not the addressable space; the trigger table's highest observed
//! event flag is `$E8` and the whole retail set fits the base bank.

use crate::chest::{Chest, ChestContents, ChestOutcome};
use crate::error::MapError;
use crate::inventory::{INVENTORY_SLOTS, Inventory};

/// Which bit array a flag lives in.
///
/// Three arrays, not four: there is no `Chest` variant, because chest flags
/// are not a bank. They are the upper half of [`FlagBank::Event`] — the
/// `$F120` bits, which the cartridge reaches through the same door as the
/// extended event flags. [`Flag::chest`] builds one. See the module docs for
/// the byte-level proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FlagBank {
    /// `Event_Flags` plus `Extended_Event_Flags`, one id space `$000..=$1FF`.
    Event,
    /// The temp event flags at `$F140`, which the disassembly mislabels
    /// `Chest_Flags`. Independent of everything else — a temp flag and a chest
    /// flag of the same id are different bits in different arrays.
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
            // $F100 and $F120, one id space $000..=$1FF. Ids from $100 up are
            // the same bits chest flags use.
            FlagBank::Event => 512,
            // $F140 to $F160, where the town bank starts.
            FlagBank::Temp => 256,
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

    /// A `ChestFlag_*` id — **the same bit** as `Flag::event(0x100 + id)`.
    ///
    /// Chest flags are not a bank of their own. They are the `$F120` half of
    /// the event id space, shared with the extended event flags, and the
    /// cartridge reaches them through the same door. Both constructors are
    /// kept because both names appear in the cartridge's data and a call site
    /// reads better saying which one it means.
    ///
    /// That sharing is deliberate: story logic gates content on "has the
    /// player opened this chest" by testing the chest's own flag. See the
    /// module docs.
    #[must_use]
    pub const fn chest(id: u16) -> Flag {
        Flag {
            bank: FlagBank::Event,
            id: 0x100 + id,
        }
    }

    /// A `TempEveFlag_*` id, in the `$F140` bank.
    ///
    /// Independent of [`Flag::chest`]: same id, different bit, different
    /// array. The disassembly labels this bank `Chest_Flags`, which is where
    /// an earlier revision of this engine got it wrong.
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
    /// The temp event flags at `$F140`.
    ///
    /// **Shape change, 2026-08-15 (second revision).** This field briefly held
    /// chest *and* temp flags together, on the belief that `$F140` was the
    /// chest bank. It is not: chest flags are the upper half of
    /// `event_flags`, and `$F140` carries only temp flags. A snapshot written
    /// under the previous shape has its chest bits in the wrong array and
    /// cannot be migrated by reshaping alone — the two name-spaces were never
    /// actually merged in RAM, so the bits are simply not there to recover.
    pub temp_flags: [u8; 32],
    /// `Town_Flags`.
    pub town_flags: [u8; 16],
    /// `Inventory` (`$F410`), forty item slots, `0` for empty.
    ///
    /// **Save-format addition, 2026-08-15.** A snapshot written before this
    /// existed has no item list; loading one should treat the party as
    /// carrying nothing, which is also what a new game starts with.
    pub inventory: [u8; INVENTORY_SLOTS],
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
/// such rule, and since retail's temp flags are simply bits in the `$F140`
/// bank the point is now trivially true: nothing bulk-clears them, not at map
/// load, not on save/load (the bank sits inside `SRAM_Buffer`, so it is saved
/// with everything else).
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
    /// `$F140`: the temp event flags.
    temp: Vec<u8>,
    town: Vec<u8>,
    /// `Inventory` (`$F410`), the party's forty item slots.
    inventory: Inventory,
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
            temp: vec![0; FlagBank::Temp.bytes()],
            town: vec![0; FlagBank::Town.bytes()],
            party: [CharId::EMPTY; PARTY_SLOTS],
            inventory: Inventory::new(),
            money: 0,
        }
    }

    /// The party's item list.
    #[must_use]
    pub const fn inventory(&self) -> &Inventory {
        &self.inventory
    }

    /// The party's item list, mutably.
    pub const fn inventory_mut(&mut self) -> &mut Inventory {
        &mut self.inventory
    }

    /// Opens `chest`, granting its contents and setting its flag.
    ///
    /// `FieldRoutine_ItemFound` (`ps4.asm:137246`) in order: test the flag and
    /// leave if it is set, grant, then set the flag. The grant comes *first*,
    /// and that ordering is load-bearing — a full inventory diverts to the swap
    /// path with the flag still clear, so the chest stays shut and can be
    /// opened again after the player makes room.
    pub fn open_chest(&mut self, chest: &Chest) -> ChestOutcome {
        if self.is_set(chest.chest_flag()) {
            return ChestOutcome::AlreadyOpen;
        }
        match chest.contents {
            ChestContents::Meseta(amount) => {
                self.add_money(amount);
                let _ = self.set(chest.chest_flag());
                ChestOutcome::Meseta { amount }
            }
            ChestContents::Item(item) => match self.inventory.add(item) {
                Ok(slot) => {
                    let _ = self.set(chest.chest_flag());
                    ChestOutcome::Took { item, slot }
                }
                // No flag, no grant: the chest is still shut.
                Err(_) => ChestOutcome::Full { item },
            },
        }
    }

    /// Completes a chest whose contents would not fit, by giving up whatever
    /// is in `slot`.
    ///
    /// The player has chosen what to drop, so the grant now succeeds and the
    /// flag is set — the same tail `open_chest` runs.
    ///
    /// # Errors
    ///
    /// [`MapError::InventoryFull`] if `slot` is out of range. An already-open
    /// chest yields [`ChestOutcome::AlreadyOpen`] without touching anything.
    pub fn complete_chest_swap(
        &mut self,
        chest: &Chest,
        slot: usize,
    ) -> Result<ChestOutcome, MapError> {
        if self.is_set(chest.chest_flag()) {
            return Ok(ChestOutcome::AlreadyOpen);
        }
        let ChestContents::Item(item) = chest.contents else {
            // Meseta never needs a slot, so this is the ordinary path.
            return Ok(self.open_chest(chest));
        };
        self.inventory.swap(slot, item)?;
        let _ = self.set(chest.chest_flag());
        Ok(ChestOutcome::Took { item, slot })
    }

    /// Whether `chest` has already been opened, which is what decides its
    /// sprite frame at map build.
    ///
    /// `LoadTreasureChests` calls `ChestFlags_Test` as it loads each chest and
    /// sets the open animation frame when it comes back set, so open-state is
    /// derived rather than stored.
    #[must_use]
    pub fn chest_is_open(&self, chest: &Chest) -> bool {
        self.is_set(chest.chest_flag())
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
            FlagBank::Temp => &self.temp,
            FlagBank::Town => &self.town,
        }
    }

    fn bank_mut(&mut self, bank: FlagBank) -> &mut [u8] {
        match bank {
            FlagBank::Event => &mut self.event,
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
            temp_flags: [0; 32],
            inventory: *self.inventory.slots(),
            town_flags: [0; 16],
            party: self.party,
            money: self.money,
        };
        snapshot.event_flags.copy_from_slice(&self.event);
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
            temp: snapshot.temp_flags.to_vec(),
            inventory: Inventory::from_slots(snapshot.inventory),
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
    fn the_three_banks_are_independent() {
        let mut state = GameState::new();
        state.set(Flag::chest(8)).unwrap();
        assert!(state.is_set(Flag::chest(8)));
        assert!(state.is_clear(Flag::event(8)));
        assert!(state.is_clear(Flag::town(8)));
    }

    #[test]
    fn a_chest_flag_is_the_same_bit_as_the_extended_event_flag_above_it() {
        // `$F120` is one bank with two name-spaces on it. The chest system
        // reaches it through the same door as the extended event flags — three
        // call sites, all `0x05762E`/`0x057670`, in the module docs.
        assert_eq!(Flag::chest(0x18), Flag::event(0x118));
        assert_eq!(Flag::chest(0).bank, FlagBank::Event);

        let mut state = GameState::new();
        state.set(Flag::chest(0x18)).unwrap();
        assert!(
            state.is_set(Flag::event(0x118)),
            "opening a chest is visible as an extended event flag"
        );

        // And back, including the clear — story code reads these bits.
        let mut state = GameState::new();
        state.set(Flag::event(0x118)).unwrap();
        assert!(state.is_set(Flag::chest(0x18)));
        state.clear(Flag::chest(0x18)).unwrap();
        assert!(state.is_clear(Flag::event(0x118)));
    }

    #[test]
    fn a_temp_flag_is_independent_of_the_chest_flag_of_that_id() {
        // The correction that cost the most: `$F140` is the temp bank, not the
        // chest bank. An earlier revision aliased these and was wrong in both
        // directions. Id 8 is `TempEveFlag_BioPlantAlarm` and
        // `ChestFlag_Alshline`; they never touch.
        let mut state = GameState::new();
        state.set(Flag::temp(8)).unwrap();
        assert!(state.is_clear(Flag::chest(8)), "different array");
        assert!(
            state.is_clear(Flag::event(8)),
            "and not an event flag either"
        );

        state.set(Flag::chest(8)).unwrap();
        assert!(state.is_set(Flag::temp(8)), "still set, untouched");

        state.clear(Flag::chest(8)).unwrap();
        assert!(state.is_set(Flag::temp(8)), "clearing one leaves the other");

        assert_ne!(Flag::temp(0x13), Flag::chest(0x13));
        assert_eq!(Flag::temp(8).bank, FlagBank::Temp);
    }

    #[test]
    fn the_eleven_preloaded_chests_read_as_open_at_a_new_game() {
        // The initialiser pre-sets eleven `$F120` bits, and every one is a real
        // chest's flag. Nothing in the cartridge clears a `$F120` bit — the
        // clear door's only caller is dead code — so these eleven chests are
        // unlootable from the first frame and stay that way.
        const PRELOADED: [u16; 11] = [
            0x27, 0x28, 0x2A, 0x34, 0x38, 0x50, 0x5B, 0x5D, 0x6B, 0x78, 0xA7,
        ];
        let mut state = GameState::new();
        for id in PRELOADED {
            state.set(Flag::chest(id)).unwrap();
        }
        for id in PRELOADED {
            assert!(state.is_set(Flag::chest(id)), "chest ${id:02X}");
            // Each is equally an extended event flag, which is how the
            // initialiser writes them.
            assert!(state.is_set(Flag::event(0x100 + id)));
            // And none of them touched the temp bank.
            assert!(state.is_clear(Flag::temp(id)), "temp ${id:02X} untouched");
        }
    }

    #[test]
    fn the_banks_hold_what_the_ram_map_gives_them() {
        // Four banks, not five. `$F140` runs to `$F160` where town starts, so
        // it is 32 bytes rather than the clone's 22 + 10 split.
        assert_eq!(FlagBank::Event.capacity(), 512, "$F100 + $F120");
        assert_eq!(FlagBank::Temp.capacity(), 256, "$F140..$F160");
        assert_eq!(FlagBank::Town.capacity(), 128, "$F160..$F170");

        // The highest id each bank accepts, and the first it rejects.
        let mut state = GameState::new();
        assert!(state.set(Flag::chest(255)).is_ok());
        assert!(matches!(
            // Chest 256 would be event $200, past the event bank's end.
            state.set(Flag::chest(256)),
            Err(MapError::FlagOutOfRange { .. })
        ));
        assert!(state.set(Flag::temp(255)).is_ok());
        assert!(matches!(
            state.set(Flag::temp(256)),
            Err(MapError::FlagOutOfRange { .. })
        ));
        assert!(state.set(Flag::event(511)).is_ok());
        assert!(state.set(Flag::town(127)).is_ok());
        assert!(matches!(
            state.set(Flag::town(128)),
            Err(MapError::FlagOutOfRange { .. })
        ));
    }

    fn item_chest(flag: u8, item: u8) -> Chest {
        Chest {
            cell: crate::geom::Cell::new(4, 4),
            flag,
            contents: ChestContents::Item(item),
            white: false,
            index: 0,
        }
    }

    #[test]
    fn opening_a_chest_grants_the_item_and_sets_its_flag() {
        let mut state = GameState::new();
        let chest = item_chest(24, 0x7D);
        assert!(!state.chest_is_open(&chest));

        assert_eq!(
            state.open_chest(&chest),
            ChestOutcome::Took {
                item: 0x7D,
                slot: 0
            }
        );
        assert_eq!(state.inventory().get(0), Some(0x7D));
        assert!(state.chest_is_open(&chest), "the flag records it");
    }

    #[test]
    fn a_chest_stays_open_and_cannot_be_looted_twice() {
        // Re-entering the map rebuilds the chest from the same flag, so this is
        // also what makes it draw open.
        let mut state = GameState::new();
        let chest = item_chest(24, 0x7D);
        state.open_chest(&chest);

        assert_eq!(state.open_chest(&chest), ChestOutcome::AlreadyOpen);
        assert_eq!(state.inventory().occupied(), 1, "no second copy");
        assert!(state.chest_is_open(&chest));
    }

    #[test]
    fn a_meseta_chest_pays_in_hundreds() {
        let mut state = GameState::new();
        let chest = Chest {
            contents: ChestContents::Meseta(400),
            ..item_chest(25, 0)
        };
        assert_eq!(
            state.open_chest(&chest),
            ChestOutcome::Meseta { amount: 400 }
        );
        assert_eq!(state.money(), 400);
        assert!(state.chest_is_open(&chest));
        assert_eq!(state.inventory().occupied(), 0, "meseta takes no slot");
    }

    #[test]
    fn a_full_inventory_leaves_the_chest_shut_until_the_swap() {
        // The grant happens before the flag is set, so a chest that could not
        // give up its contents is still closed and can be opened again later.
        let mut state = GameState::new();
        for id in 1..=40 {
            state.inventory_mut().add(id).unwrap();
        }
        let chest = item_chest(24, 0x7D);

        assert_eq!(state.open_chest(&chest), ChestOutcome::Full { item: 0x7D });
        assert!(!state.chest_is_open(&chest), "still shut");
        assert!(!state.inventory().contains(0x7D), "and nothing was granted");

        // The player drops slot 7 and the open completes.
        assert_eq!(
            state.complete_chest_swap(&chest, 7).unwrap(),
            ChestOutcome::Took {
                item: 0x7D,
                slot: 7
            }
        );
        assert_eq!(state.inventory().get(7), Some(0x7D));
        assert!(state.chest_is_open(&chest));
    }

    #[test]
    fn every_story_gated_id_is_a_real_chests_flag() {
        // The stronger of the two censuses, because it is behavioural rather
        // than statistical: these are the *only* ids in the ROM that reach the
        // `$F120` test door with a literal immediate, i.e. the only bits story
        // code asks about by name. Every one is a treasure chest's flag, which
        // is what makes "chest writes, story reads" the model rather than an
        // accident of overlapping id spaces.
        //
        // Ids and chests from the pack's `treasure_chests` records; the call
        // sites are the fifteen callers of `0x05762E`.
        const STORY_GATED: [(u16, &str); 11] = [
            (0x08, "TonoeBasement_B3 EclpsTorch"),
            (0x09, "LadeaTower_F5 FradeMantl"),
            (0x0A, "MachineCenter_B1 Canceller"),
            (0x0B, "Zelan_F1 PalmaRing"),
            (0x0C, "AirCastleInner_B1_Part3 AeroPrism"),
            (0x0D, "SoldiersTemple RepairKit"),
            (0xA1, "StrengthTower_F4 MotaRing"),
            (0xA2, "StrengthTower_F4 DezoRing"),
            (0xA3, "StrengthTower_F4 RykrRing"),
            (0xA4, "CourageTower_F4 AlgoRing"),
            (0xA5, "CourageTower_F4 MahlayRing"),
        ];

        let mut state = GameState::new();
        for (id, chest) in STORY_GATED {
            // The story test and the chest open are the same bit, so opening
            // the chest is what satisfies the story gate.
            state.set(Flag::chest(id)).unwrap();
            assert!(state.is_set(Flag::event(0x100 + id)), "{chest}");
            assert!(state.is_clear(Flag::temp(id)), "{chest}: not a temp flag");
        }
    }

    #[test]
    fn the_two_id_conventions_land_on_the_same_bit() {
        // The off-by-$100 killer. The pack emits the preloaded set as combined
        // ids (`0x127`), while a chest record names the same bit as `0x27`.
        // Retail's `$F120` door takes the id **raw** — it has no
        // `subi.w #$100, d0`, whatever the clone shows — so the caller owns the
        // offset, and this is where getting it wrong would hide.
        //
        // Constructing from each convention must reach one byte and one bit.
        for n in [0u16, 1, 7, 8, 0x27, 0x78, 0xA7, 0xFF] {
            let from_chest = Flag::chest(n);
            let from_combined = Flag::event(0x100 + n);
            assert_eq!(from_chest, from_combined, "id ${n:02X}");

            let mut a = GameState::new();
            a.set(from_chest).unwrap();
            let mut b = GameState::new();
            b.set(from_combined).unwrap();
            assert_eq!(
                a.snapshot().event_flags,
                b.snapshot().event_flags,
                "id ${n:02X}: same byte, same bit"
            );

            // And the bit is where the cartridge's `bset 7-(id&7)` puts it,
            // counted from `$F120` — byte 32 of the event array.
            let byte = 32 + (n as usize >> 3);
            let bit = 7 - (n & 7);
            assert_eq!(
                a.snapshot().event_flags[byte],
                1u8 << bit,
                "id ${n:02X} at $F1{:02X} bit {bit}",
                0x20 + (n >> 3)
            );
        }

        // A chest id must never reach the low half — that would be an event
        // flag of the same number, which is a different bit entirely.
        assert_ne!(Flag::chest(0x27), Flag::event(0x27));
    }

    #[test]
    fn the_snapshot_carries_four_banks() {
        // The save shape. `chest_flags` absorbed the old `temp_flags`, so a
        // stored five-bank snapshot migrates by concatenating chest then temp.
        let snapshot = GameState::new().snapshot();
        assert_eq!(snapshot.event_flags.len(), 64);
        assert_eq!(snapshot.temp_flags.len(), 32);
        assert_eq!(snapshot.town_flags.len(), 16);
        assert_eq!(snapshot.inventory.len(), 40, "$F410 to $F438");

        // A chest flag lands in the event array's upper half — `$F120`.
        let mut state = GameState::new();
        state.set(Flag::chest(0)).unwrap();
        assert_eq!(state.snapshot().event_flags[32], 0b1000_0000);

        // A temp flag lands in its own array at `$F140`.
        let mut state = GameState::new();
        state.set(Flag::temp(0)).unwrap();
        assert_eq!(state.snapshot().temp_flags[0], 0b1000_0000);

        // And the bytes the clone called `Temp_Event_Flags` are the ones from
        // `$F156` on — offset 22 into the temp array, not a bank of their own.
        let mut state = GameState::new();
        state.set(Flag::temp(22 * 8)).unwrap();
        assert_eq!(state.snapshot().temp_flags[22], 0b1000_0000);
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
            state.set(Flag::town(128)),
            Err(MapError::FlagOutOfRange { .. })
        ));
        assert!(state.is_clear(Flag::town(128)));
        assert!(state.set(Flag::town(127)).is_ok());
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
