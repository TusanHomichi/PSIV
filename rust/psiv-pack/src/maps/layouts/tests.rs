use super::*;

/// A Kosinski stream of `data` as literals, then the terminator. The
/// decoder loads the next description field as soon as the last one
/// empties, before the byte its last bit announces.
pub(crate) fn kosinski_literals(data: &[u8]) -> Vec<u8> {
    fn push_bit(out: &mut Vec<u8>, field: &mut usize, bits: &mut usize, bit: u8) {
        out[*field + *bits / 8] |= bit << (*bits % 8);
        *bits += 1;
        if *bits == 16 {
            *field = out.len();
            *bits = 0;
            out.extend_from_slice(&[0, 0]);
        }
    }
    let (mut out, mut field, mut bits) = (vec![0, 0], 0, 0);
    for &byte in data {
        push_bit(&mut out, &mut field, &mut bits, 1);
        out.push(byte);
    }
    // 0 1: a full match, whose count of 0 and next byte of 0 end the stream.
    push_bit(&mut out, &mut field, &mut bits, 0);
    push_bit(&mut out, &mut field, &mut bits, 1);
    out.extend_from_slice(&[0x00, 0xF8, 0x00]);
    out
}

/// A ROM of `streams` end to end, each stream's offset beside it.
pub(crate) fn rom_of(streams: &[Vec<u8>]) -> (Vec<u8>, Vec<usize>) {
    let mut rom = Vec::new();
    let mut offsets = Vec::new();
    for stream in streams {
        offsets.push(rom.len());
        rom.extend_from_slice(stream);
    }
    (rom, offsets)
}

fn words_bytes(words: &[u16]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_be_bytes()).collect()
}

#[test]
fn the_test_stream_helper_round_trips() {
    for length in [0, 1, 2, 13, 14, 15, 16, 17, 31, 32, 33, 46, 100] {
        let data: Vec<u8> = (0..length).map(|i| (i * 3) as u8).collect();
        let stream = kosinski_literals(&data);
        let (out, consumed) = crate::decode::kosinski::decompress(&stream, 0).unwrap();
        assert_eq!(out, data, "length {length}");
        assert_eq!(consumed, stream.len(), "length {length}");
    }
}

#[test]
fn collision_reads_each_quadrants_four_tiles_in_order() {
    // Top-left quadrant: tile 0 only (1). Top-right: tiles 2 and 3 (1|2).
    // Bottom-left: tile 12 (bit 8 of base 8 + 4 + 0 -> 4). Bottom-right:
    // tile 15 alone (8).
    let mut words = [0u16; CHUNK_WORDS];
    words[0] = 1 << 14;
    words[2] = 1 << 14;
    words[3] = 1 << 14;
    words[12] = 1 << 14;
    words[15] = 1 << 14;
    let chunks = ChunkTable {
        words: vec![words],
        blobs: Vec::new(),
    };
    let layout = Layout {
        plane: Plane::Fg,
        width_chunks: 1,
        height_chunks: 1,
        cells: vec![0],
        blob: Blob {
            rom_offset: 0,
            compressed_length: 0,
            decompressed_length: 0,
            sha256: String::new(),
        },
        anomaly: None,
    };
    let grid = decode_collision(&chunks, &layout).unwrap();
    assert_eq!((grid.width, grid.height), (2, 2));
    assert_eq!(grid.types, [1, 3, 4, 8]);
}

#[test]
fn a_padded_or_short_plane_is_an_anomaly_and_a_dirty_surplus_fails() {
    let (rom, at) = rom_of(&[
        kosinski_literals(&[1, 2, 0, 0, 0, 0]),
        kosinski_literals(&[1, 2]),
        kosinski_literals(&[1, 2, 3, 4, 5]),
    ]);
    let decodes = Decodes::new(&rom);
    let padded = decode_layout(&rom, &decodes, at[0], 2, 2, Plane::Fg).unwrap();
    assert_eq!(padded.cells, [1, 2, 0, 0]);
    let anomaly = padded.anomaly.unwrap();
    assert!(crate::json::dumps(&anomaly, true).contains("\"padded\""));
    let short = decode_layout(&rom, &decodes, at[1], 2, 2, Plane::Bg).unwrap();
    assert_eq!(short.cells, [1, 2, 0, 0]);
    assert!(crate::json::dumps(&short.anomaly.unwrap(), true).contains("2 cells are left"));
    let exact = decode_layout(&rom, &decodes, at[0], 3, 2, Plane::Fg).unwrap();
    assert!(exact.anomaly.is_none());
    let error = decode_layout(&rom, &decodes, at[2], 2, 2, Plane::Fg).unwrap_err();
    assert!(error.0.contains("not zero padding"), "{}", error.0);
    let error = decode_layout(&rom, &decodes, at[0], 65, 64, Plane::Fg).unwrap_err();
    assert!(error.0.contains("Map_Layout region"), "{}", error.0);
}

#[test]
fn a_chunk_past_the_table_gets_the_empty_definition_and_is_recorded() {
    let word = 1u16 << 14;
    let chunk = words_bytes(&[word; CHUNK_WORDS]);
    let (rom, at) = rom_of(&[
        kosinski_literals(&chunk),
        kosinski_literals(&[0, 3]),
        kosinski_literals(&[0, 0]),
        // A 32-byte pattern for tile 1.
        kosinski_literals(&[0x11; 32]),
    ]);
    let decodes = Decodes::new(&rom);
    let spec = MapLayoutSpec {
        chunk_blobs: vec![at[0]],
        layout_fg: at[1],
        layout_bg: at[2],
        width_chunks_fg: 2,
        height_chunks_fg: 1,
        width_chunks_bg: 2,
        height_chunks_bg: 1,
        collision_plane: 0,
        tilesets: vec![(1, at[3])],
        palette: 0,
        label: "Test",
    };
    let (decoded, anomalies) = decode_layout_section(&rom, &decodes, spec).unwrap();
    assert_eq!(decoded.chunks.words.len(), 4);
    assert_eq!(decoded.chunks.words[3], [0; CHUNK_WORDS]);
    let text = crate::json::dumps(&Json::from(anomalies), true);
    assert!(text.contains("\"chunk_id\": \"0x03\""), "{text}");
    assert!(text.contains("\"loaded_chunks\": 1"), "{text}");
    // Tile 0 is named by chunk 0's words (word 1 << 14 strips to tile 0)
    // and nothing loads it.
    assert_eq!(unloaded_patterns(&decoded).unwrap(), [0]);
    // The substituted chunk has no collision bit; the real one has all.
    assert_eq!(&decoded.collision.types[..2], &[0xF, 0xF]);
    assert_eq!(&decoded.collision.types[2..4], &[0, 0]);
}

#[test]
fn the_blob_records_what_the_stream_consumed() {
    let stream = kosinski_literals(&[7, 7]);
    let mut rom = vec![0xAA; 4];
    rom.extend_from_slice(&stream);
    rom.extend_from_slice(&[0xBB; 8]);
    let decodes = Decodes::new(&rom);
    let (_, blob) = decompress_blob(&rom, &decodes, 4, "Test").unwrap();
    assert_eq!(blob.compressed_length, stream.len());
    assert_eq!(blob.decompressed_length, 2);
    assert_eq!(blob.sha256, hex(&Sha256::digest(&stream)));
    let error = decompress_blob(&rom, &decodes, rom.len(), "Test").unwrap_err();
    assert!(error.0.starts_with("Test offset 0x"), "{}", error.0);
}

#[test]
fn blocking_is_the_retail_five_types() {
    let blocking: Vec<u8> = (0..16).filter(|&v| is_blocking(v)).collect();
    assert_eq!(blocking, [8, 9, 0xA, 0xB, 0xC]);
    assert_eq!(collision_type_name(0xB), "ice_block");
    assert_eq!(collision_type_name(0xF), "unnamed_F");
}
