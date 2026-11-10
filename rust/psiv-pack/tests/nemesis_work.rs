//! Intrinsic US Nemesis instruction work, independently checked with isolated
//! original-routine executions. These streams are synthetic; the CPU clocks
//! exclude IRQ, refresh and VDP/bus stalls.

use psiv_pack::decode::nemesis::{Destination, Work, decompress, decompress_with_work};

fn check(stream: &[u8], expected: &[u8], consumed: usize, clocks: u64) -> Work {
    let (bytes, actual_consumed, work) = decompress_with_work(stream, 0).unwrap();
    assert_eq!(bytes, expected);
    assert_eq!(actual_consumed, consumed);
    assert_eq!(work.cpu_cycles(Destination::Ram), Some(clocks));
    // The entries differ by LEA a4 and BRA (22 clocks), not pixel semantics.
    assert_eq!(work.cpu_cycles(Destination::Vdp), Some(clocks + 22));
    assert_eq!(decompress(stream, 0).unwrap(), (bytes.clone(), consumed));
    let mut padded = vec![0xAA; 5];
    padded.extend_from_slice(stream);
    padded.extend_from_slice(&[0xBB; 4]);
    assert_eq!(
        decompress_with_work(&padded, 5).unwrap(),
        (bytes, consumed, work)
    );
    work
}

fn bit_bytes(bits: &[u8]) -> Vec<u8> {
    let mut out = vec![0; bits.len().div_ceil(8) + 2];
    for (index, &bit) in bits.iter().enumerate() {
        out[index / 8] |= bit << (7 - index % 8);
    }
    out
}

fn table_stream(length: u8, xor: bool, empty_palettes: &[u8]) -> Vec<u8> {
    let mut stream = vec![if xor { 0x80 } else { 0 }, 1];
    stream.extend_from_slice(empty_palettes);
    // One table code, palette5, repeat7. Eight tokens write eight rows.
    stream.extend_from_slice(&[0x85, 0x70 | length, 0, 0xFF]);
    stream.extend(bit_bytes(&vec![0; usize::from(length) * 8]));
    stream
}

fn inline_stream(repeats: &[u8], xor: bool) -> Vec<u8> {
    let mut stream = vec![if xor { 0x80 } else { 0 }, 1, 0xFF];
    let mut bits = Vec::new();
    for &repeat in repeats {
        let token = (0x3F_u16 << 7) | (u16::from(repeat) << 4) | 5;
        bits.extend((0..13).rev().map(|shift| ((token >> shift) & 1) as u8));
    }
    stream.extend(bit_bytes(&bits));
    stream
}

#[test]
fn table_lengths_count_real_shift_and_expansion_work() {
    for (length, consumed, clocks, shift, slots) in [
        (0, 8, 12_330, 8, 256),
        (1, 9, 8_718, 7, 128),
        (7, 15, 5_358, 1, 2),
        (8, 16, 5_374, 0, 0),
    ] {
        let work = check(
            &table_stream(length, false, &[]),
            &[0x55; 32],
            consumed,
            clocks,
        );
        assert_eq!(work.palettes, 1);
        assert_eq!(work.full_codes, u64::from(length == 8));
        assert_eq!(work.short_codes, u64::from(length != 8));
        assert_eq!(work.table_shift_bits, shift);
        assert_eq!(work.table_slots, slots);
        assert_eq!(work.lookup_tokens, 8);
        assert_eq!(work.pixels, 64);
        assert_eq!(work.rows, 8);
    }
}

#[test]
fn empty_palette_markers_still_execute_the_table_builder_loop() {
    let work = check(
        &table_stream(1, false, &[0x81, 0x82]),
        &[0x55; 32],
        11,
        8_814,
    );
    assert_eq!(work.palettes, 3);
    assert_eq!(work.short_codes, 1);
}

#[test]
fn xor_accumulation_adds_header_and_row_work_for_both_entries() {
    let mut expected = Vec::new();
    for row in 0..8 {
        expected.extend_from_slice(&[if row % 2 == 0 { 0x55 } else { 0 }; 4]);
    }
    let lookup = check(&table_stream(1, true, &[]), &expected, 9, 8_792);
    let inline = check(&inline_stream(&[7; 8], true), &expected, 18, 5_444);
    assert!(lookup.xor_mode && inline.xor_mode);
}

#[test]
fn inline_payload_has_two_distinct_refill_branches() {
    let work = check(&inline_stream(&[7; 8], false), &[0x55; 32], 18, 5_370);
    assert_eq!(work.inline_tokens, 8);
    assert_eq!(work.lookup_tokens, 0);
    assert_eq!(work.palettes, 0);
    assert!(work.inline_marker_refills > 0);
    assert!(work.inline_payload_refills > 0);
    assert_eq!(work.inline_marker_refills + work.inline_payload_refills, 13);
    assert_eq!(work.rows, 8);
}

#[test]
fn repeats_cross_rows_and_the_final_row_cuts_off_inside_a_repeat() {
    // Repeats of six and four pixels cross nonfinal rows. After six pairs
    // (60 pixels), the next six-pixel repeat is cut short at pixel64.
    let mut repeats = [5, 3].repeat(6);
    repeats.push(5);
    let inline = check(&inline_stream(&repeats, false), &[0x55; 32], 26, 6_458);
    assert_eq!(inline.inline_tokens, 13);
    assert_eq!(inline.pixels, 64);
    let mut stream = vec![0, 1, 0x86, 0x51, 0, 0x31, 1, 0xFF];
    let mut bits = [0, 1].repeat(6);
    bits.push(0);
    stream.extend(bit_bytes(&bits));
    let lookup = check(&stream, &[0x66; 32], 11, 13_280);
    assert_eq!(lookup.lookup_tokens, 13);
    assert_eq!(lookup.short_codes, 2);
    assert_eq!(lookup.lookup_refills, 1);
    assert_eq!(lookup.pixels, 64);
    // Counting the unexecuted final two pixels would change both output and
    // cost: the routine returns before their writes and before final DBF/BRA.
    assert_eq!(inline.rows, 8);
    assert_eq!(lookup.rows, 8);
}

#[test]
fn mixed_code_lengths_and_inline_lookup_alternation_share_the_live_window() {
    let mut bits = Vec::new();
    for _ in 0..4 {
        // Code0 is one bit, code10 is two; both repeat eight pixels.
        bits.extend_from_slice(&[0, 1, 0]);
    }
    let mut table = vec![0, 1, 0x85, 0x71, 0, 0x86, 0x72, 2, 0xFF];
    table.extend(bit_bytes(&bits));
    let mut expected = Vec::new();
    for _ in 0..4 {
        expected.extend_from_slice(&[0x55; 4]);
        expected.extend_from_slice(&[0x66; 4]);
    }
    let lookup = check(&table, &expected, 12, 10_740);
    assert_eq!(lookup.lookup_tokens, 8);
    assert_eq!(lookup.short_codes, 2);
    assert_eq!(lookup.palettes, 2);
    assert_eq!(lookup.table_shift_bits, 13);

    bits.clear();
    for _ in 0..4 {
        bits.push(0);
        let token = (0x3F_u16 << 7) | (7 << 4) | 6;
        bits.extend((0..13).rev().map(|shift| ((token >> shift) & 1) as u8));
    }
    let mut mixed = vec![0, 1, 0x85, 0x71, 0, 0xFF];
    mixed.extend(bit_bytes(&bits));
    let work = check(&mixed, &expected, 15, 8_950);
    assert_eq!(work.lookup_tokens, 4);
    assert_eq!(work.inline_tokens, 4);
    assert_eq!(work.pixels, 64);
}

#[test]
fn zero_wrapped_row_count_preserves_bytes_without_claiming_retail_timing() {
    for header in [0_u16, 0x2000, 0x8000] {
        let mut stream = header.to_be_bytes().to_vec();
        stream.push(0xFF);
        let (bytes, consumed, work) = decompress_with_work(&stream, 0).unwrap();
        assert!(bytes.is_empty());
        assert_eq!(consumed, 3);
        assert_eq!(decompress(&stream, 0).unwrap(), (bytes, consumed));
        assert_eq!(work.cpu_cycles(Destination::Ram), None);
        assert_eq!(work.cpu_cycles(Destination::Vdp), None);
    }
}
