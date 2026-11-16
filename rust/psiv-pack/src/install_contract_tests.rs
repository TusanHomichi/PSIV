//! Remaining installer contracts from the Python builder, with real I/O.
use super::*;
use crate::test_support::scratch;

fn marker(dir: &Path, value: &str) -> Result<(), PackError> {
    std::fs::write(dir.join("marker"), value).map_err(|e| io_error("write", dir, e))
}

#[test]
fn readers_see_the_old_pack_until_commit_and_external_targets_are_untouched() {
    let root = scratch("reader-commit");
    let output = root.join("pack");
    install_atomically(&output, |dir| marker(dir, "old")).unwrap();
    install_atomically(&output, |dir| {
        marker(dir, "new")?;
        assert_eq!(
            std::fs::read_to_string(output.join("marker")).unwrap(),
            "old"
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(output.join("marker")).unwrap(),
        "new"
    );
    #[cfg(unix)]
    {
        let external = root.join("external");
        std::fs::create_dir(&external).unwrap();
        marker(&external, "theirs").unwrap();
        std::fs::remove_file(&output).unwrap();
        std::os::unix::fs::symlink(&external, &output).unwrap();
        install_atomically(&output, |dir| marker(dir, "ours")).unwrap();
        assert_eq!(
            std::fs::read_to_string(external.join("marker")).unwrap(),
            "theirs"
        );
        std::fs::remove_file(&output).unwrap();
        std::os::unix::fs::symlink("missing-version", &output).unwrap();
        install_atomically(&output, |dir| marker(dir, "repaired")).unwrap();
        assert_eq!(
            std::fs::read_to_string(output.join("marker")).unwrap(),
            "repaired"
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn directory_mode_installs_and_rolls_back_a_failed_second_rename() {
    let root = scratch("directory-rollback");
    let output = root.join("pack");
    std::fs::create_dir_all(&output).unwrap();
    marker(&output, "old").unwrap();
    let old = root.join(".pack.link-1-ab");
    assert!(replace_directory(&output, &root.join("missing-version"), &old).is_err());
    assert_eq!(
        std::fs::read_to_string(output.join("marker")).unwrap(),
        "old"
    );
    assert!(!old.exists());
    let version = root.join("version");
    std::fs::create_dir(&version).unwrap();
    marker(&version, "new").unwrap();
    assert!(
        replace_directory(&output, &version, &old)
            .unwrap()
            .is_empty()
    );
    assert!(!output.is_symlink());
    assert_eq!(
        std::fs::read_to_string(output.join("marker")).unwrap(),
        "new"
    );
    assert!(!old.exists());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn killed_build_child() {
    let Some(root) = std::env::var_os("PSIV_INSTALL_CHILD_ROOT") else {
        return;
    };
    use std::io::Write as _;
    install_atomically(
        &PathBuf::from(root).join("pack"),
        |dir| -> Result<(), PackError> {
            marker(dir, "partial")?;
            println!("STAGED");
            std::io::stdout().flush().unwrap();
            loop {
                std::thread::park();
            }
        },
    )
    .unwrap();
}

#[test]
fn killing_a_builder_keeps_the_old_pack_and_the_next_install_cleans_staging() {
    use std::io::BufRead as _;
    use std::process::{Command, Stdio};
    let root = scratch("killed-build");
    let output = root.join("pack");
    install_atomically(&output, |dir| marker(dir, "old")).unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "install::contract_tests::killed_build_child",
            "--nocapture",
        ])
        .env("PSIV_INSTALL_CHILD_ROOT", &root)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut reader = std::io::BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line).unwrap() == 0 {
            child.wait().unwrap();
            panic!("child exited before staging");
        }
        if line.trim() == "STAGED" {
            break;
        }
    }
    let staging = std::fs::read_dir(&root)
        .unwrap()
        .filter_map(|e| {
            let path = e.unwrap().path();
            path.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .contains(".build-")
                .then_some(path)
        })
        .collect::<Vec<_>>();
    // Reap before asserting: even a broken assertion cannot strand the child.
    child.kill().unwrap();
    child.wait().unwrap();
    assert_eq!(staging.len(), 1);
    assert!(staging[0].exists());
    assert_eq!(
        std::fs::read_to_string(output.join("marker")).unwrap(),
        "old"
    );
    install_atomically(&output, |dir| marker(dir, "new")).unwrap();
    assert!(!staging[0].exists());
    assert_eq!(
        std::fs::read_to_string(output.join("marker")).unwrap(),
        "new"
    );
    std::fs::remove_dir_all(root).unwrap();
}
