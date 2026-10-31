//! Kosinski decompression: `psiv_tools/kosinski.py`, which transcribes the
//! game's `KosDecomp`. Description fields are little-endian words consumed
//! LSB first, and a field is reloaded only after its sixteenth bit is taken,
//! so `consumed` matches the routine's own read count.

use super::DecodeError;

fn error(message: String) -> DecodeError {
    DecodeError(message)
}

/// The `d5`/`d4` description-field pair.
struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    field: u16,
    remaining: u32,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8], offset: usize) -> Result<Self, DecodeError> {
        let mut reader = BitReader {
            data,
            pos: offset,
            field: 0,
            remaining: 0,
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
    if offset > data.len() {
        return Err(error(format!("Offset 0x{offset:X} is outside the input")));
    }
    let mut reader = BitReader::new(data, offset)?;
    let mut out = Vec::new();
    loop {
        if reader.bit()? == 1 {
            // KosDecomp_ChkBit: copy one byte as is.
            let byte = reader.byte()?;
            out.push(byte);
            continue;
        }
        if reader.bit()? == 0 {
            // KosDecomp_Match: high count bit, then low.
            let count = (reader.bit()? << 1) | reader.bit()?;
            let displacement = i32::from(reader.byte()?) - 0x100;
            copy_match(&mut out, displacement, usize::from(count) + 2)?;
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
            copy_match(&mut out, displacement, usize::from(count) + 2)?;
            continue;
        }
        // KosDecomp_FullMatch2
        match reader.byte()? {
            0 => break,
            // Back to the main loop; the field is refilled only when empty.
            1 => continue,
            extended => copy_match(&mut out, displacement, usize::from(extended) + 1)?,
        }
    }
    Ok((out, reader.pos - offset))
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

    #[test]
    fn refuses_bad_input() {
        assert!(decompress(&[0x00], 0).is_err());
        // An inline match before any output.
        assert!(decompress(&[0x00, 0x00, 0xFF], 0).is_err());
        assert!(decompress(&[0x00, 0x00], 3).is_err());
    }
}
