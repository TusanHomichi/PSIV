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

use crate::battle::Stats;
use crate::chest::{Chest, ChestContents, ChestOutcome};
use crate::error::MapError;
use crate::inventory::{INVENTORY_SLOTS, Inventory};
use crate::roster::{CHARACTER_COUNT, CharacterRoster, REUNION_FLAG};

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

/// Number of persisted macro records at `$F444`.
pub const MACRO_COUNT: usize = 8;

/// Number of four-byte commands in one persisted macro record.
pub const MACRO_COMMANDS: usize = 5;

/// Number of saved vehicle records at `$FA80`.
pub const VEHICLE_COUNT: usize = 3;

/// Size of one saved vehicle record.
pub const VEHICLE_RECORD_BYTES: usize = 0x20;

/// One four-byte command inside a retail macro record.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MacroCommand {
    /// Character id, the first byte.
    pub character_id: u8,
    /// Retail command index, the second byte.
    pub command_index: u8,
    /// Technique, skill or item id, the third byte.
    pub ability_id: u8,
    /// The fourth byte is currently unused/alignment in retail.
    pub reserved: u8,
}

/// One 20-byte retail macro record: five four-byte commands.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MacroRecord {
    /// Commands in the order the battle macro interpreter reads them.
    pub commands: [MacroCommand; MACRO_COMMANDS],
}

/// One 0x20-byte saved Land Rover, Ice Digger or Hydrofoil record.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VehicleRecord {
    /// `$00`, current vehicle HP.
    pub current_hp: u16,
    /// `$02`, maximum vehicle HP.
    pub max_hp: u16,
    /// `$04`, the vehicle's skill availability mask.
    pub skill_mask: u8,
    /// `$05`, not read by any saved-vehicle consumer in the disassembly.
    pub reserved_05: u8,
    /// Even bytes `$06..$14`: current uses for eight vehicle skills.
    pub current_skill_uses: [u8; 8],
    /// Odd bytes `$07..$15`: maximum uses for eight vehicle skills.
    pub max_skill_uses: [u8; 8],
    /// `$16..$1F`, not read by any saved-vehicle consumer in the disassembly.
    pub reserved_tail: [u8; 10],
}

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
    /// `Character_Stats` (`$F500`-`$FA80`): the eleven records, whole.
    ///
    /// **Save-format addition, 2026-08-15 (delta #4).** The records are carried
    /// entire rather than field-picked, including the `_battle` tier — the save
    /// routine copies `$F100`..`$FB00` in one pass (`ps4.asm:134843`, annotated
    /// "save data for the Event Flags through the Vehicle Stats"), so retail
    /// saves those bytes too. They are stale between battles and simply never
    /// read while stale.
    ///
    /// A snapshot written before this existed has no roster; loading one should
    /// leave every seat empty and re-seat from the pack.
    pub characters: [Option<Stats>; CHARACTER_COUNT],
    /// `Current_Party_Slots`, `$FF` for an empty slot.
    pub party: [u8; PARTY_SLOTS],
    /// `Current_Money`.
    pub money: u32,
    /// `Vehicle_Index`, `$F43C`.
    pub vehicle_index: u16,
    /// `Button_Mappings_Index`, `$F43E`.
    pub button_mappings_index: u16,
    /// `Message_Speed`, `$F440`.
    pub message_speed: u16,
    /// `Battle_Speed`, `$F442`.
    pub battle_speed: u16,
    /// `Macro_Data`, `$F444..$F4E4`.
    pub macros: [MacroRecord; MACRO_COUNT],
    /// `Saved_Vehicle_Stats`, `$FA80..$FB00`.
    pub vehicles: [VehicleRecord; VEHICLE_COUNT],
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
    /// `Character_Stats` (`$F500`): the eleven character records.
    roster: CharacterRoster,
    party: [u8; PARTY_SLOTS],
    money: u32,
    vehicle_index: u16,
    button_mappings_index: u16,
    message_speed: u16,
    battle_speed: u16,
    macros: [MacroRecord; MACRO_COUNT],
    vehicles: [VehicleRecord; VEHICLE_COUNT],
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
        let mut macros = [MacroRecord::default(); MACRO_COUNT];
        macros[0].commands[0] = MacroCommand {
            character_id: 0,
            command_index: 1,
            ability_id: 0,
            reserved: 0,
        };
        GameState {
            event: vec![0; FlagBank::Event.bytes()],
            temp: vec![0; FlagBank::Temp.bytes()],
            town: vec![0; FlagBank::Town.bytes()],
            party: [CharId::EMPTY; PARTY_SLOTS],
            inventory: Inventory::new(),
            roster: CharacterRoster::new(),
            money: 0,
            vehicle_index: 0,
            button_mappings_index: 0,
            message_speed: 2,
            battle_speed: 2,
            macros,
            vehicles: [
                VehicleRecord {
                    current_hp: 0x02E4,
                    max_hp: 0x02E4,
                    skill_mask: 3,
                    reserved_05: 0,
                    current_skill_uses: [8, 8, 0, 0, 0, 0, 0, 0],
                    max_skill_uses: [8, 8, 0, 0, 0, 0, 0, 0],
                    reserved_tail: [0; 10],
                },
                VehicleRecord {
                    current_hp: 0x03C0,
                    max_hp: 0x03C0,
                    skill_mask: 0x50,
                    reserved_05: 0,
                    current_skill_uses: [8, 4, 0, 0, 0, 0, 0, 0],
                    max_skill_uses: [8, 4, 0, 0, 0, 0, 0, 0],
                    reserved_tail: [0; 10],
                },
                VehicleRecord {
                    current_hp: 0x02A8,
                    max_hp: 0x02A8,
                    skill_mask: 0x0C,
                    reserved_05: 0,
                    current_skill_uses: [8, 2, 0, 0, 0, 0, 0, 0],
                    max_skill_uses: [8, 2, 0, 0, 0, 0, 0, 0],
                    reserved_tail: [0; 10],
                },
            ],
        }
    }

    /// The eleven character records.
    #[must_use]
    pub const fn roster(&self) -> &CharacterRoster {
        &self.roster
    }

    /// The eleven character records, mutably.
    pub const fn roster_mut(&mut self) -> &mut CharacterRoster {
        &mut self.roster
    }

    /// The characters currently in the party, in slot order.
    #[must_use]
    pub fn party_members(&self) -> Vec<CharId> {
        self.party
            .iter()
            .filter(|id| **id != CharId::EMPTY)
            .map(|id| CharId(*id))
            .collect()
    }

    /// Both halves of `Battle_VictoryMessage`'s experience award.
    ///
    /// The party pass sets `gain_exp_flag` on every occupied slot and pays the
    /// living; the absent pass pays every other seated character carrying the
    /// flag, unless `EventFlag_Reunion` is set. Returns `(party, absent)` — the
    /// ids each pass actually paid.
    ///
    /// `each` is the per-head share `battle::split_rewards` computed, not the
    /// pool: the cartridge divides once and both passes add the same quotient.
    pub fn award_experience(&mut self, each: u16) -> (Vec<CharId>, Vec<CharId>) {
        let party = self.party_members();
        let reunion = self.is_set(Flag::event(REUNION_FLAG));
        let paid_party = self.roster.award_party(&party, each);
        let paid_absent = self.roster.award_absent(&party, each, reunion);
        (paid_party, paid_absent)
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

    /// The selected vehicle, `0` for on foot.
    #[must_use]
    pub const fn vehicle_index(&self) -> u16 {
        self.vehicle_index
    }

    /// Sets the selected vehicle index.
    pub const fn set_vehicle_index(&mut self, index: u16) {
        self.vehicle_index = index;
    }

    /// The persisted button mapping selector.
    #[must_use]
    pub const fn button_mappings_index(&self) -> u16 {
        self.button_mappings_index
    }

    /// Sets the persisted button mapping selector.
    pub const fn set_button_mappings_index(&mut self, index: u16) {
        self.button_mappings_index = index;
    }

    /// The persisted message speed setting.
    #[must_use]
    pub const fn message_speed(&self) -> u16 {
        self.message_speed
    }

    /// Sets the persisted message speed setting.
    pub const fn set_message_speed(&mut self, speed: u16) {
        self.message_speed = speed;
    }

    /// The persisted battle speed setting.
    #[must_use]
    pub const fn battle_speed(&self) -> u16 {
        self.battle_speed
    }

    /// Sets the persisted battle speed setting.
    pub const fn set_battle_speed(&mut self, speed: u16) {
        self.battle_speed = speed;
    }

    /// The eight persisted macro records.
    #[must_use]
    pub const fn macros(&self) -> &[MacroRecord; MACRO_COUNT] {
        &self.macros
    }

    /// The eight persisted macro records, mutably.
    pub const fn macros_mut(&mut self) -> &mut [MacroRecord; MACRO_COUNT] {
        &mut self.macros
    }

    /// The three persisted vehicle records.
    #[must_use]
    pub const fn vehicles(&self) -> &[VehicleRecord; VEHICLE_COUNT] {
        &self.vehicles
    }

    /// The three persisted vehicle records, mutably.
    pub const fn vehicles_mut(&mut self) -> &mut [VehicleRecord; VEHICLE_COUNT] {
        &mut self.vehicles
    }

    /// Retail `RecoverStats`: refill the current party's HP, TP and skill
    /// uses, clear ailments, then refill all three saved vehicle use banks.
    /// `DoVehicleRecovery` does not write saved vehicle HP.
    pub fn recover_stats(&mut self) {
        for who in self.party_members() {
            if let Some(stats) = self.roster_mut().get_mut(who) {
                stats.curr_hp = stats.max_hp;
                stats.curr_tp = stats.max_tp;
                stats.status = 0;
                stats.curr_skill_uses = stats.max_skill_uses;
            }
        }
        for vehicle in self.vehicles_mut() {
            vehicle.current_skill_uses = vehicle.max_skill_uses;
        }
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
    /// `loc_67CC6` searches from the start for the selected item ID (even
    /// when a later duplicate was selected), compacts once, then appends the
    /// found item to slot 39 of the full inventory and sets its chest flag.
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
        let discard = self.inventory.get(slot).ok_or(MapError::InventoryFull)?;
        let first = self
            .inventory
            .slots()
            .iter()
            .position(|id| *id == discard)
            .ok_or(MapError::InventoryFull)?;
        let mut inventory = self.inventory.clone();
        inventory.remove_in_field(first);
        let slot = inventory.add(item)?;
        self.inventory = inventory;
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
            characters: core::array::from_fn(|index| {
                self.roster.seats().get(index).cloned().flatten()
            }),
            inventory: *self.inventory.slots(),
            town_flags: [0; 16],
            party: self.party,
            money: self.money,
            vehicle_index: self.vehicle_index,
            button_mappings_index: self.button_mappings_index,
            message_speed: self.message_speed,
            battle_speed: self.battle_speed,
            macros: self.macros,
            vehicles: self.vehicles,
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
            roster: CharacterRoster::from_seats(&snapshot.characters),
            town: snapshot.town_flags.to_vec(),
            party: snapshot.party,
            money: snapshot.money,
            vehicle_index: snapshot.vehicle_index,
            button_mappings_index: snapshot.button_mappings_index,
            message_speed: snapshot.message_speed,
            battle_speed: snapshot.battle_speed,
            macros: snapshot.macros,
            vehicles: snapshot.vehicles,
        }
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
