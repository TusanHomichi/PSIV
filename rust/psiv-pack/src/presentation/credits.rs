//! $079D0C's ROM-only credits inputs. No composed images or baked palettes:
//! the consumer needs raw 4bpp and complete pattern names for changing CRAM.
use super::slice;
use crate::decode::{enigma, kosinski, nemesis};
use crate::{PackError, Rom};
use psiv_data::{
    CreditsArt, CreditsCommands, CreditsFontUpload, CreditsMapping, CreditsPalette, CreditsText,
    EndingCreditsAssets,
};
use std::path::Path;

fn word(rom: &[u8], at: usize) -> Result<u16, PackError> {
    Ok(u16::from_be_bytes(
        slice(rom, at, 2, "credits word")?.try_into().unwrap(),
    ))
}
fn long(rom: &[u8], at: usize) -> Result<u32, PackError> {
    Ok(u32::from_be_bytes(
        slice(rom, at, 4, "credits pointer")?.try_into().unwrap(),
    ))
}

fn text(rom: &[u8], start: usize, count: usize, end: usize) -> Result<Vec<CreditsText>, PackError> {
    let mut at = start;
    let mut records = Vec::new();
    for _ in 0..count {
        let remaining = slice(rom, at, end.saturating_sub(at), "credits text")?;
        let len = remaining
            .iter()
            .position(|&b| b == 0xFF)
            .ok_or_else(|| PackError(format!("unterminated credits text at {at:#08x}")))?
            + 1;
        records.push(CreditsText {
            rom_addr: at as u32,
            bytes: remaining[..len].to_vec(),
        });
        at += len;
    }
    Ok(records)
}

pub(super) fn extract(rom: &Rom) -> Result<EndingCreditsAssets, PackError> {
    let data = rom.bytes();
    let mut art = Vec::new();
    for (at, vram) in [(0x1DE278, 0x2000), (0x1DEA48, 0x4000)] {
        let (tiles, consumed) = kosinski::decompress(data, at)?;
        art.push(CreditsArt {
            rom_addr: at as u32,
            compression: "kosinski".into(),
            consumed,
            vram_destination: vram,
            tiles,
        });
    }
    let font_upload = CreditsFontUpload {
        rom_addr: 0x7A26C,
        destination_tile: word(data, 0x7A26C)?,
        art_rom_addr: long(data, 0x7A26E)?,
        terminator: word(data, 0x7A272)?,
    };
    let (tiles, consumed) = nemesis::decompress(data, font_upload.art_rom_addr as usize)?;
    art.push(CreditsArt {
        rom_addr: font_upload.art_rom_addr,
        compression: "nemesis".into(),
        consumed,
        vram_destination: font_upload.destination_tile.wrapping_mul(32),
        tiles,
    });
    let mut mappings = Vec::new();
    // Every source is 16 columns by 28 rows; $0100 / $2200 in d0 are base
    // tile words. The support mappings are adjacent 0x380-byte RAM blocks.
    for (at, base, destinations) in [
        (0x1DF09C, 0x100, vec![0xFFFF1000]),
        (0x1DF17C, 0x100, vec![0xFFFF1380]),
        (0x1DF294, 0x100, vec![0xFFFF1700]),
        (0x1DF3E0, 0x100, vec![0xFFFF1A80, 0xFFFF1E00]),
        (0x1DF4C2, 0x2200, vec![0xFFFF2000]),
    ] {
        let (words, consumed) = enigma::decompress(data, at, base, Some(16 * 28))?;
        mappings.push(CreditsMapping {
            rom_addr: at as u32,
            consumed,
            base_tile: base,
            ram_destinations: destinations,
            columns: 16,
            rows: 28,
            words,
        });
    }
    let palette =
        |at: usize, start_word: usize, count: usize| -> Result<CreditsPalette, PackError> {
            let words = slice(data, at, count * 2, "credits palette")?
                .chunks_exact(2)
                .map(|b| u16::from_be_bytes([b[0], b[1]]))
                .collect();
            Ok(CreditsPalette {
                rom_addr: at as u32,
                start_word,
                words,
            })
        };
    let commands = [(0x7A522, 38), (0x7A548, 22)]
        .into_iter()
        .map(|(at, len)| {
            Ok(CreditsCommands {
                rom_addr: at as u32,
                bytes: slice(data, at, len, "credits commands")?.to_vec(),
            })
        })
        .collect::<Result<_, PackError>>()?;
    let assets = EndingCreditsAssets {
        version: psiv_data::CREDITS_VERSION,
        source_rom_sha256: crate::rom::EXPECTED_SHA256.into(),
        art,
        font_upload,
        mappings,
        palettes: vec![palette(0x1DF53A, 0, 32)?, palette(0x1DF57A, 48, 16)?],
        commands,
        // $380 + $80 ticks, two header/name blocks per $40 ticks; eight
        // render calls per tick. Stage 3 uses the final five header records.
        headers: text(data, 0x1FEE26, 37, 0x1FF066)?,
        names: text(data, 0x1FEB42, 128, 0x1FEE25)?,
        fade: palette(0x7A6DE, 50, 40)?,
    };
    assets
        .validate(crate::rom::EXPECTED_SHA256)
        .map_err(PackError)?;
    Ok(assets)
}

pub(super) fn build(rom: &Rom, root: &Path) -> Result<crate::json::Json, PackError> {
    let assets = extract(rom)?;
    let value = serde_json::to_value(&assets).map_err(|e| PackError(e.to_string()))?;
    let sha256 = crate::write_json(
        root,
        psiv_data::CREDITS_FILE,
        &crate::json::Json::from(value),
    )?;
    Ok(crate::json::Json::object()
        .with("file", psiv_data::CREDITS_FILE)
        .with("sha256", sha256)
        .with("version", psiv_data::CREDITS_VERSION))
}

#[cfg(test)]
mod tests;
