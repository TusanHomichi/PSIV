//! The shopkeeper portraits: the port of `psiv_tools/shop_portraits.py`.
//! `Win_ShopMeseta` draws one of the art blobs the two portrait groups point
//! at through the shared 6x6 Plane A mapping, on CRAM line 2.

use std::path::Path;

use psiv_data::ShopPortrait;

use crate::decode::nemesis;
use crate::json::Json;
use crate::sprites::field::pal_init_line_3;
use crate::{PackError, gfx, png};

/// The two pointer groups (offset, count), then the shared 6x6 mapping.
const GROUPS: [(usize, usize); 2] = [(0x68006, 18), (0x6804E, 49)];
const MAPPING: usize = 0x2A2B36;
/// The mapping's 36 words: consecutive tiles from this one.
const FIRST_TILE_WORD: u16 = 0xC55C;
const TILES: usize = 36;
const COLUMNS: usize = 6;

/// One portrait: the typed record and what the game doesn't read.
pub struct Portrait {
    pub record: ShopPortrait,
    pub research: Json,
}

/// Decode and write every distinct portrait, in art-address order.
pub fn write(rom: &[u8], root: &Path) -> Result<Vec<Portrait>, PackError> {
    let mut pointers = Vec::new();
    for (base, count) in GROUPS {
        for at in (base..base + count * 4).step_by(4) {
            pointers.push(crate::m68k::l(rom, at)? as usize);
        }
    }
    pointers.sort_unstable();
    pointers.dedup();
    for (n, at) in (MAPPING..MAPPING + TILES * 2).step_by(2).enumerate() {
        if crate::m68k::w(rom, at)? != FIRST_TILE_WORD + n as u16 {
            return Err(PackError(
                "shop portrait mapping differs from the retail 6x6 tile grid".into(),
            ));
        }
    }
    let palette = pal_init_line_3(rom)?;
    pointers
        .into_iter()
        .map(|address| {
            let (raw, consumed) = nemesis::decompress(rom, address)?;
            let tiles = gfx::decode_tiles(&raw)?;
            if tiles.len() != TILES {
                return Err(PackError(format!(
                    "shop portrait {address:#x} has {} tiles",
                    tiles.len()
                )));
            }
            let (width, height, pixels) = gfx::compose_sheet(&tiles, COLUMNS)?;
            let image = png::encode_indexed(width as u32, height as u32, &pixels, &palette, &[0])?;
            let relative = format!("shops/portraits/{address:06x}.png");
            crate::write_file(root, &relative, &image)?;
            let research = Json::object()
                .with("png_sha256", png::image_sha256(&image)?)
                .with("source_consumed", consumed)
                .with("mapping", format!("0x{MAPPING:06X}"))
                .with("size_pixels", vec![width, height])
                .with("cram_line", 2u8);
            Ok(Portrait {
                record: ShopPortrait {
                    art: format!("0x{address:06X}"),
                    png: relative,
                },
                research,
            })
        })
        .collect()
}
