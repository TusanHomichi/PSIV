//! The public Kosinski work API against US instruction-path costs.
//! These synthetic streams cover the original-routine component probes;
//! clocks exclude interrupts, refresh and hardware stalls.

use psiv_pack::decode::kosinski::{Work, decompress, decompress_with_work};

fn check_work(stream: &[u8], expected: &[u8], cycles: u64) -> Work {
    let (out, consumed, work) = decompress_with_work(stream, 0).unwrap();
    assert_eq!(out, expected);
    assert_eq!(consumed, stream.len());
    assert_eq!(work.cpu_cycles(), cycles);
    assert_eq!(decompress(stream, 0).unwrap(), (out.clone(), consumed));
    // Work depends on the stream's executed paths, not its ROM offset or
    // unrelated bytes before and after it.
    let mut padded = vec![0xAA; 7];
    padded.extend_from_slice(stream);
    padded.extend_from_slice(&[0xBB; 4]);
    assert_eq!(
        decompress_with_work(&padded, 7).unwrap(),
        (out, consumed, work)
    );
    work
}

#[test]
fn setup_literal_and_termination_work() {
    // Empty stream: setup48 + two saved-CCR bits72 + match-prefix
    // branches24 + full-offset work52 + extended BEQ10 + byte8 +
    // terminating BEQ10 + stack restore8 + RTS16 = 248 clocks.
    let empty = check_work(&[0x02, 0, 0, 0xF8, 0], b"", 248);
    assert_eq!(empty, Work::default());
    let one = check_work(&[0x05, 0, b'a', 0, 0xF8, 0], b"a", 314);
    assert_eq!(one.literals, 1);
    let two = check_work(&[0x0B, 0, b'a', b'b', 0, 0xF8, 0], b"ab", 380);
    assert_eq!(two.literals, 2);
}

#[test]
fn inline_count_bits_and_overlapping_copy_work() {
    for count in 0_u8..=3 {
        // Literal1; inline 00, count high/low; terminator01.
        let descriptor = 0x41 | ((count >> 1) << 3) | ((count & 1) << 4);
        let stream = [descriptor, 0, b'x', 0xFF, 0, 0xF8, 0];
        let out = vec![b'x'; usize::from(count) + 3];
        let work = check_work(&stream, &out, 564 + u64::from(count) * 32);
        assert_eq!(work.inline_matches, 1);
        assert_eq!(work.match_bytes, u64::from(count) + 2);
        assert_eq!(work.full_matches + work.extended_matches, 0);
    }
}

#[test]
fn full_short_and_extended_count_paths_are_distinct() {
    for count in 1_u8..=7 {
        // Literal1; full01; full01 terminator. Offset -1, short count.
        let stream = [0x15, 0, b'x', 0xFF, 0xF8 | count, 0, 0xF8, 0];
        let out = vec![b'x'; usize::from(count) + 3];
        let work = check_work(&stream, &out, 588 + u64::from(count - 1) * 32);
        assert_eq!(work.full_matches, 1);
        assert_eq!(work.match_bytes, u64::from(count) + 2);
        assert_eq!(work.inline_matches + work.extended_matches, 0);
    }
    for count in [2_u8, 9, 255] {
        let stream = [0x15, 0, b'x', 0xFF, 0xF8, count, 0, 0xF8, 0];
        let out = vec![b'x'; usize::from(count) + 2];
        let work = check_work(&stream, &out, 632 + u64::from(count - 2) * 32);
        assert_eq!(work.extended_matches, 1);
        assert_eq!(work.match_bytes, u64::from(count) + 1);
        assert_eq!(work.inline_matches + work.full_matches, 0);
    }
}

#[test]
fn extended_one_is_a_noop_not_a_copy_or_descriptor_refill() {
    // Two full-match tokens: extended1, then extended0. Neither's
    // displacement is dereferenced, so no prior output is required.
    let work = check_work(&[0x0A, 0, 0, 0xF8, 1, 0, 0xF8, 0], b"", 440);
    assert_eq!(work.noops, 1);
    assert_eq!(work.match_bytes, 0);
    assert_eq!(work.descriptor_reloads, 0);
}

#[test]
fn descriptor_expiry_refills_even_on_the_terminators_last_bit() {
    // Fourteen literal bits then terminator01 consumes all sixteen.
    // Retail refills before testing that last bit, so the next field
    // precedes the terminator's three data bytes and is consumed too.
    let mut stream = vec![0xFF, 0xBF];
    stream.extend_from_slice(b"abcdefghijklmn");
    stream.extend_from_slice(&[0, 0, 0, 0xF8, 0]);
    let work = check_work(&stream, b"abcdefghijklmn", 1216);
    assert_eq!(work.literals, 14);
    assert_eq!(work.descriptor_reloads, 1);
    // Removing the refill leaves too few bytes for a terminated stream.
    stream.drain(16..18);
    assert!(decompress_with_work(&stream, 0).is_err());
}

#[test]
fn descriptor_expiry_inside_inline_count_preserves_the_shifted_bit() {
    // Thirteen literals, inline00 then high count bit1 (bit sixteen).
    // The new descriptor begins with low count1, then terminator01.
    let mut stream = vec![0xFF, 0x9F];
    stream.extend_from_slice(&[b'x'; 13]);
    stream.extend_from_slice(&[0x05, 0, 0xFF, 0, 0xF8, 0]);
    let work = check_work(&stream, &[b'x'; 18], 1496);
    assert_eq!(work.literals, 13);
    assert_eq!(work.inline_matches, 1);
    assert_eq!(work.match_bytes, 5);
    assert_eq!(work.descriptor_reloads, 1);
}
