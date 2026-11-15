//! Map retirement consumer contracts (#552), through the actual loader.

use super::{PIATA, require_pack};
use psiv_data::{CellPos, ContentsType, GameData, MapId, TransitionTable};

#[test]
fn map_records_keep_their_typed_coordinates_contents_and_ordered_events() {
    let Some(dir) = require_pack() else { return };
    let data = GameData::load(&dir).unwrap();
    if let Some(p) = data.map(PIATA) {
        assert_eq!(
            (p.dimensions.width_cells, p.dimensions.height_cells),
            (64, 64)
        );
        assert_eq!(p.events, [0]);
        let n = &p.npcs[0];
        assert_eq!((n.x_pixels, n.y_pixels), (480, 736));
        assert_eq!(n.pos(), CellPos::new(30, 47));
        assert_eq!(n.dialogue_id, 83);
        let door = p.warps.iter().find(|w| w.target.id == MapId(0x11)).unwrap();
        assert_eq!(door.table, TransitionTable::MapChange);
        assert_eq!(door.destination.pos(), CellPos::new(31, 19));
    }
    if let Some(cave) = data
        .maps()
        .map(|(_, m)| m)
        .find(|m| m.symbol.as_deref() == Some("IslandCave"))
    {
        assert!(cave.flags.poisons());
        assert!(cave.flags.rolls_random_battles());
        assert_eq!(cave.treasure_chests.len(), 1);
        let c = &cave.treasure_chests[0];
        assert_eq!(c.contents_type, ContentsType::Item);
        assert_eq!(c.item_id, Some(133));
        assert_eq!(c.meseta, None);
        assert_eq!(c.pos(), CellPos::new(82, 73));
    }
    for (id, m) in data.maps() {
        let entry = data.manifest().maps.iter().find(|e| e.id == id).unwrap();
        let raw: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.join(&entry.json)).unwrap()).unwrap();
        assert_eq!(serde_json::to_value(&m.events).unwrap(), raw["events"]);
    }
}

#[test]
fn damaged_real_map_fields_are_rejected_by_the_consumer() {
    let Some(dir) = require_pack() else { return };
    let data = GameData::load(&dir).unwrap();
    if !data.contains(PIATA) {
        return;
    }
    let chest_available = data.maps().any(|(_, m)| !m.treasure_chests.is_empty());
    assert_eq!(
        assert_damaged_fields(&data),
        4 + usize::from(chest_available)
    );
}

#[test]
fn a_chestless_subset_still_checks_every_available_field() {
    let Some(dir) = require_pack() else { return };
    let data = GameData::load(&dir).unwrap();
    let Some(p) = data.map(PIATA) else { return };
    assert!(p.treasure_chests.is_empty());
    let mut manifest = data.manifest().clone();
    let entries = std::mem::take(&mut manifest.maps);
    for entry in entries {
        if entry.id == PIATA {
            manifest.maps.push(entry);
        } else {
            manifest.unpacked_warp_targets.push(psiv_data::MapRef {
                id: entry.id,
                symbol: entry.symbol,
            });
        }
    }
    let subset = GameData::from_parts(manifest, vec![p.clone()]).unwrap();
    assert_eq!(subset.len(), 1);
    assert_eq!(assert_damaged_fields(&subset), 4);
}

fn assert_damaged_fields(data: &GameData) -> usize {
    let p = data.map(PIATA).unwrap();
    let mut checked = 0;
    for field in [
        "dimensions.cell_pixels",
        "dialogue_tree",
        "npcs[0]",
        "treasure_chests[0]",
        "warps[0]",
    ] {
        let mut bad = p.clone();
        match field {
            "dimensions.cell_pixels" => bad.dimensions.cell_pixels = 8,
            "dialogue_tree" => bad.dialogue_tree = 0,
            "npcs[0]" => bad.npcs[0].x_cell = bad.dimensions.width_cells,
            "treasure_chests[0]" => {
                // Piata has no chests: use an existing real chest, then break
                // its tagged contents while keeping the position in bounds.
                let Some(chest) = data.maps().flat_map(|(_, m)| &m.treasure_chests).next() else {
                    eprintln!("skipping chest corruption: this subset contains no chests");
                    continue;
                };
                let mut chest = chest.clone();
                chest.x_cell = 1;
                chest.y_cell = 1;
                chest.contents_type = ContentsType::Item;
                chest.item_id = None;
                chest.meseta = Some(100);
                bad.treasure_chests = vec![chest];
            }
            "warps[0]" => bad.warps[0].rect.as_mut().unwrap().width = 0,
            _ => unreachable!(),
        }
        let error = GameData::from_parts(data.manifest().clone(), vec![bad])
            .unwrap_err()
            .to_string();
        assert!(error.contains(field), "{field}: {error}");
        checked += 1;
    }
    checked
}
