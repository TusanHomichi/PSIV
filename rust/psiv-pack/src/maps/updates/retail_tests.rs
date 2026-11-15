//! test_map_updates.py's extractor contracts; comparison tooling stays Python.
use super::*;
use crate::{decode::Decodes, maps::extract_maps};
use serde_json::{Value, json};

#[test]
fn every_retail_update_is_accounted_for_in_map_list_order() {
    let Some(rom) = crate::test_support::retail_rom("map update order") else {
        return;
    };
    let data = rom.bytes();
    let maps = extract_maps(data, &Decodes::new(data)).unwrap();
    let updates = extract_map_updates(data, maps.real()).unwrap();
    let c = Value::from(&updates.census);
    assert_eq!(c["routine_count"], 64);
    assert_eq!(c["map_count"], 361);
    let routines = c["routines"].as_array().unwrap();
    assert_eq!(
        routines
            .iter()
            .map(|r| r["index"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        (0..64).collect::<Vec<_>>()
    );
    for record in maps.real() {
        assert_eq!(
            updates.per_map[&record.id]
                .iter()
                .map(|e| Value::from(e)["index"].as_u64().unwrap())
                .collect::<Vec<_>>(),
            record
                .map_updates
                .ids
                .iter()
                .map(|&id| u64::from(id))
                .collect::<Vec<_>>(),
            "map {}",
            record.id
        );
    }
    // A reversed or deduplicated map list must also fail for a synthetic
    // ordering with repetitions, independent of today's cartridge lists.
    let mut record = maps.real().find(|r| r.id == 0x54).unwrap().clone();
    record.map_updates.ids = vec![2, 1, 2, 0];
    let ordered = extract_map_updates(data, std::iter::once(&record)).unwrap();
    assert_eq!(
        ordered.per_map[&record.id]
            .iter()
            .map(|e| Value::from(e)["index"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        [2, 1, 2, 0]
    );
    let water = Value::from(&updates.per_map[&0x54][0]);
    assert_eq!(updates.per_map[&0x54].len(), 1);
    assert_eq!(water["index"], 2);
    let cycles = water["program"]["cycles"].as_array().unwrap();
    assert_eq!(cycles.len(), 1);
    let cycle = &cycles[0];
    assert_eq!(cycle["frame_mask"], 7);
    assert_eq!(
        cycle["phase"],
        json!({"kind":"frame","mask":0x18,"shift":3})
    );
    assert_eq!(cycle["frames"].as_array().unwrap().len(), 4);
    for phase in 0..4 {
        assert_eq!(
            cycle["frames"][phase],
            json!((0..4).map(|color|
            json!({"slot":26+color,"word":w(data,0x54b0c+phase*2+color*8).unwrap()})
        ).collect::<Vec<_>>())
        );
    }
    assert_eq!(routines[0x2d]["address"], "0x055C1C");
    assert_eq!(
        routines[0x2d]["program"],
        json!({"kind":"flags","chests":[11],"flag":0x72,"clear_chest":false})
    );
    assert_eq!(l(data, 0x55c22).unwrap(), 0x5762e);
    assert_eq!(w(data, 0x57634).unwrap(), 0xf120);
    assert_eq!(routines[0x1f]["program"]["cycles"][0]["phase"]["mask"], 0);
    let garuberk = routines[0x24]["program"]["cycles"][0]["frames"][0]
        .as_array()
        .unwrap();
    assert_eq!(
        garuberk.last().unwrap()["word"],
        w(data, 0x55952 + 0x14).unwrap()
    );
    assert_eq!(
        routines[0x2c]["program"]["cycles"][0]["frames"]
            .as_array()
            .unwrap()
            .len(),
        14
    );
    assert_eq!(
        routines
            .iter()
            .filter(|r| r["program"]["kind"] == "unsupported")
            .map(|r| r["index"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        [0x11, 0x12, 0x22]
    );
}

#[test]
fn corrupt_retail_branch_clock_and_map_index_are_rejected() {
    let Some(rom) = crate::test_support::retail_rom("map update refusals") else {
        return;
    };
    let data = rom.bytes();
    let maps = extract_maps(data, &Decodes::new(data)).unwrap();
    let table = dispatch_table(data).unwrap();
    let mut broken = data.to_vec();
    broken[table + 8..table + 10].copy_from_slice(&0x4e75u16.to_be_bytes());
    assert!(
        extract_map_updates(&broken, maps.real())
            .err()
            .unwrap()
            .0
            .contains("bra.w")
    );
    let mut record = maps.real().find(|r| r.id == 0x54).unwrap().clone();
    record.map_updates.ids = vec![64];
    assert!(
        extract_map_updates(data, std::iter::once(&record))
            .err()
            .unwrap()
            .0
            .contains("out of bounds")
    );
    broken.copy_from_slice(data);
    broken[0x54aec..0x54aee].copy_from_slice(&0x4e71u16.to_be_bytes());
    assert!(
        extract_map_updates(&broken, maps.real())
            .err()
            .unwrap()
            .0
            .contains("lsr.w")
    );
}
