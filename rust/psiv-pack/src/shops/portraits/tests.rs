//! Decoded pixels, palette and shared-image checks (#560).
use super::*;
use crate::test_support::{retail_rom, scratch};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::io::Write;

// Recompress only IDAT, preserving the image chunks. Comparing decoded
// content must accept this even though its compressed bytes differ.
fn uncompressed_png(bytes: &[u8]) -> Vec<u8> {
    use std::io::Read;
    let mut out = bytes[..8].to_vec();
    let mut at = 8;
    while at < bytes.len() {
        let len = u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        let kind = &bytes[at + 4..at + 8];
        if kind == b"IDAT" {
            let mut raw = Vec::new();
            flate2::read::ZlibDecoder::new(&bytes[at + 8..at + 8 + len])
                .read_to_end(&mut raw)
                .unwrap();
            let mut encoder =
                flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::none());
            encoder.write_all(&raw).unwrap();
            let payload = encoder.finish().unwrap();
            out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
            out.extend_from_slice(kind);
            out.extend_from_slice(&payload);
            let mut crc = flate2::Crc::new();
            crc.update(kind);
            crc.update(&payload);
            out.extend_from_slice(&crc.sum().to_be_bytes());
        } else {
            out.extend_from_slice(&bytes[at..at + len + 12]);
        }
        at += len + 12;
    }
    out
}

#[test]
fn all_eight_portraits_keep_source_pixels_palette_and_shared_images() {
    let Some(rom) = retail_rom("shop portrait pixels") else {
        return;
    };
    let data = rom.bytes();
    let root = scratch("shop-portraits");
    let records = write(data, &root).unwrap();
    assert_eq!(records.len(), 8);
    let mut expected_art = BTreeSet::new();
    for (base, count) in [(0x68006, 18), (0x6804e, 49)] {
        for at in (base..base + count * 4).step_by(4) {
            expected_art.insert(u32::from_be_bytes(data[at..at + 4].try_into().unwrap()));
        }
    }
    let palette: Vec<_> = (0..16)
        .flat_map(|i| {
            let at = 0x296300 + i * 2;
            gfx::Color::decode(u16::from_be_bytes([data[at], data[at + 1]])).rgb()
        })
        .collect();
    let mut images = std::collections::BTreeMap::new();
    for (record, art) in records.iter().zip(expected_art) {
        assert_eq!(record.record.art, format!("0x{art:06X}"));
        assert_eq!(record.record.png, format!("shops/portraits/{art:06x}.png"));
        let extra = Value::from(&record.research);
        assert_eq!(extra["size_pixels"], json!([48, 48]));
        assert_eq!(extra["cram_line"], 2);
        assert_eq!(extra["mapping"], "0x2A2B36");
        let bytes = std::fs::read(root.join(&record.record.png)).unwrap();
        let image = png::decode(&bytes).unwrap();
        assert_eq!((image.width, image.height, image.colour), (48, 48, 3));
        assert_eq!(image.plte, palette);
        assert_eq!(image.trns, [0]);
        let (raw, consumed) = nemesis::decompress(data, art as usize).unwrap();
        assert_eq!(raw.len(), 36 * 32);
        assert_eq!(extra["source_consumed"], consumed);
        // Read packed nibbles directly; do not reuse decode_tiles or the
        // production compositor to predict the pixels under test.
        let mut expected = Vec::new();
        for y in 0..48 {
            for x in 0..48 {
                let tile = y / 8 * 6 + x / 8;
                let packed = raw[tile * 32 + y % 8 * 4 + x % 8 / 2];
                expected.push((packed >> (4 * (1 - x % 2))) & 15);
            }
        }
        assert_eq!(image.pixels, expected, "portrait pixels at {art:#x}");
        assert!(expected.iter().any(|&p| p != 0));
        let digest = png::image_sha256(&bytes).unwrap();
        assert_eq!(extra["png_sha256"], digest);
        let alternate = uncompressed_png(&bytes);
        assert_ne!(alternate, bytes);
        assert_eq!(png::image_sha256(&alternate).unwrap(), digest);
        images.insert(art, digest);
    }
    assert_eq!(images.values().collect::<BTreeSet<_>>().len(), 8);
    let dialogue = crate::dialogue::retirement::emission().unwrap();
    assert_eq!(
        images[&0x29fc66],
        png::image_sha256(&dialogue.files["dialogue/portraits/12_Baker.png"]).unwrap()
    );
    crate::presentation::build(&rom, &root).unwrap();
    let shared = std::fs::read(root.join("presentation/portraits/shopkeeper_2.png")).unwrap();
    assert_eq!(images[&0x29de1e], png::image_sha256(&shared).unwrap());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_changed_portrait_mapping_is_refused() {
    let Some(rom) = retail_rom("shop portrait mapping refusal") else {
        return;
    };
    let mut data = rom.bytes().to_vec();
    data[MAPPING + 1] ^= 1;
    let root = scratch("shop-portrait-bad-map");
    let error = write(&data, &root).err().expect("changed mapping accepted");
    assert!(error.0.contains("retail 6x6 tile grid"), "{error}");
    assert!(!root.exists(), "mapping refusal precedes output");
}
