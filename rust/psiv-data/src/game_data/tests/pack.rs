//! Manifest-level failures, and the warps that point outside the pack.
//!
//! The manifest and the record set have to describe the same pack, and every warp
//! has to land somewhere the manifest knows about; these are the cases that prove
//! both directions of that are refused.

use super::fixtures::{
    MapJson, SKIPPED_MOTAVIA, assemble_with, expect_validation, manifest_of, manifest_text,
    warp_json,
};
use crate::{DataError, GameData, Manifest, MapId, PACK_FORMAT_VERSION};

#[test]
fn a_format_version_mismatch_is_refused() {
    let piata = MapJson::default();
    let academy = MapJson::academy();
    let manifest: Manifest = serde_json::from_str(&manifest_text(
        PACK_FORMAT_VERSION + 1,
        &[&piata, &academy],
        SKIPPED_MOTAVIA,
        "",
    ))
    .unwrap();
    match GameData::from_parts(manifest, vec![piata.parse(), academy.parse()]) {
        Err(DataError::FormatVersion { found, expected }) => {
            assert_eq!(
                (found, expected),
                (PACK_FORMAT_VERSION + 1, PACK_FORMAT_VERSION)
            );
        }
        other => panic!("expected a FormatVersion error, got {other:?}"),
    }
}

#[test]
fn a_map_record_from_a_different_pack_version_is_refused() {
    // Each map file carries its own `format_version`, so a stale file left in a
    // rebuilt pack directory is caught even when the manifest looks current.
    let stale = MapJson {
        version: PACK_FORMAT_VERSION + 1,
        ..Default::default()
    };
    match assemble_with(&stale) {
        Err(DataError::FormatVersion { found, .. }) => {
            assert_eq!(found, PACK_FORMAT_VERSION + 1);
        }
        other => panic!("expected a FormatVersion error, got {other:?}"),
    }
}

#[test]
fn a_map_listed_twice_in_the_manifest_is_refused() {
    let piata = MapJson::default();
    let manifest: Manifest = serde_json::from_str(&manifest_text(
        PACK_FORMAT_VERSION,
        &[&piata, &piata],
        SKIPPED_MOTAVIA,
        "",
    ))
    .unwrap();
    match GameData::from_parts(manifest, vec![piata.parse()]) {
        Err(DataError::DuplicateMapId { id, detail }) => {
            assert_eq!(id, MapId(0x010));
            assert!(detail.contains("listed twice"), "{detail}");
        }
        other => panic!("expected a DuplicateMapId error, got {other:?}"),
    }
}

#[test]
fn a_map_both_packed_and_skipped_is_refused() {
    let piata = MapJson::default();
    let academy = MapJson::academy();
    let manifest: Manifest = serde_json::from_str(&manifest_text(
        PACK_FORMAT_VERSION,
        &[&piata, &academy],
        r#"{"id": 16, "symbol": "Piata", "reason": "contradiction"}"#,
        "",
    ))
    .unwrap();
    match GameData::from_parts(manifest, vec![piata.parse(), academy.parse()]) {
        Err(DataError::DuplicateMapId { id, detail }) => {
            assert_eq!(id, MapId(0x010));
            assert!(detail.contains("skipped"), "{detail}");
        }
        other => panic!("expected a DuplicateMapId error, got {other:?}"),
    }
}

#[test]
fn two_records_claiming_the_same_id_are_refused() {
    let piata = MapJson::default();
    let manifest = manifest_of(&[&piata]);
    match GameData::from_parts(manifest, vec![piata.parse(), piata.parse()]) {
        Err(DataError::DuplicateMapId { id, detail }) => {
            assert_eq!(id, MapId(0x010));
            assert!(detail.contains("two map records"), "{detail}");
        }
        other => panic!("expected a DuplicateMapId error, got {other:?}"),
    }
}

#[test]
fn the_manifest_and_the_record_set_must_describe_the_same_pack() {
    let piata = MapJson::default();
    let academy = MapJson::academy();

    // Listed but not loaded.
    let manifest = manifest_of(&[&piata, &academy]);
    expect_validation(
        GameData::from_parts(manifest, vec![piata.parse()]),
        "manifest.maps",
        "no record was loaded",
    );

    // Loaded but not listed.
    let manifest = manifest_of(&[&piata]);
    expect_validation(
        GameData::from_parts(manifest, vec![piata.parse(), academy.parse()]),
        "manifest.maps",
        "does not list",
    );
}

#[test]
fn a_warp_to_a_map_nobody_has_heard_of_is_refused() {
    let map = MapJson {
        warps: warp_json(
            r#""rect": {"x": 1, "y": 1, "width": 1, "height": 1}"#,
            r#""target": {"id": 291, "id_hex": "0x123", "symbol": "SomewhereElse"}"#,
            r#""destination": {"x_cell": 2, "y_cell": 3}"#,
        ),
        ..Default::default()
    };
    let text = expect_validation(
        assemble_with(&map),
        "warps[0].target",
        "neither packed nor listed",
    );
    assert!(
        text.contains("0x123"),
        "the target id should be hex: {text}"
    );
}

#[test]
fn a_destination_outside_the_target_map_is_refused() {
    let map = MapJson {
        warps: warp_json(
            r#""rect": {"x": 1, "y": 1, "width": 1, "height": 1}"#,
            r#""target": {"id": 17, "id_hex": "0x011", "symbol": "PiataAcademy"}"#,
            r#""destination": {"x_cell": 2, "y_cell": 8}"#,
        ),
        ..Default::default()
    };
    let text = expect_validation(
        assemble_with(&map),
        "warps[0].destination",
        "outside map 0x011",
    );
    assert!(text.contains("PiataAcademy"), "{text}");
}
