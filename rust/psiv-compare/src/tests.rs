//! The comparison without Godot: crop, region, normalization and decode.

use psiv_pack::png::{Image, encode_indexed, encode_rgb};

use super::*;

const REGION: Region = Region {
    x: 144,
    y: 104,
    width: 112,
    height: 96,
};

fn frame() -> Rgb {
    Rgb {
        width: WIDTH,
        height: HEIGHT,
        pixels: vec![0; WIDTH * HEIGHT * 3],
    }
}

fn paint(rgb: &Rgb, x: usize, y: usize, colour: [u8; 3]) -> Rgb {
    let mut out = rgb.clone();
    let start = (y * rgb.width + x) * 3;
    out.pixels[start..start + 3].copy_from_slice(&colour);
    out
}

fn png(rgb: &Rgb) -> Vec<u8> {
    encode_rgb(rgb.width as u32, rgb.height as u32, &rgb.pixels).unwrap()
}

#[test]
fn a_difference_outside_the_region_does_not_count() {
    let oracle = frame();
    let godot = paint(&oracle, 10, 10, [255, 0, 0]);
    assert!(rmse(&godot, &oracle).unwrap() > 0.0);
    let (left, right) = (
        crop(&godot, REGION).unwrap(),
        crop(&oracle, REGION).unwrap(),
    );
    assert_eq!(rmse(&left, &right).unwrap(), 0.0);
}

#[test]
fn a_difference_inside_the_region_counts() {
    let oracle = frame();
    // Both corners and the middle: an off-by-one edge would miss one.
    for (x, y) in [(144, 104), (255, 199), (200, 150)] {
        let godot = paint(&oracle, x, y, [255, 0, 0]);
        let (left, right) = (
            crop(&godot, REGION).unwrap(),
            crop(&oracle, REGION).unwrap(),
        );
        assert!(rmse(&left, &right).unwrap() > 0.0, "({x},{y})");
    }
    // One pixel past each edge is outside.
    for (x, y) in [(143, 104), (256, 199), (144, 103), (255, 200)] {
        let godot = paint(&oracle, x, y, [255, 0, 0]);
        let (left, right) = (
            crop(&godot, REGION).unwrap(),
            crop(&oracle, REGION).unwrap(),
        );
        assert_eq!(rmse(&left, &right).unwrap(), 0.0, "({x},{y})");
    }
}

#[test]
fn the_crop_is_the_rectangle() {
    let cropped = crop(&paint(&frame(), 144, 104, [1, 2, 3]), REGION).unwrap();
    assert_eq!((cropped.width, cropped.height), (112, 96));
    assert_eq!(cropped.pixels.len(), 112 * 96 * 3);
    assert_eq!(&cropped.pixels[..3], &[1, 2, 3]);
    let last = paint(&frame(), 255, 199, [4, 5, 6]);
    assert_eq!(
        &crop(&last, REGION).unwrap().pixels[112 * 96 * 3 - 3..],
        &[4, 5, 6]
    );
}

#[test]
fn a_region_outside_the_frame_or_malformed_is_refused() {
    for text in [
        "300,0,40,10",
        "0,0,0,10",
        "0,0,10,0",
        "-1,0,10,10",
        "0,220,10,10",
        "0,0,321,1",
        "1,2,3",
        "1,2,3,4,5",
        "a,b,c,d",
        "",
    ] {
        assert!(Region::parse(text).is_err(), "{text:?}");
    }
    assert_eq!(Region::parse("144,104,112,96").unwrap(), REGION);
    assert_eq!(
        Region::parse("0,0,320,224").unwrap().to_string(),
        "0,0,320,224"
    );
}

#[test]
fn a_3x_capture_samples_one_pixel_from_each_block_of_its_centred_surface() {
    // A 1280x800 viewport: the 960x672 surface sits at (160,64).
    let mut capture = Rgb {
        width: 1280,
        height: 800,
        pixels: vec![99; 1280 * 800 * 3],
    };
    let mut expected = frame();
    for (x, y, colour) in [
        (0, 0, [1, 2, 3]),
        (319, 223, [4, 5, 6]),
        (17, 40, [7, 8, 9]),
    ] {
        capture = paint(&capture, 160 + x * 3, 64 + y * 3, colour);
        // The block's other pixels are not sampled.
        capture = paint(&capture, 160 + x * 3 + 1, 64 + y * 3 + 2, [200, 200, 200]);
        expected = paint(&expected, x, y, colour);
    }
    let mut normalized = normalize_godot(&capture).unwrap();
    // Everything else reads the fill.
    for (index, value) in normalized.pixels.iter_mut().enumerate() {
        if *value == 99 {
            *value = 0;
        } else {
            assert_ne!(expected.pixels[index], 0, "byte {index}");
        }
    }
    assert_eq!(normalized, expected);
    assert_eq!(normalize_godot(&frame()).unwrap(), frame());
    let small = Rgb {
        width: 959,
        height: 672,
        pixels: vec![0; 959 * 672 * 3],
    };
    assert!(normalize_godot(&small).is_err());
}

#[test]
fn rmse_is_the_root_mean_square_over_every_channel() {
    let one = Rgb {
        width: 2,
        height: 1,
        pixels: vec![0, 0, 0, 0, 0, 0],
    };
    let other = Rgb {
        width: 2,
        height: 1,
        pixels: vec![6, 0, 0, 0, 0, 0],
    };
    assert_eq!(rmse(&one, &other).unwrap(), 6.0_f64.sqrt());
    assert!(rmse(&one, &frame()).is_err());
}

#[test]
fn alpha_premultiplies_onto_black() {
    let rgba = Image {
        header: &[],
        width: 3,
        height: 1,
        colour: 6,
        plte: &[],
        trns: &[],
        pixels: vec![200, 100, 50, 0, 200, 100, 50, 255, 200, 100, 50, 128],
    };
    assert_eq!(
        rgb(&rgba).unwrap().pixels,
        vec![0, 0, 0, 200, 100, 50, 100, 50, 25]
    );
    let indexed = encode_indexed(2, 1, &[0, 1], &[[10, 20, 30], [40, 50, 60]], &[0]).unwrap();
    assert_eq!(
        read_rgb(&indexed).unwrap().pixels,
        vec![0, 0, 0, 40, 50, 60]
    );
    let grey = Image {
        colour: 0,
        pixels: vec![1, 2, 3],
        ..rgba
    };
    assert!(rgb(&grey).is_err());
}

#[test]
fn the_report_line_names_the_region_and_the_error() {
    let oracle = frame();
    let godot = paint(&oracle, 10, 10, [255, 0, 0]);
    let whole = compare(&png(&godot), &png(&oracle), None).unwrap();
    assert!(
        whole.starts_with("godot=320x224 normalized=320x224 oracle=320x224 rmse="),
        "{whole}"
    );
    assert!(!whole.ends_with("rmse=0.000000"), "{whole}");
    assert_eq!(
        compare(&png(&godot), &png(&oracle), Some(REGION)).unwrap(),
        "godot=320x224 normalized=320x224 oracle=320x224 region=144,104,112,96 rmse=0.000000"
    );
    let wrong = Rgb {
        width: 2,
        height: 2,
        pixels: vec![0; 12],
    };
    let error = compare(&png(&godot), &png(&wrong), None).unwrap_err();
    assert_eq!(error.0, "oracle frame is 2x2, expected 320x224");
    assert!(compare(b"not a png", &png(&oracle), None).is_err());
}
