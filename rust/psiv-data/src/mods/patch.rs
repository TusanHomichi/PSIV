//! Sparse ID-keyed JSON patching and reverse diffing for runtime pack tables.
//!
//! Pack tables store records in arrays keyed by stable cartridge IDs (`id`,
//! `character_id`, `tree`, `index`, `event_battle_index`, `level`, `map_id`).
//! Patching by ID instead of array index keeps mods resilient and readable.
//!
//! For `dialogue/trees.json` entries, two ergonomic helpers are also supported:
//!
//! - `"replace": [["old", "new"], ...]` — updates matching text in `segments`,
//!   `pages`, and `text` in lockstep.
//! - `"lines": ["Line 1", "Line 2"]` (or nested `[["Page 1 L1", "L2"], ["Page 2"]]`)
//!   — rebuilds `segments`, `pages`, and `text` cleanly (preserving any leading
//!   portrait control code unless `"keep_portrait": false`).

use super::schema_check;
use serde_json::{Map, Value, json};

/// Pattern matching one or more pack-relative JSON paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FilePattern {
    /// Exact pack-relative path (for example `"battle/enemies.json"`).
    Exact(&'static str),
    /// Directory prefix and filename suffix (for example `"maps/"` + `".json"`).
    Prefix {
        /// Leading directory prefix including trailing `/`.
        prefix: &'static str,
        /// Required filename suffix.
        suffix: &'static str,
    },
}

impl FilePattern {
    pub(crate) fn matches(self, rel_file: &str) -> bool {
        match self {
            Self::Exact(exact) => rel_file == exact,
            Self::Prefix { prefix, suffix } => {
                rel_file.starts_with(prefix) && rel_file.ends_with(suffix)
            }
        }
    }
}

/// Declarative specification of one ID-keyed pack table array.
///
/// Shared between [`array_key_field`] and the schema/pack guard tests so a
/// field rename cannot silently drift away from the actual pack schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct KeyedArraySpec {
    pub file: FilePattern,
    pub path: &'static [&'static str],
    pub key_field: &'static str,
}

pub(crate) const MAPS_JSON: FilePattern = FilePattern::Prefix {
    prefix: "maps/",
    suffix: ".json",
};

/// Every pack table array that supports sparse ID-keyed patching and diffing.
pub(crate) const KEYED_ARRAYS: &[KeyedArraySpec] = &[
    KeyedArraySpec {
        file: FilePattern::Exact("battle/enemies.json"),
        path: &["enemies"],
        key_field: "id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/equipment.json"),
        path: &["items"],
        key_field: "id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/characters.json"),
        path: &["characters"],
        key_field: "character_id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/abilities.json"),
        path: &["techniques"],
        key_field: "id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/abilities.json"),
        path: &["skills"],
        key_field: "id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/abilities.json"),
        path: &["enemy_skills"],
        key_field: "id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/abilities.json"),
        path: &["item_effects"],
        key_field: "id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/abilities.json"),
        path: &["combos"],
        key_field: "id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/formations.json"),
        path: &["formations"],
        key_field: "id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/formations.json"),
        path: &["boss_formations"],
        key_field: "event_battle_index",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/formations.json"),
        path: &["map_bindings"],
        key_field: "map_id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/levels.json"),
        path: &["characters"],
        key_field: "character_id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("battle/levels.json"),
        path: &["characters", "*", "levels"],
        key_field: "level",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("shops.json"),
        path: &["inventories"],
        key_field: "index",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("shops.json"),
        path: &["inns"],
        key_field: "index",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("shops.json"),
        path: &["counters"],
        key_field: "id",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("dialogue/trees.json"),
        path: &["trees"],
        key_field: "tree",
    },
    KeyedArraySpec {
        file: FilePattern::Exact("dialogue/trees.json"),
        path: &["trees", "*", "entries"],
        key_field: "id",
    },
    KeyedArraySpec {
        file: MAPS_JSON,
        path: &["npcs"],
        key_field: "index",
    },
    KeyedArraySpec {
        file: MAPS_JSON,
        path: &["warps"],
        key_field: "index",
    },
    KeyedArraySpec {
        file: MAPS_JSON,
        path: &["treasure_chests"],
        key_field: "index",
    },
    KeyedArraySpec {
        file: MAPS_JSON,
        path: &["interaction_areas"],
        key_field: "index",
    },
    KeyedArraySpec {
        file: MAPS_JSON,
        path: &["map_updates"],
        key_field: "index",
    },
];

/// Returns the stable ID field name when `path` inside `rel_file` addresses a
/// known ID-keyed record array.
pub(crate) fn array_key_field(rel_file: &str, path: &[&str]) -> Option<&'static str> {
    KEYED_ARRAYS
        .iter()
        .find(|spec| spec.file.matches(rel_file) && spec.path == path)
        .map(|spec| spec.key_field)
}

/// Returns `true` when `(rel_file, schema_path)` addresses a JSON object that
/// maps arbitrary user/asset keys rather than fixed struct fields.
fn is_dynamic_map(rel_file: &str, schema_path: &[&str]) -> bool {
    match (rel_file, schema_path) {
        (file, ["map_update_images" | "art_cycle_images"]) if MAPS_JSON.matches(file) => true,
        ("dialogue/font.json", ["by_char"]) => true,
        ("dialogue/window.json", ["roles"]) => true,
        _ => false,
    }
}

fn is_dialogue_entry(rel_file: &str, path: &[&str]) -> bool {
    rel_file == "dialogue/trees.json" && path == ["trees", "*", "entries", "*"]
}

fn format_path(display_path: &[&str], key: &str) -> String {
    if display_path.is_empty() {
        key.to_owned()
    } else {
        format!("{}.{key}", display_path.join("."))
    }
}

/// Applies `patch` onto `base` in place, using ID-keyed merging for known
/// table arrays and strict schema-validated merging for objects.
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
        (target, replacement) => {
            *target = replacement.clone();
            Ok(())
        }
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
            // Keep null entries on non-dynamic structs so schema_check::verify_patched
            // still sees (and rejects) unknown null-valued keys like `{"HP": null}`
            // instead of silently dropping them as a no-op.
            base_map.insert(k.to_owned(), Value::Null);
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

/// Applies `"replace"` and/or `"lines"` dialogue shorthands onto a dialogue entry
/// object, keeping `segments`, `pages`, and `text` consistent.
fn apply_dialogue_shorthand(
    entry: &mut Map<String, Value>,
    patch: &Map<String, Value>,
) -> Result<(), String> {
    if let Some(replaces) = patch.get("replace") {
        let pairs = replaces.as_array().ok_or_else(|| {
            "dialogue `replace` must be an array of [old, new] string pairs".to_owned()
        })?;
        for (i, pair) in pairs.iter().enumerate() {
            let arr = pair.as_array().ok_or_else(|| {
                format!("dialogue `replace[{i}]` must be a 2-element [old, new] array")
            })?;
            if arr.len() != 2 {
                return Err(format!(
                    "dialogue `replace[{i}]` must have exactly 2 elements"
                ));
            }
            let old = arr[0]
                .as_str()
                .ok_or_else(|| format!("dialogue `replace[{i}][0]` must be a string"))?;
            let new = arr[1]
                .as_str()
                .ok_or_else(|| format!("dialogue `replace[{i}][1]` must be a string"))?;
            let mut matched = false;
            if let Some(segments) = entry.get_mut("segments").and_then(Value::as_array_mut) {
                for seg in segments {
                    if let Some(text) = seg.get("text").and_then(Value::as_str)
                        && text.contains(old)
                    {
                        let replaced = text.replace(old, new);
                        seg["text"] = Value::String(replaced);
                        matched = true;
                    }
                }
            }
            if let Some(pages) = entry.get_mut("pages").and_then(Value::as_array_mut) {
                for page in pages {
                    if let Some(lines) = page.get_mut("lines").and_then(Value::as_array_mut) {
                        for line in lines {
                            if let Some(s) = line.as_str()
                                && s.contains(old)
                            {
                                *line = Value::String(s.replace(old, new));
                            }
                        }
                    }
                }
            }
            if let Some(text) = entry.get("text").and_then(Value::as_str) {
                entry.insert("text".to_owned(), Value::String(text.replace(old, new)));
            }
            if !matched {
                return Err(format!(
                    "dialogue `replace[{i}]` did not match any text segment: {old:?}"
                ));
            }
        }
    }

    if let Some(lines_val) = patch.get("lines") {
        let keep_portrait = match patch.get("keep_portrait") {
            None => true,
            Some(Value::Bool(flag)) => *flag,
            Some(_) => return Err("dialogue `keep_portrait` must be a boolean".to_owned()),
        };
        let pages_lines = parse_dialogue_lines(lines_val)?;
        let leading_portrait = if keep_portrait {
            entry
                .get("segments")
                .and_then(Value::as_array)
                .and_then(|segs| segs.first())
                .filter(|seg| seg.get("ctrl").and_then(Value::as_str) == Some("portrait"))
                .cloned()
        } else {
            None
        };

        let mut new_segments = Vec::new();
        if let Some(p) = leading_portrait {
            new_segments.push(p);
        }
        let mut new_pages = Vec::new();
        let mut flat_lines = Vec::new();
        let page_count = pages_lines.len();
        for (page_idx, page) in pages_lines.iter().enumerate() {
            if page_idx > 0 {
                new_segments.push(json!({"code": "0xFD", "ctrl": "wait", "operands": []}));
            }
            for (line_idx, line) in page.iter().enumerate() {
                if line_idx > 0 {
                    new_segments.push(json!({"code": "0xFC", "ctrl": "newline", "operands": []}));
                }
                new_segments.push(json!({"text": line}));
                flat_lines.push(line.as_str());
            }
            let end = if page_idx + 1 < page_count {
                "wait"
            } else {
                "end"
            };
            let mut page_obj = Map::new();
            page_obj.insert("lines".to_owned(), json!(page));
            page_obj.insert("end".to_owned(), json!(end));
            new_pages.push(Value::Object(page_obj));
        }

        entry.insert("segments".to_owned(), Value::Array(new_segments));
        entry.insert("pages".to_owned(), Value::Array(new_pages));
        entry.insert("text".to_owned(), Value::String(flat_lines.join("\n")));
    }

    Ok(())
}

fn parse_dialogue_lines(value: &Value) -> Result<Vec<Vec<String>>, String> {
    let arr = value
        .as_array()
        .ok_or_else(|| "dialogue `lines` must be an array of strings or pages".to_owned())?;
    if arr.is_empty() {
        return Err("dialogue `lines` must not be empty".to_owned());
    }
    if arr.iter().all(Value::is_string) {
        let lines: Vec<String> = arr
            .iter()
            .map(|v| v.as_str().unwrap_or_default().to_owned())
            .collect();
        return Ok(vec![lines]);
    }
    let mut pages = Vec::with_capacity(arr.len());
    for (i, page) in arr.iter().enumerate() {
        let sub = page
            .as_array()
            .ok_or_else(|| format!("dialogue `lines[{i}]` must be an array of strings"))?;
        let mut lines = Vec::with_capacity(sub.len());
        for (j, line) in sub.iter().enumerate() {
            let s = line
                .as_str()
                .ok_or_else(|| format!("dialogue `lines[{i}][{j}]` must be a string"))?;
            lines.push(s.to_owned());
        }
        pages.push(lines);
    }
    Ok(pages)
}

/// Computes a minimal sparse JSON patch such that applying it to `base`
/// reproduces `edited`. Returns `Ok(None)` when `base == edited`.
///
/// Returns `Err` if a known ID-keyed table adds, removes, duplicates, or omits
/// record IDs, because sparse ID-keyed patches only update existing records.
pub fn diff_json(base: &Value, edited: &Value, rel_file: &str) -> Result<Option<Value>, String> {
    diff_at_slice(base, edited, rel_file, &[])
}

fn diff_at_slice(
    base: &Value,
    edited: &Value,
    rel_file: &str,
    path: &[&str],
) -> Result<Option<Value>, String> {
    if base == edited {
        return Ok(None);
    }
    if let (Value::Array(base_arr), Value::Array(edited_arr), Some(key_field)) =
        (base, edited, array_key_field(rel_file, path))
    {
        return diff_keyed_array(base_arr, edited_arr, rel_file, path, key_field);
    }

    match (base, edited) {
        (Value::Object(base_map), Value::Object(edited_map)) => {
            let mut out = Map::new();
            for (k, ev) in edited_map {
                match base_map.get(k) {
                    Some(bv) => {
                        let mut next_path: Vec<&str> = path.to_vec();
                        next_path.push(k.as_str());
                        if let Some(d) = diff_at_slice(bv, ev, rel_file, &next_path)? {
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
        _ => Ok(Some(edited.clone())),
    }
}

fn diff_keyed_array(
    base_arr: &[Value],
    edited_arr: &[Value],
    rel_file: &str,
    path: &[&str],
    key_field: &str,
) -> Result<Option<Value>, String> {
    let label = if path.is_empty() {
        rel_file.to_owned()
    } else {
        path.join(".")
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
    let mut child_path: Vec<&str> = path.to_vec();
    child_path.push("*");
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
        if let Some(d) = diff_at_slice(base_item, edited_item, rel_file, &child_path)? {
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
