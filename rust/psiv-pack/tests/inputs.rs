//! Repository input guard (#214, #561). Run through the build heavy-lock class.
//! Only Linux can run the syscall audit; other hosts explicitly skip it.
#[cfg(target_os = "linux")]
#[path = "support/input_audit.rs"]
mod audit;

#[cfg(target_os = "linux")]
#[test]
fn actual_rust_builder_reads_only_declared_inputs_and_rejects_injected_reads() {
    use audit::{checked, copy_tree, create_run_root, trace};
    use std::path::Path;
    use std::process::Command;
    use std::time::Instant;
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let rom_name = "Phantasy Star IV (USA).md";
    let Some(bytes) = audit::retail_bytes(&repo.join(rom_name)) else {
        return;
    };
    // Everything is kept under build/, including failed traces and the mutated
    // candidate. No ignored input other than the verified ROM enters the tree.
    let scratch = create_run_root(&repo.join(format!("build/pack-inputs-{}", std::process::id())));
    let tree = scratch.join("tree");
    copy_tree(&repo, &tree);
    let rom = tree.join(rom_name);
    std::fs::write(&rom, bytes).unwrap();
    let target = scratch.join("target");
    let build = || {
        checked(
            Command::new("cargo")
                .current_dir(&tree)
                .args([
                    "build",
                    "--release",
                    "--locked",
                    "--manifest-path",
                    "rust/Cargo.toml",
                    "-p",
                    "psiv-pack",
                    "--bin",
                    "psiv-pack",
                    "--target-dir",
                ])
                .arg(&target),
        )
    };
    build();
    let binary = target.join("release/psiv-pack");
    // An independent std-only program defines exact loader/runtime reads.
    // Unlike tracing `psiv-pack --claims`, this cannot excuse an accidental
    // read added to the builder's startup. It shares no builder source.
    let probe_src = scratch.join("runtime").with_extension("rs");
    let probe = scratch.join("runtime");
    std::fs::write(
        &probe_src,
        "fn main() { std::thread::scope(|s| { for _ in 0..8 { s.spawn(|| { let _ = std::thread::available_parallelism(); let v = vec![0u8; 4_000_000]; std::hint::black_box(v); }); } }); }",
    )
    .unwrap();
    checked(
        Command::new("rustc")
            .args(["--edition=2024", "-O"])
            .arg(&probe_src)
            .arg("-o")
            .arg(&probe),
    );
    let baseline = trace(&probe, &[], &tree, &scratch.join("runtime.trace"), None);
    assert!(baseline.process.status.success());
    let mut runtime = baseline.reads;
    runtime.remove(&probe);
    // glibc can query these only when thread creation or allocator contention
    // crosses a threshold; a small independent probe need not trigger them.
    // These are exact kernel metadata files, never directory allowances.
    runtime.extend(
        [
            "/proc/sys/vm/overcommit_memory",
            "/sys/devices/system/cpu/online",
        ]
        .map(std::path::PathBuf::from),
    );
    let output = scratch.join("output");
    std::fs::create_dir(&output).unwrap();
    let pack = output.join("pack");
    let started = Instant::now();
    let clean = trace(
        &binary,
        &[&rom, &pack],
        &tree,
        &scratch.join("clean.trace"),
        Some(&output),
    );
    let seconds = started.elapsed().as_secs_f64();
    assert!(clean.reads.contains(&rom), "audit did not observe the ROM");
    let bad = clean.offenders(&runtime, &rom, &output, &binary);
    assert!(
        bad.is_empty(),
        "undeclared Rust pack reads/probes: {bad:#?}"
    );
    assert!(
        clean.process.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&clean.process.stderr)
    );
    assert!(pack.join("manifest.json").is_file());
    eprintln!(
        "[pack-inputs] clean tracked tree + ROM, full Rust pack: {seconds:.2}s; evidence {}",
        scratch.display()
    );

    let planted = [
        "oracle/layouts/planted_capture.json",
        &["psiv_tools", "untracked_table.json"].join("/"),
    ];
    for name in planted {
        let path = tree.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "{}").unwrap();
    }
    let external = scratch.join("undeclared-table");
    std::fs::write(&external, "{}").unwrap();
    // The path inside the permitted output resolves to an undeclared input.
    std::os::unix::fs::symlink(&external, output.join("alias")).unwrap();
    let missing = scratch.join("absent-link-target");
    let directory = scratch.join("external-directory");
    std::fs::create_dir(&directory).unwrap();
    let metadata = scratch.join("metadata-target");
    let staged = scratch.join("staged-target");
    for path in [&metadata, &staged] {
        std::fs::write(path, "{}").unwrap();
    }
    std::os::unix::fs::symlink(&missing, output.join("dangling")).unwrap();
    std::os::unix::fs::symlink(&directory, output.join("directory")).unwrap();
    std::os::unix::fs::symlink(&output, output.join("branch")).unwrap();
    let runtime_target = Path::new("/proc/self/maps");
    assert!(runtime.contains(runtime_target));
    std::os::unix::fs::symlink(runtime_target, output.join("runtime-alias")).unwrap();
    let main = tree.join("rust/psiv-pack/src/main.rs");
    let source = std::fs::read_to_string(&main).unwrap();
    let injection = format!(
        r#"
        for name in {:?} {{ let _ = std::fs::read(name); }}
        let _ = std::fs::read("docs/README.md");
        let _ = std::fs::read({:?});
        std::thread::spawn(|| {{
            let _ = std::fs::File::open("oracle/layouts/absent_capture.json");
            let _ = std::fs::metadata(["psiv_tools", "absent_table.json"].join("/"));
        }}).join().unwrap();
        let out = std::path::Path::new({:?});
        let _ = std::fs::read(out.join("dangling"));
        let _ = std::fs::metadata(out.join("directory/missing-descendant"));
        std::fs::remove_file(out.join("dangling")).unwrap();
        std::os::unix::fs::symlink({:?}, out.join("transient")).unwrap();
        let _ = std::fs::metadata(out.join("transient"));
        std::fs::remove_file(out.join("transient")).unwrap();
        std::fs::create_dir(out.join("stage")).unwrap();
        std::os::unix::fs::symlink({:?}, out.join("stage/alias")).unwrap();
        let _ = std::fs::metadata(out.join("stage/alias"));
        std::fs::rename(out.join("stage"), out.join("renamed-stage")).unwrap();
        std::fs::remove_dir_all(out.join("renamed-stage")).unwrap();
        let _ = std::fs::metadata(out.join("branch/../parent-probe"));
        let _ = std::fs::metadata(out.join("runtime-alias/missing-descendant"));
    "#,
        planted,
        output.join("alias"),
        output,
        metadata,
        staged
    );
    assert_eq!(source.matches("fn main() -> ExitCode {").count(), 1);
    std::fs::write(
        &main,
        source.replace(
            "fn main() -> ExitCode {",
            &format!("fn main() -> ExitCode {{{injection}"),
        ),
    )
    .unwrap();
    build();
    let injected = trace(
        &binary,
        &[Path::new("--claims")],
        &tree,
        &scratch.join("injected.trace"),
        Some(&output),
    );
    assert!(
        injected.process.status.success(),
        "injection must not rely on a build failure"
    );
    let bad = injected.offenders(&runtime, &rom, &output, &binary);
    let expected = [
        tree.join(planted[0]),
        tree.join(planted[1]),
        tree.join("docs/README.md"),
        tree.join("oracle/layouts/absent_capture.json"),
        tree.join("psiv_tools").join("absent_table.json"),
        external,
        missing,
        directory,
        metadata,
        staged,
        output.join("branch/../parent-probe"),
        runtime_target.to_owned(),
    ]
    .into();
    assert_eq!(
        bad, expected,
        "negative control must name every undeclared input/link dependency"
    );
    eprintln!(
        "[pack-inputs] rejected all {} injected Rust input/link dependencies",
        bad.len()
    );
}

#[cfg(not(target_os = "linux"))]
#[test]
fn input_audit_requires_linux_strace() {
    eprintln!("Rust pack input audit skipped: requires Linux strace");
}
