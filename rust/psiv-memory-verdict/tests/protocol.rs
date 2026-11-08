use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn malformed_json_exits_two_and_emits_an_error_object() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_psiv-memory-verdict"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"{broken").unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        response["error"]
            .as_str()
            .unwrap()
            .contains("invalid memory policy JSON")
    );
}

#[test]
fn identity_matches_current_build_input_contents() {
    let identity = psiv_memory_verdict::evaluate(serde_json::json!({"operation": "identity"}));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (name, contents) in identity.as_object().expect("build identity") {
        assert_eq!(
            contents.as_str().expect("source text"),
            std::fs::read_to_string(root.join(name)).expect("current build input"),
            "{name}"
        );
    }
    assert_eq!(identity.as_object().unwrap().len(), 7);
}
