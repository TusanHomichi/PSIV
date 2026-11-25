//! Sparse ID-keyed JSON patching, cell-level grid patching, explicit `"$add"`
//! record creation, and reverse diffing for runtime pack tables.
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
use super::keyed::{apply_keyed_array, diff_keyed_array, walk_keyed_array_whole_base_arrays};
use super::schema_check;
use super::tables::{array_key_field, is_dynamic_map, keyed_array_spec};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};

fn format_path(display_path: &[&str], key: &str) -> String {
    if display_path.is_empty() {
        key.to_owned()
    } else {
        format!("{}.{key}", display_path.join("."))
    }
}

/// Ownership map recording which mod added each `(rel_file, table_path, record_id)`.
pub(crate) type AddedRecordOwners = BTreeMap<(String, String, String), String>;

/// Set of `(rel_file, table_path, record_id)` records added or edited by active mods.
pub(crate) type TouchedRecords = BTreeSet<(String, String, String)>;

/// Per-patch state threaded through [`apply_at_slice`] and [`apply_keyed_array`].
pub(crate) struct ApplyContext<'a> {
    pub(crate) strict_base_keys: bool,
    pub(crate) mod_id: Option<&'a str>,
    pub(crate) added_owners: Option<&'a mut AddedRecordOwners>,
    pub(crate) touched: Option<&'a mut TouchedRecords>,
    pub(crate) added_tables: BTreeSet<String>,
}

impl<'a> ApplyContext<'a> {
    fn standalone(strict_base_keys: bool) -> Self {
        Self {
            strict_base_keys,
            mod_id: None,
            added_owners: None,
            touched: None,
            added_tables: BTreeSet::new(),
        }
    }

    pub(crate) fn added_record_owner(
        &self,
        rel_file: &str,
        table_label: &str,
        id_key: &str,
    ) -> Option<&str> {
        self.added_owners
            .as_deref()?
            .get(&(
                rel_file.to_owned(),
                table_label.to_owned(),
                id_key.to_owned(),
            ))
            .map(String::as_str)
    }

    pub(crate) fn record_added(&mut self, rel_file: &str, table_label: &str, id_key: &str) {
        self.added_tables.insert(table_label.to_owned());
        if let (Some(mod_id), Some(owners)) = (self.mod_id, self.added_owners.as_deref_mut()) {
            owners.insert(
                (
                    rel_file.to_owned(),
                    table_label.to_owned(),
                    id_key.to_owned(),
                ),
                mod_id.to_owned(),
            );
        }
    }

    pub(crate) fn record_touched(&mut self, rel_file: &str, table_label: &str, id_key: &str) {
        if let Some(touched) = self.touched.as_deref_mut() {
            touched.insert((
                rel_file.to_owned(),
                table_label.to_owned(),
                id_key.to_owned(),
            ));
        }
    }
}

/// Per-diff state threaded through [`diff_at_slice`] and [`diff_keyed_array`].
pub(crate) struct DiffContext<'a> {
    pub(crate) allow_derived: bool,
    pub(crate) copied_base_arrays: &'a mut Vec<String>,
    pub(crate) added_tables: BTreeSet<String>,
}

/// Applies `patch` onto `base` in place, using ID-keyed merging for known
/// table arrays, explicit `"$add"` appending for additive-capable tables,
/// sparse cell patching for known 2D grids, and strict schema-validated
/// merging for objects.
pub fn apply_json_patch(base: &mut Value, patch: &Value, rel_file: &str) -> Result<(), String> {
    let baseline = schema_check::prepare_baseline(rel_file, base);
    let mut ctx = ApplyContext::standalone(baseline.is_none());
    apply_at_slice(base, patch, rel_file, &[], &[], &mut ctx)?;
    if let Some(baseline) = &baseline {
        schema_check::verify_patched(rel_file, baseline, base)?;
    }
    Ok(())
}

/// Like [`apply_json_patch`], tracking multi-mod added-record ownership and
/// touched records for [`super::PackOverlay`].
pub(crate) fn apply_json_patch_tracked(
    base: &mut Value,
    patch: &Value,
    rel_file: &str,
    mod_id: &str,
    added_owners: &mut AddedRecordOwners,
    touched: &mut TouchedRecords,
) -> Result<(), String> {
    let baseline = schema_check::prepare_baseline(rel_file, base);
    let mut ctx = ApplyContext {
        strict_base_keys: baseline.is_none(),
        mod_id: Some(mod_id),
        added_owners: Some(added_owners),
        touched: Some(touched),
        added_tables: BTreeSet::new(),
    };
    apply_at_slice(base, patch, rel_file, &[], &[], &mut ctx)?;
    if let Some(baseline) = &baseline {
        schema_check::verify_patched(rel_file, baseline, base)?;
    }
    Ok(())
}

pub(crate) fn apply_at_slice(
    base: &mut Value,
    patch: &Value,
    rel_file: &str,
    schema_path: &[&str],
    display_path: &[&str],
    ctx: &mut ApplyContext<'_>,
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
                apply_object_entry(base_obj, k, v, rel_file, schema_path, display_path, ctx)?;
            }
            return Ok(());
        }
    }

    if let (Value::Array(base_arr), Some(spec)) =
        (base as &mut Value, keyed_array_spec(rel_file, schema_path))
    {
        let _ = apply_keyed_array(
            base_arr,
            patch,
            rel_file,
            schema_path,
            display_path,
            spec,
            ctx,
        )?;
        return Ok(());
    }

    if let (Value::Array(base_rows), Some(grid_spec)) =
        (base as &mut Value, cell_grid_spec(rel_file, schema_path))
    {
        return apply_cell_grid_patch(base_rows, patch, display_path, grid_spec.kind);
    }

    match (base, patch) {
        (Value::Object(base_map), Value::Object(patch_map)) => {
            for (k, v) in patch_map {
                apply_object_entry(base_map, k, v, rel_file, schema_path, display_path, ctx)?;
            }
            reconcile_table_counts_after_add(
                base_map,
                patch_map,
                rel_file,
                schema_path,
                display_path,
                &ctx.added_tables,
            );
            Ok(())
        }
        (Value::Array(base_arr), Value::Array(patch_arr)) if ctx.strict_base_keys => {
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

/// Keeps table-level count fields (`formation_count`, tree `entry_count`, root
/// `entry_count`) consistent when `"$add"` appended records, unless the patch
/// explicitly overwrote the count field itself.
fn reconcile_table_counts_after_add(
    base_map: &mut Map<String, Value>,
    patch_map: &Map<String, Value>,
    rel_file: &str,
    schema_path: &[&str],
    display_path: &[&str],
    added_tables: &BTreeSet<String>,
) {
    match (rel_file, schema_path) {
        ("battle/formations.json", [])
            if added_tables.contains("formations")
                && !patch_map.contains_key("formation_count")
                && base_map.contains_key("formation_count") =>
        {
            if let Some(len) = base_map
                .get("formations")
                .and_then(Value::as_array)
                .map(Vec::len)
            {
                base_map.insert("formation_count".to_owned(), json!(len));
            }
        }
        ("dialogue/trees.json", ["trees", "*"])
            if !patch_map.contains_key("entry_count") && base_map.contains_key("entry_count") =>
        {
            let entries_label = format_path(display_path, "entries");
            if added_tables.contains(&entries_label)
                && let Some(len) = base_map
                    .get("entries")
                    .and_then(Value::as_array)
                    .map(Vec::len)
            {
                base_map.insert("entry_count".to_owned(), json!(len));
            }
        }
        ("dialogue/trees.json", [])
            if !patch_map.contains_key("entry_count")
                && base_map.contains_key("entry_count")
                && added_tables
                    .iter()
                    .any(|p| p.starts_with("trees.") && p.ends_with(".entries")) =>
        {
            if let Some(trees) = base_map.get("trees").and_then(Value::as_array) {
                let total: usize = trees
                    .iter()
                    .filter_map(|tree| tree.get("entries")?.as_array())
                    .map(Vec::len)
                    .sum();
                base_map.insert("entry_count".to_owned(), json!(total));
            }
        }
        _ => {}
    }
}

/// Recursive key-union schema inferred from existing base JSON values for files
/// outside `psiv-data` (such as Godot-only `battle/art/*.json` files).
enum BaseShape {
    Object(BTreeMap<String, BaseShape>),
    Array(Box<BaseShape>),
    Any,
}

impl BaseShape {
    fn merge(&mut self, value: &Value) {
        match value {
            Value::Object(map) => {
                if matches!(self, Self::Any) {
                    *self = Self::Object(BTreeMap::new());
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
    ctx: &mut ApplyContext<'_>,
) -> Result<(), String> {
    let dynamic = is_dynamic_map(rel_file, schema_path);
    if ctx.strict_base_keys && !base_map.contains_key(k) && !dynamic {
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
        apply_at_slice(existing, v, rel_file, &next_schema, &next_display, ctx)?;
    } else {
        base_map.insert(k.to_owned(), v.clone());
    }
    Ok(())
}

/// Computes a minimal sparse JSON patch such that applying it to `base`
/// reproduces `edited`. Returns `Ok(None)` when `base == edited`.
///
/// Fails closed (strict shareability, [`DiffOptions::default`]) if exporting
/// the diff would copy a whole non-empty unkeyed base JSON array. Appended
/// records in additive-capable ID-keyed tables export under `"$add"`;
/// deletions, reorders, duplicates, and additions in non-additive tables fail
/// closed.
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
    let mut ctx = DiffContext {
        allow_derived: options.allow_derived,
        copied_base_arrays,
        added_tables: BTreeSet::new(),
    };
    diff_at_slice(base, edited, rel_file, &[], &[], &mut ctx)
}

pub(crate) fn diff_at_slice(
    base: &Value,
    edited: &Value,
    rel_file: &str,
    schema_path: &[&str],
    display_path: &[&str],
    ctx: &mut DiffContext<'_>,
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
                    if let Some(d) =
                        diff_at_slice(bv, ev, rel_file, &next_schema, &next_display, ctx)?
                    {
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

    if let (Value::Array(base_arr), Value::Array(edited_arr), Some(spec)) =
        (base, edited, keyed_array_spec(rel_file, schema_path))
    {
        let (diff, added_any) = diff_keyed_array(
            base_arr,
            edited_arr,
            rel_file,
            schema_path,
            display_path,
            spec,
            ctx,
        )?;
        if added_any {
            ctx.added_tables.insert(display_path.join("."));
        }
        return Ok(diff);
    }

    if let (Value::Array(base_rows), Value::Array(edited_rows), Some(grid_spec)) =
        (base, edited, cell_grid_spec(rel_file, schema_path))
    {
        return diff_cell_grid(
            base_rows,
            edited_rows,
            display_path,
            grid_spec.kind,
            ctx.allow_derived,
            ctx.copied_base_arrays,
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
                        if let Some(d) =
                            diff_at_slice(bv, ev, rel_file, &next_schema, &next_display, ctx)?
                        {
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
            strip_derived_counts_from_diff(
                &mut out,
                edited_map,
                rel_file,
                schema_path,
                display_path,
                &ctx.added_tables,
            );
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
            if !ctx.allow_derived {
                return Err(format!(
                    "refusing to export whole unkeyed base array `{label}`: copying a base pack array embeds cartridge-derived data into a shareable mod (re-run with `--allow-derived` for private local edits)"
                ));
            }
            ctx.copied_base_arrays.push(label);
            Ok(Some(edited.clone()))
        }
        _ => Ok(Some(edited.clone())),
    }
}

/// Drops derived table-level count diffs when they match the auto-reconciled
/// length produced by `"$add"`, keeping exported mods composable across
/// multiple mods while still round-tripping identically.
fn strip_derived_counts_from_diff(
    out: &mut Map<String, Value>,
    edited_map: &Map<String, Value>,
    rel_file: &str,
    schema_path: &[&str],
    display_path: &[&str],
    added_tables: &BTreeSet<String>,
) {
    match (rel_file, schema_path) {
        ("battle/formations.json", []) if added_tables.contains("formations") => {
            if let (Some(count), Some(len)) = (
                edited_map.get("formation_count").and_then(Value::as_u64),
                edited_map
                    .get("formations")
                    .and_then(Value::as_array)
                    .map(|arr| arr.len() as u64),
            ) && count == len
            {
                out.remove("formation_count");
            }
        }
        ("dialogue/trees.json", ["trees", "*"]) => {
            let entries_label = format_path(display_path, "entries");
            if added_tables.contains(&entries_label)
                && let (Some(count), Some(len)) = (
                    edited_map.get("entry_count").and_then(Value::as_u64),
                    edited_map
                        .get("entries")
                        .and_then(Value::as_array)
                        .map(|arr| arr.len() as u64),
                )
                && count == len
            {
                out.remove("entry_count");
            }
        }
        ("dialogue/trees.json", [])
            if added_tables
                .iter()
                .any(|p| p.starts_with("trees.") && p.ends_with(".entries")) =>
        {
            if let (Some(count), Some(trees)) = (
                edited_map.get("entry_count").and_then(Value::as_u64),
                edited_map.get("trees").and_then(Value::as_array),
            ) {
                let total: u64 = trees
                    .iter()
                    .filter_map(|t| t.get("entries")?.as_array())
                    .map(|arr| arr.len() as u64)
                    .sum();
                if count == total {
                    out.remove("entry_count");
                }
            }
        }
        _ => {}
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

pub(crate) fn walk_whole_base_arrays(
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
        walk_keyed_array_whole_base_arrays(
            base_arr,
            patch,
            rel_file,
            schema_path,
            display_path,
            key_field,
            out,
        );
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
