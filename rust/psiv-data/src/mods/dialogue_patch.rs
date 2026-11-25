//! Ergonomic `"replace"` and `"lines"` dialogue shorthands for
//! `dialogue/trees.json` entries, plus exact reverse synthesis during diffing.

use serde_json::{Map, Value, json};

pub(crate) fn is_dialogue_entry(rel_file: &str, path: &[&str]) -> bool {
    rel_file == "dialogue/trees.json"
        && (path == ["trees", "*", "entries", "*"] || path == ["system_messages", "messages", "*"])
}

/// Applies `"replace"` and/or `"lines"` dialogue shorthands onto a dialogue entry
/// object, keeping `segments`, `pages`, and `text` consistent.
pub(crate) fn apply_dialogue_shorthand(
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

/// Attempts to express a dialogue entry diff using `"replace"` or `"lines"`
/// shorthand so editing dialogue text never dumps retail `segments`/`pages`
/// arrays. Verifies exact equivalence of `segments`, `pages`, and `text`.
pub(crate) fn synthesize_dialogue_shorthand(
    base_obj: &Map<String, Value>,
    edited_obj: &Map<String, Value>,
) -> Option<Map<String, Value>> {
    if base_obj.get("segments") == edited_obj.get("segments")
        && base_obj.get("pages") == edited_obj.get("pages")
    {
        return None;
    }

    let shorthand_matches = |candidate: &Map<String, Value>| -> bool {
        let mut trial = base_obj.clone();
        apply_dialogue_shorthand(&mut trial, candidate).is_ok()
            && trial.get("segments") == edited_obj.get("segments")
            && trial.get("pages") == edited_obj.get("pages")
            && trial.get("text") == edited_obj.get("text")
    };

    // 1. Prefer `"replace"` when segment structure is unchanged and only text runs changed.
    if let (Some(Value::Array(base_segs)), Some(Value::Array(edited_segs))) =
        (base_obj.get("segments"), edited_obj.get("segments"))
        && base_segs.len() == edited_segs.len()
    {
        let mut pairs = Vec::new();
        let mut only_text_changed = true;
        for (b, e) in base_segs.iter().zip(edited_segs.iter()) {
            if b == e {
                continue;
            }
            let (Some(bo), Some(eo)) = (b.as_object(), e.as_object()) else {
                only_text_changed = false;
                break;
            };
            if bo.len() != 1 || eo.len() != 1 {
                only_text_changed = false;
                break;
            }
            let (Some(bt), Some(et)) = (
                bo.get("text").and_then(Value::as_str),
                eo.get("text").and_then(Value::as_str),
            ) else {
                only_text_changed = false;
                break;
            };
            let pair = json!([bt, et]);
            if !pairs.contains(&pair) {
                pairs.push(pair);
            }
        }
        if only_text_changed && !pairs.is_empty() {
            let mut candidate = Map::new();
            candidate.insert("replace".to_owned(), Value::Array(pairs));
            if shorthand_matches(&candidate) {
                return Some(candidate);
            }
        }
    }

    // 2. Otherwise try `"lines"` (preserving leading portrait if present, else `keep_portrait: false`).
    if let Some(Value::Array(edited_pages)) = edited_obj.get("pages")
        && !edited_pages.is_empty()
    {
        let mut page_lines: Vec<Vec<String>> = Vec::with_capacity(edited_pages.len());
        let mut ok = true;
        for page in edited_pages {
            let Some(lines) = page.get("lines").and_then(Value::as_array) else {
                ok = false;
                break;
            };
            let mut row = Vec::with_capacity(lines.len());
            for line in lines {
                let Some(s) = line.as_str() else {
                    ok = false;
                    break;
                };
                row.push(s.to_owned());
            }
            if !ok {
                break;
            }
            page_lines.push(row);
        }
        if ok {
            let lines_val = if page_lines.len() == 1 {
                json!(page_lines[0])
            } else {
                json!(page_lines)
            };
            let mut keep = Map::new();
            keep.insert("lines".to_owned(), lines_val.clone());
            if shorthand_matches(&keep) {
                return Some(keep);
            }
            let mut drop_portrait = Map::new();
            drop_portrait.insert("lines".to_owned(), lines_val);
            drop_portrait.insert("keep_portrait".to_owned(), Value::Bool(false));
            if shorthand_matches(&drop_portrait) {
                return Some(drop_portrait);
            }
        }
    }

    None
}
