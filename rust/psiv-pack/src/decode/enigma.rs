//! Enigma decompression: `psiv_tools/enigma.py`, which transcribes the
//! game's `EniDecomp` and records the six details taken from the routine
//! (only the V and H flags; the 7-bit format read given back; `dbf` counts;
//! the straddle reload; the base added, not or-ed; flag bits without a
//! refill; the rounded consumed length). This port keeps its branches one
//! for one. The output is pattern-name words.

use super::DecodeError;

const WINDOW_BITS: i32 = 0x10;
/// Header bit 1 -> `ori.w #$1000`; bit 0 -> `ori.w #$800`.
const FLAG_V_FLIP: u8 = 0x02;
const FLAG_H_FLIP: u8 = 0x01;
const SUPPORTED_FLAGS: u8 = FLAG_V_FLIP | FLAG_H_FLIP;
const WORD_V_FLIP: u16 = 0x1000;
const WORD_H_FLIP: u16 = 0x0800;
/// `EniDecomp_Masks` covers 1..16 bits.
const MAX_INLINE_BITS: u8 = 16;

fn error(message: String) -> DecodeError {
    DecodeError(message)
}

/// `EniDecomp_Masks`, indexed by bit count.
fn mask(bits: i32) -> u16 {
    if bits >= 16 {
        0xFFFF
    } else {
        (1u16 << bits) - 1
    }
}

/// The five header bytes and the two copy words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    pub inline_bits: u8,
    pub flags: u8,
    /// `a2`, already offset by the base tile.
    pub incremental_word: u16,
    /// `a4`, already offset by the base tile.
    pub literal_word: u16,
    /// `a3`, the caller's `d0`.
    pub base_tile: u16,
}

pub fn read_header(data: &[u8], offset: usize, base_tile: u16) -> Result<Header, DecodeError> {
    if offset + 6 > data.len() {
        return Err(error(format!(
            "Header at 0x{offset:X} runs past end of input"
        )));
    }
    // move.b (a0)+, d0 / ext.w d0 / movea.w d0, a5
    let inline_bits = data[offset];
    if !(1..=MAX_INLINE_BITS).contains(&inline_bits) {
        return Err(error(format!(
            "Enigma stream at 0x{offset:X} declares {inline_bits} inline bits; \
             EniDecomp_Masks only covers 1..{MAX_INLINE_BITS}"
        )));
    }
    let flags = data[offset + 1];
    if flags & !SUPPORTED_FLAGS != 0 {
        return Err(error(format!(
            "Enigma stream at 0x{offset:X} declares flag byte 0x{flags:02X}; EniDecomp only \
             implements the V ($02) and H ($01) bits, so any other bit would leave its \
             per-value bits unconsumed"
        )));
    }
    let incremental = u16::from_be_bytes([data[offset + 2], data[offset + 3]]);
    let literal = u16::from_be_bytes([data[offset + 4], data[offset + 5]]);
    Ok(Header {
        inline_bits,
        flags,
        incremental_word: incremental.wrapping_add(base_tile),
        literal_word: literal.wrapping_add(base_tile),
        base_tile,
    })
}

/// `a0`, the 16-bit window `d5` and the shift counter `d6`.
struct Stream<'a> {
    data: &'a [u8],
    pos: usize,
    window: u16,
    shift: i32,
}

impl Stream<'_> {
    fn byte_at(&self, pos: usize) -> Result<u8, DecodeError> {
        self.data
            .get(pos)
            .copied()
            .ok_or_else(|| error(format!("Read past end of input at 0x{pos:X}")))
    }

    fn next_byte(&mut self) -> Result<u8, DecodeError> {
        let value = self.byte_at(self.pos)?;
        self.pos += 1;
        Ok(value)
    }

    /// `EniDecomp_FetchByte`: drop `used` bits, refill below 9.
    fn fetch_byte(&mut self, used: i32) -> Result<(), DecodeError> {
        self.shift -= used;
        if self.shift < 9 {
            self.shift += 8;
            self.window = (self.window << 8) | u16::from(self.next_byte()?);
        }
        Ok(())
    }

    /// `loc_41B92`'s tail: a fresh window from the next two bytes.
    fn reload_window(&mut self) -> Result<(), DecodeError> {
        let high = self.next_byte()?;
        let low = self.next_byte()?;
        self.window = u16::from_be_bytes([high, low]);
        Ok(())
    }

    /// `subq.w #1, d6 / btst d6, d5`: no refill check, deliberately.
    fn flag_bit(&mut self) -> Result<bool, DecodeError> {
        self.shift -= 1;
        if self.shift < 0 {
            return Err(error(format!(
                "Bit counter underflowed at 0x{:X}; the stream is not Enigma",
                self.pos
            )));
        }
        Ok((self.window >> self.shift) & 1 == 1)
    }
}

/// `EniDecomp_FetchInlineValue`: flags, then `inline_bits` bits, plus base.
fn fetch_inline_value(stream: &mut Stream<'_>, header: &Header) -> Result<u16, DecodeError> {
    let mut value_base = header.base_tile; // move.w a3, d3
    if header.flags & FLAG_V_FLIP != 0 && stream.flag_bit()? {
        value_base |= WORD_V_FLIP;
    }
    if header.flags & FLAG_H_FLIP != 0 && stream.flag_bit()? {
        value_base |= WORD_H_FLIP;
    }
    let bits = i32::from(header.inline_bits);
    let value_mask = mask(bits);
    let mut value = stream.window; // move.w d5, d1
    let remainder = stream.shift - bits; // sub.w a5, d7

    if remainder < 0 {
        // The value straddles the window: take what is left, then borrow the
        // top bits of the next byte without consuming it.
        stream.shift = remainder + WINDOW_BITS;
        let short_by = -remainder;
        value = ((u32::from(value) << short_by) & 0xFFFF) as u16;
        // move.b (a0), d5 replaces the window's low byte; rol.b rotates that
        // byte alone, and the and.w after it is 16 bits wide.
        let next = stream.byte_at(stream.pos)?;
        let borrowed = ((stream.window & 0xFF00)
            | u16::from(next.rotate_left(short_by as u32 & 7)))
            & mask(short_by);
        value = value.wrapping_add(borrowed);
        value = (value & value_mask).wrapping_add(value_base);
        stream.reload_window()?;
        return Ok(value);
    }
    if remainder == 0 {
        // Consumes the window exactly: d6 = 16 and a wholesale reload.
        stream.shift = WINDOW_BITS;
        value = (value & value_mask).wrapping_add(value_base);
        stream.reload_window()?;
        return Ok(value);
    }
    // loc_41BA4: inside the window.
    value >>= remainder;
    value = (value & value_mask).wrapping_add(value_base);
    stream.fetch_byte(bits)?;
    Ok(value)
}

/// Decompress the stream at `offset` with the caller's base tile: the words,
/// and the consumed length rounded as `EniDecomp_Done` rounds it.
/// `max_words` bounds the output as the destination buffer does.
pub fn decompress(
    data: &[u8],
    offset: usize,
    base_tile: u16,
    max_words: Option<usize>,
) -> Result<(Vec<u16>, usize), DecodeError> {
    if offset > data.len() {
        return Err(error(format!("Offset 0x{offset:X} is outside the input")));
    }
    let header = read_header(data, offset, base_tile)?;
    let mut stream = Stream {
        data,
        pos: offset + 6,
        window: 0,
        shift: WINDOW_BITS,
    };
    // move.b (a0)+, d5 / asl.w #8, d5 / move.b (a0)+, d5
    stream.reload_window()?;
    let mut out: Vec<u16> = Vec::new();
    let mut incremental = header.incremental_word; // a2
    let emit = |out: &mut Vec<u16>, word: u16| -> Result<(), DecodeError> {
        if max_words.is_some_and(|max| out.len() >= max) {
            return Err(error(format!(
                "Enigma stream at 0x{offset:X} produced more than {} words; it does not \
                 fit the destination it was decoded for",
                max_words.unwrap_or_default()
            )));
        }
        out.push(word);
        Ok(())
    };

    loop {
        // EniDecompLoop: always read 7 bits, then decide whether it was 6.
        if stream.shift < 7 {
            return Err(error(format!(
                "Only {} bits left in the window at 0x{:X}; the stream is not Enigma",
                stream.shift, stream.pos
            )));
        }
        let entry = (stream.window >> (stream.shift - 7)) & 0x7F;
        let mut count = entry;
        let used = if entry >= 0x40 {
            7
        } else {
            count >>= 1; // lsr.w #1, d2: the 7th bit belongs to the next entry
            6
        };
        stream.fetch_byte(used)?;
        let count = count & 0x0F; // andi.w #$F, d2
        let mode = entry >> 4;
        match mode {
            // EniDecomp_00: the incremental word, incremented.
            0 | 1 => {
                for _ in 0..=count {
                    emit(&mut out, incremental)?;
                    incremental = incremental.wrapping_add(1);
                }
            }
            // EniDecomp_01: the literal word.
            2 | 3 => {
                for _ in 0..=count {
                    emit(&mut out, header.literal_word)?;
                }
            }
            // EniDecomp_100 / 101 / 110
            4..=6 => {
                let mut value = fetch_inline_value(&mut stream, &header)?;
                for _ in 0..=count {
                    emit(&mut out, value)?;
                    value = match mode {
                        5 => value.wrapping_add(1),
                        6 => value.wrapping_sub(1),
                        _ => value,
                    };
                }
            }
            // EniDecomp_111
            _ => {
                if count == 0x0F {
                    break;
                }
                for _ in 0..=count {
                    let value = fetch_inline_value(&mut stream, &header)?;
                    emit(&mut out, value)?;
                }
            }
        }
    }
    // EniDecomp_Done
    let mut end = stream.pos - 1;
    if stream.shift == WINDOW_BITS {
        end -= 1;
    }
    if end & 1 == 1 {
        end += 1;
    }
    Ok((out, end - offset))
}

/// The words as the 68000 wrote them: big-endian, in order.
pub fn words_to_bytes(words: &[u16]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_be_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 4-bit inline values, no flags, incremental 0x0010, literal 0x0020.
    fn header() -> Vec<u8> {
        vec![0x04, 0x00, 0x00, 0x10, 0x00, 0x20]
    }

    #[test]
    fn incremental_literal_and_end() {
        // Entries (6 bits unless the top bit is set):
        // 00 0010 -> 3 incremental words; 01 0001 -> 2 literals;
        // 111 1111 -> end. Bits: 000010 010001 1111111 + padding.
        // 0000 1001 0001 1111 1110 0000 ...
        let mut stream = header();
        stream.extend_from_slice(&[0x09, 0x1F, 0xE0, 0x00, 0x00, 0x00]);
        let (words, consumed) = decompress(&stream, 0, 0, None).unwrap();
        assert_eq!(words, [0x10, 0x11, 0x12, 0x20, 0x20]);
        assert_eq!(consumed % 2, 0);
        // The base tile is added to every word.
        let (based, _) = decompress(&stream, 0, 0x6400, None).unwrap();
        assert_eq!(based, [0x6410, 0x6411, 0x6412, 0x6420, 0x6420]);
        // A destination of four words refuses the fifth.
        assert!(decompress(&stream, 0, 0, Some(4)).is_err());
    }

    #[test]
    fn inline_value_modes() {
        // 100 0001 + value 0101 -> 0x5, 0x5; 101 0000 + 0011 -> 0x3;
        // 110 0001 + 1000 -> 0x8, 0x7; 111 1111 end.
        // 1000001 0101 1010000 0011 1100001 1000 1111111
        let bits = "1000001010110100000011110000110001111111";
        let mut stream = header();
        let mut bytes: Vec<u8> = bits
            .as_bytes()
            .chunks(8)
            .map(|c| {
                let s = std::str::from_utf8(c).unwrap();
                u8::from_str_radix(&format!("{s:0<8}"), 2).unwrap()
            })
            .collect();
        bytes.extend_from_slice(&[0, 0, 0, 0]);
        stream.extend_from_slice(&bytes);
        let (words, _) = decompress(&stream, 0, 0, None).unwrap();
        assert_eq!(words, [0x5, 0x5, 0x3, 0x8, 0x7]);
    }

    #[test]
    fn refuses_headers_the_routine_cannot_run() {
        let mut zero_bits = header();
        zero_bits[0] = 0;
        assert!(decompress(&zero_bits, 0, 0, None).is_err());
        let mut priority = header();
        priority[1] = 0x04;
        assert!(decompress(&priority, 0, 0, None).is_err());
        assert!(decompress(&header()[..5], 0, 0, None).is_err());
    }

    #[test]
    fn words_are_big_endian() {
        assert_eq!(words_to_bytes(&[0x1234, 0x00FF]), [0x12, 0x34, 0x00, 0xFF]);
    }
}
