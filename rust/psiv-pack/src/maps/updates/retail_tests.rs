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
        routines[0x22]["program"],
        json!({
            "kind": "tile_dma_cycle",
            "gates": [
                {"bank": "temp", "flag": 0x15, "set": false},
                {"bank": "temp", "flag": 0x17, "set": false},
            ],
            "frame_mask": 7,
            "frame_count": 3,
        })
    );
    assert!(
        routines
            .iter()
            .all(|r| r["program"]["kind"] != "unsupported"),
        "all 64 retail map updates must decode cleanly"
    );
    let vah_fort = Value::from(
        updates.per_map[&0xCA]
            .iter()
            .find(|e| Value::from(*e)["index"] == 0x11)
            .unwrap(),
    );
    let vah_groups = vah_fort["program"]["groups"].as_array().unwrap();
    assert_eq!(vah_groups.len(), 6);
    assert_eq!(
        vah_groups
            .iter()
            .map(|g| g["chunks"].as_array().unwrap().len())
            .sum::<usize>(),
        28
    );
    assert_eq!(
        conveyor_belt_chunks(&updates.per_map[&0xCA]),
        (0xA8..=0xAF).collect::<Vec<u8>>()
    );
    let wpn_plnt = Value::from(
        updates.per_map[&0xC7]
            .iter()
            .find(|e| Value::from(*e)["index"] == 0x12)
            .unwrap(),
    );
    let wpn_groups = wpn_plnt["program"]["groups"].as_array().unwrap();
    assert_eq!(wpn_groups.len(), 6);
    assert_eq!(
        wpn_groups
            .iter()
            .map(|g| g["chunks"].as_array().unwrap().len())
            .sum::<usize>(),
        28
    );
    assert_eq!(
        conveyor_belt_chunks(&updates.per_map[&0xC7]),
        (0xA8..=0xAF).collect::<Vec<u8>>()
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
    // Corrupting MapUpdate_GaruberkTowerPart4's DMA length ($04F0 at $0558BC)
    // must be refused.
    broken.copy_from_slice(data);
    let dma_len_at = 0x55832 + 0x8A;
    assert_eq!(w(&broken, dma_len_at).unwrap(), 0x04F0);
    broken[dma_len_at..dma_len_at + 2].copy_from_slice(&0x0500u16.to_be_bytes());
    assert!(
        extract_map_updates(&broken, maps.real())
            .err()
            .unwrap()
            .0
            .contains("tile DMA cycle")
    );
    // Corrupting MapUpdate_VahFortConveyorBelts's even-frame refresh check
    // (`btst #0, (Main_Frame_Count+1).w` at $05502A) must be refused.
    broken.copy_from_slice(data);
    assert_eq!(w(&broken, 0x5502A).unwrap(), 0x0838);
    broken[0x5502A..0x5502C].copy_from_slice(&0x4E71u16.to_be_bytes());
    assert!(
        extract_map_updates(&broken, maps.real())
            .err()
            .unwrap()
            .0
            .contains("conveyor belt")
    );
}
