//! JSON adapter for the flight-fade oracle validator; no capture logic here.

use psiv_compare::flight_fades::{self, Capture};
use serde_json::json;

fn run(args: &[String]) -> Result<(), String> {
    let output = match args {
        [mode] if mode == "config" => json!({"watches": flight_fades::watches()}),
        [mode, path] if mode == "analyze" => {
            let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
            let capture: Capture =
                serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
            flight_fades::analyze(capture)?
        }
        _ => return Err("usage: flight-fades config | analyze INPUT.json".into()),
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&output).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn main() {
    if let Err(error) = run(&std::env::args().skip(1).collect::<Vec<_>>()) {
        eprintln!("flight-fades: {error}");
        std::process::exit(2);
    }
}
