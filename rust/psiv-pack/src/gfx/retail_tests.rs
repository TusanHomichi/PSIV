//! Local retail acceptance retained from `test_gfx.py` and `test_enigma.py` (#558).
//! Fixtures contain offsets, lengths and digests only, never decoded art.
use super::*;
use crate::decode::{enigma, kosinski};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn fixture() -> Value {
    serde_json::from_str(include_str!("../../tests/fixtures/primitive_retail.json")).unwrap()
}
fn hex(v: &Value) -> usize {
    usize::from_str_radix(v.as_str().unwrap().trim_start_matches("0x"), 16).unwrap()
}
fn num(v: &Value) -> usize {
    v.as_u64().unwrap() as usize
}
fn sha(data: &[u8]) -> String {
    crate::png::hex(&Sha256::digest(data))
}
fn ptr(rom: &[u8], at: usize) -> usize {
    u32::from_be_bytes(rom[at..at + 4].try_into().unwrap()) as usize
}
fn art(rom: &[u8], record: &Value) -> Vec<[u8; 64]> {
    let at = hex(&record["rom_offset"]);
    let n = num(&record["compressed_size"]);
    let bytes = match record["compression"].as_str().unwrap() {
        "nemesis" => {
            decompress_art(rom, at, "primitive mapping art", Some(n), None)
                .unwrap()
                .data
        }
        "kosinski" => {
            let (bytes, used) = kosinski::decompress(rom, at).unwrap();
            assert_eq!(used, n);
            bytes
        }
        other => panic!("unexpected art compression {other}"),
    };
    decode_tiles(&bytes).unwrap()
}

#[test]
fn named_retail_art_keeps_metadata_lengths_and_all_68_output_digests() {
    let Some(rom) = crate::test_support::retail_rom("graphics primitives") else {
        return;
    };
    let rom = rom.bytes();
    let f = fixture();
    let records = f["nemesis"].as_array().unwrap();
    assert_eq!(records.len(), 68);
    let mut digests = BTreeSet::new();
    for record in records {
        let at = hex(&record["rom_offset"]);
        let size = num(&record["compressed_size"]);
        let a =
            decompress_art(rom, at, record["label"].as_str().unwrap(), Some(size), None).unwrap();
        assert_eq!(a.consumed, num(&record["consumed"]), "{}", record["label"]);
        assert!(a.consumed == size || a.consumed == size + 1);
        assert_eq!(usize::from(a.tile_count), num(&record["tile_count"]));
        assert_eq!(a.xor_mode, record["xor_mode"].as_bool().unwrap());
        assert_eq!(a.data.len(), usize::from(a.tile_count) * 32);
        assert_eq!(
            a.decompressed_sha256,
            record["decompressed_sha256"].as_str().unwrap()
        );
        digests.insert(a.decompressed_sha256);
        assert!(decompress_art(rom, at, "bad documented size", Some(size - 2), None).is_err());
        assert!(decompress_art(rom, at, "crossed landmark", None, Some(size - 2)).is_err());
    }
    assert_eq!(
        sha(digests
            .into_iter()
            .collect::<Vec<_>>()
            .join("\n")
            .as_bytes()),
        "e3d8797053723adb2d2c8e5fd1afea3006d5469b83089b39fdecda8f1c8dff3c"
    );
    let font = decompress_art(rom, 0x2A303A, "font", Some(790), None).unwrap();
    let tiles = decode_tiles(&font.data).unwrap();
    assert_eq!(
        tiles[0].iter().copied().collect::<BTreeSet<_>>(),
        [14, 15].into()
    );
    let rows: Vec<String> = tiles[0]
        .chunks(8)
        .map(|row| {
            row.iter()
                .map(|&p| if p == 15 { '#' } else { '.' })
                .collect()
        })
        .collect();
    assert_eq!(
        sha(rows.join("\n").as_bytes()),
        "c2a1654303f0144a4f113f745a9d460009ce46627226fb58f75be25eb6f2f895"
    );
    assert_eq!(rows[7], "........");
    let top_i: String = tiles[8][..8]
        .iter()
        .map(|&p| if p == 15 { '#' } else { '.' })
        .collect();
    assert_eq!(
        sha(top_i.as_bytes()),
        "6cb9edbb07416577f6dc7c08c2e11021d09f3cb0fe84bf07802f213e9936105c"
    );
    assert_eq!(
        sha(&rom[0x2A3542..0x2A3A42]),
        "19d37143313650150495f3740e9e8389377014217d93151671ad9ee955895da7"
    );
    for p in f["palettes"].as_array().unwrap() {
        let (at, n) = (num(&p["offset"]), num(&p["size"]));
        assert_eq!(sha(&rom[at..at + n]), p["sha256"].as_str().unwrap());
        assert!(
            decode_palette(&rom[at..at + n])
                .unwrap()
                .iter()
                .all(|c| c.raw & !CRAM_DEFINED_MASK == 0)
        );
    }
}

#[test]
fn all_35_retail_mappings_keep_words_composed_pixels_and_callsite_bases() {
    let Some(rom) = crate::test_support::retail_rom("Enigma mappings") else {
        return;
    };
    let rom = rom.bytes();
    let f = fixture();
    let records = f["mappings"].as_array().unwrap();
    assert_eq!(records.len(), 35);
    let mut total = 0;
    for r in records {
        let at = hex(&r["rom_offset"]);
        let base = hex(&r["base_tile"]) as u16;
        let (columns, rows) = (num(&r["columns"]), num(&r["rows"]));
        let (words, used) = enigma::decompress(rom, at, base, Some(columns * rows)).unwrap();
        assert_eq!(used, num(&r["compressed_size"]), "{}", r["label"]);
        assert_eq!(words.len(), columns * rows);
        assert_eq!(
            sha(&enigma::words_to_bytes(&words)),
            r["decoded_sha256"].as_str().unwrap()
        );
        let tiles = art(rom, &r["art"]);
        assert_eq!(tiles.len(), num(&r["art_tile_count"]));
        let origin = hex(&r["art_vram_tile"]);
        assert_eq!(usize::from(base & 0x7FF), origin);
        let header = enigma::read_header(rom, at, base).unwrap();
        assert!(words.iter().all(|w| (w >> 13) & 3 == (base >> 13) & 3));
        assert!(header.flags & 1 != 0 || words.iter().all(|w| w & 0x800 == 0));
        assert!(header.flags & 2 != 0 || words.iter().all(|w| w & 0x1000 == 0));
        let (w, h, pixels) = compose(&words, columns, &tiles, origin).unwrap();
        assert_eq!((w, h), (columns * 8, rows * 8));
        assert_eq!(
            sha(&pixels),
            r["pixels_sha256"].as_str().unwrap(),
            "{}",
            r["label"]
        );
        if base != 0 {
            let mut call = vec![0x41, 0xF9];
            call.extend_from_slice(&(at as u32).to_be_bytes());
            call.extend_from_slice(&[0x30, 0x3C]);
            call.extend_from_slice(&base.to_be_bytes());
            assert_eq!(
                rom.windows(call.len()).filter(|b| *b == call).count(),
                1,
                "{}",
                r["label"]
            );
        }
        if at == 0x1D236C {
            let (wrong, _) = enigma::decompress(rom, at, 0x60DC, None).unwrap();
            assert!(
                compose(&wrong, columns, &tiles, origin).is_err(),
                "fork revision=0 base must fail"
            );
        }
        total += words.len();
    }
    assert_eq!(total, 35436);
}

#[test]
fn retail_pointer_tables_bound_battle_art_mappings_palettes_and_portraits() {
    let Some(rom) = crate::test_support::retail_rom("graphics pointer boundaries") else {
        return;
    };
    let rom = rom.bytes();
    let mut palettes = BTreeSet::new();
    let mut arts = BTreeSet::new();
    let mut maps = BTreeSet::new();
    let mut landmarks = vec![0x237A60, 0x23A226, 0x27AE00];
    // Independently named boundaries beyond BattleBGArtPtrs's coverage.
    for i in 0..32 {
        let at = 0x6ED4 + i * 12;
        let (a, m, p) = (ptr(rom, at), ptr(rom, at + 4), ptr(rom, at + 8));
        arts.insert(a);
        maps.insert(m);
        palettes.insert(p);
        landmarks.extend([a, m]);
        let decoded = decompress_art(rom, a, "battle art", Some(m - a), None).unwrap();
        assert!(decoded.consumed <= m - a + 1);
        let (words, _) = enigma::decompress(rom, m, 0, Some(64 * 24)).unwrap();
        assert_eq!(words.len(), 64 * 24);
        assert!(words.iter().all(|w| w & 0xE000 == 0));
    }
    assert_eq!((arts.len(), maps.len(), palettes.len()), (20, 20, 31));
    assert_eq!(
        palettes.into_iter().collect::<Vec<_>>(),
        (0..31).map(|i| 0x7054 + i * 26).collect::<Vec<_>>()
    );
    // Each mapping's end is either a next art pointer or one of the three
    // explicit extra landmarks used by the Python acceptance check.
    let mut next_art = 0;
    for m in maps {
        let (_, n) = enigma::decompress(rom, m, 0, None).unwrap();
        if arts.contains(&(m + n)) {
            next_art += 1;
        }
        assert!(
            landmarks.contains(&(m + n)),
            "mapping at {m:X} ends at {:X}",
            m + n
        );
    }
    assert_eq!(next_art, 17);
    let portraits: BTreeSet<_> = (1..40).map(|i| ptr(rom, 0x6A4B0 + i * 4)).collect();
    assert_eq!(portraits.len(), 36);
    assert_eq!(ptr(rom, 0x6A4B0), 0);
    assert_eq!(ptr(rom, 0x6A4B0 + 39 * 4), 0x2A28DA);
    let beyond = ptr(rom, 0x6A4B0 + 40 * 4);
    assert_ne!(beyond, 0x29DBBE);
    assert!(decompress_art(rom, beyond, "beyond portrait table", None, None).is_err());
    for at in &portraits {
        assert_eq!(
            decompress_art(rom, *at, "portrait", None, None)
                .unwrap()
                .tile_count,
            36
        );
    }
    for at in [
        0x29DBBE, 0x29DE1E, 0x29E0A4, 0x29E340, 0x29E576, 0x29E7A6, 0x29EA5C,
    ] {
        assert!(!portraits.contains(&at));
        let pointer = (at as u32).to_be_bytes();
        let first = rom.windows(4).position(|w| w == pointer).unwrap();
        assert!((0x68000..0x69000).contains(&first));
        assert_eq!(
            decompress_art(rom, at, "shopkeeper portrait", None, None)
                .unwrap()
                .tile_count,
            36
        );
    }
}

fn bins(path: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(path).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(bins(&path));
        } else if path.extension().is_some_and(|e| e == "bin") {
            files.push(path);
        }
    }
    files.sort();
    files
}

#[test]
fn reference_payloads_pin_stream_lengths_offsets_and_scrolling_text_exception() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference/ps4disasm");
    if !root.exists() {
        eprintln!("local reference absent: primitive source checks skipped");
        return;
    }
    let mapping_files = bins(&root.join("plane mappings"));
    assert_eq!(mapping_files.len(), 83);
    let mut characters = 0;
    for path in mapping_files {
        let mut data = std::fs::read(&path).unwrap();
        let size = data.len();
        data.extend_from_slice(&[0; 4]);
        let (words, n) = enigma::decompress(&data, 0, 0, None).unwrap();
        if path.file_name().unwrap() == "Scrolling Text Background Enigma.bin" {
            assert_eq!((size, n, words.len()), (684, 336, 640));
            assert!(enigma::read_header(&data, n, 0).is_err());
        } else {
            assert_eq!(n, size, "{}", path.display());
        }
        assert!(!words.is_empty());
        if path.parent().unwrap().file_name().unwrap() == "characters" {
            characters += 1;
            assert!([36, 42].contains(&words.len()));
        }
    }
    assert_eq!(characters, 43);
    let Some(rom) = crate::test_support::retail_rom("reference graphics offsets") else {
        return;
    };
    let rom = rom.bytes();
    let mut files = bins(&root.join("graphics"));
    files.extend(bins(&root.join("plane mappings")));
    let f = fixture();
    let mut expected = BTreeMap::new();
    for section in ["nemesis", "mappings"] {
        for r in f[section].as_array().unwrap() {
            expected.insert(
                hex(&r["rom_offset"]),
                (num(&r["compressed_size"]), section == "nemesis"),
            );
        }
    }
    expected.insert(0x2A3542, (1280, false));
    for r in f["mappings"].as_array().unwrap() {
        let a = &r["art"];
        if a["compression"] == "kosinski" {
            let at = hex(&a["rom_offset"]);
            let n = num(&a["compressed_size"]);
            // These binclude files include alignment padding after Kosinski's
            // terminator. Pin their whole payload, not file size = consumption.
            assert!(
                files.iter().any(|p| {
                    let blob = std::fs::read(p).unwrap();
                    blob.len() >= n
                        && blob == rom[at..at + blob.len()]
                        && rom.windows(blob.len()).filter(|w| *w == blob).count() == 1
                }),
                "Kosinski reference at {at:X}"
            );
        }
    }
    for (at, (n, lookahead)) in expected {
        assert!(
            files.iter().any(|path| {
                let blob = std::fs::read(path).unwrap();
                // The scrolling-text file also includes subsequent sprite records.
                (blob.len() == n
                    || (lookahead && blob.len() + 1 == n)
                    || (path.file_name().unwrap() == "Scrolling Text Background Enigma.bin"
                        && n == 336))
                    && blob == rom[at..at + blob.len()]
                    && rom.windows(blob.len()).filter(|w| *w == blob).count() == 1
            }),
            "no unique reference payload for {at:X}+{n}"
        );
    }
    let palettes: Vec<_> = bins(&root.join("palettes/battle"))
        .iter()
        .map(|p| std::fs::read(p).unwrap())
        .collect();
    for i in 0..31 {
        let at = 0x7054 + i * 26;
        assert!(
            palettes.iter().any(|p| p == &rom[at..at + 26]),
            "battle palette {i}"
        );
    }
}
