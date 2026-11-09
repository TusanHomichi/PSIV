use psiv_float_guard::{examples, float_sites, scan};
use serde_json::{Value, json};
use std::io::{self, Read};

fn run() -> Result<Value, String> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map_err(|e| e.to_string())?;
    let request: Value = serde_json::from_str(&input).map_err(|e| e.to_string())?;
    match request["operation"].as_str() {
        Some("float") => Ok(json!(float_sites(
            request["source"].as_str().ok_or("missing source")?
        ))),
        Some("docs") => Ok(json!(examples(
            request["source"].as_str().ok_or("missing source")?
        )?)),
        Some("scan") => Ok(json!(scan(std::path::Path::new(
            request["root"].as_str().ok_or("missing root")?
        ))?)),
        _ => Err("unknown float guard operation".into()),
    }
}
fn main() {
    match run() {
        Ok(result) => println!("{result}"),
        Err(error) => {
            eprintln!("float guard: {error}");
            std::process::exit(2);
        }
    }
}
