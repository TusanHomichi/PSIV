//! Linux syscall audit of the real CLI, including threads and failed probes.
//! Runtime allowances come from a separate std-only probe, never from the
//! builder being checked. No repository path is an allowed runtime input.
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};

pub fn checked(command: &mut Command) -> Output {
    let output = command.output().expect("start audit prerequisite");
    assert!(
        output.status.success(),
        "{command:?}:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

pub fn copy_tree(repo: &Path, tree: &Path) {
    let files = checked(Command::new("git").current_dir(repo).args([
        "ls-files",
        "-z",
        "--cached",
        "--others",
        "--exclude-standard",
    ]));
    for name in files.stdout.split(|&b| b == 0).filter(|n| !n.is_empty()) {
        let name = std::str::from_utf8(name).expect("UTF-8 repository path");
        let source = repo.join(name);
        let target = tree.join(name);
        let Ok(meta) = source.symlink_metadata() else {
            continue;
        };
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        if meta.is_symlink() {
            std::os::unix::fs::symlink(std::fs::read_link(source).unwrap(), target).unwrap();
        } else if meta.is_file() {
            std::fs::copy(source, target).unwrap();
        }
    }
}

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => (),
            part => out.push(part),
        }
    }
    out
}

// strace C strings (including non-ASCII, quotes, backslashes and octal).
fn quoted(text: &str) -> (String, usize) {
    assert!(text.starts_with('"'), "missing quoted path: {text}");
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 1;
    while i < bytes.len() {
        let b = bytes[i];
        i += 1;
        match b {
            b'"' => return (String::from_utf8(out).expect("UTF-8 input path"), i),
            b'\\' => {
                let b = bytes[i];
                i += 1;
                out.push(match b {
                    b'n' => b'\n',
                    b'r' => b'\r',
                    b't' => b'\t',
                    b'\\' => b'\\',
                    b'"' => b'"',
                    b'0'..=b'7' => {
                        let mut value = b - b'0';
                        for _ in 0..2 {
                            if let Some(b'0'..=b'7') = bytes.get(i) {
                                value = value * 8 + bytes[i] - b'0';
                                i += 1;
                            } else {
                                break;
                            }
                        }
                        value
                    }
                    _ => panic!("unsupported strace escape: {text}"),
                });
            }
            b => out.push(b),
        }
    }
    panic!("unterminated strace string: {text}")
}

fn annotation(text: &str) -> Option<&str> {
    let (_, tail) = text.split_once('<')?;
    let (path, _) = tail.split_once('>')?;
    path.starts_with('/').then_some(path)
}

fn own_proc(path: PathBuf, pid: &str) -> PathBuf {
    if let Ok(tail) = path.strip_prefix(format!("/proc/{pid}")) {
        Path::new("/proc/self").join(tail)
    } else {
        path
    }
}

// Output links are dependencies at creation, even if an eventual read fails
// or the link is deleted before parsing. A self-contained pack cannot depend
// on an external link target. Parent traversal is rejected separately rather
// than incorrectly cancelling `..` before following a symlink.
fn output_links(root: &Path, reads: &mut BTreeSet<PathBuf>) {
    let meta = match root.symlink_metadata() {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
        Err(e) => panic!("cannot inspect initial output {}: {e}", root.display()),
    };
    if meta.is_symlink() {
        reads.insert(
            root.parent()
                .unwrap()
                .join(std::fs::read_link(root).unwrap()),
        );
    } else if meta.is_dir() {
        for entry in std::fs::read_dir(root).unwrap() {
            output_links(&entry.unwrap().path(), reads);
        }
    }
}

fn at_path<'a>(args: &'a str, cwd: &Path) -> (PathBuf, &'a str) {
    let (fd, rest) = args.split_once(", ").expect("dirfd and path");
    let base = annotation(fd).map(PathBuf::from).unwrap_or_else(|| {
        assert_eq!(fd, "AT_FDCWD", "unresolved dirfd: {args}");
        cwd.to_owned()
    });
    let (name, used) = quoted(rest);
    (base.join(name), &rest[used..])
}

/// Read/probe paths and link dependencies from one syscall. Record both
/// spellings and the kernel's resolved descriptor; never use a later
/// canonicalization as the sole protection for output symlinks.
fn paths(line: &str, cwd: &Path, pid: &str) -> Vec<PathBuf> {
    if line.starts_with("+++") || line.starts_with("---") {
        return vec![];
    }
    let (call, args) = line.split_once('(').expect("complete strace syscall");
    assert!(
        !matches!(call, "chdir" | "fchdir"),
        "builder changed cwd: {line}"
    );
    if matches!(call, "symlink" | "symlinkat") {
        let (target, used) = quoted(args);
        let rest = args[used..].strip_prefix(", ").unwrap();
        let link = if call == "symlinkat" {
            at_path(rest, cwd).0
        } else {
            cwd.join(quoted(rest).0)
        };
        return vec![link.parent().unwrap().join(target)];
    }
    if matches!(
        call,
        "rename" | "renameat" | "renameat2" | "link" | "linkat"
    ) {
        // Moving or hard-linking a foreign entry into output is also an
        // undeclared dependency, even if the source later disappears.
        let (source, rest) = if matches!(call, "rename" | "link") {
            let (name, used) = quoted(args);
            (cwd.join(name), &args[used..])
        } else {
            at_path(args, cwd)
        };
        let rest = rest.strip_prefix(", ").unwrap();
        let target = if matches!(call, "rename" | "link") {
            cwd.join(quoted(rest).0)
        } else {
            at_path(rest, cwd).0
        };
        return vec![source, target];
    }
    let (base, rest) = if matches!(
        call,
        "openat"
            | "openat2"
            | "newfstatat"
            | "statx"
            | "readlinkat"
            | "faccessat"
            | "faccessat2"
            | "execveat"
            | "unlinkat"
    ) {
        let (fd, rest) = args.split_once(", ").expect("dirfd and path");
        let base = annotation(fd).map(PathBuf::from).unwrap_or_else(|| {
            assert_eq!(fd, "AT_FDCWD", "unresolved dirfd: {line}");
            cwd.to_owned()
        });
        (base, rest)
    } else {
        (cwd.to_owned(), args)
    };
    let (name, consumed) = quoted(rest);
    let flags = rest[consumed..]
        .rsplit_once(" = ")
        .expect("syscall result")
        .0;
    // Read the flags after the path; a filename containing O_WRONLY is still
    // an input. Exclusive creation cannot open an existing input.
    if call.starts_with("open")
        && (flags.contains("O_WRONLY") || (flags.contains("O_CREAT") && flags.contains("O_EXCL")))
    {
        return vec![];
    }
    let raw = base.join(name);
    let requested = normalize(&raw);
    let mut found = vec![own_proc(raw.clone(), pid)];
    if requested != raw {
        found.push(own_proc(requested, pid));
    }
    if let Some(path) = line
        .rsplit_once(" = ")
        .and_then(|(_, result)| annotation(result))
    {
        found.push(own_proc(PathBuf::from(path), pid));
    } else if !raw.starts_with("/proc")
        && let Ok(path) = raw.canonicalize()
    {
        found.push(own_proc(path, pid));
    }
    found
}

pub struct Trace {
    pub process: Output,
    pub reads: BTreeSet<PathBuf>,
    links: BTreeSet<PathBuf>,
}

impl Trace {
    pub fn offenders(
        &self,
        runtime: &BTreeSet<PathBuf>,
        rom: &Path,
        output: &Path,
        binary: &Path,
    ) -> BTreeSet<PathBuf> {
        let mut bad = offenders(&self.reads, runtime, rom, output, binary);
        // Runtime/ROM permissions apply to reads, never to output links.
        // Otherwise a link to an allowed runtime directory could hide a
        // missing descendant probe outside that exact runtime allowance.
        bad.extend(
            self.links
                .iter()
                .filter(|p| {
                    !p.starts_with(output) || p.components().any(|c| c == Component::ParentDir)
                })
                .cloned(),
        );
        bad
    }
}

pub fn trace(
    binary: &Path,
    args: &[&Path],
    cwd: &Path,
    log: &Path,
    output: Option<&Path>,
) -> Trace {
    let mut reads = BTreeSet::new();
    let mut links = BTreeSet::new();
    if let Some(output) = output {
        output_links(output, &mut links);
    }
    let process = Command::new("strace").current_dir(cwd)
        .env_remove("LD_PRELOAD").env_remove("LD_LIBRARY_PATH")
        .args(["-ff", "-qq", "-yy", "-s", "0", "-e",
            "trace=open,openat,openat2,access,faccessat,faccessat2,stat,lstat,newfstatat,statx,readlink,readlinkat,chdir,fchdir,execve,execveat,symlink,symlinkat,rename,renameat,renameat2,link,linkat,unlink,unlinkat,rmdir", "-o"])
        .arg(log).arg(binary).args(args).output().expect("strace is required on Linux");
    let prefix = format!("{}.", log.file_name().unwrap().to_str().unwrap());
    let entries: Vec<_> = std::fs::read_dir(log.parent().unwrap())
        .unwrap()
        .map(Result::unwrap)
        .filter(|e| e.file_name().to_str().unwrap().starts_with(&prefix))
        .collect();
    let main_pid = entries
        .iter()
        .find(|e| {
            std::fs::read_to_string(e.path())
                .unwrap()
                .starts_with("execve(")
        })
        .expect("trace did not record execution")
        .file_name()
        .into_string()
        .unwrap();
    let main_pid = main_pid.strip_prefix(&prefix).unwrap();
    for entry in entries {
        for line in std::fs::read_to_string(entry.path()).unwrap().lines() {
            if matches!(
                line.split_once('(').map(|(call, _)| call),
                Some(
                    "symlink"
                        | "symlinkat"
                        | "rename"
                        | "renameat"
                        | "renameat2"
                        | "link"
                        | "linkat"
                )
            ) {
                links.extend(paths(line, cwd, main_pid));
            } else {
                reads.extend(paths(line, cwd, main_pid));
            }
        }
    }
    assert!(
        !reads.is_empty(),
        "strace recorded no paths: {}",
        String::from_utf8_lossy(&process.stderr)
    );
    Trace {
        process,
        reads,
        links,
    }
}

pub fn offenders(
    reads: &BTreeSet<PathBuf>,
    runtime: &BTreeSet<PathBuf>,
    rom: &Path,
    output: &Path,
    binary: &Path,
) -> BTreeSet<PathBuf> {
    reads
        .iter()
        .filter(|p| {
            *p != rom
                && *p != binary
                && !runtime.contains(*p)
                && (!p.starts_with(output) || p.components().any(|c| c == Component::ParentDir))
        })
        .cloned()
        .collect()
}

#[test]
fn trace_paths_resolve_dirfds_missing_probes_and_escaped_names() {
    let cwd = Path::new("/tree");
    assert_eq!(
        paths(
            r#"openat(4</out/stage>, "a\040b", O_RDONLY) = 5</outside/table>"#,
            cwd,
            "12"
        ),
        [
            PathBuf::from("/out/stage/a b"),
            PathBuf::from("/outside/table")
        ]
    );
    assert_eq!(
        paths(
            r#"statx(AT_FDCWD</tree>, "hidden/../missing", 0, 0, 0) = -1 ENOENT"#,
            cwd,
            "12"
        ),
        [
            PathBuf::from("/tree/hidden/../missing"),
            PathBuf::from("/tree/missing")
        ]
    );
    assert_eq!(
        paths(
            r#"statx(3</proc/12/maps>, "", AT_EMPTY_PATH, 0, 0) = 0"#,
            cwd,
            "12"
        ),
        [PathBuf::from("/proc/self/maps")]
    );
    assert!(
        paths(
            r#"openat(AT_FDCWD</tree>, "out", O_WRONLY|O_CREAT) = 3</tree/out>"#,
            cwd,
            "12"
        )
        .is_empty()
    );
    let reads = [
        "/rom",
        "/out/pack",
        "/out-sibling/hidden",
        "/tree/docs/README.md",
        "/bin/pack",
    ]
    .map(PathBuf::from)
    .into();
    assert_eq!(
        offenders(
            &reads,
            &BTreeSet::new(),
            Path::new("/rom"),
            Path::new("/out"),
            Path::new("/bin/pack")
        ),
        [
            PathBuf::from("/out-sibling/hidden"),
            PathBuf::from("/tree/docs/README.md")
        ]
        .into()
    );
}

pub fn retail_bytes(path: &Path) -> Option<Vec<u8>> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("local retail ROM absent: Rust pack input audit skipped");
            return None;
        }
        Err(e) => panic!("local ROM is unusable for the input audit: {e}"),
    };
    psiv_pack::Rom::verify(path, bytes.clone()).unwrap();
    Some(bytes)
}

#[test]
fn input_audit_skips_only_absent_roms_and_keeps_flag_names_in_paths() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../build/input-rom-kind-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    assert!(retail_bytes(&root.join("missing")).is_none());
    let wrong = root.join("wrong");
    std::fs::write(&wrong, [0; 16]).unwrap();
    assert!(std::panic::catch_unwind(|| retail_bytes(&wrong)).is_err());
    assert!(std::panic::catch_unwind(|| retail_bytes(&root)).is_err());
    let reads = paths(
        r#"openat(AT_FDCWD</tree>, "O_WRONLY-table", O_RDONLY) = 3</tree/O_WRONLY-table>"#,
        Path::new("/tree"),
        "12",
    );
    assert!(reads.contains(&PathBuf::from("/tree/O_WRONLY-table")));
    assert_eq!(
        paths(
            r#"access("/missing", R_OK)  = -1 ENOENT (No such file or directory)"#,
            Path::new("/tree"),
            "12"
        ),
        [PathBuf::from("/missing")]
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn link_dependencies_survive_removal_and_parent_traversal_is_not_excused() {
    let cwd = Path::new("/tree");
    // Capture the target from creation, without consulting a link that may
    // already have disappeared. Failed creation is also an attempted dependency.
    for line in [
        r#"symlink("/external/missing", "/out/alias") = 0"#,
        r#"symlinkat("/external/missing", 4</out/stage>, "alias") = 0"#,
    ] {
        assert_eq!(paths(line, cwd, "12"), [PathBuf::from("/external/missing")]);
    }
    assert_eq!(
        paths(
            r#"renameat2(4</out>, "stage", 4</out>, "renamed", 0) = 0"#,
            cwd,
            "12"
        ),
        [PathBuf::from("/out/stage"), PathBuf::from("/out/renamed")]
    );
    let parent = PathBuf::from("/out/branch/../outside");
    assert_eq!(
        offenders(
            &[parent.clone()].into(),
            &BTreeSet::new(),
            Path::new("/rom"),
            Path::new("/out"),
            Path::new("/bin/pack")
        ),
        [parent].into()
    );
}
