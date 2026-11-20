use super::*;

// Synthetic content with the retail contract's extents; no cartridge rows.
pub(crate) fn fixture(sha256: &str) -> EndingCreditsAssets {
    let text = |start, end, count, byte| {
        let mut next = start;
        (0..count)
            .map(|i| {
                let len = if i + 1 == count {
                    (end - next) as usize
                } else {
                    2
                };
                let mut bytes = vec![byte; len];
                bytes[len - 1] = 0xFF;
                let record = CreditsText {
                    rom_addr: next,
                    bytes,
                };
                next += len as u32;
                record
            })
            .collect()
    };
    EndingCreditsAssets {
        version: CREDITS_VERSION,
        source_rom_sha256: sha256.into(),
        art: [
            (0x1DE278, "kosinski", 0x2000, 87),
            (0x1DEA48, "kosinski", 0x4000, 67),
            (0x2A3350, "nemesis", 0xF800, 29),
        ]
        .into_iter()
        .map(|(rom_addr, kind, vram_destination, tiles)| CreditsArt {
            rom_addr,
            compression: kind.into(),
            vram_destination,
            consumed: 2,
            tiles: vec![0x12; tiles * 32],
        })
        .collect(),
        font_upload: CreditsFontUpload {
            rom_addr: 0x7A26C,
            destination_tile: 0x7C0,
            art_rom_addr: 0x2A3350,
            terminator: 0xFFFF,
        },
        mappings: [
            (0x1DF09C, 0x100, vec![0xFFFF1000]),
            (0x1DF17C, 0x100, vec![0xFFFF1380]),
            (0x1DF294, 0x100, vec![0xFFFF1700]),
            (0x1DF3E0, 0x100, vec![0xFFFF1A80, 0xFFFF1E00]),
            (0x1DF4C2, 0x2200, vec![0xFFFF2000]),
        ]
        .into_iter()
        .map(|(rom_addr, base_tile, ram_destinations)| CreditsMapping {
            rom_addr,
            base_tile,
            ram_destinations,
            consumed: 2,
            columns: 16,
            rows: 28,
            words: vec![base_tile; 448],
        })
        .collect(),
        palettes: vec![
            CreditsPalette {
                rom_addr: 0x1DF53A,
                start_word: 0,
                words: vec![0; 32],
            },
            CreditsPalette {
                rom_addr: 0x1DF57A,
                start_word: 48,
                words: vec![0; 16],
            },
        ],
        commands: [(0x7A522, 18), (0x7A548, 10)]
            .into_iter()
            .map(|(rom_addr, count)| {
                let mut bytes: Vec<_> = (0..count).flat_map(|tick| [tick, 0]).collect();
                bytes.extend([0xFF, 0]);
                CreditsCommands { rom_addr, bytes }
            })
            .collect(),
        headers: text(0x1FEE26, 0x1FF066, 37, 0x80),
        names: text(0x1FEB42, 0x1FEE25, 128, 0x20),
        fade: CreditsPalette {
            rom_addr: 0x7A6DE,
            start_word: 50,
            words: vec![0; 40],
        },
    }
}

#[test]
fn credits_validation_rejects_incomplete_or_malformed_renderer_inputs() {
    let good = fixture("synthetic");
    assert_eq!(good.validate("synthetic"), Ok(()));
    let changes: &[fn(&mut EndingCreditsAssets)] = &[
        |a| a.version += 1,
        |a| a.source_rom_sha256.clear(),
        |a| {
            a.art.pop();
        },
        |a| {
            a.art[0].tiles.pop();
        },
        |a| a.art[0].consumed = usize::MAX,
        |a| a.font_upload.terminator = 0,
        |a| {
            a.mappings.remove(1);
        },
        |a| a.mappings[1] = a.mappings[0].clone(),
        |a| a.mappings[3].ram_destinations.pop().map(|_| ()).unwrap(),
        |a| a.mappings[4].words[0] = 0x100,
        |a| a.mappings[0].rows = 16,
        |a| {
            a.palettes.pop();
        },
        |a| a.palettes[0].words[0] = 0xFFFF,
        |a| {
            a.fade.words.pop();
        },
        |a| {
            a.commands.pop();
        },
        |a| a.commands[0].bytes[36] = 0,
        |a| a.commands[0].bytes[1] = 13,
        |a| a.commands[0].bytes[2] = 64,
        |a| a.commands[1].bytes[2] = 0,
        |a| {
            a.headers.remove(0);
        },
        |a| a.headers[1].rom_addr += 1,
        |a| a.headers[0].bytes[1] = 0,
        |a| a.names[0].bytes[0] = 0xFF,
        |a| a.headers[0].bytes[0] = 0x7F,
    ];
    for (index, change) in changes.iter().enumerate() {
        let mut bad = good.clone();
        change(&mut bad);
        assert!(
            bad.validate("synthetic").is_err(),
            "negative control {index}"
        );
    }
}

#[test]
fn credits_file_loader_names_absence_old_version_and_malformed_records_for_rebuild() {
    let dir = std::env::temp_dir().join(format!("psiv-credits-load-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("presentation")).unwrap();
    let path = dir.join(CREDITS_FILE);
    let _ = std::fs::remove_file(&path);
    assert!(
        EndingCreditsAssets::load(&dir, "s")
            .unwrap_err()
            .to_string()
            .contains("rebuild the pack")
    );
    let good = fixture("s");
    std::fs::write(&path, serde_json::to_vec(&good).unwrap()).unwrap();
    assert_eq!(EndingCreditsAssets::load(&dir, "s").unwrap(), good);
    for bad in ["{}", "{", "{\"version\": 0}"] {
        std::fs::write(&path, bad).unwrap();
        assert!(
            EndingCreditsAssets::load(&dir, "s")
                .unwrap_err()
                .to_string()
                .contains("rebuild the pack")
        );
    }
    let mut old = good;
    old.version = 0;
    std::fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
    assert!(
        EndingCreditsAssets::load(&dir, "s")
            .unwrap_err()
            .to_string()
            .contains("incompatible credits version")
    );
    std::fs::remove_dir_all(dir).unwrap();
}
