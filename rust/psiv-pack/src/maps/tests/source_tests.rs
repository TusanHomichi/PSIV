//! Optional disassembly checks from tests/test_maps.py. The reference fork is
//! a symbol oracle only; record behavior is checked against verified retail.

use super::records::XY_RANGES;
use super::retail_tests::{array, retail};
use crate::map_symbols::FIELD_OBJECT_SYMBOLS;
use crate::sound_defs::MUSIC_NAMES;

fn source(name: &str) -> Option<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference/ps4disasm")
        .join(name);
    match std::fs::read(&path) {
        Ok(bytes) => Some(String::from_utf8_lossy(&bytes).into_owned()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("local disassembly absent: map source tests skipped");
            None
        }
        Err(e) => panic!("cannot read {}: {e}", path.display()),
    }
}

fn block<'a>(asm: &'a str, label: &str) -> &'a str {
    asm.split_once(&format!("\n{label}:")).unwrap().1
}

fn jumps(asm: &str, label: &str, stride: usize) -> Vec<String> {
    let mut entries = Vec::new();
    for line in block(asm, label)
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let Some(rest) = line.strip_prefix("bra.w") else {
            break;
        };
        let (symbol, annotation) = rest.split_once(';').unwrap();
        let annotation = annotation.split_whitespace().next().unwrap();
        let index = if let Some(hex) = annotation.strip_prefix('$') {
            usize::from_str_radix(hex, 16).unwrap()
        } else {
            annotation.parse().unwrap()
        };
        assert_eq!(index, entries.len() * stride);
        entries.push(symbol.trim().to_owned());
    }
    entries
}

#[test]
fn symbol_tables_and_jump_bounds_match_the_reference() {
    let Some(asm) = source("ps4.asm") else { return };
    let objects = jumps(&asm, "FieldObjectsJmpTbl", 4);
    assert_eq!(objects.len(), 222);
    assert_eq!(
        objects
            .iter()
            .map(|s| s.strip_prefix("FieldObj_").unwrap_or(s))
            .collect::<Vec<_>>(),
        FIELD_OBJECT_SYMBOLS
    );
    assert_eq!(
        jumps(&asm, "XYRangeJmpTbl", 1)
            .iter()
            .map(|s| s.strip_prefix("XYRange_").unwrap())
            .collect::<Vec<_>>(),
        XY_RANGES
    );
    assert_eq!(jumps(&asm, "MapUpdateJmpTbl", 1).len(), 64);
    assert_eq!(jumps(&asm, "MapDataManagerJmpTbl", 1).len(), 0x83);
    let managers = block(&asm, "MapDataManagerJmpTbl")
        .split("; =================================================================")
        .next()
        .unwrap();
    assert!(managers.contains("if revision=0"));
    assert_eq!(
        managers
            .lines()
            .filter(|s| s.starts_with("\tbra.w"))
            .count(),
        0xa0 + 2
    );
    let constants = source("ps4.constants.asm").expect("reference constants");
    let mut found = std::collections::BTreeMap::new();
    for line in constants.lines().map(str::trim) {
        let Some(rest) = line.strip_prefix("MusicID_") else {
            continue;
        };
        let Some((name, right)) = rest.split_once('=') else {
            continue;
        };
        if !right.trim_start().starts_with("id(PtrMusic_") {
            continue;
        }
        let id = usize::from_str_radix(
            right
                .split_once(';')
                .unwrap()
                .1
                .trim()
                .trim_start_matches('$'),
            16,
        )
        .unwrap();
        found.insert(id, name.trim());
    }
    assert_eq!(found.len(), 52);
    assert_eq!(found.first_key_value().unwrap().0, &0x81);
    for (id, name) in found {
        assert_eq!(MUSIC_NAMES[id - 0x81], name);
    }
}

#[test]
fn encounter_table_and_piata_targets_round_trip_to_source() {
    let Some(asm) = source("ps4.asm") else { return };
    let Some(r) = retail() else { return };
    let mut values = Vec::new();
    for line in block(&asm, "Battle_EnemyFormationIndexes").lines() {
        let line = line.split(';').next().unwrap().trim();
        if line.is_empty() {
            continue;
        }
        let Some(value) = line.strip_prefix("dc.b") else {
            break;
        };
        values.push(u8::from_str_radix(value.trim().trim_start_matches('$'), 16).unwrap());
    }
    assert_eq!(values.len(), 416);
    assert_eq!(&r.rom.bytes()[0x8050..0x81f0], values);
    assert_eq!(
        r.rom
            .bytes()
            .windows(values.len())
            .filter(|w| *w == values)
            .count(),
        1
    );
    let mut targets = Vec::new();
    for line in block(&asm, "Map_Piata").lines() {
        if line.trim().starts_with("; Objects") {
            break;
        }
        let code = line.split(';').next().unwrap().trim();
        if let Some(operand) = code.strip_prefix("dc.w").map(str::trim)
            && let Some(symbol) = operand.strip_prefix("MapID_")
        {
            targets.push(symbol);
        }
    }
    let p = &r.records["maps"][0x10];
    let actual: Vec<_> = ["transitions", "transitions_2"]
        .into_iter()
        .flat_map(|k| {
            array(&p[k]["entries"])
                .iter()
                .map(|t| t["target"]["symbol"].as_str().unwrap())
        })
        .collect();
    assert_eq!(actual, targets);
    for (start, end) in [(0x28501c, 0x2853c8), (0x2853cc, 0x28541f)] {
        assert!(asm.contains(&format!("(0x{start:08X}-0x{end:08X}")));
    }
}
