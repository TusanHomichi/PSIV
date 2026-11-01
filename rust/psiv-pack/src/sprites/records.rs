//! Sprite mapping and animation records, byte for byte:
//! `psiv_tools/sprites/records.py`, whose package docstring is the transcription.

use crate::PackError;

pub const FACINGS: [(u8, &str); 4] = [(0, "down"), (4, "up"), (8, "right"), (0xC, "left")];
pub const TILE_INDEX_MASK: u16 = 0x07FF;
pub const HFLIP_BIT: u16 = 1 << 11;
pub const VFLIP_BIT: u16 = 1 << 12;
/// `NemDecomp_ToRAM` destinations are `$FFFF0000 | tile << 5`.
pub const RAM_BASE: u32 = 0xFFFF_0000;
pub const ROM_LIMIT: u32 = 0x40_0000;
const PIECE_SIZE: usize = 6;
const SEQUENCE_PER_FRAME_FLAG: u8 = 0x80;

/// The name of a facing byte.
pub fn facing_name(facing: u8) -> Option<&'static str> {
    FACINGS
        .iter()
        .find(|(value, _)| *value == facing)
        .map(|(_, name)| *name)
}

/// `$13(a4)` -> CRAM line: retail field objects store only these four.
pub fn tile_props_line(props: u8) -> Option<u8> {
    match props {
        0x00 => Some(0),
        0x20 => Some(1),
        0x40 => Some(2),
        0x60 => Some(3),
        _ => None,
    }
}

pub(super) fn bounds(rom: &[u8], offset: usize, size: usize) -> Result<(), PackError> {
    if offset + size > rom.len() {
        return Err(PackError(format!(
            "read of {size} bytes at 0x{offset:06X} runs past the ROM"
        )));
    }
    Ok(())
}

pub(super) fn u8_at(rom: &[u8], offset: usize) -> Result<u8, PackError> {
    bounds(rom, offset, 1)?;
    Ok(rom[offset])
}

pub(super) fn s8_at(rom: &[u8], offset: usize) -> Result<i8, PackError> {
    Ok(u8_at(rom, offset)? as i8)
}

pub(super) fn u16_at(rom: &[u8], offset: usize) -> Result<u16, PackError> {
    bounds(rom, offset, 2)?;
    Ok(u16::from_be_bytes([rom[offset], rom[offset + 1]]))
}

pub(super) fn s16_at(rom: &[u8], offset: usize) -> Result<i16, PackError> {
    Ok(u16_at(rom, offset)? as i16)
}

pub(super) fn u32_at(rom: &[u8], offset: usize) -> Result<u32, PackError> {
    bounds(rom, offset, 4)?;
    Ok(u32::from_be_bytes([
        rom[offset],
        rom[offset + 1],
        rom[offset + 2],
        rom[offset + 3],
    ]))
}

pub(super) fn hex6(value: usize) -> String {
    format!("0x{value:06X}")
}

/// One six-byte sprite piece of a mapping record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Piece {
    pub y: i8,
    pub size_byte: u8,
    pub tile_word: u16,
    pub x: i8,
}

impl Piece {
    pub fn width_tiles(&self) -> usize {
        usize::from((self.size_byte >> 2) & 3) + 1
    }

    pub fn height_tiles(&self) -> usize {
        usize::from(self.size_byte & 3) + 1
    }
}

/// A `Mappings_*` record: one drawn frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mapping {
    pub header_byte: u8,
    pub pieces: Vec<Piece>,
}

/// `decode_mapping`: byte 0 is the piece count minus one.
pub fn decode_mapping(rom: &[u8], offset: usize) -> Result<Mapping, PackError> {
    let count = usize::from(u8_at(rom, offset)?) + 1;
    let header_byte = u8_at(rom, offset + 1)?;
    let mut pieces = Vec::with_capacity(count);
    for index in 0..count {
        let base = offset + 2 + index * PIECE_SIZE;
        bounds(rom, base, PIECE_SIZE)?;
        pieces.push(Piece {
            y: s8_at(rom, base)?,
            size_byte: u8_at(rom, base + 1)?,
            tile_word: u16_at(rom, base + 2)?,
            x: s8_at(rom, base + 4)?,
        });
    }
    Ok(Mapping {
        header_byte,
        pieces,
    })
}

/// A `SprMapsData_*` record: the frame list for one facing. A frame shows for
/// its duration byte plus one game frames.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnimationSequence {
    pub rom_offset: usize,
    pub per_frame_durations: bool,
    /// `(mapping offset, duration byte)`.
    pub frames: Vec<(usize, u8)>,
}

/// `decode_sequence`, in whichever of its two forms it is stored.
pub fn decode_sequence(rom: &[u8], offset: usize) -> Result<AnimationSequence, PackError> {
    let head = u8_at(rom, offset)?;
    let per_frame = head & SEQUENCE_PER_FRAME_FLAG != 0;
    let count = usize::from(if per_frame { head & 0x7F } else { head });
    if count == 0 {
        return Err(PackError(format!(
            "sequence at {} declares no frames",
            hex6(offset)
        )));
    }
    let mut durations = Vec::with_capacity(count);
    let table = if per_frame {
        for i in 0..count {
            durations.push(u8_at(rom, offset + 1 + i)?);
        }
        offset + ((count + 2) & !1)
    } else {
        durations.resize(count, u8_at(rom, offset + 1)?);
        offset + 2
    };
    let mut frames = Vec::with_capacity(count);
    for (i, duration) in durations.into_iter().enumerate() {
        frames.push((u32_at(rom, table + 4 * i)? as usize, duration));
    }
    Ok(AnimationSequence {
        rom_offset: offset,
        per_frame_durations: per_frame,
        frames,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mapping_stores_its_piece_count_minus_one() {
        let rom = [0, 7, 0xFF, 0b0110, 0x01, 0x23, 0xFE, 0x00];
        let mapping = decode_mapping(&rom, 0).unwrap();
        assert_eq!(mapping.header_byte, 7);
        let piece = mapping.pieces[0];
        assert_eq!((piece.y, piece.x, piece.tile_word), (-1, -2, 0x123));
        assert_eq!((piece.width_tiles(), piece.height_tiles()), (2, 3));
        assert!(
            decode_mapping(&rom[..7], 0)
                .unwrap_err()
                .0
                .contains("runs past")
        );
    }

    #[test]
    fn a_shared_duration_and_per_frame_durations_both_read() {
        let shared = [2, 9, 0, 0, 0, 1, 0, 0, 0, 2];
        let sequence = decode_sequence(&shared, 0).unwrap();
        assert!(!sequence.per_frame_durations);
        assert_eq!(sequence.frames, [(1, 9), (2, 9)]);
        // An even count pads to the next even offset; 2 frames: 1 + 2 is 3, so 4.
        let per_frame = [0x82, 5, 6, 0xAA, 0, 0, 0, 1, 0, 0, 0, 2];
        let sequence = decode_sequence(&per_frame, 0).unwrap();
        assert!(sequence.per_frame_durations);
        assert_eq!(sequence.frames, [(1, 5), (2, 6)]);
        assert!(
            decode_sequence(&[0x80], 0)
                .unwrap_err()
                .0
                .contains("no frames")
        );
        assert!(
            decode_sequence(&[0, 0], 0)
                .unwrap_err()
                .0
                .contains("no frames")
        );
    }
}
