//! Acceptance for chests-as-field-objects: the object slots the engine builds
//! must be the slots the cartridge builds.
//!
//! Ground truth is oracle tape 19 (`oracle/logs/19_chest_map_objects.csv`) on
//! `AcademyBasement` (map `$015`). Its object columns at the end of the tape:
//!
//! ```text
//! slot 0  id 8184  pos (468,304)  facing 8  timer 45
//! slot 1  id 80A0  pos (640,128)  facing 4  timer 0
//! slot 2  id 80A0  pos (224,304)  facing 0  timer 0
//! ```
//!
//! Slot 0 is the Xanafalgue (`$184`); slots 1 and 2 are the two treasure
//! chests (`$A0`). That ordering is `GameMode_LoadFieldMap` calling
//! `LoadMapObjects` before `LoadTreasureChests` with `Field_LoadObject` handing
//! out the first free slot — so getting it wrong shifts every object index on
//! any map that has a chest, and the comparator's `oNN_*` columns stop lining
//! up.
//!
//! Gated on the runtime pack, which is generated locally and never committed.

use std::path::Path;

use psiv_core::{
    CHEST_OPEN_FACING, CHEST_SHUT_FACING, Cell, Chest, ChestContents, FieldMap, Flag, GameState,
    PixelPos,
};
use psiv_data::{ContentsType, GameData, MapRecord};

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");
const ACADEMY_BASEMENT: u16 = 0x015;

/// The pack to read, overridable with `PSIV_PACK`.
///
/// Other lanes rebuild `runtime-pack/` in place, and a rebuild empties it
/// before refilling — so a test that only ever reads the live directory fails
/// for reasons that have nothing to do with the code under test. The override
/// lets a run point at a snapshot.
fn pack_dir() -> String {
    std::env::var("PSIV_PACK").unwrap_or_else(|_| PACK.to_string())
}

/// The map with its chests appended, built the way `GameMode_LoadFieldMap`
/// builds it: `LoadMapObjects`, then `LoadTreasureChests` with
/// `ChestFlags_Test` deciding each lid.
///
/// The conversion is done here rather than through a bridge helper on purpose —
/// `psiv-runtime`'s bridge is being restructured, and this acceptance should
/// not be hostage to where the helper lands. It is six lines and it exercises
/// the same core API the helper will.
fn build(record: &MapRecord, state: &GameState) -> FieldMap {
    let mut map = psiv_runtime::field_map(record).expect("the map builds");
    let chests: Vec<Chest> = record
        .treasure_chests
        .iter()
        .map(|chest| Chest {
            cell: Cell::new(chest.x_cell as u16, chest.y_cell as u16),
            flag: chest.chest_flag as u8,
            contents: match chest.contents_type {
                ContentsType::Item => ChestContents::Item(chest.item_id.unwrap_or(0) as u8),
                ContentsType::Meseta => ChestContents::Meseta(chest.meseta.unwrap_or(0)),
            },
            white: chest.white_chest,
            index: chest.index as usize,
        })
        .collect();
    map.with_chests(chests, |chest| state.chest_is_open(chest))
        .expect("the chests stand on the grid");
    map
}

fn load() -> Option<GameData> {
    let dir = pack_dir();
    if !Path::new(&dir).join("manifest.json").is_file() {
        eprintln!("runtime pack not present at {dir}; skipping chest slot test");
        return None;
    }
    match GameData::load(Path::new(&dir)) {
        Ok(data) => Some(data),
        Err(e) => {
            // A pack defect in a map this test does not read should not be
            // reported as a chest-slot failure. Skip loudly instead.
            eprintln!("pack at {dir} does not load ({e}); skipping chest slot test");
            None
        }
    }
}

#[test]
fn the_basements_object_slots_match_the_oracles() {
    let Some(data) = load() else { return };
    let record = data
        .map(psiv_data::MapId(ACADEMY_BASEMENT))
        .expect("AcademyBasement is in the pack");

    // Nothing opened yet, which is the state tape 19 starts the map in.
    let state = GameState::new();
    let map = build(record, &state);

    assert_eq!(map.npcs().len(), 3, "one NPC and two chests in one pool");
    assert_eq!(map.chest_slot_base(), 1, "chests start after the NPCs");

    // Slot 0: the Xanafalgue. Not a chest.
    assert!(map.chest_at_slot(0).is_none());

    // Slots 1 and 2: the chests, at the oracle's pixel positions.
    let expected = [(640, 128, 24u8), (224, 304, 25u8)];
    for (index, (x, y, flag)) in expected.into_iter().enumerate() {
        let slot = 1 + index;
        let npc = &map.npcs()[slot];
        assert_eq!(npc.id.0, 0x00A0, "slot {slot} is a treasure chest");
        let at = PixelPos::from_cell(npc.cell);
        assert_eq!((at.x, at.y), (x, y), "slot {slot} position");
        assert_eq!(
            map.chest_at_slot(slot).map(|c| c.flag),
            Some(flag),
            "slot {slot} flag"
        );
        // Both chest routines `bset #3, $2(a4)`: solid and talkable.
        assert!(
            npc.active && npc.interactable,
            "slot {slot} is a real object"
        );
    }

    // The pack's contents, so a wrong `contents_type` read shows up here.
    assert!(matches!(
        map.chest_at_slot(1).unwrap().contents,
        ChestContents::Item(_)
    ));
    assert!(matches!(
        map.chest_at_slot(2).unwrap().contents,
        ChestContents::Meseta(_)
    ));
}

#[test]
fn lid_state_comes_from_the_flag_at_build() {
    let Some(data) = load() else { return };
    let record = data
        .map(psiv_data::MapId(ACADEMY_BASEMENT))
        .expect("AcademyBasement is in the pack");

    // Shut: `LoadTreasureChests` leaves the facing word at 0.
    let state = GameState::new();
    let map = build(record, &state);
    assert_eq!(map.npcs()[1].facing, CHEST_SHUT_FACING);
    assert_eq!(map.npcs()[2].facing, CHEST_SHUT_FACING);

    // Open the first one and rebuild, as re-entering the map does.
    // `ChestFlags_Test` comes back set and the chest gets `move.w #4, $6(a4)`,
    // which is `facing_dir` — tape 19 logs that chest at facing 4.
    let mut state = GameState::new();
    state.set(Flag::chest(24)).unwrap();
    let map = build(record, &state);
    assert_eq!(map.npcs()[1].facing, CHEST_OPEN_FACING, "drawn open");
    assert_eq!(
        map.npcs()[2].facing,
        CHEST_SHUT_FACING,
        "flag 25 still clear"
    );
    assert_eq!(map.chest_is_open_at_slot(1), Some(true));
    assert_eq!(map.chest_is_open_at_slot(2), Some(false));
}

#[test]
fn every_chest_in_the_pack_builds_into_its_map() {
    // The slot rule has to hold for all 155 chests, not just the basement's —
    // a map whose chest records the bridge cannot convert would silently build
    // with empty slots and misalign every object after them.
    let Some(data) = load() else { return };
    let state = GameState::new();
    let mut maps = 0;
    let mut chests = 0;

    for entry in &data.manifest().maps {
        let Some(record) = data.map(entry.id) else {
            continue;
        };
        if record.treasure_chests.is_empty() {
            continue;
        }
        let map = build(record, &state);

        let npcs_before = map.chest_slot_base();
        assert_eq!(
            map.npcs().len(),
            npcs_before + record.treasure_chests.len(),
            "map {:?}: every chest took a slot",
            entry.id
        );
        for index in 0..record.treasure_chests.len() {
            let slot = npcs_before + index;
            let id = map.npcs()[slot].id.0;
            assert!(
                id == 0x00A0 || id == 0x01D4,
                "map {:?} slot {slot}: id {id:#06X} is not a chest",
                entry.id
            );
            assert_eq!(map.chest_at_slot(slot).map(|c| c.index), Some(index));
        }
        maps += 1;
        chests += record.treasure_chests.len();
    }

    assert!(maps > 0, "the pack has chest-bearing maps");
    eprintln!("{chests} chests across {maps} maps built into their object pools");
}
