//! The pack's PNG files: `psiv_tools/png.py`'s encoder, and its image digest.
//!
//! The same chunks as the Python encoder: IHDR (8-bit, no interlace), PLTE
//! and a tRNS cut after its last transparent entry for an indexed image, one
//! IDAT with filter type 0 on every row, IEND. The deflate stream is not the
//! same bytes (zlib, zlib-ng and miniz compress alike rows differently), so a
//! PNG matches the Python builder's by [`image_sha256`], taken over what the
//! file decodes to, never by the file's own SHA-256.

use std::io::{Read as _, Write as _};

use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use sha2::{Digest as _, Sha256};

const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
const COLOR_TYPE_RGB: u8 = 2;
const COLOR_TYPE_INDEXED: u8 = 3;
/// The compression level. Any level decodes to the same image; this one is
/// the speed/size balance zlib picks by default.
const LEVEL: u32 = 6;
const IMAGE_DIGEST_DOMAIN: &[u8] = b"psiv-image-sha256-v1\0";

/// A PNG the encoder refuses to write, or a file the digest cannot read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PngError(pub String);

impl std::fmt::Display for PngError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PngError {}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], payload: &[u8]) {
    let length = u32::try_from(payload.len()).expect("a PNG chunk under 4 GiB");
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(payload);
    let mut crc = flate2::Crc::new();
    crc.update(kind);
    crc.update(payload);
    out.extend_from_slice(&crc.sum().to_be_bytes());
}

fn ihdr(out: &mut Vec<u8>, width: u32, height: u32, color_type: u8) -> Result<(), PngError> {
    if width == 0 || height == 0 {
        return Err(PngError(format!(
            "Image dimensions must be positive, got {width}x{height}"
        )));
    }
    let mut payload = Vec::with_capacity(13);
    payload.extend_from_slice(&width.to_be_bytes());
    payload.extend_from_slice(&height.to_be_bytes());
    // bit depth 8, deflate, adaptive filtering, no interlace
    payload.extend_from_slice(&[8, color_type, 0, 0, 0]);
    chunk(out, b"IHDR", &payload);
    Ok(())
}

fn idat(out: &mut Vec<u8>, pixels: &[u8], stride: usize, height: usize) -> Result<(), PngError> {
    if pixels.len() != stride * height {
        return Err(PngError(format!(
            "Pixel buffer is {} bytes, expected {} ({height} rows of {stride})",
            pixels.len(),
            stride * height
        )));
    }
    let mut encoder = ZlibEncoder::new(
        Vec::with_capacity(pixels.len() / 2 + 64),
        Compression::new(LEVEL),
    );
    for row in pixels.chunks_exact(stride) {
        encoder.write_all(&[0]).expect("writing to a Vec");
        encoder.write_all(row).expect("writing to a Vec");
    }
    let compressed = encoder.finish().expect("writing to a Vec");
    chunk(out, b"IDAT", &compressed);
    Ok(())
}

/// An 8-bit indexed image: `pixels` row-major, one palette index each;
/// `transparent` lists the indices that become fully transparent.
pub fn encode_indexed(
    width: u32,
    height: u32,
    pixels: &[u8],
    palette: &[[u8; 3]],
    transparent: &[u8],
) -> Result<Vec<u8>, PngError> {
    if !(1..=256).contains(&palette.len()) {
        return Err(PngError(format!(
            "Palette must hold 1..256 colours, got {}",
            palette.len()
        )));
    }
    if let Some(&index) = pixels
        .iter()
        .filter(|&&p| usize::from(p) >= palette.len())
        .max()
    {
        return Err(PngError(format!(
            "Pixel index {index} has no palette entry ({} colours)",
            palette.len()
        )));
    }
    let mut out = Vec::with_capacity(pixels.len() / 2 + 3 * palette.len() + 128);
    out.extend_from_slice(SIGNATURE);
    ihdr(&mut out, width, height, COLOR_TYPE_INDEXED)?;
    chunk(&mut out, b"PLTE", palette.as_flattened());
    let mut alpha = vec![255u8; palette.len()];
    for &index in transparent {
        if usize::from(index) >= palette.len() {
            return Err(PngError(format!(
                "Transparent index {index} is outside the palette"
            )));
        }
        alpha[usize::from(index)] = 0;
    }
    if let Some(last) = alpha.iter().rposition(|&a| a != 255) {
        // tRNS may be truncated after the last non-opaque entry.
        chunk(&mut out, b"tRNS", &alpha[..=last]);
    }
    idat(&mut out, pixels, width as usize, height as usize)?;
    chunk(&mut out, b"IEND", b"");
    Ok(out)
}

/// An 8-bit-per-channel truecolour image; `pixels` is RGB triples.
pub fn encode_rgb(width: u32, height: u32, pixels: &[u8]) -> Result<Vec<u8>, PngError> {
    let mut out = Vec::with_capacity(pixels.len() / 2 + 128);
    out.extend_from_slice(SIGNATURE);
    ihdr(&mut out, width, height, COLOR_TYPE_RGB)?;
    idat(&mut out, pixels, width as usize * 3, height as usize)?;
    chunk(&mut out, b"IEND", b"");
    Ok(out)
}

fn paeth(left: u8, up: u8, corner: u8) -> u8 {
    let estimate = i16::from(left) + i16::from(up) - i16::from(corner);
    let to_left = (estimate - i16::from(left)).abs();
    let to_up = (estimate - i16::from(up)).abs();
    let to_corner = (estimate - i16::from(corner)).abs();
    if to_left <= to_up && to_left <= to_corner {
        left
    } else if to_up <= to_corner {
        up
    } else {
        corner
    }
}

fn unfilter(raw: &[u8], stride: usize, height: usize, bpp: usize) -> Result<Vec<u8>, PngError> {
    if raw.len() != (stride + 1) * height {
        return Err(PngError(format!(
            "IDAT inflates to {} bytes, expected {} ({height} rows of 1 + {stride})",
            raw.len(),
            (stride + 1) * height
        )));
    }
    let mut out = vec![0u8; stride * height];
    let zero = vec![0u8; stride];
    for y in 0..height {
        let line = &raw[y * (stride + 1)..(y + 1) * (stride + 1)];
        let (before, rest) = out.split_at_mut(y * stride);
        let previous: &[u8] = if y == 0 {
            &zero
        } else {
            &before[(y - 1) * stride..]
        };
        let row = &mut rest[..stride];
        row.copy_from_slice(&line[1..]);
        match line[0] {
            0 => {}
            1 => {
                for x in bpp..stride {
                    row[x] = row[x].wrapping_add(row[x - bpp]);
                }
            }
            2 => {
                for x in 0..stride {
                    row[x] = row[x].wrapping_add(previous[x]);
                }
            }
            3 => {
                for x in 0..stride {
                    let left = if x >= bpp { row[x - bpp] } else { 0 };
                    let mean = ((u16::from(left) + u16::from(previous[x])) >> 1) as u8;
                    row[x] = row[x].wrapping_add(mean);
                }
            }
            4 => {
                for x in 0..stride {
                    let left = if x >= bpp { row[x - bpp] } else { 0 };
                    let corner = if x >= bpp { previous[x - bpp] } else { 0 };
                    row[x] = row[x].wrapping_add(paeth(left, previous[x], corner));
                }
            }
            kind => {
                return Err(PngError(format!(
                    "Scanline {y} has filter type {kind}, not 0-4"
                )));
            }
        }
    }
    Ok(out)
}

/// `psiv_tools.png.image_sha256`: the SHA-256 of the IHDR fields, the PLTE
/// and tRNS payloads and the unfiltered rows, so the same image gives the
/// same digest whichever deflate wrote it. 8-bit, non-interlaced images only.
pub fn image_sha256(data: &[u8]) -> Result<String, PngError> {
    if data.len() < 8 || &data[..8] != SIGNATURE {
        return Err(PngError("missing PNG signature".into()));
    }
    let mut pos = 8;
    let mut header: Option<&[u8]> = None;
    let mut plte: &[u8] = b"";
    let mut trns: &[u8] = b"";
    let mut compressed = Vec::new();
    while pos < data.len() {
        if pos + 12 > data.len() {
            return Err(PngError(format!("truncated chunk at byte {pos}")));
        }
        let length = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
        let kind = &data[pos + 4..pos + 8];
        let end = pos + 12 + length;
        if end > data.len() {
            return Err(PngError(format!("truncated chunk at byte {pos}")));
        }
        let payload = &data[pos + 8..pos + 8 + length];
        let mut crc = flate2::Crc::new();
        crc.update(kind);
        crc.update(payload);
        if crc.sum().to_be_bytes() != data[pos + 8 + length..end] {
            return Err(PngError(format!("chunk at byte {pos} fails its CRC")));
        }
        pos = end;
        match kind {
            b"IHDR" => header = Some(payload),
            b"IDAT" => compressed.extend_from_slice(payload),
            b"PLTE" => plte = payload,
            b"tRNS" => trns = payload,
            b"IEND" => break,
            _ => {}
        }
    }
    let header = header
        .filter(|h| h.len() == 13)
        .ok_or_else(|| PngError("missing or malformed IHDR".into()))?;
    let width = u32::from_be_bytes(header[0..4].try_into().unwrap()) as usize;
    let height = u32::from_be_bytes(header[4..8].try_into().unwrap()) as usize;
    let (depth, colour, interlace) = (header[8], header[9], header[12]);
    let bpp = match colour {
        0 | 3 => 1,
        2 => 3,
        4 => 2,
        6 => 4,
        _ => 0,
    };
    if depth != 8 || bpp == 0 || interlace != 0 {
        return Err(PngError(format!(
            "image_sha256 takes 8-bit non-interlaced images, not depth {depth} \
             colour type {colour} interlace {interlace}"
        )));
    }
    let mut raw = Vec::new();
    ZlibDecoder::new(compressed.as_slice())
        .read_to_end(&mut raw)
        .map_err(|e| PngError(format!("IDAT does not inflate: {e}")))?;
    let pixels = unfilter(&raw, width * bpp, height, bpp)?;
    let mut digest = Sha256::new();
    digest.update(IMAGE_DIGEST_DOMAIN);
    digest.update(header);
    for (kind, payload) in [(b"PLTE", plte), (b"tRNS", trns)] {
        digest.update(kind);
        digest.update((payload.len() as u32).to_be_bytes());
        digest.update(payload);
    }
    digest.update(&pixels);
    Ok(hex(&digest.finalize()))
}

/// Lowercase hex, as Python's `hexdigest()`.
pub fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(out, "{b:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexed_round_trips_through_the_digest() {
        let palette = [[0, 0, 0], [255, 0, 0], [0, 255, 0]];
        let pixels = [0, 1, 2, 1, 0, 2];
        let png = encode_indexed(3, 2, &pixels, &palette, &[0]).unwrap();
        let changed = encode_indexed(3, 2, &[0, 1, 2, 1, 0, 1], &palette, &[0]).unwrap();
        let opaque = encode_indexed(3, 2, &pixels, &palette, &[]).unwrap();
        let a = image_sha256(&png).unwrap();
        assert_ne!(
            a,
            image_sha256(&changed).unwrap(),
            "a pixel is part of the image"
        );
        assert_ne!(
            a,
            image_sha256(&opaque).unwrap(),
            "tRNS is part of the image"
        );
    }

    #[test]
    fn encoder_refuses_what_python_refuses() {
        let palette = [[0, 0, 0]];
        assert!(encode_indexed(1, 1, &[1], &palette, &[]).is_err());
        assert!(encode_indexed(1, 1, &[0], &palette, &[1]).is_err());
        assert!(encode_indexed(0, 1, &[], &palette, &[]).is_err());
        assert!(encode_indexed(2, 1, &[0], &palette, &[]).is_err());
        assert!(encode_rgb(1, 1, &[0, 0]).is_err());
    }

    #[test]
    fn digest_refuses_a_damaged_file() {
        let mut png = encode_rgb(1, 1, &[1, 2, 3]).unwrap();
        let last = png.len() - 1;
        png[last] ^= 1;
        assert!(image_sha256(&png).is_err());
        assert!(image_sha256(b"not a png").is_err());
    }
}
