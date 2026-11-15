//! Public scene-load queries need no ROM or generated pack. Synthetic streams
//! exercise identities and input-dependent work through the consumer API.

use psiv_pack::{
    json::Json,
    presentation::load_work::{art, panel},
};
use sha2::{Digest, Sha256};

#[test]
fn timing_queries_do_not_require_a_renderable_panel() {
    let mut rom = vec![0; 0x07B000 + 30];
    let offset = 0x07B000;
    // No art upload and no layers: retail's $FFFF sentinels skip them.
    for at in [0, 10, 20] {
        rom[offset + at..offset + at + 2].copy_from_slice(&0xFFFFu16.to_be_bytes());
    }
    let report = panel(&rom, 0).unwrap();
    assert_eq!(report.at(&["art"]).unwrap(), &Json::Null);
    assert!(
        report
            .at(&["mappings"])
            .unwrap()
            .items()
            .unwrap()
            .is_empty()
    );
    assert!(panel(&rom, 0xC0).is_err());
}

#[test]
fn art_reports_the_existing_decoders_work_and_identity() {
    let stream = [0, 1, 0x85, 0x71, 0, 0xFF, 0, 0, 0];
    let report = art(&stream, 0).unwrap();
    assert_eq!(report.at(&["decoded_bytes"]).unwrap().int().unwrap(), 32);
    assert_eq!(report.at(&["cpu_cycles"]).unwrap().int().unwrap(), 8740);
    assert!(art(&stream[..7], 0).is_err());
}

#[test]
fn equal_output_sizes_do_not_imply_equal_work() {
    let short = art(&[0, 1, 0x85, 0x71, 0, 0xFF, 0, 0, 0], 0).unwrap();
    let mut long = vec![0, 1, 0x85, 0x78, 0, 0xFF];
    long.extend([0; 10]);
    let long = art(&long, 0).unwrap();
    assert_eq!(
        short.at(&["decoded_sha256"]).unwrap(),
        long.at(&["decoded_sha256"]).unwrap()
    );
    assert_ne!(
        short.at(&["cpu_cycles"]).unwrap(),
        long.at(&["cpu_cycles"]).unwrap()
    );
}

fn digest(bytes: &[u8]) -> Json {
    psiv_pack::png::hex(&Sha256::digest(bytes)).into()
}

#[test]
fn art_identities_cover_the_consumed_stream_at_its_source() {
    let stream = [0, 1, 0x85, 0x71, 0, 0xFF, 0, 0, 0];
    let mut rom = vec![0xAA; 12];
    rom.extend(stream);
    rom.extend([0xBB; 4]);
    let report = art(&rom, 12).unwrap();
    assert_eq!(report.at(&["source"]).unwrap(), &Json::from("0x00000C"));
    assert_eq!(report.at(&["consumed_bytes"]).unwrap(), &Json::from(9));
    assert_eq!(report.at(&["compressed_sha256"]).unwrap(), &digest(&stream));
    assert_eq!(
        report.at(&["decoded_sha256"]).unwrap(),
        &digest(&[0x55; 32])
    );
    assert_eq!(report.at(&["lookup_tokens"]).unwrap(), &Json::from(8));
    assert_eq!(report.at(&["inline_tokens"]).unwrap(), &Json::from(0));
    // Same bytes decoded through a different table path have different work
    // and compressed identity; an output-only identity would miss the change.
    rom.truncate(12);
    rom.extend([0, 1, 0x85, 0x78, 0, 0xFF]);
    rom.extend([0; 10]);
    let changed = art(&rom, 12).unwrap();
    assert_eq!(
        report.at(&["decoded_sha256"]).unwrap(),
        changed.at(&["decoded_sha256"]).unwrap()
    );
    assert_ne!(
        report.at(&["compressed_sha256"]).unwrap(),
        changed.at(&["compressed_sha256"]).unwrap()
    );
    assert_ne!(
        report.at(&["cpu_cycles"]).unwrap(),
        changed.at(&["cpu_cycles"]).unwrap()
    );
}

#[test]
fn panel_reports_art_and_each_mapping_with_its_own_base_tile() {
    let offset = 0x07B000;
    let mut rom = vec![0; offset + 30];
    let art_stream = [0, 1, 0x85, 0x71, 0, 0xFF, 0, 0, 0];
    let map_stream = [4, 0, 0, 0x10, 0, 0x20, 0x09, 0x1F, 0xE0, 0, 0, 0];
    rom[0..art_stream.len()].copy_from_slice(&art_stream);
    rom[32..32 + map_stream.len()].copy_from_slice(&map_stream);
    rom[offset..offset + 2].copy_from_slice(&0x100u16.to_be_bytes());
    rom[offset + 6..offset + 10].copy_from_slice(&[1, 2, 3, 4]);
    for (at, tile) in [(10, 0x200u16), (20, 0x400)] {
        rom[offset + at..offset + at + 2].copy_from_slice(&tile.to_be_bytes());
        rom[offset + at + 6..offset + at + 10].copy_from_slice(&32u32.to_be_bytes());
    }
    let report = panel(&rom, 0).unwrap();
    assert_eq!(report.at(&["art"]).unwrap(), &art(&rom, 0).unwrap());
    for (key, value) in [
        ("destination_tile", 0x100),
        ("row_size", 1),
        ("col_size", 2),
        ("x_cells", 3),
        ("y_cells", 4),
    ] {
        assert_eq!(report.at(&[key]).unwrap(), &Json::from(value));
    }
    let layers = report.at(&["mappings"]).unwrap().items().unwrap();
    assert_eq!(layers.len(), 2);
    for (layer, tile) in layers.iter().zip([0x200u16, 0x400]) {
        let words: Vec<_> = [0x10, 0x11, 0x12, 0x20, 0x20].map(|w| w + tile).into();
        assert_eq!(layer.at(&["base_tile"]).unwrap(), &Json::from(tile));
        assert_eq!(layer.at(&["decoded_words"]).unwrap(), &Json::from(5));
        assert_eq!(
            layer.at(&["decoded_sha256"]).unwrap(),
            &digest(&psiv_pack::decode::enigma::words_to_bytes(&words))
        );
        let consumed = layer.at(&["consumed_bytes"]).unwrap().int().unwrap() as usize;
        assert_eq!(
            layer.at(&["compressed_sha256"]).unwrap(),
            &digest(&map_stream[..consumed])
        );
    }
    // Each enabled stream is required; neither a bad pointer nor a truncated
    // record can silently produce a partial work report.
    let outside = rom.len() as u32;
    rom[offset + 26..offset + 30].copy_from_slice(&outside.to_be_bytes());
    assert!(panel(&rom, 0).is_err());
    assert!(panel(&rom[..offset + 29], 0).is_err());
}
