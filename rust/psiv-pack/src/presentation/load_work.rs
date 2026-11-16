//! Inputs to scene-load timing measurements, including panels whose pictures
//! are not yet emitted. Intrinsic Nemesis CPU work is not elapsed load time:
//! a panel also backs up tiles, decodes Enigma maps and draws its layers.

use super::{PANEL_RECORD_SIZE, be, hex6, panel_record_offset, slice};
use crate::decode::{enigma, nemesis};
use crate::json::Json;
use crate::{PackError, png};
use sha2::{Digest, Sha256};

/// The actual stream bytes and executed Nemesis paths, independent of scene.
pub fn art(rom: &[u8], source: usize) -> Result<Json, PackError> {
    let (bytes, consumed, work) = nemesis::decompress_with_work(rom, source)?;
    Ok(Json::object()
        .with("source", hex6(source))
        .with("consumed_bytes", consumed)
        .with(
            "compressed_sha256",
            png::hex(&Sha256::digest(&rom[source..source + consumed])),
        )
        .with("decoded_bytes", bytes.len())
        .with("decoded_sha256", png::hex(&Sha256::digest(&bytes)))
        .with("xor_mode", work.xor_mode)
        .with("lookup_tokens", work.lookup_tokens)
        .with("inline_tokens", work.inline_tokens)
        .with("cpu_cycles", work.cpu_cycles(nemesis::Destination::Vdp)))
}

/// Every input selected by retail `Panel_Create` / `Panel_Draw`, regardless
/// of whether the presentation pack can currently render the panel.
pub fn panel(rom: &[u8], id: u16) -> Result<Json, PackError> {
    let offset = panel_record_offset(id)?;
    let record = slice(rom, offset, PANEL_RECORD_SIZE, "panel record")?;
    let destination = be(record, 0, 2);
    let mut layers = Vec::new();
    for at in [10, 20] {
        let tile = be(record, at, 2) as u16;
        if tile == 0xFFFF {
            continue;
        }
        let source = be(record, at + 6, 4);
        let (words, consumed) = enigma::decompress(rom, source, tile, None)?;
        layers.push(
            Json::object()
                .with("source", hex6(source))
                .with("base_tile", tile)
                .with("consumed_bytes", consumed)
                .with(
                    "compressed_sha256",
                    png::hex(&Sha256::digest(&rom[source..source + consumed])),
                )
                .with("decoded_words", words.len())
                .with(
                    "decoded_sha256",
                    png::hex(&Sha256::digest(enigma::words_to_bytes(&words))),
                ),
        );
    }
    Ok(Json::object()
        .with("id", id)
        .with("record_offset", hex6(offset))
        .with("record_hex", png::hex(record))
        .with("destination_tile", destination)
        .with("row_size", record[6])
        .with("col_size", record[7])
        .with("x_cells", record[8])
        .with("y_cells", record[9])
        .with(
            "art",
            if destination == 0xFFFF {
                Json::Null
            } else {
                art(rom, be(record, 2, 4))?
            },
        )
        .with("mappings", layers))
}
