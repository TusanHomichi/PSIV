//! Retail SRAM serialization for the persistent game state.
//!
//! The cartridge does not write a compact save format. `TransferToSRAM`
//! copies the logical range `$F100..$FB00` (2560 bytes) into one of three
//! interleaved SRAM blocks, and stores a wrapping byte sum in the common
//! header. This module keeps that layout on disk so the runtime can use the
//! same bytes for a save slot without teaching the game core about files.
//!
//! A disk slot is a self-contained retail-shaped slice: the 0x100-byte
//! logical header interleaved into 0x200 physical bytes, followed by one
//! 0xA00-byte logical payload interleaved into its 0x1400-byte retail block.
//! The common physical SRAM device is therefore represented by three files,
//! rather than by one shared 0x4000-byte device. The remaining file-level
//! divergence is recorded in `docs/SAVE_SCOUT.md`.

use core::fmt;

use crate::battle::{StatPair, StatTriple, Stats};
use crate::{
    INVENTORY_SLOTS, MacroCommand, MacroRecord, PARTY_SLOTS, StateSnapshot, VEHICLE_RECORD_BYTES,
    VehicleRecord,
};

/// The number of save files exposed by the retail title and save menu.
pub const RETAIL_SLOT_COUNT: usize = 3;
/// Logical bytes in the SRAM header at `$F000..$F100`.
pub const RETAIL_HEADER_LOGICAL_BYTES: usize = 0x100;
/// Physical bytes occupied by the interleaved SRAM header.
pub const RETAIL_HEADER_PHYSICAL_BYTES: usize = 0x200;
/// Logical payload bytes copied from `$F100` through `$FB00`.
pub const RETAIL_PAYLOAD_BYTES: usize = 0xA00;
/// Physical stride reserved for one interleaved retail payload block.
pub const RETAIL_SLOT_STRIDE_BYTES: usize = 0x1400;
/// Physical bytes in one self-contained disk slot.
pub const RETAIL_SLOT_FILE_BYTES: usize = RETAIL_HEADER_PHYSICAL_BYTES + RETAIL_SLOT_STRIDE_BYTES;

const SIGNATURE_OFFSET: usize = 0x02;
const SAVE_SLOT_OFFSET: usize = 0x12;
const CHECKSUM_OFFSET: usize = 0x14;
// This is outside every retail header field currently addressed by the ROM.
// It carries occupancy for the Rust snapshot's Option<Stats> seats.
const RUST_ROSTER_MASK_OFFSET: usize = 0x20;
// A native-only marker and complemented byte in unused common-header space.
// The cartridge never saves ED51; native restart preserves inherited exits.
const RUST_DUNGEON_EXIT_OFFSET: usize = 0x22;
const LOCATION_WORLD_OFFSET: usize = 0x300;
const LOCATION_MAP_2_OFFSET: usize = 0x302;
const LOCATION_MAP_OFFSET: usize = 0x304;
const LOCATION_X_OFFSET: usize = 0x306;
const LOCATION_Y_OFFSET: usize = 0x308;
const PARTY_OFFSET: usize = 0x30A;
const INVENTORY_OFFSET: usize = 0x310;
const MONEY_OFFSET: usize = 0x338;
const VEHICLE_INDEX_OFFSET: usize = 0x33C;
const BUTTON_MAPPINGS_INDEX_OFFSET: usize = 0x33E;
const MESSAGE_SPEED_OFFSET: usize = 0x340;
const BATTLE_SPEED_OFFSET: usize = 0x342;
const MACRO_OFFSET: usize = 0x344;
const MACRO_RECORD_BYTES: usize = 0x14;
const CHARACTER_OFFSET: usize = 0x400;
const CHARACTER_STRIDE: usize = 0x80;
const CHARACTER_RECORD_BYTES: usize = 0x80;
const VEHICLE_OFFSET: usize = 0x980;
const SIGNATURE: &[u8; 15] = b"PHANTASY STAR 4";

/// The location fields the retail load path restores before entering the
/// field. Coordinates are pixel words, as they are at `$F406`/`$F408`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RetailLocation {
    /// `$F400`, the world/planet index.
    pub world_index: u16,
    /// `$F402`, the secondary field-map index.
    pub map_index_2: u16,
    /// `$F404`, the primary field-map index.
    pub map_index: u16,
    /// `$F406`, the saved leader x position in pixels.
    pub char_x: u16,
    /// `$F408`, the saved leader y position in pixels.
    pub char_y: u16,
}

/// A snapshot plus the retail location needed to boot it into the field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetailSave {
    /// Persistent flags, inventory, roster, party and money.
    pub snapshot: StateSnapshot,
    /// Map and leader position restored by the title continue path.
    pub location: RetailLocation,
}

/// A malformed or corrupt retail-shaped save slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveError {
    /// A slot index is outside the three visible retail slots.
    InvalidSlot {
        /// The requested zero-based slot.
        slot: usize,
        /// The number of supported slots.
        slots: usize,
    },
    /// A byte slice is not the expected retail-shaped length.
    InvalidLength {
        /// The required byte count.
        expected: usize,
        /// The received byte count.
        actual: usize,
    },
    /// The common SRAM signature is not `PHANTASY STAR 4`.
    InvalidSignature,
    /// The payload sum does not match the header's stored word.
    ChecksumMismatch {
        /// The slot whose checksum failed.
        slot: usize,
        /// The checksum stored in the interleaved header.
        expected: u16,
        /// The checksum calculated from `$F100..$FB00`.
        actual: u16,
    },
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SaveError::InvalidSlot { slot, slots } => {
                write!(f, "save slot {slot} is outside 0..{slots}")
            }
            SaveError::InvalidLength { expected, actual } => {
                write!(f, "save slot has {actual} bytes; expected {expected}")
            }
            SaveError::InvalidSignature => write!(f, "save slot signature is not PHANTASY STAR 4"),
            SaveError::ChecksumMismatch {
                slot,
                expected,
                actual,
            } => write!(
                f,
                "save slot {slot} checksum is {actual:#06x}; header says {expected:#06x}"
            ),
        }
    }
}

impl std::error::Error for SaveError {}

/// A validated, retail-interleaved slot file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetailSlot {
    bytes: [u8; RETAIL_SLOT_FILE_BYTES],
}

impl RetailSlot {
    /// Encodes a save into the selected retail slot shape.
    pub fn encode(save: &RetailSave, slot: usize) -> Result<RetailSlot, SaveError> {
        validate_slot(slot)?;
        let mut payload = save.snapshot.to_retail_payload();
        write_u16(
            &mut payload,
            LOCATION_WORLD_OFFSET,
            save.location.world_index,
        );
        write_u16(
            &mut payload,
            LOCATION_MAP_2_OFFSET,
            save.location.map_index_2,
        );
        write_u16(&mut payload, LOCATION_MAP_OFFSET, save.location.map_index);
        write_u16(&mut payload, LOCATION_X_OFFSET, save.location.char_x);
        write_u16(&mut payload, LOCATION_Y_OFFSET, save.location.char_y);

        let mut header = [0; RETAIL_HEADER_LOGICAL_BYTES];
        header[SIGNATURE_OFFSET..SIGNATURE_OFFSET + SIGNATURE.len()].copy_from_slice(SIGNATURE);
        write_u16(&mut header, SAVE_SLOT_OFFSET, slot as u16);
        write_u16(
            &mut header,
            CHECKSUM_OFFSET + slot * 2,
            payload_checksum(&payload),
        );
        let roster_mask = save
            .snapshot
            .characters
            .iter()
            .enumerate()
            .fold(0_u16, |mask, (index, character)| {
                mask | u16::from(character.is_some()) << index
            });
        write_u16(&mut header, RUST_ROSTER_MASK_OFFSET, roster_mask);

        let mut bytes = [0; RETAIL_SLOT_FILE_BYTES];
        interleave(&header, &mut bytes[..RETAIL_HEADER_PHYSICAL_BYTES]);
        interleave(
            &payload,
            &mut bytes[RETAIL_HEADER_PHYSICAL_BYTES..RETAIL_SLOT_FILE_BYTES],
        );
        Ok(RetailSlot { bytes })
    }

    /// Validates a disk slot's length, signature and retail checksum.
    pub fn from_bytes(bytes: &[u8], slot: usize) -> Result<RetailSlot, SaveError> {
        validate_slot(slot)?;
        if bytes.len() != RETAIL_SLOT_FILE_BYTES {
            return Err(SaveError::InvalidLength {
                expected: RETAIL_SLOT_FILE_BYTES,
                actual: bytes.len(),
            });
        }
        let mut owned = [0; RETAIL_SLOT_FILE_BYTES];
        owned.copy_from_slice(bytes);
        let header =
            deinterleave::<RETAIL_HEADER_LOGICAL_BYTES>(&owned[..RETAIL_HEADER_PHYSICAL_BYTES]);
        if &header[SIGNATURE_OFFSET..SIGNATURE_OFFSET + SIGNATURE.len()] != SIGNATURE {
            return Err(SaveError::InvalidSignature);
        }
        let payload = deinterleave::<RETAIL_PAYLOAD_BYTES>(
            &owned[RETAIL_HEADER_PHYSICAL_BYTES..RETAIL_SLOT_FILE_BYTES],
        );
        let expected = read_u16(&header, CHECKSUM_OFFSET + slot * 2);
        let actual = payload_checksum(&payload);
        if expected != actual {
            return Err(SaveError::ChecksumMismatch {
                slot,
                expected,
                actual,
            });
        }
        Ok(RetailSlot { bytes: owned })
    }

    /// Applies the retail title's per-slot erase operation to a raw slot file.
    ///
    /// `Title_EraseOption` calls `loc_64DC0`, which zeros the selected
    /// `$200200 + slot * $1400` physical payload block. It does not rewrite
    /// the common header, signature, or checksum table; validation notices the
    /// stale checksum on the next title pass.
    pub fn erase_physical_payload(bytes: &[u8], slot: usize) -> Result<RetailSlot, SaveError> {
        validate_slot(slot)?;
        if bytes.len() != RETAIL_SLOT_FILE_BYTES {
            return Err(SaveError::InvalidLength {
                expected: RETAIL_SLOT_FILE_BYTES,
                actual: bytes.len(),
            });
        }
        let mut owned = [0; RETAIL_SLOT_FILE_BYTES];
        owned.copy_from_slice(bytes);
        owned[RETAIL_HEADER_PHYSICAL_BYTES..].fill(0);
        Ok(RetailSlot { bytes: owned })
    }

    /// Returns the exact physical bytes represented by this slot file.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; RETAIL_SLOT_FILE_BYTES] {
        &self.bytes
    }

    /// Preserve the live dungeon exit in the native header extension. Retail
    /// payload bytes and their checksum stay unchanged; old slots omit it.
    pub fn with_dungeon_exit(mut self, index: u8) -> Self {
        let offset = RUST_DUNGEON_EXIT_OFFSET * 2 + 1;
        for (i, byte) in [b'T', b'R', index, !index].into_iter().enumerate() {
            self.bytes[offset + i * 2] = byte;
        }
        self
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

    /// Returns the checksum recorded in this slot's header.
    pub fn checksum(&self, slot: usize) -> Result<u16, SaveError> {
        validate_slot(slot)?;
        let header = deinterleave::<RETAIL_HEADER_LOGICAL_BYTES>(
            &self.bytes[..RETAIL_HEADER_PHYSICAL_BYTES],
        );
        Ok(read_u16(&header, CHECKSUM_OFFSET + slot * 2))
    }

    /// Decodes the validated slot into game state and a boot location.
    pub fn decode(&self) -> Result<RetailSave, SaveError> {
        let header = deinterleave::<RETAIL_HEADER_LOGICAL_BYTES>(
            &self.bytes[..RETAIL_HEADER_PHYSICAL_BYTES],
        );
        let payload = deinterleave::<RETAIL_PAYLOAD_BYTES>(
            &self.bytes[RETAIL_HEADER_PHYSICAL_BYTES..RETAIL_SLOT_FILE_BYTES],
        );
        let roster_mask = read_u16(&header, RUST_ROSTER_MASK_OFFSET);
        let snapshot = snapshot_from_payload(&payload, Some(roster_mask));
        Ok(RetailSave {
            snapshot,
            location: RetailLocation {
                world_index: read_u16(&payload, LOCATION_WORLD_OFFSET),
                map_index_2: read_u16(&payload, LOCATION_MAP_2_OFFSET),
                map_index: read_u16(&payload, LOCATION_MAP_OFFSET),
                char_x: read_u16(&payload, LOCATION_X_OFFSET),
                char_y: read_u16(&payload, LOCATION_Y_OFFSET),
            },
        })
    }
}

impl StateSnapshot {
    /// Serializes the state into the exact logical `$F100..$FB00` payload.
    ///
    /// All modeled retail state is written at its retail offset. The remaining
    /// bytes in a character record are the four-byte tail the disassembly never
    /// addresses and stay zero, as retail's character initializer leaves them.
    #[must_use]
    pub fn to_retail_payload(&self) -> [u8; RETAIL_PAYLOAD_BYTES] {
        encode_snapshot(self)
    }

    /// Parses a logical retail payload, treating an all-zero character record
    /// as an unseated roster slot. `RetailSlot::decode` uses its Rust-only
    /// occupancy mask so even an intentionally all-zero `Stats` survives.
    pub fn from_retail_payload(payload: &[u8]) -> Result<StateSnapshot, SaveError> {
        if payload.len() != RETAIL_PAYLOAD_BYTES {
            return Err(SaveError::InvalidLength {
                expected: RETAIL_PAYLOAD_BYTES,
                actual: payload.len(),
            });
        }
        let mut owned = [0; RETAIL_PAYLOAD_BYTES];
        owned.copy_from_slice(payload);
        Ok(snapshot_from_payload(&owned, None))
    }
}

fn encode_snapshot(snapshot: &StateSnapshot) -> [u8; RETAIL_PAYLOAD_BYTES] {
    let mut payload = [0; RETAIL_PAYLOAD_BYTES];
    payload[..snapshot.event_flags.len()].copy_from_slice(&snapshot.event_flags);
    let temp_end = 0x40 + snapshot.temp_flags.len();
    payload[0x40..temp_end].copy_from_slice(&snapshot.temp_flags);
    let town_end = 0x60 + snapshot.town_flags.len();
    payload[0x60..town_end].copy_from_slice(&snapshot.town_flags);
    payload[PARTY_OFFSET..PARTY_OFFSET + PARTY_SLOTS].copy_from_slice(&snapshot.party);
    payload[INVENTORY_OFFSET..INVENTORY_OFFSET + INVENTORY_SLOTS]
        .copy_from_slice(&snapshot.inventory);
    write_u32(&mut payload, MONEY_OFFSET, snapshot.money);
    write_u16(&mut payload, VEHICLE_INDEX_OFFSET, snapshot.vehicle_index);
    write_u16(
        &mut payload,
        BUTTON_MAPPINGS_INDEX_OFFSET,
        snapshot.button_mappings_index,
    );
    write_u16(&mut payload, MESSAGE_SPEED_OFFSET, snapshot.message_speed);
    write_u16(&mut payload, BATTLE_SPEED_OFFSET, snapshot.battle_speed);
    for (index, record) in snapshot.macros.iter().enumerate() {
        encode_macro(
            record,
            &mut payload[MACRO_OFFSET + index * MACRO_RECORD_BYTES
                ..MACRO_OFFSET + (index + 1) * MACRO_RECORD_BYTES],
        );
    }
    for (index, character) in snapshot.characters.iter().enumerate() {
        if let Some(character) = character {
            encode_stats(
                character,
                &mut payload[CHARACTER_OFFSET + index * CHARACTER_STRIDE
                    ..CHARACTER_OFFSET + index * CHARACTER_STRIDE + CHARACTER_RECORD_BYTES],
            );
        }
    }
    for (index, vehicle) in snapshot.vehicles.iter().enumerate() {
        encode_vehicle(
            vehicle,
            &mut payload[VEHICLE_OFFSET + index * VEHICLE_RECORD_BYTES
                ..VEHICLE_OFFSET + (index + 1) * VEHICLE_RECORD_BYTES],
        );
    }
    payload
}

fn snapshot_from_payload(
    payload: &[u8; RETAIL_PAYLOAD_BYTES],
    roster_mask: Option<u16>,
) -> StateSnapshot {
    let mut event_flags = [0; 64];
    event_flags.copy_from_slice(&payload[..64]);
    let mut temp_flags = [0; 32];
    temp_flags.copy_from_slice(&payload[0x40..0x60]);
    let mut town_flags = [0; 16];
    town_flags.copy_from_slice(&payload[0x60..0x70]);
    let mut party = [0; PARTY_SLOTS];
    party.copy_from_slice(&payload[PARTY_OFFSET..PARTY_OFFSET + PARTY_SLOTS]);
    let mut inventory = [0; INVENTORY_SLOTS];
    inventory.copy_from_slice(&payload[INVENTORY_OFFSET..INVENTORY_OFFSET + INVENTORY_SLOTS]);
    let macros = core::array::from_fn(|index| {
        let start = MACRO_OFFSET + index * MACRO_RECORD_BYTES;
        decode_macro(&payload[start..start + MACRO_RECORD_BYTES])
    });
    let vehicles = core::array::from_fn(|index| {
        let start = VEHICLE_OFFSET + index * VEHICLE_RECORD_BYTES;
        decode_vehicle(&payload[start..start + VEHICLE_RECORD_BYTES])
    });
    let characters = core::array::from_fn(|index| {
        let start = CHARACTER_OFFSET + index * CHARACTER_STRIDE;
        let bytes = &payload[start..start + CHARACTER_RECORD_BYTES];
        let occupied = roster_mask.map_or_else(
            || bytes.iter().any(|byte| *byte != 0),
            |mask| mask & (1 << index) != 0,
        );
        occupied.then(|| decode_stats(bytes))
    });
    StateSnapshot {
        event_flags,
        temp_flags,
        characters,
        inventory,
        town_flags,
        party,
        money: read_u32(payload, MONEY_OFFSET),
        vehicle_index: read_u16(payload, VEHICLE_INDEX_OFFSET),
        button_mappings_index: read_u16(payload, BUTTON_MAPPINGS_INDEX_OFFSET),
        message_speed: read_u16(payload, MESSAGE_SPEED_OFFSET),
        battle_speed: read_u16(payload, BATTLE_SPEED_OFFSET),
        macros,
        vehicles,
    }
}

fn encode_stats(stats: &Stats, bytes: &mut [u8]) {
    bytes[0x00..0x06].copy_from_slice(&stats.name_bytes);
    write_u16(bytes, 0x06, stats.profession);
    write_u16(bytes, 0x08, stats.level);
    write_u32(bytes, 0x0A, stats.experience);
    write_u16(bytes, 0x0E, stats.curr_hp);
    write_u16(bytes, 0x10, stats.max_hp);
    write_u16(bytes, 0x12, stats.curr_tp);
    write_u16(bytes, 0x14, stats.max_tp);
    bytes[0x16] = stats.status;
    encode_triple(bytes, 0x18, stats.strength);
    encode_triple(bytes, 0x1B, stats.mental);
    encode_triple(bytes, 0x1E, stats.agility);
    encode_triple(bytes, 0x21, stats.dexterity);
    encode_pair(bytes, 0x24, stats.attack);
    encode_pair(bytes, 0x28, stats.defence);
    encode_pair(bytes, 0x2C, stats.mental_defence);
    for (index, (&property, &shadow)) in stats
        .element_props
        .iter()
        .zip(stats.element_shadow.iter())
        .enumerate()
    {
        bytes[0x30 + index * 2] = property;
        bytes[0x31 + index * 2] = shadow;
    }
    bytes[0x4C..0x50].copy_from_slice(&stats.equipment);
    bytes[0x50..0x52].copy_from_slice(&stats.weapon_elements);
    bytes[0x52..0x62].copy_from_slice(&stats.techniques);
    bytes[0x62..0x6A].copy_from_slice(&stats.skills);
    for (index, (&current, &maximum)) in stats
        .curr_skill_uses
        .iter()
        .zip(stats.max_skill_uses.iter())
        .enumerate()
    {
        bytes[0x6A + index * 2] = current;
        bytes[0x6B + index * 2] = maximum;
    }
    bytes[0x7A] = u8::from(stats.gain_exp_flag);
    bytes[0x7B] = stats.physical_prop_save;
}

fn decode_stats(bytes: &[u8]) -> Stats {
    let element_props = core::array::from_fn(|index| bytes[0x30 + index * 2]);
    let element_shadow = core::array::from_fn(|index| bytes[0x31 + index * 2]);
    let name_bytes = core::array::from_fn(|index| bytes[index]);
    let techniques = core::array::from_fn(|index| bytes[0x52 + index]);
    let skills = core::array::from_fn(|index| bytes[0x62 + index]);
    let curr_skill_uses = core::array::from_fn(|index| bytes[0x6A + index * 2]);
    let max_skill_uses = core::array::from_fn(|index| bytes[0x6B + index * 2]);
    Stats {
        name_bytes,
        profession: read_u16(bytes, 0x06),
        level: read_u16(bytes, 0x08),
        experience: read_u32(bytes, 0x0A),
        curr_hp: read_u16(bytes, 0x0E),
        max_hp: read_u16(bytes, 0x10),
        curr_tp: read_u16(bytes, 0x12),
        max_tp: read_u16(bytes, 0x14),
        status: bytes[0x16],
        strength: decode_triple(bytes, 0x18),
        mental: decode_triple(bytes, 0x1B),
        agility: decode_triple(bytes, 0x1E),
        dexterity: decode_triple(bytes, 0x21),
        attack: decode_pair(bytes, 0x24),
        defence: decode_pair(bytes, 0x28),
        mental_defence: decode_pair(bytes, 0x2C),
        element_props,
        element_shadow,
        weapon_elements: [bytes[0x50], bytes[0x51]],
        equipment: [bytes[0x4C], bytes[0x4D], bytes[0x4E], bytes[0x4F]],
        techniques,
        skills,
        curr_skill_uses,
        max_skill_uses,
        physical_prop_save: bytes[0x7B],
        enemy_id: 0,
        gain_exp_flag: bytes[0x7A] != 0,
    }
}

fn encode_macro(record: &MacroRecord, bytes: &mut [u8]) {
    for (index, command) in record.commands.iter().enumerate() {
        let offset = index * 4;
        bytes[offset] = command.character_id;
        bytes[offset + 1] = command.command_index;
        bytes[offset + 2] = command.ability_id;
        bytes[offset + 3] = command.reserved;
    }
}

fn decode_macro(bytes: &[u8]) -> MacroRecord {
    MacroRecord {
        commands: core::array::from_fn(|index| {
            let offset = index * 4;
            MacroCommand {
                character_id: bytes[offset],
                command_index: bytes[offset + 1],
                ability_id: bytes[offset + 2],
                reserved: bytes[offset + 3],
            }
        }),
    }
}

fn encode_vehicle(record: &VehicleRecord, bytes: &mut [u8]) {
    write_u16(bytes, 0x00, record.current_hp);
    write_u16(bytes, 0x02, record.max_hp);
    bytes[0x04] = record.skill_mask;
    bytes[0x05] = record.reserved_05;
    for (index, (&current, &maximum)) in record
        .current_skill_uses
        .iter()
        .zip(record.max_skill_uses.iter())
        .enumerate()
    {
        bytes[0x06 + index * 2] = current;
        bytes[0x07 + index * 2] = maximum;
    }
    bytes[0x16..0x20].copy_from_slice(&record.reserved_tail);
}

fn decode_vehicle(bytes: &[u8]) -> VehicleRecord {
    VehicleRecord {
        current_hp: read_u16(bytes, 0x00),
        max_hp: read_u16(bytes, 0x02),
        skill_mask: bytes[0x04],
        reserved_05: bytes[0x05],
        current_skill_uses: core::array::from_fn(|index| bytes[0x06 + index * 2]),
        max_skill_uses: core::array::from_fn(|index| bytes[0x07 + index * 2]),
        reserved_tail: core::array::from_fn(|index| bytes[0x16 + index]),
    }
}

fn encode_triple(bytes: &mut [u8], offset: usize, value: StatTriple) {
    bytes[offset] = value.base;
    bytes[offset + 1] = value.modified;
    bytes[offset + 2] = value.battle;
}

fn decode_triple(bytes: &[u8], offset: usize) -> StatTriple {
    StatTriple {
        base: bytes[offset],
        modified: bytes[offset + 1],
        battle: bytes[offset + 2],
    }
}

fn encode_pair(bytes: &mut [u8], offset: usize, value: StatPair) {
    write_u16(bytes, offset, value.derived);
    write_u16(bytes, offset + 2, value.battle);
}

fn decode_pair(bytes: &[u8], offset: usize) -> StatPair {
    StatPair {
        derived: read_u16(bytes, offset),
        battle: read_u16(bytes, offset + 2),
    }
}

fn validate_slot(slot: usize) -> Result<(), SaveError> {
    if slot < RETAIL_SLOT_COUNT {
        Ok(())
    } else {
        Err(SaveError::InvalidSlot {
            slot,
            slots: RETAIL_SLOT_COUNT,
        })
    }
}

fn payload_checksum(payload: &[u8; RETAIL_PAYLOAD_BYTES]) -> u16 {
    payload
        .iter()
        .fold(0_u16, |sum, byte| sum.wrapping_add(u16::from(*byte)))
}

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn interleave<const N: usize>(logical: &[u8; N], physical: &mut [u8]) {
    debug_assert_eq!(physical.len(), N * 2);
    for (index, byte) in logical.iter().enumerate() {
        physical[1 + index * 2] = *byte;
    }
}

fn deinterleave<const N: usize>(physical: &[u8]) -> [u8; N] {
    debug_assert_eq!(physical.len(), N * 2);
    let mut logical = [0; N];
    for (index, byte) in logical.iter_mut().enumerate() {
        *byte = physical[1 + index * 2];
    }
    logical
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::{StatPair, StatTriple};
    use crate::{CHARACTER_COUNT, CharId, Flag, GameState, MacroCommand, VehicleRecord};

    fn stats(index: u8) -> Stats {
        Stats {
            name_bytes: [index, index + 1, index + 2, index + 3, 0xFE, 0],
            profession: u16::from(index),
            level: u16::from(index) + 1,
            experience: 1000 + u32::from(index),
            curr_hp: 30 - u16::from(index),
            max_hp: 40 + u16::from(index),
            curr_tp: 12 + u16::from(index),
            max_tp: 20 + u16::from(index),
            status: index & 3,
            strength: StatTriple {
                base: index,
                modified: index + 1,
                battle: index + 2,
            },
            mental: StatTriple::uniform(index + 3),
            agility: StatTriple::uniform(index + 4),
            dexterity: StatTriple::uniform(index + 5),
            attack: StatPair::uniform(100 + u16::from(index)),
            defence: StatPair {
                derived: 200 + u16::from(index),
                battle: 190 + u16::from(index),
            },
            mental_defence: StatPair::uniform(300 + u16::from(index)),
            element_props: [index; 14],
            element_shadow: [index + 1; 14],
            weapon_elements: [index, index + 1],
            equipment: [index, index + 1, index + 2, index + 3],
            techniques: [index + 10; 16],
            skills: [index + 20; 8],
            curr_skill_uses: [index + 30; 8],
            max_skill_uses: [index + 40; 8],
            physical_prop_save: index + 4,
            enemy_id: 0,
            gain_exp_flag: index.is_multiple_of(2),
        }
    }

    fn mid_progress_state() -> GameState {
        let mut state = GameState::new();
        state.set(Flag::event(0x12)).unwrap();
        state.set(Flag::event(0x118)).unwrap();
        state.set(Flag::temp(0x24)).unwrap();
        state.set(Flag::town(0x2A)).unwrap();
        state.inventory_mut().add(0x7D).unwrap();
        state.inventory_mut().add(0x22).unwrap();
        state.add_money(4321);
        state.set_party([Some(CharId(0)), Some(CharId(1)), None, None, None]);
        for index in 0..CHARACTER_COUNT {
            state
                .roster_mut()
                .seat(CharId(index as u8), stats(index as u8))
                .unwrap();
        }
        state.roster_mut().get_mut(CharId(0)).unwrap().curr_hp = 3;
        state.roster_mut().get_mut(CharId(0)).unwrap().status = 1;
        state.roster_mut().get_mut(CharId(0)).unwrap().equipment = [0x21, 0x22, 0x23, 0x24];
        state.set_vehicle_index(2);
        state.set_button_mappings_index(4);
        state.set_message_speed(3);
        state.set_battle_speed(1);
        state.macros_mut()[3].commands[2] = MacroCommand {
            character_id: 4,
            command_index: 2,
            ability_id: 31,
            reserved: 0xA5,
        };
        state.vehicles_mut()[1] = VehicleRecord {
            current_hp: 0x0123,
            max_hp: 0x0456,
            skill_mask: 0xA5,
            reserved_05: 0x5A,
            current_skill_uses: [1, 2, 3, 4, 5, 6, 7, 8],
            max_skill_uses: [8, 7, 6, 5, 4, 3, 2, 1],
            reserved_tail: [0xCC; 10],
        };
        state
    }

    #[test]
    fn logical_payload_round_trips_byte_for_byte() {
        let payload = mid_progress_state().snapshot().to_retail_payload();
        let decoded = StateSnapshot::from_retail_payload(&payload).unwrap();
        assert_eq!(decoded.to_retail_payload(), payload);
    }

    #[test]
    fn retail_slot_round_trips_the_whole_snapshot_and_location() {
        let original = mid_progress_state();
        let save = RetailSave {
            snapshot: original.snapshot(),
            location: RetailLocation {
                world_index: 1,
                map_index_2: 0xFFFF,
                map_index: 0x13,
                char_x: 48 * 16,
                char_y: 18 * 16,
            },
        };
        let slot = RetailSlot::encode(&save, 1).unwrap();
        assert_eq!(slot.as_bytes().len(), RETAIL_SLOT_FILE_BYTES);
        assert_eq!(slot.as_bytes()[0], 0, "MOVEP even byte is a hole");
        assert_eq!(slot.as_bytes()[RETAIL_HEADER_PHYSICAL_BYTES], 0);
        assert_eq!(slot.checksum(1).unwrap(), 0x8B64);

        let loaded = RetailSlot::from_bytes(slot.as_bytes(), 1)
            .unwrap()
            .decode()
            .unwrap();
        assert_eq!(loaded, save);
        assert_eq!(GameState::from_snapshot(&loaded.snapshot), original);
    }

    #[test]
    fn checksum_rejects_a_corrupted_payload_byte() {
        let save = RetailSave {
            snapshot: mid_progress_state().snapshot(),
            location: RetailLocation::default(),
        };
        let slot = RetailSlot::encode(&save, 0).unwrap();
        let mut bytes = *slot.as_bytes();
        bytes[RETAIL_HEADER_PHYSICAL_BYTES + 1 + 2 * INVENTORY_OFFSET] ^= 1;
        assert!(matches!(
            RetailSlot::from_bytes(&bytes, 0),
            Err(SaveError::ChecksumMismatch { slot: 0, .. })
        ));
    }

    #[test]
    fn a_zero_stats_seat_survives_through_the_rust_occupancy_extension() {
        let mut state = GameState::new();
        state
            .roster_mut()
            .seat(
                CharId(4),
                Stats {
                    name_bytes: [0; 6],
                    profession: 0,
                    level: 0,
                    experience: 0,
                    curr_hp: 0,
                    max_hp: 0,
                    curr_tp: 0,
                    max_tp: 0,
                    status: 0,
                    strength: StatTriple::default(),
                    mental: StatTriple::default(),
                    agility: StatTriple::default(),
                    dexterity: StatTriple::default(),
                    attack: StatPair::default(),
                    defence: StatPair::default(),
                    mental_defence: StatPair::default(),
                    element_props: [0; 14],
                    element_shadow: [0; 14],
                    weapon_elements: [0; 2],
                    equipment: [0; 4],
                    techniques: [0; 16],
                    skills: [0; 8],
                    curr_skill_uses: [0; 8],
                    max_skill_uses: [0; 8],
                    physical_prop_save: 0,
                    enemy_id: 0,
                    gain_exp_flag: false,
                },
            )
            .unwrap();
        let save = RetailSave {
            snapshot: state.snapshot(),
            location: RetailLocation::default(),
        };
        let slot = RetailSlot::encode(&save, 0).unwrap();
        let loaded = RetailSlot::from_bytes(slot.as_bytes(), 0)
            .unwrap()
            .decode()
            .unwrap();
        assert_eq!(
            loaded.snapshot.characters[4],
            Some(Stats {
                name_bytes: [0; 6],
                profession: 0,
                level: 0,
                experience: 0,
                curr_hp: 0,
                max_hp: 0,
                curr_tp: 0,
                max_tp: 0,
                status: 0,
                strength: StatTriple::default(),
                mental: StatTriple::default(),
                agility: StatTriple::default(),
                dexterity: StatTriple::default(),
                attack: StatPair::default(),
                defence: StatPair::default(),
                mental_defence: StatPair::default(),
                element_props: [0; 14],
                element_shadow: [0; 14],
                weapon_elements: [0; 2],
                equipment: [0; 4],
                techniques: [0; 16],
                skills: [0; 8],
                curr_skill_uses: [0; 8],
                max_skill_uses: [0; 8],
                physical_prop_save: 0,
                enemy_id: 0,
                gain_exp_flag: false,
            })
        );
    }
}
