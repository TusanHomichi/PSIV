//! End-to-end tests for the data-driven scenes spike (#793).
//!
//! These tests use a self-contained synthetic `GameData` + `DialogueSet`
//! fixture so they always execute (with or without a local ROM pack) and
//! exercise the real `psiv-runtime` + `psiv-core` scene runner.

use std::collections::BTreeMap;
use std::sync::Arc;

use psiv_core::{Cell, Direction, EventIndex, Flag, Input, StepFrames};
use psiv_data::{
    CellRegion, DataSceneError, DialogueEntry, DialogueSet, DialogueTree, FontFile, GameData,
    Glyph, GlyphMetrics, Manifest, MapRecord, OpenAnimation, PACK_FORMAT_VERSION, Page, PageEnd,
    PixelRect, PortraitFile, PortraitGeometry, Role, RoleTile, ScrollArrow, Segment, TextWindow,
    TreeFile, TreeWindow, WindowFile, WindowGeometry, WindowPalette, WindowRecord,
};

use crate::{DataSceneLoadError, Runtime, RuntimeEvent};

const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const TREE1_ROM_ADDR: u32 = 0x07A000;
const TREE2_ROM_ADDR: u32 = 0x07B000;

fn synthetic_dialogue_set() -> Arc<DialogueSet> {
    let make_entry = |id: u16, line: &str| DialogueEntry {
        id,
        text: line.to_owned(),
        segments: vec![Segment::Text(line.to_owned())],
        pages: vec![Page {
            lines: vec![line.to_owned()],
            end: PageEnd::End,
        }],
    };
    let tree1_entries: Vec<DialogueEntry> = (0u16..=0x6D)
        .map(|id| make_entry(id, &format!("Tree 1 line {id}")))
        .collect();
    let tree2_entries: Vec<DialogueEntry> = (0u16..=2)
        .map(|id| make_entry(id, &format!("Tree 2 line {id}")))
        .collect();
    let entry_count = (tree1_entries.len() + tree2_entries.len()) as u32;

    let mut roles = BTreeMap::new();
    for (idx, role) in Role::ALL.iter().enumerate() {
        roles.insert(
            role.as_str().to_owned(),
            RoleTile {
                tile: idx as u32,
                x: (idx as i32) * 8,
                y: 0,
                width: 8,
                height: 8,
                flip_h: false,
                flip_v: false,
                priority: true,
            },
        );
    }

    Arc::new(DialogueSet {
        trees: TreeFile {
            format_version: PACK_FORMAT_VERSION,
            tree_count: 2,
            entry_count,
            trees: vec![
                DialogueTree {
                    tree: 1,
                    label: "DialogueTree1".to_owned(),
                    rom_offset: Some(format!("0x{TREE1_ROM_ADDR:06X}")),
                    is_talk_tree: false,
                    portrait_operand_bytes: 1,
                    entry_count: tree1_entries.len() as u32,
                    entries: tree1_entries,
                },
                DialogueTree {
                    tree: 2,
                    label: "DialogueTree2".to_owned(),
                    rom_offset: Some(format!("0x{TREE2_ROM_ADDR:06X}")),
                    is_talk_tree: false,
                    portrait_operand_bytes: 1,
                    entry_count: tree2_entries.len() as u32,
                    entries: tree2_entries,
                },
            ],
            window: TreeWindow {
                chars_per_line: 32,
                lines_per_window: 2,
                glyph_width: 8,
                glyph_height: 16,
                rect: PixelRect {
                    x: 32,
                    y: 160,
                    width: 256,
                    height: 32,
                },
                scroll_arrow: ScrollArrow {
                    screen_x: 280,
                    screen_y: 184,
                    png: "dialogue/arrow.png".to_owned(),
                    width: 16,
                    height: 8,
                },
            },
            system_messages: None,
            world_interaction: None,
            talk_options: None,
        },
        font: FontFile {
            format_version: PACK_FORMAT_VERSION,
            png: "dialogue/font.png".to_owned(),
            by_char: BTreeMap::from([(' ', 0)]),
            glyphs: vec![Glyph {
                byte: 0,
                char: Some(' '),
                x: 0,
                y: 0,
                width: 8,
                height: 16,
                blank: true,
            }],
            glyph: GlyphMetrics {
                count: 1,
                width: 8,
                height: 16,
            },
            unmapped_glyphs: Vec::new(),
        },
        window: WindowFile {
            format_version: PACK_FORMAT_VERSION,
            png: "dialogue/window.png".to_owned(),
            geometry: WindowGeometry {
                border_cells: 1,
                cell_pixels: 8,
                shadow: false,
                open_animation: OpenAnimation {
                    axis: "horizontal".to_owned(),
                    from: "center".to_owned(),
                    step_cells: 2,
                    frames_per_step: 1,
                },
            },
            roles,
            text_window: TextWindow {
                window: 0,
                rect: PixelRect {
                    x: 24,
                    y: 152,
                    width: 272,
                    height: 48,
                },
                chars_per_line: 32,
                lines_per_window: 2,
                glyph_width: 8,
                glyph_height: 16,
                interior: CellRegion {
                    x_cell: 4,
                    y_cell: 20,
                    width_cells: 32,
                    height_cells: 4,
                },
            },
            portrait_window: psiv_data::PortraitWindow {
                window: 1,
                rect: PixelRect {
                    x: 40,
                    y: 104,
                    width: 48,
                    height: 48,
                },
            },
            windows: vec![
                WindowRecord {
                    index: 0,
                    name: "dialogue".to_owned(),
                    rect: PixelRect {
                        x: 24,
                        y: 152,
                        width: 272,
                        height: 48,
                    },
                    width_cells: 34,
                    height_cells: 6,
                },
                WindowRecord {
                    index: 1,
                    name: "portrait".to_owned(),
                    rect: PixelRect {
                        x: 40,
                        y: 104,
                        width: 48,
                        height: 48,
                    },
                    width_cells: 6,
                    height_cells: 6,
                },
            ],
            menu_windows: Vec::new(),
            palette: WindowPalette {
                colors: vec![[0, 0, 0]; 16],
                fill_index: 14,
            },
        },
        portraits: PortraitFile {
            format_version: PACK_FORMAT_VERSION,
            count: 0,
            portraits: Vec::new(),
            geometry: PortraitGeometry {
                width: 48,
                height: 48,
                screen_x: 40,
                screen_y: 104,
                talk_slot_stride_tiles: 8,
            },
        },
    })
}

fn synthetic_pack() -> GameData {
    let map_primary_json = format!(
        r#"{{
            "format_version": {PACK_FORMAT_VERSION},
            "id": 16,
            "id_hex": "0x010",
            "symbol": "Piata",
            "record_offset": "0x11C690",
            "png": "maps/010_Piata.png",
            "dimensions": {{
                "width_cells": 4, "height_cells": 4,
                "width_chunks": 2, "height_chunks": 2,
                "width_pixels": 64, "height_pixels": 64,
                "cell_pixels": 16
            }},
            "collision": {{
                "plane": "bg", "plane_byte": 1,
                "width_cells": 4, "height_cells": 4,
                "rows": [[0,0,0,0],[0,0,0,0],[0,0,0,0],[0,0,0,0]]
            }},
            "music": {{"id": 132, "symbol": "MotabiaTown", "changes_music": true}},
            "flags": {{"poison": 0, "random_battles": 0, "town_teleport": 1, "dungeon_teleport_index": 0}},
            "dialogue_tree": 1,
            "warps": [{{
                "index": 0, "table": 2, "trigger": "map_change_tile",
                "record_offset": "0x11C6F6",
                "range": {{"id": 3, "name": "XYExact"}},
                "source": {{"x_byte": 3, "y_byte": 2, "x_cell": 3, "y_cell": 3}},
                "rect": {{"x": 3, "y": 3, "width": 1, "height": 1}},
                "target": {{"id": 17, "id_hex": "0x011", "symbol": "PiataAcademy"}},
                "destination": {{"x_cell": 1, "y_cell": 1}},
                "facing": {{"id": 4, "name": "up"}},
                "character_alignment": 4
            }}],
            "npcs": [
                {{
                    "index": 0, "record_offset": "0x11C734", "object_id": 60,
                    "symbol": "NPCType2", "x_pixels": 32, "y_pixels": 16,
                    "x_cell": 2, "y_cell": 1,
                    "facing": {{"id": 0, "name": "down"}},
                    "dialogue_id": 1, "art_tile": 792, "sprite_reason": "synthetic"
                }},
                {{
                    "index": 1, "record_offset": "0x11C744", "object_id": 61,
                    "symbol": "NPCType3", "x_pixels": 48, "y_pixels": 16,
                    "x_cell": 3, "y_cell": 1,
                    "facing": {{"id": 0, "name": "down"}},
                    "dialogue_id": 2, "art_tile": 796, "sprite_reason": "synthetic"
                }}
            ],
            "treasure_chests": [],
            "patch_tiles": {{
                "png": "maps/010_patches.png",
                "tile_pixels": 32,
                "count": 6,
                "tiles": [
                    {{"index": 0, "chunk_id": 66, "x": 0, "collision": [8, 8, 8, 8]}},
                    {{"index": 1, "chunk_id": 68, "x": 32, "collision": [8, 8, 8, 8]}},
                    {{"index": 2, "chunk_id": 67, "x": 64, "collision": [8, 8, 8, 8]}},
                    {{"index": 3, "chunk_id": 69, "x": 96, "collision": [8, 8, 8, 8]}},
                    {{"index": 4, "chunk_id": 41, "x": 128, "collision": [0, 0, 0, 0]}},
                    {{"index": 5, "chunk_id": 42, "x": 160, "collision": [0, 0, 0, 0]}}
                ]
            }}
        }}"#
    );

    let map_secondary_json = format!(
        r#"{{
            "format_version": {PACK_FORMAT_VERSION},
            "id": 17,
            "id_hex": "0x011",
            "symbol": "PiataAcademy",
            "record_offset": "0x11C800",
            "png": "maps/011_PiataAcademy.png",
            "dimensions": {{
                "width_cells": 4, "height_cells": 4,
                "width_chunks": 2, "height_chunks": 2,
                "width_pixels": 64, "height_pixels": 64,
                "cell_pixels": 16
            }},
            "collision": {{
                "plane": "bg", "plane_byte": 1,
                "width_cells": 4, "height_cells": 4,
                "rows": [[0,0,0,0],[0,0,0,0],[0,0,0,0],[0,0,0,0]]
            }},
            "music": {{"id": 132, "symbol": "MotabiaTown", "changes_music": true}},
            "flags": {{"poison": 0, "random_battles": 0, "town_teleport": 1, "dungeon_teleport_index": 0}},
            "dialogue_tree": 2,
            "warps": [{{
                "index": 0, "table": 2, "trigger": "map_change_tile",
                "record_offset": "0x11C850",
                "range": {{"id": 3, "name": "XYExact"}},
                "source": {{"x_byte": 1, "y_byte": 0, "x_cell": 1, "y_cell": 1}},
                "rect": {{"x": 1, "y": 1, "width": 1, "height": 1}},
                "target": {{"id": 16, "id_hex": "0x010", "symbol": "Piata"}},
                "destination": {{"x_cell": 1, "y_cell": 1}},
                "facing": {{"id": 0, "name": "down"}},
                "character_alignment": 4
            }}],
            "npcs": [],
            "treasure_chests": []
        }}"#
    );

    let manifest_json = format!(
        r#"{{
            "format_version": {PACK_FORMAT_VERSION},
            "generator": "psiv_tools.pack",
            "rom": {{"sha256": "{DIGEST}", "size_bytes": 3145728}},
            "collision": {{"cell_pixels": 16, "blocking_types": [8, 9, 10, 11, 12]}},
            "warps": {{
                "count": 2, "rect_units": "collision cells",
                "standing_cell_y_offset": 1, "without_map_change_cell": []
            }},
            "guild": {{"path": "guild/guild.json", "sha256": "x"}},
            "fifth": {{"path": "fifth/fifth.json", "sha256": "x"}},
            "map_count": 2,
            "maps": [
                {{
                    "id": 16, "id_hex": "0x010", "symbol": "Piata",
                    "json": "maps/010_Piata.json", "png": "maps/010_Piata.png",
                    "json_sha256": "{DIGEST}", "png_sha256": "{DIGEST}",
                    "width_cells": 4, "height_cells": 4,
                    "width_pixels": 64, "height_pixels": 64
                }},
                {{
                    "id": 17, "id_hex": "0x011", "symbol": "PiataAcademy",
                    "json": "maps/011_PiataAcademy.json", "png": "maps/011_PiataAcademy.png",
                    "json_sha256": "{DIGEST}", "png_sha256": "{DIGEST}",
                    "width_cells": 4, "height_cells": 4,
                    "width_pixels": 64, "height_pixels": 64
                }}
            ],
            "skipped": [],
            "unpacked_warp_targets": [],
            "unloaded_patterns": [],
            "layout_blob_size_mismatches": []
        }}"#
    );

    let manifest: Manifest = serde_json::from_str(&manifest_json).expect("manifest parses");
    let primary: MapRecord = serde_json::from_str(&map_primary_json).expect("map 0x010 parses");
    let secondary: MapRecord = serde_json::from_str(&map_secondary_json).expect("map 0x011 parses");
    let mut data =
        GameData::from_parts(manifest, vec![primary, secondary]).expect("pack validates");
    data.set_dialogue(synthetic_dialogue_set());
    data
}

fn runtime_at(cell: Cell) -> Runtime {
    Runtime::new(
        synthetic_pack(),
        0x010,
        cell,
        Direction::Down,
        StepFrames::default(),
    )
    .expect("synthetic runtime builds")
}

fn drive_scene_to_completion(rt: &mut Runtime) -> Vec<(usize, Vec<RuntimeEvent>)> {
    let mut trace = Vec::new();
    for tick in 0..200 {
        let events = rt.tick(Input::Neutral);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, RuntimeEvent::SceneFaulted { .. })),
            "scene faulted at tick {tick}: {events:?}"
        );
        let opened_dialogue = events
            .iter()
            .any(|e| matches!(e, RuntimeEvent::SceneDialogue { .. }));
        if !events.is_empty() {
            trace.push((tick, events));
        }
        if opened_dialogue {
            rt.dialogue_closed();
        }
        if !rt.scene_active() {
            return trace;
        }
    }
    panic!("scene did not finish within 200 ticks");
}

#[test]
fn data_defined_piata_chaz_alone_matches_compiled_scene_identically() {
    let json = r#"{
        "format_version": 1,
        "name": "Event_PiataChazAlone",
        "event_id": 160,
        "target_map": 16,
        "ops": [
            {
                "op": "run_dialogue",
                "source": { "kind": "entry", "id": 109 },
                "window": "standard"
            },
            {
                "op": "set_flag",
                "flag": { "bank": "event", "id": 21 },
                "value": true
            }
        ]
    }"#;

    let mut compiled = runtime_at(Cell::new(0, 0));
    assert!(compiled.install_scene(EventIndex(0x00A0)));
    let compiled_trace = drive_scene_to_completion(&mut compiled);

    let mut data_driven = runtime_at(Cell::new(0, 0));
    data_driven
        .register_data_scene_json(json)
        .expect("data scene compiles and validates");
    assert!(data_driven.install_scene(EventIndex(0x00A0)));
    let data_trace = drive_scene_to_completion(&mut data_driven);

    assert_eq!(data_trace, compiled_trace);
    assert_eq!(data_driven.game.snapshot(), compiled.game.snapshot());
    assert!(data_driven.game.is_set(Flag::event(0x15)));
}

#[test]
fn data_defined_bioplant_door_with_owned_chunk_slices_matches_compiled_scene() {
    let json = r#"{
        "format_version": 1,
        "name": "Event_BioPlantDoorOpening",
        "event_id": 8,
        "target_map": 16,
        "ops": [
            { "op": "play_sound", "id": 226 },
            {
                "op": "write_actor_map_chunks",
                "actor": { "kind": "party_member", "slot": 0 },
                "chunks": [
                    { "dx": 0, "dy": 0, "chunk_id": 66 },
                    { "dx": 0, "dy": 32, "chunk_id": 68 }
                ]
            },
            { "op": "wait_frames", "frames": 17 },
            {
                "op": "write_actor_map_chunks",
                "actor": { "kind": "party_member", "slot": 0 },
                "chunks": [
                    { "dx": 0, "dy": 0, "chunk_id": 67 },
                    { "dx": 0, "dy": 32, "chunk_id": 69 }
                ]
            },
            { "op": "wait_frames", "frames": 17 },
            {
                "op": "write_actor_map_chunks",
                "actor": { "kind": "party_member", "slot": 0 },
                "chunks": [
                    { "dx": 0, "dy": 0, "chunk_id": 41 },
                    { "dx": 0, "dy": 32, "chunk_id": 42 }
                ]
            },
            { "op": "wait_frames", "frames": 17 },
            {
                "op": "set_flag",
                "flag": { "bank": "temp", "id": 0 },
                "value": true
            }
        ]
    }"#;

    let mut compiled = runtime_at(Cell::new(0, 1));
    assert!(compiled.install_scene(EventIndex(8)));
    let compiled_trace = drive_scene_to_completion(&mut compiled);

    let mut data_driven = runtime_at(Cell::new(0, 1));
    let program = psiv_data::DataSceneProgram::from_json(json).expect("door scene parses");
    data_driven
        .register_data_scene(&program)
        .expect("door scene compiles and validates");
    assert!(data_driven.install_scene(EventIndex(8)));
    let data_trace = drive_scene_to_completion(&mut data_driven);

    assert_eq!(data_trace, compiled_trace);
    assert_eq!(
        data_driven.effects.chunk_patches,
        compiled.effects.chunk_patches
    );
    assert_eq!(
        data_driven.effects.chunk_patches,
        vec![(0, 0, 0x29), (0, 1, 0x2A)]
    );
    assert_eq!(data_driven.game.snapshot(), compiled.game.snapshot());
    assert!(data_driven.game.is_set(Flag::temp(0)));
}

#[test]
fn data_scene_overrides_existing_event_and_registers_custom_event_id() {
    let override_piata = r#"{
        "format_version": 1,
        "name": "Mod_OverridePiataChazAlone",
        "event_id": 160,
        "target_map": 16,
        "ops": [
            { "op": "add_money", "amount": 500 },
            {
                "op": "set_flag",
                "flag": { "bank": "event", "id": 99 },
                "value": true
            },
            { "op": "end" }
        ]
    }"#;

    let custom_event = format!(
        r#"{{
            "format_version": 1,
            "name": "Mod_CustomSideQuest",
            "event_id": 32513,
            "target_map": 16,
            "ops": [
                {{
                    "op": "branch_flag",
                    "flag": {{ "bank": "event", "id": 99 }},
                    "if_set": 1,
                    "if_clear": 4
                }},
                {{ "op": "set_dialogue_tree", "rom_addr": {TREE2_ROM_ADDR} }},
                {{
                    "op": "run_dialogue",
                    "source": {{ "kind": "entry", "id": 1 }},
                    "window": "standard"
                }},
                {{
                    "op": "set_flag",
                    "flag": {{ "bank": "event", "id": 100 }},
                    "value": true
                }},
                {{ "op": "end" }}
            ]
        }}"#
    );

    let mut rt = runtime_at(Cell::new(0, 0));
    let initial_money = rt.game.money();
    rt.register_data_scene_json(override_piata)
        .expect("override registers");
    rt.register_data_scene_json(&custom_event)
        .expect("custom event registers");

    // Overridden retail event $A0 executes mod ops instead of compiled ops.
    assert!(rt.install_scene(EventIndex(0x00A0)));
    drive_scene_to_completion(&mut rt);
    assert_eq!(rt.game.money(), initial_money + 500);
    assert!(rt.game.is_set(Flag::event(99)));
    assert!(!rt.game.is_set(Flag::event(0x15)));

    // Custom event $7F01 executes branching + dialogue tree switch + flag set.
    assert!(rt.install_scene(EventIndex(0x7F01)));
    let trace = drive_scene_to_completion(&mut rt);
    assert!(trace.iter().any(|(_, events)| {
        events
            .iter()
            .any(|e| matches!(e, RuntimeEvent::SceneDialogue { entry: 1 }))
    }));
    assert!(rt.game.is_set(Flag::event(100)));

    // Clearing the override restores the compiled retail scene.
    assert!(rt.clear_data_scene_override(EventIndex(0x00A0)));
    assert!(rt.install_scene(EventIndex(0x00A0)));
    drive_scene_to_completion(&mut rt);
    assert!(rt.game.is_set(Flag::event(0x15)));
}

#[test]
fn malformed_data_scenes_fail_closed_at_load_time() {
    let mut rt = runtime_at(Cell::new(0, 0));

    // 1. Out-of-bounds jump target fails closed naming op index.
    let bad_jump = r#"{
        "format_version": 1,
        "name": "BadJump",
        "event_id": 32520,
        "ops": [
            { "op": "jump", "to": 99 }
        ]
    }"#;
    let err = rt.register_data_scene_json(bad_jump).unwrap_err();
    assert!(
        matches!(err, DataSceneLoadError::Validation(ref v) if v.op_index == 0),
        "expected validation error at op 0, got {err:?}"
    );
    assert!(err.to_string().contains("scene op 0"));

    // 2. Unknown op fails closed.
    let unknown_op = r#"{
        "format_version": 1,
        "name": "BadOp",
        "event_id": 32520,
        "ops": [{ "op": "teleport_to_moon" }]
    }"#;
    assert!(matches!(
        rt.register_data_scene_json(unknown_op),
        Err(DataSceneLoadError::Envelope(DataSceneError::Json(_)))
    ));

    // 3. Unknown field fails closed (`deny_unknown_fields`).
    let unknown_field = r#"{
        "format_version": 1,
        "name": "BadField",
        "event_id": 32520,
        "ops": [{ "op": "wait", "ticks": 2, "surprise": true }]
    }"#;
    assert!(matches!(
        rt.register_data_scene_json(unknown_field),
        Err(DataSceneLoadError::Envelope(DataSceneError::Json(_)))
    ));

    // 4. Malformed operands (empty chunks slice, zero pan speed, out-of-range flag).
    let empty_chunks = r#"{
        "format_version": 1,
        "name": "EmptyChunks",
        "event_id": 32520,
        "target_map": 16,
        "ops": [
            {
                "op": "write_actor_map_chunks",
                "actor": { "kind": "party_member", "slot": 0 },
                "chunks": []
            }
        ]
    }"#;
    assert!(matches!(
        rt.register_data_scene_json(empty_chunks),
        Err(DataSceneLoadError::Validation(ref v)) if v.op_index == 0
    ));

    let zero_pan_speed = r#"{
        "format_version": 1,
        "name": "ZeroPan",
        "event_id": 32520,
        "ops": [
            {
                "op": "pan_camera",
                "subject": { "kind": "at", "x": 32, "y": 32 },
                "speed": 0
            }
        ]
    }"#;
    assert!(matches!(
        rt.register_data_scene_json(zero_pan_speed),
        Err(DataSceneLoadError::Validation(ref v)) if v.op_index == 0
    ));

    let bad_flag = r#"{
        "format_version": 1,
        "name": "BadFlag",
        "event_id": 32520,
        "ops": [
            { "op": "set_flag", "flag": { "bank": "temp", "id": 999 }, "value": true }
        ]
    }"#;
    assert!(matches!(
        rt.register_data_scene_json(bad_flag),
        Err(DataSceneLoadError::Validation(ref v)) if v.op_index == 0
    ));

    // 5. Pack cross-reference failures: unknown map, out-of-bounds NPC,
    //    unknown dialogue tree ROM address, out-of-bounds dialogue entry,
    //    and absent patch chunk id.
    let unknown_map = r#"{
        "format_version": 1,
        "name": "UnknownMap",
        "event_id": 32520,
        "target_map": 4095,
        "ops": [{ "op": "end" }]
    }"#;
    assert!(matches!(
        rt.register_data_scene_json(unknown_map),
        Err(DataSceneLoadError::PackCrossRef { op_index: None, .. })
    ));

    let bad_npc = r#"{
        "format_version": 1,
        "name": "BadNpc",
        "event_id": 32520,
        "target_map": 16,
        "ops": [
            { "op": "face", "actor": { "kind": "npc", "index": 9 }, "facing": "up" }
        ]
    }"#;
    assert!(matches!(
        rt.register_data_scene_json(bad_npc),
        Err(DataSceneLoadError::PackCrossRef {
            op_index: Some(0),
            ..
        })
    ));

    let bad_tree = r#"{
        "format_version": 1,
        "name": "BadTree",
        "event_id": 32520,
        "target_map": 16,
        "ops": [{ "op": "set_dialogue_tree", "rom_addr": 1193046 }]
    }"#;
    assert!(matches!(
        rt.register_data_scene_json(bad_tree),
        Err(DataSceneLoadError::PackCrossRef {
            op_index: Some(0),
            ..
        })
    ));

    let bad_entry = r#"{
        "format_version": 1,
        "name": "BadEntry",
        "event_id": 32520,
        "target_map": 16,
        "ops": [
            {
                "op": "run_dialogue",
                "source": { "kind": "entry", "id": 250 },
                "window": "standard"
            }
        ]
    }"#;
    assert!(matches!(
        rt.register_data_scene_json(bad_entry),
        Err(DataSceneLoadError::PackCrossRef {
            op_index: Some(0),
            ..
        })
    ));

    let bad_chunk = r#"{
        "format_version": 1,
        "name": "BadChunk",
        "event_id": 32520,
        "target_map": 16,
        "ops": [
            {
                "op": "write_map_chunks",
                "chunks": [{ "x": 0, "y": 0, "chunk_id": 255 }]
            }
        ]
    }"#;
    assert!(matches!(
        rt.register_data_scene_json(bad_chunk),
        Err(DataSceneLoadError::PackCrossRef {
            op_index: Some(0),
            ..
        })
    ));

    // None of the rejected loads registered event 32520.
    assert!(!rt.install_scene(EventIndex(32520)));
}

#[test]
fn data_scene_validation_tracks_branch_dependent_map_contexts() {
    let mut rt = runtime_at(Cell::new(1, 1));

    // Negative control: starts on map 0x011 (0 NPCs), branches around LoadMap 0x010 (2 NPCs),
    // and then references NPC 0 at op 2. Because the branch reaches op 2 still on map 0x011,
    // load-time validation must reject op 2 instead of trusting the skipped LoadMap.
    let branched_bad_npc = r#"{
        "format_version": 1,
        "name": "BranchedBadNpc",
        "event_id": 32521,
        "target_map": 17,
        "ops": [
            {
                "op": "branch_flag",
                "flag": { "bank": "event", "id": 1 },
                "if_set": 2,
                "if_clear": 1
            },
            {
                "op": "load_map",
                "map": 16,
                "prev_map": 17,
                "start_x": 2,
                "start_y": 2,
                "facing": "down",
                "align": 4,
                "clear_load_flags": 0
            },
            {
                "op": "face",
                "actor": { "kind": "npc", "index": 0 },
                "facing": "down"
            }
        ]
    }"#;
    let err = rt
        .register_data_scene_json(branched_bad_npc)
        .expect_err("branch bypassing LoadMap must reject NPC ref invalid on target_map 0x011");
    assert!(
        matches!(
            err,
            DataSceneLoadError::PackCrossRef {
                op_index: Some(2),
                ref reason
            } if reason.contains("0x011")
        ),
        "expected map 0x011 NPC rejection at op 2, got {err}"
    );

    // Positive control: guarding op 3 with SkipUnlessMap [16] ensures NPC 0 is only reachable
    // when current_map is 0x010 (which has 2 NPCs).
    let guarded_good_npc = r#"{
        "format_version": 1,
        "name": "GuardedGoodNpc",
        "event_id": 32522,
        "target_map": 17,
        "ops": [
            {
                "op": "branch_flag",
                "flag": { "bank": "event", "id": 1 },
                "if_set": 2,
                "if_clear": 1
            },
            {
                "op": "load_map",
                "map": 16,
                "prev_map": 17,
                "start_x": 2,
                "start_y": 2,
                "facing": "down",
                "align": 4,
                "clear_load_flags": 0
            },
            {
                "op": "skip_unless_map",
                "maps": [16],
                "skip": 1
            },
            {
                "op": "face",
                "actor": { "kind": "npc", "index": 0 },
                "facing": "down"
            }
        ]
    }"#;
    rt.register_data_scene_json(guarded_good_npc)
        .expect("SkipUnlessMap-guarded NPC ref must pass branch-aware validation");

    // Multi-map SkipUnlessMap with no target_map must validate fallthrough against every listed
    // map, rejecting NPC 0 because map 0x011 has no NPCs even though map 0x010 has 2.
    let multi_map_bad_npc = r#"{
        "format_version": 1,
        "name": "MultiMapBadNpc",
        "event_id": 32523,
        "ops": [
            {
                "op": "skip_unless_map",
                "maps": [16, 17],
                "skip": 1
            },
            {
                "op": "face",
                "actor": { "kind": "npc", "index": 0 },
                "facing": "down"
            }
        ]
    }"#;
    let err = rt
        .register_data_scene_json(multi_map_bad_npc)
        .expect_err("multi-map SkipUnlessMap must reject NPC ref invalid on map 0x011");
    assert!(
        matches!(
            err,
            DataSceneLoadError::PackCrossRef {
                op_index: Some(1),
                ref reason
            } if reason.contains("0x011")
        ),
        "expected map 0x011 NPC rejection at op 1, got {err}"
    );
}
