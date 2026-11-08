//! #64: the Zol slugs' Fusion, frame by frame, as oracle tape 33 shows it.
//!
//! Formation `$D2` seats the two slugs at positions `$8E` and `$9A`
//! (columns 14 and 26, screen x 112 and 208). Tape 33's frames, with `t`
//! counted from the objects' first one (f25168), fit these pieces with no
//! pixel apart (the frames and the fit are in the PR that added this test).
//! No pack is needed: the art is the pack's, the timeline is the runtime's.

use psiv_runtime::{Fusion, FusionPiece};

fn piece(x: i16, frame: u8, flipped: bool) -> FusionPiece {
    FusionPiece {
        x,
        y: 120,
        frame,
        flipped,
    }
}

fn d2() -> Fusion {
    Fusion::new(112, 208)
}

#[test]
fn the_windup_stands_six_frames_then_flattens_six() {
    for t in 0..6 {
        assert_eq!(
            d2().pieces(t),
            [piece(112, 0, false), piece(208, 0, false)],
            "t={t}"
        );
    }
    for t in 6..12 {
        assert_eq!(
            d2().pieces(t),
            [piece(112, 1, false), piece(208, 1, false)],
            "t={t}"
        );
    }
}

#[test]
fn the_slide_moves_a_pixel_a_frame_and_mirrors_every_other_six() {
    // f25180, f25186, f25192, f25200, f25227.
    assert_eq!(
        d2().pieces(12),
        [piece(113, 1, false), piece(207, 1, false)]
    );
    assert_eq!(d2().pieces(18), [piece(119, 1, true), piece(201, 1, true)]);
    assert_eq!(
        d2().pieces(24),
        [piece(125, 1, false), piece(195, 1, false)]
    );
    assert_eq!(d2().pieces(32), [piece(133, 1, true), piece(187, 1, true)]);
    assert_eq!(d2().pieces(59), [piece(160, 1, true), piece(160, 1, true)]);
}

#[test]
fn the_left_piece_goes_and_the_right_grows_into_the_metaslug() {
    // f25228: both arrived; BattleObj_Fusion2 deletes itself.
    assert_eq!(d2().pieces(60), [piece(160, 1, false)]);
    assert_eq!(d2().growth_start(), 61);
    // f25229, f25235, f25241, f25247; the seat at f25248.
    assert_eq!(d2().pieces(61), [piece(160, 0, false)]);
    assert_eq!(d2().pieces(66), [piece(160, 0, false)]);
    assert_eq!(d2().pieces(67), [piece(160, 2, false)]);
    assert_eq!(d2().pieces(73), [piece(160, 3, false)]);
    assert_eq!(d2().pieces(79), [piece(160, 4, false)]);
    assert_eq!(d2().seat(), 80);
    assert!(d2().pieces(80).is_empty());
}

#[test]
fn growth_waits_for_the_later_piece_and_object_order_decides_the_frame() {
    // The right piece is BattleObj_Fusion and runs first. Arriving last, it
    // sees both arrivals at once; arriving first, it sees the left one a
    // frame later.
    let right_last = Fusion::new(150, 200);
    assert_eq!(right_last.growth_start(), 11 + 40 + 1);
    let left_last = Fusion::new(100, 170);
    assert_eq!(left_last.growth_start(), 11 + 60 + 2);
    // Either order of the arguments is the same pair.
    assert_eq!(Fusion::new(200, 150), right_last);
}
