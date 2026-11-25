//! Tests for published JSON Schemas, CI sync guard, schema-vs-loader parity,
//! and mod/pack version compatibility policy (#790).

use super::*;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static SCHEMA_TEST_ID: AtomicUsize = AtomicUsize::new(1);

fn temp_dir(label: &str) -> PathBuf {
    let n = SCHEMA_TEST_ID.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "psiv-mod-schema-test-{label}-{}-{n}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn docs_schemas_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/schemas")
}

pub(super) fn load_published_schema(rel: &str) -> Value {
    let path = docs_schemas_dir().join(rel);
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("could not read published schema {}: {e}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("invalid JSON in {}: {e}", path.display()))
}

// ---------------------------------------------------------------------------
// Focused Draft 2020-12 JSON Schema validator for test assertions
// ---------------------------------------------------------------------------

pub(super) fn validate_against_schema(root: &Value, instance: &Value) -> Result<(), String> {
    validate_node(root, root, instance, "$")
}

fn validate_node(root: &Value, schema: &Value, instance: &Value, path: &str) -> Result<(), String> {
    match schema {
        Value::Bool(true) => return Ok(()),
        Value::Bool(false) => return Err(format!("{path}: value disallowed by false schema")),
        Value::Object(_) => {}
        _ => return Err(format!("{path}: malformed schema node")),
    }
    let obj = schema.as_object().unwrap();

    if let Some(ref_str) = obj.get("$ref").and_then(Value::as_str) {
        let def_name = ref_str
            .strip_prefix("#/$defs/")
            .ok_or_else(|| format!("{path}: unsupported $ref {ref_str}"))?;
        let target = root
            .get("$defs")
            .and_then(|d| d.get(def_name))
            .ok_or_else(|| format!("{path}: unresolved $ref {ref_str}"))?;
        validate_node(root, target, instance, path)?;
    }

    if let Some(expected_const) = obj.get("const")
        && instance != expected_const
    {
        return Err(format!(
            "{path}: expected const {expected_const}, got {instance}"
        ));
    }

    if let Some(allowed) = obj.get("enum").and_then(Value::as_array)
        && !allowed.contains(instance)
    {
        return Err(format!("{path}: value {instance} is not in enum"));
    }

    if let Some(not_schema) = obj.get("not")
        && validate_node(root, not_schema, instance, path).is_ok()
    {
        return Err(format!(
            "{path}: value {instance} matched forbidden `not` schema"
        ));
    }

    if let Some(ty) = obj.get("type")
        && !matches_json_type(ty, instance)
    {
        return Err(format!(
            "{path}: type mismatch (expected {ty}, got {instance})"
        ));
    }

    if let Some(s) = instance.as_str() {
        let len = s.chars().count();
        if let Some(min) = obj.get("minLength").and_then(Value::as_u64)
            && (len as u64) < min
        {
            return Err(format!("{path}: string length {len} < minLength {min}"));
        }
        if let Some(max) = obj.get("maxLength").and_then(Value::as_u64)
            && (len as u64) > max
        {
            return Err(format!("{path}: string length {len} > maxLength {max}"));
        }
        if let Some(pat) = obj.get("pattern").and_then(Value::as_str)
            && !matches_schema_pattern(pat, s)
        {
            return Err(format!("{path}: string {s:?} failed pattern {pat:?}"));
        }
    }

    if let Some(num) = instance.as_i64() {
        if let Some(min) = obj.get("minimum").and_then(Value::as_i64)
            && num < min
        {
            return Err(format!("{path}: integer {num} < minimum {min}"));
        }
        if let Some(max) = obj.get("maximum").and_then(Value::as_i64)
            && num > max
        {
            return Err(format!("{path}: integer {num} > maximum {max}"));
        }
    } else if let Some(num) = instance.as_u64() {
        if let Some(min) = obj.get("minimum").and_then(Value::as_u64)
            && num < min
        {
            return Err(format!("{path}: integer {num} < minimum {min}"));
        }
        if let Some(max) = obj.get("maximum").and_then(Value::as_u64)
            && num > max
        {
            return Err(format!("{path}: integer {num} > maximum {max}"));
        }
    }

    if let Some(map) = instance.as_object() {
        if let Some(req) = obj.get("required").and_then(Value::as_array) {
            for key in req.iter().filter_map(Value::as_str) {
                if !map.contains_key(key) {
                    return Err(format!("{path}: missing required field `{key}`"));
                }
            }
        }
        if let Some(dep_req) = obj.get("dependentRequired").and_then(Value::as_object) {
            for (trigger, required_keys) in dep_req {
                if map.contains_key(trigger)
                    && let Some(keys) = required_keys.as_array()
                {
                    for req_key in keys.iter().filter_map(Value::as_str) {
                        if !map.contains_key(req_key) {
                            return Err(format!(
                                "{path}: field `{trigger}` requires sibling field `{req_key}`"
                            ));
                        }
                    }
                }
            }
        }
        if let Some(prop_names) = obj.get("propertyNames") {
            for key in map.keys() {
                validate_node(
                    root,
                    prop_names,
                    &Value::String(key.clone()),
                    &format!("{path}.{key}"),
                )?;
            }
        }
        let props = obj.get("properties").and_then(Value::as_object);
        let add_props = obj.get("additionalProperties");
        for (k, v) in map {
            let child_path = format!("{path}.{k}");
            if let Some(prop_schema) = props.and_then(|p| p.get(k)) {
                validate_node(root, prop_schema, v, &child_path)?;
            } else if let Some(ap) = add_props {
                match ap {
                    Value::Bool(false) => {
                        return Err(format!("{child_path}: unknown field `{k}` disallowed"));
                    }
                    Value::Bool(true) => {}
                    sub => validate_node(root, sub, v, &child_path)?,
                }
            }
        }
    }

    if let Some(arr) = instance.as_array() {
        if let Some(min) = obj.get("minItems").and_then(Value::as_u64)
            && (arr.len() as u64) < min
        {
            return Err(format!("{path}: array len {} < minItems {min}", arr.len()));
        }
        if let Some(max) = obj.get("maxItems").and_then(Value::as_u64)
            && (arr.len() as u64) > max
        {
            return Err(format!("{path}: array len {} > maxItems {max}", arr.len()));
        }
        let prefix_len = if let Some(prefixes) = obj.get("prefixItems").and_then(Value::as_array) {
            for (idx, (elem, sub)) in arr.iter().zip(prefixes.iter()).enumerate() {
                validate_node(root, sub, elem, &format!("{path}[{idx}]"))?;
            }
            prefixes.len()
        } else {
            0
        };
        if let Some(items_schema) = obj.get("items") {
            for (idx, elem) in arr.iter().enumerate().skip(prefix_len) {
                validate_node(root, items_schema, elem, &format!("{path}[{idx}]"))?;
            }
        }
    }

    if let Some(branches) = obj.get("oneOf").and_then(Value::as_array) {
        let mut passed = 0usize;
        let mut last_err = String::new();
        for branch in branches {
            match validate_node(root, branch, instance, path) {
                Ok(()) => passed += 1,
                Err(e) => last_err = e,
            }
        }
        if passed != 1 {
            return Err(format!(
                "{path}: expected exactly 1 matching `oneOf` branch, got {passed} (last error: {last_err})"
            ));
        }
    }

    if let Some(branches) = obj.get("anyOf").and_then(Value::as_array) {
        let mut last_err = String::new();
        let mut matched = false;
        for branch in branches {
            match validate_node(root, branch, instance, path) {
                Ok(()) => {
                    matched = true;
                    break;
                }
                Err(e) => last_err = e,
            }
        }
        if !matched {
            return Err(format!(
                "{path}: matched no `anyOf` branch (last error: {last_err})"
            ));
        }
    }

    if let Some(branches) = obj.get("allOf").and_then(Value::as_array) {
        for branch in branches {
            validate_node(root, branch, instance, path)?;
        }
    }

    Ok(())
}

fn matches_json_type(expected: &Value, instance: &Value) -> bool {
    match expected {
        Value::String(ty) => matches_single_type(ty, instance),
        Value::Array(arr) => arr
            .iter()
            .filter_map(Value::as_str)
            .any(|ty| matches_single_type(ty, instance)),
        _ => true,
    }
}

fn matches_single_type(ty: &str, instance: &Value) -> bool {
    match ty {
        "null" => instance.is_null(),
        "boolean" => instance.is_boolean(),
        "integer" => instance.as_i64().is_some() || instance.as_u64().is_some(),
        "number" => instance.is_number(),
        "string" => instance.is_string(),
        "array" => instance.is_array(),
        "object" => instance.is_object(),
        _ => true,
    }
}

fn matches_schema_pattern(pattern: &str, text: &str) -> bool {
    match pattern {
        "^[a-z0-9_-]+$" => {
            !text.is_empty()
                && text
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
        }
        "\\S" => text.chars().any(|c| !c.is_whitespace()),
        "^[0-9]+$" => !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()),
        "^(0|[1-9][0-9]*)$" => {
            text == "0"
                || (!text.is_empty()
                    && matches!(text.as_bytes()[0], b'1'..=b'9')
                    && text.bytes().all(|b| b.is_ascii_digit()))
        }
        "^[0-9a-fA-F]{64}$" => text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit()),
        "^[0-9a-fA-F]*$" => text.bytes().all(|b| b.is_ascii_hexdigit()),
        "^0[xX][0-9a-fA-F]{1,2}$" => {
            let Some(rest) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) else {
                return false;
            };
            (1..=2).contains(&rest.len()) && rest.bytes().all(|b| b.is_ascii_hexdigit())
        }
        other => panic!("unhandled schema regex pattern in test validator: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 1. CI sync guard: docs/schemas/ must stay byte-for-byte synced with psiv-data
// ---------------------------------------------------------------------------

#[test]
fn published_docs_schemas_are_in_sync_with_psiv_data() {
    let dir = docs_schemas_dir();
    if let Err(problems) = check_mod_schemas(&dir) {
        panic!(
            "docs/schemas/ is out of sync with psiv-data:\n{}\n\
             Run `cargo run --manifest-path rust/Cargo.toml -p psiv-pack -- mod schema docs/schemas`",
            problems.join("\n")
        );
    }

    // Negative control: check_mod_schemas must fail on a mutated copy and on a stale file.
    let scratch = temp_dir("sync-neg");
    let written = write_mod_schemas(&scratch).unwrap();
    assert!(written.contains(&MOD_SCHEMA_FILE.to_owned()));
    assert!(written.contains(&"patches/battle/enemies.schema.json".to_owned()));
    assert!(written.contains(&"patches/dialogue/trees.schema.json".to_owned()));
    assert!(written.contains(&"patches/map.schema.json".to_owned()));
    check_mod_schemas(&scratch).expect("freshly written schemas must pass check");

    fs::write(scratch.join("patches/stale_old.schema.json"), "{}\n").unwrap();
    let stale_err = check_mod_schemas(&scratch).expect_err("stale schema file must fail check");
    assert!(
        stale_err
            .iter()
            .any(|m| m.contains("stale_old.schema.json")),
        "{stale_err:?}"
    );
    fs::remove_file(scratch.join("patches/stale_old.schema.json")).unwrap();

    fs::write(scratch.join(MOD_SCHEMA_FILE), "{}\n").unwrap();
    let drift_err = check_mod_schemas(&scratch).expect_err("drifted schema must fail check");
    assert!(
        drift_err.iter().any(|m| m.contains(MOD_SCHEMA_FILE)),
        "{drift_err:?}"
    );

    let _ = fs::remove_dir_all(&scratch);
}

// ---------------------------------------------------------------------------
// 2. Schema validation & parity with runtime loader (positive + negative controls)
// ---------------------------------------------------------------------------

#[test]
fn mod_manifest_and_profile_schemas_match_runtime_validation() {
    let mod_schema = load_published_schema(MOD_SCHEMA_FILE);
    let profile_schema = load_published_schema(MOD_PROFILE_SCHEMA_FILE);

    // Positive controls: minimal and full mod.json.
    for valid in [
        json!({
            "format_version": MOD_FORMAT_VERSION,
            "id": "hard-mode",
            "name": "Hard Mode",
            "version": "1.0.0"
        }),
        serde_json::to_value(ModManifest::new("tough_zoran-1", "Tough Zoran", "2.0.0")).unwrap(),
    ] {
        validate_against_schema(&mod_schema, &valid)
            .unwrap_or_else(|e| panic!("valid mod.json rejected by schema: {e}"));
        let parsed: ModManifest = serde_json::from_value(valid).unwrap();
        parsed.validate(Path::new(MOD_MANIFEST_FILE)).unwrap();
    }

    // Negative controls: every bad manifest fails BOTH JSON Schema and runtime loader.
    for (label, bad) in [
        (
            "unknown field",
            json!({"format_version": 1, "id": "bad", "name": "Bad", "version": "1.0.0", "enabled_flag": true}),
        ),
        (
            "uppercase id",
            json!({"format_version": 1, "id": "BadMod", "name": "Bad", "version": "1.0.0"}),
        ),
        (
            "windows reserved id",
            json!({"format_version": 1, "id": "con", "name": "Bad", "version": "1.0.0"}),
        ),
        (
            "blank name",
            json!({"format_version": 1, "id": "bad", "name": "   ", "version": "1.0.0"}),
        ),
        (
            "bad format_version too old",
            json!({"format_version": 0, "id": "bad", "name": "Bad", "version": "1.0.0"}),
        ),
        (
            "bad format_version too new",
            json!({"format_version": MOD_FORMAT_VERSION + 1, "id": "bad", "name": "Bad", "version": "1.0.0"}),
        ),
        (
            "bad pack_format_version too old",
            json!({"format_version": 1, "id": "bad", "name": "Bad", "version": "1.0.0", "pack_format_version": 1}),
        ),
        (
            "bad pack_format_version too new",
            json!({"format_version": 1, "id": "bad", "name": "Bad", "version": "1.0.0", "pack_format_version": PACK_FORMAT_VERSION + 1}),
        ),
        (
            "wrong type for order",
            json!({"format_version": 1, "id": "bad", "name": "Bad", "version": "1.0.0", "order": "high"}),
        ),
    ] {
        validate_against_schema(&mod_schema, &bad)
            .expect_err(&format!("JSON Schema must reject {label}: {bad}"));
        let runtime_ok = serde_json::from_value::<ModManifest>(bad.clone())
            .ok()
            .and_then(|m| m.validate(Path::new(MOD_MANIFEST_FILE)).ok())
            .is_some();
        assert!(
            !runtime_ok,
            "runtime loader must also reject {label}: {bad}"
        );
    }

    // ModProfile positive and negative controls.
    let valid_profile = json!({
        "format_version": MOD_PROFILE_FORMAT_VERSION,
        "mods": [{"id": "hard-mode", "name": "Hard Mode", "version": "1.0.0"}]
    });
    validate_against_schema(&profile_schema, &valid_profile).unwrap();
    let bad_profile = json!({
        "format_version": MOD_PROFILE_FORMAT_VERSION,
        "mods": [],
        "extra_key": 1
    });
    validate_against_schema(&profile_schema, &bad_profile).unwrap_err();
    assert!(serde_json::from_value::<ModProfile>(bad_profile).is_err());
}

#[test]
fn patch_schemas_validate_id_keyed_dialogue_shorthands_and_reject_bad_inputs() {
    let enemies_schema = load_published_schema("patches/battle/enemies.schema.json");
    let trees_schema = load_published_schema("patches/dialogue/trees.schema.json");
    let map_schema = load_published_schema("patches/map.schema.json");
    let levels_schema = load_published_schema("patches/battle/levels.schema.json");
    let abilities_schema = load_published_schema("patches/battle/abilities.schema.json");

    // 1. Enemies patch: ID-keyed object form & array form pass; bad types, ranges, and typos fail.
    validate_against_schema(&enemies_schema, &json!({"enemies": {"10": {"hp": 99}}})).unwrap();
    validate_against_schema(
        &enemies_schema,
        &json!({"enemies": [{"id": 10, "hp": 99, "stats": {"attack": 20}}]}),
    )
    .unwrap();

    for (label, bad_patch) in [
        ("unknown field HP", json!({"enemies": {"10": {"HP": 99}}})),
        (
            "nested unknown field ATK",
            json!({"enemies": {"10": {"stats": {"ATK": 20}}}}),
        ),
        (
            "hp exceeds u16::MAX",
            json!({"enemies": {"10": {"hp": 70000}}}),
        ),
        ("negative hp", json!({"enemies": {"10": {"hp": -1}}})),
        ("string hp", json!({"enemies": {"10": {"hp": "99"}}})),
        ("null required hp", json!({"enemies": {"10": {"hp": null}}})),
        ("array patch missing id", json!({"enemies": [{"hp": 99}]})),
        (
            "non-numeric ID key",
            json!({"enemies": {"zoran": {"hp": 99}}}),
        ),
        (
            "zero-padded numeric ID key",
            json!({"enemies": {"010": {"hp": 99}}}),
        ),
    ] {
        validate_against_schema(&enemies_schema, &bad_patch)
            .expect_err(&format!("enemies schema must reject {label}: {bad_patch}"));
    }

    // 1b. Pinned patch `format_version` constants reject unsupported versions across tables.
    let backgrounds_schema = load_published_schema("patches/battle/art/backgrounds.schema.json");
    let travel_schema = load_published_schema("patches/travel.schema.json");
    let sound_index_schema = load_published_schema("patches/sound/index.schema.json");
    validate_against_schema(
        &backgrounds_schema,
        &json!({"format_version": PACK_FORMAT_VERSION}),
    )
    .unwrap();
    validate_against_schema(&backgrounds_schema, &json!({"format_version": 999}))
        .expect_err("backgrounds schema must reject unsupported format_version");
    validate_against_schema(&travel_schema, &json!({"format_version": 2})).unwrap();
    validate_against_schema(&travel_schema, &json!({"format_version": 1}))
        .expect_err("travel schema must reject unsupported format_version");
    validate_against_schema(
        &sound_index_schema,
        &json!({"format_version": crate::sound::SOUND_FORMAT_VERSION}),
    )
    .unwrap();
    validate_against_schema(&sound_index_schema, &json!({"format_version": 99}))
        .expect_err("sound/index schema must reject unsupported format_version");

    // 2. Dialogue patch: replace, 1D lines, 2D lines + keep_portrait pass; invalid forms fail.
    for valid_entry in [
        json!({"replace": [["Thank goodness you're safe!", "Glad to see you in one piece!"]]}),
        json!({"lines": ["Hello!", "Modded world!"]}),
        json!({"lines": [["Page 1 L1", "Page 1 L2"], ["Page 2"]], "keep_portrait": false}),
    ] {
        let doc = json!({"trees": {"34": {"entries": {"0": valid_entry}}}});
        validate_against_schema(&trees_schema, &doc)
            .unwrap_or_else(|e| panic!("valid dialogue patch rejected: {e}"));
    }

    for (label, bad_entry) in [
        (
            "keep_portrait without lines",
            json!({"keep_portrait": false}),
        ),
        (
            "bad keep_portrait type",
            json!({"lines": ["Hi"], "keep_portrait": "nope"}),
        ),
        ("empty lines array", json!({"lines": []})),
        ("malformed replace pair", json!({"replace": [["only-one"]]})),
        (
            "unknown field alongside lines",
            json!({"lines": ["Hi"], "typo": 1}),
        ),
    ] {
        let doc = json!({"trees": {"34": {"entries": {"0": bad_entry}}}});
        validate_against_schema(&trees_schema, &doc)
            .expect_err(&format!("dialogue schema must reject {label}: {doc}"));
    }

    // 3. Map patch: treasure_chests (keyed), map_updates (nullable keyed), dynamic maps, unkeyed arrays.
    validate_against_schema(
        &map_schema,
        &json!({
            "png_over": null,
            "map_update_images": {"maps/010_Piata.png": null},
            "treasure_chests": {"0": {"contents_type": "meseta", "item_id": null, "meseta": 500}},
            "map_updates": null
        }),
    )
    .unwrap();
    validate_against_schema(&map_schema, &json!({"treasures": {"0": {"meseta": 500}}}))
        .expect_err("map schema must reject wrong key `treasures`");
    validate_against_schema(
        &map_schema,
        &json!({
            "collision": {"rows": {"cells": [[1, 1, 8], {"row": 1, "col": 2, "value": 0}]}}
        }),
    )
    .unwrap();
    validate_against_schema(
        &map_schema,
        &json!({"collision": {"rows": {"cells": [[1, 1, 16]]}}}),
    )
    .expect_err("map schema must reject out-of-range collision nibble 16");
    validate_against_schema(
        &map_schema,
        &json!({"collision": {"rows": {"cells": [{"row": 1, "col": 1, "value": 8, "extra": 1}]}}}),
    )
    .expect_err("map schema must reject unknown field in sparse cell object");
    validate_against_schema(
        &map_schema,
        &json!({"map_effects": [{"entry": 1, "decoded": true, "bogus_effect_key": 7}]}),
    )
    .expect_err("map schema must reject unknown key inside unkeyed replacement array");

    // 4. Nested keyed arrays (`battle/levels.json`) and default-field null reset (`battle/abilities.json`).
    validate_against_schema(
        &levels_schema,
        &json!({"characters": {"0": {"levels": {"2": {"hp": 45}}}}}),
    )
    .unwrap();
    validate_against_schema(
        &levels_schema,
        &json!({"characters": {"0": {"levels": {"2": {"HP": 45}}}}}),
    )
    .expect_err("levels schema must reject nested typo HP");
    validate_against_schema(&abilities_schema, &json!({"combos": null})).unwrap();

    // 5. Sprite sheet palette metadata (`palette.cram_line`) is modeled and strictly validated.
    let sprite_schema = load_published_schema("patches/sprite.schema.json");
    validate_against_schema(
        &sprite_schema,
        &json!({"sheets": {"Chaz": {"palette": {"cram_line": 2}}}}),
    )
    .unwrap();
    validate_against_schema(
        &sprite_schema,
        &json!({"sheets": {"Chaz": {"palette": {"cram_line": 4}}}}),
    )
    .expect_err("sprite schema must reject out-of-range cram_line 4");
    validate_against_schema(
        &sprite_schema,
        &json!({"sheets": {"Chaz": {"palette": {"cram_lin": 2}}}}),
    )
    .expect_err("sprite schema must reject typo palette.cram_lin");
}

#[test]
fn modding_guide_examples_validate_against_published_schemas() {
    let markdown =
        fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/MODDING.md"))
            .expect("read docs/MODDING.md");
    let mut blocks = Vec::new();
    let mut in_json = false;
    let mut current = String::new();
    for line in markdown.lines() {
        let trimmed = line.trim();
        if !in_json {
            if trimmed == "```json" {
                in_json = true;
                current.clear();
            }
        } else if trimmed.starts_with("```") {
            blocks.push(current.clone());
            in_json = false;
        } else {
            current.push_str(line);
            current.push('\n');
        }
    }
    assert_eq!(
        blocks.len(),
        5,
        "expected 5 worked JSON examples in MODDING.md"
    );

    let mod_val: Value = serde_json::from_str(&blocks[0]).unwrap();
    let enemy_val: Value = serde_json::from_str(&blocks[1]).unwrap();
    let dialogue_val: Value = serde_json::from_str(&blocks[2]).unwrap();
    let map_val: Value = serde_json::from_str(&blocks[3]).unwrap();
    let formation_val: Value = serde_json::from_str(&blocks[4]).unwrap();

    validate_against_schema(&load_published_schema(MOD_SCHEMA_FILE), &mod_val)
        .expect("MODDING.md mod.json must satisfy mod.schema.json");
    validate_against_schema(
        &load_published_schema("patches/battle/enemies.schema.json"),
        &enemy_val,
    )
    .expect("MODDING.md enemies patch must satisfy enemies.schema.json");
    validate_against_schema(
        &load_published_schema("patches/dialogue/trees.schema.json"),
        &dialogue_val,
    )
    .expect("MODDING.md dialogue patch must satisfy trees.schema.json");
    validate_against_schema(&load_published_schema("patches/map.schema.json"), &map_val)
        .expect("MODDING.md sparse map collision patch must satisfy map.schema.json");
    validate_against_schema(
        &load_published_schema("patches/battle/formations.schema.json"),
        &formation_val,
    )
    .expect("MODDING.md additive formation patch must satisfy formations.schema.json");
}

// ---------------------------------------------------------------------------
// 3. Mod / pack version compatibility policy tests
// ---------------------------------------------------------------------------

#[test]
fn version_compatibility_policy_handles_additive_and_breaking_bumps() {
    // Active policy accepts current versions.
    let current = ModManifest::new("compat-ok", "Compat OK", "1.0.0");
    current.validate(Path::new(MOD_MANIFEST_FILE)).unwrap();

    // Additive pack-format bump (e.g. pack bumped from 2 -> 3 without breaking table layouts):
    // existing mods declaring pack_format_version 2 stay compatible alongside 3.
    let additive_pack = VersionCompatibilityPolicy {
        min: 2,
        current: 3,
        max: 3,
    };
    let mut v2_mod = ModManifest::new("legacy-v2", "Legacy V2", "1.0.0");
    v2_mod.pack_format_version = 2;
    v2_mod
        .validate_with_policies(
            Path::new(MOD_MANIFEST_FILE),
            MOD_FORMAT_COMPATIBILITY,
            additive_pack,
        )
        .expect("additive pack bump must keep min..=max mods compatible");

    let mut v3_mod = ModManifest::new("modern-v3", "Modern V3", "1.0.0");
    v3_mod.pack_format_version = 3;
    v3_mod
        .validate_with_policies(
            Path::new(MOD_MANIFEST_FILE),
            MOD_FORMAT_COMPATIBILITY,
            additive_pack,
        )
        .expect("current pack_format_version must pass");

    // Below minimum (breaking pack change): fails closed naming mod, declared version, range, and upgrade action.
    let mut too_old_pack = ModManifest::new("ancient-v1", "Ancient V1", "1.0.0");
    too_old_pack.pack_format_version = 1;
    let err = too_old_pack
        .validate_with_policies(
            Path::new(MOD_MANIFEST_FILE),
            MOD_FORMAT_COMPATIBILITY,
            additive_pack,
        )
        .expect_err("pack_format_version below min must fail");
    assert_eq!(err.mod_id, "ancient-v1");
    assert_eq!(err.file, MOD_MANIFEST_FILE);
    assert!(err.detail.contains("pack format_version 1"), "{err}");
    assert!(err.detail.contains("2..=3"), "{err}");
    assert!(err.detail.contains("psiv-pack mod check"), "{err}");

    // Above maximum (mod targets a newer PSIV build): fails closed naming mod, declared version, range, and update action.
    let mut too_new_pack = ModManifest::new("future-v4", "Future V4", "1.0.0");
    too_new_pack.pack_format_version = 4;
    let err = too_new_pack
        .validate_with_policies(
            Path::new(MOD_MANIFEST_FILE),
            MOD_FORMAT_COMPATIBILITY,
            additive_pack,
        )
        .expect_err("pack_format_version above max must fail");
    assert_eq!(err.mod_id, "future-v4");
    assert!(err.detail.contains("pack format_version 4"), "{err}");
    assert!(err.detail.contains("2..=3"), "{err}");
    assert!(err.detail.contains("update PSIV"), "{err}");

    // Breaking pack bump (`min` advances to 3): pack_format_version 2 is now rejected.
    let breaking_pack = VersionCompatibilityPolicy {
        min: 3,
        current: 3,
        max: 3,
    };
    let err = v2_mod
        .validate_with_policies(
            Path::new(MOD_MANIFEST_FILE),
            MOD_FORMAT_COMPATIBILITY,
            breaking_pack,
        )
        .expect_err("breaking pack bump must reject pre-break pack_format_version");
    assert!(err.detail.contains("pack format_version 2"), "{err}");
    assert!(err.detail.contains("3..=3"), "{err}");
    assert!(err.detail.contains("breaking pack changes"), "{err}");

    // Mod manifest format_version: additive window (1..=2) accepts 1 and 2, rejects 0 and 3.
    let additive_mod = VersionCompatibilityPolicy {
        min: 1,
        current: 2,
        max: 2,
    };
    let mut m_v1 = ModManifest::new("mod-v1", "Mod V1", "1.0.0");
    m_v1.format_version = 1;
    m_v1.validate_with_policies(
        Path::new(MOD_MANIFEST_FILE),
        additive_mod,
        PACK_FORMAT_COMPATIBILITY,
    )
    .unwrap();
    let mut m_v0 = ModManifest::new("mod-v0", "Mod V0", "1.0.0");
    m_v0.format_version = 0;
    let err0 = m_v0
        .validate_with_policies(
            Path::new(MOD_MANIFEST_FILE),
            additive_mod,
            PACK_FORMAT_COMPATIBILITY,
        )
        .unwrap_err();
    assert!(err0.detail.contains("format_version 0"), "{err0}");
    assert!(err0.detail.contains("1..=2"), "{err0}");
    assert!(err0.detail.contains("psiv-pack mod check"), "{err0}");

    let mut m_v3 = ModManifest::new("mod-v3", "Mod V3", "1.0.0");
    m_v3.format_version = 3;
    let err3 = m_v3
        .validate_with_policies(
            Path::new(MOD_MANIFEST_FILE),
            additive_mod,
            PACK_FORMAT_COMPATIBILITY,
        )
        .unwrap_err();
    assert!(err3.detail.contains("format_version 3"), "{err3}");
    assert!(err3.detail.contains("1..=2"), "{err3}");
    assert!(err3.detail.contains("update PSIV"), "{err3}");

    // End-to-end via load_mod against active build policy.
    let scratch = temp_dir("compat-load");
    let bad_dir = scratch.join("future-mod");
    fs::create_dir_all(&bad_dir).unwrap();
    fs::write(
        bad_dir.join(MOD_MANIFEST_FILE),
        format!(
            r#"{{"format_version":1,"id":"future-mod","name":"Future","version":"1.0.0","pack_format_version":{}}}"#,
            PACK_FORMAT_VERSION + 5
        ),
    )
    .unwrap();
    let err = load_mod(&bad_dir).expect_err("load_mod must reject future pack_format_version");
    assert_eq!(err.mod_id, "future-mod");
    assert_eq!(err.file, MOD_MANIFEST_FILE);
    assert!(
        err.detail
            .contains(&format!("pack format_version {}", PACK_FORMAT_VERSION + 5)),
        "{err}"
    );
    assert!(
        err.detail
            .contains(&PACK_FORMAT_COMPATIBILITY.format_range()),
        "{err}"
    );
    let _ = fs::remove_dir_all(&scratch);
}
