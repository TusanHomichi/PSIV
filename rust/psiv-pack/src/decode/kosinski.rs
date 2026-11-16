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
    /// US setup at `$041BEA`: stack allocation, two descriptor-byte moves,
    /// descriptor load and bit-counter initialization cost
    /// 8+16+12+8+4 = 48. Restoring SP and RTS cost 8+16.
    /// Verify the instruction bytes locally; cartridge excerpts stay out of Git.
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
    #[cfg(test)]
    super::observed::record(super::Call::Kosinski { offset });
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

    /// A minimal Kosinski encoder for fixtures (tests/test_formations.py's
    /// `KosinskiWriter`): a description field is reserved the moment the
    /// previous one fills, before the operand bytes of the element whose
    /// bit filled it, as `KosDecomp` reloads.
    struct Writer {
        out: Vec<u8>,
        field_at: usize,
        bits: u16,
        count: u32,
    }

    impl Writer {
        fn new() -> Self {
            let mut writer = Writer {
                out: Vec::new(),
                field_at: 0,
                bits: 0,
                count: 0,
            };
            writer.reserve();
            writer
        }

        fn reserve(&mut self) {
            self.field_at = self.out.len();
            self.out.extend([0, 0]);
            (self.bits, self.count) = (0, 0);
        }

        fn store(&mut self) {
            self.out[self.field_at..self.field_at + 2].copy_from_slice(&self.bits.to_le_bytes());
        }

        fn bit(&mut self, value: bool) {
            self.bits |= u16::from(value) << self.count;
            self.count += 1;
            if self.count == 16 {
                self.store();
                self.reserve();
            }
        }

        fn literals(mut self, data: &[u8]) -> Self {
            for &byte in data {
                self.bit(true);
                self.out.push(byte);
            }
            self
        }

        fn full(&mut self, displacement: i32, count: u8) {
            assert!((-8192..=-1).contains(&displacement) && count <= 7);
            let word = displacement as u16;
            self.bit(false);
            self.bit(true);
            self.out.push(word as u8);
            self.out.push(((word >> 5) as u8 & 0xF8) | count);
        }

        fn full_match(mut self, displacement: i32, length: u8) -> Self {
            assert!((3..=9).contains(&length));
            self.full(displacement, length - 2);
            self
        }

        fn finish(mut self) -> Vec<u8> {
            self.full(-0x2000, 0);
            self.out.push(0);
            self.store();
            self.out
        }
    }

    #[test]
    fn a_full_match_reaches_back_across_both_displacement_bytes() {
        // -8 fits the low byte; -300 and -8192 need the high byte's top five
        // bits, which `KosDecomp_FullMatch` shifts left by five. The window
        // has no period, so a wrong displacement copies different bytes.
        let base: Vec<u8> = (0..8192u32)
            .map(|i| (i.wrapping_mul(0x9E37_79B1) >> 24) as u8)
            .collect();
        for (displacement, length) in [(-8, 5u8), (-300, 9), (-8192, 3)] {
            let stream = Writer::new()
                .literals(&base)
                .full_match(displacement, length)
                .finish();
            let (out, consumed) = decompress(&stream, 0).unwrap();
            let from = base.len() - displacement.unsigned_abs() as usize;
            assert_eq!(&out[base.len()..], &base[from..from + usize::from(length)]);
            assert_eq!(consumed, stream.len());
        }
    }

    #[test]
    fn twenty_literals_need_a_second_field_before_the_sixteenth_byte() {
        let data: Vec<u8> = (1..=20).collect();
        let stream = Writer::new().literals(&data).finish();
        assert_eq!(decompress(&stream, 0).unwrap(), (data, stream.len()));
        // Field, sixteen literals, the second field, four literals and the
        // three-byte terminator.
        assert_eq!(stream.len(), 2 + 16 + 2 + 4 + 3);
    }

    #[test]
    fn a_truncated_stream_is_refused() {
        let stream = Writer::new().literals(b"abc").finish();
        assert!(decompress(&stream, 0).is_ok());
        for cut in 1..stream.len() {
            assert!(decompress(&stream[..cut], 0).is_err(), "cut at {cut}");
        }
    }

    #[test]
    fn refuses_bad_input() {
        assert!(decompress(&[0x00], 0).is_err());
        // An inline match before any output.
        assert!(decompress(&[0x00, 0x00, 0xFF], 0).is_err());
        assert!(decompress(&[0x00, 0x00], 3).is_err());
    }
}
