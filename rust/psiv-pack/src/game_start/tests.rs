//! Synthetic data, no ROM content.

use std::collections::BTreeSet;

use super::chain::*;
use super::*;

#[test]
fn flags_pack_most_significant_bit_first_and_round_trip() {
    let set: BTreeSet<u16> = [0x100, 0x107, 0x108, 0x11F].into();
    let packed = pack_flags(&set, 0x100).unwrap();
    assert_eq!(&packed[..4], &[0x81, 0x80, 0x00, 0x01]);
    assert_eq!(
        flag_ids(&packed, 0x100),
        set.into_iter().collect::<Vec<_>>()
    );
    assert!(pack_flags(&[0x20].into(), 0x100).is_err());
    assert!(pack_flags(&[0x100].into(), 0).is_err());
}

#[test]
fn a_pattern_must_match_the_expected_number_of_times() {
    let rom = [
        0x31, 0xFC, 0x12, 0x34, 0xEC, 0x28, 0x31, 0xFC, 0x56, 0x78, 0xEC, 0x28,
    ];
    let read = |expect, pick| {
        read_pattern(
            &rom,
            &[lit(&[0x31, 0xFC]), Part::Any(2), ram(0xEC28)],
            "x",
            None,
            expect,
            pick,
        )
    };
    assert_eq!(read(2, 1).unwrap().value().unwrap(), 0x5678);
    assert_eq!(read(2, 0).unwrap().rom_offset, 0);
    assert!(
        read(1, 0)
            .unwrap_err()
            .0
            .contains("expected 1 match(es), found 2")
    );
    let windowed = move_w(&rom, 0xEC28, "x", Some((6, 12))).unwrap();
    assert_eq!(
        (windowed.rom_offset, windowed.value().unwrap()),
        (6, 0x5678)
    );
}

#[test]
fn a_position_gated_trigger_never_fires_before_control() {
    let trigger = |position_gated, gates: Vec<(u16, bool)>| Trigger {
        index: 0,
        rom_offset: 0,
        event: Some(1),
        gates,
        position_gated,
    };
    let flags: BTreeSet<u16> = [7].into();
    assert!(trigger(false, vec![(7, true), (0x15, false)]).fires(&flags));
    assert!(!trigger(false, vec![(7, false)]).fires(&flags));
    assert!(!trigger(true, vec![]).fires(&flags));
}

#[test]
fn the_flag_id_comes_from_a_moveq_or_a_move_w() {
    assert_eq!(preceding_d0(&[0x70, 0x15, 0, 0], 2).unwrap(), 0x15);
    assert_eq!(
        preceding_d0(&[0x30, 0x3C, 0x01, 0x07, 0, 0], 4).unwrap(),
        0x107
    );
    assert!(preceding_d0(&[0, 0, 0, 0, 0], 4).is_err());
}

/// Ported from tests/test_pack_game_start.py (#216): the manifest's headline
/// of where a new game begins, read from the file this section writes. The
/// two scenes before control are pinned against oracle/logs/01_newgame.csv
/// (eflags_00 = 01000400).
#[test]
fn retail_headline_is_where_a_new_game_begins() {
    use sha2::Digest;
    let Some(rom) = crate::test_support::retail_rom("game start retail test") else {
        return;
    };
    let root = crate::test_support::scratch("game-start");
    let part = serde_json::Value::from(&build(&rom, &root).unwrap());
    let blob = std::fs::read(root.join(NAME)).unwrap();
    std::fs::remove_dir_all(&root).unwrap();
    let start = &part["game_start"];
    assert_eq!(start["file"], NAME);
    assert_eq!(
        start["sha256"].as_str().unwrap(),
        crate::png::hex(&sha2::Sha256::digest(&blob))
    );
    let file: serde_json::Value = serde_json::from_slice(&blob).unwrap();
    assert_eq!(
        (&file["format_version"], &file["kind"]),
        (&2.into(), &"game_start".into())
    );
    let mut sections: Vec<&str> = file
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .filter(|k| !["format_version", "kind", "note"].contains(k))
        .collect();
    sections.sort_unstable();
    assert_eq!(
        sections,
        ["first_control", "new_game_init", "title_handoff"]
    );
    assert_eq!(start["party"], serde_json::json!(["Chaz"]));
    assert_eq!(start["map"]["symbol"], "PiataAcademy_F1");
    assert_eq!(
        (&start["x_cell"], &start["y_cell"]),
        (&48.into(), &19.into())
    );
    assert_eq!(
        start["facing"],
        serde_json::json!({"id": 0, "name": "down"})
    );
    assert_eq!(start["event_flags_set"], serde_json::json!([0x07, 0x15]));
    assert_eq!(start["scene_chain"], serde_json::json!(["0x9F", "0xA0"]));
    assert_eq!(
        start["town_flags_set"],
        serde_json::json!([0x00, 0x10, 0x19])
    );
    assert_eq!(
        start["extended_event_flags_set"].as_array().unwrap().len(),
        11
    );
    assert_eq!(start["chest_flags_set"], serde_json::json!([]));
    let bank = |name: &str| {
        start["flag_banks"][name]["raw_hex"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    assert_eq!(bank("event_flags"), format!("01000400{}", "00".repeat(28)));
    assert_eq!(bank("town_flags"), format!("80008040{}", "00".repeat(28)));
}
