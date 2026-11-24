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

use serde_json::{Map, Value, json};

/// Returns the stable ID field name when `path` inside `rel_file` addresses a
/// known ID-keyed record array.
fn array_key_field(rel_file: &str, path: &[&str]) -> Option<&'static str> {
    match (rel_file, path) {
        ("battle/enemies.json", ["enemies"]) => Some("id"),
        ("battle/equipment.json", ["items"]) => Some("id"),
        ("battle/characters.json", ["characters"]) => Some("character_id"),
        (
            "battle/abilities.json",
            ["techniques" | "skills" | "enemy_skills" | "item_effects" | "combos"],
        ) => Some("id"),
        ("battle/formations.json", ["formations"]) => Some("id"),
        ("battle/formations.json", ["boss_formations"]) => Some("event_battle_index"),
        ("battle/formations.json", ["map_bindings"]) => Some("map_id"),
        ("battle/levels.json", ["characters"]) => Some("character_id"),
        ("battle/levels.json", ["characters", "*", "levels"]) => Some("level"),
        ("shops.json", ["inventories" | "inns"]) => Some("index"),
        ("shops.json", ["counters"]) => Some("id"),
        ("dialogue/trees.json", ["trees"]) => Some("tree"),
        ("dialogue/trees.json", ["trees", "*", "entries"]) => Some("id"),
        (file, ["npcs" | "warps" | "treasures"]) if file.starts_with("maps/") => Some("index"),
        _ => None,
    }
}

fn is_dialogue_entry(rel_file: &str, path: &[&str]) -> bool {
    rel_file == "dialogue/trees.json" && path == ["trees", "*", "entries", "*"]
}

/// Applies `patch` onto `base` in place, using ID-keyed merging for known
/// table arrays and RFC 7396 recursive merge for objects.
pub fn apply_json_patch(base: &mut Value, patch: &Value, rel_file: &str) -> Result<(), String> {
    apply_at_slice(base, patch, rel_file, &[])
}

fn apply_at_slice(
    base: &mut Value,
    patch: &Value,
    rel_file: &str,
    path: &[&str],
) -> Result<(), String> {
    if is_dialogue_entry(rel_file, path)
        && let (Value::Object(base_obj), Value::Object(patch_obj)) = (base as &mut Value, patch)
        && (patch_obj.contains_key("replace") || patch_obj.contains_key("lines"))
    {
        apply_dialogue_shorthand(base_obj, patch_obj)?;
        for (k, v) in patch_obj {
            if matches!(k.as_str(), "replace" | "lines" | "keep_portrait") {
                continue;
            }
            if v.is_null() {
                base_obj.remove(k);
            } else {
                base_obj.insert(k.clone(), v.clone());
            }
        }
        return Ok(());
    }

    if let (Value::Array(base_arr), Some(key_field)) =
        (base as &mut Value, array_key_field(rel_file, path))
    {
        return apply_keyed_array(base_arr, patch, rel_file, path, key_field);
    }

    match (base, patch) {
        (Value::Object(base_map), Value::Object(patch_map)) => {
            for (k, v) in patch_map {
                if v.is_null() {
                    base_map.remove(k);
                    continue;
                }
                if let Some(existing) = base_map.get_mut(k) {
                    let mut next_path: Vec<&str> = path.to_vec();
                    next_path.push(k.as_str());
                    apply_at_slice(existing, v, rel_file, &next_path)?;
                } else {
                    base_map.insert(k.clone(), v.clone());
                }
            }
            Ok(())
        }
        (target, replacement) => {
            *target = replacement.clone();
            Ok(())
        }
    }
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
    path: &[&str],
    key_field: &str,
) -> Result<(), String> {
    let mut child_path: Vec<&str> = path.to_vec();
    child_path.push("*");

    match patch {
        Value::Object(by_id) => {
            for (id_key, entry_patch) in by_id {
                let Some(record) = base_arr.iter_mut().find(|item| {
                    value_key_string(item, key_field).as_deref() == Some(id_key.as_str())
                }) else {
                    return Err(format!(
                        "{}: no record with {key_field}={id_key}",
                        path.join(".")
                    ));
                };
                apply_at_slice(record, entry_patch, rel_file, &child_path)?;
            }
            Ok(())
        }
        Value::Array(items) => {
            for (idx, entry_patch) in items.iter().enumerate() {
                let Some(id_key) = value_key_string(entry_patch, key_field) else {
                    return Err(format!(
                        "{}[{idx}]: patch entry must carry `{key_field}`",
                        path.join(".")
                    ));
                };
                let Some(record) = base_arr.iter_mut().find(|item| {
                    value_key_string(item, key_field).as_deref() == Some(id_key.as_str())
                }) else {
                    return Err(format!(
                        "{}: no record with {key_field}={id_key}",
                        path.join(".")
                    ));
                };
                apply_at_slice(record, entry_patch, rel_file, &child_path)?;
            }
            Ok(())
        }
        _ => Err(format!(
            "{}: expected an ID-keyed object or array of records carrying `{key_field}`",
            path.join(".")
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
        let keep_portrait = patch
            .get("keep_portrait")
            .and_then(Value::as_bool)
            .unwrap_or(true);
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
        let portrait_operand = leading_portrait
            .as_ref()
            .and_then(|seg| seg.get("operands"))
            .and_then(Value::as_array)
            .and_then(|ops| ops.first())
            .and_then(Value::as_u64);

        let mut new_segments = Vec::new();
        if let Some(p) = leading_portrait {
            new_segments.push(p);
        }
        let mut new_pages = Vec::new();
        let mut flat_lines = Vec::new();
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
            let mut page_obj = Map::new();
            page_obj.insert("lines".to_owned(), json!(page));
            if let Some(pid) = portrait_operand {
                page_obj.insert("portrait".to_owned(), json!(pid));
            }
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
/// reproduces `edited`. Returns `None` when `base == edited`.
#[must_use]
pub fn diff_json(base: &Value, edited: &Value, rel_file: &str) -> Option<Value> {
    diff_at_slice(base, edited, rel_file, &[])
}

fn diff_at_slice(base: &Value, edited: &Value, rel_file: &str, path: &[&str]) -> Option<Value> {
    if base == edited {
        return None;
    }
    if let (Value::Array(base_arr), Value::Array(edited_arr), Some(key_field)) =
        (base, edited, array_key_field(rel_file, path))
        && base_arr.len() == edited_arr.len()
        && let Some(keyed) = diff_keyed_array(base_arr, edited_arr, rel_file, path, key_field)
    {
        return keyed;
    }

    match (base, edited) {
        (Value::Object(base_map), Value::Object(edited_map)) => {
            let mut out = Map::new();
            for (k, ev) in edited_map {
                match base_map.get(k) {
                    Some(bv) => {
                        let mut next_path: Vec<&str> = path.to_vec();
                        next_path.push(k.as_str());
                        if let Some(d) = diff_at_slice(bv, ev, rel_file, &next_path) {
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
                None
            } else {
                Some(Value::Object(out))
            }
        }
        _ => Some(edited.clone()),
    }
}

fn diff_keyed_array(
    base_arr: &[Value],
    edited_arr: &[Value],
    rel_file: &str,
    path: &[&str],
    key_field: &str,
) -> Option<Option<Value>> {
    let mut base_by_id = Map::new();
    for item in base_arr {
        let key = value_key_string(item, key_field)?;
        base_by_id.insert(key, item.clone());
    }
    let mut child_path: Vec<&str> = path.to_vec();
    child_path.push("*");
    let mut out = Map::new();
    for edited_item in edited_arr {
        let key = value_key_string(edited_item, key_field)?;
        let base_item = base_by_id.get(&key)?;
        if let Some(d) = diff_at_slice(base_item, edited_item, rel_file, &child_path) {
            out.insert(key, d);
        }
    }
    if out.is_empty() {
        Some(None)
    } else {
        Some(Some(Value::Object(out)))
    }
}
