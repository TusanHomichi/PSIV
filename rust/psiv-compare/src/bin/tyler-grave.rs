//! Capture and check Tyler's grave through the current oracle host.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use psiv_compare::tyler_grave::measure;
use serde_json::json;
use sha2::{Digest, Sha256};

const USAGE: &str = "tyler-grave [--output DIR] [--log CSV]";
const PATCHES: [&str; 10] = [
    "7000:FFFFEC28:0120",
    "7000:FFFFEC2A:0000",
    "7000:FFFFEC4E:02",
    "7000:FFFFEF00:0008",
    "7000:FFFFF406:0130",
    "7000:FFFFF408:01B0",
    "7000:FFFFF40A:00010203",
    "7200:FFFFF110:08",
    "7200:FFFFECA8:0044",
    "7200:FFFFEC20:000C",
];

fn hash(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    Ok(psiv_pack::png::hex(&Sha256::digest(std::fs::read(path)?)))
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let mut output = root.join("build/tyler-grave");
    let mut retained = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output" => output = PathBuf::from(args.next().ok_or("--output needs a directory")?),
            "--log" => retained = Some(PathBuf::from(args.next().ok_or("--log needs a CSV path")?)),
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(());
            }
            _ => return Err(format!("unknown option {arg}; {USAGE}").into()),
        }
    }
    let output = std::path::absolute(output)?;
    std::fs::create_dir_all(&output)?;
    let log = std::path::absolute(
        retained
            .clone()
            .unwrap_or_else(|| output.join("oracle.csv")),
    )?;
    let rom = root.join("Phantasy Star IV (USA).md");
    let mut command = None;
    if retained.is_none() {
        // Reuse the one host-freshness/build seam; all capture parameters,
        // execution, measurement and receipt generation belong to Rust here.
        let resolved = Command::new("python3")
            .args(["-m", "oracle.host_binary", "--path"])
            .current_dir(&root)
            .output()?;
        std::io::Write::write_all(&mut std::io::stderr(), &resolved.stderr)?;
        if !resolved.status.success() {
            return Err(format!("oracle host resolver failed: {}", resolved.status).into());
        }
        let host = String::from_utf8(resolved.stdout)?.trim().to_owned();
        if host.is_empty() || host.contains('\n') {
            return Err("invalid oracle host path".into());
        }
        let path = |relative: &str| root.join(relative).to_string_lossy().into_owned();
        let mut invocation = vec![
            host,
            "--core".into(),
            path("oracle/core/genesis_plus_gx_libretro.so"),
            "--rom".into(),
            rom.to_string_lossy().into_owned(),
            "--map".into(),
            path("oracle/ram_map.tsv"),
            "--tape".into(),
            path("oracle/tapes/28_meeting_rika_retail_probe.tape"),
            "--groups".into(),
            "core,objects,camera,pos,flagbytes".into(),
            "--out".into(),
            log.to_string_lossy().into_owned(),
            "--dump-frames".into(),
            "7199,7200,7208,7213,7218,7268,7330".into(),
            "--dump-frames-dir".into(),
            output.join("frames").to_string_lossy().into_owned(),
        ];
        for patch in PATCHES {
            invocation.extend(["--ram-patch".into(), patch.into()]);
        }
        let status = Command::new(&invocation[0])
            .args(&invocation[1..])
            .current_dir(&root)
            .status()?;
        if !status.success() {
            return Err(format!("oracle capture failed: {status}").into());
        }
        command = Some(invocation);
    }
    let log_bytes = std::fs::read(&log)?;
    let measurement = measure(std::str::from_utf8(&log_bytes)?)?;
    let mut receipt = json!({"measurement":measurement,"command":command,
        "log_sha256":psiv_pack::png::hex(&Sha256::digest(&log_bytes))});
    if rom.is_file() {
        receipt["rom_sha256"] = json!(hash(&rom)?);
    }
    std::fs::write(
        output.join("receipt.json"),
        format!("{}\n", serde_json::to_string_pretty(&receipt)?),
    )?;
    println!("{}", receipt["measurement"]);
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("tyler-grave: {error}");
            ExitCode::FAILURE
        }
    }
}
