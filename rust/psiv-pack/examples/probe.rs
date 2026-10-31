//! `probe <rom>`: the ported extractors that no claimed section writes yet,
//! as one JSON document, `{"<name>": <record>, ...}`, sorted and indented as
//! the pack writes JSON. `tools/pack_parity.py` compares it byte for byte
//! with the same record from the Python extractors (#216); a port lists its
//! extractor here until a section claims what it writes.

use std::path::Path;

use psiv_pack::Rom;
use psiv_pack::decode::Decodes;
use psiv_pack::json::{Json, dumps};
use psiv_pack::maps::{ExtractedMaps, draw, layouts};
use psiv_pack::png::image_sha256;

/// Every map with a layout section, as the pack decodes and renders it. A
/// render is held to Python by its decoded image.
fn layouts(
    data: &[u8],
    decodes: &Decodes,
    maps: &ExtractedMaps,
) -> Result<Vec<Json>, psiv_pack::PackError> {
    let mut out = Vec::new();
    for record in maps.real().filter(|record| record.layout.planes.is_some()) {
        let (decoded, anomalies) =
            layouts::decode_layout_section(data, decodes, layouts::layout_spec(record)?)?;
        let palette = layouts::chunk_palette(data, decoded.spec.palette)?;
        let image = draw::render_layout(
            &decoded.chunks,
            &decoded.bg,
            &decoded.fg,
            &decoded.patterns,
            &palette,
        )?;
        let (overlay, counts) = draw::priority_overlay(&decoded, &palette)?;
        out.push(
            Json::object()
                .with("id", record.id)
                .with("layout", decoded.to_json())
                .with("anomalies", anomalies)
                .with("unloaded_patterns", layouts::unloaded_patterns(&decoded)?)
                .with("png_sha256", image_sha256(&image)?)
                .with(
                    "png_over_sha256",
                    overlay.as_deref().map(image_sha256).transpose()?,
                )
                .with("priority", counts.to_json()),
        );
    }
    Ok(out)
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
