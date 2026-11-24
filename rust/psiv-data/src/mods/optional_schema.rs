//! Schema validation for newly inserted or `null`-replaced optional pack fields.
//!
//! Retail pack files carry unmodeled provenance keys on some outer structs
//! (`code_sha256`, `rom_offset`, `sha256`, etc.), so those outer structs cannot
//! wear `#[serde(deny_unknown_fields)]` directly without breaking retail pack
//! loading. When a mod inserts an omitted optional field or replaces `null`,
//! [`validate_optional_subtree`] enforces the modeled schema keys fail-closed.

use super::patch::{MAPS_JSON, format_path};
use serde_json::{Map, Value};

fn check_allowed_object_keys(
    obj: &Map<String, Value>,
    allowed: &[&str],
    rel_file: &str,
    path: &str,
) -> Result<(), String> {
    for key in obj.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(format!(
                "unknown field `{path}.{key}` (not in schema for `{rel_file}`)"
            ));
        }
    }
    Ok(())
}

/// Validates a newly inserted (or `null`-replaced) allowlisted optional field
/// against its modeled schema keys so typos fail closed.
pub(crate) fn validate_optional_subtree(
    rel_file: &str,
    schema_path: &[&str],
    field: &str,
    value: &Value,
    display_path: &[&str],
) -> Result<(), String> {
    let here = format_path(display_path, field);
    match (rel_file, schema_path, field) {
        (file, [], "interaction_areas") if MAPS_JSON.matches(file) => {
            if let Some(items) = value.as_array() {
                const KEYS: &[&str] = &[
                    "index",
                    "record_offset",
                    "range",
                    "source",
                    "rect",
                    "flag_type",
                    "flag",
                    "interaction_type",
                    "parameter",
                    "event_index",
                ];
                for (i, item) in items.iter().enumerate() {
                    if let Some(obj) = item.as_object() {
                        check_allowed_object_keys(obj, KEYS, rel_file, &format!("{here}[{i}]"))?;
                    }
                }
            }
        }
        (file, [], "map_updates") if MAPS_JSON.matches(file) => {
            if let Some(items) = value.as_array() {
                const KEYS: &[&str] = &["index", "routine", "address", "program"];
                for (i, item) in items.iter().enumerate() {
                    if let Some(obj) = item.as_object() {
                        check_allowed_object_keys(obj, KEYS, rel_file, &format!("{here}[{i}]"))?;
                    }
                }
            }
        }
        (file, [], "map_effects") if MAPS_JSON.matches(file) => {
            validate_map_effects(value, rel_file, &here)?;
        }
        (file, [], "layout_variants") if MAPS_JSON.matches(file) => {
            if let Some(items) = value.as_array() {
                const KEYS: &[&str] = &["png", "png_over", "planes", "collision", "vehicle_battle"];
                const COLL_KEYS: &[&str] = &["plane", "width_cells", "height_cells", "rows"];
                for (i, item) in items.iter().enumerate() {
                    let Some(obj) = item.as_object() else {
                        continue;
                    };
                    let vp = format!("{here}[{i}]");
                    check_allowed_object_keys(obj, KEYS, rel_file, &vp)?;
                    if let Some(cobj) = obj.get("collision").and_then(Value::as_object) {
                        check_allowed_object_keys(
                            cobj,
                            COLL_KEYS,
                            rel_file,
                            &format!("{vp}.collision"),
                        )?;
                    }
                }
            }
        }
        (file, [], "patch_tiles") if MAPS_JSON.matches(file) => {
            if let Some(obj) = value.as_object() {
                const KEYS: &[&str] = &["png", "png_over", "tile_pixels", "count", "tiles"];
                const TILE_KEYS: &[&str] = &[
                    "index",
                    "chunk_id",
                    "fg_chunk_id",
                    "bg_chunk_id",
                    "x",
                    "priority_tiles",
                    "collision",
                ];
                check_allowed_object_keys(obj, KEYS, rel_file, &here)?;
                if let Some(tiles) = obj.get("tiles").and_then(Value::as_array) {
                    for (i, tile) in tiles.iter().enumerate() {
                        if let Some(tobj) = tile.as_object() {
                            check_allowed_object_keys(
                                tobj,
                                TILE_KEYS,
                                rel_file,
                                &format!("{here}.tiles[{i}]"),
                            )?;
                        }
                    }
                }
            }
        }
        (file, [], "scroll") if MAPS_JSON.matches(file) => {
            if let Some(obj) = value.as_object() {
                const KEYS: &[&str] = &[
                    "mode",
                    "padding_byte",
                    "fg_scroll_mode",
                    "fg_scroll_word",
                    "fg_step_counters",
                    "bg_scroll_mode",
                    "bg_scroll_word",
                    "bg_step_counters",
                ];
                check_allowed_object_keys(obj, KEYS, rel_file, &here)?;
            }
        }
        ("shops.json", ["counters", "*"], "greeting") => {
            if let Some(obj) = value.as_object() {
                const KEYS: &[&str] = &["index", "trade_fragment", "text_variant"];
                check_allowed_object_keys(obj, KEYS, rel_file, &here)?;
            }
        }
        ("dialogue/trees.json", [], "system_messages") => {
            if let Some(obj) = value.as_object() {
                const KEYS: &[&str] = &["label", "messages", "cannot_get_off"];
                check_allowed_object_keys(obj, KEYS, rel_file, &here)?;
            }
        }
        ("dialogue/trees.json", [], "world_interaction") => {
            if let Some(obj) = value.as_object() {
                const KEYS: &[&str] = &["world_trees", "first_world_override"];
                check_allowed_object_keys(obj, KEYS, rel_file, &here)?;
            }
        }
        ("dialogue/trees.json", [], "talk_options") => {
            if let Some(obj) = value.as_object() {
                const KEYS: &[&str] = &[
                    "tree_low",
                    "tree_high",
                    "tree_high_from",
                    "default_entry",
                    "anger_tower_map",
                    "anger_tower_clear_flag",
                    "anger_tower_entry",
                    "entries",
                ];
                check_allowed_object_keys(obj, KEYS, rel_file, &here)?;
            }
        }
        (file, [], "palette_variants")
            if file.starts_with("sprites/") && file.ends_with(".json") =>
        {
            if let Some(items) = value.as_array() {
                const PV_KEYS: &[&str] = &["map_ids", "sheet_ids", "sheets"];
                const SHEET_KEYS: &[&str] = &[
                    "id",
                    "png",
                    "frame_width",
                    "frame_height",
                    "frame_count",
                    "origin_x",
                    "origin_y",
                    "sequences",
                ];
                for (i, item) in items.iter().enumerate() {
                    let Some(obj) = item.as_object() else {
                        continue;
                    };
                    let pvp = format!("{here}[{i}]");
                    check_allowed_object_keys(obj, PV_KEYS, rel_file, &pvp)?;
                    if let Some(sheets) = obj.get("sheets").and_then(Value::as_array) {
                        for (si, s) in sheets.iter().enumerate() {
                            if let Some(sobj) = s.as_object() {
                                check_allowed_object_keys(
                                    sobj,
                                    SHEET_KEYS,
                                    rel_file,
                                    &format!("{pvp}.sheets[{si}]"),
                                )?;
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_map_effects(value: &Value, rel_file: &str, here: &str) -> Result<(), String> {
    let Some(items) = value.as_array() else {
        return Ok(());
    };
    const EFFECT_KEYS: &[&str] = &["entry", "decoded", "kinds", "paths", "reason"];
    const PATH_KEYS: &[&str] = &[
        "gates",
        "unconditional",
        "aborts_remaining_entries",
        "deferred",
        "deferred_effects",
        "writes",
    ];
    const GATE_KEYS: &[&str] = &["bank", "flag", "required", "symbol"];
    const PAL_KEYS: &[&str] = &["resolved_as", "cram_line", "words_hex", "affects"];
    const AFFECTS_KEYS: &[&str] = &["npc_sheets"];
    const REPL_KEYS: &[&str] = &["npc_index", "from", "to"];
    const WRITE_KEYS: &[&str] = &[
        "kind",
        "at",
        "object_index",
        "object_id",
        "dialogue_id",
        "pixels",
        "cell_x",
        "cell_y",
        "chunk_id",
        "plane",
        "source",
        "bank",
        "flag",
        "cells",
        "collision_authoritative",
        "patch_tile",
    ];
    const CELL_KEYS: &[&str] = &["x", "y", "collision"];
    for (i, item) in items.iter().enumerate() {
        let Some(obj) = item.as_object() else {
            continue;
        };
        let ep = format!("{here}[{i}]");
        check_allowed_object_keys(obj, EFFECT_KEYS, rel_file, &ep)?;
        if let Some(paths) = obj.get("paths").and_then(Value::as_array) {
            for (pi, p) in paths.iter().enumerate() {
                let Some(pobj) = p.as_object() else {
                    continue;
                };
                let pp = format!("{ep}.paths[{pi}]");
                check_allowed_object_keys(pobj, PATH_KEYS, rel_file, &pp)?;
                if let Some(gates) = pobj.get("gates").and_then(Value::as_array) {
                    for (gi, g) in gates.iter().enumerate() {
                        if let Some(gobj) = g.as_object() {
                            check_allowed_object_keys(
                                gobj,
                                GATE_KEYS,
                                rel_file,
                                &format!("{pp}.gates[{gi}]"),
                            )?;
                        }
                    }
                }
                if let Some(defs) = pobj.get("deferred_effects").and_then(Value::as_array) {
                    for (di, d) in defs.iter().enumerate() {
                        let Some(dobj) = d.as_object() else {
                            continue;
                        };
                        let dp = format!("{pp}.deferred_effects[{di}]");
                        check_allowed_object_keys(dobj, PAL_KEYS, rel_file, &dp)?;
                        if let Some(aobj) = dobj.get("affects").and_then(Value::as_object) {
                            let ap = format!("{dp}.affects");
                            check_allowed_object_keys(aobj, AFFECTS_KEYS, rel_file, &ap)?;
                            if let Some(ns) = aobj.get("npc_sheets").and_then(Value::as_array) {
                                for (ni, n) in ns.iter().enumerate() {
                                    if let Some(nobj) = n.as_object() {
                                        check_allowed_object_keys(
                                            nobj,
                                            REPL_KEYS,
                                            rel_file,
                                            &format!("{ap}.npc_sheets[{ni}]"),
                                        )?;
                                    }
                                }
                            }
                        }
                    }
                }
                if let Some(writes) = pobj.get("writes").and_then(Value::as_array) {
                    for (wi, w) in writes.iter().enumerate() {
                        let Some(wobj) = w.as_object() else {
                            continue;
                        };
                        let wp = format!("{pp}.writes[{wi}]");
                        check_allowed_object_keys(wobj, WRITE_KEYS, rel_file, &wp)?;
                        if let Some(cells) = wobj.get("cells").and_then(Value::as_array) {
                            for (ci, c) in cells.iter().enumerate() {
                                if let Some(cobj) = c.as_object() {
                                    check_allowed_object_keys(
                                        cobj,
                                        CELL_KEYS,
                                        rel_file,
                                        &format!("{wp}.cells[{ci}]"),
                                    )?;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
