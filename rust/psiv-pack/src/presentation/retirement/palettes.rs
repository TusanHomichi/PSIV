//! Palette requests checked beside the assembled 68000 instructions that
//! read them (test_presentation_pack.py); these are instruction encodings,
//! addresses and counts, not copied cartridge bytes or content hashes.
use super::*;

#[test]
fn palette_windows_cover_the_retail_request_sites_and_deepest_reads() {
    let Some(e) = emission() else { return };
    let data = e.rom.bytes();
    let sites: &[(usize, usize, usize, &[u8])] = &[
        (
            0x09F2BC,
            0x05A68A,
            64,
            &[
                0x41, 0xF9, 0x00, 0x09, 0xF2, 0xBC, 0x43, 0xF8, 0xFB, 0x00, 0x70, 0x1F, 0x22, 0xD8,
            ],
        ),
        (
            0x296300,
            0x06BF2A,
            17,
            &[
                0x41, 0xF9, 0x00, 0x29, 0x63, 0x00, 0x43, 0xF8, 0xFB, 0x40, 0x7E, 0x10, 0x4E, 0x41,
            ],
        ),
        (
            0x0782DA,
            0x0782AE,
            54,
            &[0x7C, 0x08, 0x47, 0xF9, 0x00, 0x07, 0x82, 0xDA],
        ),
        (
            0x07339A,
            0x073302,
            9,
            &[0x41, 0xF8, 0xFB, 0x00, 0x43, 0xF9, 0x00, 0x07, 0x33, 0x9A],
        ),
        (
            0x06EE16,
            0x06ED42,
            21,
            &[
                0x43, 0xF8, 0xFB, 0x60, 0x45, 0xF9, 0x00, 0x06, 0xEE, 0x16, 0x7E, 0x06,
            ],
        ),
        (
            0x2EEA04,
            0x0796D4,
            16,
            &[
                0x41, 0xF9, 0x00, 0x2E, 0xEA, 0x04, 0x43, 0xF8, 0xFB, 0x80, 0x3E, 0x3C, 0x00, 0x0F,
                0x4E, 0x41,
            ],
        ),
        (
            0x0730FC,
            0x073054,
            16,
            &[0x41, 0xF8, 0xFB, 0x60, 0x43, 0xF9, 0x00, 0x07, 0x30, 0xFC],
        ),
        (
            0x1DE258,
            0x0704D8,
            16,
            &[
                0x41, 0xF9, 0x00, 0x1D, 0xE2, 0x58, 0x43, 0xF8, 0xFB, 0x80, 0x7E, 0x0F, 0x4E, 0x41,
            ],
        ),
        (
            0x070626,
            0x0705E8,
            9,
            &[
                0x41, 0xF9, 0x00, 0x07, 0x06, 0x26, 0x43, 0xF8, 0xFB, 0x04, 0x72, 0x08,
            ],
        ),
        (
            0x070672,
            0x070638,
            22,
            &[0x43, 0xF8, 0xFB, 0x00, 0x45, 0xF9, 0x00, 0x07, 0x06, 0x72],
        ),
        (
            0x052848,
            0x052832,
            14,
            &[
                0x43, 0xF9, 0x00, 0x05, 0x28, 0x48, 0x45, 0xF8, 0xFB, 0x60, 0x7E, 0x0D, 0x34, 0xD9,
                0x51, 0xCF, 0xFF, 0xFC,
            ],
        ),
        (
            0x07A274,
            0x07A0BE,
            22,
            &[
                0x41, 0xF9, 0x00, 0x07, 0xA2, 0x74, 0x43, 0xF8, 0xFB, 0x24, 0x41, 0xF0, 0x10, 0x00,
                0x7E, 0x07, 0x4E, 0x41,
            ],
        ),
        (
            0x07A2A0,
            0x07A0DC,
            10,
            &[
                0x41, 0xF9, 0x00, 0x07, 0xA2, 0xA0, 0x43, 0xF8, 0xFB, 0x3C, 0x41, 0xF0, 0x10, 0x00,
                0x7E, 0x01, 0x4E, 0x41,
            ],
        ),
        (
            0x18711A,
            0x0705A8,
            32,
            &[
                0x41, 0xF9, 0x00, 0x18, 0x71, 0x1A, 0x43, 0xF8, 0xFB, 0x80, 0x3E, 0x3C, 0x00, 0x1F,
                0x4E, 0x41,
            ],
        ),
        (
            0x29754E,
            0x06B9EA,
            17,
            &[
                0x41, 0xF9, 0x00, 0x29, 0x75, 0x4E, 0x43, 0xF8, 0xFB, 0x60, 0x7E, 0x10, 0x4E, 0x41,
            ],
        ),
        (
            0x29756E,
            0x06BAC2,
            17,
            &[
                0x41, 0xF9, 0x00, 0x29, 0x75, 0x6E, 0x43, 0xF8, 0xFB, 0x60, 0x7E, 0x10, 0x4E, 0x41,
            ],
        ),
        (
            0x29758E,
            0x06BB9A,
            17,
            &[
                0x41, 0xF9, 0x00, 0x29, 0x75, 0x8E, 0x43, 0xF8, 0xFB, 0x60, 0x7E, 0x10, 0x4E, 0x41,
            ],
        ),
        (
            0x1DE0F8,
            0x0641F6,
            16,
            &[
                0x41, 0xF9, 0x00, 0x1D, 0xE0, 0xF8, 0x43, 0xF8, 0xFB, 0x40, 0x3E, 0x3C, 0x00, 0x0F,
                0x4E, 0x41,
            ],
        ),
        (
            0x1DE0D8,
            0x06421E,
            16,
            &[
                0x41, 0xF9, 0x00, 0x1D, 0xE0, 0xD8, 0x43, 0xF8, 0xFB, 0x40, 0x3E, 0x3C, 0x00, 0x0F,
                0x4E, 0x41,
            ],
        ),
        (
            0x1DE138,
            0x064402,
            16,
            &[
                0x41, 0xF9, 0x00, 0x1D, 0xE1, 0x38, 0x43, 0xF8, 0xFB, 0x60, 0x3E, 0x3C, 0x00, 0x0F,
                0x4E, 0x41,
            ],
        ),
        (
            0x1DE118,
            0x064450,
            16,
            &[
                0x41, 0xF9, 0x00, 0x1D, 0xE1, 0x18, 0x43, 0xF8, 0xFB, 0x20, 0x3E, 0x3C, 0x00, 0x0F,
                0x4E, 0x41,
            ],
        ),
        (
            0x1DE118,
            0x064498,
            16,
            &[
                0x41, 0xF9, 0x00, 0x1D, 0xE1, 0x18, 0x43, 0xF8, 0xFB, 0x20, 0x3E, 0x3C, 0x00, 0x0F,
                0x4E, 0x41,
            ],
        ),
    ];
    let packed: BTreeMap<_, _> = array(&e.manifest["palettes"])
        .iter()
        .map(|p| (hex(&p["rom_addr"]), p))
        .collect();
    for &(addr, site, words, code) in sites {
        assert_eq!(&data[site..site + code.len()], code, "request {addr:#x}");
        assert!(PALETTE_REQUESTS.contains(&(addr, words)));
        assert_eq!(packed[&addr]["words"], words);
    }
    for p in packed.values() {
        let at = hex(&p["rom_addr"]);
        assert_eq!(p["raw_hex"], png::hex(&data[at..at + num(&p["words"]) * 2]));
    }
    assert_eq!(
        &data[0x7A0A6..0x7A0AE],
        &[0x7A, 0x0E, 0x7C, 0, 0x0C, 0x46, 0, 0x0E]
    );
    assert_eq!(&data[0x7A0D2..0x7A0D6], &[4, 0x41, 0, 9]);
    let delays: Vec<_> = data[0x782D2..]
        .iter()
        .copied()
        .take_while(|&b| b != 0xFF)
        .collect();
    assert_eq!(delays.len(), 6);
    assert!(!delays.contains(&0));
    assert_eq!(num(&packed[&0x782DA]["words"]), 9 * delays.len());
    let source_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../rust/psiv-core/src/scenes/retail_endgame.rs");
    let source = std::fs::read_to_string(source_path).unwrap();
    let entries = source
        .split_once("const RYKROS_PALETTE_DELAYS")
        .unwrap()
        .1
        .split_once("= &[")
        .unwrap()
        .1
        .split_once(']')
        .unwrap()
        .0;
    let ticks: Vec<u8> = entries
        .split(',')
        .filter_map(|s| {
            let s = s.trim();
            if s.is_empty() {
                None
            } else {
                Some(s.parse().unwrap())
            }
        })
        .collect();
    assert_eq!(ticks, delays.iter().map(|v| v + 1).collect::<Vec<_>>());
    assert_eq!(&data[0x730FC + 32..0x730FC + 34], &[0xFF, 0xFF]);
    // Plate engine loop reads a second word at $E(a1) after each nonnegative
    // colour, reaching the word immediately before loc_6E922.
    let (address, words) = (0x06E908, 14);
    let terminator = (0..words)
        .find(|i| be(data, address + i * 2, 2) & 0x8000 != 0)
        .unwrap();
    assert!(terminator > 0);
    assert_eq!(terminator + 7, words - 1);
    assert_eq!(address + 2 * (terminator + 7), 0x6E922);
    assert_eq!(packed[&address]["words"], words);
}

#[test]
fn emitted_palette_list_includes_each_drawn_layer_and_refuses_record_mutation() {
    let Some(e) = emission() else { return };
    let records: BTreeMap<_, _> = array(&e.manifest["palettes"])
        .iter()
        .map(|p| (hex(&p["rom_addr"]), p))
        .collect();
    let mut drawn = Vec::new();
    for r in array(&e.manifest["panels"]) {
        let at = panel_record_offset(num(&r["id"]) as u16).unwrap();
        let raw = &e.rom.bytes()[at..at + PANEL_RECORD_SIZE];
        let layer = |tile, palette| (be(raw, tile, 2) != 0xFFFF).then(|| be(raw, palette, 4));
        drawn.push(PanelPalettes {
            a: layer(10, 12),
            b: layer(20, 22),
        });
    }
    for p in palettes(e.rom.bytes(), &drawn).unwrap() {
        let p = Value::from(&p);
        assert_eq!(*records[&hex(&p["rom_addr"])], p);
    }
    let check = |p: &Value| {
        p["raw_hex"]
            == png::hex(
                &e.rom.bytes()[hex(&p["rom_addr"])..hex(&p["rom_addr"]) + num(&p["words"]) * 2],
            )
    };
    let mut bad = (*records.values().next().unwrap()).clone();
    bad["raw_hex"] = json!("00");
    assert!(!check(&bad));
    // Existing synthetic panel_palettes_are_listed_once... tests absence,
    // named-request deduplication and address order without needing a ROM.
}
