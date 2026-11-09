//! The port of `psiv_tools/battle_fusion_art.py`: the Zol slugs' Fusion
//! sprites, `BattleObj_Fusion`'s five mapping records drawn in the casting
//! slug's palette. That module cites the routines it checks.

use std::path::Path;

use super::enemy_palette;
use crate::battle::animations::mapping::{decode_record, entries_of};
use crate::decode::nemesis;
use crate::json::Json;
use crate::{PackError, png};

/// `BattleObj_Fusion`, its first-run test and `jsr loc_12618`.
const ROUTINE: usize = 0x01A090;
const SIGNATURE: [u8; 6] = [0x08, 0xD4, 0x00, 0x07, 0x66, 0x00];
const SIGNATURE_TAIL: [u8; 6] = [0x4E, 0xB9, 0x00, 0x01, 0x26, 0x18];
/// `BattleObj_Fusion2`, where the first object's code ends.
const ROUTINE_BOUND: usize = 0x01A300;
/// `loc_10796`'s `lea (art).l, a0` / `jsr (NemDecomp).l`.
const ART_LEA: usize = 0x0107A0;
const MAPPING_FIELD: u16 = 0x000C;
/// The Zol slug, whose turn this is and whose palette the pieces use.
const ZOL_SLUG: usize = 34;
const PNG_DIRECTORY: &str = "battle/art/fusion";

fn refuse<T>(message: String) -> Result<T, PackError> {
    Err(PackError(message))
}

fn hex6(value: usize) -> String {
    format!("0x{value:06X}")
}

fn word(rom: &[u8], at: usize) -> Result<u16, PackError> {
    Ok(crate::m68k::w(rom, at)?)
}

/// `fusion_mappings`: the distinct records `move.l #record,$C(a4)` selects.
fn mappings(rom: &[u8]) -> Result<Vec<usize>, PackError> {
    let tail = ROUTINE + SIGNATURE.len() + 2;
    if rom.get(ROUTINE..ROUTINE + SIGNATURE.len()) != Some(&SIGNATURE[..])
        || rom.get(tail..tail + SIGNATURE_TAIL.len()) != Some(&SIGNATURE_TAIL[..])
    {
        return refuse(format!(
            "BattleObj_Fusion at {} does not start with its first-run test and loc_12618 call",
            hex6(ROUTINE)
        ));
    }
    let mut found = Vec::new();
    for at in (ROUTINE..ROUTINE_BOUND - 7).step_by(2) {
        if word(rom, at)? != 0x297C || word(rom, at + 6)? != MAPPING_FIELD {
            continue;
        }
        let pointer = crate::m68k::l(rom, at + 2)? as usize;
        if !found.contains(&pointer) {
            found.push(pointer);
        }
    }
    if found.len() != 5 {
        return refuse(format!(
            "BattleObj_Fusion selects {} mapping records, not 5",
            found.len()
        ));
    }
    Ok(found)
}

/// `fusion_art_offset`: the Nemesis stream `loc_10796` loads.
fn art_offset(rom: &[u8]) -> Result<usize, PackError> {
    if word(rom, ART_LEA)? != 0x41F9 || word(rom, ART_LEA + 6)? != 0x4EB9 {
        return refuse(format!(
            "loc_10796 at {} is not `lea (art).l, a0` / `jsr (NemDecomp).l`",
            hex6(ART_LEA)
        ));
    }
    Ok(crate::m68k::l(rom, ART_LEA + 2)? as usize)
}

/// `_tile_pixels`: whole tiles only, two pixels per byte.
fn tile_pixels(data: &[u8]) -> Vec<[u8; 64]> {
    data.as_chunks::<32>()
        .0
        .iter()
        .map(|tile| {
            let mut out = [0u8; 64];
            for (i, byte) in tile.iter().enumerate() {
                out[i * 2] = byte >> 4;
                out[i * 2 + 1] = byte & 0x0F;
            }
            out
        })
        .collect()
}

/// `vdp_size`: (columns, rows), horizontal count in bits 3-2.
fn vdp_size(size: i128) -> (i128, i128) {
    (((size >> 2) & 3) + 1, (size & 3) + 1)
}

/// One sprite of a record, as `render_record` reads it.
struct Sprite {
    x: i128,
    y: i128,
    columns: i128,
    rows: i128,
    tile: i128,
    h_flip: bool,
    v_flip: bool,
    valid: bool,
    rom_offset: String,
}

fn sprite(entry: &Json) -> Sprite {
    let int = |key: &str| match entry.get(key) {
        Some(Json::Int(n)) => *n,
        _ => 0,
    };
    let flag = |key: &str| matches!(entry.get(key), Some(Json::Bool(true)));
    let (columns, rows) = vdp_size(int("size"));
    Sprite {
        x: int("x"),
        y: int("y"),
        columns,
        rows,
        tile: int("tile_index"),
        h_flip: flag("h_flip"),
        v_flip: flag("v_flip"),
        valid: flag("attributes_valid"),
        rom_offset: match entry.get("rom_offset") {
            Some(Json::Str(s)) => s.clone(),
            _ => String::new(),
        },
    }
}

/// A rendered record: its origin, its size and its indexed pixels.
type Rendered = ([i128; 2], [i128; 2], Vec<u8>);

/// `render_record`: a record as an indexed image (tiles run down each
/// column first), its origin and its size.
fn render(tiles: &[[u8; 64]], sprites: &[Sprite]) -> Result<Rendered, PackError> {
    let left = sprites.iter().map(|s| s.x).min().unwrap_or(0);
    let top = sprites.iter().map(|s| s.y).min().unwrap_or(0);
    let right = sprites
        .iter()
        .map(|s| s.x + s.columns * 8)
        .max()
        .unwrap_or(0);
    let bottom = sprites.iter().map(|s| s.y + s.rows * 8).max().unwrap_or(0);
    let (width, height) = (right - left, bottom - top);
    let mut pixels = vec![0u8; (width * height) as usize];
    for s in sprites {
        if !s.valid || s.tile + s.columns * s.rows > tiles.len() as i128 {
            return refuse(format!("mapping entry {} is outside the art", s.rom_offset));
        }
        for y in 0..s.rows * 8 {
            let sy = if s.v_flip { s.rows * 8 - 1 - y } else { y };
            for x in 0..s.columns * 8 {
                let sx = if s.h_flip { s.columns * 8 - 1 - x } else { x };
                let tile = &tiles[(s.tile + (sx / 8) * s.rows + sy / 8) as usize];
                let pixel = tile[((sy % 8) * 8 + sx % 8) as usize];
                if pixel != 0 {
                    pixels[((s.y - top + y) * width + s.x - left + x) as usize] = pixel;
                }
            }
        }
    }
    Ok(([left, top], [width, height], pixels))
}

/// `emit_fusion_art`: write the frames and return the file.
pub fn write(rom: &[u8], root: &Path) -> Result<Json, PackError> {
    let art = art_offset(rom)?;
    let (data, _) = nemesis::decompress(rom, art)?;
    let tiles = tile_pixels(&data);
    let palette = enemy_palette(rom, ZOL_SLUG)?;
    let mut frames = Vec::new();
    for (index, mapping) in mappings(rom)?.into_iter().enumerate() {
        let record = decode_record(rom, mapping).map_err(PackError)?;
        let sprites: Vec<Sprite> = entries_of(&record).iter().map(sprite).collect();
        let (origin, size, pixels) = render(&tiles, &sprites)?;
        let name = format!("{PNG_DIRECTORY}/{mapping:06X}.png");
        let image = png::encode_indexed(size[0] as u32, size[1] as u32, &pixels, &palette, &[0])?;
        crate::write_file(root, &name, &image)?;
        frames.push(
            Json::object()
                .with("index", index)
                .with("mapping", hex6(mapping))
                .with(
                    "sprites",
                    record.get("sprite_count").cloned().unwrap_or(Json::Null),
                )
                .with("png", name)
                .with(
                    "origin",
                    origin.iter().map(|&v| Json::Int(v)).collect::<Vec<_>>(),
                )
                .with(
                    "size",
                    size.iter().map(|&v| Json::Int(v)).collect::<Vec<_>>(),
                ),
        );
    }
    Ok(Json::object()
        .with("kind", "battle_fusion_art")
        .with(
            "source",
            Json::object()
                .with("routine", "BattleObj_Fusion")
                .with("rom_offset", hex6(ROUTINE))
                .with("art", "loc_10796")
                .with("art_rom_offset", hex6(art))
                .with("tiles", tiles.len()),
        )
        .with("palette_enemy_id", ZOL_SLUG)
        .with("count", frames.len())
        .with("frames", frames))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tile(value: u8) -> [u8; 64] {
        let mut t = [0u8; 64];
        t[0] = value;
        t
    }

    fn at(x: i128, y: i128, size: i128, index: i128, h_flip: bool) -> Sprite {
        let (columns, rows) = vdp_size(size);
        Sprite {
            x,
            y,
            columns,
            rows,
            tile: index,
            h_flip,
            v_flip: false,
            valid: true,
            rom_offset: "0x000000".into(),
        }
    }

    #[test]
    fn a_sprite_runs_down_each_column_first() {
        // Size 0b0101: two columns, two rows; tiles 0,1 fill the first column.
        let tiles = [tile(1), tile(2), tile(3), tile(4)];
        let (origin, size, pixels) = render(&tiles, &[at(-8, -16, 0b0101, 0, false)]).unwrap();
        assert_eq!((origin, size), ([-8, -16], [16, 16]));
        assert_eq!((pixels[0], pixels[8 * 16], pixels[8]), (1, 2, 3));
    }

    #[test]
    fn a_flipped_sprite_reads_its_tiles_mirrored_and_one_past_the_art_refuses() {
        let tiles = [tile(1)];
        let (_, _, pixels) = render(&tiles, &[at(0, 0, 0, 0, true)]).unwrap();
        assert_eq!((pixels[0], pixels[7]), (0, 1));
        assert!(render(&tiles, &[at(0, 0, 0, 1, false)]).is_err());
    }
}
