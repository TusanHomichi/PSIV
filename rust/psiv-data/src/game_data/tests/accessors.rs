//! Happy path and accessors: a pack that loads, the accessors it answers with,
//! and the two dead-data cases a valid pack may still contain.

use super::fixtures::{
    DIGEST, MapJson, SKIPPED_MOTAVIA, assemble, assemble_with, manifest_text, warp_json,
};
use crate::collision::{CollisionType, Plane};
use crate::map::{Direction, TransitionTable};
use crate::{CellPos, GameData, Manifest, MapId, PACK_FORMAT_VERSION};

#[test]
fn a_well_formed_pack_loads() {
    let data = assemble_with(&MapJson::default()).expect("the default fixture pack is valid");

    assert_eq!(data.len(), 2);
    assert!(!data.is_empty());
    assert!(data.contains(MapId(0x010)));
    assert!(!data.contains(MapId(0x000)));
    assert_eq!(
        data.skip_reason(MapId(0x000)),
        Some("paged layout format not decoded")
    );
    assert_eq!(data.skip_reason(MapId(0x010)), None);
    assert!(data.knows(MapId(0x000)), "skipped maps are still known");
    assert!(!data.knows(MapId(0x123)));
    assert_eq!(data.manifest().rom.sha256.as_str(), DIGEST);

    let piata = data.map(MapId(0x010)).expect("Piata is packed");
    assert_eq!(piata.label(), "Piata");
    assert_eq!(piata.music.id, 132);
    assert!(!piata.flags.allows_town_teleport());
    assert!(!piata.flags.rolls_random_battles());
    assert_eq!(piata.dialogue_tree, 2);
    assert_eq!(piata.collision.plane, Plane::Bg);
    assert_eq!(piata.npcs[0].dialogue_id, 64);
    assert_eq!(piata.npcs[0].facing.direction(), Some(Direction::Down));
    assert_eq!(piata.treasure_chests[0].item_id, Some(125));
    assert_eq!(piata.warps[0].table, TransitionTable::MapChange);
    assert_eq!(piata.warps[0].facing.direction(), Some(Direction::Up));
    assert_eq!(piata.warps[0].trigger, "map_change_tile");
}

#[test]
fn maps_iterate_in_ascending_id_order() {
    let academy = MapJson::academy();
    let piata = MapJson::default();
    // Manifest order is deliberately the reverse of id order.
    let data = assemble(&[&academy, &piata]).expect("valid");
    let ids: Vec<MapId> = data.map_ids().collect();
    assert_eq!(ids, vec![MapId(0x010), MapId(0x011)]);
    let labels: Vec<String> = data.maps().map(|(_, m)| m.label()).collect();
    assert_eq!(labels, vec!["Piata", "PiataAcademy"]);
}

#[test]
fn collision_and_warp_lookups_agree_with_the_grid() {
    let data = assemble_with(&MapJson::default()).unwrap();
    let piata = data.map(MapId(0x010)).unwrap();

    let door = CellPos::new(1, 1);
    assert_eq!(piata.collision_at(door), Some(CollisionType::MapChange));
    assert!(
        !piata.blocks_at(door),
        "a map_change cell is walked onto, not blocked"
    );
    assert!(piata.blocks_at(CellPos::new(1, 2)), "0x8 is solid");
    assert!(
        piata.blocks_at(CellPos::new(9, 9)),
        "outside the map blocks"
    );

    let hits: Vec<MapId> = piata.warps_at(door).map(|w| w.target.id).collect();
    assert_eq!(hits, vec![MapId(0x011)]);
    assert_eq!(piata.warps_at(CellPos::new(0, 0)).count(), 0);
    assert!(piata.contains(door));
    assert!(!piata.contains(CellPos::new(4, 0)));
}

#[test]
fn a_warp_may_target_a_map_the_manifest_declares_unpacked() {
    let map = MapJson {
        warps: warp_json(
            r#""rect": {"x": 0, "y": 0, "width": 4, "height": 1}"#,
            r#""target": {"id": 0, "id_hex": "0x000", "symbol": "Motavia"}"#,
            // Nowhere near this map's 4x4 bounds, which is the point: an
            // unpacked target's size is unknown and must not be guessed at.
            r#""destination": {"x_cell": 46, "y_cell": 142}"#,
        ),
        ..Default::default()
    };
    assemble_with(&map).expect("a skipped map is a known target");

    // The other list of known-but-unpacked maps works the same way.
    let piata = MapJson {
        warps: warp_json(
            r#""rect": {"x": 0, "y": 0, "width": 4, "height": 1}"#,
            r#""target": {"id": 24, "id_hex": "0x018", "symbol": "PiataDorm"}"#,
            r#""destination": {"x_cell": 45, "y_cell": 30}"#,
        ),
        ..Default::default()
    };
    let academy = MapJson::academy();
    let manifest: Manifest = serde_json::from_str(&manifest_text(
        PACK_FORMAT_VERSION,
        &[&piata, &academy],
        SKIPPED_MOTAVIA,
        r#"{"id": 24, "id_hex": "0x018", "symbol": "PiataDorm"}"#,
    ))
    .unwrap();
    GameData::from_parts(manifest, vec![piata.parse(), academy.parse()])
        .expect("an unpacked warp target is a known target");
}

#[test]
fn a_warp_with_no_trigger_rectangle_is_accepted() {
    // XYRange 0 is `Null` and never fires; a clipped half-plane can also fall
    // entirely off the map. Dead data, not a defect.
    let map = MapJson {
        warps: warp_json(
            r#""rect": null"#,
            r#""target": {"id": 17, "id_hex": "0x011", "symbol": "PiataAcademy"}"#,
            r#""destination": {"x_cell": 2, "y_cell": 3}"#,
        ),
        ..Default::default()
    };
    let data = assemble_with(&map).expect("a rectangle-less warp is not an error");
    let piata = data.map(MapId(0x010)).unwrap();
    assert!(piata.warps[0].rect.is_none());
    assert_eq!(piata.warps_at(CellPos::new(1, 1)).count(), 0);
}

#[test]
fn a_null_dungeon_teleport_index_is_accepted() {
    // Bit 7 preserves the live exit index; nine retail maps set it.
    let map = MapJson {
        flags: r#"{"poison": 1, "random_battles": 1, "town_teleport": 1,
                   "dungeon_teleport_index": null}"#
            .to_string(),
        ..Default::default()
    };
    let data = assemble_with(&map).expect("a null dungeon teleport index is normal");
    assert_eq!(
        data.map(MapId(0x010)).unwrap().flags.dungeon_teleport_index,
        None
    );
}
