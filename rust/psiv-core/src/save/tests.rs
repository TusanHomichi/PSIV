//! The codec against its own bytes: round trips, the checksum, the native
//! header extensions and the standing-Y conventions.

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
    // The Y word is row 17 (`$0110`), the object Y of standing row 18.
    assert_eq!(slot.checksum(1).unwrap(), 0x8B54);

    let loaded = RetailSlot::from_bytes(slot.as_bytes(), 1)
        .unwrap()
        .decode()
        .unwrap();
    assert_eq!(loaded, save);
    assert_eq!(GameState::from_snapshot(&loaded.snapshot), original);
}

/// A slot file as the cartridge leaves it: `InitSRAM` zeroes the device
/// (ps4.asm:134737), then the signature, the slot word and the slot's
/// checksum are the only header bytes it writes. `y_word` is the raw
/// `$F408` word.
fn cartridge_image(snapshot: &StateSnapshot, x: u16, y_word: u16, slot: usize) -> Vec<u8> {
    let mut payload = snapshot.to_retail_payload();
    write_u16(&mut payload, LOCATION_MAP_OFFSET, 0x13);
    write_u16(&mut payload, LOCATION_X_OFFSET, x);
    write_u16(&mut payload, LOCATION_Y_OFFSET, y_word);
    let mut header = [0; RETAIL_HEADER_LOGICAL_BYTES];
    header[SIGNATURE_OFFSET..SIGNATURE_OFFSET + SIGNATURE.len()].copy_from_slice(SIGNATURE);
    write_u16(&mut header, SAVE_SLOT_OFFSET, slot as u16);
    write_u16(
        &mut header,
        CHECKSUM_OFFSET + slot * 2,
        payload_checksum(&payload),
    );
    let mut bytes = vec![0; RETAIL_SLOT_FILE_BYTES];
    interleave(&header, &mut bytes[..RETAIL_HEADER_PHYSICAL_BYTES]);
    interleave(&payload, &mut bytes[RETAIL_HEADER_PHYSICAL_BYTES..]);
    bytes
}

fn logical_header(bytes: &[u8]) -> [u8; RETAIL_HEADER_LOGICAL_BYTES] {
    deinterleave(&bytes[..RETAIL_HEADER_PHYSICAL_BYTES])
}

fn logical_payload(bytes: &[u8]) -> [u8; RETAIL_PAYLOAD_BYTES] {
    deinterleave(&bytes[RETAIL_HEADER_PHYSICAL_BYTES..])
}

#[test]
fn a_native_slot_and_a_cartridge_image_agree_on_standing_y() {
    // Standing on row 18: the leader object's `curr_y_pos` is row 17's
    // pixel Y, the word `SaveGame` stores (ps4.asm:119576-119577).
    let snapshot = mid_progress_state().snapshot();
    let standing = 18 * 16;
    let image = cartridge_image(&snapshot, 48 * 16, standing - 16, 0);
    let from_image = RetailSlot::from_bytes(&image, 0).unwrap().decode().unwrap();
    assert_eq!(from_image.location.char_y, standing);

    let native = RetailSlot::encode(&from_image, 0).unwrap();
    assert_eq!(
        logical_payload(native.as_bytes()),
        logical_payload(&image),
        "a native SAVE writes the cartridge's payload, Y word included"
    );
    assert_eq!(
        native.checksum(0).unwrap(),
        RetailSlot::from_bytes(&image, 0)
            .unwrap()
            .checksum(0)
            .unwrap()
    );
    let header = logical_header(native.as_bytes());
    assert_eq!(
        &header[RUST_STANDING_Y_OFFSET..RUST_STANDING_Y_OFFSET + 2],
        b"SY"
    );
    assert_eq!(native.decode().unwrap(), from_image);
    assert_eq!(
        GameState::from_snapshot(&from_image.snapshot),
        mid_progress_state(),
        "a cartridge image without the native roster mask seats its party"
    );
}

#[test]
fn a_legacy_native_slot_keeps_its_standing_row() {
    // Before the `SY` marker the codec wrote the standing row times 16
    // and the roster mask. Strip the marker to rebuild that file: it must
    // load on the same row, and its next SAVE writes the retail word.
    let save = RetailSave {
        snapshot: mid_progress_state().snapshot(),
        location: RetailLocation {
            map_index: 0x13,
            char_x: 48 * 16,
            char_y: 18 * 16,
            ..RetailLocation::default()
        },
    };
    let current = RetailSlot::encode(&save, 2).unwrap();
    let mut header = logical_header(current.as_bytes());
    assert_ne!(read_u16(&header, RUST_ROSTER_MASK_OFFSET), 0);
    header[RUST_STANDING_Y_OFFSET..RUST_STANDING_Y_OFFSET + 2].fill(0);
    let mut payload = logical_payload(current.as_bytes());
    write_u16(&mut payload, LOCATION_Y_OFFSET, 18 * 16);
    write_u16(
        &mut header,
        CHECKSUM_OFFSET + 2 * 2,
        payload_checksum(&payload),
    );
    let mut legacy = vec![0; RETAIL_SLOT_FILE_BYTES];
    interleave(&header, &mut legacy[..RETAIL_HEADER_PHYSICAL_BYTES]);
    interleave(&payload, &mut legacy[RETAIL_HEADER_PHYSICAL_BYTES..]);

    let loaded = RetailSlot::from_bytes(&legacy, 2)
        .unwrap()
        .decode()
        .unwrap();
    assert_eq!(loaded, save, "a legacy slot loads where it was saved");
    let resaved = RetailSlot::encode(&loaded, 2).unwrap();
    assert_eq!(resaved, current);
    assert_eq!(
        read_u16(&logical_payload(resaved.as_bytes()), LOCATION_Y_OFFSET),
        17 * 16
    );

    // Negative controls: the same legacy word read as a cartridge image
    // (no roster mask) is one row lower, and a damaged marker is legacy.
    let mut no_mask = header;
    write_u16(&mut no_mask, RUST_ROSTER_MASK_OFFSET, 0);
    interleave(&no_mask, &mut legacy[..RETAIL_HEADER_PHYSICAL_BYTES]);
    let as_image = RetailSlot::from_bytes(&legacy, 2)
        .unwrap()
        .decode()
        .unwrap();
    assert_eq!(as_image.location.char_y, 19 * 16);
    let mut damaged = logical_header(current.as_bytes());
    damaged[RUST_STANDING_Y_OFFSET + 1] = b'X';
    let mut bytes = current.as_bytes().to_vec();
    interleave(&damaged, &mut bytes[..RETAIL_HEADER_PHYSICAL_BYTES]);
    let misread = RetailSlot::from_bytes(&bytes, 2).unwrap().decode().unwrap();
    assert_eq!(misread.location.char_y, 17 * 16);
}

#[test]
fn a_legacy_native_slot_with_no_roster_is_still_legacy() {
    // A session started without battle files seats no roster, so its slots
    // carry a zero roster mask; the slot writer still adds `TR`. Without the
    // `SY` marker that slot is legacy, not a cartridge image.
    let save = RetailSave {
        snapshot: GameState::new().snapshot(),
        location: RetailLocation {
            map_index: 0x13,
            char_x: 48 * 16,
            char_y: 18 * 16,
            ..RetailLocation::default()
        },
    };
    let current = RetailSlot::encode(&save, 0).unwrap().with_dungeon_exit(0);
    let mut header = logical_header(current.as_bytes());
    assert_eq!(read_u16(&header, RUST_ROSTER_MASK_OFFSET), 0);
    header[RUST_STANDING_Y_OFFSET..RUST_STANDING_Y_OFFSET + 2].fill(0);
    let mut payload = logical_payload(current.as_bytes());
    write_u16(&mut payload, LOCATION_Y_OFFSET, 18 * 16);
    write_u16(&mut header, CHECKSUM_OFFSET, payload_checksum(&payload));
    let mut legacy = vec![0; RETAIL_SLOT_FILE_BYTES];
    interleave(&header, &mut legacy[..RETAIL_HEADER_PHYSICAL_BYTES]);
    interleave(&payload, &mut legacy[RETAIL_HEADER_PHYSICAL_BYTES..]);
    let loaded = RetailSlot::from_bytes(&legacy, 0)
        .unwrap()
        .decode()
        .unwrap();
    assert_eq!(loaded.location.char_y, 18 * 16);
    // The same file with the `TR` bytes cleared is a cartridge image.
    header[RUST_DUNGEON_EXIT_OFFSET..RUST_DUNGEON_EXIT_OFFSET + 4].fill(0);
    interleave(&header, &mut legacy[..RETAIL_HEADER_PHYSICAL_BYTES]);
    let image = RetailSlot::from_bytes(&legacy, 0)
        .unwrap()
        .decode()
        .unwrap();
    assert_eq!(image.location.char_y, 19 * 16);
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
fn the_saved_party_bridge_round_trips_in_the_native_header() {
    let save = RetailSave {
        snapshot: mid_progress_state().snapshot(),
        location: RetailLocation::default(),
    };
    let bridge = [
        Some(CharId(0)),
        Some(CharId(1)),
        Some(CharId(2)),
        None,
        None,
    ];
    let encoded = RetailSlot::encode(&save, 0)
        .unwrap()
        .with_saved_party_slots(Some(bridge));
    assert_eq!(encoded.saved_party_slots(), Some(bridge));
    // The payload and its checksum are untouched.
    assert_eq!(
        RetailSlot::from_bytes(encoded.as_bytes(), 0)
            .unwrap()
            .decode()
            .unwrap(),
        save
    );
    // The exact layout: `P`, `S`, the five slots at the interleaved stride,
    // and the complement of their sum.
    let offset = RUST_SAVED_PARTY_OFFSET * 2 + 1;
    assert_eq!(&encoded.as_bytes()[offset..offset + 2], b"P\0");
    assert_eq!(encoded.as_bytes()[offset + 2], b'S');
    let slots: Vec<u8> = (0..5)
        .map(|index| encoded.as_bytes()[offset + 4 + index * 2])
        .collect();
    assert_eq!(slots, vec![0, 1, 2, 0xFF, 0xFF]);
    let sum = slots.iter().fold(0_u8, |sum, byte| sum.wrapping_add(*byte));
    assert_eq!(encoded.as_bytes()[offset + 14], !sum);
    // It coexists with the dungeon exit just before it.
    let both = RetailSlot::encode(&save, 0)
        .unwrap()
        .with_dungeon_exit(7)
        .with_saved_party_slots(Some(bridge));
    assert_eq!(both.dungeon_exit(), Some(7));
    assert_eq!(both.saved_party_slots(), Some(bridge));
    // No bridge pending: nothing is written.
    let plain = RetailSlot::encode(&save, 0)
        .unwrap()
        .with_saved_party_slots(None);
    assert_eq!(plain.saved_party_slots(), None);
    assert_eq!(plain.as_bytes()[offset], 0);
    // An all-empty bridge writes nothing and reads None, so the writer
    // never produces a field the reader refuses.
    let empty = RetailSlot::encode(&save, 0)
        .unwrap()
        .with_saved_party_slots(Some([None; 5]));
    assert_eq!(empty.as_bytes()[offset], 0);
    assert_eq!(empty.saved_party_slots(), None);
    // A retail save's zeroed header has no marker: None.
    let mut retail = encoded.as_bytes().to_vec();
    retail[offset..=offset + 14].fill(0);
    assert_eq!(
        RetailSlot::from_bytes(&retail, 0)
            .unwrap()
            .saved_party_slots(),
        None
    );
    // A damaged bridge is refused rather than guessed.
    let mut damaged = encoded.as_bytes().to_vec();
    damaged[offset + 4] ^= 0xFF;
    let damaged = RetailSlot::from_bytes(&damaged, 0).unwrap();
    assert_eq!(damaged.saved_party_slots(), None);
}

#[test]
fn the_cleared_game_flag_round_trips_at_sram_200035() {
    let save = RetailSave {
        snapshot: mid_progress_state().snapshot(),
        location: RetailLocation::default(),
    };
    let clear = RetailSlot::encode(&save, 0).unwrap().mark_game_cleared();
    assert_eq!(clear.as_bytes()[2 * GAME_CLEARED_OFFSET + 1], 1);
    assert!(clear.game_cleared());
    // The payload and its checksum are untouched: a retail reader still
    // validates the slot, and the flag rides in the common header.
    let decoded = RetailSlot::from_bytes(clear.as_bytes(), 0).unwrap();
    assert!(decoded.game_cleared());
    assert_eq!(decoded.decode().unwrap(), save);
    let uncleared = RetailSlot::encode(&save, 0).unwrap();
    assert!(!uncleared.game_cleared());
    assert_eq!(uncleared.as_bytes()[2 * GAME_CLEARED_OFFSET + 1], 0);
    // Sticky: marking never clears an already-set byte.
    assert!(
        uncleared
            .mark_game_cleared()
            .mark_game_cleared()
            .game_cleared()
    );
    assert!(RetailSlot::cleared_in_bytes(clear.as_bytes()));
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
