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

/// Ported from [retired test_pack_game_start](https://github.com/TusanHomichi/PSIV/blob/902868b517fd6b8eb0aee15b5dd500623cc7954c/tests/test_pack_game_start.py) (#216): the manifest's headline
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

// Ported from [retired test_newgame](https://github.com/TusanHomichi/PSIV/blob/902868b517fd6b8eb0aee15b5dd500623cc7954c/tests/test_newgame.py) (#548). The synthetic cases first: the
// bit arithmetic of `EventFlags_Set`, on bytes the test writes.

/// `TestFlagPacking`: `lsr.w #3,d0` picks the byte and `move.w #7,d1 /
/// sub.w d2,d1 / bset d1` puts flag 0 in bit 7 and flag 7 in bit 0.
#[test]
fn flag_ids_read_most_significant_bit_first_from_the_bank_base() {
    assert_eq!(flag_ids(&[0x80], 0), [0]);
    assert_eq!(flag_ids(&[0x01], 0), [7]);
    assert_eq!(flag_ids(&[0x00, 0x80], 0), [8]);
    assert_eq!(flag_ids(&[0xFF], 0), (0..8).collect::<Vec<u16>>());
    // `Extended_Event_Flags` holds ids from $100 up.
    assert_eq!(flag_ids(&[0x80], 0x100), [0x100]);
    assert_eq!(flag_ids(&[0, 0, 0, 0, 0x01], 0x100), [0x127]);
    assert!(flag_ids(&[0; 32], 0x100).is_empty());
}

/// `TestFlagPacking2`: packing is the inverse of reading, and the oracle's
/// first longs (oracle/logs/01_newgame.csv: eflags_00 = 01000400,
/// town_flags_00 = 80008040) fall out of it.
#[test]
fn packing_inverts_reading_and_gives_the_oracles_first_longs() {
    let cases: [(&[u16], u16); 5] = [
        (&[7, 21], 0),
        (&[0x127, 0x1A7], 0x100),
        (&[0, 0x10, 0x19], 0),
        (&[], 0),
        (&[255], 0),
    ];
    for (flags, base) in cases {
        let set: BTreeSet<u16> = flags.iter().copied().collect();
        let packed = pack_flags(&set, base).unwrap();
        assert_eq!(packed.len(), FLAG_BANK_BYTES);
        assert_eq!(flag_ids(&packed, base), flags, "base 0x{base:03X}");
    }
    let first_long =
        |flags: &[u16]| long(&pack_flags(&flags.iter().copied().collect(), 0).unwrap(), 0);
    assert_eq!(first_long(&[0x07, 0x15]), 0x0100_0400);
    assert_eq!(first_long(&[0x00, 0x10, 0x19]), 0x8000_8040);
    // An extended id in the base bank, and one past its 256 flags.
    for flag in [0x127, 0x100] {
        let error = pack_flags(&[flag].into(), 0).unwrap_err();
        assert!(error.0.contains("outside the 32-byte bank"), "{}", error.0);
    }
}

/// `TestCharacterSymbols`: the `CharID_*` order, and a party slot is a
/// character or `$FF`.
#[test]
fn eleven_characters_in_id_order_and_six_party_slots() {
    assert_eq!(CHARACTER_SYMBOLS.len(), 11);
    assert_eq!(
        (
            CHARACTER_SYMBOLS[0],
            CHARACTER_SYMBOLS[1],
            CHARACTER_SYMBOLS[10]
        ),
        ("Chaz", "Alys", "Seth")
    );
    assert_eq!((EMPTY_SLOT, PARTY_SLOTS), (0xFF, 6));
    let slot = |id| serde_json::Value::from(&character(id).unwrap());
    assert_eq!(slot(1)["symbol"], "Alys");
    assert_eq!(slot(EMPTY_SLOT)["empty"], true);
    assert!(character(11).is_err());
}

// The retail cases: `TestNewGameAgainstTheRom` and `TestAgainstThePackedMap`.

/// The ROM and `extract_new_game`'s document, read once for every retail test.
fn retail() -> Option<&'static (Vec<u8>, serde_json::Value)> {
    static RETAIL: std::sync::OnceLock<Option<(Vec<u8>, serde_json::Value)>> =
        std::sync::OnceLock::new();
    RETAIL
        .get_or_init(|| {
            let rom = crate::test_support::retail_rom("new game retail tests")?;
            let data = rom.bytes().to_vec();
            let maps = extract_maps(&data, &Decodes::new(&data)).unwrap();
            let (_, json) = extract_new_game(&data, maps.real()).unwrap();
            Some((data, serde_json::Value::from(&json)))
        })
        .as_ref()
}

const MAP_PIATA_ACADEMY: u64 = 0x11;
const MAP_PIATA_ACADEMY_F1: u64 = 0x13;

fn symbols(slots: &serde_json::Value) -> Vec<Option<&str>> {
    slots
        .as_array()
        .unwrap()
        .iter()
        .map(|slot| slot["symbol"].as_str())
        .collect()
}

/// The title screen never loads a map: it sets the field mode's "play
/// event" routine and an event index, on the academy's ground floor, where
/// the scene plays and not where the player is handed control.
#[test]
fn retail_title_starts_an_event_from_a_table_that_ends_where_code_begins() {
    let Some((rom, doc)) = retail() else { return };
    let handoff = &doc["title_handoff"];
    assert_eq!(handoff["game_mode_routine"]["value"], 0xC);
    assert_eq!(handoff["event_index"]["value"], GAME_START_EVENT_INDEX);
    assert_eq!(handoff["scene_map"]["id"], MAP_PIATA_ACADEMY);
    assert_eq!(handoff["scene_map"]["symbol"], "PiataAcademy");
    assert_eq!(event_ptrs(rom).unwrap(), 0x05A2B4);
    assert_eq!(
        event_routine(rom, GAME_START_EVENT_INDEX).unwrap(),
        0x073946
    );
    assert_eq!(
        event_routine(rom, GAME_START_EVENT_INDEX + 1).unwrap(),
        0x073ECE
    );
    let routines = event_routines(rom).unwrap();
    assert_eq!(routines.len(), 0xA1);
    assert_eq!(EVENT_PTRS + routines.len() * 4, 0x05A538);
    // Event_PiataChazAlone is the last routine by address, which is why its
    // extent comes from its tail jump rather than the next entry.
    assert_eq!(routines.iter().max(), Some(&routines[0xA0]));
}

/// What `loc_44414` writes: Chaz and Alys, 500 meseta, no items, and eleven
/// extended flags copied back over a 512-byte clear.
#[test]
fn retail_initialiser_state() {
    let Some((rom, doc)) = retail() else { return };
    let init = &doc["new_game_init"];
    assert_eq!(init["routine"], "0x044414");
    assert_eq!(
        symbols(&init["party_slots"]),
        [Some("Chaz"), Some("Alys"), None, None, None, None]
    );
    assert_eq!(
        (&init["party_slots"][0]["id"], &init["party_slots"][1]["id"]),
        (&0.into(), &1.into())
    );
    assert_eq!(init["money"], 500);
    assert_eq!(init["inventory"]["slots"], 40);
    assert_eq!(init["inventory"]["all_empty"], true);
    assert_eq!(
        init["settings"],
        serde_json::json!({"message_speed": 2, "battle_speed": 2, "world_index": 0})
    );
    let flags = &init["flags"];
    assert_eq!(flags["cleared_region"]["bytes"], 512);
    let extended = &flags["extended_event_flags"];
    assert_eq!(extended["address"], "0xFFFFF120");
    assert_eq!(extended["source"], "0x044632");
    assert_eq!(extended["bytes"], 32);
    // Thirty-two bytes and the ids they hold pin the whole table.
    assert_eq!(
        extended["set"],
        serde_json::json!([
            "0x127", "0x128", "0x12A", "0x134", "0x138", "0x150", "0x15B", "0x15D", "0x16B",
            "0x178", "0x1A7"
        ])
    );
    // The clone names Chest_Flags ($F140) here; retail's `lea` names $F120.
    let lea = init["reads"]
        .as_array()
        .unwrap()
        .iter()
        .find(|read| read["label"] == "loc_44414: Extended_Event_Flags table")
        .unwrap();
    let raw = lea["raw_hex"].as_str().unwrap();
    assert!(
        raw.contains(&format!("43f8{EXTENDED_EVENT_FLAGS:04x}")),
        "{raw}"
    );
    assert!(!raw.contains(&format!("43f8{CHEST_FLAGS:04x}")), "{raw}");
    assert_eq!(flags["chest_flags"]["set"], serde_json::json!([]));
    // Town_Flags is four bytes, not two.
    let town = &flags["town_flags"];
    assert_eq!(town["bytes"], 4);
    assert_eq!(town["raw_hex"], "80008040");
    assert_eq!(town["set"], serde_json::json!(["0x000", "0x010", "0x019"]));
    // InitialCharStats, applied to all eleven characters.
    let stats = &init["character_stats"];
    assert_eq!(stats["source"], "0x2A8ACA");
    assert_eq!(stats["characters"], CHARACTER_SYMBOLS.len());
    // The typed record the game reads says the same.
    let (typed, _, extended_set, town_set) = read_new_game_init(rom).unwrap();
    assert_eq!(typed.money, 500);
    assert_eq!((extended_set.len(), town_set), (11, vec![0x00, 0x10, 0x19]));
}

/// The player starts alone as Chaz on the academy's first floor; during the
/// scene it was Alys leading. The retail immediates, not the clone's
/// $58/$22/right.
#[test]
fn retail_player_starts_alone_as_chaz_where_the_epilogue_puts_him() {
    let Some((_, doc)) = retail() else { return };
    let control = &doc["first_control"];
    assert_eq!(
        symbols(&control["party"]),
        [Some("Chaz"), None, None, None, None, None]
    );
    assert_eq!(
        symbols(&control["event"]["party_during_scene"]),
        [Some("Alys"), Some("Chaz"), None, None]
    );
    assert_eq!(control["map"]["id"], MAP_PIATA_ACADEMY_F1);
    assert_eq!(control["map"]["symbol"], "PiataAcademy_F1");
    assert_eq!(
        control["facing"],
        serde_json::json!({"id": 0, "name": "down"})
    );
    assert_eq!(control["character_alignment"], 0);
    assert_eq!(control["music"]["symbol"], "MotabiaTown");
    let position = &control["position"];
    let at = |key: &str| position[key].as_u64().unwrap();
    assert_eq!((at("x_start_word"), at("y_start_word")), (0x60, 0x24));
    assert_eq!((at("x_pixels"), at("y_pixels")), (768, 288));
    assert_eq!((at("x_cell"), at("y_cell")), (48, 19));
    assert_eq!(at("x_pixels"), at("x_start_word") * START_POS_PIXELS);
    assert_eq!(
        at("y_cell"),
        at("y_pixels") / COLLISION_CELL_PIXELS + STANDING_CELL_Y_OFFSET
    );
}

/// Two scenes run before control: `RunEvents` walks the start map's
/// triggers after Event_GameStart, and one plays Event_PiataChazAlone, which
/// sets the very flag its own trigger tests. The map's other triggers are
/// reported with the reason each cannot fire.
#[test]
fn retail_two_scenes_run_before_control_and_the_chain_stops_itself() {
    let Some((_, doc)) = retail() else { return };
    let chain = doc["first_control"]["scene_chain"].as_array().unwrap();
    let field = |key: &str| chain.iter().map(|s| s[key].clone()).collect::<Vec<_>>();
    assert_eq!(field("event_hex"), ["0x9F", "0xA0"]);
    assert_eq!(field("routine"), ["0x073946", "0x073ECE"]);
    assert_eq!(chain[1]["rom_end"], "0x073EE0");
    let sets = |scene: &serde_json::Value| {
        scene["sets"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["flag_hex"].clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(sets(&chain[0]), ["0x07"]);
    assert_eq!(sets(&chain[1]), ["0x15"]);
    let trigger = |scene: &serde_json::Value, key: &str, value: serde_json::Value| {
        scene["triggers_after"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t[key] == value)
            .unwrap()
            .clone()
    };
    assert_eq!(
        trigger(&chain[1], "event", "0xA0".into())["gates"],
        serde_json::json!([{"flag": 0x15, "flag_hex": "0x15", "required": "clear"}])
    );
    let indices: Vec<u64> = chain[0]["triggers_after"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["index"].as_u64().unwrap())
        .collect();
    assert_eq!(indices, [3, 10, 124]);
    // RunEvent_FindingAlys waits for the party to stand somewhere.
    assert_eq!(
        trigger(&chain[0], "index", 3.into())["position_gated"],
        true
    );
    // RunEvent_SuspicionOnPrincipal wants a flag a new game has not set.
    let suspicion = trigger(&chain[0], "index", 10.into());
    assert_eq!(suspicion["position_gated"], false);
    assert!(
        suspicion["gates"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!({"flag": 9, "flag_hex": "0x09", "required": "set"}))
    );
}

/// The flag banks at first control match the emulator oracle
/// (oracle/logs/01_newgame.csv, frame 6938), and all four banks are scanned:
/// their setters share one bit routine and differ only in the `lea`.
#[test]
fn retail_flag_banks_at_first_control() {
    let Some((rom, doc)) = retail() else { return };
    let control = &doc["first_control"];
    let flags = control["event_flags_set"].as_array().unwrap();
    assert_eq!(
        flags.iter().map(|f| f["id"].clone()).collect::<Vec<_>>(),
        [0x07, 0x15]
    );
    assert_eq!(
        flags
            .iter()
            .map(|f| f["symbol"].clone())
            .collect::<Vec<_>>(),
        ["EventFlag_PiataFirstTime", "EventFlag_PiataChazControl"]
    );
    let banks = &control["flag_banks"];
    assert_eq!(banks["event_flags"]["first_long"], "0x01000400");
    assert_eq!(
        banks["event_flags"]["raw_hex"],
        format!("01000400{}", "00".repeat(28))
    );
    assert_eq!(banks["extended_event_flags"]["first_long"], "0x00000000");
    assert_eq!(banks["town_flags"]["first_long"], "0x80008040");
    assert_eq!(banks["chest_flags"]["raw_hex"], "00".repeat(32));
    let banks = banks.as_object().unwrap();
    assert_eq!(banks.len(), 4);
    assert!(banks.values().all(|bank| bank["bytes"] == 32));

    let setters = flag_setters(rom).unwrap();
    let mut names: Vec<&str> = setters.iter().map(|(_, name)| *name).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "chest_flags",
            "event_flags",
            "extended_event_flags",
            "town_flags"
        ]
    );
    assert_eq!(setters[0], (EVENT_FLAGS_SET, "event_flags"));
    assert_eq!(control["chest_flags_set"], serde_json::json!([]));
    assert_eq!(
        control["town_flags_set"],
        serde_json::json!([0x00, 0x10, 0x19])
    );
    // Neither scene touches anything but the base bank, so the extended bank
    // is exactly what the initialiser left.
    let extended: Vec<String> = control["extended_event_flags_set"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| format!("0x{:03X}", f.as_u64().unwrap()))
        .collect();
    assert_eq!(extended.len(), 11);
    assert_eq!(
        serde_json::json!(extended),
        doc["new_game_init"]["flags"]["extended_event_flags"]["set"]
    );
}

/// RunEvent_PiataChazAlone: one flag gate, one dispatch, no position.
/// RunEvent_Null00 dispatches nothing, so it never fires.
#[test]
fn retail_trigger_decoder_reads_gates_and_dispatch() {
    let Some((rom, _)) = retail() else { return };
    let trigger = read_trigger(rom, 124).unwrap();
    assert_eq!(trigger.event, Some(0xA0));
    assert_eq!(trigger.gates, [(0x15, false)]);
    assert!(!trigger.position_gated);
    assert!(trigger.fires(&BTreeSet::new()));
    assert!(!trigger.fires(&[0x15].into()));
    let null = read_trigger(rom, 0).unwrap();
    assert_eq!(null.event, None);
    assert!(!null.fires(&BTreeSet::new()));
}

/// Fail-closed: a ROM whose initialiser or title handoff moved, or whose
/// event table does not resolve, is refused rather than read wrongly.
#[test]
fn retail_moved_sites_are_refused() {
    let Some((rom, _)) = retail() else { return };
    let broken = |at: usize, len: usize| {
        let mut copy = rom.clone();
        copy[at..at + len].fill(0);
        copy
    };
    let error = read_new_game_init(&broken(NEW_GAME_INIT + 0x50, 8)).unwrap_err();
    assert!(error.0.contains("loc_44414"), "{}", error.0);
    let error = read_title_handoff(&broken(TITLE_SET_MAP, 8)).unwrap_err();
    assert!(
        error.0.contains("loc_4335C: Field_Map_Index"),
        "{}",
        error.0
    );
    // The `lea`'s displacement inside loc_5A27A.
    let unresolved = broken(0x05A280, 2);
    let maps = extract_maps(rom, &Decodes::new(rom)).unwrap();
    let error = extract_new_game(&unresolved, maps.real()).unwrap_err();
    assert!(error.0.contains("EventPtrs resolves to"), "{}", error.0);
}

/// `TestAgainstThePackedMap`: the start cell is somewhere the player can
/// stand, on the map file the maps section writes for the map the start
/// names, with Alys there as a field object and the start's music the map's.
#[test]
fn retail_start_cell_is_walkable_on_the_packed_start_map() {
    let Some((rom, doc)) = retail() else { return };
    let control = &doc["first_control"];
    let decodes = Decodes::new(rom);
    let inputs = crate::maps::pictures::map_inputs(rom, &decodes).unwrap();
    let field = crate::sprites::emit::field_sprites(rom, &decodes, &inputs).unwrap();
    let (record, sprites) = inputs
        .maps
        .real()
        .zip(&field.maps)
        .find(|(record, _)| record.id as u64 == MAP_PIATA_ACADEMY_F1)
        .unwrap();
    let section = crate::maps::overworld::decode_map_section(rom, &decodes, record).unwrap();
    let pictures = crate::maps::pictures::map_pictures(
        rom,
        &decodes,
        record,
        &section,
        &inputs.effects,
        &sprites.effects,
        &inputs.updates,
    )
    .unwrap();
    let programs = inputs
        .updates
        .per_map
        .get(&record.id)
        .map_or(&[][..], Vec::as_slice);
    let (file, _, _) =
        crate::maps::file::map_file(rom, record, &section, &pictures, sprites, programs).unwrap();
    let map = serde_json::Value::from(&file);
    assert_eq!(map["id"], control["map"]["id"]);
    let x = control["position"]["x_cell"].as_u64().unwrap() as usize;
    let y = control["position"]["y_cell"].as_u64().unwrap() as usize;
    assert!(x < map["dimensions"]["width_cells"].as_u64().unwrap() as usize);
    assert!(y < map["dimensions"]["height_cells"].as_u64().unwrap() as usize);
    // Type 0 is ordinary ground, and so is every neighbour: the game does not
    // drop the player onto a warp or into a wall.
    let rows = &map["collision"]["rows"];
    let around: Vec<&serde_json::Value> = (y - 1..=y + 1)
        .flat_map(|row| (x - 1..=x + 1).map(move |column| &rows[row][column]))
        .collect();
    assert_eq!(around, [&serde_json::json!(0); 9]);
    // The party is Chaz alone and Alys is a field object, which makes the
    // opening "go and find her" rather than a party of two.
    let alys = map["npcs"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|npc| npc["symbol"] == "NPCAlysPiata")
        .count();
    assert_eq!(alys, 1);
    assert_eq!(control["music"]["id"], map["music"]["id"]);
}
