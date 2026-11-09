//! `battle/art/enemy_attacks.json` and the attack frames: the port of
//! `psiv_tools/battle_animation_art.py`. Each enemy whose attack composition
//! is exact gets its selected mapping records drawn on one stable canvas
//! around the shared fighter pivot, from its tile bank with the attack PLC
//! patterns laid over it. A body whose presentation only an oracle receipt
//! observed ships no frames (#214): those captures are shared across bodies,
//! so they are not this routine's art.

use std::path::Path;

use super::enemies::{bank, bounds, records};
use super::{enemy_palette, safe};
use crate::battle::animations::objects::plc_art;
use crate::json::Json;
use crate::{PackError, png};

const BASE_OBJECT_Y: i128 = 0x00D8;
const PNG_DIRECTORY: &str = "battle/art/enemy_attacks";
const RECEIPT_FRAMES_DEFERRED: &str = "oracle receipt frames are shared across routine bodies, \
so they are not this routine's art; deferred until its effect objects render from the ROM";

fn hex6(value: usize) -> String {
    format!("0x{value:06X}")
}

fn int(value: &Json, key: &str) -> Option<i128> {
    match value.get(key) {
        Some(Json::Int(n)) => Some(*n),
        _ => None,
    }
}

fn string<'a>(value: &'a Json, key: &str) -> Option<&'a str> {
    match value.get(key) {
        Some(Json::Str(s)) => Some(s),
        _ => None,
    }
}

fn list(value: &Json, key: &str) -> Vec<Json> {
    match value.get(key) {
        Some(Json::Array(items)) => items.clone(),
        _ => Vec::new(),
    }
}

fn field(value: &Json, key: &str) -> Json {
    value.get(key).cloned().unwrap_or(Json::Null)
}

/// `_attack_movement_pixels`: the motion contract the Godot layer reads.
fn movement_pixels(movement: &Json) -> Json {
    let runtime = match movement.get("runtime") {
        Some(runtime @ Json::Object(pairs)) if !pairs.is_empty() => runtime,
        _ => {
            return Json::object()
                .with("kind", "static_offset")
                .with("initial_offset_pixels", vec![0u8, 0])
                .with("step_pixels", vec![0u8, 0])
                .with("limit_offset_pixels", Json::Null)
                .with("branches", Vec::<Json>::new());
        }
    };
    let limit = int(runtime, "limit_hardware");
    let axis = string(runtime, "axis");
    let limit_offset = limit.map(|hardware| {
        let offset = hardware - BASE_OBJECT_Y;
        let on = |name: &str| Json::Int(if axis == Some(name) { offset } else { 0 });
        Json::Array(vec![on("x"), on("y")])
    });
    Json::object()
        .with("kind", field(runtime, "kind"))
        .with(
            "initial_offset_pixels",
            field(runtime, "initial_offset_pixels"),
        )
        .with("step_pixels", field(runtime, "step_pixels"))
        .with("limit_hardware", limit.map(Json::Int))
        .with("limit_offset_pixels", limit_offset)
        .with(
            "branches",
            runtime
                .get("branches")
                .cloned()
                .unwrap_or(Json::Array(Vec::new())),
        )
}

/// One sprite of a mapping record, as the renderer reads it.
struct Sprite {
    x: i128,
    y: i128,
    width: i128,
    height: i128,
    tile: i128,
    h_flip: bool,
    v_flip: bool,
}

fn sprites(record: &Json) -> Vec<Sprite> {
    list(record, "entries")
        .iter()
        .map(|entry| Sprite {
            x: int(entry, "x").unwrap_or(0),
            y: int(entry, "y").unwrap_or(0),
            width: int(entry, "width_tiles").unwrap_or(0),
            height: int(entry, "height_tiles").unwrap_or(0),
            tile: int(entry, "tile_index").unwrap_or(0),
            h_flip: matches!(entry.get("h_flip"), Some(Json::Bool(true))),
            v_flip: matches!(entry.get("v_flip"), Some(Json::Bool(true))),
        })
        .collect()
}

/// The canvas: its top-left relative to the body, and its size.
#[derive(Clone, Copy)]
struct Canvas {
    min_x: i128,
    min_y: i128,
    width: i128,
    height: i128,
}

/// `_attack_canvas`: one transparent canvas holding the body and every
/// frame's sprites around the pivot.
fn canvas(records: &[Json], body_width: i128, body_height: i128, pivot: (i128, i128)) -> Canvas {
    let boxes: Vec<(i128, i128, i128, i128)> = records
        .iter()
        .flat_map(sprites)
        .map(|s| (pivot.0 + s.x, pivot.1 + s.y, s.width * 8, s.height * 8))
        .collect();
    if boxes.is_empty() {
        return Canvas {
            min_x: 0,
            min_y: 0,
            width: body_width.max(8),
            height: body_height.max(8),
        };
    }
    let min_x = boxes.iter().map(|b| b.0).min().unwrap_or(0).min(0);
    let min_y = boxes.iter().map(|b| b.1).min().unwrap_or(0).min(0);
    let max_x = boxes
        .iter()
        .map(|b| b.0 + b.2)
        .max()
        .unwrap_or(0)
        .max(body_width);
    let max_y = boxes
        .iter()
        .map(|b| b.1 + b.3)
        .max()
        .unwrap_or(0)
        .max(body_height);
    Canvas {
        min_x,
        min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    }
}

/// `_render_attack_mapping`: one record's sprites (tiles row-major within a
/// sprite), or an empty canvas for a hidden frame.
fn render(
    tiles: &[[u8; 64]],
    record: &Json,
    canvas: Canvas,
    pivot: (i128, i128),
    palette: &[[u8; 3]],
) -> Result<Vec<u8>, PackError> {
    let mut pixels = vec![0u8; (canvas.width * canvas.height) as usize];
    if !matches!(record.get("hidden"), Some(Json::Bool(true))) {
        for s in sprites(record) {
            let (sx0, sy0) = (pivot.0 + s.x - canvas.min_x, pivot.1 + s.y - canvas.min_y);
            for y in 0..s.height * 8 {
                let source_y = if s.v_flip { s.height * 8 - 1 - y } else { y };
                for x in 0..s.width * 8 {
                    let source_x = if s.h_flip { s.width * 8 - 1 - x } else { x };
                    let index = s.tile + (source_y / 8) * s.width + source_x / 8;
                    let tile = usize::try_from(index)
                        .ok()
                        .and_then(|i| tiles.get(i))
                        .ok_or_else(|| {
                            PackError(format!("attack sprite pattern {index} is outside its bank"))
                        })?;
                    let pixel = tile[((source_y % 8) * 8 + source_x % 8) as usize];
                    if pixel != 0 {
                        pixels[((sy0 + y) * canvas.width + sx0 + x) as usize] = pixel;
                    }
                }
            }
        }
    }
    Ok(png::encode_indexed(
        canvas.width as u32,
        canvas.height as u32,
        &pixels,
        palette,
        &[0],
    )?)
}

/// `emit_enemy_attack_art`: write the frames and return the index, from the
/// enemy-animation file this pack writes beside it.
pub fn write(rom: &[u8], root: &Path, animations: &Json) -> Result<Json, PackError> {
    let records = records(rom)?;
    let bounds = bounds(&records, rom.len());
    let mut entries = Vec::new();
    let mut frame_total = 0;
    for animation in list(animations, "animations") {
        let id = int(&animation, "enemy_id").unwrap_or(0) as usize;
        let composition = field(&animation, "composition");
        let movement = field(&animation, "movement");
        let mut entry = Json::object()
            .with("id", id)
            .with("symbol", field(&animation, "symbol"))
            .with("status", field(&composition, "status"))
            .with("reason", field(&composition, "reason"))
            .with(
                "movement",
                Json::object()
                    .with("status", field(&movement, "status"))
                    .with("reason", field(&movement, "reason"))
                    .with("runtime", movement_pixels(&movement))
                    .with("writes", field(&movement, "writes")),
            )
            .with("frames", Vec::<Json>::new())
            .with(
                "source",
                Json::object()
                    .with("enemy_art_record", hex6(0x27F3AE + id * 20))
                    .with("mapping_consumer", "Battle_FillSpriteAttributes")
                    .with("grand_cross", 0u8),
            );
        if string(&composition, "status") != Some("exact") {
            entries.push(entry);
            continue;
        }
        let sequence = field(&animation, "frame_sequence");
        if let Some(receipt @ Json::Object(_)) = animation.get("oracle_receipt")
            && string(&sequence, "timing_model") == Some("observed")
        {
            let mut source = field(&entry, "source");
            source.insert(
                "oracle_receipt",
                Json::object()
                    .with("routine_offset", field(receipt, "routine_offset"))
                    .with(
                        "representative_enemy_id",
                        field(receipt, "representative_enemy_id"),
                    )
                    .with("presentation", field(receipt, "presentation"))
                    .with("action_frame", field(receipt, "action_frame"))
                    .with("receipt", field(receipt, "receipt")),
            );
            entry.insert("source", source);
            entry.insert("status", "deferred");
            entry.insert("reason", RECEIPT_FRAMES_DEFERRED);
            entries.push(entry);
            continue;
        }
        let record = &records[id];
        let (mut tiles, sources) = bank(rom, record, &bounds)?;
        let plc = field(&animation, "attack_plc");
        let ranges = list(&plc, "ranges");
        let plc_tiles = plc_art(rom, &list(&plc, "loads")).tiles();
        if !plc_tiles.is_empty() {
            if tiles.len() < plc_tiles.len() {
                tiles.resize(plc_tiles.len(), [0; 64]);
            }
            for range in &ranges {
                let (first, last) = (
                    int(range, "first_pattern").unwrap_or(0),
                    int(range, "last_pattern_exclusive").unwrap_or(0),
                );
                for pattern in first..last {
                    if let Ok(i) = usize::try_from(pattern)
                        && i < plc_tiles.len()
                    {
                        tiles[i] = plc_tiles[i];
                    }
                }
            }
        }
        let (words, _, columns) = super::enemies::mapping(rom, record)?;
        let body_width = columns as i128 * 8;
        let rows = (words.len() / columns) as i128;
        let body_top = (15 - rows) * 8;
        let pivot = (body_width, BASE_OBJECT_Y - 0x80 - body_top);
        let mapping_records = list(&sequence, "mapping_records");
        let canvas = canvas(&mapping_records, body_width, rows * 8, pivot);
        let palette = enemy_palette(rom, id)?;
        let durations = list(&sequence, "frame_durations");
        let symbol = string(&animation, "symbol").unwrap_or_default().to_owned();
        let mut frames = Vec::new();
        for (index, mapping_record) in mapping_records.iter().enumerate() {
            let image = render(&tiles, mapping_record, canvas, pivot, &palette)?;
            let path = format!(
                "{PNG_DIRECTORY}/{id:03}_{}_frame{index:02}.png",
                safe(&symbol)
            );
            crate::write_file(root, &path, &image)?;
            frame_total += 1;
            frames.push(
                Json::object()
                    .with("index", index)
                    .with("mapping_rom_offset", field(mapping_record, "rom_offset"))
                    .with("png", path)
                    .with("png_sha256", png::image_sha256(&image)?)
                    .with(
                        "duration",
                        durations.get(index).cloned().unwrap_or(Json::Null),
                    ),
            );
        }
        let mut art: Vec<Json> = sources
            .iter()
            .map(|s| {
                Json::object()
                    .with("field", format!("+0x{:02X}", 2 + 4 * s.field))
                    .with("rom_offset", hex6(s.offset))
                    .with("first_pattern", s.first_pattern)
                    .with("pattern_count", s.tile_count)
            })
            .collect();
        art.extend(ranges.iter().map(|range| {
            Json::object()
                .with("field", "attack_plc")
                .with("plc_id", field(range, "plc_id"))
                .with("call_rom_offset", field(range, "call_rom_offset"))
                .with("record_rom_offset", field(range, "record_rom_offset"))
                .with("stream_rom_offset", field(range, "stream_rom_offset"))
                .with("first_pattern", field(range, "first_pattern"))
                .with("pattern_count", field(range, "pattern_count"))
        }));
        entry.insert("frames", frames);
        entry.insert(
            "origin_pixels",
            vec![Json::Int(canvas.min_x), Json::Int(canvas.min_y)],
        );
        entry.insert("width_pixels", Json::Int(canvas.width));
        entry.insert("height_pixels", Json::Int(canvas.height));
        entry.insert("frame_duration", field(&sequence, "frame_duration"));
        entry.insert("total_frames", field(&sequence, "total_frames"));
        entry.insert("art", art);
        entries.push(entry);
    }
    let count = |status: &str| {
        entries
            .iter()
            .filter(|e| string(e, "status") == Some(status))
            .count()
    };
    let census = Json::object()
        .with("enemies", entries.len())
        .with("exact", count("exact"))
        .with("partial", count("partial"))
        .with("deferred", count("deferred"))
        .with("frames", frame_total);
    let rom_sha256 = animations
        .get("source")
        .map(|s| field(s, "rom_sha256"))
        .unwrap_or(Json::Null);
    Ok(Json::object()
        .with("format_version", crate::battle::FORMAT_VERSION)
        .with("kind", "battle_enemy_attack_art")
        .with("count", entries.len())
        .with(
            "source",
            Json::object()
                .with("enemy_attack_table", "0x00D0A4")
                .with("enemy_art_table", "0x27F3AE")
                .with("mapping_consumer", "Battle_FillSpriteAttributes")
                .with("mapping_header_bytes", 1u8)
                .with("mapping_entry_bytes", 6u8)
                .with("art_bank_order", vec![3u8, 1, 2])
                .with("rom_sha256", rom_sha256)
                .with("grand_cross", 0u8),
        )
        .with("census", census)
        .with("enemies", entries))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(x: i128, y: i128, w: u8, h: u8) -> Json {
        Json::object().with(
            "entries",
            vec![
                Json::object()
                    .with("x", Json::Int(x))
                    .with("y", Json::Int(y))
                    .with("width_tiles", w)
                    .with("height_tiles", h)
                    .with("tile_index", 0u8)
                    .with("h_flip", false)
                    .with("v_flip", false),
            ],
        )
    }

    #[test]
    fn the_canvas_holds_the_body_and_every_sprite() {
        let c = canvas(&[record(-40, -8, 1, 1)], 32, 24, (32, 10));
        assert_eq!((c.min_x, c.min_y, c.width, c.height), (-8, 0, 40, 24));
        let empty = canvas(&[], 0, 0, (0, 0));
        assert_eq!((empty.width, empty.height), (8, 8));
    }

    #[test]
    fn a_sprite_reads_its_tiles_row_major_and_a_hidden_frame_is_empty() {
        let mut tiles = [[0u8; 64]; 2];
        tiles[0][0] = 1;
        tiles[1][0] = 2;
        let c = Canvas {
            min_x: 0,
            min_y: 0,
            width: 16,
            height: 8,
        };
        let image = render(&tiles, &record(0, 0, 2, 1), c, (0, 0), &[[0, 0, 0]; 16]).unwrap();
        let decoded = png::decode(&image).unwrap();
        assert_eq!((decoded.pixels[0], decoded.pixels[8]), (1, 2));
        let hidden = record(0, 0, 2, 1).with("hidden", true);
        let image = render(&tiles, &hidden, c, (0, 0), &[[0, 0, 0]; 16]).unwrap();
        assert!(png::decode(&image).unwrap().pixels.iter().all(|&p| p == 0));
    }

    #[test]
    fn a_linear_runtime_names_its_limit_on_its_axis() {
        let movement = Json::object().with(
            "runtime",
            Json::object()
                .with("kind", "linear")
                .with("axis", "x")
                .with("limit_hardware", 0xE0u16)
                .with("initial_offset_pixels", vec![8u8, 0])
                .with("step_pixels", vec![1u8, 0]),
        );
        let out = serde_json::Value::from(&movement_pixels(&movement));
        assert_eq!(out["limit_offset_pixels"], serde_json::json!([8, 0]));
        assert_eq!(out["branches"], serde_json::json!([]));
        let none = serde_json::Value::from(&movement_pixels(&Json::object()));
        assert!(none["limit_offset_pixels"].is_null());
        assert!(none.get("limit_hardware").is_none());
    }
}
