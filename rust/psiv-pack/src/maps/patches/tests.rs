//! Synthetic chunks, no ROM data.

use super::super::draw::VRAM_TILES;
use super::super::layouts::{Blob, CollisionGrid, Layout, MapLayoutSpec};
use super::super::overworld::{LayoutPatch, PatchWrite};
use super::*;

fn blob() -> Blob {
    Blob {
        rom_offset: 0,
        compressed_length: 0,
        decompressed_length: 0,
        sha256: String::new(),
    }
}

fn layout(plane: Plane, cells: Vec<u8>, width_chunks: usize) -> Layout {
    let height_chunks = cells.len() / width_chunks;
    Layout {
        plane,
        width_chunks,
        height_chunks,
        cells,
        blob: blob(),
        anomaly: None,
    }
}

/// Tile 1 is colours 0 and 3 alternating; chunks are `words`.
fn decoded(words: Vec<[u16; 16]>, fg: Vec<u8>, bg: Vec<u8>, plane: u8) -> MapLayout {
    let mut tiles = vec![[0u8; 64]; VRAM_TILES];
    let mut loaded = vec![false; VRAM_TILES];
    for (i, pixel) in tiles[1].iter_mut().enumerate() {
        *pixel = if i % 2 == 0 { 0 } else { 3 };
    }
    loaded[1] = true;
    MapLayout {
        spec: MapLayoutSpec {
            chunk_blobs: Vec::new(),
            layout_fg: 0,
            layout_bg: 0,
            width_chunks_fg: 2,
            height_chunks_fg: 1,
            width_chunks_bg: 2,
            height_chunks_bg: 1,
            collision_plane: plane,
            tilesets: Vec::new(),
            palette: 0,
            label: "Test",
        },
        chunks: ChunkTable {
            words,
            blobs: Vec::new(),
        },
        fg: layout(Plane::Fg, fg, 2),
        bg: layout(Plane::Bg, bg, 2),
        collision: CollisionGrid {
            width: 0,
            height: 0,
            types: Vec::new(),
        },
        patterns: TilePatterns {
            tiles,
            loaded,
            tilesets: Vec::new(),
        },
    }
}

const PRIORITY: u16 = 1 << 15;
const LINE_1: u16 = 1 << 13;
const COLLIDE: u16 = 1 << 14;

fn palette() -> Vec<[u8; 3]> {
    (0..32).map(|i| [i, i, i]).collect()
}

#[test]
fn a_composed_chunk_keeps_colour_0_of_line_1_as_zero() {
    let map = decoded(vec![[1 | LINE_1; 16]], vec![0, 0], vec![0, 0], 0);
    let (tile, placed) = compose_chunk(&map.chunks, &map.patterns, 0, false).unwrap();
    assert_eq!(placed, 16);
    assert_eq!((tile[0], tile[1]), (0, 19));
    let (above, placed) = compose_chunk(&map.chunks, &map.patterns, 0, true).unwrap();
    assert_eq!(placed, 0);
    assert!(above.iter().all(|&p| p == 0));
}

#[test]
fn an_unloaded_priority_tile_is_not_placed() {
    let mut words = [0u16; 16];
    words[0] = 9 | PRIORITY;
    words[1] = 1 | PRIORITY;
    let map = decoded(vec![words], vec![0, 0], vec![0, 0], 0);
    let (_, placed) = compose_chunk(&map.chunks, &map.patterns, 0, true).unwrap();
    assert_eq!(placed, 1);
}

#[test]
fn the_overlay_atlas_exists_only_with_priority_pixels_or_composites() {
    let plain = decoded(vec![[1; 16]], vec![0, 0], vec![0, 0], 0);
    let ids = BTreeSet::from([0u8]);
    let (_, overlay, entries) = patch_atlas(&plain, &ids, &palette(), &[]).unwrap();
    assert!(overlay.is_none());
    assert_eq!(entries.len(), 1);
    assert_eq!(field(&entries[0], "chunk_id"), Some(&Json::from(0u8)));
    // A composite pair gets one even when it is fully transparent.
    let (_, overlay, entries) =
        patch_atlas(&plain, &BTreeSet::new(), &palette(), &[(0, 0)]).unwrap();
    assert!(overlay.is_some());
    assert_eq!(field(&entries[0], "chunk_id"), Some(&Json::Null));
    assert!(patch_atlas(&plain, &BTreeSet::new(), &palette(), &[]).is_err());
    let with = decoded(vec![[1 | PRIORITY; 16]], vec![0, 0], vec![0, 0], 0);
    let (_, overlay, _) = patch_atlas(&with, &ids, &palette(), &[]).unwrap();
    assert!(overlay.is_some());
}

#[test]
fn a_composite_takes_collision_from_the_collision_plane() {
    // Chunk 1 blocks everywhere; chunk 0 nowhere.
    let words = vec![[1; 16], [1 | COLLIDE; 16]];
    for (plane, collision) in [(0u8, 0xFu8), (1, 0)] {
        let map = decoded(words.clone(), vec![1, 0], vec![0, 0], plane);
        let (_, _, entries) = patch_atlas(&map, &BTreeSet::new(), &palette(), &[(1, 0)]).unwrap();
        assert_eq!(
            field(&entries[0], "collision"),
            Some(&Json::from(vec![collision; 4]))
        );
    }
}

fn effect(write: Json) -> Json {
    Json::object().with("paths", vec![Json::object().with("writes", vec![write])])
}

fn layout_write(chunk_id: i128, plane: &str) -> Json {
    Json::object()
        .with("kind", "layout_write")
        .with("at", "0x000010")
        .with("chunk_id", Json::Int(chunk_id))
        .with("plane", plane)
        .with("cell_x", 4)
        .with("cell_y", 6)
}

#[test]
fn a_write_resolves_to_cells_and_says_whether_collision_reads_it() {
    let map = decoded(vec![[1; 16], [1 | COLLIDE; 16]], vec![0, 0], vec![0, 0], 1);
    let effects = [
        effect(layout_write(1, "bg")),
        effect(layout_write(1, "fg")),
        effect(Json::object().with("kind", "object_despawn")),
        Json::object().with("decoded", false),
    ];
    let (resolved, chunks, counts) = resolve_map_effects(&map, &effects).unwrap();
    assert_eq!(chunks, [1, 1]);
    assert_eq!(
        counts,
        PatchCounts {
            layout_writes: 2,
            collision_authoritative: 1,
            picture_only: 1,
            cells: 8,
            map_change_cells: 0,
        }
    );
    let write = &items(&items(&resolved[0], "paths")[0], "writes")[0];
    assert_eq!(
        field(write, "collision_authoritative"),
        Some(&Json::Bool(true))
    );
    let cells = items(write, "cells");
    assert_eq!(
        (int(&cells[3], "x"), int(&cells[3], "y")),
        (Some(5), Some(7))
    );
    assert_eq!(int(&cells[3], "collision"), Some(0xF));
    // An undecoded effect still comes back with an empty path list.
    assert_eq!(field(&resolved[3], "paths"), Some(&Json::Array(Vec::new())));
    let error = resolve_map_effects(&map, &[effect(layout_write(2, "bg"))]).unwrap_err();
    assert!(
        error.0.contains("outside the 2 this map loads"),
        "{}",
        error.0
    );
}

fn hook(flag: u16, plane: Plane, chunk_id: u8) -> LayoutPatch {
    LayoutPatch {
        plane,
        page: 0,
        routine: 0,
        event_flag: flag,
        writes: vec![PatchWrite {
            plane,
            chunk_x: 1,
            chunk_y: 0,
            chunk_ids: vec![chunk_id],
            displacement: 0,
        }],
    }
}

fn world(map: MapLayout, patches: Vec<LayoutPatch>) -> Overworld {
    let table = |plane| super::super::overworld::PageTable {
        plane,
        rom_offset: 0,
        pages: Vec::new(),
    };
    Overworld {
        map_id: 0,
        symbol: "Test",
        tables: [table(Plane::Fg), table(Plane::Bg)],
        layout: map,
        patches,
        anomalies: Vec::new(),
    }
}

#[test]
fn overworld_hooks_group_by_flag_and_may_not_overlap() {
    let map = decoded(vec![[1; 16]; 3], vec![0, 0], vec![0, 0], 0);
    let hooked = world(
        map.clone(),
        vec![hook(7, Plane::Fg, 2), hook(7, Plane::Bg, 1)],
    );
    let (groups, pairs) = resolve_overworld_patches(&map, Some(&hooked)).unwrap();
    assert_eq!(pairs, [(2, 1)]);
    assert_eq!(int(&groups[0], "event_flag"), Some(7));
    let tile = &items(&groups[0], "tiles")[0];
    assert_eq!(int(tile, "collision_chunk_id"), Some(2));
    let clash = world(
        map.clone(),
        vec![hook(7, Plane::Fg, 2), hook(8, Plane::Fg, 1)],
    );
    let error = resolve_overworld_patches(&map, Some(&clash)).unwrap_err();
    assert!(error.0.contains("overlapping flags 0x7/0x8"), "{}", error.0);
    let high = world(map.clone(), vec![hook(512, Plane::Fg, 2)]);
    assert!(resolve_overworld_patches(&map, Some(&high)).is_err());
    let missing = world(map.clone(), vec![hook(7, Plane::Fg, 9)]);
    assert!(resolve_overworld_patches(&map, Some(&missing)).is_err());
    assert_eq!(
        resolve_overworld_patches(&map, None).unwrap(),
        (Vec::new(), Vec::new())
    );
}

#[test]
fn a_write_or_tile_with_no_atlas_entry_is_refused() {
    let mut resolved = vec![effect(layout_write(5, "fg"))];
    let error = index_writes(&mut resolved, &[]).unwrap_err();
    assert!(error.0.starts_with("no atlas tile"), "{}", error.0);
    let tile = Json::object().with("index", 3).with("chunk_id", 5);
    index_writes(&mut resolved, &[tile]).unwrap();
    let write = &items(&items(&resolved[0], "paths")[0], "writes")[0];
    assert_eq!(int(write, "patch_tile"), Some(3));
}

#[test]
fn a_moved_or_incomplete_scene_table_is_refused() {
    let row = SceneChunkWrite {
        scene: "Test",
        map_id: 0,
        chunks: &[1],
        table: Some((1, 3)),
        checks: &[(0, "AB")],
    };
    assert_eq!(
        row_chunks(&[0xAB, 2, 3, 0xFF], &row).unwrap(),
        BTreeSet::from([1, 2, 3])
    );
    assert!(
        row_chunks(&[0xAC, 2, 3, 0xFF], &row)
            .unwrap_err()
            .0
            .contains("moved")
    );
    assert!(
        row_chunks(&[0xAB, 2, 3, 4], &row)
            .unwrap_err()
            .0
            .contains("incomplete")
    );
    assert!(
        row_chunks(&[0xAB, 0xFF, 3, 0xFF], &row)
            .unwrap_err()
            .0
            .contains("incomplete")
    );
    assert!(
        row_chunks(&[0xAB, 2], &row)
            .unwrap_err()
            .0
            .contains("incomplete")
    );
}

#[test]
fn the_door_guards_and_tables_are_pinned() {
    assert!(garuberk_door_chunks(&[]).unwrap_err().0.contains("guard"));
}
