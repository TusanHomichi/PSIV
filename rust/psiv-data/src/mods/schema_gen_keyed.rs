//! Keyed-array JSON Schema builders (sparse edit patches + explicit `"$add"`
//! complete-record creation) for [`super::schema_gen`].

use super::dialogue_patch::is_tree_dialogue_entry;
use super::keyed::ADD_RECORDS_KEY;
use super::schema_gen::{
    dialogue_lines_schema, is_null_schema, key_field_is_integer, register_named_def, resolve_ref,
    transform_complete, transform_sparse,
};
use super::tables::KeyedArraySpec;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};

/// Builds the Draft 2020-12 schema for an ID-keyed pack array (`KEYED_ARRAYS`).
///
/// - Edit-only tables accept either an ID-keyed object of sparse `<Item>Patch`
///   values or an array of sparse `<Item>Patch` values carrying `key_field`.
/// - Additive-capable tables (`spec.allow_add`) additionally allow an explicit
///   `"$add"` array property on the object form whose items must satisfy the
///   complete-record `<Item>New` schema (all required fields, `additionalProperties: false`).
#[allow(clippy::too_many_arguments)]
pub(super) fn build_keyed_array_schema(
    obj: &Map<String, Value>,
    arr_nullable: bool,
    rel_file: &str,
    path: &mut Vec<String>,
    spec: &KeyedArraySpec,
    raw_defs: &Map<String, Value>,
    out_defs: &mut BTreeMap<String, Value>,
    visited: &mut BTreeSet<String>,
) -> Value {
    let key_field = spec.key_field;
    let item_schema = obj.get("items").unwrap_or(&Value::Bool(true));
    let base_def_name = item_schema
        .as_object()
        .and_then(|o| o.get("$ref"))
        .and_then(Value::as_str)
        .and_then(|r| r.strip_prefix("#/$defs/"));

    path.push("*".to_owned());
    let sparse_item = transform_sparse(item_schema, rel_file, path, raw_defs, out_defs, visited);
    let is_tree_dialogue = is_tree_dialogue_entry(
        rel_file,
        &path.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    path.pop();

    let key_is_int = key_field_is_integer(&sparse_item, key_field);
    let item_ref = if let Some(base_name) = base_def_name {
        let def_name = register_named_def(out_defs, &format!("{base_name}Patch"), &sparse_item);
        json!({ "$ref": format!("#/$defs/{def_name}") })
    } else {
        sparse_item
    };

    let key_name_schema = if key_is_int {
        json!({ "pattern": "^(0|[1-9][0-9]*)$" })
    } else {
        json!({ "minLength": 1 })
    };

    let object_form = if spec.allow_add {
        let complete_item = build_new_record_schema(
            item_schema,
            rel_file,
            key_field,
            is_tree_dialogue,
            raw_defs,
            out_defs,
            visited,
        );
        let new_item_ref = if let Some(base_name) = base_def_name {
            let def_name = register_named_def(out_defs, &format!("{base_name}New"), &complete_item);
            json!({ "$ref": format!("#/$defs/{def_name}") })
        } else {
            complete_item
        };
        let mut props = Map::new();
        props.insert(
            ADD_RECORDS_KEY.to_owned(),
            json!({
                "type": "array",
                "minItems": 1,
                "description": format!("Explicit list of complete new records to append (each must carry a unique `{key_field}`)."),
                "items": new_item_ref
            }),
        );
        json!({
            "type": "object",
            "description": format!("Sparse ID-keyed patch map keyed by `{key_field}`, plus optional `{ADD_RECORDS_KEY}` list for adding new records."),
            "properties": props,
            "propertyNames": {
                "anyOf": [
                    { "const": ADD_RECORDS_KEY },
                    key_name_schema
                ]
            },
            "additionalProperties": item_ref
        })
    } else {
        json!({
            "type": "object",
            "description": format!("Sparse ID-keyed patch map keyed by `{key_field}`."),
            "propertyNames": key_name_schema,
            "additionalProperties": item_ref
        })
    };

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
    Value::Object(out)
}

fn build_new_record_schema(
    item_schema: &Value,
    rel_file: &str,
    key_field: &str,
    is_tree_dialogue: bool,
    raw_defs: &Map<String, Value>,
    out_defs: &mut BTreeMap<String, Value>,
    visited: &mut BTreeSet<String>,
) -> Value {
    let resolved = resolve_ref(item_schema, raw_defs).unwrap_or_else(|| item_schema.clone());
    let mut complete = transform_complete(&resolved, raw_defs, out_defs, visited);

    if let Some(obj) = complete.as_object_mut() {
        ensure_non_null_required_key(obj, key_field);
        if rel_file == "battle/formations.json"
            && let Some(props) = obj.get_mut("properties").and_then(Value::as_object_mut)
        {
            props.remove("event_battle_index");
            if let Some(enemies) = props.get_mut("enemies").and_then(Value::as_object_mut) {
                enemies.insert("minItems".to_owned(), json!(1));
            }
        }
    }

    if is_tree_dialogue {
        let id_schema = complete
            .as_object()
            .and_then(|o| o.get("properties"))
            .and_then(Value::as_object)
            .and_then(|p| p.get("id"))
            .cloned()
            .unwrap_or_else(|| json!({"type": "integer", "minimum": 0, "maximum": 65535}));
        let shorthand = json!({
            "type": "object",
            "description": "Concise new dialogue entry using `lines` shorthand (automatically expanded into `segments`, `pages`, and `text`).",
            "required": ["id", "lines"],
            "additionalProperties": false,
            "properties": {
                "id": id_schema,
                "lines": dialogue_lines_schema()
            }
        });
        return json!({
            "description": "Complete new dialogue entry (either full `text`/`segments`/`pages` record or concise `lines` shorthand).",
            "oneOf": [complete, shorthand]
        });
    }

    complete
}

fn ensure_non_null_required_key(obj: &mut Map<String, Value>, key_field: &str) {
    let required = obj
        .entry("required".to_owned())
        .or_insert_with(|| Value::Array(Vec::new()));
    if let Some(arr) = required.as_array_mut()
        && !arr.iter().any(|v| v.as_str() == Some(key_field))
    {
        arr.push(Value::String(key_field.to_owned()));
    }
    if let Some(props) = obj.get_mut("properties").and_then(Value::as_object_mut)
        && let Some(prop) = props.get_mut(key_field)
    {
        *prop = strip_null_from_schema(prop.clone());
    }
}

fn strip_null_from_schema(schema: Value) -> Value {
    match schema {
        Value::Object(mut obj) => {
            if let Some(Value::Array(types)) = obj.get_mut("type") {
                types.retain(|v| v.as_str() != Some("null"));
                if types.len() == 1 {
                    let single = types.remove(0);
                    obj.insert("type".to_owned(), single);
                }
            }
            if let Some(Value::Array(branches)) = obj.get_mut("anyOf") {
                branches.retain(|b| !is_null_schema(b));
                if branches.len() == 1 {
                    return branches.remove(0);
                }
            }
            Value::Object(obj)
        }
        other => other,
    }
}
