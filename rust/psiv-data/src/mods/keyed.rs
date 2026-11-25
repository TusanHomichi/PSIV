//! ID-keyed array patching, explicit `"$add"` record creation, reverse diffing,
//! and shareability traversal for [`super::patch`].

use super::dialogue_patch::{
    expand_new_dialogue_entry, is_tree_dialogue_entry, synthesize_new_dialogue_entry,
};
use super::patch::{
    ApplyContext, DiffContext, apply_at_slice, diff_at_slice, walk_whole_base_arrays,
};
use super::schema_check;
use super::tables::KeyedArraySpec;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

/// Reserved object key inside an additive-capable ID-keyed table patch that
/// explicitly appends complete new records without weakening unknown-ID errors
/// on ordinary edit keys.
pub(crate) const ADD_RECORDS_KEY: &str = "$add";

/// Extracts the stringified key of a keyed-array record, or `None` when missing
/// or of an unsupported JSON type.
pub(crate) fn value_key_string(record: &Value, key_field: &str) -> Option<String> {
    let val = record.as_object()?.get(key_field)?;
    match val {
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

/// Applies an ID-keyed patch (object form with optional `"$add"` list, or
/// edit-only array form) onto `base_arr`.
///
/// Returns `Ok(true)` when one or more records were appended via `"$add"`.
pub(crate) fn apply_keyed_array(
    base_arr: &mut Vec<Value>,
    patch: &Value,
    rel_file: &str,
    schema_path: &[&str],
    display_path: &[&str],
    spec: &KeyedArraySpec,
    ctx: &mut ApplyContext<'_>,
) -> Result<bool, String> {
    let key_field = spec.key_field;
    let table_label = display_path.join(".");
    let mut child_schema: Vec<&str> = schema_path.to_vec();
    child_schema.push("*");

    match patch {
        Value::Object(by_id) => {
            let add_items = by_id.get(ADD_RECORDS_KEY);
            if add_items.is_some() && !spec.allow_add {
                return Err(format!(
                    "{table_label}: adding new records via `{ADD_RECORDS_KEY}` is not supported for this table"
                ));
            }

            // Collect keys targeted by ordinary edits first so we reject any
            // patch that tries to both edit and add the same ID in one pass.
            let mut edited_keys = BTreeSet::new();
            for (id_key, entry_patch) in by_id {
                if id_key == ADD_RECORDS_KEY {
                    continue;
                }
                let Some(record) = base_arr.iter_mut().find(|item| {
                    value_key_string(item, key_field).as_deref() == Some(id_key.as_str())
                }) else {
                    return Err(format!(
                        "{table_label}: no record with {key_field}={id_key}"
                    ));
                };
                edited_keys.insert(id_key.clone());
                let mut child_display: Vec<&str> = display_path.to_vec();
                child_display.push(id_key.as_str());
                apply_at_slice(
                    record,
                    entry_patch,
                    rel_file,
                    &child_schema,
                    &child_display,
                    ctx,
                )?;
                ctx.record_touched(rel_file, &table_label, id_key);
            }

            let Some(add_value) = add_items else {
                return Ok(false);
            };
            let new_items = add_value.as_array().ok_or_else(|| {
                format!("{table_label}.{ADD_RECORDS_KEY}: expected an array of complete records")
            })?;
            if new_items.is_empty() {
                return Err(format!(
                    "{table_label}.{ADD_RECORDS_KEY}: must contain at least one record"
                ));
            }

            let existing_keys: BTreeSet<String> = base_arr
                .iter()
                .filter_map(|item| value_key_string(item, key_field))
                .collect();
            let mut seen_new = BTreeSet::new();
            let mut prepared = Vec::with_capacity(new_items.len());
            let mut prepared_keys = Vec::with_capacity(new_items.len());

            for (idx, raw_item) in new_items.iter().enumerate() {
                if !raw_item.is_object() {
                    return Err(format!(
                        "{table_label}.{ADD_RECORDS_KEY}[{idx}]: added record must be a JSON object"
                    ));
                }
                let Some(id_key) = value_key_string(raw_item, key_field) else {
                    return Err(format!(
                        "{table_label}.{ADD_RECORDS_KEY}[{idx}]: added record must carry `{key_field}`"
                    ));
                };
                if !seen_new.insert(id_key.clone()) {
                    return Err(format!(
                        "{table_label}: duplicate new record {key_field}={id_key} in `{ADD_RECORDS_KEY}`"
                    ));
                }
                if edited_keys.contains(&id_key) {
                    return Err(format!(
                        "{table_label}: cannot both edit and add record {key_field}={id_key} in the same patch"
                    ));
                }
                if existing_keys.contains(&id_key) {
                    if let Some(owner) = ctx.added_record_owner(rel_file, &table_label, &id_key) {
                        return Err(format!(
                            "{table_label}: conflicting new record {key_field}={id_key} (already added by mod `{owner}`)"
                        ));
                    }
                    return Err(format!(
                        "{table_label}: cannot add record {key_field}={id_key} (already exists in base pack)"
                    ));
                }

                let materialized = if is_tree_dialogue_entry(rel_file, &child_schema) {
                    expand_new_dialogue_entry(raw_item, &table_label, &id_key)?
                } else {
                    raw_item.clone()
                };
                schema_check::validate_added_record(
                    rel_file,
                    schema_path,
                    &table_label,
                    key_field,
                    &id_key,
                    &materialized,
                )?;
                prepared.push(materialized);
                prepared_keys.push(id_key);
            }

            base_arr.extend(prepared);
            for id_key in prepared_keys {
                ctx.record_added(rel_file, &table_label, &id_key);
                ctx.record_touched(rel_file, &table_label, &id_key);
            }
            Ok(true)
        }
        Value::Array(items) => {
            for (idx, entry_patch) in items.iter().enumerate() {
                let Some(id_key) = value_key_string(entry_patch, key_field) else {
                    return Err(format!(
                        "{table_label}[{idx}]: patch entry must carry `{key_field}`"
                    ));
                };
                let Some(record) = base_arr.iter_mut().find(|item| {
                    value_key_string(item, key_field).as_deref() == Some(id_key.as_str())
                }) else {
                    return Err(format!(
                        "{table_label}: no record with {key_field}={id_key}"
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
                    ctx,
                )?;
                ctx.record_touched(rel_file, &table_label, &id_key);
            }
            Ok(false)
        }
        _ => Err(format!(
            "{table_label}: expected an ID-keyed object or array of records carrying `{key_field}`"
        )),
    }
}

/// Diffs two ID-keyed pack arrays.
///
/// Existing records produce sparse ID-keyed field diffs. For additive-capable
/// tables (`spec.allow_add`), records appended after the unmodified base order
/// are exported into `"$add": [...]`. Deletions, reorders, mid-list insertions,
/// duplicates, and additions in non-additive tables fail closed.
pub(crate) fn diff_keyed_array(
    base_arr: &[Value],
    edited_arr: &[Value],
    rel_file: &str,
    schema_path: &[&str],
    display_path: &[&str],
    spec: &KeyedArraySpec,
    ctx: &mut DiffContext<'_>,
) -> Result<(Option<Value>, bool), String> {
    let key_field = spec.key_field;
    let label = if display_path.is_empty() {
        rel_file.to_owned()
    } else {
        display_path.join(".")
    };
    let mut base_by_id: BTreeMap<String, &Value> = BTreeMap::new();
    let mut base_order = Vec::with_capacity(base_arr.len());
    for (idx, item) in base_arr.iter().enumerate() {
        let Some(key) = value_key_string(item, key_field) else {
            return Err(format!(
                "{label}[{idx}]: base entry is missing `{key_field}`"
            ));
        };
        base_order.push(key.clone());
        base_by_id.insert(key, item);
    }

    let mut child_schema: Vec<&str> = schema_path.to_vec();
    child_schema.push("*");
    let mut seen = BTreeSet::new();
    let mut edited_order = Vec::with_capacity(edited_arr.len());
    let mut out = Map::new();
    let mut added_items = Vec::new();

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
        match base_by_id.get(&key) {
            Some(base_item) => {
                let mut child_display: Vec<&str> = display_path.to_vec();
                child_display.push(key.as_str());
                if let Some(d) = diff_at_slice(
                    base_item,
                    edited_item,
                    rel_file,
                    &child_schema,
                    &child_display,
                    ctx,
                )? {
                    out.insert(key, d);
                }
            }
            None => {
                if !spec.allow_add {
                    return Err(format!(
                        "{label}: adding new record {key_field}={key} is not supported in sparse ID-keyed patches"
                    ));
                }
                let exported = if is_tree_dialogue_entry(rel_file, &child_schema)
                    && let Value::Object(obj) = edited_item
                {
                    synthesize_new_dialogue_entry(obj)
                } else {
                    edited_item.clone()
                };
                added_items.push(exported);
            }
        }
    }

    for key in base_by_id.keys() {
        if !seen.contains(key) {
            return Err(format!(
                "{label}: removing record {key_field}={key} is not supported in sparse ID-keyed patches"
            ));
        }
    }

    let edited_base_order: Vec<&String> = edited_order
        .iter()
        .filter(|k| base_by_id.contains_key(*k))
        .collect();
    if edited_base_order.len() != base_order.len()
        || edited_base_order
            .iter()
            .zip(base_order.iter())
            .any(|(a, b)| *a != b)
    {
        return Err(format!(
            "{label}: reordering records is not supported in sparse ID-keyed patches"
        ));
    }

    if !added_items.is_empty() {
        if edited_order[..base_order.len()] != base_order[..] {
            return Err(format!(
                "{label}: newly added records must be appended after existing base records"
            ));
        }
        out.insert(ADD_RECORDS_KEY.to_owned(), Value::Array(added_items));
    }

    let added_any = out.contains_key(ADD_RECORDS_KEY);
    if out.is_empty() {
        Ok((None, false))
    } else {
        Ok((Some(Value::Object(out)), added_any))
    }
}

/// Walks a keyed array patch for [`super::patch::collect_whole_base_array_patches`],
/// ignoring `"$add"` (newly authored records are never whole-base-array copies).
pub(crate) fn walk_keyed_array_whole_base_arrays(
    base_arr: &[Value],
    patch: &Value,
    rel_file: &str,
    schema_path: &[&str],
    display_path: &[&str],
    key_field: &str,
    out: &mut Vec<String>,
) {
    let mut child_schema: Vec<&str> = schema_path.to_vec();
    child_schema.push("*");
    match patch {
        Value::Object(by_id) => {
            for (id_key, entry_patch) in by_id {
                if id_key == ADD_RECORDS_KEY {
                    continue;
                }
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
}
