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
        self.write("sprites/party.json", &empty);
        self.write(
            "sprites/npcs.json",
            &empty.replace("field_party", "field_npcs"),
        );
    }

    fn load(&self) -> Result<GameData, DataError> {
        GameData::load(&self.dir)
    }
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
