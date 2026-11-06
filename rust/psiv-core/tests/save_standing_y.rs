//! The `$F408` standing-Y word (#129): a native slot, a cartridge image and a
//! native slot written before the `SY` marker, through the codec's public
//! surface only.
//!
//! The cartridge's `SaveGame` stores the leader object's `curr_y_pos`, one
//! row above the standing cell (ps4.asm:119576-119577), and its loader turns
//! the word back into a start row (`loc_53546`, ps4.asm:110808-110814).
//! `RetailLocation::char_y` is the standing row times 16.

mod common;

use common::stats;
use psiv_core::{
    CharId, GameState, RETAIL_HEADER_LOGICAL_BYTES, RETAIL_HEADER_PHYSICAL_BYTES,
    RETAIL_PAYLOAD_BYTES, RETAIL_SLOT_FILE_BYTES, RetailLocation, RetailSave, RetailSlot,
    StateSnapshot,
};

// Logical header offsets (`docs/camp/SAVE_FORMAT.md`).
const SIGNATURE: usize = 0x02;
const SAVE_SLOT: usize = 0x12;
const CHECKSUM: usize = 0x14;
const ROSTER_MASK: usize = 0x20;
const DUNGEON_EXIT: usize = 0x22;
const SAVED_PARTY: usize = 0x26;
const STANDING_Y: usize = 0x2E;
// Logical payload offsets.
const LOCATION_MAP: usize = 0x304;
const LOCATION_X: usize = 0x306;
const LOCATION_Y: usize = 0x308;

/// A slot file split into its logical header and payload, to rebuild files
/// the current writer no longer makes.
struct Logical {
    header: [u8; RETAIL_HEADER_LOGICAL_BYTES],
    payload: [u8; RETAIL_PAYLOAD_BYTES],
}

impl Logical {
    fn of(bytes: &[u8]) -> Logical {
        let mut header = [0; RETAIL_HEADER_LOGICAL_BYTES];
        let mut payload = [0; RETAIL_PAYLOAD_BYTES];
        for (index, byte) in header.iter_mut().enumerate() {
            *byte = bytes[1 + index * 2];
        }
        for (index, byte) in payload.iter_mut().enumerate() {
            *byte = bytes[RETAIL_HEADER_PHYSICAL_BYTES + 1 + index * 2];
        }
        Logical { header, payload }
    }

    fn word(bytes: &[u8], offset: usize) -> u16 {
        u16::from_be_bytes([bytes[offset], bytes[offset + 1]])
    }

    fn set_word(bytes: &mut [u8], offset: usize, value: u16) {
        bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
    }

    /// The physical file, with the slot's payload checksum restamped.
    fn file(&mut self, slot: usize) -> Vec<u8> {
        let sum = self
            .payload
            .iter()
            .fold(0_u16, |sum, byte| sum.wrapping_add(u16::from(*byte)));
        Logical::set_word(&mut self.header, CHECKSUM + slot * 2, sum);
        let mut bytes = vec![0; RETAIL_SLOT_FILE_BYTES];
        for (index, byte) in self.header.iter().enumerate() {
            bytes[1 + index * 2] = *byte;
        }
        for (index, byte) in self.payload.iter().enumerate() {
            bytes[RETAIL_HEADER_PHYSICAL_BYTES + 1 + index * 2] = *byte;
        }
        bytes
    }
}

fn load(bytes: &[u8], slot: usize) -> RetailSave {
    RetailSlot::from_bytes(bytes, slot)
        .unwrap()
        .decode()
        .unwrap()
}

fn party_state() -> GameState {
    let mut state = GameState::new();
    state.set_party([Some(CharId(0)), Some(CharId(1)), None, None, None]);
    for index in [0_u8, 1, 4] {
        let mut record = stats();
        record.name_bytes[0] = index + 1;
        state.roster_mut().seat(CharId(index), record).unwrap();
    }
    state
}

fn standing_at(snapshot: StateSnapshot, row: u16) -> RetailSave {
    RetailSave {
        snapshot,
        location: RetailLocation {
            map_index: 0x13,
            char_x: 48 * 16,
            char_y: row * 16,
            ..RetailLocation::default()
        },
    }
}

/// A slot file as the cartridge leaves it: `InitSRAM` zeroes the device
/// (ps4.asm:134737), then the signature, the slot word and the slot's
/// checksum are the only header bytes it writes. `y_word` is the raw `$F408`.
fn cartridge_image(snapshot: &StateSnapshot, y_word: u16, slot: usize) -> Vec<u8> {
    let mut image = Logical {
        header: [0; RETAIL_HEADER_LOGICAL_BYTES],
        payload: snapshot.to_retail_payload(),
    };
    image.header[SIGNATURE..SIGNATURE + 15].copy_from_slice(b"PHANTASY STAR 4");
    Logical::set_word(&mut image.header, SAVE_SLOT, slot as u16);
    Logical::set_word(&mut image.payload, LOCATION_MAP, 0x13);
    Logical::set_word(&mut image.payload, LOCATION_X, 48 * 16);
    Logical::set_word(&mut image.payload, LOCATION_Y, y_word);
    image.file(slot)
}

/// The file the codec wrote before the marker: the same header without `SY`
/// and the standing row itself in `$F408`.
fn legacy_of(current: &[u8], standing_row: u16) -> Logical {
    let mut legacy = Logical::of(current);
    legacy.header[STANDING_Y..STANDING_Y + 2].fill(0);
    Logical::set_word(&mut legacy.payload, LOCATION_Y, standing_row * 16);
    legacy
}

#[test]
fn a_cartridge_image_stands_one_row_below_its_object_y() {
    let snapshot = party_state().snapshot();
    let image = cartridge_image(&snapshot, 17 * 16, 0);
    let loaded = load(&image, 0);
    assert_eq!(loaded.location.char_y, 18 * 16);
    assert_eq!(
        GameState::from_snapshot(&loaded.snapshot),
        party_state(),
        "a cartridge image without the native roster mask seats its party"
    );
}

#[test]
fn a_native_save_writes_the_cartridges_payload_and_the_marker() {
    let snapshot = party_state().snapshot();
    let image = cartridge_image(&snapshot, 17 * 16, 0);
    let native = RetailSlot::encode(&load(&image, 0), 0).unwrap();
    let written = Logical::of(native.as_bytes());
    assert_eq!(
        written.payload,
        Logical::of(&image).payload,
        "a native SAVE writes the cartridge's payload, Y word included"
    );
    assert_eq!(Logical::word(&written.payload, LOCATION_Y), 17 * 16);
    assert_eq!(&written.header[STANDING_Y..STANDING_Y + 2], b"SY");
    assert_eq!(
        Logical::word(&written.header, ROSTER_MASK),
        0b1_0011,
        "the mask names the seated records"
    );
    assert_eq!(load(native.as_bytes(), 0), load(&image, 0));
}

#[test]
fn a_legacy_native_slot_keeps_its_standing_row() {
    let save = standing_at(party_state().snapshot(), 18);
    let current = RetailSlot::encode(&save, 2).unwrap().with_dungeon_exit(3);
    let mut legacy = legacy_of(current.as_bytes(), 18);
    let legacy_file = legacy.file(2);
    let loaded = load(&legacy_file, 2);
    assert_eq!(loaded, save, "a legacy slot loads where it was saved");
    assert_eq!(
        Logical::word(
            &Logical::of(RetailSlot::encode(&loaded, 2).unwrap().as_bytes()).payload,
            LOCATION_Y
        ),
        17 * 16,
        "its next SAVE writes the retail word"
    );

    // A damaged marker is not the marker: the file reads as legacy.
    let mut damaged = Logical::of(current.as_bytes());
    damaged.header[STANDING_Y + 1] = b'X';
    assert_eq!(load(&damaged.file(2), 2).location.char_y, 17 * 16);
}

#[test]
fn a_zero_roster_legacy_slot_is_legacy_by_its_exit_or_its_saved_party() {
    // A session without battle files seats no roster: its mask is zero, but
    // the slot writer always adds `TR`, and a sleep scene's SAVE adds `PS`.
    let save = standing_at(GameState::new().snapshot(), 18);
    let bridge = Some([Some(CharId(0)), None, None, None, None]);
    let with_exit = RetailSlot::encode(&save, 0).unwrap().with_dungeon_exit(0);
    let with_both = with_exit.clone().with_saved_party_slots(bridge);

    let mut exit_only = legacy_of(with_exit.as_bytes(), 18);
    assert_eq!(Logical::word(&exit_only.header, ROSTER_MASK), 0);
    assert_eq!(load(&exit_only.file(0), 0).location.char_y, 18 * 16);

    let mut bridge_only = legacy_of(with_both.as_bytes(), 18);
    bridge_only.header[DUNGEON_EXIT..DUNGEON_EXIT + 4].fill(0);
    assert_ne!(bridge_only.header[SAVED_PARTY], 0);
    let bridge_file = bridge_only.file(0);
    assert_eq!(load(&bridge_file, 0).location.char_y, 18 * 16);

    // Negative control: with neither, the same word is a cartridge image's
    // object Y, one row higher than the standing row it reads.
    bridge_only.header[SAVED_PARTY..STANDING_Y].fill(0);
    assert_eq!(load(&bridge_only.file(0), 0).location.char_y, 19 * 16);
}

#[test]
fn a_legacy_slot_with_its_native_metadata_cleared_reads_as_a_cartridge_image() {
    let save = standing_at(party_state().snapshot(), 18);
    let current = RetailSlot::encode(&save, 1).unwrap().with_dungeon_exit(0);
    let mut legacy = legacy_of(current.as_bytes(), 18);
    legacy.header[ROSTER_MASK..STANDING_Y].fill(0);
    let as_image = load(&legacy.file(1), 1);
    assert_eq!(as_image.location.char_y, 19 * 16);
    assert_eq!(as_image.snapshot, save.snapshot, "it still seats its party");
}
