//! Embedded build inputs let a fresh verifier reject stale local binaries.
use serde_json::{Value, json};

pub(super) fn build_inputs() -> Value {
    json!({
        "rust/psiv-memory-verdict/src/identity.rs": include_str!("identity.rs"),
        "rust/psiv-memory-verdict/src/lib.rs": include_str!("lib.rs"),
        "rust/psiv-memory-verdict/src/main.rs": include_str!("main.rs"),
        "rust/psiv-memory-verdict/Cargo.toml": include_str!("../Cargo.toml"),
        "rust/Cargo.toml": include_str!("../../Cargo.toml"),
        "rust/Cargo.lock": include_str!("../../Cargo.lock"),
        "rust-toolchain.toml": include_str!("../../../rust-toolchain.toml")
    })
}
