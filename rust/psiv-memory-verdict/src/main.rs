#![forbid(unsafe_code)]
use std::io::{self, Read};

fn main() {
    let mut input = String::new();
    let result = io::stdin()
        .read_to_string(&mut input)
        .map_err(|error| error.to_string())
        .and_then(|_| serde_json::from_str(&input).map_err(|error| error.to_string()));
    match result {
        Ok(request) => println!("{}", psiv_memory_verdict::evaluate(request)),
        Err(error) => {
            println!(
                "{}",
                serde_json::json!({"error": format!("invalid memory policy JSON: {error}")})
            );
            std::process::exit(2);
        }
    }
}
