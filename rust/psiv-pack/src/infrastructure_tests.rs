//! Builder infrastructure contracts retained independently of Python parity.
use super::*;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

fn files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            assert!(!entry.file_type().unwrap().is_symlink());
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(
                    path.strip_prefix(root)
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .replace('\\', "/"),
                    std::fs::read(&path).unwrap(),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

/// Check declarations against bytes, rather than comparing two builders that
/// could share the same bad inventory. Every manifest file reference is read.
fn inventory(value: &Value, files: &BTreeMap<String, Vec<u8>>) -> Result<usize, String> {
    let mut count = 0;
    match value {
        Value::Object(object) => {
            for (field, digest_field) in [
                ("file", "sha256"),
                ("path", "sha256"),
                ("json", "json_sha256"),
                ("png", "png_sha256"),
                ("png_over", "png_over_sha256"),
            ] {
                let Some(name) = object.get(field).and_then(Value::as_str) else {
                    continue;
                };
                let bytes = files
                    .get(name)
                    .ok_or_else(|| format!("missing declared file {name}"))?;
                count += 1;
                let digest = if name.ends_with(".png") {
                    png::image_sha256(bytes).map_err(|e| e.0)?
                } else {
                    png::hex(&Sha256::digest(bytes))
                };
                let expected = object.get(digest_field).or_else(|| {
                    (field == "path" && name.ends_with(".png"))
                        .then(|| object.get("png_sha256"))
                        .flatten()
                });
                let expected =
                    expected.ok_or_else(|| format!("missing {digest_field} for {name}"))?;
                if expected != &Value::String(digest) {
                    return Err(format!("wrong {digest_field} for {name}"));
                }
                if let Some(size) = object.get("size_bytes")
                    && size.as_u64() != Some(bytes.len() as u64)
                {
                    return Err(format!("wrong size for {name}"));
                }
            }
            for child in object.values() {
                count += inventory(child, files)?;
            }
        }
        Value::Array(array) => {
            for child in array {
                count += inventory(child, files)?;
            }
        }
        _ => (),
    }
    Ok(count)
}

#[test]
fn inventory_refuses_missing_files_wrong_hashes_and_wrong_sizes() {
    let good = serde_json::json!({"file":"a.json", "sha256":png::hex(&Sha256::digest(b"{}")), "size_bytes":2});
    let bytes = [("a.json".to_owned(), b"{}".to_vec())].into();
    assert_eq!(inventory(&good, &bytes), Ok(1));
    assert!(
        inventory(&good, &BTreeMap::new())
            .unwrap_err()
            .contains("missing")
    );
    let mut bad = good.clone();
    bad.as_object_mut().unwrap().remove("sha256");
    assert!(
        inventory(&bad, &bytes)
            .unwrap_err()
            .contains("missing sha256")
    );
    let mut bad = good.clone();
    bad["sha256"] = "wrong".into();
    assert!(inventory(&bad, &bytes).unwrap_err().contains("sha256"));
    let mut bad = good;
    bad["size_bytes"] = 3.into();
    assert!(inventory(&bad, &bytes).unwrap_err().contains("size"));
}

#[test]
fn whole_pack_manifest_inventory_and_serial_parallel_builds_agree() {
    let Some(rom) = test_support::retail_rom("pack manifest and reproducibility") else {
        return;
    };
    let root = test_support::scratch("infrastructure");
    let serial = root.join("serial");
    let parallel = root.join("parallel");
    build_pack(&rom, &serial, SECTIONS, 1, |_, _, _| {}).unwrap();
    build_pack(&rom, &parallel, SECTIONS, 8, |_, _, _| {}).unwrap();
    let first = files(&serial);
    assert_eq!(
        first.keys().collect::<Vec<_>>(),
        files(&parallel).keys().collect::<Vec<_>>()
    );
    for (name, bytes) in &first {
        assert!(
            bytes == &std::fs::read(parallel.join(name)).unwrap(),
            "nondeterministic {name}"
        );
    }
    let manifest: Value = serde_json::from_slice(&first[MANIFEST_NAME]).unwrap();
    assert_eq!(
        manifest["rom"]["sha256"],
        png::hex(&Sha256::digest(rom.bytes()))
    );
    assert_eq!(manifest["rom"]["size_bytes"], rom.bytes().len());
    assert_eq!(manifest["format_version"], PACK_FORMAT_VERSION);
    assert!(
        inventory(&manifest, &first).unwrap() >= manifest["maps"].as_array().unwrap().len() * 2
    );
    for (name, bytes) in &first {
        let prefix = name.split('/').next().unwrap();
        if name.contains('/') {
            assert!(
                manifest[prefix].is_object() || manifest[prefix].is_array(),
                "uncategorised {name}"
            );
        }
        if name.ends_with(".json") {
            let value: Value = serde_json::from_slice(bytes).unwrap();
            if matches!(prefix, "maps" | "sprites" | "battle" | "dialogue") || !name.contains('/') {
                assert_eq!(value["format_version"], PACK_FORMAT_VERSION, "{name}");
            } else if prefix == "sound" {
                assert_eq!(value["format_version"], 1, "{name}");
            }
            // The presentation index deliberately preserves insertion order
            // (presentation/mod.rs); every other pack JSON is sorted.
            if name != "presentation/panels.json" {
                let canonical = json::pack_bytes(&json::Json::from(&value));
                assert!(bytes == &canonical, "noncanonical {name}");
            }
            assert!(bytes.ends_with(b"\n"), "missing final newline: {name}");
        }
    }
    let mut events = BTreeMap::<String, usize>::new();
    for entry in manifest["maps"].as_array().unwrap() {
        let map: Value = serde_json::from_slice(&first[entry["json"].as_str().unwrap()]).unwrap();
        assert_eq!(map["png_over"], entry["png_over"]);
        if entry["png_over"].is_null() {
            assert!(entry["png_over_sha256"].is_null());
            assert_eq!(entry["priority_tiles"], 0);
        } else {
            assert!(entry["priority_tiles"].as_u64().unwrap() > 0);
        }
        for event in map["events"].as_array().unwrap() {
            *events.entry(event.to_string()).or_default() += 1;
        }
    }
    assert_eq!(
        serde_json::to_value(events).unwrap(),
        manifest["census"]["event_ids"]
    );
    assert_eq!(
        manifest["map_count"],
        manifest["maps"].as_array().unwrap().len()
    );
    assert_eq!(manifest["collision"]["type_space"], 16);
    assert_eq!(
        manifest["collision"]["named_types_are_not_the_valid_set"],
        true
    );
    assert!(manifest["collision"]["type_names"].get("0x7").is_none());
    assert!(manifest["census"]["collision_types"].get("7").is_some());
    assert!(manifest["census"]["npc_facing_bytes"].get("16").is_some());
    assert!(manifest["census"]["dialogue_trees"].get("43").is_some());
    assert!(manifest["census"]["dialogue_trees"].get("0").is_none());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn invalid_manifest_parts_fail_and_failure_never_reports_a_manifest() {
    let root = test_support::scratch("bad-part");
    fn invalid(_: &Rom, _: &Path) -> Result<json::Json, PackError> {
        Ok(json::Json::from(1))
    }
    let sections = [Section {
        name: "bad",
        claims: &[],
        build: invalid,
    }];
    let mut progress = Vec::new();
    let error = build_pack(
        &Rom::for_tests(vec![]),
        &root,
        &sections,
        1,
        |_, _, name| progress.push(name.to_owned()),
    )
    .unwrap_err();
    assert!(error.0.contains("not an object"));
    assert!(!progress.iter().any(|name| name == MANIFEST_STEP));
    assert!(!root.join(MANIFEST_NAME).exists());
}
