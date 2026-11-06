//! `GameData::load`, against a real directory.
//!
//! The unit cases above go through `from_parts`; these write a pack to a temporary
//! directory and read it back, which is what covers the file reading and the
//! format-version check that runs before any map is opened.

use super::fixtures::{DIGEST, MapJson, SKIPPED_MOTAVIA, manifest_text};
use crate::error::DataError;
use crate::{GameData, MapId, PACK_FORMAT_VERSION};

/// A throwaway pack directory that deletes itself.
struct TempPack {
    dir: std::path::PathBuf,
}

impl TempPack {
    fn new(name: &str) -> TempPack {
        let dir =
            std::env::temp_dir().join(format!("psiv-data-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("maps")).expect("create the temp pack directory");
        TempPack { dir }
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.dir.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create temp pack dirs");
        }
        std::fs::write(path, contents).expect("write a temp pack file");
    }

    /// Write a complete, valid pack.
    fn populate(&self, version: u32) {
        let piata = MapJson::default();
        let academy = MapJson::academy();
        self.write(
            "manifest.json",
            &manifest_text(version, &[&piata, &academy], SKIPPED_MOTAVIA, ""),
        );
        self.write(&piata.json_name(), &piata.text());
        self.write(&academy.json_name(), &academy.text());
        // Pack format 1 always carries the sprite index files; a minimal pair
        // keeps the synthetic pack loadable without dragging art into tests.
        let empty = format!(
            r#"{{"format_version": {version}, "kind": "field_party", "sheet_count": 0, "sheets": []}}"#
        );
        let line = vec![[0u8, 0, 0]; 16];
        let rows = vec![[1u8; 8]; 8];
        let money = vec![1u8; 8];
        self.write(
            "guild/guild.json",
            &serde_json::json!({
                "format_version": 1, "kind": "guild", "rom_sha256": DIGEST,
                "window": {"id": 11, "width": 4, "height": 4, "x": 1, "y": 1},
                "names": ["a", "b", "c", "d", "e", "f", "g", "h"], "pending": "p",
                "rows": rows, "money": money,
                "font_line": line, "done_line": line, "closed_line": line,
            })
            .to_string(),
        );
        let row = serde_json::json!({"x": 16, "y": 32, "character": 2, "first_line": 1, "picked_flag": 3});
        self.write(
            "fifth/fifth.json",
            &serde_json::json!({
                "format_version": 1, "kind": "fifth", "rom_sha256": DIGEST,
                "rows": [row, row, row, row, row], "records": [2, 4, 6, 8, 9],
            })
            .to_string(),
        );
        self.write_fusion(version);
        self.write("sprites/party.json", &empty);
        self.write(
            "sprites/npcs.json",
            &empty.replace("field_party", "field_npcs"),
        );
    }

    /// The Fusion frames' index and five placeholder PNGs (#64).
    fn write_fusion(&self, version: u32) {
        let frames: Vec<_> = (0..crate::FUSION_FRAMES)
            .map(|index| {
                let png = format!("battle/art/fusion/{index}.png");
                self.write(&png, "png");
                serde_json::json!({"index": index, "png": png, "origin": [-24, -32], "size": [48, 32]})
            })
            .collect();
        self.write(
            crate::FUSION_ART_PATH,
            &serde_json::json!({"format_version": version, "kind": "battle_fusion_art",
                                "palette_enemy_id": 34, "frames": frames})
            .to_string(),
        );
    }

    fn load(&self) -> Result<GameData, DataError> {
        GameData::load(&self.dir)
    }
}

/// #64: a pack from before the Fusion's frames is refused with a rebuild
/// message, so no battle starts with its art missing.
#[test]
fn a_pack_without_the_fusion_frames_is_refused_for_a_rebuild() {
    let pack = TempPack::new("no-fusion");
    pack.populate(PACK_FORMAT_VERSION);
    std::fs::remove_file(pack.dir.join(crate::FUSION_ART_PATH)).expect("remove the frames");
    match pack.load() {
        Err(DataError::ManifestMismatch {
            field, manifest, ..
        }) => assert_eq!(
            (field, manifest.as_str()),
            ("fusion", "absent; rebuild the pack")
        ),
        other => panic!("expected a rebuild refusal, got {other:?}"),
    }
}

#[test]
fn fusion_frames_out_of_order_or_without_their_pngs_are_refused() {
    let pack = TempPack::new("bad-fusion");
    pack.populate(PACK_FORMAT_VERSION);
    std::fs::remove_file(pack.dir.join("battle/art/fusion/3.png")).expect("remove a frame");
    assert!(matches!(
        pack.load(),
        Err(DataError::ManifestMismatch {
            field: "fusion",
            ..
        })
    ));
    let pack = TempPack::new("bad-fusion-order");
    pack.populate(PACK_FORMAT_VERSION);
    let path = pack.dir.join(crate::FUSION_ART_PATH);
    let text = std::fs::read_to_string(&path)
        .unwrap()
        .replacen("\"index\":1", "\"index\":2", 1);
    std::fs::write(&path, text).unwrap();
    assert!(matches!(
        pack.load(),
        Err(DataError::ManifestMismatch {
            field: "fusion",
            ..
        })
    ));
}

impl Drop for TempPack {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn load_reads_a_directory_of_files() {
    let pack = TempPack::new("happy");
    pack.populate(PACK_FORMAT_VERSION);
    let data = pack.load().expect("a valid pack directory loads");
    assert_eq!(data.len(), 2);
    assert_eq!(data.map(MapId(0x011)).unwrap().label(), "PiataAcademy");
    // The fixture writes no `dialogue/`, so this pack has none: the runtime
    // refuses to build from it rather than opening no window.
    assert!(data.dialogue().is_none());
}

/// An older pack has no job board: it is refused, naming what to do, so the
/// setup screen asks for a rebuild instead of the game opening an invisible
/// menu at the guild's counter.
#[test]
fn a_pack_without_the_guild_is_refused() {
    let pack = TempPack::new("no-guild");
    pack.populate(PACK_FORMAT_VERSION);
    let manifest = std::fs::read_to_string(pack.dir.join("manifest.json")).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    value.as_object_mut().unwrap().remove("guild");
    pack.write("manifest.json", &value.to_string());
    let error = pack.load().expect_err("no guild section");
    assert!(error.to_string().contains("rebuild the pack"), "{error}");
}

#[test]
fn a_pack_without_the_fifth_table_is_refused() {
    let pack = TempPack::new("no-fifth");
    pack.populate(PACK_FORMAT_VERSION);
    let manifest = std::fs::read_to_string(pack.dir.join("manifest.json")).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    value.as_object_mut().unwrap().remove("fifth");
    pack.write("manifest.json", &value.to_string());
    let error = pack.load().expect_err("no fifth section");
    assert!(error.to_string().contains("rebuild the pack"), "{error}");
}

#[test]
fn load_fails_when_the_manifest_is_missing() {
    let pack = TempPack::new("no-manifest");
    match pack.load() {
        Err(err @ DataError::Io { .. }) => {
            assert!(err.to_string().contains("manifest.json"), "{err}");
        }
        other => panic!("expected an Io error, got {other:?}"),
    }
}

#[test]
fn load_fails_when_a_listed_map_file_is_missing() {
    let pack = TempPack::new("missing-map");
    pack.populate(PACK_FORMAT_VERSION);
    std::fs::remove_file(pack.dir.join(MapJson::academy().json_name())).unwrap();
    match pack.load() {
        Err(err @ DataError::Io { .. }) => {
            assert!(err.to_string().contains("011_PiataAcademy.json"), "{err}");
        }
        other => panic!("expected an Io error, got {other:?}"),
    }
}

#[test]
fn load_checks_the_format_version_before_reading_any_map() {
    let pack = TempPack::new("bad-version");
    pack.populate(PACK_FORMAT_VERSION + 7);
    // Every map file is deleted, so reaching a map read at all would be an Io
    // error and this test would fail.
    std::fs::remove_dir_all(pack.dir.join("maps")).unwrap();
    match pack.load() {
        Err(DataError::FormatVersion { found, .. }) => {
            assert_eq!(found, PACK_FORMAT_VERSION + 7);
        }
        other => panic!("expected a FormatVersion error, got {other:?}"),
    }
}

#[test]
fn load_fails_on_malformed_json() {
    let pack = TempPack::new("malformed");
    pack.populate(PACK_FORMAT_VERSION);
    pack.write(&MapJson::default().json_name(), "{ this is not json");
    match pack.load() {
        Err(err @ DataError::Json { .. }) => {
            assert!(err.to_string().contains("010_Piata.json"), "{err}");
        }
        other => panic!("expected a Json error, got {other:?}"),
    }
}

#[test]
fn load_catches_a_map_file_that_disagrees_with_its_manifest_entry() {
    let pack = TempPack::new("id-drift");
    pack.populate(PACK_FORMAT_VERSION);
    let impostor = MapJson {
        id: 0x099,
        ..Default::default()
    };
    pack.write(&MapJson::default().json_name(), &impostor.text());
    match pack.load() {
        Err(DataError::ManifestMismatch { field, .. }) => assert_eq!(field, "id"),
        other => panic!("expected a ManifestMismatch error, got {other:?}"),
    }

    let renamed = MapJson {
        symbol: "NotPiata",
        ..Default::default()
    };
    pack.write(&MapJson::default().json_name(), &renamed.text());
    match pack.load() {
        Err(DataError::ManifestMismatch { field, .. }) => assert_eq!(field, "symbol"),
        other => panic!("expected a ManifestMismatch error, got {other:?}"),
    }
}
