//! Query stream-dependent work without building or modifying a pack.
//! `scene_load_work <rom> panel:10d art:1d712c ...` (hexadecimal inputs).

use psiv_pack::{
    PackError, Rom,
    json::{Json, dumps},
    presentation::load_work,
};
use std::path::Path;

fn run() -> Result<Json, PackError> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((path, queries)) = args.split_first().filter(|(_, q)| !q.is_empty()) else {
        return Err(PackError(
            "usage: scene_load_work <rom> panel:HEX|art:HEX ...".into(),
        ));
    };
    let rom = Rom::read(Path::new(path))?;
    let mut out = Json::object();
    for query in queries {
        let (kind, hex) = query
            .split_once(':')
            .ok_or_else(|| PackError(format!("invalid query {query}")))?;
        let value = usize::from_str_radix(hex, 16).map_err(|e| PackError(e.to_string()))?;
        let record = match kind {
            "panel" => load_work::panel(
                rom.bytes(),
                u16::try_from(value).map_err(|e| PackError(e.to_string()))?,
            )?,
            "art" => load_work::art(rom.bytes(), value)?,
            _ => return Err(PackError(format!("unknown query {query}"))),
        };
        out.insert(query, record);
    }
    Ok(out)
}

fn main() {
    match run() {
        Ok(value) => println!("{}", dumps(&value, true)),
        Err(error) => {
            eprintln!("scene_load_work: {error}");
            std::process::exit(2);
        }
    }
}
