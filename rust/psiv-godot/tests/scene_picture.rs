//! The Le Roof scenes' plane-A picture (#594): staged by its `LoadSceneAsset`,
//! shown by the next `DmaPlanes`, cleared by the plane redraw, driven by the
//! registered scene records. Imports the production plane state without
//! starting Godot or loading a ROM.

#[allow(dead_code)]
#[path = "../src/cutscene/picture_plane.rs"]
mod picture_plane;

#[allow(dead_code)]
#[path = "../src/cutscene/state.rs"]
mod state;

use picture_plane::{PictureChange, PicturePlane};
use psiv_core::{EventIndex, PresentationOp, SceneOp, scene_for};
use state::PresentationState;

/// `loc_1DDE0A`, the galaxy's cells, and the `LoadArt` they index.
const GALAXY: u32 = 0x001D_DE0A;
const GALAXY_ART: (u32, u16) = (0x001D_B2C8, 0x260);

/// What the pack says: only the galaxy's cells are a picture.
fn picture_art(source: u32) -> Option<(u32, u16)> {
    (source == GALAXY).then_some(GALAXY_ART)
}

/// Runs `ops` as the shell orders them (the plane first, a `LoadArt`
/// recorded as the shell records it) and returns, per op, what the plane
/// did and whether a picture showed after it.
fn run(ops: &[SceneOp]) -> Vec<(PictureChange, Option<u32>)> {
    let mut plane = PicturePlane::default();
    let mut presentation = PresentationState::default();
    ops.iter()
        .map(|op| {
            let change = plane.consume(op, picture_art, |art, tile| {
                presentation.art_loaded(art, tile)
            });
            if let SceneOp::LoadArt { rom_addr, tile } = op {
                presentation.load_art(*rom_addr, *tile);
            }
            (change, plane.visible)
        })
        .collect()
}

fn ops(event: u16) -> &'static [SceneOp] {
    scene_for(EventIndex(event)).expect("registered").ops
}

/// The ops whose picture changed, and the visible picture after each.
fn shown(ops: &[SceneOp]) -> Vec<(usize, Option<u32>)> {
    let mut last = None;
    let mut out = Vec::new();
    for (index, (_, visible)) in run(ops).into_iter().enumerate() {
        if visible != last {
            out.push((index, visible));
            last = visible;
        }
    }
    out
}

#[test]
fn meeting_le_roof_shows_the_galaxy_from_its_dma_to_the_map_reload() {
    let ops = ops(0x0048);
    // Op 8 is the DMA after the art (4), the cells (5) and the palette (6-7);
    // op 18 is `RefreshPlaneFG`/`BG`.
    assert!(matches!(ops[8], SceneOp::DmaPlanes));
    assert!(matches!(
        ops[18],
        SceneOp::Presentation {
            op: PresentationOp::ReloadMapChunks
        }
    ));
    assert_eq!(shown(ops), [(8, Some(GALAXY)), (18, None)]);
    assert_eq!(run(ops)[5].0, PictureChange::Staged(GALAXY));
}

#[test]
fn le_roof_again_shows_the_galaxy_until_its_first_init_vram() {
    let ops = ops(0x801C);
    assert!(matches!(ops[7], SceneOp::DmaPlanes));
    assert!(matches!(ops[14], SceneOp::InitVramAndCram));
    // The panel DMAs after op 14 show no picture.
    assert_eq!(shown(ops), [(7, Some(GALAXY)), (14, None)]);
}

/// Negative controls: without its `LoadSceneAsset` (the record before #594)
/// the scene's DMA shows nothing; cells loaded before their art are refused
/// and never shown; another scene's asset (the Machine Center's) is not a
/// picture; and a DMA keeps plane A until a redraw.
#[test]
fn a_picture_needs_its_cells_after_its_art_before_the_dma() {
    let without: Vec<SceneOp> = ops(0x801C)
        .iter()
        .copied()
        .filter(|op| {
            !matches!(
                op,
                SceneOp::Presentation {
                    op: PresentationOp::LoadSceneAsset { .. }
                }
            )
        })
        .collect();
    assert_eq!(shown(&without), []);

    let cells = SceneOp::Presentation {
        op: PresentationOp::LoadSceneAsset {
            source_rom_addr: GALAXY,
            destination_ram: 0xFFFF_0000,
        },
    };
    let art = SceneOp::LoadArt {
        rom_addr: GALAXY_ART.0,
        tile: GALAXY_ART.1,
    };
    let early = run(&[cells, art, SceneOp::DmaPlanes]);
    assert_eq!(
        early[0].0,
        PictureChange::ArtMissing {
            source: GALAXY,
            art: GALAXY_ART.0,
            tile: GALAXY_ART.1
        }
    );
    assert_eq!(early[2].1, None);

    let other = SceneOp::Presentation {
        op: PresentationOp::LoadSceneAsset {
            source_rom_addr: 0x001D_3710,
            destination_ram: 0xFFFF_0000,
        },
    };
    assert_eq!(
        run(&[art, other, SceneOp::DmaPlanes])[1].0,
        PictureChange::None
    );

    let kept = run(&[
        art,
        cells,
        SceneOp::DmaPlanes,
        SceneOp::PanelCreate { id: 0x11B },
        SceneOp::DmaPlanes,
        SceneOp::InitVramAndCram,
        SceneOp::DmaPlanes,
    ]);
    let visible: Vec<_> = kept.iter().map(|(_, visible)| *visible).collect();
    assert_eq!(
        visible,
        [
            None,
            None,
            Some(GALAXY),
            Some(GALAXY),
            Some(GALAXY),
            None,
            None
        ]
    );
}

/// A teardown outside the scene's ops (its end, a field reload, a map
/// change, the return to title: the shell's `clear_planes`) drops a staged
/// picture as well as a shown one, so no later DMA revives either.
#[test]
fn a_teardown_drops_a_staged_and_a_shown_picture_for_good() {
    let art = SceneOp::LoadArt {
        rom_addr: GALAXY_ART.0,
        tile: GALAXY_ART.1,
    };
    let cells = SceneOp::Presentation {
        op: PresentationOp::LoadSceneAsset {
            source_rom_addr: GALAXY,
            destination_ram: 0xFFFF_0000,
        },
    };
    let mut presentation = PresentationState::default();
    presentation.load_art(GALAXY_ART.0, GALAXY_ART.1);
    let loaded = |art: u32, tile: u16| presentation.art_loaded(art, tile);
    for shown in [false, true] {
        let mut plane = PicturePlane::default();
        plane.consume(&art, picture_art, loaded);
        plane.consume(&cells, picture_art, loaded);
        if shown {
            plane.consume(&SceneOp::DmaPlanes, picture_art, loaded);
            assert_eq!(plane.visible, Some(GALAXY));
        }
        plane.clear();
        assert_eq!(plane, PicturePlane::default());
        plane.consume(&SceneOp::DmaPlanes, picture_art, loaded);
        assert_eq!(plane.visible, None, "shown before the teardown: {shown}");
    }
}
