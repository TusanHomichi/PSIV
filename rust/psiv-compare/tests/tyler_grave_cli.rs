//! Failed validation must not produce a success receipt.

#[test]
fn invalid_retained_csv_fails_without_writing_a_receipt() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("psiv-tyler-{}-{unique}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let log = directory.join("invalid.csv");
    std::fs::write(&log, "frame\n7200\n").unwrap();
    let output = directory.join("output");
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_tyler-grave"))
        .arg("--log")
        .arg(&log)
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stderr).contains("missing frame 7199"));
    assert!(!output.join("receipt.json").exists());
    std::fs::remove_dir_all(&directory).unwrap();
}
