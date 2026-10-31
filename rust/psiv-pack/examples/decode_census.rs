//! `decode_census <rom> <census.json>`: decode every call the census lists
//! and print, one JSON object per line in the census's order,
//! `{"consumed": N, "sha256": "..."}` or `{"error": "..."}`, then a last line
//! `{"seconds": S}` with the decode time. `tools/pack_parity.py` compares the
//! lines with the Python decoders' (#216, slice 1).

use std::path::Path;
use std::time::Instant;

use psiv_pack::Rom;
use psiv_pack::decode::{Call, Decodes};
use sha2::{Digest as _, Sha256};

fn call(entry: &serde_json::Value) -> Call {
    let offset = entry["offset"].as_u64().expect("an offset") as usize;
    match entry["decoder"].as_str().expect("a decoder") {
        "nemesis" => Call::Nemesis { offset },
        "kosinski" => Call::Kosinski { offset },
        "enigma" => Call::Enigma {
            offset,
            base_tile: entry["base_tile"].as_u64().expect("a base tile") as u16,
            max_words: entry["max_words"].as_u64().map(|n| n as usize),
        },
        other => panic!("unknown decoder {other}"),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [rom, census] = args.as_slice() else {
        eprintln!("usage: decode_census <rom> <census.json>");
        std::process::exit(2);
    };
    let rom = Rom::read(Path::new(rom)).unwrap_or_else(|e| {
        eprintln!("decode_census: error: {e}");
        std::process::exit(2)
    });
    let census: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(census).expect("the census")).expect("JSON");
    let calls: Vec<Call> = census["calls"]
        .as_array()
        .expect("calls")
        .iter()
        .map(call)
        .collect();
    let decodes = Decodes::new(rom.bytes());
    let start = Instant::now();
    let workers = psiv_pack::default_workers();
    let results: Vec<serde_json::Value> = std::thread::scope(|scope| {
        let chunk = calls.len().div_ceil(workers.max(1)).max(1);
        let handles: Vec<_> = calls
            .chunks(chunk)
            .map(|part| {
                let decodes = &decodes;
                scope.spawn(move || {
                    part.iter()
                        .map(|&c| match decodes.get(c) {
                            Ok(decoded) => serde_json::json!({
                                "consumed": decoded.consumed(),
                                "sha256": psiv_pack::png::hex(&Sha256::digest(decoded.bytes())),
                            }),
                            Err(error) => serde_json::json!({"error": error.0}),
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("a decode thread"))
            .collect()
    });
    let seconds = start.elapsed().as_secs_f64();
    for result in &results {
        println!("{result}");
    }
    println!("{}", serde_json::json!({ "seconds": seconds }));
}
