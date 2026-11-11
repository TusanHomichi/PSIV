use super::driver::command_vocabulary;
use super::records::voice_table;
use super::*;

fn walk(track: &[u8]) -> Result<Decoded, PackError> {
    decode_track(track, 0, track.len(), "t", 0)
}

#[test]
fn notes_take_a_delay_only_below_0x80() {
    // note+delay, note, note+delay, end
    let decoded = walk(&[0x81, 0x10, 0x82, 0x83, 0x04, 0xF2]).unwrap();
    assert_eq!(decoded.states, 4);
    assert_eq!(decoded.covered.len(), 6);
    assert!(decoded.has_track_end);
    assert_eq!(decoded.command_counts, BTreeMap::from([(0xF2, 1)]));
}

#[test]
fn a_loop_walks_both_ways_and_a_call_returns() {
    let track = [
        0xF7, 0x00, 0x02, 0xFF, 0xFC, // 0: loop to 4-4 = 0, or on to 5
        0xF8, 0x00, 0x02, // 5: gosub 7+2 = 9, returning to 8
        0xF2, // 8: end
        0xF9, // 9: return
    ];
    let decoded = walk(&track).unwrap();
    assert_eq!(decoded.states, 4);
    assert_eq!(decoded.max_call_depth, 1);
    assert_eq!(
        decoded.command_counts,
        BTreeMap::from([(0xF2, 1), (0xF7, 1), (0xF8, 1), (0xF9, 1)])
    );
    assert!(decoded.has_track_end);
}

#[test]
fn the_walk_refuses_what_the_driver_cannot_play() {
    for (track, message) in [
        (&[0xF9][..], "t: RETURN without GOSUB at 0x000000"),
        (
            &[0xFF, 0x01, 0x00],
            "t: undocumented FF meta 0x01 at 0x000001",
        ),
        (
            &[0xF6, 0x00, 0x10],
            "t: 0xF6 at 0x000000 targets outside track at 0x000012",
        ),
        (
            &[0xF6, 0xFF, 0xF0],
            "t: 0xF6 at 0x000000 targets outside track at 0x-0000E",
        ),
        (
            &[0xF0, 0x00, 0x00],
            "t: 0xF0 at 0x000000 truncates at 0x000003",
        ),
        (&[0x81], "t: note at 0x000000 lacks lookahead"),
        (&[0xEF], "t: EF at 0x000000 lacks instrument"),
        (
            &[0xF8, 0xFF, 0xFE],
            "t: GOSUB stack exceeds static safety limit",
        ),
    ] {
        assert_eq!(walk(track).err().map(|e| e.0).as_deref(), Some(message));
    }
}

#[test]
fn envelopes_end_at_a_control_byte_but_not_inside_chg_mult() {
    assert_eq!(
        envelope_span(&[1, 0x84, 0x81, 2, 0x83], 0, 5, "modulation", 0).unwrap(),
        (0, 5, 0x83)
    );
    assert_eq!(
        envelope_span(&[1, 0x84, 0x81], 0, 3, "volume", 0).unwrap(),
        (0, 3, 0x81)
    );
    assert_eq!(
        envelope_span(&[1, 2], 0, 2, "volume", 4).unwrap_err().0,
        "volume envelope 4 has no terminal control byte"
    );
    assert_eq!(
        envelope_span(&[0x84], 0, 1, "modulation", 1).unwrap_err().0,
        "modulation envelope 1 truncates CHG_MULT"
    );
}

#[test]
fn a_voice_table_pads_with_zeros_and_covers_every_used_voice() {
    let mut rom = vec![7; 2 * VOICE_SIZE];
    rom.extend([0, 0]);
    let (count, padding, voices) = voice_table(&rom, 0, rom.len(), "r", &[]).unwrap();
    assert_eq!((count, padding, voices.len()), (2, 2, 2));
    assert_eq!(voices[1].rom_offset, "0x000019");

    let used = Decoded {
        fm_instruments: BTreeSet::from([2]),
        ..Decoded::default()
    };
    assert_eq!(
        voice_table(&rom, 0, rom.len(), "r", &[used]).unwrap_err().0,
        "r: used FM voice index exceeds the full voice table"
    );
    *rom.last_mut().unwrap() = 1;
    assert_eq!(
        voice_table(&rom, 0, rom.len(), "r", &[]).unwrap_err().0,
        "r: nonzero bytes after the FM voice table"
    );
}

#[test]
fn another_image_fails_the_first_anchor() {
    let error = verify_provenance(&vec![0; 0x100000]).unwrap_err().0;
    assert!(
        error.starts_with("sound provenance anchor mismatch at 0x0D0008: expected "),
        "{error}"
    );
}

#[test]
fn the_vocabulary_documents_every_primary_command_once() {
    let opcodes: Vec<u8> = COMMAND_SPECS.iter().map(|s| s.opcode).collect();
    assert_eq!(opcodes, (0xE0..=0xFF).collect::<Vec<u8>>());
    let vocabulary = command_vocabulary();
    assert_eq!(vocabulary["primary"]["0xF7"]["relative_word_at"], 3);
    assert_eq!(vocabulary["primary"]["0xFF"]["variable"], true);
    assert_eq!(vocabulary["primary"]["0xE0"].get("variable"), None);
    assert_eq!(vocabulary["total_documented_entries"], 33);
}

/// Ported from tests/test_sound.py's documentation check (#216): every
/// primary command and the one meta opcode are named in the sound docs.
#[test]
fn the_sound_docs_name_every_command() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/sound/SOUND_EXTRACTION.md");
    let document = std::fs::read_to_string(path).unwrap();
    for spec in COMMAND_SPECS.iter() {
        let name = format!("`{:02X}`", spec.opcode);
        assert!(document.contains(&name), "{name}");
    }
    assert!(document.contains("`00`"));
}

fn read_json(root: &std::path::Path, relative: &str) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(root.join(relative)).unwrap()).unwrap()
}

fn records(file: &serde_json::Value, path: &[&str]) -> Vec<serde_json::Value> {
    let mut node = &file["records"];
    for key in path {
        node = &node[*key];
    }
    node["records"].as_array().unwrap().clone()
}

/// Ported from tests/test_sound.py (#216): what the retail driver data holds,
/// read back from the files the section writes.
#[test]
fn retail_sound_records_census_and_raw_slices() {
    use sha2::Digest;
    let Some(rom) = crate::test_support::retail_rom("sound retail test") else {
        return;
    };
    let data = rom.bytes();
    let root = crate::test_support::scratch("sound");
    build(&rom, &root).unwrap();
    let music = read_json(&root, "sound/music.json");
    let sfx = read_json(&root, "sound/sfx.json");
    let index = read_json(&root, "sound/index.json");
    let groups = [
        records(&music, &[]),
        records(&sfx, &["regular"]),
        records(&sfx, &["special"]),
    ];

    // The complete id space and the track inventory.
    let census = &index["census"];
    assert_eq!(groups.iter().map(Vec::len).collect::<Vec<_>>(), [52, 67, 3]);
    for (key, count) in [
        ("music_track_count", 468),
        ("regular_sfx_track_count", 86),
        ("special_sfx_track_count", 3),
        ("track_count", 557),
    ] {
        assert_eq!(census[key], count, "{key}");
    }
    assert_eq!(census["unresolved"], serde_json::json!([]));
    assert_eq!(index["unresolved"], serde_json::json!([]));

    // The command census is the documented vocabulary's.
    let seen: std::collections::BTreeSet<u8> = census["command_opcodes_seen"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| u8::from_str_radix(v.as_str().unwrap().trim_start_matches("0x"), 16).unwrap())
        .collect();
    assert_eq!(census["command_vocabulary_size"], seen.len());
    assert!(
        seen.iter()
            .all(|op| COMMAND_SPECS.iter().any(|s| s.opcode == *op))
    );
    assert_eq!(census["meta_opcodes_seen"], serde_json::json!(["0x00"]));

    // Every track carries a sorted, distinct instrument census.
    let tracks = census["instruments_used_per_track"].as_array().unwrap();
    assert_eq!(tracks.len(), 557);
    let ids: std::collections::BTreeSet<String> =
        tracks.iter().map(|t| t["track_id"].to_string()).collect();
    assert_eq!(ids.len(), 557);
    for track in tracks {
        for key in ["instruments_used", "psg_instruments_used"] {
            let list: Vec<i64> = track[key]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap())
                .collect();
            let mut sorted = list.clone();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(list, sorted, "{key}");
        }
    }

    // Every voice table emits all its definitions, padded by at most a byte.
    let mut voices = Vec::new();
    for group in &groups {
        let mut sum = 0;
        for record in group {
            let count = record["voices"].as_array().unwrap().len();
            assert_eq!(record["header"]["voice_count"], count);
            assert!(matches!(
                record["header"]["voice_padding_bytes"].as_u64(),
                Some(0 | 1)
            ));
            sum += count;
        }
        voices.push(sum);
    }
    assert_eq!(voices, [234, 88, 5]);

    // The raw record slices are the ROM's bytes.
    let hex = |v: &serde_json::Value| {
        usize::from_str_radix(v.as_str().unwrap().trim_start_matches("0x"), 16).unwrap()
    };
    for record in groups.iter().flatten() {
        let source = &record["record"];
        let (start, end) = (
            hex(&source["rom_offset"]),
            hex(&source["end_offset_exclusive"]),
        );
        let raw = std::fs::read(
            root.join("sound")
                .join(source["raw_file"].as_str().unwrap()),
        )
        .unwrap();
        assert_eq!(raw, &data[start..end], "{}", record["symbol"]);
        assert_eq!(
            source["raw_sha256"].as_str().unwrap(),
            crate::png::hex(&sha2::Sha256::digest(&raw))
        );
    }

    // The index hashes every file it lists.
    for entry in index["files"].as_array().unwrap() {
        let bytes = std::fs::read(root.join(entry["file"].as_str().unwrap())).unwrap();
        assert_eq!(entry["size_bytes"], bytes.len());
        assert_eq!(
            entry["sha256"].as_str().unwrap(),
            crate::png::hex(&sha2::Sha256::digest(&bytes))
        );
    }

    // A second build writes the same files.
    let again = crate::test_support::scratch("sound-again");
    build(&rom, &again).unwrap();
    let listing = |dir: &std::path::Path| {
        let mut files = Vec::new();
        let mut stack = vec![dir.join("sound")];
        while let Some(path) = stack.pop() {
            for entry in std::fs::read_dir(&path).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    files.push((
                        path.strip_prefix(dir).unwrap().to_owned(),
                        std::fs::read(&path).unwrap(),
                    ));
                }
            }
        }
        files.sort();
        files
    };
    assert!(listing(&root) == listing(&again), "two builds differ");
    std::fs::remove_dir_all(&root).unwrap();
    std::fs::remove_dir_all(&again).unwrap();

    // Provenance refuses drift.
    let mut changed = data.to_vec();
    changed[0xD1C40] ^= 0x01;
    assert!(extract(&changed, &[]).is_err());
}
