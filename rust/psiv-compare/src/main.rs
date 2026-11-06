//! `presentation-rmse [--region x,y,width,height] GODOT.png ORACLE.png`: one
//! presentation comparison (`psiv_compare`), printed as the line
//! `tools/certify.py` reads. Exit 0 with the line, 1 when an image or the
//! region can't be compared, 2 on bad usage.

use std::path::PathBuf;
use std::process::ExitCode;

use psiv_compare::{CompareError, Region, compare};

const USAGE: &str = "usage: presentation-rmse [--region x,y,width,height] GODOT.png ORACLE.png";

fn main() -> ExitCode {
    let mut region = None;
    let mut paths = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let value = match arg.as_str() {
            "--region" => args.next(),
            _ => match arg.strip_prefix("--region=") {
                Some(value) => Some(value.to_owned()),
                None if arg.starts_with('-') => {
                    eprintln!("presentation-rmse: unknown option {arg}\n{USAGE}");
                    return ExitCode::from(2);
                }
                None => {
                    paths.push(PathBuf::from(arg));
                    continue;
                }
            },
        };
        let Some(value) = value else {
            eprintln!("presentation-rmse: --region needs x,y,width,height\n{USAGE}");
            return ExitCode::from(2);
        };
        match Region::parse(&value) {
            Ok(parsed) => region = Some(parsed),
            Err(error) => {
                eprintln!("presentation-rmse: {error}\n{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    let [godot, oracle] = &paths[..] else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let read = |path: &PathBuf| {
        std::fs::read(path)
            .map_err(|error| CompareError(format!("cannot read {}: {error}", path.display())))
    };
    match read(godot)
        .and_then(|godot| Ok((godot, read(oracle)?)))
        .and_then(|(godot, oracle)| compare(&godot, &oracle, region))
    {
        Ok(line) => {
            println!("{line}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("presentation-rmse: error: {error}");
            ExitCode::FAILURE
        }
    }
}
