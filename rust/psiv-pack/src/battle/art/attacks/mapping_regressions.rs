//! ROM-free extractor checks from tests/test_gate_battle_attack.py (#551).
//! Non-square sprites catch a row/column stride swap that square sprites miss.
use super::*;

#[test]
fn non_square_column_stride_both_flips_and_transparent_black_sat_ownership() {
    let canvas = Canvas {
        min_x: 0,
        min_y: 0,
        width: 16,
        height: 24,
    };
    let sprite = |tile: u8, width: u8, height: u8, flip, priority| {
        Json::object()
            .with("tile_index", tile)
            .with("width_tiles", width)
            .with("height_tiles", height)
            .with("h_flip", flip)
            .with("v_flip", flip)
            .with("x", 0u8)
            .with("y", 0u8)
            .with("priority", priority)
    };
    let tiles: Vec<_> = (1..=6).map(|i| [i; 64]).collect();
    let palette = [[0; 3]; 16];
    for (flip, expected) in [(false, [1, 4, 2, 5, 3, 6]), (true, [6, 3, 5, 2, 4, 1])] {
        let record = Json::object().with("entries", vec![sprite(0, 2, 3, flip, false)]);
        let bytes = render(&tiles, &record, canvas, (0, 0), &palette, None).unwrap();
        let p = png::decode(&bytes).unwrap();
        assert_eq!(
            [
                p.pixels[0],
                p.pixels[8],
                p.pixels[128],
                p.pixels[136],
                p.pixels[256],
                p.pixels[264]
            ],
            expected
        );
    }
    let mut tiles = [[1; 64], [2; 64]];
    tiles[0][0] = 0;
    for first_priority in [false, true] {
        let record = Json::object().with(
            "entries",
            vec![
                sprite(0, 1, 1, false, first_priority),
                sprite(1, 1, 1, false, !first_priority),
            ],
        );
        let bytes = |priority| {
            render(
                &tiles,
                &record,
                Canvas {
                    width: 8,
                    height: 8,
                    ..canvas
                },
                (0, 0),
                &palette,
                priority,
            )
            .unwrap()
        };
        let combined = png::decode(&bytes(None)).unwrap().pixels;
        assert_eq!(combined, [vec![2], vec![1; 63]].concat());
        let front = png::decode(&bytes(Some(first_priority))).unwrap().pixels;
        let rear = png::decode(&bytes(Some(!first_priority))).unwrap().pixels;
        assert_eq!(front, [vec![0], vec![1; 63]].concat());
        assert_eq!(rear, [vec![2], vec![0; 63]].concat());
        assert!(front.iter().zip(&rear).all(|(&a, &b)| a == 0 || b == 0));
        assert_eq!(
            front
                .iter()
                .zip(&rear)
                .map(|(a, b)| a | b)
                .collect::<Vec<_>>(),
            combined
        );
    }
}
