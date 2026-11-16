//! The formation checks of tests/test_formations.py,
//! tests/test_inline_formations.py and [retired test_battle_pack](https://github.com/TusanHomichi/PSIV/blob/902868b517fd6b8eb0aee15b5dd500623cc7954c/tests/test_battle_pack.py) (#549).
//! The constructed inline-record cases copy no cartridge bytes; the rest read
//! the local retail ROM, and two read the local disassembly clone's
//! uncompressed formation listings as an independent oracle.

use std::collections::BTreeSet;
use std::path::PathBuf;

use psiv_data::{EnemiesFile, EquipmentFile, FormationsFile};
use serde_json::{Value, json};

use super::*;
use crate::battle::tests::retail;

/// The one formation whose declared count disagrees with its entries.
const COUNT_MISMATCH: u16 = 0x177;
/// `lea (xxx).l, a0`.
const LEA_ABS_A0: [u8; 2] = [0x41, 0xF9];

fn rom(what: &str) -> Option<Vec<u8>> {
    crate::test_support::retail_rom(what).map(|rom| rom.bytes().to_vec())
}

fn files() -> Option<(&'static Value, FormationsFile)> {
    let retail = retail()?;
    Some((retail.file("formations"), retail.typed("formations")))
}

// Decompression: each documented range consumed exactly.

#[test]
fn every_block_decodes_to_its_documented_range() {
    let Some(rom) = rom("formation blocks") else {
        return;
    };
    let decodes = Decodes::new(&rom);
    let expected = [
        ("Battle_FormationIndexes", 0x77B, 0x1100),
        ("Battle_FormationData1", 0x442, 0x6AC),
        ("Battle_FormationData2", 0x457, 0x6B4),
        ("Battle_FormationData3", 0x462, 0x68E),
        ("Battle_FormationData4", 0x3F0, 0x604),
        ("Battle_BossFormationData", 0x96, 0x12C),
    ];
    let blocks: Vec<&Block> = std::iter::once(&INDEX_BLOCK)
        .chain(&BLOCKS)
        .chain([&BOSS_BLOCK])
        .collect();
    for (block, (label, compressed, decompressed)) in blocks.into_iter().zip(expected) {
        assert_eq!(block.label, label);
        let (bytes, source) = decompress(&rom, &decodes, block).unwrap();
        let source = Value::from(&source);
        assert_eq!(source["compressed_size"], compressed, "{label}");
        assert_eq!(bytes.len(), decompressed, "{label}");
        assert_eq!(source["decompressed_size"], decompressed, "{label}");
        assert_eq!(source["compression"], "kosinski", "{label}");
        assert_eq!(source["compressed_sha256"].as_str().unwrap().len(), 64);
    }
    let first = Value::from(&decompress(&rom, &decodes, &BLOCKS[0]).unwrap().1);
    assert_eq!(
        (&first["rom_offset"], &first["rom_end_exclusive"]),
        (&json!("0x283E6C"), &json!("0x2842AE"))
    );
}

#[test]
fn block_4_runs_to_the_boss_data_and_a_shorter_range_is_refused() {
    let Some(rom) = rom("formation block 4") else {
        return;
    };
    let decodes = Decodes::new(&rom);
    assert_eq!(BLOCKS[3].end, BOSS_BLOCK.start);
    // The disassembly's first inline range annotation for block 4.
    let short = Block {
        end: 0x284C3D,
        ..BLOCKS[3]
    };
    let error = decompress(&rom, &decodes, &short).err().unwrap();
    assert!(error.0.contains("documented range"), "{}", error.0);
}

#[test]
fn the_terminator_scan_agrees_with_the_structural_split() {
    let Some(rom) = rom("formation terminators") else {
        return;
    };
    let decodes = Decodes::new(&rom);
    for block in BLOCKS.iter().chain([&BOSS_BLOCK]) {
        let (bytes, _) = decompress(&rom, &decodes, block).unwrap();
        let terminators = bytes.iter().filter(|&&b| b == TERMINATOR).count();
        assert_eq!(split(&bytes).unwrap().len(), terminators, "{}", block.label);
    }
}

/// The disassembly's uncompressed formation listings, when the clone is here.
fn oracle_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../reference/ps4disasm/battles");
    if dir.is_dir() {
        return Some(dir);
    }
    eprintln!("disassembly clone absent: formation oracle skipped");
    None
}

const ORACLE_FILES: [&str; 5] = [
    "battle_formations_1.asm",
    "battle_formations_2.asm",
    "battle_formations_3.asm",
    "battle_formations_4.asm",
    "boss_formations.asm",
];

/// A `dc.b`-only listing's bytes, and each `EnemyID_` comment's value and
/// symbol. Anything but a comment, a blank or `dc.b` fails: the listing is
/// no longer the plain data this oracle assumes.
fn read_listing(path: &std::path::Path) -> (Vec<u8>, Vec<(u8, String)>) {
    let text = std::fs::read_to_string(path).unwrap();
    let (mut bytes, mut annotated) = (Vec::new(), Vec::new());
    for (number, raw) in text.lines().enumerate() {
        let (code, comment) = raw.split_once(';').unwrap_or((raw, ""));
        let code = code.trim();
        if code.is_empty() {
            continue;
        }
        let operands = code
            .strip_prefix("dc.b")
            .unwrap_or_else(|| panic!("{}:{}: not a dc.b listing", path.display(), number + 1));
        let values: Vec<u8> = operands
            .split(',')
            .map(|token| {
                let token = token.trim().strip_prefix('$').unwrap_or_else(|| {
                    panic!("{}:{}: non-hex operand", path.display(), number + 1)
                });
                u8::from_str_radix(token, 16).unwrap()
            })
            .collect();
        if let Some(symbol) = comment.trim().strip_prefix("EnemyID_") {
            annotated.push((values[0], symbol.trim_end_matches(',').to_owned()));
        }
        bytes.extend(values);
    }
    (bytes, annotated)
}

#[test]
fn decompressed_blocks_equal_the_disassemblys_plain_listings() {
    let Some(rom) = rom("formation oracle") else {
        return;
    };
    let Some(dir) = oracle_dir() else { return };
    let decodes = Decodes::new(&rom);
    for (block, file) in BLOCKS.iter().chain([&BOSS_BLOCK]).zip(ORACLE_FILES) {
        let (listing, _) = read_listing(&dir.join(file));
        let (bytes, _) = decompress(&rom, &decodes, block).unwrap();
        assert!(bytes == listing, "{} differs from {file}", block.label);
    }
}

#[test]
fn enemy_ids_are_zero_based_as_the_listings_annotate_them() {
    let Some(retail) = retail() else { return };
    let Some(dir) = oracle_dir() else { return };
    let enemies: EnemiesFile = retail.typed("enemies");
    let mut seen = std::collections::BTreeMap::new();
    for file in ORACLE_FILES {
        for (id, symbol) in read_listing(&dir.join(file)).1 {
            seen.entry(id).or_insert(symbol);
        }
    }
    assert!(seen.len() > 100, "{} annotated ids", seen.len());
    assert_eq!(seen[&0], "Helex");
    assert_eq!(seen[&1], "MonsterFly");
    for (id, symbol) in seen {
        let enemy = &enemies.enemies[usize::from(id)];
        assert_eq!((enemy.id, &enemy.symbol), (u16::from(id), &symbol));
    }
}

// The records.

#[test]
fn block_counts_and_contiguous_block_aligned_ids() {
    let Some((raw, file)) = files() else { return };
    let research = raw["formations"].as_array().unwrap();
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for record in research {
        *counts.entry(record["block"].as_str().unwrap()).or_default() += 1;
    }
    assert_eq!(
        counts.into_iter().collect::<Vec<_>>(),
        [
            ("block_1", 128),
            ("block_2", 128),
            ("block_3", 128),
            ("block_4", 120)
        ]
    );
    assert_eq!((file.formation_count, file.formations.len()), (504, 504));
    assert_eq!(
        (file.boss_formation_count, file.boss_formations.len()),
        (27, 27)
    );
    let ids: Vec<u16> = file.formations.iter().map(|f| f.id.unwrap()).collect();
    assert_eq!(ids, (0..0x1F8).collect::<Vec<u16>>());
    for (id, block) in [(0x080, "block_2"), (0x100, "block_3"), (0x180, "block_4")] {
        assert_eq!(research[id]["block"], block);
        assert_eq!(research[id]["index_in_block"], 0);
    }
    assert_eq!(research[0x1F7]["index_in_block"], 119);
    let bosses: Vec<u16> = file
        .boss_formations
        .iter()
        .map(|f| f.event_battle_index.unwrap())
        .collect();
    assert_eq!(bosses, (0..27).collect::<Vec<u16>>());
}

#[test]
fn formation_zero_under_the_packs_field_names() {
    let Some((raw, _)) = files() else { return };
    let first = &raw["formations"][0];
    for old in [
        "surprise_agility",
        "run_agility",
        "item_drop_rate",
        "dropped_item",
        "enemy_count",
        "enemy_count_matches_entries",
    ] {
        assert!(first.get(old).is_none(), "{old}");
    }
    let header = [
        ("block", json!("block_1")),
        ("index_in_block", json!(0)),
        ("ambush_chance", json!(16)),
        ("run_chance", json!(0)),
        ("can_run", json!(true)),
        ("drop_rate", json!(8)),
        ("drop_item", json!(0x80)),
        ("count", json!(2)),
        ("count_matches_entries", json!(true)),
        ("group_1_mask", json!("0x03")),
        ("group_2_mask", json!("0x00")),
        ("group_1_slots", json!([1, 2])),
        ("group_2_slots", json!([])),
    ];
    for (key, value) in header {
        assert_eq!(first[key], value, "{key}");
    }
    // (enemy, position) pairs, with the slot and the groups it belongs to.
    assert_eq!(
        first["enemies"],
        json!([
            {"slot": 1, "enemy_id": 1, "position": 14, "groups": [1]},
            {"slot": 2, "enemy_id": 1, "position": 26, "groups": [1]},
        ])
    );
}

#[test]
fn every_record_resolves_its_enemies_and_drops() {
    let Some(retail) = retail() else { return };
    let Some((_, file)) = files() else { return };
    let enemies: EnemiesFile = retail.typed("enemies");
    let items: EquipmentFile = retail.typed("equipment");
    // Item ids are one-based: `ItemID_Dagger = 1`, `ItemID_Antidote = $80`.
    assert_eq!(items.item(1).unwrap().symbol, "Dagger");
    assert_eq!(items.item(0x80).unwrap().symbol, "Antidote");
    let known: BTreeSet<u16> = enemies.enemies.iter().map(|e| e.id).collect();
    let mut used = BTreeSet::new();
    for formation in file.formations.iter().chain(&file.boss_formations) {
        let key = formation.key();
        assert!((1..=4).contains(&formation.enemies.len()), "{key}");
        for (index, enemy) in formation.enemies.iter().enumerate() {
            assert_eq!(usize::from(enemy.slot), index + 1, "{key}");
            assert!(known.contains(&enemy.enemy_id), "{key}: {}", enemy.enemy_id);
            used.insert(enemy.enemy_id);
        }
        if let Some(drop) = formation.drop_item {
            assert!(items.item(drop).is_some(), "{key}: item {drop}");
        }
    }
    assert!(used.len() > 100);
}

#[test]
fn the_one_count_mismatch_is_carried_as_census() {
    let Some((raw, file)) = files() else { return };
    // Formation $177 declares four enemies and lists three; the disassembly's
    // listing has the same bytes, so the pack reports it, not repairs it.
    assert_eq!(raw["census"]["count_mismatches"], json!([COUNT_MISMATCH]));
    let mismatched: Vec<u16> = file
        .formations
        .iter()
        .chain(&file.boss_formations)
        .filter(|f| !f.count_matches_entries)
        .map(|f| f.id.unwrap())
        .collect();
    assert_eq!(mismatched, [COUNT_MISMATCH]);
    let odd = &file.formations[usize::from(COUNT_MISMATCH)];
    assert_eq!((odd.count, odd.enemies.len()), (4, 3));
}

// The encounter groups, the map bindings and the overworld grids.

#[test]
fn the_encounter_groups_and_their_source() {
    let Some((raw, file)) = files() else { return };
    let groups = file.encounter_groups.as_ref().unwrap();
    assert_eq!((groups.group_count, groups.entries_per_group), (68, 32));
    assert_eq!(groups.groups.len(), 68);
    for (index, group) in groups.groups.iter().enumerate() {
        assert_eq!(group.group as usize, index);
        assert_eq!(group.formation_ids.len(), 32, "group {index}");
    }
    assert_eq!(groups.groups[0].formation_ids[..6], [0, 0, 1, 1, 2, 3]);
    let research = &raw["encounter_groups"];
    assert_eq!(research["group_size_bytes"], 64);
    assert_eq!(research["source"]["rom_offset"], "0x2836EC");
    assert_eq!(research["source"]["decompressed_size"], 68 * 64);
    // Referential integrity, reaching the last formation.
    let referenced: BTreeSet<u16> = groups
        .groups
        .iter()
        .flat_map(|g| g.formation_ids.iter().copied())
        .collect();
    assert!(referenced.iter().all(|&id| id < 0x1F8));
    assert_eq!(referenced.last(), Some(&0x1F7));
    assert_eq!(raw["census"]["formation_ids_referenced"], referenced.len());
}

#[test]
fn a_group_naming_a_missing_formation_is_refused() {
    let Some(rom) = rom("formation group references") else {
        return;
    };
    let decodes = Decodes::new(&rom);
    let mut known: BTreeSet<u16> = (0..0x1F8).collect();
    assert!(groups(&rom, &decodes, &known).is_ok());
    known.remove(&0x1F7);
    let error = groups(&rom, &decodes, &known).err().unwrap();
    assert!(error.0.contains("0x1F7"), "{}", error.0);
}

#[test]
fn every_map_id_is_bound_and_the_table_overrun_stays_visible() {
    let Some((raw, file)) = files() else { return };
    assert_eq!(file.map_bindings.len(), 417);
    let ids: Vec<u16> = file.map_bindings.iter().map(|b| b.map_id).collect();
    assert_eq!(ids, (0..417).collect::<Vec<u16>>());
    let modes: BTreeSet<&str> = file.map_bindings.iter().map(|b| b.mode.as_str()).collect();
    // Four modes: the map the 416-byte table cannot reach has its own.
    assert_eq!(
        modes,
        BTreeSet::from(["group", "none", "outside_table", "position_grid"])
    );
    let outside: Vec<(u16, &str)> = file
        .map_bindings
        .iter()
        .filter(|b| !b.in_table)
        .map(|b| (b.map_id, b.mode.as_str()))
        .collect();
    assert_eq!(outside, [(0x1A0, "outside_table")]);
    let census = &raw["census"];
    assert_eq!(census["table_covers_maps"], 416);
    assert_eq!(census["map_ids"], 417);
    assert_eq!(census["maps_outside_table"], 1);
}

#[test]
fn the_two_overworld_position_grids() {
    let Some((_, file)) = files() else { return };
    let mut grids: Vec<(&str, u32, u32)> = file
        .position_grids
        .iter()
        .map(|g| (g.name.as_str(), g.columns, g.rows))
        .collect();
    grids.sort();
    assert_eq!(grids, [("dezolis", 64, 32), ("motavia", 64, 64)]);
    for grid in &file.position_grids {
        assert_eq!(grid.cell_size_pixels, 64, "{}", grid.name);
        // Rows of columns, indexed [y][x] as `(y >> 6) * 64 + (x >> 6)` is.
        assert_eq!(grid.cells.len() as u32, grid.rows, "{}", grid.name);
        assert!(
            grid.cells
                .iter()
                .all(|row| row.len() as u32 == grid.columns)
        );
        assert_eq!(
            grid.indexing.as_deref(),
            Some("(character_y >> 6) * 64 + (character_x >> 6)")
        );
    }
}

// The inline records Fusion's and COMBINE's battle objects load.

/// A blank image holding `record` at every inline offset, with `loads[i]`
/// copies of the `lea` that names the i-th.
fn inline_image(record: &[u8], loads: [usize; 5]) -> Vec<u8> {
    let mut data = vec![0u8; 0x30000];
    let mut cursor = 0x100;
    for ((_, offset, _), count) in INLINE.iter().zip(loads) {
        data[*offset..offset + record.len()].copy_from_slice(record);
        for _ in 0..count {
            data[cursor..cursor + 2].copy_from_slice(&LEA_ABS_A0);
            data[cursor + 2..cursor + 6].copy_from_slice(&(*offset as u32).to_be_bytes());
            cursor += 16;
        }
    }
    data
}

/// Ambush 1, run 2, drop 3, item 4, one enemy, group masks; enemy 9 at
/// position 5; the terminator.
const RECORD: [u8; 10] = [1, 2, 3, 4, 1, 1, 0, 9, 5, TERMINATOR];

#[test]
fn each_inline_record_is_read_to_its_terminator() {
    let parsed = inline(&inline_image(&RECORD, [1; 5])).unwrap();
    let labels: Vec<&str> = parsed.iter().map(|(f, _)| f.label.as_str()).collect();
    assert_eq!(labels, INLINE.map(|(label, ..)| label));
    for (record, _) in &parsed {
        let f = &record.formation;
        assert_eq!((f.ambush_chance, f.run_chance, f.drop_rate), (1, 2, 3));
        assert_eq!(f.drop_item, Some(4));
        let enemies: Vec<(u8, u16, u8)> = f
            .enemies
            .iter()
            .map(|e| (e.slot, e.enemy_id, e.position))
            .collect();
        assert_eq!(enemies, [(1, 9, 5)]);
    }
}

#[test]
fn an_inline_record_with_no_lea_two_leas_or_no_terminator_is_refused() {
    let error = |image: Vec<u8>| inline(&image).err().unwrap().0;
    let none = error(inline_image(&RECORD, [0, 1, 1, 1, 1]));
    assert!(
        none.contains("expected one `lea`") && none.contains("found 0"),
        "{none}"
    );
    let two = error(inline_image(&RECORD, [1, 1, 2, 1, 1]));
    assert!(
        two.contains("loc_1308C") && two.contains("found 2"),
        "{two}"
    );
    let unterminated: Vec<u8> = (1..=16).collect();
    let open = error(inline_image(&unterminated, [1; 5]));
    assert!(open.contains("no terminator"), "{open}");
}

/// (slot, enemy, position).
type Seat = (u8, u16, u8);

#[test]
fn the_retail_inline_records_each_seat_their_enemies() {
    let Some((raw, file)) = files() else { return };
    let seated: Vec<(&str, Vec<Seat>)> = file
        .inline_formations
        .iter()
        .map(|inline| {
            let f = &inline.formation;
            assert_eq!(f.run_chance, 0, "{}", inline.label);
            assert!(f.count_matches_entries, "{}", inline.label);
            (
                inline.label.as_str(),
                f.enemies
                    .iter()
                    .map(|e| (e.slot, e.enemy_id, e.position))
                    .collect(),
            )
        })
        .collect();
    // Fusion's MetaSlug and loc_23C84's TwinArms; COMBINE's LifeDeletr,
    // FISSION's four JR.OOZE and the InfantWorm's SandWorm.
    assert_eq!(
        seated,
        [
            ("loc_1A2F4", vec![(1, 36, 0x14)]),
            ("loc_23D00", vec![(1, 87, 0x14)]),
            ("loc_1308C", vec![(1, 26, 0x14)]),
            (
                "loc_1987E",
                vec![(1, 35, 0x08), (2, 35, 0x10), (3, 35, 0x18), (4, 35, 0x20)]
            ),
            ("loc_224E8", vec![(1, 80, 0x14)]),
        ]
    );
    for record in raw["inline_formations"].as_array().unwrap() {
        assert!(record.get("run_agility").is_none());
        let keys: BTreeSet<&str> = record["enemies"][0]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            BTreeSet::from(["enemy_id", "groups", "position", "slot"])
        );
    }
}
