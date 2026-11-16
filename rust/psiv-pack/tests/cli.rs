//! ROM-free checks of the player's executable interface.
use std::path::PathBuf;
use std::process::Command;

#[test]
fn claims_and_argument_or_input_errors_use_the_public_protocol() {
    let binary = env!("CARGO_BIN_EXE_psiv-pack");
    let claims = Command::new(binary).arg("--claims").output().unwrap();
    assert!(claims.status.success());
    assert!(claims.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&claims.stdout).unwrap();
    assert_eq!(
        value,
        serde_json::Value::from(&psiv_pack::claims(psiv_pack::SECTIONS))
    );
    let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../build/pack-cli-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap();
    let rom = scratch.join("wrong-rom");
    std::fs::write(&rom, [0; 16]).unwrap();
    let out = scratch.join("pack");
    std::fs::create_dir(&out).unwrap();
    std::fs::write(out.join("sentinel"), "old").unwrap();
    let cases = [
        vec![],
        vec!["--unknown".into()],
        vec!["--claims".into(), "extra".into()],
        vec![rom.clone()],
        vec![rom.clone(), out.clone(), "extra".into()],
        vec!["--bad".into(), out.clone()],
        vec![scratch.join("missing"), out.clone()],
        vec![scratch.clone(), out.clone()],
        vec![rom, out.clone()],
    ];
    for args in cases {
        let output = Command::new(binary).args(&args).output().unwrap();
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty());
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.starts_with("psiv-pack: error: "), "{error}");
        assert_eq!(
            std::fs::read_to_string(out.join("sentinel")).unwrap(),
            "old"
        );
        assert_eq!(std::fs::read_dir(&out).unwrap().count(), 1);
    }
    std::fs::remove_dir_all(scratch).unwrap();
}
