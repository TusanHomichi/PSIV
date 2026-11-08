//! #40: a Motavia overworld battle draws the background the cartridge picks
//! for the chunk under the leader.
//!
//! `FieldRoutine_Battle` (`ps4.asm:117936-117944`) reads the chunk under
//! `Character_1` and `GetMotaBattleBGIndex` (`ps4.asm:118165`) indexes
//! `MotaBattleBGIndexes` with it, unbounded. The table runs 214 bytes, to
//! `FieldRoutine_Interaction`; the disassembly labels only the first 42, and a
//! port that read 42 drew background 0 on a third of Motavia's walkable
//! cells.
//!
//! The two positions are oracle tape 31's: its natural Motavia battle at
//! f93785 (leader at x 1296, y 1408: chunk 41, the beach), and the same
//! battle with the leader moved to x 2320 at f93786 (chunk 184, the grass).
//! Retail drew those two backgrounds; the frames are in the PR that added
//! this test.

use std::path::Path;
use std::sync::OnceLock;

use psiv_core::{CharId, Flag, GameState, RetailLocation, RetailSave};
use psiv_data::{BattleBackgrounds, BattleFiles, DataError, GameData, MOTAVIA_TERRAIN_ENTRIES};
use psiv_runtime::{Runtime, Session};

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

fn pack_dir() -> &'static Path {
    static DIR: OnceLock<std::path::PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        std::env::var_os("PSIV_RUNTIME_PACK")
            .map(Into::into)
            .unwrap_or_else(|| PACK.into())
    })
}

fn pack() -> Option<&'static (GameData, BattleFiles)> {
    static PACK_FILES: OnceLock<Option<(GameData, BattleFiles)>> = OnceLock::new();
    PACK_FILES
        .get_or_init(|| {
            let path = pack_dir();
            if !path.join("manifest.json").is_file() {
                eprintln!("runtime pack absent; skipping the Motavia background tests");
                return None;
            }
            Some((
                GameData::load(path).expect("pack loads"),
                BattleFiles::load(path).expect("battle files load"),
            ))
        })
        .as_ref()
}

/// A session on Motavia (map 0) with the leader at retail pixel `(x, y)`.
fn motavia_at(x: u16, y: u16) -> Option<Session> {
    motavia_with(x, y, &[])
}

/// [`motavia_at`] with these event flags set.
fn motavia_with(x: u16, y: u16, flags: &[u16]) -> Option<Session> {
    let (data, files) = pack()?;
    let initial = Session::start(data.clone())
        .with_battles(files.clone())
        .field()
        .expect("the pack boots");
    let mut game = GameState::from_snapshot(&initial.runtime().game().snapshot());
    game.set_party([Some(CharId(0)), None, None, None, None]);
    for &flag in flags {
        game.set(Flag::event(flag)).expect("flag in range");
    }
    Some(
        Session::start(data.clone())
            .with_battles(files.clone())
            .from_save(RetailSave {
                snapshot: game.snapshot(),
                location: RetailLocation {
                    world_index: 0,
                    map_index_2: 0,
                    map_index: 0,
                    char_x: x,
                    char_y: y,
                },
            })
            .expect("the session starts"),
    )
}

/// The pack's index for the background named `symbol`.
fn background(symbol: &str) -> u8 {
    let path = pack_dir().join(psiv_data::BATTLE_BACKGROUNDS_PATH);
    let file: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("backgrounds.json"))
            .expect("backgrounds.json parses");
    let entry = file["backgrounds"]
        .as_array()
        .expect("a background list")
        .iter()
        .find(|entry| entry["symbol"] == symbol)
        .unwrap_or_else(|| panic!("no background {symbol}"));
    u8::try_from(entry["index"].as_u64().expect("an index")).expect("a byte")
}

fn chunk_and_background(runtime: &Runtime) -> (Option<u8>, Option<u8>) {
    (
        runtime.vehicle_battle_terrain(),
        runtime.battle_background(None),
    )
}

#[test]
fn a_pack_whose_motavia_table_stops_at_the_disassembly_label_is_refused() {
    // No pack needed: the loader is what keeps an older extractor's 42-byte
    // table from drawing background 0 again.
    let dir = std::env::temp_dir().join(format!("psiv-motavia-table-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("battle/art")).expect("temporary art directory");
    let write = |count: usize| {
        let json = serde_json::json!({
            "format_version": psiv_data::PACK_FORMAT_VERSION,
            "selection": {
                "event_battle": {"indexes": [13]},
                "field_map": {"indexes": [255, 4], "none": 255},
                "motavia_terrain": {"indexes": vec![3u8; count]},
                "dark_force_2_swap": {"from": 4, "to": 5},
            },
        });
        std::fs::write(
            dir.join(psiv_data::BATTLE_BACKGROUNDS_PATH),
            json.to_string(),
        )
        .expect("write backgrounds");
    };
    write(42);
    assert!(matches!(
        BattleBackgrounds::load(&dir),
        Err(DataError::ManifestMismatch {
            field: "motavia_terrain",
            ..
        })
    ));
    write(MOTAVIA_TERRAIN_ENTRIES);
    let tables = BattleBackgrounds::load(&dir)
        .expect("the full table loads")
        .expect("the file is there");
    // Chunk 184 is past the label's 42 and still has its own background.
    assert_eq!(tables.select(None, 0, Some(184), false, false), Some(2));
    assert_eq!(tables.motavia(184), Some(2));
    std::fs::remove_dir_all(&dir).expect("remove temporary pack");
}

#[test]
fn every_motavia_chunk_has_a_background() {
    let Some((data, _)) = pack() else { return };
    let tables = data
        .battle_backgrounds()
        .expect("the retail pack has the selection tables");
    let grid = data
        .map(psiv_data::MapId(0))
        .and_then(|record| record.vehicle_battle.as_ref())
        .expect("Motavia has a chunk grid");
    let mut past_the_label = 0;
    for row in &grid.rows {
        for &chunk in row {
            let chunk = (chunk & 0xFF) as u8;
            assert!(
                tables.motavia(chunk).is_some(),
                "chunk {chunk} has no entry"
            );
            past_the_label += usize::from(chunk >= 42);
        }
    }
    assert!(past_the_label > 0, "the grid uses chunks past the label");
}

#[test]
fn tape_31_s_battle_draws_the_beach_and_the_grass_a_chunk_further_east() {
    let Some(natural) = motavia_at(1296, 1408) else {
        return;
    };
    assert_eq!(
        chunk_and_background(natural.runtime()),
        (Some(41), Some(background("MotaBeach")))
    );
    let moved = motavia_at(2320, 1408).expect("the pack is there");
    assert_eq!(
        chunk_and_background(moved.runtime()),
        (Some(184), Some(background("MotaGrass")))
    );
}

#[test]
fn a_vehicle_on_a_chunk_past_the_label_picks_its_formation_group_by_it() {
    // `Battle_SetupEnemyData` picks the Motavia vehicle group from
    // `Mota_Battle_BG_Index` (`ps4.asm:11829-11838`). Chunk 48, on the same
    // row, stores the sea, index 3: group `$A`, not the `$8` a 42-byte table
    // would have given.
    let Some(session) = motavia_at(62 * 32 + 16, 1408) else {
        return;
    };
    let runtime = session.runtime();
    assert_eq!(runtime.vehicle_battle_terrain(), Some(48));
    assert_eq!(runtime.motavia_battle_background(), background("MotaSea"));
}

#[test]
fn a_fixture_staging_a_battle_on_another_map_keeps_the_live_context() {
    // The debug window fixture overrides the map and nothing else: a map-0
    // override still reads the chunk underfoot, and a Dezolis override after
    // Dark Force 2 still takes the swap (`loc_6C24`).
    let Some(grass) = motavia_at(2320, 1408) else {
        return;
    };
    assert_eq!(
        grass.runtime().battle_background_on(None, 0),
        Some(background("MotaGrass"))
    );
    let after = motavia_with(2320, 1408, &[0x9E]).expect("the pack is there");
    assert_eq!(
        grass.runtime().battle_background_on(None, 1),
        Some(background("Dezo1"))
    );
    assert_eq!(
        after.runtime().battle_background_on(None, 1),
        Some(background("Dezo2")),
        "Dark Force 2's flag swaps Dezolis's background"
    );
    // An event battle wins over all of it.
    assert_eq!(
        after.runtime().battle_background_on(Some(0), 0x17),
        Some(background("AcademyBasement"))
    );
}
