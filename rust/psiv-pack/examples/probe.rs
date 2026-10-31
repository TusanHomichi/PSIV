//! `probe <rom>`: the ported extractors that no claimed section writes yet,
//! as one JSON document, `{"<name>": <record>, ...}`, sorted and indented as
//! the pack writes JSON. `tools/pack_parity.py` compares it byte for byte
//! with the same record from the Python extractors (#216); a port lists its
//! extractor here until a section claims what it writes.

use std::path::Path;

use psiv_pack::Rom;
use psiv_pack::json::{Json, dumps};

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
        let decodes = psiv_pack::decode::Decodes::new(rom.bytes());
        Ok(Json::object()
            .with("cram", cram)
            .with(
                "maps",
                psiv_pack::maps::extract_maps(rom.bytes(), &decodes)?.to_json(rom.bytes()),
            )
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
