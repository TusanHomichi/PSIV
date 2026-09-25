//! The record's serde shape: what is required, what is surplus, what round-trips.

use super::fixtures::MapJson;
use crate::MapId;
use crate::collision::CollisionType;
use crate::map::{CellRect, MapRecord};

#[test]
fn a_missing_section_is_an_error_but_a_surplus_one_is_not() {
    let bare = r#"{"format_version": 1, "id": 16, "symbol": "Piata"}"#;
    assert!(serde_json::from_str::<MapRecord>(bare).is_err());

    // The pack already carries fields this crate ignores (`id_hex`,
    // `record_offset`, `item_symbol`, ...); one more must not break a load.
    let with_extra =
        MapJson::default()
            .text()
            .replacen('{', r#"{"encounters": {"mode": "none"}, "#, 1);
    let record: MapRecord =
        serde_json::from_str(&with_extra).expect("unknown fields are ignored, not denied");
    assert_eq!(record.id, MapId(0x010));
}

#[test]
fn a_record_round_trips_through_json() {
    let original = MapJson::default().parse();
    let text = serde_json::to_string(&original).unwrap();
    let back: MapRecord = serde_json::from_str(&text).unwrap();
    assert_eq!(original, back);
    assert_eq!(
        back.collision.grid.rows().next().unwrap(),
        [CollisionType::Normal; 4]
    );
    assert_eq!(
        back.warps[0].rect,
        Some(CellRect {
            x: 1,
            y: 1,
            width: 1,
            height: 1
        })
    );
}
