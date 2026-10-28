//! `psiv-pack <rom> <pack>`: see the library's documentation for the
//! interface the setup screen reads.

use std::io::Write as _;
use std::path::Path;
use std::process::ExitCode;

use psiv_pack::{PackError, Rom, SECTIONS, build_sections, claims, default_workers, install};

const USAGE: &str = "usage: psiv-pack <rom> <pack> | psiv-pack --claims";

fn run(args: &[String]) -> Result<(), PackError> {
    match args {
        [flag] if flag == "--claims" => {
            println!("{}", psiv_pack::json::dumps(&claims(SECTIONS), true));
            Ok(())
        }
        [rom, pack] if !rom.starts_with("--") && !pack.starts_with("--") => {
            let rom = Rom::read(Path::new(rom))?;
            let stdout = std::io::stdout();
            let installed = install::install_atomically(Path::new(pack), |root| {
                build_sections(
                    &rom,
                    root,
                    SECTIONS,
                    default_workers(),
                    |done, total, name| {
                        let mut out = stdout.lock();
                        let _ = writeln!(out, "progress {done}/{total} {name}");
                        let _ = out.flush();
                    },
                )
            })?;
            for warning in installed.warnings {
                eprintln!("psiv-pack: warning: {warning}");
            }
            println!("installed {pack}");
            Ok(())
        }
        _ => Err(PackError(USAGE.into())),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("psiv-pack: error: {error}");
            ExitCode::from(2)
        }
    }
}
