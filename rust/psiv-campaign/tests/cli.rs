//! The `psiv-campaign` binary: argument handling and exit codes.

mod common;

use std::path::PathBuf;
use std::process::Command;

use common::{MAIN_ROUTE, pack};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_psiv-campaign"))
}

fn scratch(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name)
}

#[test]
fn usage_errors_exit_two() {
    for args in [
        &[][..],
        &["frobnicate"],
        &["plan"],
        &["plan", "--from-map", "0"],
        &["validate"],
    ] {
        let out = bin().args(args).output().unwrap();
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(!out.stderr.is_empty(), "{args:?} prints a usage message");
    }
    let out = bin()
        .args([
            "plan",
            "--from-map",
            "zero",
            "--from-cell",
            "1,2",
            "--to-map",
            "3",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("bad map id"));
}

#[test]
fn plan_prints_the_aiedo_route() {
    if pack().is_none() {
        return;
    }
    let out = bin()
        .args([
            "plan",
            "--from-map",
            "0",
            "--from-cell",
            "84,64",
            "--to-map",
            "0x54",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("transition index 7"), "{text}");
    assert!(text.contains("0x100706"), "{text}");
    assert!(text.contains("arrive (47,83) facing Up"), "{text}");
    assert!(text.contains("67 steps"), "{text}");
}

#[test]
fn plan_exits_one_when_there_is_no_walkable_chain() {
    if pack().is_none() {
        return;
    }
    let out = bin()
        .args([
            "plan",
            "--from-map",
            "0",
            "--from-cell",
            "84,64",
            "--to-map",
            "1",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stdout).contains("no plan"));
}

#[test]
fn validate_accepts_main_and_rejects_a_broken_copy() {
    if pack().is_none() {
        return;
    }
    let ok = bin().args(["validate", MAIN_ROUTE]).output().unwrap();
    assert_eq!(
        ok.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&ok.stdout)
    );
    let text = String::from_utf8(ok.stdout).unwrap();
    assert!(text.contains("0 errors"), "{text}");
    assert!(
        !text.contains("verify: chapter"),
        "R1 played every step R0 left unconfirmed; none is listed: {text}"
    );

    // A step marked unconfirmed is still listed by name.
    let marked = std::fs::read_to_string(MAIN_ROUTE).unwrap().replacen(
        "{\"do\": \"expect\", \"flags_set\": [\"event:0x8\"]",
        "{\"do\": \"expect\", \"verify\": true, \"flags_set\": [\"event:0x8\"]",
        1,
    );
    let path = scratch("marked-route.json");
    std::fs::write(&path, marked).unwrap();
    let listed = bin().arg("validate").arg(&path).output().unwrap();
    let text = String::from_utf8(listed.stdout).unwrap();
    assert!(
        text.contains("verify: chapter \"academy\" objective 1"),
        "{text}"
    );

    let broken = std::fs::read_to_string(MAIN_ROUTE)
        .unwrap()
        .replace("\"item\": \"CIRCLET\"", "\"item\": \"NO-SUCH-ITEM\"");
    let path = scratch("broken-route.json");
    std::fs::write(&path, broken).unwrap();
    let bad = bin().arg("validate").arg(&path).output().unwrap();
    assert_eq!(bad.status.code(), Some(1));
    let text = String::from_utf8(bad.stdout).unwrap();
    assert!(
        text.contains("error: chapter \"zema-outfit\" objective"),
        "{text}"
    );
    assert!(text.contains("NO-SUCH-ITEM"), "{text}");
}
