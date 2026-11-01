//! Nemesis decompression: `psiv_tools/nemesis.py`, which transcribes the
//! game's `NemDecomp` / `NemDecomp_Main` and records the three details taken
//! from the routine (the header fixes the output length; one byte of
//! lookahead may be read; XOR mode accumulates across rows). This port keeps
//! its branches one for one.

use super::DecodeError;

const ROWS_PER_TILE: u32 = 8;
/// `cmpi.b #$FC,d1`: six leading 1 bits mark inline data.
const INLINE_MARKER: u8 = 0xFC;

fn error(message: String) -> DecodeError {
    DecodeError(message)
}

/// The header word, decoded the way `NemDecomp_Main` decodes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    pub tile_count: u16,
    pub xor_mode: bool,
    pub row_count: u16,
}

pub fn read_header(data: &[u8], offset: usize) -> Result<Header, DecodeError> {
    if offset + 2 > data.len() {
        return Err(error(format!(
            "Header at 0x{offset:X} runs past end of input"
        )));
    }
    let raw = u16::from_be_bytes([data[offset], data[offset + 1]]);
    // lsl.w #1,d2 / bcc: the sign bit is the XOR flag and is shifted out.
    let tile_count = raw & 0x7FFF;
    Ok(Header {
        tile_count,
        xor_mode: raw & 0x8000 != 0,
        // lsl.w #2,d2 / movea.w d2,a5: the row counter is a 16-bit register.
        row_count: tile_count.wrapping_shl(3),
    })
}

/// `a0` plus the `d5`/`d6` bit window from `Nem_ProcessCompressedData`.
struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    window: u16,
    bits: u32,
}

impl Reader<'_> {
    fn byte(&mut self) -> Result<u8, DecodeError> {
        let value = *self
            .data
            .get(self.pos)
            .ok_or_else(|| error(format!("Read past end of input at 0x{:X}", self.pos)))?;
        self.pos += 1;
        Ok(value)
    }

    fn prime(&mut self) -> Result<(), DecodeError> {
        // move.b (a0)+,d5 / asl.w #8,d5 / move.b (a0)+,d5 / move.w #$10,d6
        let high = self.byte()?;
        let low = self.byte()?;
        self.window = u16::from_be_bytes([high, low]);
        self.bits = 0x10;
        Ok(())
    }

    /// `d1 = d5 >> (d6-8)`: the next eight code bits.
    fn peek8(&self) -> u8 {
        (self.window >> (self.bits - 8)) as u8
    }

    /// The `cmpi.w #9,d6` refill: `asl.w #8,d5` drops the consumed byte.
    fn refill(&mut self) -> Result<(), DecodeError> {
        if self.bits < 9 {
            self.bits += 8;
            self.window = (self.window << 8) | u16::from(self.byte()?);
        }
        Ok(())
    }

    fn take(&mut self, count: u32) -> Result<(), DecodeError> {
        self.bits -= count;
        self.refill()
    }
}

/// One `Nem_Code_Table` entry: code length, repeat count, palette index.
type Code = (u8, u8, u8);

/// `Nem_BuildCodeTable`: the prefix code expanded to 256 lookahead slots,
/// and the position of the first compressed byte.
fn build_code_table(
    data: &[u8],
    offset: usize,
) -> Result<([Option<Code>; 256], usize), DecodeError> {
    let mut table = [None; 256];
    let mut pos = offset;
    let mut read = || -> Result<u8, DecodeError> {
        let value = *data
            .get(pos)
            .ok_or_else(|| error(format!("Code table at 0x{offset:X} runs past end of input")))?;
        pos += 1;
        Ok(value)
    };
    let mut marker = read()?;
    loop {
        // Nem_BCT_ChkEnd
        if marker == 0xFF {
            return Ok((table, pos));
        }
        // Nem_BCT_NewPalIndex: andi.w #$F keeps the low nybble.
        let palette_index = marker & 0x0F;
        loop {
            let descriptor = read()?;
            // cmpi.b #$80 / bcc: a set sign bit is the next palette marker.
            if descriptor >= 0x80 {
                marker = descriptor;
                break;
            }
            let length = descriptor & 0x0F;
            let repeat = (descriptor & 0x70) >> 4;
            let code = read()?;
            if length > 8 {
                return Err(error(format!(
                    "Code table at 0x{offset:X}: code length {length} exceeds 8 bits"
                )));
            }
            let shift = 8 - u32::from(length);
            let base = usize::from(code) << shift;
            let span = 1usize << shift;
            if base + span > 0x100 {
                return Err(error(format!(
                    "Code table at 0x{offset:X}: {length}-bit code 0x{code:02X} \
                     expands past the 256-entry table"
                )));
            }
            for slot in &mut table[base..base + span] {
                *slot = Some((length, repeat, palette_index));
            }
        }
    }
}

/// Decompress the stream at `offset`: the output and the bytes the 68000
/// routine would have read (the stream's length, or one more).
pub fn decompress(data: &[u8], offset: usize) -> Result<(Vec<u8>, usize), DecodeError> {
    if offset > data.len() {
        return Err(error(format!("Offset 0x{offset:X} is outside the input")));
    }
    let header = read_header(data, offset)?;
    let (table, pos) = build_code_table(data, offset + 2)?;
    let mut out = Vec::with_capacity(usize::from(header.row_count) * 4);
    if header.row_count == 0 {
        return Ok((out, pos - offset));
    }
    let mut reader = Reader {
        data,
        pos,
        window: 0,
        bits: 0,
    };
    reader.prime()?;

    let mut rows_left = header.row_count; // a5
    let mut row: u32 = 0; // d4
    let mut nybbles_left = ROWS_PER_TILE; // d3
    let mut previous: u32 = 0; // d2, the XOR accumulator

    loop {
        let lookahead = reader.peek8();
        let (repeat, palette_index) = if lookahead >= INLINE_MARKER {
            // Nem_PCD_InlineData: 6 marker bits, then 7 payload bits, taken
            // in two steps with a refill check between.
            reader.take(6)?;
            reader.bits -= 7;
            let payload = reader.window >> reader.bits;
            let entry = ((payload & 0x70) >> 4, payload & 0x0F);
            reader.refill()?;
            (entry.0 as u8, entry.1 as u8)
        } else {
            let (length, repeat, palette_index) =
                table[usize::from(lookahead)].ok_or_else(|| {
                    error(format!(
                        "Undefined code 0x{lookahead:02X} at 0x{:X}; the code table does not \
                     cover this bit pattern",
                        reader.pos
                    ))
                })?;
            reader.take(u32::from(length))?;
            (repeat, palette_index)
        };
        // Nem_PCD_WritePixel: dbf runs repeat+1 times.
        for _ in 0..=repeat {
            row = (row << 4) | u32::from(palette_index);
            nybbles_left -= 1;
            if nybbles_left != 0 {
                continue;
            }
            if header.xor_mode {
                // Nem_PCD_WriteRowToRAM_XOR: d2 is never reset.
                previous ^= row;
                out.extend_from_slice(&previous.to_be_bytes());
            } else {
                out.extend_from_slice(&row.to_be_bytes());
            }
            rows_left -= 1;
            if rows_left == 0 {
                return Ok((out, reader.pos - offset));
            }
            // Nem_PCD_NewRow
            row = 0;
            nybbles_left = ROWS_PER_TILE;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One tile of one colour: the code `0` (1 bit) is palette index 5,
    /// repeat 7 (eight pixels), so eight codes fill eight rows.
    fn solid_tile(xor: bool) -> Vec<u8> {
        let mut stream = vec![if xor { 0x80 } else { 0x00 }, 0x01];
        // Palette marker 0x85, descriptor: repeat 7, length 1; code 0.
        stream.extend_from_slice(&[0x85, 0x71, 0x00, 0xFF]);
        // Eight 1-bit codes of 0, then padding for the lookahead.
        stream.extend_from_slice(&[0x00, 0x00, 0x00]);
        stream
    }

    #[test]
    fn decodes_a_solid_tile() {
        let (out, consumed) = decompress(&solid_tile(false), 0).unwrap();
        assert_eq!(out, [0x55; 32]);
        assert!(consumed <= solid_tile(false).len());
    }

    #[test]
    fn xor_mode_accumulates_rows() {
        let (out, _) = decompress(&solid_tile(true), 0).unwrap();
        // Rows alternate between the row and zero.
        for (index, row) in out.chunks(4).enumerate() {
            let expected = if index % 2 == 0 { 0x5555_5555u32 } else { 0 };
            assert_eq!(u32::from_be_bytes(row.try_into().unwrap()), expected);
        }
    }

    #[test]
    fn refuses_truncated_and_undefined_input() {
        assert!(decompress(&[0x00], 0).is_err());
        let truncated = &solid_tile(false)[..7];
        assert!(decompress(truncated, 0).is_err());
        // A table whose only code is `0`: a lookahead of $80 has no entry.
        let mut undefined = vec![0x00, 0x01, 0x85, 0x71, 0x00, 0xFF];
        undefined.extend_from_slice(&[0x80, 0x00, 0x00]);
        assert!(decompress(&undefined, 0).is_err());
    }
}
