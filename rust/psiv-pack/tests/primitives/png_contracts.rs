//! Image identity and refusal contracts from test_png.py and TestPngWriter.
use flate2::{Compression, write::ZlibEncoder};
use psiv_pack::png::*;
use std::io::Write;

const PALETTE: [[u8; 3]; 4] = [[0, 0, 0], [255, 0, 0], [0, 255, 0], [0, 0, 255]];
const PIXELS: [u8; 16] = [0, 1, 2, 3, 3, 2, 1, 0, 1, 1, 2, 2, 3, 3, 0, 0];

/// Replace a chunk with a valid CRC, so malformed-content tests reach the
/// format/scanline checks rather than merely exercising the CRC refusal.
fn replace(data: &[u8], wanted: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut out = data[..8].to_vec();
    let mut pos = 8;
    while pos < data.len() {
        let n = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
        let kind = data[pos + 4..pos + 8].try_into().unwrap();
        let kind: &[u8; 4] = kind;
        let bytes = if kind == wanted {
            payload
        } else {
            &data[pos + 8..pos + 8 + n]
        };
        out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
        out.extend_from_slice(kind);
        out.extend_from_slice(bytes);
        let mut crc = flate2::Crc::new();
        crc.update(kind);
        crc.update(bytes);
        out.extend_from_slice(&crc.sum().to_be_bytes());
        pos += n + 12;
    }
    out
}

#[test]
fn every_pixel_palette_alpha_and_shape_contribute_to_image_identity() {
    let original = encode_indexed(4, 4, &PIXELS, &PALETTE, &[0]).unwrap();
    let expected = image_sha256(&original).unwrap();
    for at in 0..PIXELS.len() {
        let mut pixels = PIXELS;
        pixels[at] = (pixels[at] + 1) % 4;
        assert_ne!(
            image_sha256(&encode_indexed(4, 4, &pixels, &PALETTE, &[0]).unwrap()).unwrap(),
            expected
        );
    }
    let mut palette = PALETTE;
    palette[3][2] = 254;
    assert_ne!(
        image_sha256(&encode_indexed(4, 4, &PIXELS, &palette, &[0]).unwrap()).unwrap(),
        expected
    );
    for alpha in [&[][..], &[1][..], &[0, 1][..]] {
        assert_ne!(
            image_sha256(&encode_indexed(4, 4, &PIXELS, &PALETTE, alpha).unwrap()).unwrap(),
            expected
        );
    }
    for (w, h) in [(2, 8), (8, 2), (16, 1)] {
        assert_ne!(
            image_sha256(&encode_indexed(w, h, &PIXELS, &PALETTE, &[0]).unwrap()).unwrap(),
            expected
        );
    }
}

#[test]
fn encoders_preserve_pixels_palette_dimensions_and_trim_alpha() {
    let data = encode_indexed(4, 4, &PIXELS, &PALETTE, &[1]).unwrap();
    let image = decode(&data).unwrap();
    assert_eq!((image.width, image.height, image.colour), (4, 4, 3));
    assert_eq!(image.header[8..], [8, 3, 0, 0, 0]);
    assert_eq!(image.plte, PALETTE.as_flattened());
    assert_eq!(image.pixels, PIXELS);
    assert_eq!(image.trns, [255, 0]);
    let pixels: Vec<u8> = (0..12).collect();
    let data = encode_rgb(2, 2, &pixels).unwrap();
    let image = decode(&data).unwrap();
    assert_eq!((image.width, image.height, image.colour), (2, 2, 2));
    assert_eq!(image.header[8..], [8, 2, 0, 0, 0]);
    assert_eq!(image.pixels, pixels);
    assert!(image.plte.is_empty() && image.trns.is_empty());
    for palette in [&[][..], &[[0; 3]; 257][..]] {
        assert!(encode_indexed(1, 1, &[0], palette, &[]).is_err());
    }
    assert!(encode_rgb(0, 1, &[]).is_err());
    assert!(encode_rgb(1, 0, &[]).is_err());
}

#[test]
fn corruption_unsupported_formats_and_malformed_rows_are_refused() {
    let data = encode_indexed(4, 4, &PIXELS, &PALETTE, &[0]).unwrap();
    let mut damaged = data.clone();
    let at = damaged.len() - 20;
    damaged[at] ^= 255;
    assert!(image_sha256(&damaged).is_err());
    assert!(image_sha256(&data[8..]).is_err());
    assert!(image_sha256(&data[..data.len() - 30]).is_err());
    for (at, value) in [(8, 4), (9, 1), (12, 1)] {
        let mut header = decode(&data).unwrap().header.to_vec();
        header[at] = value;
        assert!(image_sha256(&replace(&data, b"IHDR", &header)).is_err());
    }
    assert!(image_sha256(&replace(&data, b"IHDR", &[0; 12])).is_err());
    assert!(image_sha256(&replace(&data, b"IDAT", b"not zlib")).is_err());
    for raw in [&[5, 0, 1, 2, 3][..], &[0, 0, 1][..]] {
        let mut compressed = ZlibEncoder::new(Vec::new(), Compression::default());
        compressed.write_all(raw).unwrap();
        assert!(image_sha256(&replace(&data, b"IDAT", &compressed.finish().unwrap())).is_err());
    }
    // Valid dimensions and a valid zlib stream/CRC reach filter refusal itself.
    let mut raw = vec![0; 20];
    raw[0] = 5;
    let mut compressed = ZlibEncoder::new(Vec::new(), Compression::default());
    compressed.write_all(&raw).unwrap();
    assert!(
        image_sha256(&replace(&data, b"IDAT", &compressed.finish().unwrap()))
            .unwrap_err()
            .0
            .contains("filter type 5")
    );
}
