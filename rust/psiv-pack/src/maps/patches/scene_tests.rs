//! Citation rows for the scene/atlas contract. Atlas-producing rows come from
//! the real builder. These five guard-only rows document chunks supplied by
//! map-load effects or overworld hooks, as in test_scene_chunk_atlas.py.
use super::*;

const GUARDS: &[SceneChunkWrite] = &[
    SceneChunkWrite {
        scene: "Event_JuzaDefeated",
        map_id: 0x87,
        chunks: &[0x16],
        table: None,
        // loc_6FBC4 stair rows; RefreshPlaneBG at $06FBA6.
        checks: &[(0x6fba6, "4EB9000540FA")],
    },
    SceneChunkWrite {
        scene: "Event_MachineCenterAppearing",
        map_id: 0,
        chunks: &[0xd3],
        table: None,
        // loc_6B5C0: GetMapLayoutChunkBG($39,$5A), move.b at $06B5D4.
        checks: &[(0x6b5d4, "12BC00D3")],
    },
    SceneChunkWrite {
        scene: "Cutscene_Landale",
        map_id: 1,
        chunks: &[0x2c],
        table: None,
        // Event_DezoSpaceportAppearing; also written by load hook loc_53E98.
        checks: &[(0x6c15c, "12BC002C")],
    },
    SceneChunkWrite {
        scene: "Cutscene_PsycoWand",
        map_id: 0x39,
        chunks: &[0x8f, 0x90],
        table: None,
        // FG (9,5), BG (9,6): the two move.b sites.
        checks: &[(0x75810, "12BC008F"), (0x75824, "12BC0090")],
    },
    SceneChunkWrite {
        scene: "Cutscene_ZioDefeated",
        map_id: 0,
        chunks: &[0x3f],
        table: None,
        // Event_MotaSpaceportAppearing, loc_6B7FA, GetMapLayoutChunkBG($1A,$2D).
        checks: &[(0x6b80e, "12BC003F")],
    },
];

pub(in crate::maps) fn cited_rows(rom: &[u8]) -> BTreeMap<&'static str, (usize, BTreeSet<u16>)> {
    let mut rows = BTreeMap::new();
    for row in SCENE_CHUNK_WRITES.iter().chain(GUARDS) {
        let chunks = row_chunks(rom, row)
            .unwrap()
            .into_iter()
            .map(u16::from)
            .collect();
        assert!(
            rows.insert(row.scene, (row.map_id, chunks)).is_none(),
            "{} repeated",
            row.scene
        );
    }
    rows
}

#[test]
fn guard_only_rows_pin_their_own_writes_without_adding_atlas_chunks() {
    let Some(rom) = crate::test_support::retail_rom("scene guard rows") else {
        return;
    };
    for row in GUARDS {
        assert!(!row_chunks(rom.bytes(), row).unwrap().is_empty());
        assert!(!SCENE_CHUNK_WRITES.iter().any(|r| r.scene == row.scene));
        for &(at, _) in row.checks {
            let mut broken = rom.bytes().to_vec();
            broken[at] ^= 1;
            assert!(row_chunks(&broken, row).unwrap_err().0.contains(row.scene));
        }
    }
}
