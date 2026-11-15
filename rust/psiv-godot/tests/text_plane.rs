//! Production RunText2 plane state: synthetic glyphs, no ROM or Godot required.
#[path = "../src/cutscene/text_plane.rs"]
mod text_plane;
use psiv_core::{EventIndex, PresentationOp, SceneOp, scene_for};
use text_plane::{TextCell, TextPlane};

fn clear() -> SceneOp {
    SceneOp::Presentation {
        op: PresentationOp::ClearPlanes,
    }
}

#[test]
fn retail_staff_destinations_need_each_transfer_and_survive_palette_ops() {
    let ops = scene_for(EventIndex(0x8021)).unwrap().ops;
    let mut plane = TextPlane::default();
    let mut drawn = 0;
    let mut transferred: u16 = 0;
    let mut rolling = false;
    for op in ops {
        if let SceneOp::DrawTextToPlane {
            entry,
            plane: dest,
            vram,
        } = *op
        {
            assert_eq!(entry, 0x12 + drawn);
            assert_eq!(dest, 0xFFFF_8406 + u32::from(drawn) * 0x180);
            assert_eq!(vram, 0xC100 + u32::from(drawn) * 0x80);
            plane.draw(dest, vram, "X").unwrap();
            // New destination remains invisible until DMA (negative control).
            assert_eq!(plane.cell(3, 8 + usize::from(drawn) * 3), None);
            drawn += 1;
            rolling = true;
        }
        plane.consume(op);
        if rolling && matches!(op, SceneOp::DmaPlanes) && transferred < 4 {
            transferred += 1;
            for row in 0..usize::from(transferred) {
                assert_eq!(
                    plane.cell(3, 8 + row * 3),
                    Some(TextCell {
                        ch: 'X',
                        half: 0,
                        line: 2
                    })
                );
                assert_eq!(
                    plane.cell(3, 9 + row * 3),
                    Some(TextCell {
                        ch: 'X',
                        half: 1,
                        line: 2
                    })
                );
                // Opening origin is wrong for staff (negative control).
                assert_eq!(plane.cell(5, 8 + row * 3), None);
            }
        }
        if matches!(
            op,
            SceneOp::Presentation {
                op: PresentationOp::EndingStaffRollFadeOut { .. }
                    | PresentationOp::EndingStaffRollFadeIn { .. }
            }
        ) {
            assert_eq!(
                transferred, 4,
                "all four entries need a transfer before the transition"
            );
            assert!(plane.is_visible());
        }
        if rolling
            && matches!(
                op,
                SceneOp::Presentation {
                    op: PresentationOp::PaletteRestoreTone { .. }
                }
            )
        {
            assert!(
                !plane.is_visible(),
                "clear and DMA remove the staff before restore"
            );
            break;
        }
    }
    assert_eq!(drawn, 4);
}

#[test]
fn clear_is_staged_and_cannot_revive_text_after_a_redraw() {
    let mut p = TextPlane::default();
    p.draw(0xFFFF8406, 0xC100, "X").unwrap();
    assert!(!p.is_visible());
    p.consume(&SceneOp::DmaPlanes);
    assert!(p.is_visible());
    p.consume(&clear());
    assert!(p.is_visible(), "buffer clear is not a VRAM transfer");
    p.consume(&SceneOp::DmaPlanes);
    assert!(!p.is_visible());
    p.draw(0xFFFF8406, 0xC100, "Y").unwrap();
    p.consume(&SceneOp::DmaPlanes);
    p.consume(&SceneOp::InitVramAndCram);
    p.consume(&SceneOp::DmaPlanes);
    assert!(!p.is_visible());
}

#[test]
fn opening_uses_its_destinations_and_tile_allocations_for_both_pages() {
    let mut p = TextPlane::default();
    let mut count = 0;
    for op in scene_for(EventIndex(0x9F)).unwrap().ops {
        if let SceneOp::DrawTextToPlane { entry, plane, vram } = *op {
            let row = (count % 4) as usize;
            assert_eq!(entry, 0x3A + count);
            assert_eq!(plane, 0xFFFF840A + row as u32 * 0x180);
            assert_eq!(vram, 0x4200 + row as u32 * 0x80);
            p.draw(plane, vram, if count < 4 { "A" } else { "B" })
                .unwrap();
            count += 1;
        }
        p.consume(op);
        if matches!(op, SceneOp::IntroTextFadeUp) {
            for row in 0..4 {
                assert_eq!(
                    p.cell(5, 8 + row * 3),
                    Some(TextCell {
                        ch: if count == 4 { 'A' } else { 'B' },
                        half: 0,
                        line: 2
                    })
                );
                assert_eq!(p.cell(3, 8 + row * 3), None);
            }
        }
    }
    assert_eq!(count, 8);
}

#[test]
fn pattern_reuse_and_plane_wrapping_follow_runtext2() {
    let mut p = TextPlane::default();
    p.draw(0xFFFF8FFE, 0xC100, "AB").unwrap();
    p.consume(&SceneOp::DmaPlanes);
    assert_eq!(p.cell(63, 31).unwrap().ch, 'A');
    assert_eq!(p.cell(0, 31).unwrap().ch, 'B');
    assert_eq!(p.cell(63, 0).unwrap().half, 1);
    p.draw(0xFFFF8406, 0x4100, "Z").unwrap();
    assert_eq!(
        p.cell(63, 31).unwrap().ch,
        'Z',
        "VRAM patterns update even before plane DMA"
    );
    assert!(p.draw(0x9280, 0xC100, "X").is_err());
    assert!(p.draw(0xFFFF8407, 0xC100, "X").is_err());
}
