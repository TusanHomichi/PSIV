//! Paged extraction contracts retained from test_overworld.py (#554).
//! Expected page bytes come from the local verified ROM, never a digest fixture.
use super::*;
use crate::maps::extract_maps;
use serde_json::{Value, json};
use std::sync::OnceLock;

struct Retail {
    rom: crate::Rom,
    records: Vec<MapRecord>,
    worlds: Vec<Overworld>,
}
fn retail() -> Option<&'static Retail> {
    static INPUT: OnceLock<Option<Retail>> = OnceLock::new();
    INPUT
        .get_or_init(|| {
            let rom = crate::test_support::retail_rom("overworld extraction tests")?;
            let decodes = Decodes::new(rom.bytes());
            let records: Vec<_> = extract_maps(rom.bytes(), &decodes)
                .unwrap()
                .real()
                .cloned()
                .collect();
            let worlds = records
                .iter()
                .take(2)
                .map(|r| decode_overworld(rom.bytes(), &decodes, r).unwrap())
                .collect();
            Some(Retail {
                rom,
                records,
                worlds,
            })
        })
        .as_ref()
}

#[test]
fn geometry_and_planet_selection_match_the_loader() {
    assert_eq!((PAGE_BYTES, ROW_BYTES, PAGE_ROWS), (1024, 128, 8));
    assert_eq!((PAGE_COUNT, WIDTH_CHUNKS, HEIGHT_CHUNKS), (16, 128, 128));
    assert_eq!((WIDTH_PIXELS, HEIGHT_PIXELS), (4096, 4096));
    assert_eq!(HEIGHT_CHUNKS, PAGE_COUNT * PAGE_ROWS);
    assert_eq!((WINDOW_ROWS, WINDOW_PAGES), (32, 4));
    assert_eq!(
        WINDOW_ROWS * ROW_BYTES,
        (MAP_LAYOUT_BG - MAP_LAYOUT_FG) as usize
    );
    assert_eq!(
        (0..417).filter(|&id| is_overworld(id)).collect::<Vec<_>>(),
        [0, 1]
    );
    for site in [PAGE_TABLE_FG, PAGE_TABLE_BG, PATCH_TABLE_FG, PATCH_TABLE_BG] {
        assert_eq!(site.table_for(0), site.motavia);
        assert_eq!(site.table_for(1), site.dezolis);
        assert_ne!(site.motavia, site.dezolis);
    }
    assert_eq!(
        super::super::layouts::collision_type_name(MAP_CHANGE_COLLISION_TYPE),
        "map_change"
    );
    assert!(!super::super::layouts::is_blocking(
        MAP_CHANGE_COLLISION_TYPE
    ));
}

#[test]
fn pages_follow_the_table_including_aliases_and_row_boundaries() {
    let Some(r) = retail() else { return };
    for w in &r.worlds {
        let m = &w.layout;
        assert_eq!(
            (
                m.spec.width_chunks_fg,
                m.spec.height_chunks_fg,
                m.spec.width_chunks_bg,
                m.spec.height_chunks_bg
            ),
            (128, 128, 128, 128)
        );
        assert_eq!(m.spec.collision_plane(), Plane::Bg);
        assert_eq!(
            (m.spec.layout_fg, m.spec.layout_bg),
            (w.tables[0].rom_offset, w.tables[1].rom_offset)
        );
        for (table, layout) in w.tables.iter().zip([&m.fg, &m.bg]) {
            let distinct = if w.map_id == 0 { 16 } else { 8 };
            assert_eq!(table.distinct_pages(), distinct);
            assert_eq!(table.pages.len(), 16);
            assert_eq!(table.data_start(), table.rom_offset + 64);
            assert_eq!(table.data_end(), table.data_start() + distinct * 1024);
            assert_eq!(layout.anomaly.is_some(), w.map_id == 1);
            for (i, page) in table.pages.iter().enumerate() {
                let at = long_at(r.rom.bytes(), table.rom_offset + i * 4);
                assert_eq!(page.rom_offset, at);
                assert_eq!(page.index, i);
                assert_eq!(page.first_row(), i * 8);
                assert_eq!(
                    page.aliases,
                    if w.map_id == 1 && i >= 8 {
                        Some(7)
                    } else {
                        None
                    }
                );
                // Independent raw page copy checks every row and column, including
                // page 8 where Dezolis must alias page 7 rather than advance.
                assert_eq!(
                    &layout.cells[i * 1024..(i + 1) * 1024],
                    &r.rom.bytes()[at..at + 1024]
                );
                assert_eq!(
                    page.sha256,
                    hex(&Sha256::digest(&r.rom.bytes()[at..at + 1024]))
                );
            }
            assert_eq!(layout.cells.len(), 128 * 128);
            assert!(
                layout
                    .cells
                    .iter()
                    .all(|&id| usize::from(id) < m.chunks.words.len())
            );
            if w.map_id == 0 {
                assert_eq!(hex(&Sha256::digest(&layout.cells)), layout.blob.sha256);
            } else {
                let a = Value::from(layout.anomaly.as_ref().unwrap());
                assert_eq!(a["kind"], "aliased_pages");
                assert_eq!(a["distinct_pages"], 8);
            }
        }
        assert_eq!(w.tables[0].data_end(), w.tables[1].rom_offset);
        assert_eq!(w.tables[1].data_end(), r.records[w.map_id].palette.pointer);
        let (start, end, count) = if w.map_id == 0 {
            (0x1065a2, 0x107db7, 256)
        } else {
            (0x114914, 0x11557c, 128)
        };
        let [blob] = m.chunks.blobs.as_slice() else {
            panic!("chunk stream count")
        };
        assert_eq!(
            (
                blob.rom_offset,
                blob.rom_offset + blob.compressed_length,
                m.chunks.words.len()
            ),
            (start, end, count)
        );
        assert!(end < w.tables[0].rom_offset);
        let (high_fg, high_bg, distinct_fg, distinct_bg) = if w.map_id == 0 {
            (181, 210, 30, 158)
        } else {
            (103, 110, 14, 82)
        };
        for (l, high, distinct) in [(&m.fg, high_fg, distinct_fg), (&m.bg, high_bg, distinct_bg)] {
            assert_eq!(l.cells.iter().copied().max(), Some(high));
            assert_eq!(l.cells.iter().collect::<BTreeSet<_>>().len(), distinct);
        }
    }
}

#[test]
fn collision_reads_raw_bg_chunk_quadrants_and_retains_planet_terrain() {
    let Some(r) = retail() else { return };
    for w in &r.worlds {
        let m = &w.layout;
        let grid = &m.collision;
        assert_eq!((grid.width, grid.height), (256, 256));
        for y in 0..256 {
            for x in 0..256 {
                let id = m.bg.cells[(y / 2) * 128 + x / 2];
                let words = &m.chunks.words[usize::from(id)];
                let at = (y % 2) * 8 + (x % 2) * 2;
                let bit = |i: usize| u8::from(words[i] & 0x4000 != 0);
                assert_eq!(
                    grid.types[y * 256 + x],
                    bit(at) + 2 * bit(at + 1) + 4 * bit(at + 4) + 8 * bit(at + 5)
                );
            }
        }
        let counts = grid.histogram();
        let expected = if w.map_id == 0 {
            [(0, 14956), (1, 123), (8, 6307), (9, 43518), (10, 632)].as_slice()
        } else {
            [(0, 8267), (1, 78), (8, 55487), (11, 1704)].as_slice()
        };
        for (i, &count) in counts.iter().enumerate() {
            assert_eq!(
                count,
                expected
                    .iter()
                    .find(|&&(k, _)| k == i)
                    .map_or(0, |&(_, n)| n)
            );
        }
        if w.map_id == 1 {
            for y in 128..256 {
                assert_eq!(
                    &grid.types[y * 256..(y + 1) * 256],
                    &grid.types[(112 + y % 16) * 256..(113 + y % 16) * 256]
                );
            }
            assert_eq!(
                grid.types[128 * 256..]
                    .iter()
                    .filter(|&&v| !super::super::layouts::is_blocking(v))
                    .count(),
                128
            );
            assert!(
                r.records[1]
                    .transitions_2
                    .entries
                    .iter()
                    .all(|t| t.record.source_y_tile < 128)
            );
        }
    }
    let c = &r.worlds[0].layout.collision;
    assert_eq!(c.types[143 * 256 + 46], 1);
    assert_eq!(c.types[0], 9);
}

#[test]
fn retail_patches_are_page_hooks_without_duplicate_writes() {
    let Some(r) = retail() else { return };
    for w in &r.worlds {
        assert_eq!(w.patches.len(), if w.map_id == 0 { 9 } else { 3 });
        for plane in PLANES {
            let routines = patches::read_patch_table(r.rom.bytes(), w.map_id, plane).unwrap();
            let mut written = BTreeSet::new();
            for (page, &routine) in routines.iter().enumerate() {
                let hooks: Vec<_> = w
                    .patches
                    .iter()
                    .filter(|p| p.plane == plane && p.page == page)
                    .collect();
                if hooks.is_empty() {
                    assert_eq!(
                        &r.rom.bytes()[routine..routine + 2],
                        &0x4e75u16.to_be_bytes()
                    );
                }
                for p in hooks {
                    assert_eq!(p.routine, routine);
                    assert!(p.event_symbol().is_some());
                    assert!(p.writes.iter().all(|w| w.chunk_y / 8 == page));
                }
            }
            for write in w
                .patches
                .iter()
                .flat_map(|p| &p.writes)
                .filter(|w| w.plane == plane)
            {
                for step in 0..write.chunk_ids.len() {
                    assert!(written.insert((write.chunk_x + step, write.chunk_y)));
                }
            }
        }
        let v = Value::from(&w.to_json());
        assert_eq!(v["compression"], Value::Null);
        assert_eq!(v["page_bytes"], 1024);
        assert_eq!(v["page_count"], 16);
        assert_eq!(v["window_rows"], 32);
        assert_eq!(v["patch_count"], w.patches.len());
        assert_eq!(v["wraps"], json!({"x_pixels":4096,"y_pixels":4096}));
        if w.map_id == 1 {
            assert_eq!(
                v["patched_events"],
                json!(["EventFlag_DarkForce2", "EventFlag_DezoSpaceport"])
            );
            assert_eq!(v["anomalies"].as_array().unwrap().len(), 2);
            assert_eq!(v["tables"]["bg"]["distinct_pages"], 8);
        }
    }
}

#[test]
fn every_loader_signature_and_invalid_record_path_is_guarded() {
    let Some(r) = retail() else { return };
    let report = Value::from(&check_code_sites(r.rom.bytes()).unwrap());
    assert_eq!(report["page_copy"]["longs_copied"], 256);
    assert_eq!(report["page_copy"]["compression"], Value::Null);
    assert_eq!(report["get_map_layout_chunk"]["row_bytes"], 128);
    assert_eq!(report["get_map_layout_chunk"]["window_rows"], 32);
    assert_eq!(GET_MAP_LAYOUT_CHUNK_BG - GET_MAP_LAYOUT_CHUNK_FG, 6);
    for (at, size) in [
        (PAGE_TABLE_FG.rom_offset, 20),
        (PAGE_TABLE_BG.rom_offset, 20),
        (PATCH_TABLE_FG.rom_offset, 20),
        (PATCH_TABLE_BG.rom_offset, 20),
        (PAGE_COPY_FG, 12),
        (PAGE_COPY_BG, 12),
        (GET_MAP_LAYOUT_CHUNK_FG, 24),
    ] {
        for i in 0..size {
            let mut broken = r.rom.bytes().to_vec();
            broken[at + i] ^= 1;
            assert!(
                check_code_sites(&broken).is_err(),
                "unguarded byte {:#x}",
                at + i
            );
        }
    }
    let mut broken = r.rom.bytes().to_vec();
    broken[PAGE_TABLE_FG.rom_offset + 2] ^= 0xff;
    assert!(read_page_table(&broken, 0, Plane::Fg).is_err());
    let interior = r.records.iter().find(|r| r.id == 0x10).unwrap();
    assert!(overworld_spec(interior).is_err());
    assert!(decode_overworld(&[], &Decodes::new(&[]), interior).is_err());
    assert!(decode_patches(&[], 0x10).is_err());
    assert!(patches::read_patch_table(&[], 0x10, Plane::Bg).is_err());
    let mut bad = r.records[0].clone();
    bad.layout.planes = interior.layout.planes;
    assert!(overworld_spec(&bad).is_err());
}

#[test]
fn reference_names_the_retail_loader_and_patch_symbols() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference/ps4disasm");
    let read = |name| match std::fs::read_to_string(root.join(name)) {
        Ok(s) => Some(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("local disassembly absent: overworld symbol check skipped");
            None
        }
        Err(e) => panic!("cannot read reference {name}: {e}"),
    };
    let Some(asm) = read("ps4.asm") else { return };
    let constants = read("ps4.constants.asm").unwrap();
    for text in [
        "loc_539E2:",
        "\tbeq.w\tloc_53A26",
        "\tbeq.w\tloc_53A8E",
        "btst\t#0, ($FFFFEC29).w",
        "GetMapLayoutChunkFG:",
        "GetMapLayoutChunkBG:",
        "\tandi.w\t#$1F, d1\n\tlsl.w\t#7, d1",
    ] {
        assert!(asm.contains(text), "{text}");
    }
    assert!(constants.contains("Field_Map_Index = ramaddr($FFFFEC28)"));
    for label in ["loc_107DC2", "loc_10BE02", "loc_115584", "loc_1175C4"] {
        assert!(asm.contains(&format!("lea\t({label}).l, a2")));
    }
    for (value, symbol) in
        (0..512).filter_map(|v| crate::map_symbols::event_flag_symbol(v).map(|s| (v, s)))
    {
        let literal = if value > 9 {
            format!("${value:X}")
        } else {
            value.to_string()
        };
        assert!((asm.clone() + &constants).contains(&format!("{symbol} = {literal}")));
    }
    assert!(asm.contains("bra.w\tMapDataMan_MovingPlatforms\t; $14"));
    assert!(asm.contains("bra.w\tMapDataMan_Dezolis\t; $26"));
    let body = asm
        .split_once("MapDataMan_Dezolis:")
        .unwrap()
        .1
        .split("; ---")
        .next()
        .unwrap();
    assert!(body.contains("Chunk_Table"));
    assert!(!body.contains("Map_Layout"));
}
