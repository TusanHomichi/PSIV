//! Dialogue extraction contracts from the Python suite (#557).
use super::*;
use crate::test_support::{retail_rom, scratch};
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::OnceLock};

pub(crate) struct Emission {
    pub rom: Rom,
    pub fragment: Value,
    pub files: BTreeMap<String, Vec<u8>>,
}
impl Emission {
    pub fn document(&self, name: &str) -> Value {
        serde_json::from_slice(&self.files[&format!("dialogue/{name}.json")]).unwrap()
    }
}
pub(crate) fn emission() -> Option<&'static Emission> {
    static EMISSION: OnceLock<Option<Emission>> = OnceLock::new();
    EMISSION
        .get_or_init(|| {
            let rom = retail_rom("dialogue extraction")?;
            let root = scratch("dialogue");
            let fragment = Value::from(&build(&rom, &root).unwrap())["dialogue"].clone();
            fn collect(root: &Path, at: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
                for entry in std::fs::read_dir(at).unwrap() {
                    let path = entry.unwrap().path();
                    if path.is_dir() {
                        collect(root, &path, files);
                    } else {
                        files.insert(
                            path.strip_prefix(root)
                                .unwrap()
                                .to_str()
                                .unwrap()
                                .replace('\\', "/"),
                            std::fs::read(path).unwrap(),
                        );
                    }
                }
            }
            let mut files = BTreeMap::new();
            collect(&root, &root, &mut files);
            std::fs::remove_dir_all(root).unwrap();
            Some(Emission {
                rom,
                fragment,
                files,
            })
        })
        .as_ref()
}

fn check_references(fragment: &Value, files: &BTreeMap<String, Vec<u8>>) -> Result<(), String> {
    for (section, path_key, digest_key) in [
        ("trees", "path", "sha256"),
        ("font", "path", "sha256"),
        ("font", "png", "png_sha256"),
        ("chrome", "path", "sha256"),
        ("chrome", "png", "png_sha256"),
        ("menu_font", "path", "png_sha256"),
        ("portraits", "path", "sha256"),
    ] {
        let path = fragment[section][path_key].as_str().unwrap();
        let bytes = files.get(path).ok_or_else(|| format!("missing {path}"))?;
        let digest = if path.ends_with(".png") {
            crate::png::image_sha256(bytes).map_err(|e| e.to_string())?
        } else {
            sha256(bytes)
        };
        if fragment[section][digest_key] != digest {
            return Err(format!("digest {path}"));
        }
    }
    let arrow = &fragment["chrome"]["scroll_arrow"];
    let path = arrow["png"].as_str().unwrap();
    let bytes = files.get(path).ok_or_else(|| format!("missing {path}"))?;
    if arrow["png_sha256"] != crate::png::image_sha256(bytes).unwrap() {
        return Err(format!("digest {path}"));
    }
    Ok(())
}

#[test]
fn emitted_references_versions_signatures_and_rebuild_are_consistent() {
    let Some(e) = emission() else { return };
    check_references(&e.fragment, &e.files).unwrap();
    assert_eq!(e.files.len(), 47);
    assert_eq!(u32::from(FORMAT_VERSION), crate::PACK_FORMAT_VERSION);
    for name in ["trees", "font", "window", "portraits"] {
        assert_eq!(e.document(name)["format_version"], FORMAT_VERSION);
    }
    assert_eq!(
        e.fragment["signatures"],
        Value::from(&Json::from(check_signatures(e.rom.bytes()).unwrap()))
    );
    let root = scratch("dialogue-repeat");
    assert_eq!(
        Value::from(&build(&e.rom, &root).unwrap())["dialogue"],
        e.fragment
    );
    for (path, bytes) in &e.files {
        assert_eq!(&std::fs::read(root.join(path)).unwrap(), bytes);
    }
    std::fs::remove_dir_all(root).unwrap();
    let mut bad = e.fragment.clone();
    bad["trees"]["path"] = json!("dialogue/missing.json");
    assert!(
        check_references(&bad, &e.files)
            .unwrap_err()
            .contains("missing")
    );
    bad = e.fragment.clone();
    bad["font"]["sha256"] = json!("incorrect");
    assert!(
        check_references(&bad, &e.files)
            .unwrap_err()
            .contains("digest")
    );
}

#[test]
fn every_routine_and_palette_mirror_refuses_corruption() {
    let Some(e) = emission() else { return };
    for (_, at, _, _, _) in SIGNATURES {
        let mut corrupt = e.rom.bytes().to_vec();
        corrupt[at] ^= 1;
        assert!(check_signatures(&corrupt).is_err());
    }
    let p = palette(e.rom.bytes()).unwrap();
    assert_eq!(p.len(), 16);
    assert_eq!(p[14], [0, 32, 98]);
    assert_eq!(p[15], [238, 238, 238]);
    let mut corrupt = e.rom.bytes().to_vec();
    corrupt[PAL_INIT_LINE_3] ^= 1;
    assert!(palette(&corrupt).is_err());
}
