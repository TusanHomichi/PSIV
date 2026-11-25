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
        checks: &[(
            0x6fba6,
            6,
            "56a86c9c5e40f1fa732dd709176385f0a2b558ad8c166e5b999999fbd3d89edb",
        )],
    },
    SceneChunkWrite {
        scene: "Event_MachineCenterAppearing",
        map_id: 0,
        chunks: &[0xd3],
        table: None,
        // loc_6B5C0: GetMapLayoutChunkBG($39,$5A), move.b at $06B5D4.
        checks: &[(
            0x6b5d4,
            4,
            "4ec3451ea80cd49e387961d4499ac200068c3776732b710724dd2ebda86d3c2b",
        )],
    },
    SceneChunkWrite {
        scene: "Cutscene_Landale",
        map_id: 1,
        chunks: &[0x2c],
        table: None,
        // Event_DezoSpaceportAppearing; also written by load hook loc_53E98.
        checks: &[(
            0x6c15c,
            4,
            "aa78a65fd1b556e142664633478a88412cb1f2cf484a327d4d812cf46ff7fb08",
        )],
    },
    SceneChunkWrite {
        scene: "Cutscene_PsycoWand",
        map_id: 0x39,
        chunks: &[0x8f, 0x90],
        table: None,
        // FG (9,5), BG (9,6): the two move.b sites.
        checks: &[
            (
                0x75810,
                4,
                "330f75404c68dc3d2197164fbf92ef153a1d1d7ba1a0c8a6069847642d487143",
            ),
            (
                0x75824,
                4,
                "7f8f3aeca8b3434992bb7e77b1642ebc6b0b6e2249adcadac9790b605fff5116",
            ),
        ],
    },
    SceneChunkWrite {
        scene: "Cutscene_ZioDefeated",
        map_id: 0,
        chunks: &[0x3f],
        table: None,
        // Event_MotaSpaceportAppearing, loc_6B7FA, GetMapLayoutChunkBG($1A,$2D).
        checks: &[(
            0x6b80e,
            4,
            "cb617b9b39be914995f5d7fd667bc42b587c90ebb1fd630afdedcdb4153a11ac",
        )],
    },
];

pub(in crate::maps) fn cited_rows(
    rom: &[u8],
) -> BTreeMap<&'static str, Vec<(usize, BTreeSet<u16>)>> {
    let mut rows: BTreeMap<&'static str, Vec<(usize, BTreeSet<u16>)>> = BTreeMap::new();
    for row in SCENE_CHUNK_WRITES.iter().chain(GUARDS) {
        let chunks = row_chunks(rom, row)
            .unwrap()
            .into_iter()
            .map(u16::from)
            .collect();
        let entry = rows.entry(row.scene).or_default();
        assert!(
            !entry.iter().any(|(map_id, _)| *map_id == row.map_id),
            "{} repeated for map {:#05x}",
            row.scene,
            row.map_id
        );
        entry.push((row.map_id, chunks));
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
        assert!(
            !SCENE_CHUNK_WRITES
                .iter()
                .any(|r| r.scene == row.scene && r.map_id == row.map_id)
        );
        for &(at, _, _) in row.checks {
            let mut broken = rom.bytes().to_vec();
            broken[at] ^= 1;
            assert!(row_chunks(&broken, row).unwrap_err().0.contains(row.scene));
        }
    }
}
