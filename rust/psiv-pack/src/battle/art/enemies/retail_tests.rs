//! The static enemy checks from tests/test_battle_art.py (#550).
//! Dynamic replacement pieces and their coverage belong to #551.

use super::*;
use crate::battle::image_tests::{Output, assert_png, palette, plane};
use crate::test_support::retail_rom;

#[test]
fn retail_tables_art_geometry_and_body_completeness() {
    let Some(rom) = retail_rom("enemy image census") else {
        return;
    };
    let rom = rom.bytes();
    let records = records(rom).unwrap();
    assert_eq!(records.len(), 153);
    assert_eq!(ART_TABLE + ART_STRIDE * records.len(), ENEMY_PALETTE_TABLE);
    assert_eq!(
        ENEMY_PALETTE_TABLE + ENEMY_PALETTE_STRIDE * records.len(),
        0x280CC8
    );
    let bounds = bounds(&records, rom.len());
    let mut art = BTreeMap::new();
    let (mut equal, mut holes, mut complete, mut symmetric, mut raw) = (0, 0, 0, 0, Vec::new());
    for record in &records {
        assert!((1..=20).contains(&record.half_width));
        assert!((5..=15).contains(&record.height));
        let (bank, sources) = bank(rom, record, &bounds).unwrap();
        for source in sources {
            let (bytes, _) = nemesis::decompress(rom, source.offset).unwrap();
            art.insert(source.offset, bytes);
        }
        let first = nemesis::read_header(rom, record.art[0]).unwrap().tile_count;
        assert!(record.vram_patterns >= first);
        equal += usize::from(record.vram_patterns == first);
        let (words, format, columns) = mapping(rom, record).unwrap();
        assert_eq!(columns, 2 * usize::from(record.half_width));
        assert_eq!(words.len(), columns * usize::from(record.height));
        assert!(words.iter().all(|w| usize::from(w & 0x7ff) < bank.len()));
        if format == "raw_words" {
            raw.push(record.id);
            assert!(enigma::decompress(rom, record.mapping, 0, Some(4096)).is_err());
        }
        let missing = words
            .iter()
            .filter(|&&w| w & 0x7ff != 0 && bank[usize::from(w & 0x7ff)] == [0; 64])
            .count();
        holes += missing;
        complete += usize::from(missing == 0);
        let mirrored = words.chunks(columns).all(|row| {
            row.iter()
                .zip(row.iter().rev())
                .all(|(&l, &r)| l & 0x7ff == r & 0x7ff && (l & 0x7ff == 0 || (l ^ r) & 0x800 != 0))
        });
        symmetric += usize::from(mirrored);
        if record.id == 0 {
            assert!(mirrored);
        }
        if record.id == 12 {
            assert!(!mirrored);
            assert_eq!(
                (columns, record.height, bank.len(), missing),
                (10, 10, 213, 0)
            );
        }
        if record.id == 130 {
            assert_eq!(
                (columns, record.height, bank.len(), format),
                (40, 15, 393, "enigma")
            );
        }
    }
    assert_eq!((equal, holes, complete, symmetric), (49, 303, 84, 69));
    assert_eq!(raw, [0x86, 0x87]);
    assert_eq!(art.len(), 199);
    assert_eq!(art.values().map(|b| b.len() / 32).sum::<usize>(), 11541);
    assert_eq!(art.values().collect::<BTreeSet<_>>().len(), 187);
    let helex = &records[0];
    assert_eq!(
        (helex.symbol, helex.offset, helex.half_width, helex.height),
        ("Helex", 0x27F3AE, 4, 8)
    );
    assert_eq!(helex.art, [0x20D5F0, 0x20D2C8, 0x20D2A6]);
    let (_, sources) = bank(rom, helex, &bounds).unwrap();
    assert_eq!(
        sources
            .iter()
            .map(|s| (s.tile_count, s.first_pattern))
            .collect::<Vec<_>>(),
        [(23, 3), (72, 26), (3, 0)]
    );
    let mut broken = super::records(rom).unwrap().remove(0);
    broken.mapping = ART_TABLE;
    assert!(mapping(rom, &broken).is_err());
}

#[test]
fn body_bank_order_beats_all_five_alternatives() {
    let Some(rom) = retail_rom("enemy bank ordering") else {
        return;
    };
    let rom = rom.bytes();
    let records = records(rom).unwrap();
    let mut art = BTreeMap::new();
    for offset in records.iter().flat_map(|r| r.art) {
        art.entry(offset).or_insert_with(|| {
            gfx::decode_tiles(&nemesis::decompress(rom, offset).unwrap().0).unwrap()
        });
    }
    let mappings: Vec<_> = records.iter().map(|r| mapping(rom, r).unwrap().0).collect();
    let mut scores = BTreeMap::new();
    for a in 0..3 {
        for b in 0..3 {
            for c in 0..3 {
                if a == b || a == c || b == c {
                    continue;
                }
                let (mut holes, mut blanks) = (0, 0);
                for (record, words) in records.iter().zip(&mappings) {
                    let bank: Vec<_> = [a, b, c]
                        .iter()
                        .flat_map(|&f| &art[&record.art[f]])
                        .collect();
                    blanks += usize::from(*bank[0] == [0; 64]);
                    holes += words
                        .iter()
                        .filter(|&&w| w & 0x7ff != 0 && *bank[usize::from(w & 0x7ff)] == [0; 64])
                        .count();
                }
                scores.insert([a, b, c], (holes, blanks));
            }
        }
    }
    assert_eq!(scores[&BANK_ORDER], (303, 141));
    for (order, (holes, _)) in scores {
        if order != BANK_ORDER {
            assert!(holes * 2 > 303 * 3, "{order:?}");
        }
    }
}

#[test]
fn every_body_png_contains_its_bank_mapping_and_assembled_palette() {
    let Some(rom) = retail_rom("enemy image content") else {
        return;
    };
    let rom = rom.bytes();
    let root = Output::new("enemy-images");
    let emitted = serde_json::Value::from(&write(rom, &root.0).unwrap());
    assert_eq!(
        emitted["palette_layout"]["cram_lines"],
        serde_json::json!([1, 2])
    );
    let entries = emitted["enemies"].as_array().unwrap();
    assert_eq!(entries.len(), 153);
    for (record, entry) in records(rom).unwrap().iter().zip(entries) {
        let raw: Vec<u8> = [2, 0, 1]
            .iter()
            .flat_map(|&f| nemesis::decompress(rom, record.art[f]).unwrap().0)
            .collect();
        let (words, _, columns) = mapping(rom, record).unwrap();
        let pixels = plane(&words, columns, &raw, 0);
        assert!(
            pixels.iter().all(|&p| p < 14),
            "no line-dependent/UI pixels"
        );
        let mut colors = vec![[0; 3]; 16];
        colors[1] = gfx::Color::decode(0xEEE).rgb();
        colors[3..14].copy_from_slice(&palette(rom, 0x27FFA2 + record.id * 22, 11));
        colors[14] = gfx::Color::decode(0x600).rgb();
        let bytes = std::fs::read(root.0.join(entry["png"].as_str().unwrap())).unwrap();
        assert_png(
            &bytes,
            (columns * 8, usize::from(record.height) * 8),
            &pixels,
            &colors,
        );
        assert_eq!(entry["png_sha256"], png::image_sha256(&bytes).unwrap());
    }
}
