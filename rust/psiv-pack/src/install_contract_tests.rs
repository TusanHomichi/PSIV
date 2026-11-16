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

// A child is always killed and reaped, including readiness failures and panics.
struct TestChild(std::process::Child);
impl Drop for TestChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        self.0.wait().expect("reap installer child");
    }
}

fn installer_child(root: &Path, threads: usize, signal_ready: bool) -> TestChild {
    use std::process::{Command, Stdio};
    TestChild(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "install::contract_tests::killed_build_child",
                "--nocapture",
            ])
            .env("PSIV_INSTALL_CHILD_ROOT", root)
            .env(
                "PSIV_INSTALL_SIGNAL_READY",
                if signal_ready { "yes" } else { "no" },
            )
            .env("RUST_TEST_THREADS", threads.to_string())
            .stdout(Stdio::null())
            .spawn()
            .unwrap(),
    )
}

fn await_staging(
    child: &mut TestChild,
    root: &Path,
    timeout: std::time::Duration,
) -> Result<(), String> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if root.join("ready").is_file() {
            return Ok(());
        }
        if let Some(status) = child.0.try_wait().map_err(|e| e.to_string())? {
            return Err(format!("installer child exited before staging: {status}"));
        }
        if std::time::Instant::now() >= deadline {
            return Err("timed out waiting for staging".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn killed_build_child() {
    let Some(root) = std::env::var_os("PSIV_INSTALL_CHILD_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    install_atomically(&root.join("pack"), |dir| -> Result<(), PackError> {
        marker(dir, "partial")?;
        if std::env::var("PSIV_INSTALL_SIGNAL_READY").as_deref() == Ok("yes") {
            // A separate file is independent of libtest's serial/parallel
            // stdout prefixes, buffering and capture settings.
            std::fs::write(root.join("ready"), "staged").unwrap();
        }
        loop {
            std::thread::park();
        }
    })
    .unwrap();
}

#[test]
fn readiness_timeout_and_unwinding_terminate_and_reap_the_child() {
    let root = scratch("child-timeout");
    std::fs::create_dir_all(&root).unwrap();
    let mut child = installer_child(&root, 1, false);
    let pid = child.0.id();
    assert!(
        await_staging(&mut child, &root, std::time::Duration::from_millis(150))
            .unwrap_err()
            .contains("timed out")
    );
    drop(child);
    assert!(!alive(pid), "timeout child must be reaped");
    let pid = std::cell::Cell::new(0);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let child = installer_child(&root, 4, false);
            pid.set(child.0.id());
            panic!("exercise child cleanup on assertion failure");
        }))
        .is_err()
    );
    assert!(!alive(pid.get()), "unwinding must reap the child");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn killing_a_builder_keeps_the_old_pack_and_the_next_install_cleans_staging() {
    for threads in [1, 4] {
        let root = scratch("killed-build");
        let output = root.join("pack");
        install_atomically(&output, |dir| marker(dir, "old")).unwrap();
        let mut child = installer_child(&root, threads, true);
        await_staging(&mut child, &root, std::time::Duration::from_secs(10)).unwrap();
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
        drop(child); // kill and reap before the next install checks liveness
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
}
