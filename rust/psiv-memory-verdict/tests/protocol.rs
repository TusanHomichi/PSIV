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
