#![forbid(unsafe_code)]

use serde_json::{Value, json};
use std::io::{self, Read};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut text = String::new();
    let result = io::stdin()
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())
        .and_then(|_| serde_json::from_str::<Value>(&text).map_err(|e| e.to_string()))
        .and_then(|request| {
            if request["operation"] == "identity" {
                Ok(json!({
                    "rust/psiv-native-verify/src/lib.rs": include_str!("lib.rs"),
                    "rust/psiv-native-verify/src/main.rs": include_str!("main.rs"),
                    "rust/psiv-native-verify/Cargo.toml": include_str!("../Cargo.toml"),
                    "rust/Cargo.toml": include_str!("../../Cargo.toml"),
                    "rust/Cargo.lock": include_str!("../../Cargo.lock"),
                    "rust-toolchain.toml": include_str!("../../../rust-toolchain.toml")
                }))
            } else {
                psiv_native_verify::exchange(&request)
            }
        });
    match result {
        Ok(value) => {
            println!("{value}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            println!("{}", json!({"error":error}));
            ExitCode::FAILURE
        }
    }
}
