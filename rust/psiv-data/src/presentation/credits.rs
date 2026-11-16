//! Credits data loaded by retail $079D0C..$07A0A6. Tile words retain all VDP
//! attributes; art is raw packed 4bpp, independent of the current CRAM.
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::DataError;

/// Required credits contract, independently versioned from the pack envelope.
pub const CREDITS_FILE: &str = "presentation/credits.json";
/// First complete credits contract.
pub const CREDITS_VERSION: u32 = 1;

/// Inputs for `EndingCreditsAssets` and all three `EndingCreditsStage`s.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EndingCreditsAssets {
    /// Contract version, independent of the pack envelope.
    pub version: u32,
    /// ROM identity shared with the enclosing manifest.
    pub source_rom_sha256: String,
    /// Small planet, large planet and credit font, in upload order.
    pub art: Vec<CreditsArt>,
    /// The signed tile upload command and its terminator.
    pub font_upload: CreditsFontUpload,
    /// Five complete pattern-name streams in source-address order.
    pub mappings: Vec<CreditsMapping>,
    /// Initial uploads to palette buffer words 0 and 48.
    pub palettes: Vec<CreditsPalette>,
    /// The two stage command tables, including their sentinels.
    pub commands: Vec<CreditsCommands>,
    /// Header records indexed by CredText_A >> 3.
    pub headers: Vec<CreditsText>,
    /// Name records indexed by (CredText_B >> 3) * 4 plus line.
    pub names: Vec<CreditsText>,
    /// Eight five-colour rows selected by CreditsFadeInOut.
    pub fade: CreditsPalette,
}

/// Decompressed tiles, in upload order. Two colour indices per byte, high first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditsArt {
    /// Source address in the verified US retail ROM.
    pub rom_addr: u32,
    /// Retail decompressor used for this stream.
    pub compression: String,
    /// Compressed bytes consumed, including the stream terminator.
    pub consumed: usize,
    /// VRAM byte address where the tiles are uploaded.
    pub vram_destination: u16,
    /// Packed 4bpp bytes, 32 per 8×8 tile, high nibble first.
    pub tiles: Vec<u8>,
}

/// The signed tile command and sentinel at $07A26C.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditsFontUpload {
    /// Source address in the verified US retail ROM.
    pub rom_addr: u32,
    /// Destination tile number passed to the VRAM address helper.
    pub destination_tile: u16,
    /// Nemesis source pointer read from the upload command.
    pub art_rom_addr: u32,
    /// Signed negative upload sentinel.
    pub terminator: u16,
}

/// Enigma's d0 is an additive base tile word, never a length.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditsMapping {
    /// Source address in the verified US retail ROM.
    pub rom_addr: u32,
    /// Compressed bytes consumed, including the stream terminator.
    pub consumed: usize,
    /// Additive pattern-name base passed in d0 to EniDecomp.
    pub base_tile: u16,
    /// All RAM destinations receiving this decoded stream.
    pub ram_destinations: Vec<u32>,
    /// Number of tile columns consumed by PlaneMapToRAM.
    pub columns: usize,
    /// Number of tile rows in the complete decoded stream.
    pub rows: usize,
    /// Big-endian words, decoded without discarding attribute bits.
    pub words: Vec<u16>,
}

/// Palette buffer destination is a word index. Fade holds eight five-word rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditsPalette {
    /// Source address in the verified US retail ROM.
    pub rom_addr: u32,
    /// First destination word in Palette_Table_Buffer.
    pub start_word: usize,
    /// Big-endian words, decoded without discarding attribute bits.
    pub words: Vec<u16>,
}

/// (TextCounter & $3F, opcode) pairs followed by $FF and its alignment byte.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditsCommands {
    /// Source address in the verified US retail ROM.
    pub rom_addr: u32,
    /// Original record bytes, including terminator and command alignment.
    pub bytes: Vec<u8>,
}

/// An ordinal record, including its $FF terminator. Headers use the credit
/// tile charset; names use the normal RunText2 font charset. No lossy string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditsText {
    /// Source address in the verified US retail ROM.
    pub rom_addr: u32,
    /// Original record bytes, including terminator and command alignment.
    pub bytes: Vec<u8>,
}

impl EndingCreditsAssets {
    /// Refuses old, absent or malformed contracts with an actionable rebuild
    /// error. `sha256` is the enclosing manifest's verified ROM identity.
    pub fn load(pack_dir: &Path, sha256: &str) -> Result<Self, DataError> {
        let path = pack_dir.join(CREDITS_FILE);
        let fail = |message: String| DataError::Presentation {
            path: path.clone(),
            message: format!("{message}; rebuild the pack with psiv-pack"),
        };
        let bytes = std::fs::read(&path).map_err(|e| fail(e.to_string()))?;
        let value: Self = serde_json::from_slice(&bytes).map_err(|e| fail(e.to_string()))?;
        value.validate(sha256).map_err(fail)?;
        Ok(value)
    }

    /// Check completeness, identities, dimensions and references without a ROM.
    pub fn validate(&self, sha256: &str) -> Result<(), String> {
        let require = |ok: bool, message: &str| {
            if ok { Ok(()) } else { Err(message.to_owned()) }
        };
        require(
            self.version == CREDITS_VERSION,
            "incompatible credits version",
        )?;
        require(self.source_rom_sha256 == sha256, "credits ROM identity")?;
        require(self.art.len() == 3, "missing credits art")?;
        for (art, (at, kind, end, vram, tiles)) in self.art.iter().zip([
            (0x1DE278, "kosinski", 0x1DEA48, 0x2000, 87),
            (0x1DEA48, "kosinski", 0x1DF09C, 0x4000, 67),
            (0x2A3350, "nemesis", 0x2A3543, 0xF800, 29),
        ]) {
            require(
                art.rom_addr == at
                    && art.compression == kind
                    && art.vram_destination == vram
                    && art.tiles.len() == tiles * 32
                    && art.consumed > 0
                    && (art.consumed as u64)
                        .checked_add(u64::from(at))
                        .is_some_and(|n| n <= end),
                "credits art identity, extent or tile count",
            )?;
        }
        let font = &self.font_upload;
        require(
            font.rom_addr == 0x7A26C
                && font.destination_tile == 0x7C0
                && font.art_rom_addr == self.art[2].rom_addr
                && font.terminator == 0xFFFF,
            "credits font upload or terminator",
        )?;
        require(self.mappings.len() == 5, "missing credits mapping")?;
        for (mapping, (at, end, base, destinations)) in self.mappings.iter().zip([
            (0x1DF09C, 0x1DF17C, 0x100, &[0xFFFF1000][..]),
            (0x1DF17C, 0x1DF294, 0x100, &[0xFFFF1380][..]),
            (0x1DF294, 0x1DF3E0, 0x100, &[0xFFFF1700][..]),
            (0x1DF3E0, 0x1DF4C2, 0x100, &[0xFFFF1A80, 0xFFFF1E00][..]),
            (0x1DF4C2, 0x1DF52C, 0x2200, &[0xFFFF2000][..]),
        ]) {
            require(
                mapping.rom_addr == at
                    && mapping.base_tile == base
                    && mapping.ram_destinations == destinations
                    && (mapping.columns, mapping.rows) == (16, 28)
                    && mapping.words.len() == 448
                    && mapping.consumed > 0
                    && u64::from(at)
                        .checked_add(mapping.consumed as u64)
                        .is_some_and(|n| n <= end),
                "credits mapping identity, extent or dimensions",
            )?;
            let art = if base == 0x100 {
                &self.art[0]
            } else {
                &self.art[1]
            };
            require(
                mapping.words.iter().all(|word| {
                    let tile = word & 0x7FF;
                    let first = art.vram_destination / 32;
                    tile >= first && usize::from(tile - first) < art.tiles.len() / 32
                }),
                "credits mapping references unavailable tile",
            )?;
        }
        require(self.palettes.len() == 2, "missing initial credits palette")?;
        for (palette, (at, start, count)) in self
            .palettes
            .iter()
            .zip([(0x1DF53A, 0, 32), (0x1DF57A, 48, 16)])
        {
            require(
                palette.rom_addr == at
                    && palette.start_word == start
                    && palette.words.len() == count,
                "initial credits palette identity or size",
            )?;
        }
        require(
            self.fade.rom_addr == 0x7A6DE
                && self.fade.start_word == 50
                && self.fade.words.len() == 8 * 5,
            "credits fade table identity or size",
        )?;
        require(
            self.palettes
                .iter()
                .chain([&self.fade])
                .all(|p| p.words.iter().all(|word| word & !0xEEE == 0)),
            "invalid credits CRAM word",
        )?;
        require(self.commands.len() == 2, "missing credits command table")?;
        for (commands, (at, length)) in self.commands.iter().zip([(0x7A522, 38), (0x7A548, 22)]) {
            require(
                commands.rom_addr == at && commands.bytes.len() == length,
                "credits command table identity or size",
            )?;
            let pairs: Vec<_> = commands.bytes.chunks_exact(2).collect();
            require(
                pairs.last() == Some(&&[0xFF, 0][..])
                    && pairs[..pairs.len() - 1]
                        .iter()
                        .all(|p| p[0] < 64 && p[1] <= 12)
                    && pairs[..pairs.len() - 1]
                        .windows(2)
                        .all(|p| p[0][0] < p[1][0]),
                "invalid credits command or terminator",
            )?;
        }
        validate_text(&self.headers, 0x1FEE26, 37, 0x1FF066, true)?;
        validate_text(&self.names, 0x1FEB42, 128, 0x1FEE25, false)?;
        Ok(())
    }
}

fn validate_text(
    records: &[CreditsText],
    start: u32,
    count: usize,
    end: u32,
    header: bool,
) -> Result<(), String> {
    if records.len() != count {
        return Err("missing credits text record".into());
    }
    let mut next = start;
    for record in records {
        if record.rom_addr != next
            || record.bytes.last() != Some(&0xFF)
            || !record.bytes[..record.bytes.len() - 1].iter().all(|&b| {
                if header {
                    b == 0 || (0x80..=0x9B).contains(&b)
                } else {
                    b < 0xF0
                }
            })
        {
            return Err("credits text address, charset or terminator".into());
        }
        next = next
            .checked_add(record.bytes.len() as u32)
            .ok_or("credits text extent")?;
    }
    if next != end {
        return Err("credits text address coverage".into());
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests;
