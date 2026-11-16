//! PSIV owns the manifest input that supplies its pinned Rust toolchain.
//! Nix evaluation checks availability; this contract prevents lock maintenance
//! from restoring burrow's older transitive overlay. Runs without local assets.

use serde_json::Value;

const FLAKE: &str = include_str!("../../../flake.nix");
const LOCK: &str = include_str!("../../../flake.lock");
const DECLARATION: &str = "inputs.burrow.inputs.rust-overlay.url = ";

fn assert_owned(flake: &str, lock: &Value) {
    let url = flake
        .lines()
        .find_map(|line| line.trim().strip_prefix(DECLARATION))
        .and_then(|value| value.strip_prefix('"'))
        .and_then(|value| value.split_once('"'))
        .map(|(url, _)| url)
        .expect("PSIV must override burrow's older rust-overlay");
    let revision = url
        .split_once('?')
        .into_iter()
        .flat_map(|(_, query)| query.split('&'))
        .find_map(|parameter| parameter.strip_prefix("rev="))
        .expect("the overlay override must pin a revision");
    assert_eq!(revision.len(), 40);
    assert!(
        revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );
    let nodes = &lock["nodes"];
    let root = lock["root"].as_str().expect("lock root");
    let burrow = nodes[root]["inputs"]["burrow"]
        .as_str()
        .expect("burrow input");
    let overlay = nodes[burrow]["inputs"]["rust-overlay"]
        .as_str()
        .expect("rust-overlay input");
    let overlay = &nodes[overlay];
    assert_eq!(overlay["original"]["rev"], revision);
    assert_eq!(overlay["locked"]["rev"], revision);
    assert_eq!(
        overlay["inputs"]["nixpkgs"],
        serde_json::json!(["burrow", "nixpkgs"])
    );
}

#[test]
fn psiv_owns_the_overlay_revision_instead_of_inheriting_burrows_lock() {
    assert_owned(FLAKE, &serde_json::from_str(LOCK).unwrap());
}

#[test]
fn negative_control_an_inherited_overlay_is_rejected() {
    let inherited = FLAKE
        .lines()
        .filter(|line| !line.trim().starts_with(DECLARATION))
        .collect::<Vec<_>>()
        .join("\n");
    let lock: Value = serde_json::from_str(LOCK).unwrap();
    assert!(std::panic::catch_unwind(|| assert_owned(&inherited, &lock)).is_err());
}
