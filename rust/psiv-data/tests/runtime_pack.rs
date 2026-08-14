//! Integration test against the real runtime pack.
//!
//! Gated on the pack existing, mirroring the Python suite's ROM gating: the
//! pack is Sega-derived and never committed (docs/RUNTIME_DESIGN.md, "Data
//! path"), so a fresh clone must not fail this. Build one with
//! `python -m psiv_tools pack <rom> runtime-pack/` and the test starts running.
//!
//! Set `PSIV_RUNTIME_PACK` to test a pack somewhere else -- a subset built for
//! one town, say, which is what `build_pack`'s `map_ids` argument is for.

use psiv_data::{CollisionType, Direction, GameData, MapId, TransitionTable};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// `<repo>/runtime-pack`, reached from this crate at `<repo>/rust/psiv-data`,
/// unless `PSIV_RUNTIME_PACK` names somewhere else.
fn pack_dir() -> PathBuf {
    match std::env::var_os("PSIV_RUNTIME_PACK") {
        Some(path) => PathBuf::from(path),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("runtime-pack"),
    }
}

/// The pack directory, or `None` with a note on stderr when it is absent.
fn require_pack() -> Option<PathBuf> {
    let dir = pack_dir();
    if dir.join("manifest.json").is_file() {
        return Some(dir);
    }
    eprintln!(
        "skipping: no runtime pack at {}. Build one with \
         `python -m psiv_tools pack <rom> runtime-pack/`, or set PSIV_RUNTIME_PACK.",
        dir.display()
    );
    None
}

/// Map $010, Piata -- the vertical slice's home town.
const PIATA: MapId = MapId(0x010);

#[test]
fn the_real_pack_loads_and_validates() {
    let Some(dir) = require_pack() else { return };

    // Loading is the assertion: every validation in the crate runs here, on
    // every map in the pack, and any failure fails this test naming the map and
    // field that broke.
    let data = GameData::load(&dir).expect("the runtime pack should load and validate");

    assert!(!data.is_empty(), "the pack contains no maps");
    assert_eq!(
        data.len(),
        data.manifest().maps.len(),
        "every manifest entry should have produced a record"
    );
    assert_eq!(data.manifest().rom.sha256.as_str().len(), 64);
    assert!(data.manifest().rom.size_bytes > 0);

    for (id, record) in data.maps() {
        assert_eq!(record.id, id);
        let dims = &record.dimensions;
        assert_eq!(
            record.collision.grid.cells().len(),
            (dims.width_cells * dims.height_cells) as usize,
            "map {id} grid size"
        );
        assert!(
            (1..=43).contains(&record.dialogue_tree),
            "map {id} binds dialogue tree {}",
            record.dialogue_tree
        );

        // Every warp facing the cartridge stores is one of the four
        // `Map_Start_Facing_Dir` names.
        for warp in &record.warps {
            assert!(
                warp.facing.direction().is_some(),
                "map {id} warp {} has undecoded facing byte 0x{:X}",
                warp.index,
                warp.facing.id
            );
        }
    }
}

/// The whole cartridge holds exactly one field object whose facing byte is not
/// one of the four `Map_Start_Facing_Dir` values: `AiedoPub`'s third object, an
/// `NPCType30` with byte `$10`. Pinned rather than asserted away, so that if a
/// packer change starts producing more of them, this says so.
const KNOWN_UNDECODED_FACINGS: &[(MapId, u32, u8)] = &[(MapId(0x064), 2, 0x10)];

#[test]
fn only_the_one_known_object_has_an_undecoded_facing() {
    let Some(dir) = require_pack() else { return };
    let data = GameData::load(&dir).expect("the runtime pack should load and validate");

    let found: Vec<(MapId, u32, u8)> = data
        .maps()
        .flat_map(|(id, record)| {
            record
                .npcs
                .iter()
                .filter(|npc| npc.facing.direction().is_none())
                .map(move |npc| (id, npc.index, npc.facing.id))
        })
        .collect();

    for entry in &found {
        assert!(
            KNOWN_UNDECODED_FACINGS.contains(entry),
            "map {} object {} has an unexpected undecoded facing byte 0x{:X}",
            entry.0,
            entry.1,
            entry.2
        );
    }
}

/// Read the manifest as raw JSON. The diagnostics the packer publishes beside
/// the inventory -- the census, the dead-doorway report -- are deliberately not
/// in [`psiv_data::Manifest`]: the runtime does not consume them, and modelling
/// them would make every future report a schema change. Tests read them
/// straight from the file instead.
fn raw_manifest(dir: &std::path::Path) -> serde_json::Value {
    serde_json::from_str(
        &std::fs::read_to_string(dir.join("manifest.json")).expect("read the manifest"),
    )
    .expect("the manifest is JSON")
}

/// A `{"7": 172}` count map from the manifest, as a sorted map.
fn census_counts(manifest: &serde_json::Value, key: &str) -> BTreeMap<u64, u64> {
    manifest["census"][key]
        .as_object()
        .unwrap_or_else(|| panic!("the manifest publishes census.{key}"))
        .iter()
        .map(|(value, count)| {
            (
                value.parse::<u64>().expect("census keys are numbers"),
                count.as_u64().expect("census values are counts"),
            )
        })
        .collect()
}

fn tally(counts: &mut BTreeMap<u64, u64>, value: u64) {
    *counts.entry(value).or_default() += 1;
}

#[test]
fn the_packers_census_matches_every_value_we_parsed() {
    let Some(dir) = require_pack() else { return };
    let data = GameData::load(&dir).expect("the runtime pack should load and validate");
    let manifest = raw_manifest(&dir);

    // The census is what the packer observed in the ROM; these are the same
    // quantities counted off the typed values this crate produced. Comparing
    // them end to end catches a decode drift that no per-map assertion would:
    // one wrong nibble anywhere in 1.7 million cells moves a count.
    let mut collision = BTreeMap::new();
    let mut npc_facing = BTreeMap::new();
    let mut warp_facing = BTreeMap::new();
    let mut trees = BTreeMap::new();

    for (_, record) in data.maps() {
        for cell in record.collision.grid.cells() {
            tally(&mut collision, u64::from(cell.code()));
        }
        for npc in &record.npcs {
            tally(&mut npc_facing, u64::from(npc.facing.id));
        }
        for warp in &record.warps {
            tally(&mut warp_facing, u64::from(warp.facing.id));
        }
        tally(&mut trees, u64::from(record.dialogue_tree));
    }

    assert_eq!(
        collision,
        census_counts(&manifest, "collision_types"),
        "collision type counts disagree with the packer"
    );
    assert_eq!(
        npc_facing,
        census_counts(&manifest, "npc_facing_bytes"),
        "npc facing byte counts disagree with the packer"
    );
    assert_eq!(
        warp_facing,
        census_counts(&manifest, "warp_facing_bytes"),
        "warp facing byte counts disagree with the packer"
    );
    assert_eq!(
        trees,
        census_counts(&manifest, "dialogue_trees"),
        "dialogue tree bindings disagree with the packer"
    );
}

#[test]
fn the_manifest_says_its_named_types_are_not_the_valid_set() {
    let Some(dir) = require_pack() else { return };
    let manifest = raw_manifest(&dir);

    // The nibble has sixteen values and `TileCollNormalPtrs` has sixteen
    // entries; the eight names are documentation. An earlier manifest listed
    // only the names, which is what led this crate to model a closed set and
    // reject retail code $7. The packer now says so outright, and this holds it
    // to that.
    assert_eq!(manifest["collision"]["type_space"].as_u64(), Some(16));
    assert_eq!(
        manifest["collision"]["named_types_are_not_the_valid_set"].as_bool(),
        Some(true)
    );

    // Codes outside the eight named ones are present in retail and walkable.
    let census = census_counts(&manifest, "collision_types");
    let unnamed: Vec<u64> = census
        .keys()
        .copied()
        .filter(|code| {
            !CollisionType::try_from(*code as u8)
                .expect("census codes are nibbles")
                .is_named()
        })
        .collect();
    assert_eq!(unnamed, vec![0x7], "retail uses exactly one unnamed code");
    assert!(!CollisionType::Unnamed(0x7).blocks());
}

#[test]
fn the_packers_dead_doorway_report_matches_the_grid_we_loaded() {
    let Some(dir) = require_pack() else { return };
    let data = GameData::load(&dir).expect("the runtime pack should load and validate");

    // Table 2 is reached only from a standing collision type of 1, so a table-2
    // rectangle covering no type-1 cell can never fire. The packer computes
    // that from its own collision decode and lists the casualties in
    // `warps.doors_without_map_change_cell`. Recomputing it from the grid this
    // crate loaded, and demanding the two agree exactly, proves the runtime is
    // reading the same collision the packer reasoned about -- in both
    // directions, so neither a missed nor an invented anomaly slips through.
    let mut found: Vec<(u16, u32)> = Vec::new();
    let mut live_doors = 0;
    for (id, record) in data.maps() {
        for warp in &record.warps {
            if warp.table != TransitionTable::MapChange {
                continue;
            }
            let Some(rect) = warp.rect else { continue };
            let covers_door = (rect.y..rect.end().y).any(|y| {
                (rect.x..rect.end().x)
                    .any(|x| record.collision.grid.at(x, y) == Some(CollisionType::MapChange))
            });
            if covers_door {
                live_doors += 1;
            } else {
                found.push((id.0, warp.index));
            }
        }
    }
    found.sort_unstable();

    let manifest = raw_manifest(&dir);
    let mut reported: Vec<(u16, u32)> = manifest["warps"]["doors_without_map_change_cell"]
        .as_array()
        .expect("the packer reports dead doorways")
        .iter()
        .map(|entry| {
            (
                entry["id"].as_u64().expect("map id") as u16,
                entry["warp_index"].as_u64().expect("warp index") as u32,
            )
        })
        .collect();
    reported.sort_unstable();

    assert_eq!(
        found, reported,
        "the doorways this crate finds dead and the ones the packer reported disagree"
    );
    assert!(
        live_doors > 0,
        "no doorway in the whole pack lands on a map_change cell, which cannot be right"
    );
}

#[test]
fn piata_looks_like_piata() {
    let Some(dir) = require_pack() else { return };
    let data = GameData::load(&dir).expect("the runtime pack should load and validate");

    if !data.contains(PIATA) {
        eprintln!("skipping: map {PIATA} is not in this pack");
        return;
    }

    let piata = data.map(PIATA).expect("checked just above");
    assert_eq!(piata.symbol.as_deref(), Some("Piata"));
    assert!(
        piata.dimensions.width_cells > 1 && piata.dimensions.height_cells > 1,
        "Piata is {}x{} cells",
        piata.dimensions.width_cells,
        piata.dimensions.height_cells
    );
    assert!(!piata.npcs.is_empty(), "Piata is populated");
    assert!(
        !piata.warps.is_empty(),
        "Piata has doorways and a road out of town"
    );

    // Every warp lands somewhere the pack accounts for; `load` proved that, so
    // here we only check we can act on it.
    for warp in &piata.warps {
        assert!(
            data.knows(warp.target.id),
            "map {PIATA} warps to unknown map {}",
            warp.target.id
        );
    }

    // A town has somewhere to stand, something to walk into, and a way out.
    let grid = &piata.collision.grid;
    assert!(
        grid.cells().iter().any(|c| !c.blocks()),
        "Piata has no walkable cell"
    );
    assert!(
        grid.cells().iter().any(|c| c.blocks()),
        "Piata has no blocking cell"
    );
    assert!(
        grid.count_of(CollisionType::MapChange) > 0,
        "Piata should have map_change cells to leave by"
    );

    // The academy doorway is the vertical slice's headline warp.
    let academy = MapId(0x011);
    if data.contains(academy) {
        let door = piata
            .warps
            .iter()
            .find(|w| w.target.id == academy)
            .expect("Piata warps to the academy");
        assert_eq!(door.table, TransitionTable::MapChange, "a doorway");
        assert_eq!(door.facing.direction(), Some(Direction::Up));
        let landing = door.destination.pos();
        let inside = data.map(academy).expect("packed");
        assert!(
            inside.contains(landing),
            "the academy landing cell {landing:?} is off its map"
        );
    }
}
