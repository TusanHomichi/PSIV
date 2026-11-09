//! Kosinski decompression: `psiv_tools/kosinski.py`, which transcribes the
//! game's `KosDecomp`. Description fields are little-endian words consumed
//! LSB first, and a field is reloaded only after its sixteenth bit is taken,
//! so `consumed` matches the routine's own read count.
//!
//! [`Work`] counts the intrinsic instruction work of US retail `KosDecomp`,
//! ROM `$041BEA..=$041C89` (`ps4.asm:85262-85346`). It excludes interrupts,
//! CPU refresh, DMA/bus stalls and the caller's JSR. These are CPU clock
//! cycles, not the oracle's master cycles (seven per CPU clock).

use super::DecodeError;

fn error(message: String) -> DecodeError {
    DecodeError(message)
}

/// Data-dependent paths taken by one successful retail decompression.
///
/// Each successful stream includes one setup and one terminating token.
/// `descriptor_reloads` excludes the initial descriptor read, but includes
/// the immediate refill after bit 16 even when that bit ends the stream.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Work {
    pub literals: u64,
    pub inline_matches: u64,
    pub full_matches: u64,
    pub extended_matches: u64,
    pub noops: u64,
    pub match_bytes: u64,
    pub descriptor_reloads: u64,
}

impl Work {
    /// Intrinsic MC68000 clock cycles, including routine setup and RTS.
    ///
    /// US bytes at `$041BEA` start `558F 1F58 0001 1E98 3A17 780F`:
    /// setup = 8+16+12+8+4 = 48. Restoring SP and RTS cost 8+16.
    /// The pinned Genesis Plus GX `core/m68k/m68ki_cycles.h` opcode table,
    /// with `m68kops.h`'s branch/DBF/shift adjustments, supplies the costs:
    ///
    /// - Saved-CCR bit: LSR #1 (8), MOVE SR (6), DBF taken (10), MOVE CCR
    ///   (12) = 36. Inline count bit adds ROXL #1 (8) to LSR+DBF = 26.
    /// - Descriptor expiry costs 44 extra: DBF expired (14) instead of
    ///   taken (10), plus the four descriptor-read instructions (40).
    /// - A copied match byte costs indexed MOVE (14), postincrement MOVE
    ///   (8), DBF taken (10) = 32; the final DBF costs 4 extra and the
    ///   return BRA costs 10. These 14 are in each match's overhead.
    /// - Full offset construction uses LSL #5 = 6+2*5 = 16, not its base 6.
    ///   Conditional branches cost 10 taken, 8 untaken short, 12 untaken
    ///   word (`BEQ.W` after the extended count's `CMPI`).
    ///
    /// Token costs including bit tests: literal 66; inline 172+14+32*n;
    /// full 164+14+32*n; extended 208+14+32*n; no-op 192; terminator
    /// 176+24. Thus setup+terminator is the fixed 248 below. Hardware wait
    /// costs must be modeled separately; this is not an elapsed-frame count.
    #[must_use]
    pub fn cpu_cycles(self) -> u64 {
        248 + 66 * self.literals
            + 186 * self.inline_matches
            + 178 * self.full_matches
            + 222 * self.extended_matches
            + 192 * self.noops
            + 32 * self.match_bytes
            + 44 * self.descriptor_reloads
    }
}

/// The `d5`/`d4` description-field pair.
struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    field: u16,
    remaining: u32,
    reloads: u64,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8], offset: usize) -> Result<Self, DecodeError> {
        let mut reader = BitReader {
            data,
            pos: offset,
            field: 0,
            remaining: 0,
            reloads: 0,
        };
        reader.reload()?;
        Ok(reader)
    }

    fn reload(&mut self) -> Result<(), DecodeError> {
        if self.pos + 2 > self.data.len() {
            return Err(error(format!(
                "Description field at 0x{:X} runs past end of input",
                self.pos
            )));
        }
        // move.b (a0)+,$1(sp) / move.b (a0)+,(sp) / move.w (sp),d5
        self.field = u16::from_le_bytes([self.data[self.pos], self.data[self.pos + 1]]);
        self.pos += 2;
        self.remaining = 16;
        Ok(())
    }

    /// `lsr.w #1,d5 ; move sr,d6 ; dbf d4`: the bit first, then the reload.
    fn bit(&mut self) -> Result<u8, DecodeError> {
        let value = (self.field & 1) as u8;
        self.field >>= 1;
        self.remaining -= 1;
        if self.remaining == 0 {
            self.reload()?;
            self.reloads += 1;
        }
        Ok(value)
    }

    fn byte(&mut self) -> Result<u8, DecodeError> {
        let value = *self
            .data
            .get(self.pos)
            .ok_or_else(|| error(format!("Read past end of input at 0x{:X}", self.pos)))?;
        self.pos += 1;
        Ok(value)
    }
}

/// `KosDecomp_MatchLoop`: byte by byte, so an overlapping run extends itself.
fn copy_match(out: &mut Vec<u8>, displacement: i32, count: usize) -> Result<(), DecodeError> {
    let start = out.len() as i64 + i64::from(displacement);
    if start < 0 {
        return Err(error(format!(
            "Match displacement {displacement} points before start of output"
        )));
    }
    let start = start as usize;
    for i in 0..count {
        out.push(out[start + i]);
    }
    Ok(())
}

/// Decompress the stream at `offset`: the output and the bytes the routine
/// read, its terminator included.
pub fn decompress(data: &[u8], offset: usize) -> Result<(Vec<u8>, usize), DecodeError> {
    let (bytes, consumed, _) = decompress_with_work(data, offset)?;
    Ok((bytes, consumed))
}

/// The same decode with the retail instruction-path statistics. No emulated
/// CPU, hardware state or elapsed-time assumptions enter this calculation.
pub fn decompress_with_work(
    data: &[u8],
    offset: usize,
) -> Result<(Vec<u8>, usize, Work), DecodeError> {
    if offset > data.len() {
        return Err(error(format!("Offset 0x{offset:X} is outside the input")));
    }
    let mut reader = BitReader::new(data, offset)?;
    let mut out = Vec::new();
    let mut work = Work::default();
    loop {
        if reader.bit()? == 1 {
            // KosDecomp_ChkBit: copy one byte as is.
            let byte = reader.byte()?;
            out.push(byte);
            work.literals += 1;
            continue;
        }
        if reader.bit()? == 0 {
            // KosDecomp_Match: high count bit, then low.
            let count = (reader.bit()? << 1) | reader.bit()?;
            let displacement = i32::from(reader.byte()?) - 0x100;
            let count = usize::from(count) + 2;
            copy_match(&mut out, displacement, count)?;
            work.inline_matches += 1;
            work.match_bytes += count as u64;
            continue;
        }
        // KosDecomp_FullMatch: $E000 | ((byte1 & $F8) << 5) | byte0, sign
        // extended from 16 bits: -8192..-1.
        let low = reader.byte()?;
        let high = reader.byte()?;
        let word = 0xE000 | (u16::from(high & 0xF8) << 5) | u16::from(low);
        let displacement = i32::from(word as i16);
        let count = high & 7;
        if count != 0 {
            let count = usize::from(count) + 2;
            copy_match(&mut out, displacement, count)?;
            work.full_matches += 1;
            work.match_bytes += count as u64;
            continue;
        }
        // KosDecomp_FullMatch2
        match reader.byte()? {
            0 => break,
            // Back to the main loop; the field is refilled only when empty.
            1 => work.noops += 1,
            extended => {
                let count = usize::from(extended) + 1;
                copy_match(&mut out, displacement, count)?;
                work.extended_matches += 1;
                work.match_bytes += count as u64;
            }
        }
    }
    work.descriptor_reloads = reader.reloads;
    Ok((out, reader.pos - offset, work))
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn literals_matches_and_the_terminator() {
        // Field bits, LSB first: 1 1 (two literals), 0 0 c1 c0 (inline match
        // of count 0+2 from -2), 0 1 (full match, terminator).
        // Bits: 1,1,0,0,0,0,0,1 -> 0b1000_0011 = 0x83; rest zero.
        let stream = [0x83, 0x00, b'a', b'b', 0xFE, 0x00, 0xF8, 0x00];
        let (out, consumed) = decompress(&stream, 0).unwrap();
        assert_eq!(out, b"abab");
        assert_eq!(consumed, stream.len());
    }

    #[test]
    fn an_overlapping_match_extends_itself() {
        // 1 (literal), 0 0 1 1 (inline match, count 3+2, from -1), 0 1 end.
        // Bits: 1,0,0,1,1,0,1,0... -> 0b0101_1001 = 0x59.
        let stream = [0x59, 0x00, b'x', 0xFF, 0xF8, 0x00, 0x00];
        let (out, _) = decompress(&stream, 0).unwrap();
        assert_eq!(out, b"xxxxxx");
    }

    #[test]
    fn refuses_bad_input() {
        assert!(decompress(&[0x00], 0).is_err());
        // An inline match before any output.
        assert!(decompress(&[0x00, 0x00, 0xFF], 0).is_err());
        assert!(decompress(&[0x00, 0x00], 3).is_err());
    }
}
