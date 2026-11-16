//! Retail and optional symbol-oracle checks from tests/test_shops.py (#560).
use super::*;
use crate::test_support::retail_rom;
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn retail_lists_cover_the_exact_span_and_locations_keep_separate_index_spaces() {
    let Some(rom) = retail_rom("shop tables") else {
        return;
    };
    let data = rom.bytes();
    let lists = inventories(data).unwrap();
    assert_eq!(lists.len(), 49);
    assert_eq!(INVENTORIES_END - INVENTORIES_START, 255);
    let mut cursor = INVENTORIES_START;
    for (index, list) in lists.iter().enumerate() {
        assert_eq!(list.index, index);
        assert_eq!(list.rom_offset, cursor);
        assert_eq!(list.raw, data[cursor..cursor + list.items.len() + 1]);
        assert_eq!(list.raw.last(), Some(&0xff));
        assert!(!list.items.is_empty());
        for &item in &list.items {
            assert_eq!(item_symbol(item), Some(ITEM_SYMBOLS[usize::from(item) - 1]));
        }
        // GetOffsetByID_FF_Delim scans individual terminator bytes.
        assert_eq!(
            data[INVENTORIES_START..cursor]
                .iter()
                .filter(|&&b| b == 0xff)
                .count(),
            index
        );
        cursor += list.raw.len();
    }
    assert_eq!(cursor, INVENTORIES_END);
    check_boundary(data).unwrap();
    for at in [INVENTORIES_START, INVENTORIES_END] {
        let mut broken = data.to_vec();
        broken[at] ^= 0xff;
        assert!(inventories(&broken).is_err());
    }
    let rows = locations(data, lists.len()).unwrap();
    assert_eq!(rows.len(), 68);
    assert_eq!(
        LOCATIONS_START + rows.len() * LOCATION_ENTRY_BYTES + 2,
        0x685b6
    );
    let mut groups: BTreeMap<u8, BTreeSet<usize>> = BTreeMap::new();
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(row.rom_offset, LOCATIONS_START + index * 8);
        assert_eq!(row.raw, data[row.rom_offset..row.rom_offset + 8]);
        assert!(row.map_symbol().is_some());
        assert_eq!(row.map_symbol(), Some(MAP_SYMBOLS[usize::from(row.map_id)]));
        groups
            .entry(row.group_id())
            .or_default()
            .insert(row.index());
        assert_eq!(
            row.shop_inventory_index(),
            if row.is_inn() {
                None
            } else {
                Some(row.index())
            }
        );
        if row.is_inn() {
            assert_eq!(row.group_name(), Some("inn"));
        }
    }
    assert_eq!(groups.keys().copied().collect::<Vec<_>>(), [0, 1, 2]);
    assert_eq!(groups[&0], (0..18).collect());
    assert_eq!(groups[&2], BTreeSet::from([0x1d, 0x1e]));
    assert!(groups[&1].is_disjoint(&groups[&2]));
    assert_eq!(
        groups[&1]
            .union(&groups[&2])
            .copied()
            .collect::<BTreeSet<_>>(),
        (0..49).collect()
    );
    assert_eq!(rows.iter().filter(|r| r.is_inn()).count(), 18);
    let first = &rows[0];
    assert_eq!(
        (first.map_id, first.x, first.y, first.selector),
        (0x19, 0x290, 0x1d0, 0)
    );
    assert_eq!(MAP_SYMBOLS.len(), 417);
    assert_eq!(MAP_SYMBOLS[0x10], "Piata");
}

fn source(name: &str) -> Option<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference/ps4disasm")
        .join(name);
    match std::fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("local disassembly absent: shop source check skipped");
            None
        }
        Err(error) => panic!("cannot read {}: {error}", path.display()),
    }
}

#[test]
fn local_symbol_constants_and_inventory_source_round_trip_to_retail() {
    let Some(constants) = source("ps4.constants.asm") else {
        return;
    };
    let Some(asm) = source("ps4.asm") else { return };
    let Some(rom) = retail_rom("shop source round trip") else {
        return;
    };
    let mut ids = BTreeMap::new();
    for (prefix, symbols, first) in [
        ("ItemID_", ITEM_SYMBOLS.as_slice(), 1),
        ("MapID_", MAP_SYMBOLS.as_slice(), 0),
    ] {
        let mut count = 0;
        for line in constants.lines().map(str::trim) {
            let Some(rest) = line.strip_prefix(prefix) else {
                continue;
            };
            let (name, value) = rest.split_once('=').unwrap();
            let (expression, comment) = value.split_once(';').unwrap();
            assert!(expression.trim().starts_with("id("));
            let raw = comment.trim();
            let value = if let Some(hex) = raw.strip_prefix('$') {
                usize::from_str_radix(hex, 16).unwrap()
            } else {
                raw.parse().unwrap()
            };
            assert_eq!(value, first + count);
            assert_eq!(symbols[count], name.trim());
            if prefix == "ItemID_" {
                ids.insert(name.trim(), value as u8);
            }
            count += 1;
        }
        assert_eq!(count, symbols.len());
    }
    let body = asm.split_once("\nShopInventories:").unwrap().1;
    let mut expected = Vec::new();
    let mut ended = false;
    for line in body.lines() {
        let code = line.split(';').next().unwrap().trim();
        if code.is_empty() {
            continue;
        }
        if code == "even" {
            ended = true;
            break;
        }
        let operand = code
            .strip_prefix("dc.b")
            .expect("inventory directive")
            .trim();
        expected.push(if operand == "$FF" {
            0xff
        } else {
            ids[operand.strip_prefix("ItemID_").expect("inventory item")]
        });
    }
    assert!(ended);
    assert_eq!(expected.len(), 255);
    assert_eq!(&rom.bytes()[INVENTORIES_START..INVENTORIES_END], expected);
    assert_eq!(
        rom.bytes()
            .windows(expected.len())
            .filter(|w| *w == expected)
            .count(),
        1
    );
    let actual: Vec<_> = inventories(rom.bytes())
        .unwrap()
        .into_iter()
        .flat_map(|r| r.raw)
        .collect();
    assert_eq!(actual, expected);
    // The three preceding longs are symbolic references to local labels;
    // their address-bearing names corroborate the retail span, never fork
    // build-address annotations or rewritten scene bodies.
    let preceding = asm
        .split_once("\nloc_68198:")
        .unwrap()
        .1
        .split_once("ShopInventories:")
        .unwrap()
        .0;
    let pointers: Vec<_> = preceding
        .lines()
        .filter_map(|line| {
            let operand = line
                .split(';')
                .next()
                .unwrap()
                .trim()
                .strip_prefix("dc.l")?
                .trim();
            Some(u32::from_str_radix(operand.rsplit('_').next().unwrap(), 16).unwrap())
        })
        .collect();
    assert_eq!(pointers.len(), 3);
    for (index, pointer) in pointers.iter().enumerate() {
        assert_eq!(
            crate::m68k::l(rom.bytes(), INVENTORIES_START - 12 + index * 4).unwrap(),
            *pointer
        );
    }
    let rates = asm
        .split_once("\nloc_68112:")
        .unwrap()
        .1
        .split_once("\nloc_68136:")
        .unwrap()
        .0;
    let expected: Vec<_> = rates
        .lines()
        .filter_map(|line| {
            let code = line.split(';').next().unwrap().trim();
            if code.is_empty() {
                return None;
            }
            Some(
                code.strip_prefix("dc.b")
                    .expect("inn rate directive")
                    .split(',')
                    .map(|v| u8::from_str_radix(v.trim().strip_prefix('$').unwrap(), 16).unwrap())
                    .collect::<Vec<_>>(),
            )
        })
        .flatten()
        .collect();
    assert_eq!(expected.len(), 18 * 2);
    assert_eq!(rom.bytes()[0x68112..0x68136], expected);
}
