//! Shared assertions for the battle image extraction tests (#550).
//! Expectations come from local source bytes, never committed image digests.

use std::path::PathBuf;

use crate::{gfx, png};

/// Owned output, removed even when an assertion fails.
pub(super) struct Output(pub PathBuf);

impl Output {
    pub fn new(name: &str) -> Self {
        let path = crate::test_support::scratch(name);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(super) fn word(rom: &[u8], at: usize) -> u16 {
    u16::from_be_bytes(rom[at..at + 2].try_into().unwrap())
}

pub(super) fn palette(rom: &[u8], at: usize, count: usize) -> Vec<[u8; 3]> {
    (0..count)
        .map(|i| gfx::Color::decode(word(rom, at + i * 2)).rgb())
        .collect()
}

/// Pixel-order oracle: read packed nibbles directly, independent of the
/// production tile decoder and compositor. The mapping decoder has its own
/// tests and full-pack parity covers its inputs and consumed lengths.
pub(super) fn plane(words: &[u16], columns: usize, raw: &[u8], base: u16) -> Vec<u8> {
    let width = columns * 8;
    (0..words.len() / columns * 8)
        .flat_map(|y| {
            (0..width).map(move |x| {
                let w = words[y / 8 * columns + x / 8];
                let sx = if w & 0x800 != 0 { 7 - x % 8 } else { x % 8 };
                let sy = if w & 0x1000 != 0 { 7 - y % 8 } else { y % 8 };
                let at = usize::from((w & 0x7ff) - base) * 32 + sy * 4 + sx / 2;
                let p = (raw[at] >> (4 * (1 - sx % 2))) & 15;
                if p == 0 {
                    0
                } else {
                    ((w >> 13) & 3) as u8 * 16 + p
                }
            })
        })
        .collect()
}

/// Compare decoded pixels, palette and alpha separately so that both content
/// and colour mutations fail, regardless of the deflate implementation.
pub(super) fn assert_png(bytes: &[u8], size: (usize, usize), pixels: &[u8], palette: &[[u8; 3]]) {
    let image = png::decode(bytes).unwrap();
    assert_eq!(
        (image.width, image.height, image.colour),
        (size.0, size.1, 3)
    );
    assert!(
        image.pixels == pixels,
        "decoded image pixels differ from source composition"
    );
    assert_eq!(
        image.plte,
        palette.as_flattened(),
        "decoded palette differs from source"
    );
    assert_eq!(image.trns, &[0], "only colour zero is transparent");
}

#[test]
fn decoded_image_assertions_reject_palette_and_composition_mutations() {
    let palette = [[0, 0, 0], [20, 40, 60]];
    let pixels = [0, 1];
    let good = png::encode_indexed(2, 1, &pixels, &palette, &[0]).unwrap();
    assert_png(&good, (2, 1), &pixels, &palette);
    let bad_palette = [[0, 0, 0], [21, 40, 60]];
    for bad in [
        png::encode_indexed(2, 1, &pixels, &bad_palette, &[0]).unwrap(),
        png::encode_indexed(2, 1, &[1, 0], &palette, &[0]).unwrap(),
    ] {
        assert!(std::panic::catch_unwind(|| assert_png(&bad, (2, 1), &pixels, &palette)).is_err());
    }
}
