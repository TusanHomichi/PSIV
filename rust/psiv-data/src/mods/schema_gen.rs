//! Single-owner JSON Schema (Draft 2020-12) generation from `psiv-data` types (#790).
//!
//! Every schema published under `docs/schemas/` is derived directly from
//! `psiv-data`'s Rust types via `schemars`, paired with [`super::patch::KEYED_ARRAYS`]
//! and [`super::schema_check::for_each_patchable_table`] so adding or renaming a
//! field in Rust automatically updates the published schema and fails the CI
//! sync guard until regenerated with `psiv-pack mod schema docs/schemas`.

use super::compat::{
    MAX_SUPPORTED_MOD_FORMAT_VERSION, MAX_SUPPORTED_PACK_FORMAT_VERSION,
    MIN_SUPPORTED_MOD_FORMAT_VERSION, MIN_SUPPORTED_PACK_FORMAT_VERSION, MOD_FORMAT_VERSION,
};
use super::dialogue_patch::is_dialogue_entry;
use super::grid::{CellValueKind, cell_grid_spec};
use super::profile::{MOD_PROFILE_FORMAT_VERSION, ModProfile};
use super::schema_check::for_each_patchable_table;
use super::tables::{array_key_field, is_dynamic_map};
use super::{ModError, ModManifest, WINDOWS_RESERVED_NAMES};
use crate::dialogue::DIALOGUE_FORMAT_VERSION;
use crate::manifest::PACK_FORMAT_VERSION;
use crate::sound::SOUND_FORMAT_VERSION;
use crate::sound_test::SOUND_TEST_VERSION;
use schemars::JsonSchema;
use schemars::generate::SchemaSettings;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

/// Relative filename of the published `mod.json` JSON Schema.
pub const MOD_SCHEMA_FILE: &str = "mod.schema.json";

/// Relative filename of the published `mod_profile.json` JSON Schema.
pub const MOD_PROFILE_SCHEMA_FILE: &str = "mod_profile.schema.json";

const DRAFT_2020_12: &str = "https://json-schema.org/draft/2020-12/schema";

/// Generates every deterministic JSON Schema document (relative path -> pretty
/// JSON string ending with `\n`) directly from `psiv-data`'s Rust types.
#[must_use]
pub fn generate_mod_schemas() -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    out.insert(
        MOD_SCHEMA_FILE.to_owned(),
        format_schema(&build_mod_manifest_schema()),
    );
    out.insert(
        MOD_PROFILE_SCHEMA_FILE.to_owned(),
        format_schema(&build_mod_profile_schema()),
    );

    macro_rules! emit_tables {
        (
            exact: [ $( ($pack:literal, $schema:literal, $ty:ty) ),+ $(,)? ],
            maps: ($map_sample:literal, $map_schema:literal, $map_ty:ty),
            sprites: ($sprite_sample:literal, $sprite_schema:literal, $sprite_ty:ty),
        ) => {
            $(
                out.insert(
                    $schema.to_owned(),
                    format_schema(&build_patch_schema::<$ty>($pack, $pack)),
                );
            )+
            out.insert(
                $map_schema.to_owned(),
                format_schema(&build_patch_schema::<$map_ty>(
                    $map_sample,
                    "maps/<id>_<symbol>.json",
                )),
            );
            out.insert(
                $sprite_schema.to_owned(),
                format_schema(&build_patch_schema::<$sprite_ty>(
                    $sprite_sample,
                    "sprites/<sheet>.json",
                )),
            );
        };
    }
    for_each_patchable_table!(emit_tables);

    out
}

/// Writes all generated JSON Schema documents under `out_dir`, removing any
/// stale `.schema.json` files left over from removed or renamed tables.
pub fn write_mod_schemas(out_dir: &Path) -> Result<Vec<String>, ModError> {
    let schemas = generate_mod_schemas();
    fs::create_dir_all(out_dir).map_err(|error| {
        ModError::new(
            "schema",
            out_dir.display().to_string(),
            format!("could not create schema directory: {error}"),
        )
    })?;

    let existing = collect_existing_schema_files(out_dir)?;
    for rel in existing {
        if !schemas.contains_key(&rel) {
            let stale = out_dir.join(&rel);
            fs::remove_file(&stale).map_err(|error| {
                ModError::new(
                    "schema",
                    rel,
                    format!("could not remove stale schema file: {error}"),
                )
            })?;
        }
    }

    let mut written = Vec::with_capacity(schemas.len());
    for (rel, body) in &schemas {
        let target = out_dir.join(rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                ModError::new("schema", rel, format!("could not create dir: {error}"))
            })?;
        }
        fs::write(&target, body).map_err(|error| {
            ModError::new("schema", rel, format!("could not write schema: {error}"))
        })?;
        written.push(rel.clone());
    }
    Ok(written)
}

/// Verifies that `dir` contains the exact generated JSON Schema files with zero
/// missing, drifted, or stale `.schema.json` files.
pub fn check_mod_schemas(dir: &Path) -> Result<(), Vec<String>> {
    let expected = generate_mod_schemas();
    let mut problems = Vec::new();
    if !dir.is_dir() {
        return Err(vec![format!(
            "schema directory `{}` does not exist (regenerate with `psiv-pack mod schema docs/schemas`)",
            dir.display()
        )]);
    }

    for (rel, expected_body) in &expected {
        let path = dir.join(rel);
        match fs::read_to_string(&path) {
            Ok(actual) => {
                if actual != *expected_body {
                    problems.push(format!(
                        "out-of-sync schema `{rel}` (regenerate with `psiv-pack mod schema docs/schemas`)"
                    ));
                }
            }
            Err(_) => {
                problems.push(format!(
                    "missing schema `{rel}` (regenerate with `psiv-pack mod schema docs/schemas`)"
                ));
            }
        }
    }

    match collect_existing_schema_files(dir) {
        Ok(existing) => {
            for rel in existing {
                if !expected.contains_key(&rel) {
                    problems.push(format!(
                        "stale schema `{rel}` has no owning psiv-data type (remove or regenerate with `psiv-pack mod schema docs/schemas`)"
                    ));
                }
            }
        }
        Err(error) => problems.push(error.to_string()),
    }

    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems)
    }
}

fn collect_existing_schema_files(root: &Path) -> Result<Vec<String>, ModError> {
    let mut out = Vec::new();
    if !root.is_dir() {
        return Ok(out);
    }
    collect_schema_files_recursive(root, root, &mut out)?;
    out.sort();
    Ok(out)
}

fn collect_schema_files_recursive(
    root: &Path,
    current: &Path,
    out: &mut Vec<String>,
) -> Result<(), ModError> {
    let entries = fs::read_dir(current).map_err(|error| {
        ModError::new(
            "schema",
            current.display().to_string(),
            format!("could not read schema directory: {error}"),
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            ModError::new(
                "schema",
                current.display().to_string(),
                format!("could not read directory entry: {error}"),
            )
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_schema_files_recursive(root, &path, out)?;
        } else if path.is_file() {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            if rel.ends_with(".schema.json") {
                out.push(rel);
            }
        }
    }
    Ok(())
}

fn raw_schema_value<T: JsonSchema>() -> Value {
    let generator = SchemaSettings::draft2020_12().into_generator();
    let root = generator.into_root_schema_for::<T>();
    serde_json::to_value(&root).expect("schemars Schema must serialize to JSON")
}

fn build_mod_manifest_schema() -> Value {
    let mut root = raw_schema_value::<ModManifest>();
    let obj = root
        .as_object_mut()
        .expect("ModManifest schema must be an object");
    obj.insert("$schema".to_owned(), json!(DRAFT_2020_12));
    obj.insert("title".to_owned(), json!("PSIV Mod Manifest (mod.json)"));
    obj.insert(
        "description".to_owned(),
        json!(
            "Schema for `<mod-dir>/mod.json`, generated directly from `psiv_data::mods::ModManifest`."
        ),
    );
    obj.insert("additionalProperties".to_owned(), json!(false));

    if let Some(props) = obj.get_mut("properties").and_then(Value::as_object_mut) {
        if let Some(fv) = props
            .get_mut("format_version")
            .and_then(Value::as_object_mut)
        {
            fv.insert(
                "minimum".to_owned(),
                json!(MIN_SUPPORTED_MOD_FORMAT_VERSION),
            );
            fv.insert(
                "maximum".to_owned(),
                json!(MAX_SUPPORTED_MOD_FORMAT_VERSION),
            );
            fv.insert("default".to_owned(), json!(MOD_FORMAT_VERSION));
        }
        if let Some(pfv) = props
            .get_mut("pack_format_version")
            .and_then(Value::as_object_mut)
        {
            pfv.insert(
                "minimum".to_owned(),
                json!(MIN_SUPPORTED_PACK_FORMAT_VERSION),
            );
            pfv.insert(
                "maximum".to_owned(),
                json!(MAX_SUPPORTED_PACK_FORMAT_VERSION),
            );
            pfv.insert("default".to_owned(), json!(PACK_FORMAT_VERSION));
        }
        if let Some(id) = props.get_mut("id").and_then(Value::as_object_mut) {
            id.insert("minLength".to_owned(), json!(1));
            id.insert("maxLength".to_owned(), json!(64));
            id.insert("pattern".to_owned(), json!("^[a-z0-9_-]+$"));
            id.insert("not".to_owned(), json!({ "enum": WINDOWS_RESERVED_NAMES }));
        }
        for non_blank in ["name", "version"] {
            if let Some(field) = props.get_mut(non_blank).and_then(Value::as_object_mut) {
                field.insert("minLength".to_owned(), json!(1));
                field.insert("pattern".to_owned(), json!("\\S"));
            }
        }
    }
    canonicalize_json(root)
}

fn build_mod_profile_schema() -> Value {
    let mut root = raw_schema_value::<ModProfile>();
    let raw_defs = extract_defs(&mut root);
    let mut out_defs = BTreeMap::new();
    let mut visited = BTreeSet::new();
    let mut transformed = transform_complete(&root, &raw_defs, &mut out_defs, &mut visited);

    let obj = transformed
        .as_object_mut()
        .expect("ModProfile schema must be an object");
    obj.insert("$schema".to_owned(), json!(DRAFT_2020_12));
    obj.insert(
        "title".to_owned(),
        json!("PSIV Save Mod Profile (mod_profile.json)"),
    );
    obj.insert(
        "description".to_owned(),
        json!(
            "Schema for `saves/mods/<profile>/mod_profile.json`, generated directly from `psiv_data::mods::ModProfile`."
        ),
    );
    if let Some(props) = obj.get_mut("properties").and_then(Value::as_object_mut)
        && let Some(fv) = props
            .get_mut("format_version")
            .and_then(Value::as_object_mut)
    {
        fv.insert("const".to_owned(), json!(MOD_PROFILE_FORMAT_VERSION));
    }
    attach_defs(obj, out_defs);
    canonicalize_json(transformed)
}

fn build_patch_schema<T: JsonSchema>(sample_rel_file: &str, display_target: &str) -> Value {
    let mut root = raw_schema_value::<T>();
    let raw_defs = extract_defs(&mut root);
    let mut out_defs = BTreeMap::new();
    let mut visited = BTreeSet::new();

    let mut path = Vec::new();
    let mut transformed = transform_sparse(
        &root,
        sample_rel_file,
        &mut path,
        &raw_defs,
        &mut out_defs,
        &mut visited,
    );

    let obj = transformed
        .as_object_mut()
        .expect("patch root schema must be an object");
    obj.insert("$schema".to_owned(), json!(DRAFT_2020_12));
    obj.insert(
        "title".to_owned(),
        json!(format!("PSIV Mod Patch Schema ({display_target})")),
    );
    obj.insert(
        "description".to_owned(),
        json!(format!(
            "Sparse JSON patch schema for `patches/{display_target}`, generated from `psiv-data` Rust types with strict unknown-field rejection (`additionalProperties: false`)."
        )),
    );
    if let Some(props) = obj.get_mut("properties").and_then(Value::as_object_mut)
        && let Some(fv) = props
            .get_mut("format_version")
            .and_then(Value::as_object_mut)
    {
        let expected = expected_table_format_version(sample_rel_file).unwrap_or_else(|| {
            panic!(
                "patchable table `{sample_rel_file}` has `format_version` without a loader-pinned constant"
            )
        });
        fv.insert("const".to_owned(), json!(expected));
    }
    attach_defs(obj, out_defs);
    canonicalize_json(transformed)
}

fn expected_table_format_version(sample_rel_file: &str) -> Option<u32> {
    match sample_rel_file {
        "battle/enemies.json"
        | "battle/equipment.json"
        | "battle/characters.json"
        | "battle/abilities.json"
        | "battle/formations.json"
        | "battle/levels.json"
        | "battle/enemy_animations.json"
        | "battle/art/backgrounds.json"
        | "battle/art/command_ui.json"
        | "battle/art/damage_digits.json"
        | "battle/art/fusion.json"
        | "game_start.json" => Some(PACK_FORMAT_VERSION),
        "dialogue/trees.json"
        | "dialogue/font.json"
        | "dialogue/portraits.json"
        | "dialogue/window.json" => Some(DIALOGUE_FORMAT_VERSION),
        "sound/index.json" | "sound/driver.json" | "sound/music.json" | "sound/sfx.json" => {
            Some(SOUND_FORMAT_VERSION)
        }
        "title/sound_test.json" => Some(SOUND_TEST_VERSION),
        "guild/guild.json" | "fifth/fifth.json" | "ship_menu/ship_menu.json" => Some(1),
        "travel.json" => Some(2),
        file if file.starts_with("maps/") && file.ends_with(".json") => Some(PACK_FORMAT_VERSION),
        file if file.starts_with("sprites/") && file.ends_with(".json") => {
            Some(PACK_FORMAT_VERSION)
        }
        _ => None,
    }
}

fn key_field_is_integer(item_schema: &Value, key_field: &str) -> bool {
    let Some(prop) = item_schema
        .as_object()
        .and_then(|o| o.get("properties"))
        .and_then(Value::as_object)
        .and_then(|p| p.get(key_field))
        .and_then(Value::as_object)
    else {
        return true;
    };
    match prop.get("type") {
        Some(Value::String(s)) => s == "integer",
        Some(Value::Array(arr)) => arr.iter().any(|v| v.as_str() == Some("integer")),
        _ => false,
    }
}

fn extract_defs(root: &mut Value) -> Map<String, Value> {
    root.as_object_mut()
        .and_then(|obj| obj.remove("$defs"))
        .and_then(|val| match val {
            Value::Object(map) => Some(map),
            _ => None,
        })
        .unwrap_or_default()
}

fn attach_defs(obj: &mut Map<String, Value>, out_defs: BTreeMap<String, Value>) {
    if !out_defs.is_empty() {
        let mut defs_map = Map::new();
        for (k, v) in out_defs {
            defs_map.insert(k, v);
        }
        obj.insert("$defs".to_owned(), Value::Object(defs_map));
    }
}

fn resolve_ref<'a>(node: &'a Value, raw_defs: &'a Map<String, Value>) -> Option<Value> {
    let obj = node.as_object()?;
    let ref_str = obj.get("$ref")?.as_str()?;
    let def_name = ref_str.strip_prefix("#/$defs/")?;
    let target = raw_defs.get(def_name)?;
    let mut merged = target.clone();
    if let Some(merged_obj) = merged.as_object_mut() {
        for (k, v) in obj {
            if k != "$ref" {
                merged_obj.insert(k.clone(), v.clone());
            }
        }
    }
    Some(merged)
}

fn is_null_schema(v: &Value) -> bool {
    v.as_object()
        .and_then(|obj| obj.get("type"))
        .and_then(Value::as_str)
        == Some("null")
}

fn is_array_schema(obj: &Map<String, Value>) -> (bool, bool) {
    match obj.get("type") {
        Some(Value::String(s)) if s == "array" => (true, false),
        Some(Value::Array(arr)) => {
            let has_array = arr.iter().any(|v| v.as_str() == Some("array"));
            let has_null = arr.iter().any(|v| v.as_str() == Some("null"));
            (has_array, has_null)
        }
        _ => (false, false),
    }
}

fn make_nullable(schema: Value) -> Value {
    match schema {
        Value::Object(mut obj) => {
            if let Some(ty) = obj.get("type").cloned() {
                match ty {
                    Value::String(s) if s == "null" => return Value::Object(obj),
                    Value::String(s)
                        if obj.len() <= 6
                            && !obj.contains_key("oneOf")
                            && !obj.contains_key("anyOf") =>
                    {
                        obj.insert("type".to_owned(), json!([s, "null"]));
                        return Value::Object(obj);
                    }
                    Value::Array(mut arr) => {
                        if !arr.iter().any(|v| v.as_str() == Some("null")) {
                            arr.push(json!("null"));
                            obj.insert("type".to_owned(), Value::Array(arr));
                        }
                        return Value::Object(obj);
                    }
                    _ => {}
                }
            }
            if let Some(any_of) = obj.get_mut("anyOf").and_then(Value::as_array_mut) {
                if !any_of.iter().any(is_null_schema) {
                    any_of.push(json!({"type": "null"}));
                }
                return Value::Object(obj);
            }
            if let Some(one_of) = obj.get_mut("oneOf").and_then(Value::as_array_mut) {
                if !one_of.iter().any(is_null_schema) {
                    one_of.push(json!({"type": "null"}));
                }
                return Value::Object(obj);
            }
            let desc = obj.remove("description");
            let mut wrap = Map::new();
            if let Some(d) = desc {
                wrap.insert("description".to_owned(), d);
            }
            wrap.insert(
                "anyOf".to_owned(),
                json!([Value::Object(obj), {"type": "null"}]),
            );
            Value::Object(wrap)
        }
        other => other,
    }
}

/// Transforms a schema node in **complete-record** mode (used for `mod_profile.json`
/// and for elements of unkeyed arrays that `apply_json_patch` replaces wholesale).
/// Preserves `required` fields and enforces `additionalProperties: false` on every
/// modeled struct.
fn transform_complete(
    node: &Value,
    raw_defs: &Map<String, Value>,
    out_defs: &mut BTreeMap<String, Value>,
    visited: &mut BTreeSet<String>,
) -> Value {
    let Some(obj) = node.as_object() else {
        return node.clone();
    };

    if let Some(ref_str) = obj.get("$ref").and_then(Value::as_str)
        && let Some(def_name) = ref_str.strip_prefix("#/$defs/")
    {
        if !visited.contains(def_name)
            && let Some(def_val) = raw_defs.get(def_name)
        {
            visited.insert(def_name.to_owned());
            let transformed_def = transform_complete(def_val, raw_defs, out_defs, visited);
            out_defs.insert(def_name.to_owned(), transformed_def);
        }
        return node.clone();
    }

    let mut out = obj.clone();
    out.remove("$schema");

    let has_properties = out.get("properties").is_some_and(Value::is_object);
    if let Some(props) = out.get_mut("properties").and_then(Value::as_object_mut) {
        for val in props.values_mut() {
            *val = transform_complete(val, raw_defs, out_defs, visited);
        }
    }
    if has_properties && !out.contains_key("additionalProperties") {
        out.insert("additionalProperties".to_owned(), Value::Bool(false));
    } else if let Some(ap) = out.get_mut("additionalProperties")
        && ap.is_object()
    {
        *ap = transform_complete(ap, raw_defs, out_defs, visited);
    }

    if let Some(items) = out.get_mut("items")
        && items.is_object()
    {
        *items = transform_complete(items, raw_defs, out_defs, visited);
    }
    for kw in ["prefixItems", "anyOf", "oneOf", "allOf"] {
        if let Some(arr) = out.get_mut(kw).and_then(Value::as_array_mut) {
            for item in arr.iter_mut() {
                *item = transform_complete(item, raw_defs, out_defs, visited);
            }
        }
    }
    Value::Object(out)
}

/// Transforms a schema node in **sparse-patch** mode for `patches/<rel_file>`.
fn transform_sparse(
    node: &Value,
    rel_file: &str,
    path: &mut Vec<String>,
    raw_defs: &Map<String, Value>,
    out_defs: &mut BTreeMap<String, Value>,
    visited: &mut BTreeSet<String>,
) -> Value {
    if let Some(resolved) = resolve_ref(node, raw_defs) {
        return transform_sparse(&resolved, rel_file, path, raw_defs, out_defs, visited);
    }

    let Some(obj) = node.as_object() else {
        return node.clone();
    };

    // Unpack `anyOf: [inner, {"type": "null"}]` (schemars' representation for Option<Struct>).
    if let Some(any_of) = obj.get("anyOf").and_then(Value::as_array)
        && any_of.len() == 2
        && any_of.iter().any(is_null_schema)
    {
        let inner = any_of
            .iter()
            .find(|v| !is_null_schema(v))
            .expect("non-null branch");
        let transformed_inner =
            transform_sparse(inner, rel_file, path, raw_defs, out_defs, visited);
        let mut nullable = make_nullable(transformed_inner);
        if let (Some(desc), Some(m)) = (obj.get("description"), nullable.as_object_mut()) {
            m.entry("description".to_owned())
                .or_insert_with(|| desc.clone());
        }
        return nullable;
    }

    let (key_field, grid_spec, dialogue_entry, dynamic_map) = {
        let path_refs: Vec<&str> = path.iter().map(String::as_str).collect();
        (
            array_key_field(rel_file, &path_refs),
            cell_grid_spec(rel_file, &path_refs),
            is_dialogue_entry(rel_file, &path_refs),
            is_dynamic_map(rel_file, &path_refs),
        )
    };

    // 1. Keyed array (`KEYED_ARRAYS`): accepts either ID-keyed object or array of keyed partial objects.
    let (is_arr, arr_nullable) = is_array_schema(obj);
    if is_arr {
        if let Some(key_field) = key_field {
            let item_schema = obj.get("items").unwrap_or(&Value::Bool(true));
            let base_def_name = item_schema
                .as_object()
                .and_then(|o| o.get("$ref"))
                .and_then(Value::as_str)
                .and_then(|r| r.strip_prefix("#/$defs/"));
            path.push("*".to_owned());
            let sparse_item =
                transform_sparse(item_schema, rel_file, path, raw_defs, out_defs, visited);
            path.pop();

            let key_is_int = key_field_is_integer(&sparse_item, key_field);
            let item_ref = if let Some(base_name) = base_def_name {
                let def_name = register_patch_def(out_defs, base_name, &sparse_item);
                json!({ "$ref": format!("#/$defs/{def_name}") })
            } else {
                sparse_item
            };

            let property_names = if key_is_int {
                json!({ "pattern": "^(0|[1-9][0-9]*)$" })
            } else {
                json!({ "minLength": 1 })
            };
            let object_form = json!({
                "type": "object",
                "description": format!("Sparse ID-keyed patch map keyed by `{key_field}`."),
                "propertyNames": property_names,
                "additionalProperties": item_ref
            });

            let mut array_item = item_ref;
            if let Some(item_obj) = array_item.as_object_mut() {
                item_obj.insert("required".to_owned(), json!([key_field]));
            }
            let array_form = json!({
                "type": "array",
                "description": format!("Array of partial record patches, each carrying `{key_field}`."),
                "items": array_item
            });

            let mut branches = vec![object_form, array_form];
            if arr_nullable {
                branches.push(json!({ "type": "null" }));
            }
            let mut out = Map::new();
            if let Some(desc) = obj.get("description") {
                out.insert("description".to_owned(), desc.clone());
            }
            out.insert("oneOf".to_owned(), Value::Array(branches));
            return Value::Object(out);
        }

        if let Some(grid_spec) = grid_spec {
            let max_val = match grid_spec.kind {
                CellValueKind::CollisionNibble => 15_u64,
                CellValueKind::U8 => u64::from(u8::MAX),
                CellValueKind::U16 => u64::from(u16::MAX),
            };
            let cell_tuple = json!({
                "type": "array",
                "prefixItems": [
                    { "type": "integer", "minimum": 0 },
                    { "type": "integer", "minimum": 0 },
                    { "type": "integer", "minimum": 0, "maximum": max_val }
                ],
                "minItems": 3,
                "maxItems": 3,
                "items": false
            });
            let cell_object = json!({
                "type": "object",
                "required": ["row", "col", "value"],
                "additionalProperties": false,
                "properties": {
                    "row": { "type": "integer", "minimum": 0 },
                    "col": { "type": "integer", "minimum": 0 },
                    "value": { "type": "integer", "minimum": 0, "maximum": max_val }
                }
            });
            let sparse_grid = json!({
                "type": "object",
                "description": "Sparse cell-level 2D grid patch (`[row, col, value]` tuples or `{\"row\", \"col\", \"value\"}` objects).",
                "required": ["cells"],
                "additionalProperties": false,
                "properties": {
                    "cells": {
                        "type": "array",
                        "items": { "oneOf": [cell_tuple, cell_object] }
                    }
                }
            });
            let wholesale = transform_complete(node, raw_defs, out_defs, visited);
            let mut out = Map::new();
            if let Some(desc) = obj.get("description") {
                out.insert("description".to_owned(), desc.clone());
            }
            out.insert("oneOf".to_owned(), json!([sparse_grid, wholesale]));
            return Value::Object(out);
        }

        // Unkeyed array: replaced wholesale in `apply_json_patch`, so items stay complete.
        return transform_complete(node, raw_defs, out_defs, visited);
    }

    // 2. Internally-tagged enum (`oneOf` of objects with a `const` discriminator):
    // keep each variant's required fields (complete replacement of tagged enum values)
    // with `additionalProperties: false` so `oneOf` stays unambiguous.
    if obj.get("oneOf").is_some_and(Value::is_array) && !obj.contains_key("properties") {
        return transform_complete(node, raw_defs, out_defs, visited);
    }

    let mut out = obj.clone();
    out.remove("$schema");

    // 3. Modeled Rust struct object (`properties` present).
    if let Some(orig_props) = obj.get("properties").and_then(Value::as_object) {
        let orig_required: BTreeSet<&str> = obj
            .get("required")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();

        out.remove("required");
        out.insert("additionalProperties".to_owned(), Value::Bool(false));

        let mut new_props = Map::new();
        for (prop_name, prop_schema) in orig_props {
            path.push(prop_name.clone());
            let mut transformed =
                transform_sparse(prop_schema, rel_file, path, raw_defs, out_defs, visited);
            path.pop();
            if !orig_required.contains(prop_name.as_str()) {
                // `#[serde(default)]` fields may be cleared with `null` in patches (matching `diff_json`).
                transformed = make_nullable(transformed);
            }
            new_props.insert(prop_name.clone(), transformed);
        }

        if dialogue_entry {
            inject_dialogue_shorthands(&mut new_props, &mut out);
        }
        out.insert("properties".to_owned(), Value::Object(new_props));
        return Value::Object(out);
    }

    // 4. Dynamic `BTreeMap` object (`additionalProperties` is a schema, no `properties`).
    if let Some(ap) = obj.get("additionalProperties")
        && ap.is_object()
    {
        path.push("*".to_owned());
        let mut value_schema = transform_sparse(ap, rel_file, path, raw_defs, out_defs, visited);
        path.pop();
        if dynamic_map {
            value_schema = make_nullable(value_schema);
        }
        out.insert("additionalProperties".to_owned(), value_schema);
        return Value::Object(out);
    }

    // 5. Composite or primitive schemas.
    for kw in ["anyOf", "allOf"] {
        if let Some(arr) = out.get_mut(kw).and_then(Value::as_array_mut) {
            for item in arr.iter_mut() {
                *item = transform_sparse(item, rel_file, path, raw_defs, out_defs, visited);
            }
        }
    }
    Value::Object(out)
}

fn inject_dialogue_shorthands(props: &mut Map<String, Value>, entry_obj: &mut Map<String, Value>) {
    props.insert(
        "replace".to_owned(),
        json!({
            "type": "array",
            "description": "Dialogue patch shorthand: list of [old, new] substring replacements applied across segments, pages, and text in lockstep.",
            "items": {
                "type": "array",
                "prefixItems": [
                    { "type": "string" },
                    { "type": "string" }
                ],
                "minItems": 2,
                "maxItems": 2,
                "items": false
            }
        }),
    );
    props.insert(
        "lines".to_owned(),
        json!({
            "description": "Dialogue patch shorthand: replaces entry text cleanly as a single page of lines (`[\"Line 1\", \"Line 2\"]`) or multiple pages (`[[\"P1 L1\", \"P1 L2\"], [\"P2\"]]`).",
            "oneOf": [
                {
                    "type": "array",
                    "minItems": 1,
                    "items": { "type": "string" }
                },
                {
                    "type": "array",
                    "minItems": 1,
                    "items": {
                        "type": "array",
                        "items": { "type": "string" }
                    }
                }
            ]
        }),
    );
    props.insert(
        "keep_portrait".to_owned(),
        json!({
            "type": "boolean",
            "default": true,
            "description": "Valid only alongside `lines`: whether to keep the entry's leading portrait control code."
        }),
    );
    entry_obj.insert(
        "dependentRequired".to_owned(),
        json!({
            "keep_portrait": ["lines"]
        }),
    );
}

fn register_patch_def(
    out_defs: &mut BTreeMap<String, Value>,
    base_name: &str,
    schema: &Value,
) -> String {
    let base_key = format!("{base_name}Patch");
    match out_defs.get(&base_key) {
        None => {
            out_defs.insert(base_key.clone(), schema.clone());
            base_key
        }
        Some(existing) if existing == schema => base_key,
        Some(_) => {
            let mut idx = 2u32;
            loop {
                let candidate = format!("{base_key}{idx}");
                match out_defs.get(&candidate) {
                    None => {
                        out_defs.insert(candidate.clone(), schema.clone());
                        return candidate;
                    }
                    Some(existing) if existing == schema => return candidate,
                    Some(_) => idx += 1,
                }
            }
        }
    }
}

fn canonicalize_json(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let sorted: BTreeMap<String, Value> = map
                .into_iter()
                .map(|(k, v)| (k, canonicalize_json(v)))
                .collect();
            let mut out = Map::new();
            for (k, v) in sorted {
                out.insert(k, v);
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.into_iter().map(canonicalize_json).collect()),
        other => other,
    }
}

fn format_schema(value: &Value) -> String {
    let mut text = serde_json::to_string_pretty(value).expect("schema JSON serialization");
    text.push('\n');
    text
}
