//! `probe <rom>`: the ported extractors that no claimed section writes yet,
//! as one JSON document, `{"<name>": <record>, ...}`, sorted and indented as
//! the pack writes JSON. `tools/pack_parity.py` compares it byte for byte
//! with the same record from the Python extractors (#216); a port lists its
//! extractor here until a section claims what it writes.

use std::path::Path;

use psiv_pack::Rom;
use psiv_pack::decode::Decodes;
use psiv_pack::json::{Json, dumps};
use psiv_pack::maps::{ExtractedMaps, draw, layouts, overworld};
use psiv_pack::png::image_sha256;

/// Every real map, as the pack decodes and renders it. A render is held to
/// Python by its decoded image. For a world map, also its format facts, its
/// patches, and for each cell a write touches the patches that open it.
fn layouts(
    data: &[u8],
    decodes: &Decodes,
    maps: &ExtractedMaps,
) -> Result<Vec<Json>, psiv_pack::PackError> {
    let mut out = Vec::new();
    for record in maps.real() {
        let section = overworld::decode_map_section(data, decodes, record)?;
        let decoded = section.layout();
        let palette = layouts::chunk_palette(data, decoded.spec.palette)?;
        let image = draw::render_layout(
            &decoded.chunks,
            &decoded.bg,
            &decoded.fg,
            &decoded.patterns,
            &palette,
        )?;
        let (overlay, counts) = draw::priority_overlay(decoded, &palette)?;
        let mut entry = Json::object()
            .with("id", record.id)
            .with("layout", decoded.to_json())
            .with("anomalies", section.anomalies().to_vec())
            .with("unloaded_patterns", layouts::unloaded_patterns(decoded)?)
            .with("png_sha256", image_sha256(&image)?)
            .with(
                "png_over_sha256",
                overlay.as_deref().map(image_sha256).transpose()?,
            )
            .with("priority", counts.to_json());
        if let Some(world) = section.overworld() {
            let mut opening = Vec::new();
            for write in world.patches.iter().flat_map(|patch| &patch.writes) {
                let (x, y) = (write.chunk_x * 2, write.chunk_y * 2);
                for i in 0..4 {
                    let cell = (x + (i & 1), y + (i >> 1));
                    let routines: Vec<String> = overworld::opening_patches(world, &[cell])?
                        .iter()
                        .map(|patch| format!("0x{:06X}", patch.routine))
                        .collect();
                    opening.push(routines);
                }
            }
            entry.insert("overworld", world.to_json());
            entry.insert(
                "patches",
                world
                    .patches
                    .iter()
                    .map(overworld::LayoutPatch::to_json)
                    .collect::<Vec<_>>(),
            );
            entry.insert("opening", opening);
            entry.insert("code_sites", overworld::check_code_sites(data)?);
        }
        out.push(entry);
    }
    Ok(out)
}

/// `extract_map_effects`'s document, its per-map table as `[id, entries]`
/// rows: JSON object keys would sort as text.
fn map_effects(data: &[u8], maps: &ExtractedMaps) -> Result<Json, psiv_pack::PackError> {
    let effects = psiv_pack::maps::effects::extract_map_effects(data, maps.real())?;
    let per_map: Vec<Json> = effects
        .per_map
        .into_iter()
        .map(|(id, entries)| Json::Array(vec![Json::from(id), Json::from(entries)]))
        .collect();
    Ok(Json::object()
        .with("per_map", per_map)
        .with("replacements", effects.replacements)
        .with("dispatch_table", effects.dispatch_table)
        .with("census", effects.census))
}

/// `extract_map_updates`'s document, its per-map table as `[id, entries]` rows.
fn map_updates(data: &[u8], maps: &ExtractedMaps) -> Result<Json, psiv_pack::PackError> {
    let updates = psiv_pack::maps::updates::extract_map_updates(data, maps.real())?;
    let per_map: Vec<Json> = updates
        .per_map
        .into_iter()
        .map(|(id, entries)| Json::Array(vec![Json::from(id), Json::from(entries)]))
        .collect();
    Ok(Json::object()
        .with("per_map", per_map)
        .with("census", updates.census))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [rom] = args.as_slice() else {
        eprintln!("usage: probe <rom>");
        std::process::exit(2);
    };
    let rom = Rom::read(Path::new(rom)).unwrap_or_else(|e| {
        eprintln!("probe: error: {e}");
        std::process::exit(2)
    });
    let result = (|| -> Result<Json, psiv_pack::PackError> {
        // Every CRAM word the 12 low bits can hold, and one with the high
        // bits set, through decode_color.
        let cram: Vec<Json> = (0..0x1000u16)
            .chain([0xF000])
            .map(|word| psiv_pack::gfx::Color::decode(word).to_json())
            .collect();
        // Every XYRangeJmpTbl routine, plus one past it, at both clipped and
        // unclipped coordinates.
        let mut rects = Vec::new();
        for range in 0..16u16 {
            for (x, y) in [(0, 0), (3, 2), (40, 60), (200, 200)] {
                let rect = psiv_pack::warps::warp_rect(range, x, y, 64, 72);
                rects.push(match rect {
                    Ok(rect) => rect.map(|r| r.to_json()).into(),
                    Err(error) => Json::from(error.0),
                });
            }
        }
        let decodes = Decodes::new(rom.bytes());
        let maps = psiv_pack::maps::extract_maps(rom.bytes(), &decodes)?;
        Ok(Json::object()
            .with("cram", cram)
            .with("layouts", layouts(rom.bytes(), &decodes, &maps)?)
            .with("map_effects", map_effects(rom.bytes(), &maps)?)
            .with("map_updates", map_updates(rom.bytes(), &maps)?)
            .with("maps", maps.to_json(rom.bytes()))
            .with("warp_rects", rects)
            .with("names", psiv_pack::text::extract_names(rom.bytes())?))
    })();
    match result {
        Ok(probe) => println!("{}", dumps(&probe, true)),
        Err(error) => {
            eprintln!("probe: error: {error}");
            std::process::exit(1)
        }
    }
}
