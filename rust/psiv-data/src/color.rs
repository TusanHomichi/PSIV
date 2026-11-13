//! CRAM colours: what a palette word looks like on screen, and the indexed
//! PNGs a renderer draws from the live palette.
//!
//! The ramps are GPGX's RGB565 output levels, measured in
//! `docs/field/COLOR_PIPELINE.md`; the pack, the
//! field shell and the battle shell all read them from here.
//! [`index_png`] is the one way an indexed pack PNG becomes a texture whose
//! red channel is the pixel's palette index, so a renderer looks a colour up in
//! the runtime's CRAM (`psiv_runtime::Runtime::cram`) instead of drawing the
//! colours baked into the file.

/// Red's eight levels, by the three bits `----bbb-ggg-rrr-` gives it.
pub const RAMP_R: [u8; 8] = [0, 32, 65, 98, 139, 172, 205, 238];
/// Green's eight levels.
pub const RAMP_G: [u8; 8] = [0, 32, 68, 101, 137, 170, 206, 238];
/// Blue's eight levels.
pub const RAMP_B: [u8; 8] = [0, 32, 65, 98, 139, 172, 205, 238];

/// The colour a CRAM word shows; bits outside `$0EEE` are not read.
#[must_use]
pub fn cram_rgb(word: u16) -> [u8; 3] {
    [
        RAMP_R[usize::from((word >> 1) & 7)],
        RAMP_G[usize::from((word >> 5) & 7)],
        RAMP_B[usize::from((word >> 9) & 7)],
    ]
}

const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
const COLOR_TYPE_INDEXED: u8 = 3;

/// The CRC-32 a PNG chunk carries over its type and payload.
fn crc32(parts: &[&[u8]]) -> u32 {
    let mut crc = !0u32;
    for part in parts {
        for &byte in *part {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    0xEDB8_8320 ^ (crc >> 1)
                } else {
                    crc >> 1
                };
            }
        }
    }
    !crc
}

/// An indexed PNG with its palette replaced by `(index, 0, 0)`, its IDAT,
/// dimensions and transparency kept. Decoded, each pixel's red channel is its
/// palette index and its alpha the file's transparency, so the renderer reads
/// the index rather than guessing a slot from an RGB value two slots can share.
///
/// # Errors
///
/// A file that is not a PNG, is truncated, is not indexed (colour type 3) or
/// has no palette.
pub fn index_png(data: &[u8]) -> Result<Vec<u8>, String> {
    if !data.starts_with(SIGNATURE) {
        return Err("palette-index input is not a PNG".into());
    }
    let mut out = Vec::with_capacity(data.len());
    out.extend_from_slice(SIGNATURE);
    let mut at = SIGNATURE.len();
    let mut found = false;
    while at < data.len() {
        let truncated = || "palette-index input is truncated".to_owned();
        let head: [u8; 4] = data
            .get(at..at + 4)
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or_else(truncated)?;
        let size = u32::from_be_bytes(head) as usize;
        let whole = data.get(at..at + size + 12).ok_or_else(truncated)?;
        let kind = &whole[4..8];
        if kind == b"IHDR" && whole.get(17) != Some(&COLOR_TYPE_INDEXED) {
            return Err("palette-index input must be an indexed PNG".into());
        }
        if kind == b"PLTE" {
            let palette: Vec<u8> = (0..size / 3).flat_map(|i| [i as u8, 0, 0]).collect();
            out.extend_from_slice(&(palette.len() as u32).to_be_bytes());
            out.extend_from_slice(b"PLTE");
            out.extend_from_slice(&palette);
            out.extend_from_slice(&crc32(&[b"PLTE", &palette]).to_be_bytes());
            found = true;
        } else {
            out.extend_from_slice(whole);
        }
        at += size + 12;
    }
    if !found {
        return Err("palette-index input has no palette".into());
    }
    Ok(out)
}

/// Which CRAM slot each palette index of an indexed pack PNG draws from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexLayout {
    /// A sprite sheet: index `i` of 0-15 is word `i` of CRAM line `line`;
    /// index 0 is the VDP's transparent colour.
    Line(u8),
    /// A scene panel (`psiv_tools/presentation_pack.py`, `emit_panel`): index
    /// 0 transparent, 1-16 layer A's colours 0-15 on line `a`, 17-32 layer
    /// B's on line `b`. A layer's line is its art tile's palette select, bits
    /// 13-14.
    Panel {
        /// Layer A's line.
        a: u8,
        /// Layer B's line.
        b: u8,
    },
}

impl IndexLayout {
    /// The CRAM slot `index` draws, or `None` for a transparent index.
    #[must_use]
    pub fn slot(self, index: u8) -> Option<usize> {
        let line = |line: u8, colour: u8| usize::from(line & 3) * 16 + usize::from(colour);
        match self {
            Self::Line(_) if index == 0 || index > 15 => None,
            Self::Line(l) => Some(line(l, index)),
            Self::Panel { a, .. } if (1..=16).contains(&index) => Some(line(a, index - 1)),
            Self::Panel { b, .. } if (17..=32).contains(&index) => Some(line(b, index - 17)),
            Self::Panel { .. } => None,
        }
    }

    /// The palette select of an art tile word (`ccc` of `pccv hnnn nnnn nnnn`).
    #[must_use]
    pub const fn art_tile_line(art_tile: u16) -> u8 {
        ((art_tile >> 13) & 3) as u8
    }
}

/// RGBA pixels for an image of palette indices drawn from `cram`: each index
/// goes through `slot_of` to a CRAM word, and an index it maps to `None`, or a
/// pixel whose alpha is zero, stays transparent. `indices` and `alpha` are one
/// byte per pixel.
#[must_use]
pub fn recolor(
    indices: &[u8],
    alpha: &[u8],
    slot_of: impl Fn(u8) -> Option<usize>,
    cram: &[u16],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(indices.len() * 4);
    for (&index, &a) in indices.iter().zip(alpha) {
        match slot_of(index).and_then(|slot| cram.get(slot)) {
            Some(&word) if a != 0 => {
                let [r, g, b] = cram_rgb(word);
                out.extend_from_slice(&[r, g, b, a]);
            }
            _ => out.extend_from_slice(&[0, 0, 0, 0]),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A two-pixel indexed PNG with palette `(9,9,9) (200,10,10)`, index 0
    /// transparent, stored uncompressed (zlib stored block).
    fn tiny_indexed() -> Vec<u8> {
        fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], payload: &[u8]) {
            out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
            out.extend_from_slice(kind);
            out.extend_from_slice(payload);
            out.extend_from_slice(&crc32(&[kind, payload]).to_be_bytes());
        }
        let mut out = SIGNATURE.to_vec();
        chunk(&mut out, b"IHDR", &[0, 0, 0, 2, 0, 0, 0, 1, 8, 3, 0, 0, 0]);
        chunk(&mut out, b"PLTE", &[9, 9, 9, 200, 10, 10]);
        chunk(&mut out, b"tRNS", &[0]);
        // filter 0, pixels 0 and 1; adler32 of [0, 0, 1] is 0x00040002.
        chunk(
            &mut out,
            b"IDAT",
            &[
                0x78, 0x01, 0x01, 3, 0, 0xFC, 0xFF, 0, 0, 1, 0x00, 0x04, 0x00, 0x02,
            ],
        );
        chunk(&mut out, b"IEND", &[]);
        out
    }

    #[test]
    fn the_crc_is_png_s() {
        // The IEND chunk every PNG ends with.
        assert_eq!(crc32(&[b"IEND"]), 0xAE42_6082);
    }

    #[test]
    fn index_png_rewrites_only_the_palette() {
        let original = tiny_indexed();
        let indexed = index_png(&original).unwrap();
        assert_eq!(indexed.len(), original.len());
        let plte = indexed.windows(4).position(|w| w == b"PLTE").unwrap();
        assert_eq!(&indexed[plte + 4..plte + 10], &[0, 0, 0, 1, 0, 0]);
        assert_eq!(
            &indexed[plte + 10..plte + 14],
            &crc32(&[b"PLTE", &[0, 0, 0, 1, 0, 0]]).to_be_bytes()
        );
        // Everything else is byte for byte the same.
        assert_eq!(&indexed[..plte + 4], &original[..plte + 4]);
        assert_eq!(&indexed[plte + 14..], &original[plte + 14..]);
    }

    #[test]
    fn index_png_refuses_what_it_cannot_index() {
        assert!(index_png(b"not a png").unwrap_err().contains("not a PNG"));
        let original = tiny_indexed();
        assert!(
            index_png(&original[..original.len() - 2])
                .unwrap_err()
                .contains("truncated")
        );
        let mut rgb = original.clone();
        rgb[8 + 8 + 9] = 2;
        assert!(index_png(&rgb).unwrap_err().contains("indexed"));
    }

    #[test]
    fn cram_rgb_reads_three_bits_a_component() {
        assert_eq!(cram_rgb(0x0000), [0, 0, 0]);
        assert_eq!(cram_rgb(0x0EEE), [238, 238, 238]);
        // Red 1, green 2, blue 3; the unread bits change nothing.
        assert_eq!(cram_rgb(0x0642), [32, 68, 98]);
        assert_eq!(cram_rgb(0xF643), [32, 68, 98]);
    }

    #[test]
    fn a_layout_maps_each_index_to_its_line() {
        let sprite = IndexLayout::Line(2);
        assert_eq!(sprite.slot(0), None, "index 0 is transparent");
        assert_eq!(sprite.slot(1), Some(33));
        assert_eq!(sprite.slot(15), Some(47));
        assert_eq!(sprite.slot(16), None);
        // Panel 0's layers: art tiles $8100 (line 0) and $A100 (line 1).
        let panel = IndexLayout::Panel {
            a: IndexLayout::art_tile_line(0x8100),
            b: IndexLayout::art_tile_line(0xA100),
        };
        assert_eq!(panel, IndexLayout::Panel { a: 0, b: 1 });
        assert_eq!(panel.slot(0), None);
        assert_eq!(panel.slot(1), Some(0), "A's colour 0");
        assert_eq!(panel.slot(16), Some(15));
        assert_eq!(panel.slot(17), Some(16), "B's colour 0");
        assert_eq!(panel.slot(32), Some(31));
        assert_eq!(panel.slot(33), None);
        // Bit 12 (vertical flip) is not a palette bit.
        assert_eq!(IndexLayout::art_tile_line(0x7000), 3);
        assert_eq!(IndexLayout::art_tile_line(0x1000), 0);
    }

    #[test]
    fn recolor_reads_each_index_through_its_slot() {
        let mut cram = [0u16; 64];
        cram[16 + 1] = 0x000E;
        cram[32 + 1] = 0x0E00;
        // Index 1 of a line-1 image is slot 17; of a line-2 image, slot 33.
        let line = |base: usize| move |index: u8| (index != 0).then_some(base + usize::from(index));
        assert_eq!(
            recolor(&[1, 0], &[255, 0], line(16), &cram),
            [238, 0, 0, 255, 0, 0, 0, 0]
        );
        assert_eq!(recolor(&[1], &[255], line(32), &cram), [0, 0, 238, 255]);
        // A transparent pixel stays so whatever its index.
        assert_eq!(recolor(&[1], &[0], line(16), &cram), [0, 0, 0, 0]);
        // A slot past the buffer draws nothing rather than a wrong colour.
        assert_eq!(recolor(&[1], &[255], line(64), &cram), [0, 0, 0, 0]);
    }
    fn vehicle_recolor_failures(line: u8) -> Vec<&'static str> {
        let mut before = [0x0222; 64];
        before[49] = 0x000E;
        let mut after = before;
        after[49] = 0x0E00;
        let indices = [1, 0, 2, 1];
        let alpha = [255, 255, 0, 255];
        let draw = |line, words: &[u16]| {
            recolor(&indices, &alpha, |i| IndexLayout::Line(line).slot(i), words)
        };
        let first = draw(line, &before);
        let changed = draw(line, &after);
        let mut failures = Vec::new();
        if first[..4] != [238, 0, 0, 255] || changed[..4] != [0, 0, 238, 255] {
            failures.push("vehicle pixels must read line 3");
        }
        if first[4..12] != [0; 8] || changed[4..12] != [0; 8] {
            failures.push("transparent pixels changed");
        }
        if draw(2, &before) != draw(2, &after) {
            failures.push("another line changed");
        }
        failures
    }

    #[test]
    fn vehicle_recolor_changes_only_its_line_and_keeps_transparency() {
        assert!(vehicle_recolor_failures(3).is_empty());
    }

    #[test]
    fn vehicle_recolor_check_rejects_a_wrong_line() {
        assert_eq!(
            vehicle_recolor_failures(2),
            ["vehicle pixels must read line 3"]
        );
    }
}
