use super::pack_load_problem;
use psiv_data::{DataError, GameData};
use std::path::{Path, PathBuf};

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("psiv-pack-load-{name}-{}", std::process::id()));
        std::fs::create_dir(&path).expect("fresh scratch directory");
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).expect("cleanup");
    }
}

fn retained_error(pack: &Path) -> DataError {
    let error = GameData::load(pack).expect_err("defective pack");
    let original = error.to_string();
    // Both the Godot log and setup screen receive this unmodified diagnostic.
    assert_eq!(pack_load_problem(pack, &error), Some(original));
    error
}

#[test]
fn absent_pack_is_quiet() {
    let scratch = Scratch::new("absent");
    let pack = scratch.0.join("runtime-pack");
    let error = GameData::load(&pack).unwrap_err();
    assert!(matches!(&error, DataError::Io { source, .. }
        if source.kind() == std::io::ErrorKind::NotFound));
    assert_eq!(pack_load_problem(&pack, &error), None);
}

#[test]
fn existing_pack_without_manifest_reports_the_missing_file() {
    let scratch = Scratch::new("manifest-missing");
    let error = retained_error(&scratch.0);
    assert_eq!(
        error.path(),
        Some(scratch.0.join("manifest.json").as_path())
    );
}

#[test]
fn malformed_manifest_reports_the_parse_error() {
    let scratch = Scratch::new("malformed");
    std::fs::write(scratch.0.join("manifest.json"), "{").unwrap();
    assert!(matches!(retained_error(&scratch.0), DataError::Json { .. }));
}

#[test]
fn missing_pack_content_reports_the_missing_file() {
    let scratch = Scratch::new("content-missing");
    let manifest = serde_json::json!({
        "format_version": psiv_data::PACK_FORMAT_VERSION,
        "rom": {"sha256": "0".repeat(64), "size_bytes": 3145728},
        "maps": [{"id": 16, "json": "maps/missing.json", "png": "maps/missing.png",
                  "width_cells": 1, "height_cells": 1, "width_pixels": 16, "height_pixels": 16}],
        "skipped": [],
        "unpacked_warp_targets": []
    });
    std::fs::write(scratch.0.join("manifest.json"), manifest.to_string()).unwrap();
    let error = retained_error(&scratch.0);
    assert_eq!(
        error.path(),
        Some(scratch.0.join("maps/missing.json").as_path())
    );
    assert!(matches!(error, DataError::Io { source, .. }
        if source.kind() == std::io::ErrorKind::NotFound));
}

#[cfg(unix)]
#[test]
fn permission_probe_failure_retains_the_original_error() {
    use std::os::unix::fs::PermissionsExt;

    let scratch = Scratch::new("permission");
    let parent = scratch.0.join("blocked");
    let pack = parent.join("runtime-pack");
    std::fs::create_dir_all(&pack).unwrap();
    std::fs::write(pack.join("manifest.json"), "{}").unwrap();
    std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o000)).unwrap();
    let probe = std::fs::symlink_metadata(&pack);
    let error = GameData::load(&pack).unwrap_err();
    let problem = pack_load_problem(&pack, &error);
    // Restore traversal before assertions so a failing test can still clean up.
    std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        probe.unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied
    );
    assert!(matches!(&error, DataError::Io { source, .. }
        if source.kind() == std::io::ErrorKind::PermissionDenied));
    assert_eq!(problem, Some(error.to_string()));
}

#[cfg(unix)]
#[test]
fn symlink_loop_probe_failure_retains_the_original_error() {
    let scratch = Scratch::new("loop");
    let parent = scratch.0.join("loop");
    std::os::unix::fs::symlink("loop", &parent).unwrap();
    let pack = parent.join("runtime-pack");
    let probe = std::fs::symlink_metadata(&pack).unwrap_err();
    let error = retained_error(&pack);
    assert!(matches!(error, DataError::Io { source, .. }
        if source.raw_os_error() == probe.raw_os_error()));
}

#[cfg(unix)]
#[test]
fn dangling_pack_link_reports_the_original_error() {
    let scratch = Scratch::new("dangling");
    let pack = scratch.0.join("runtime-pack");
    std::os::unix::fs::symlink("gone", &pack).unwrap();
    retained_error(&pack);
}
