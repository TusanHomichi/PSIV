use super::*;

#[test]
fn mapping_stride_signed_offsets_and_all_size_encodings() {
    let data = [
        1, 0x5a, 0xfe, 0x0f, 0, 0x20, 0x80, 0x7f, 0, 3, 0, 0x30, 0x1c, 0xdc,
    ];
    let mapping = decode_mapping(&data, 0).unwrap();
    assert_eq!(mapping.header_byte, 0x5a);
    assert_eq!(mapping.pieces.len(), 2);
    let a = mapping.pieces[0];
    let b = mapping.pieces[1];
    assert_eq!((a.x, a.y, a.tile_word), (-128, -2, 0x20));
    assert_eq!((b.x, b.y, b.tile_word), (28, 0, 0x30));
    // Byte 5 is the unused mirrored-X byte; it must not shift the next piece.
    assert_eq!((b.width_tiles(), b.height_tiles()), (1, 4));
    for size_byte in 0..16 {
        let p = Piece { size_byte, ..a };
        assert_eq!(
            (p.width_tiles(), p.height_tiles()),
            (
                usize::from(size_byte / 4 + 1),
                usize::from(size_byte % 4 + 1)
            )
        );
    }
    assert!(decode_mapping(&data[..13], 0).is_err());
}

#[test]
fn odd_sequence_counts_align_pointers_and_ticks_do_not_wrap() {
    let data = [
        0x83, 0, 1, 255, 0, 0, 0x20, 0, 0, 0, 0x20, 8, 0, 0, 0x20, 16,
    ];
    let sequence = decode_sequence(&data, 0).unwrap();
    assert!(sequence.per_frame_durations);
    assert_eq!(sequence.frames, [(0x2000, 0), (0x2008, 1), (0x2010, 255)]);
    let sheet = build_sheet(
        vec![(
            "down".into(),
            sequence,
            (1..=3).map(|p| Frame { pixels: vec![p] }).collect(),
        )],
        Bounds {
            left: 0,
            top: 0,
            right: 1,
            bottom: 1,
        },
        &[[0; 3]; 16],
        2,
    );
    assert_eq!(sheet.sequences["walk_down"].durations, [1, 2, 256]);
    assert_eq!(sheet.sequences["idle_down"].durations, [1]);
}

#[test]
fn pattern_arithmetic_and_every_cram_selector() {
    assert_eq!(effective_tile_word(0x400, 0x18, 0), 0x418);
    for (props, line) in [(0, 0), (0x20, 1), (0x40, 2), (0x60, 3)] {
        assert_eq!(tile_props_line(props), Some(line));
        assert_eq!(
            effective_tile_word(0x400, 0x18, props),
            0x418 | u16::from(props) << 8
        );
    }
    assert_eq!(tile_props_line(0x21), None);
}

#[test]
fn asymmetric_pixels_flip_inside_tiles_and_transparency_preserves_earlier_pieces() {
    let mut source = TileSource::default();
    source.insert(0, [3; 64]);
    let mut tile = [0; 64];
    tile[0] = 1;
    tile[63] = 2;
    source.insert(1, tile);
    let bounds = Bounds {
        left: 0,
        top: 0,
        right: 8,
        bottom: 8,
    };
    let reading = Reading {
        art_tile: 0,
        tile_props: 0,
        streamed: false,
    };
    for (word, first, last) in [
        (1, 0, 63),
        (1 | HFLIP_BIT, 7, 56),
        (1 | VFLIP_BIT, 56, 7),
        (1 | HFLIP_BIT | VFLIP_BIT, 63, 0),
    ] {
        let piece = |tile_word| Piece {
            y: 0,
            x: 0,
            size_byte: 0,
            tile_word,
        };
        let mapping = Mapping {
            header_byte: 0,
            pieces: vec![piece(0), piece(word)],
        };
        let (frame, missing) = compose_frame(&mapping, &source, bounds, reading, None);
        assert!(missing.is_empty());
        let mut expected = vec![3; 64];
        expected[first] = 1;
        expected[last] = 2;
        assert_eq!(frame.pixels, expected, "word {word:#x}");
    }
}

#[test]
fn union_geometry_preserves_the_objects_origin_and_refuses_an_empty_set() {
    let mapping = Mapping {
        header_byte: 0,
        pieces: vec![
            Piece {
                x: -4,
                y: -2,
                size_byte: 7,
                tile_word: 0,
            },
            Piece {
                x: 20,
                y: 6,
                size_byte: 0,
                tile_word: 0,
            },
        ],
    };
    let a = mapping_box(&mapping);
    assert_eq!(
        a,
        Bounds {
            left: -4,
            top: -2,
            right: 28,
            bottom: 30
        }
    );
    let b = Bounds {
        left: -8,
        top: 0,
        right: 16,
        bottom: 32,
    };
    assert_eq!(
        union_box([a, b]).unwrap(),
        Bounds {
            left: -8,
            top: -2,
            right: 28,
            bottom: 32
        }
    );
    assert!(union_box([]).is_err());
}

#[test]
fn strip_interleaves_multiple_rows_of_distinct_frames() {
    let sheet = Sheet {
        frames: vec![
            Frame {
                pixels: vec![1, 2, 3, 4],
            },
            Frame {
                pixels: vec![5, 6, 7, 8],
            },
        ],
        sequences: Default::default(),
        bounds: Bounds {
            left: 0,
            top: 0,
            right: 2,
            bottom: 2,
        },
        palette: vec![[0; 3]; 16],
        palette_line: 0,
        mirrors: Default::default(),
    };
    assert_eq!(sheet.strip(), (4, 2, vec![1, 2, 5, 6, 3, 4, 7, 8]));
}
