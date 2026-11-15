//! Cartridge scene-load observations; Python owns only capture orchestration.
#![forbid(unsafe_code)]

mod analysis;
mod identity;

use psiv_pack::{Rom, json::dumps, presentation::load_work};
use serde::Serialize;
use serde_json::Value;
use std::io::Write;
use std::{collections::BTreeMap, path::Path};

fn write(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    output
        .write_all((text + "\n").as_bytes())
        .map_err(|e| e.to_string())
}

fn run(args: &[String]) -> Result<(), String> {
    let [mode, rom_path, rest @ ..] = args else {
        return Err("usage: psiv-scene-loads verify ROM | analyze ROM OUT SCENE ...".into());
    };
    let rom = Rom::read(Path::new(rom_path)).map_err(|e| e.to_string())?;
    identity::verify(rom.bytes())?;
    if mode == "verify" && rest.is_empty() {
        println!("{}", analysis::config());
        return Ok(());
    }
    let [out, scenes @ ..] = rest else {
        return Err("analyze needs OUT and SCENE ...".into());
    };
    if mode != "analyze" || scenes.is_empty() {
        return Err("analyze needs OUT and SCENE ...".into());
    }
    let out = Path::new(out);
    let mut summary = BTreeMap::new();
    let mut work = BTreeMap::<String, Value>::new();
    for scene in scenes {
        let scene: u16 = scene.parse::<u16>().map_err(|e| e.to_string())?;
        if summary.contains_key(&scene) {
            return Err(format!("duplicate fixture {scene}"));
        }
        let dir = out.join(format!("tape-{scene}"));
        let path = dir.join("analysis-input.json");
        let capture: analysis::Capture = serde_json::from_slice(
            &std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?,
        )
        .map_err(|e| e.to_string())?;
        if capture.scene != scene {
            return Err("capture fixture disagrees with requested scene".into());
        }
        let mut calls = analysis::analyze(capture)?;
        for call in &mut calls {
            let query = call.query.clone().ok_or("load has no query")?;
            if !work.contains_key(&query) {
                let (kind, value) = query.split_once(':').ok_or("invalid query")?;
                let value = usize::from_str_radix(value, 16).map_err(|e| e.to_string())?;
                let data = match kind {
                    "panel" => load_work::panel(
                        rom.bytes(),
                        u16::try_from(value).map_err(|e| e.to_string())?,
                    ),
                    "art" => load_work::art(rom.bytes(), value),
                    _ => return Err("unknown query".into()),
                }
                .map_err(|e| e.to_string())?;
                work.insert(
                    query.clone(),
                    serde_json::from_str(&dumps(&data, true)).map_err(|e| e.to_string())?,
                );
            }
            analysis::attach_work(call, work[&query].clone())?;
            println!(
                "tape {scene}: {query}: {} held frames; {} master cycles; {} further held samples after return",
                call.lag_frames,
                call.master_cycles,
                call.following_held_frames.len()
            );
        }
        write(&dir.join("loads.json"), &calls)?;
        summary.insert(scene, calls);
    }
    write(&out.join("work.json"), &work)?;
    write(&out.join("summary.json"), &summary)
}

fn main() {
    if let Err(error) = run(&std::env::args().skip(1).collect::<Vec<_>>()) {
        eprintln!("psiv-scene-loads: {error}");
        std::process::exit(2);
    }
}
