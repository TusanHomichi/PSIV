//! Sparse ID-keyed JSON patching, cell-level grid patching, and reverse diffing
//! for runtime pack tables.
//!
//! Pack tables store records in arrays keyed by stable cartridge IDs (see
//! [`super::tables::KEYED_ARRAYS`]) and 2D map/encounter grids (see
//! [`super::grid::CELL_GRIDS`]). Patching by ID or sparse cell coordinates
//! keeps mods resilient, readable, and free of copied retail tables.
//!
//! For `dialogue/trees.json` entries, two ergonomic helpers are also supported:
//!
//! - `"replace": [["old", "new"], ...]` — updates matching text in `segments`,
//!   `pages`, and `text` in lockstep.
//! - `"lines": ["Line 1", "Line 2"]` (or nested `[["Page 1 L1", "L2"], ["Page 2"]]`)
//!   — rebuilds `segments`, `pages`, and `text` cleanly (preserving any leading
//!   portrait control code unless `"keep_portrait": false`).

use super::dialogue_patch::{
    apply_dialogue_shorthand, is_dialogue_entry, synthesize_dialogue_shorthand,
};
use super::diff::DiffOptions;
use super::grid::{apply_cell_grid_patch, cell_grid_spec, diff_cell_grid};
use super::schema_check;
use super::tables::{array_key_field, is_dynamic_map};
use serde_json::{Map, Value};

fn format_path(display_path: &[&str], key: &str) -> String {
    if display_path.is_empty() {
        key.to_owned()
    } else {
        format!("{}.{key}", display_path.join("."))
    }
}

/// Applies `patch` onto `base` in place, using ID-keyed merging for known
/// table arrays, sparse cell patching for known 2D grids, and strict
/// schema-validated merging for objects.
///
/// When `rel_file` has a `psiv-data` Rust schema and `base` is a complete file
/// of that schema, unknown/ignored fields are rejected against the crate's own
/// `Deserialize` implementation (allowing all modeled optional fields while
/// rejecting typos even in newly inserted optional subtrees or replaced unkeyed
/// arrays). For files outside `psiv-data` or partial JSON fixtures, keys absent
/// from `base` (outside dynamic maps) are rejected directly.
pub fn apply_json_patch(base: &mut Value, patch: &Value, rel_file: &str) -> Result<(), String> {
    let baseline = schema_check::prepare_baseline(rel_file, base);
    let strict_base_keys = baseline.is_none();
    apply_at_slice(base, patch, rel_file, &[], &[], strict_base_keys)?;
    if let Some(baseline) = &baseline {
        schema_check::verify_patched(rel_file, baseline, base)?;
    }
    Ok(())
}

fn apply_at_slice(
    base: &mut Value,
    patch: &Value,
    rel_file: &str,
    schema_path: &[&str],
    display_path: &[&str],
    strict_base_keys: bool,
) -> Result<(), String> {
    if is_dialogue_entry(rel_file, schema_path)
        && let (Value::Object(base_obj), Value::Object(patch_obj)) = (base as &mut Value, patch)
    {
        if patch_obj.contains_key("keep_portrait") && !patch_obj.contains_key("lines") {
            return Err(format!(
                "{}: `keep_portrait` is only valid alongside `lines`",
                format_path(display_path, "keep_portrait")
            ));
        }
        if patch_obj.contains_key("replace") || patch_obj.contains_key("lines") {
            apply_dialogue_shorthand(base_obj, patch_obj)?;
            for (k, v) in patch_obj {
                if matches!(k.as_str(), "replace" | "lines" | "keep_portrait") {
                    continue;
                }
                apply_object_entry(
                    base_obj,
                    k,
                    v,
                    rel_file,
                    schema_path,
                    display_path,
                    strict_base_keys,
                )?;
            }
            return Ok(());
        }
    }

    if let (Value::Array(base_arr), Some(key_field)) =
        (base as &mut Value, array_key_field(rel_file, schema_path))
    {
        return apply_keyed_array(
            base_arr,
            patch,
            rel_file,
            schema_path,
            display_path,
            key_field,
            strict_base_keys,
        );
    }

    if let (Value::Array(base_rows), Some(grid_spec)) =
        (base as &mut Value, cell_grid_spec(rel_file, schema_path))
    {
        return apply_cell_grid_patch(base_rows, patch, display_path, grid_spec.kind);
    }

    match (base, patch) {
        (Value::Object(base_map), Value::Object(patch_map)) => {
            for (k, v) in patch_map {
                apply_object_entry(
                    base_map,
                    k,
                    v,
                    rel_file,
                    schema_path,
                    display_path,
                    strict_base_keys,
                )?;
            }
            Ok(())
        }
        (Value::Array(base_arr), Value::Array(patch_arr)) if strict_base_keys => {
            validate_replacement_array(base_arr, patch_arr, rel_file, schema_path, display_path)?;
            *base_arr = patch_arr.clone();
            Ok(())
        }
        (target, replacement) => {
            *target = replacement.clone();
            Ok(())
        }
    }
}

/// Recursive key-union schema inferred from existing base JSON values for files
/// outside `psiv-data` (such as Godot-only `battle/art/*.json` files).
enum BaseShape {
    Object(std::collections::BTreeMap<String, BaseShape>),
    Array(Box<BaseShape>),
    Any,
}

impl BaseShape {
    fn merge(&mut self, value: &Value) {
        match value {
            Value::Object(map) => {
                if matches!(self, Self::Any) {
                    *self = Self::Object(std::collections::BTreeMap::new());
                }
                if let Self::Object(fields) = self {
                    for (k, v) in map {
                        fields.entry(k.clone()).or_insert(Self::Any).merge(v);
                    }
                }
            }
            Value::Array(items) => {
                if matches!(self, Self::Any) {
                    *self = Self::Array(Box::new(Self::Any));
                }
                if let Self::Array(elem) = self {
                    for item in items {
                        elem.merge(item);
                    }
                }
            }
            _ => {}
        }
    }
}

fn validate_replacement_array(
    base_arr: &[Value],
    patch_arr: &[Value],
    rel_file: &str,
    schema_path: &[&str],
    display_path: &[&str],
) -> Result<(), String> {
    let mut elem_shape = BaseShape::Any;
    for item in base_arr {
        elem_shape.merge(item);
    }
    let mut elem_schema: Vec<&str> = schema_path.to_vec();
    elem_schema.push("*");
    let prefix = display_path.join(".");
    for (idx, item) in patch_arr.iter().enumerate() {
        let item_path = if prefix.is_empty() {
            format!("[{idx}]")
        } else {
            format!("{prefix}[{idx}]")
        };
        validate_value_against_shape(&elem_shape, item, rel_file, &elem_schema, &item_path)?;
    }
    Ok(())
}

fn validate_value_against_shape(
    shape: &BaseShape,
    value: &Value,
    rel_file: &str,
    schema_path: &[&str],
    path: &str,
) -> Result<(), String> {
    match (shape, value) {
        (BaseShape::Object(fields), Value::Object(map)) if !fields.is_empty() => {
            if is_dynamic_map(rel_file, schema_path) {
                return Ok(());
            }
            for (k, v) in map {
                let next_path = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                let Some(child_shape) = fields.get(k) else {
                    return Err(format!(
                        "unknown field `{next_path}` (not present in base `{rel_file}`)"
                    ));
                };
                let mut next_schema: Vec<&str> = schema_path.to_vec();
                next_schema.push(k);
                validate_value_against_shape(child_shape, v, rel_file, &next_schema, &next_path)?;
            }
            Ok(())
        }
        (BaseShape::Array(elem_shape), Value::Array(items)) => {
            let mut next_schema: Vec<&str> = schema_path.to_vec();
            next_schema.push("*");
            for (idx, item) in items.iter().enumerate() {
                let next_path = format!("{path}[{idx}]");
                validate_value_against_shape(elem_shape, item, rel_file, &next_schema, &next_path)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn apply_object_entry(
    base_map: &mut Map<String, Value>,
    k: &str,
    v: &Value,
    rel_file: &str,
    schema_path: &[&str],
    display_path: &[&str],
    strict_base_keys: bool,
) -> Result<(), String> {
    let dynamic = is_dynamic_map(rel_file, schema_path);
    if strict_base_keys && !base_map.contains_key(k) && !dynamic {
        return Err(format!(
            "unknown field `{}` (not present in base `{rel_file}`)",
            format_path(display_path, k)
        ));
    }
    if v.is_null() {
        if dynamic {
            base_map.remove(k);
        } else {
            match base_map.get(k) {
                Some(Value::Null) => {}
                Some(_) => {
                    // Removing an existing non-null key matches `diff_json` encoding omitted
                    // keys as `null` (so `#[serde(default)]` fields revert to default and
                    // `Option<T>` fields become `None`).
                    base_map.remove(k);
                }
                None => {
                    // Keep absent keys as `null` so `schema_check::verify_patched` still
                    // sees and rejects unknown null-valued keys like `{"HP": null}`.
                    base_map.insert(k.to_owned(), Value::Null);
                }
            }
        }
        return Ok(());
    }
    if let Some(existing) = base_map.get_mut(k) {
        let mut next_schema: Vec<&str> = schema_path.to_vec();
        next_schema.push(k);
        let mut next_display: Vec<&str> = display_path.to_vec();
        next_display.push(k);
        apply_at_slice(
            existing,
            v,
            rel_file,
            &next_schema,
            &next_display,
            strict_base_keys,
        )?;
    } else {
        base_map.insert(k.to_owned(), v.clone());
    }
    Ok(())
}

fn value_key_string(record: &Value, key_field: &str) -> Option<String> {
    let val = record.as_object()?.get(key_field)?;
    match val {
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

fn apply_keyed_array(
    base_arr: &mut [Value],
    patch: &Value,
    rel_file: &str,
    schema_path: &[&str],
    display_path: &[&str],
    key_field: &str,
    strict_base_keys: bool,
) -> Result<(), String> {
    let mut child_schema: Vec<&str> = schema_path.to_vec();
    child_schema.push("*");

    match patch {
        Value::Object(by_id) => {
            for (id_key, entry_patch) in by_id {
                let Some(record) = base_arr.iter_mut().find(|item| {
                    value_key_string(item, key_field).as_deref() == Some(id_key.as_str())
                }) else {
                    return Err(format!(
                        "{}: no record with {key_field}={id_key}",
                        display_path.join(".")
                    ));
                };
                let mut child_display: Vec<&str> = display_path.to_vec();
                child_display.push(id_key.as_str());
                apply_at_slice(
                    record,
                    entry_patch,
                    rel_file,
                    &child_schema,
                    &child_display,
                    strict_base_keys,
                )?;
            }
            Ok(())
        }
        Value::Array(items) => {
            for (idx, entry_patch) in items.iter().enumerate() {
                let Some(id_key) = value_key_string(entry_patch, key_field) else {
                    return Err(format!(
                        "{}[{idx}]: patch entry must carry `{key_field}`",
                        display_path.join(".")
                    ));
                };
                let Some(record) = base_arr.iter_mut().find(|item| {
                    value_key_string(item, key_field).as_deref() == Some(id_key.as_str())
                }) else {
                    return Err(format!(
                        "{}: no record with {key_field}={id_key}",
                        display_path.join(".")
                    ));
                };
                let mut child_display: Vec<&str> = display_path.to_vec();
                child_display.push(id_key.as_str());
                apply_at_slice(
                    record,
                    entry_patch,
                    rel_file,
                    &child_schema,
                    &child_display,
                    strict_base_keys,
                )?;
            }
            Ok(())
        }
        _ => Err(format!(
            "{}: expected an ID-keyed object or array of records carrying `{key_field}`",
            display_path.join(".")
        )),
    }
}

/// Computes a minimal sparse JSON patch such that applying it to `base`
/// reproduces `edited`. Returns `Ok(None)` when `base == edited`.
///
/// Fails closed (strict shareability, [`DiffOptions::default`]) if exporting
/// the diff would copy a whole non-empty unkeyed base JSON array. Returns `Err`
/// if a known ID-keyed table adds, removes, duplicates, reorders, or omits
/// record IDs.
pub fn diff_json(base: &Value, edited: &Value, rel_file: &str) -> Result<Option<Value>, String> {
    diff_json_with_options(base, edited, rel_file, DiffOptions::default())
}

/// Like [`diff_json`], with explicit [`DiffOptions`] controlling whether
/// non-delta whole base array copies are permitted.
pub fn diff_json_with_options(
    base: &Value,
    edited: &Value,
    rel_file: &str,
    options: DiffOptions,
) -> Result<Option<Value>, String> {
    let mut copied = Vec::new();
    diff_json_tracked(base, edited, rel_file, options, &mut copied)
}

pub(crate) fn diff_json_tracked(
    base: &Value,
    edited: &Value,
    rel_file: &str,
    options: DiffOptions,
    copied_base_arrays: &mut Vec<String>,
) -> Result<Option<Value>, String> {
    diff_at_slice(
        base,
        edited,
        rel_file,
        &[],
        &[],
        options.allow_derived,
        copied_base_arrays,
    )
}

fn diff_at_slice(
    base: &Value,
    edited: &Value,
    rel_file: &str,
    schema_path: &[&str],
    display_path: &[&str],
    allow_derived: bool,
    copied_base_arrays: &mut Vec<String>,
) -> Result<Option<Value>, String> {
    if base == edited {
        return Ok(None);
    }
    if is_dialogue_entry(rel_file, schema_path)
        && let (Value::Object(base_obj), Value::Object(edited_obj)) = (base, edited)
        && let Some(shorthand) = synthesize_dialogue_shorthand(base_obj, edited_obj)
    {
        let mut out = shorthand;
        for (k, ev) in edited_obj {
            if matches!(k.as_str(), "segments" | "pages" | "text") {
                continue;
            }
            match base_obj.get(k) {
                Some(bv) => {
                    let mut next_schema: Vec<&str> = schema_path.to_vec();
                    next_schema.push(k.as_str());
                    let mut next_display: Vec<&str> = display_path.to_vec();
                    next_display.push(k.as_str());
                    if let Some(d) = diff_at_slice(
                        bv,
                        ev,
                        rel_file,
                        &next_schema,
                        &next_display,
                        allow_derived,
                        copied_base_arrays,
                    )? {
                        out.insert(k.clone(), d);
                    }
                }
                None => {
                    out.insert(k.clone(), ev.clone());
                }
            }
        }
        for k in base_obj.keys() {
            if !matches!(k.as_str(), "segments" | "pages" | "text") && !edited_obj.contains_key(k) {
                out.insert(k.clone(), Value::Null);
            }
        }
        return Ok(Some(Value::Object(out)));
    }

    if let (Value::Array(base_arr), Value::Array(edited_arr), Some(key_field)) =
        (base, edited, array_key_field(rel_file, schema_path))
    {
        return diff_keyed_array(
            base_arr,
            edited_arr,
            rel_file,
            schema_path,
            display_path,
            key_field,
            allow_derived,
            copied_base_arrays,
        );
    }

    if let (Value::Array(base_rows), Value::Array(edited_rows), Some(grid_spec)) =
        (base, edited, cell_grid_spec(rel_file, schema_path))
    {
        return diff_cell_grid(
            base_rows,
            edited_rows,
            display_path,
            grid_spec.kind,
            allow_derived,
            copied_base_arrays,
        );
    }

    match (base, edited) {
        (Value::Object(base_map), Value::Object(edited_map)) => {
            let mut out = Map::new();
            for (k, ev) in edited_map {
                match base_map.get(k) {
                    Some(bv) => {
                        let mut next_schema: Vec<&str> = schema_path.to_vec();
                        next_schema.push(k.as_str());
                        let mut next_display: Vec<&str> = display_path.to_vec();
                        next_display.push(k.as_str());
                        if let Some(d) = diff_at_slice(
                            bv,
                            ev,
                            rel_file,
                            &next_schema,
                            &next_display,
                            allow_derived,
                            copied_base_arrays,
                        )? {
                            out.insert(k.clone(), d);
                        }
                    }
                    None => {
                        out.insert(k.clone(), ev.clone());
                    }
                }
            }
            for k in base_map.keys() {
                if !edited_map.contains_key(k) {
                    out.insert(k.clone(), Value::Null);
                }
            }
            if out.is_empty() {
                Ok(None)
            } else {
                Ok(Some(Value::Object(out)))
            }
        }
        (Value::Array(base_arr), Value::Array(_)) if !base_arr.is_empty() => {
            let label = if display_path.is_empty() {
                rel_file.to_owned()
            } else {
                display_path.join(".")
            };
            if !allow_derived {
                return Err(format!(
                    "refusing to export whole unkeyed base array `{label}`: copying a base pack array embeds cartridge-derived data into a shareable mod (re-run with `--allow-derived` for private local edits)"
                ));
            }
            copied_base_arrays.push(label);
            Ok(Some(edited.clone()))
        }
        _ => Ok(Some(edited.clone())),
    }
}

#[allow(clippy::too_many_arguments)]
fn diff_keyed_array(
    base_arr: &[Value],
    edited_arr: &[Value],
    rel_file: &str,
    schema_path: &[&str],
    display_path: &[&str],
    key_field: &str,
    allow_derived: bool,
    copied_base_arrays: &mut Vec<String>,
) -> Result<Option<Value>, String> {
    let label = if display_path.is_empty() {
        rel_file.to_owned()
    } else {
        display_path.join(".")
    };
    let mut base_by_id = Map::new();
    let mut base_order = Vec::with_capacity(base_arr.len());
    for (idx, item) in base_arr.iter().enumerate() {
        let Some(key) = value_key_string(item, key_field) else {
            return Err(format!(
                "{label}[{idx}]: base entry is missing `{key_field}`"
            ));
        };
        base_order.push(key.clone());
        base_by_id.insert(key, item.clone());
    }
    let mut child_schema: Vec<&str> = schema_path.to_vec();
    child_schema.push("*");
    let mut seen = std::collections::BTreeSet::new();
    let mut edited_order = Vec::with_capacity(edited_arr.len());
    let mut out = Map::new();
    for (idx, edited_item) in edited_arr.iter().enumerate() {
        let Some(key) = value_key_string(edited_item, key_field) else {
            return Err(format!(
                "{label}[{idx}]: edited entry is missing `{key_field}`"
            ));
        };
        if !seen.insert(key.clone()) {
            return Err(format!(
                "{label}: duplicate record {key_field}={key} is not supported in sparse patches"
            ));
        }
        edited_order.push(key.clone());
        let Some(base_item) = base_by_id.get(&key) else {
            return Err(format!(
                "{label}: adding new record {key_field}={key} is not supported in sparse ID-keyed patches"
            ));
        };
        let mut child_display: Vec<&str> = display_path.to_vec();
        child_display.push(key.as_str());
        if let Some(d) = diff_at_slice(
            base_item,
            edited_item,
            rel_file,
            &child_schema,
            &child_display,
            allow_derived,
            copied_base_arrays,
        )? {
            out.insert(key, d);
        }
    }
    for key in base_by_id.keys() {
        if !seen.contains(key) {
            return Err(format!(
                "{label}: removing record {key_field}={key} is not supported in sparse ID-keyed patches"
            ));
        }
    }
    if base_order != edited_order {
        return Err(format!(
            "{label}: reordering records is not supported in sparse ID-keyed patches"
        ));
    }
    if out.is_empty() {
        Ok(None)
    } else {
        Ok(Some(Value::Object(out)))
    }
}

/// Collects display paths where `patch` replaces a whole non-empty base JSON
/// array (unkeyed array or raw full 2D grid array) rather than using a sparse
/// ID-keyed or cell-level patch. Used by [`super::audit_mod_shareability`].
pub(crate) fn collect_whole_base_array_patches(
    base: &Value,
    patch: &Value,
    rel_file: &str,
) -> Vec<String> {
    let mut out = Vec::new();
    walk_whole_base_arrays(base, patch, rel_file, &[], &[], &mut out);
    out
}

fn walk_whole_base_arrays(
    base: &Value,
    patch: &Value,
    rel_file: &str,
    schema_path: &[&str],
    display_path: &[&str],
    out: &mut Vec<String>,
) {
    if let (Value::Array(base_arr), Some(key_field)) =
        (base, array_key_field(rel_file, schema_path))
    {
        let mut child_schema: Vec<&str> = schema_path.to_vec();
        child_schema.push("*");
        match patch {
            Value::Object(by_id) => {
                for (id_key, entry_patch) in by_id {
                    if let Some(record) = base_arr.iter().find(|item| {
                        value_key_string(item, key_field).as_deref() == Some(id_key.as_str())
                    }) {
                        let mut child_display: Vec<&str> = display_path.to_vec();
                        child_display.push(id_key.as_str());
                        walk_whole_base_arrays(
                            record,
                            entry_patch,
                            rel_file,
                            &child_schema,
                            &child_display,
                            out,
                        );
                    }
                }
            }
            Value::Array(items) => {
                for entry_patch in items {
                    if let Some(id_key) = value_key_string(entry_patch, key_field)
                        && let Some(record) = base_arr.iter().find(|item| {
                            value_key_string(item, key_field).as_deref() == Some(id_key.as_str())
                        })
                    {
                        let mut child_display: Vec<&str> = display_path.to_vec();
                        child_display.push(id_key.as_str());
                        walk_whole_base_arrays(
                            record,
                            entry_patch,
                            rel_file,
                            &child_schema,
                            &child_display,
                            out,
                        );
                    }
                }
            }
            _ => {}
        }
        return;
    }

    if let (Value::Array(base_rows), Value::Array(_), Some(_)) =
        (base, patch, cell_grid_spec(rel_file, schema_path))
    {
        if !base_rows.is_empty() {
            out.push(display_path.join("."));
        }
        return;
    }

    match (base, patch) {
        (Value::Object(base_map), Value::Object(patch_map)) => {
            for (k, pv) in patch_map {
                if let Some(bv) = base_map.get(k) {
                    let mut next_schema: Vec<&str> = schema_path.to_vec();
                    next_schema.push(k.as_str());
                    let mut next_display: Vec<&str> = display_path.to_vec();
                    next_display.push(k.as_str());
                    walk_whole_base_arrays(bv, pv, rel_file, &next_schema, &next_display, out);
                }
            }
        }
        (Value::Array(base_arr), Value::Array(_)) if !base_arr.is_empty() => {
            let label = if display_path.is_empty() {
                rel_file.to_owned()
            } else {
                display_path.join(".")
            };
            out.push(label);
        }
        _ => {}
    }
}
