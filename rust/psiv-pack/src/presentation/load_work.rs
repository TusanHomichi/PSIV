//! Inputs to scene-load timing measurements, including panels whose pictures
//! are not yet emitted. Intrinsic Nemesis CPU work is not elapsed load time:
//! a panel also backs up tiles, decodes Enigma maps and draws its layers.

use super::{PANEL_RECORD_SIZE, be, hex6, panel_record_offset, slice};
use crate::PackError;
use crate::decode::{enigma, nemesis};
use crate::json::Json;
use sha2::{Digest, Sha256};

/// The actual stream bytes and executed Nemesis paths, independent of scene.
pub fn art(rom: &[u8], source: usize) -> Result<Json, PackError> {
    let (bytes, consumed, work) = nemesis::decompress_with_work(rom, source)?;
    Ok(Json::object()
        .with("source", hex6(source))
        .with("consumed_bytes", consumed)
        .with(
            "compressed_sha256",
            format!("{:x}", Sha256::digest(&rom[source..source + consumed])),
        )
        .with("decoded_bytes", bytes.len())
        .with("decoded_sha256", format!("{:x}", Sha256::digest(&bytes)))
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
                    format!("{:x}", Sha256::digest(&rom[source..source + consumed])),
                )
                .with("decoded_words", words.len())
                .with(
                    "decoded_sha256",
                    format!("{:x}", Sha256::digest(enigma::words_to_bytes(&words))),
                ),
        );
    }
    Ok(Json::object()
        .with("id", id)
        .with("record_offset", hex6(offset))
        .with(
            "record_hex",
            record
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>(),
        )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timing_queries_do_not_require_a_renderable_panel() {
        let mut rom = vec![0; 0x07B000 + PANEL_RECORD_SIZE];
        let offset = 0x07B000;
        // No art upload and no layers: retail's $FFFF sentinels skip them.
        for at in [0, 10, 20] {
            rom[offset + at..offset + at + 2].copy_from_slice(&0xFFFFu16.to_be_bytes());
        }
        let panel = panel(&rom, 0).unwrap();
        assert_eq!(panel.at(&["art"]).unwrap(), &Json::Null);
        assert!(panel.at(&["mappings"]).unwrap().items().unwrap().is_empty());
        assert!(super::panel(&rom, 0xC0).is_err());
    }

    #[test]
    fn art_reports_the_existing_decoders_work_and_identity() {
        let stream = [0, 1, 0x85, 0x71, 0, 0xFF, 0, 0, 0];
        let report = art(&stream, 0).unwrap();
        assert_eq!(report.at(&["decoded_bytes"]).unwrap().int().unwrap(), 32);
        assert_eq!(report.at(&["cpu_cycles"]).unwrap().int().unwrap(), 8740);
        assert!(art(&stream[..7], 0).is_err());
    }

    #[test]
    fn equal_output_sizes_do_not_imply_equal_work() {
        let short = art(&[0, 1, 0x85, 0x71, 0, 0xFF, 0, 0, 0], 0).unwrap();
        let mut long = vec![0, 1, 0x85, 0x78, 0, 0xFF];
        long.extend([0; 10]);
        let long = art(&long, 0).unwrap();
        assert_eq!(
            short.at(&["decoded_sha256"]).unwrap(),
            long.at(&["decoded_sha256"]).unwrap()
        );
        assert_ne!(
            short.at(&["cpu_cycles"]).unwrap(),
            long.at(&["cpu_cycles"]).unwrap()
        );
    }
}
