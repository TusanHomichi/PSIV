//! Emission contracts from [retired test_battle_art_pack](https://github.com/TusanHomichi/PSIV/blob/902868b517fd6b8eb0aee15b5dd500623cc7954c/tests/test_battle_art_pack.py) (#551). The image
//! decode tests remain with their owning extractors (#550); this reads the
//! emitted indexes, manifest, files and their relationships.

mod attacks;
mod backgrounds;
mod metadata;
mod tiles;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::OnceLock;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::battle::image_tests::Output;
use crate::battle::tests::retail;
use crate::{json::Json, png};

struct Emission {
    fragment: Value,
    blobs: BTreeMap<String, Vec<u8>>,
    indexes: BTreeMap<String, Value>,
}

fn walk(root: &Path, dir: &Path, blobs: &mut BTreeMap<String, Vec<u8>>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            walk(root, &path, blobs);
        } else {
            let name = path
                .strip_prefix(root)
                .unwrap()
                .iter()
                .map(|s| s.to_str().unwrap())
                .collect::<Vec<_>>()
                .join("/");
            blobs.insert(name, std::fs::read(path).unwrap());
        }
    }
}

fn emit(rom: &[u8], animations: &Value) -> Emission {
    let root = Output::new("battle-art-emission");
    let fragment = Value::from(&super::build(rom, &root.0, &Json::from(animations)).unwrap());
    let mut blobs = BTreeMap::new();
    walk(&root.0, &root.0.join("battle/art"), &mut blobs);
    let indexes = fragment["files"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(key, entry)| {
            let path = entry["file"].as_str().unwrap();
            (key.clone(), serde_json::from_slice(&blobs[path]).unwrap())
        })
        .collect();
    Emission {
        fragment,
        blobs,
        indexes,
    }
}

fn emission() -> Option<&'static Emission> {
    static EMISSION: OnceLock<Option<Emission>> = OnceLock::new();
    EMISSION
        .get_or_init(|| {
            let r = retail()?;
            Some(emit(&r.rom, r.file("enemy_animations")))
        })
        .as_ref()
}

fn array(value: &Value) -> &[Value] {
    value.as_array().unwrap()
}
fn number(value: &Value) -> usize {
    value.as_u64().unwrap() as usize
}
fn hex(value: &Value) -> usize {
    usize::from_str_radix(value.as_str().unwrap().trim_start_matches("0x"), 16).unwrap()
}

fn image<'a>(e: &'a Emission, entry: &Value) -> png::Image<'a> {
    let bytes = &e.blobs[entry["png"].as_str().unwrap()];
    assert_eq!(entry["png_sha256"], png::image_sha256(bytes).unwrap());
    png::decode(bytes).unwrap()
}

/// Walk all index records, not just the exemplar enemies. Each PNG field
/// names a file in this emission. Check the decoded-content hash wherever
/// the schema stores one; Fusion predates those per-frame hash fields and
/// has source-to-image comparisons in its owning module (#550).
fn png_references(e: &Emission, index: &str, value: &Value, seen: &mut BTreeSet<String>) {
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                if key == "png" || key.ends_with("_png") {
                    if key == "initial_png" && value.is_null() {
                        assert_eq!(fields["enabled"], false);
                        assert!(array(&fields["frames"]).is_empty());
                        continue;
                    }
                    let path = value.as_str().unwrap();
                    let bytes = e.blobs.get(path).expect("index names an absent PNG");
                    let digest = png::image_sha256(bytes).unwrap();
                    if let Some(expected) = fields.get(&format!("{key}_sha256")) {
                        assert_eq!(expected, &digest);
                    } else {
                        assert_eq!(index, "fusion", "missing decoded hash for {key}");
                        assert!(path.starts_with("battle/art/fusion/"));
                    }
                    seen.insert(path.to_owned());
                } else {
                    png_references(e, index, value, seen);
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                png_references(e, index, value, seen);
            }
        }
        _ => {}
    }
}

fn assert_inventory(e: &Emission) {
    assert_eq!(e.fragment["directory"], "battle/art");
    let files = e.fragment["files"].as_object().unwrap();
    assert_eq!(files.len(), 8);
    let mut described = BTreeSet::new();
    for (key, entry) in files {
        let path = format!("battle/art/{key}.json");
        assert_eq!(entry["file"], path);
        let bytes = e.blobs.get(&path).expect("manifest names an absent index");
        assert_eq!(entry["sha256"], png::hex(&Sha256::digest(bytes)));
        let value = &e.indexes[key];
        assert_eq!(value["format_version"], crate::battle::FORMAT_VERSION);
        assert!(
            *bytes == (serde_json::to_string_pretty(value).unwrap() + "\n").as_bytes(),
            "index is not canonical newline-terminated JSON"
        );
        described.insert(path);
        let count = match key.as_str() {
            "characters" | "backgrounds" | "enemies" | "enemy_attacks" => &value["count"],
            "enemy_overlays" => &value["piece_count"],
            "command_ui" | "damage_digits" => &value["tile_count"],
            "fusion" => &value["count"],
            _ => unreachable!(),
        };
        assert_eq!(&entry["count"], count, "manifest count for {key}");
        if let Some(directory) = entry["png_directory"].as_str() {
            let images: Vec<_> = e
                .blobs
                .iter()
                .filter(|(p, _)| p.starts_with(&format!("{directory}/")) && p.ends_with(".png"))
                .collect();
            assert_eq!(entry["png_count"], images.len());
            assert_eq!(
                entry["png_bytes"],
                images.iter().map(|(_, b)| b.len()).sum::<usize>()
            );
        }
        png_references(e, key, value, &mut described);
    }
    assert_eq!(described, e.blobs.keys().cloned().collect());
    for (key, expected) in [
        ("enemies", 153),
        ("characters", 43),
        ("backgrounds", 32),
        ("enemy_attacks", 960 * 3),
    ] {
        assert_eq!(files[key]["png_count"], expected);
    }
    assert!(
        number(&files["enemy_overlays"]["png_count"]) > number(&files["enemy_overlays"]["count"])
    );
}

#[test]
fn every_file_hash_png_reference_count_and_version_describes_the_emission() {
    let Some(e) = emission() else { return };
    assert_inventory(e);
}

#[test]
fn emitting_twice_keeps_every_path_and_byte_identical() {
    let Some(e) = emission() else { return };
    let r = retail().unwrap();
    let second = emit(&r.rom, r.file("enemy_animations"));
    assert_eq!(e.fragment, second.fragment);
    assert!(
        e.blobs == second.blobs,
        "second emission differs in paths or bytes"
    );
}

#[test]
fn inventory_rejects_missing_extra_and_misreported_output() {
    let Some(e) = emission() else { return };
    assert_inventory(e);
    for corruption in ["missing", "extra", "count", "hash", "png_hash", "reference"] {
        let mut bad = Emission {
            fragment: e.fragment.clone(),
            blobs: e.blobs.clone(),
            indexes: e.indexes.clone(),
        };
        match corruption {
            "missing" => {
                bad.blobs.remove("battle/art/enemies.json");
            }
            "extra" => {
                bad.blobs.insert("battle/art/stray.png".into(), vec![]);
            }
            "count" => bad.fragment["files"]["enemies"]["png_count"] = 0.into(),
            "hash" => bad.fragment["files"]["enemies"]["sha256"] = "wrong".into(),
            "png_hash" => {
                bad.indexes.get_mut("enemies").unwrap()["enemies"][0]["png_sha256"] = "wrong".into()
            }
            _ => {
                bad.indexes.get_mut("enemies").unwrap()["enemies"][0]["png"] =
                    "battle/art/missing.png".into()
            }
        }
        // Re-serialize changed indexes and refresh the manifest digest so
        // broken PNG links/hashes reach their own relationship assertion.
        if ["png_hash", "reference"].contains(&corruption) {
            let bytes = (serde_json::to_string_pretty(&bad.indexes["enemies"]).unwrap() + "\n")
                .into_bytes();
            bad.fragment["files"]["enemies"]["sha256"] = png::hex(&Sha256::digest(&bytes)).into();
            bad.blobs.insert("battle/art/enemies.json".into(), bytes);
        }
        assert!(
            std::panic::catch_unwind(|| assert_inventory(&bad)).is_err(),
            "{corruption}"
        );
    }
}
