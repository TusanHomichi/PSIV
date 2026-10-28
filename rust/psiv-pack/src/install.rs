//! Install a pack atomically: build beside it, then swap it in.
//!
//! The port of `psiv_tools/pack_swap.py`, which owns the design and its
//! reasons; the two use the same names, so either builder cleans up after,
//! and never deletes the work of, the other. In brief:
//!
//! 1. The pack is built into `.<name>.build-<pid>-<hex>` beside `<output>`.
//! 2. The finished directory is renamed to the version
//!    `.<name>.v<UTC stamp>-<pid>`, never written again.
//! 3. On Linux and macOS `<output>` becomes a symbolic link to it in one
//!    rename; a real directory there is exchanged with the new link in one
//!    `renameat2(RENAME_EXCHANGE)` (Linux) or `renamex_np(RENAME_SWAP)`
//!    (macOS) and kept as a version. The newest [`KEEP_VERSIONS`] are kept.
//!    On Windows, where an ordinary user cannot make a symbolic link,
//!    `<output>` stays a real directory, replaced by two renames, and no
//!    older version is kept.
//!
//! A failed or killed build never touches `<output>`. The next build removes
//! a `.build-` or `.link-` entry whose process is gone, and pruning never
//! removes the version `<output>` names, nor one a live process made and has
//! yet to install.

use std::hash::{BuildHasher as _, Hasher as _};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::PackError;

/// Versions kept after a swap: the current one and the two before it.
pub const KEEP_VERSIONS: usize = 3;

/// Whether `<output>` is a symbolic link to its version, or (Windows) the
/// real directory.
pub const LINKS: bool = cfg!(unix);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Build,
    Version,
    Link,
}

/// What `entry` is to `output`'s install, and the process id in its name.
fn classify(output_name: &str, entry: &str) -> Option<(Kind, u32)> {
    let rest = entry.strip_prefix('.')?.strip_prefix(output_name)?;
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let hexits =
        |s: &str| !s.is_empty() && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    for (prefix, kind) in [(".build-", Kind::Build), (".link-", Kind::Link)] {
        if let Some(tail) = rest.strip_prefix(prefix) {
            let (pid, token) = tail.split_once('-')?;
            return (digits(pid) && hexits(token)).then(|| Some((kind, pid.parse().ok()?)))?;
        }
    }
    let tail = rest.strip_prefix(".v")?;
    let (stamp, pid) = tail.split_once("Z-")?;
    let (date, time) = stamp.split_once('T')?;
    (date.len() == 8 && digits(date) && time.len() == 12 && digits(time) && digits(pid))
        .then(|| Some((Kind::Version, pid.parse().ok()?)))?
}

fn io_error(what: &str, path: &Path, error: io::Error) -> PackError {
    PackError(format!("cannot {what} {}: {error}", path.display()))
}

fn name_of(output: &Path) -> Result<String, PackError> {
    output
        .file_name()
        .and_then(|n| n.to_str())
        .map(str::to_owned)
        .ok_or_else(|| PackError(format!("{} has no usable file name", output.display())))
}

fn entries(parent: &Path) -> Result<Vec<PathBuf>, PackError> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(parent).map_err(|e| io_error("list", parent, e))? {
        found.push(entry.map_err(|e| io_error("list", parent, e))?.path());
    }
    found.sort();
    Ok(found)
}

fn is_symlink(path: &Path) -> bool {
    path.symlink_metadata()
        .is_ok_and(|m| m.file_type().is_symlink())
}

fn lexists(path: &Path) -> bool {
    path.symlink_metadata().is_ok()
}

fn remove(path: &Path) -> Result<(), PackError> {
    let result = if is_symlink(path) || !path.is_dir() {
        remove_link_or_file(path)
    } else {
        std::fs::remove_dir_all(path)
    };
    result.map_err(|e| io_error("remove", path, e))
}

fn remove_link_or_file(path: &Path) -> io::Result<()> {
    // A directory link on Windows is removed as a directory.
    std::fs::remove_file(path).or_else(|e| {
        if cfg!(windows) && is_symlink(path) {
            std::fs::remove_dir(path)
        } else {
            Err(e)
        }
    })
}

/// A UTC stamp as `%Y%m%dT%H%M%S%fZ` writes it.
fn stamp_of(time: SystemTime) -> String {
    let since = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = since.as_secs() as i64;
    let (days, of_day) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}{month:02}{day:02}T{:02}{:02}{:02}{:06}Z",
        of_day / 3600,
        of_day % 3600 / 60,
        of_day % 60,
        since.subsec_micros()
    )
}

fn stamp() -> String {
    stamp_of(SystemTime::now())
}

/// Eight hex digits, unique enough to keep two builds in one process apart.
fn token() -> String {
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u128(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    );
    format!("{}-{:08x}", std::process::id(), hasher.finish() as u32)
}

/// True while process `pid` exists, whoever owns it.
pub fn alive(pid: u32) -> bool {
    if pid == std::process::id() {
        return true;
    }
    alive_os(pid)
}

#[cfg(unix)]
#[allow(unsafe_code)]
fn alive_os(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    // SAFETY: kill with signal 0 only checks that the process exists.
    if unsafe { libc::kill(pid, 0) } == 0 {
        return true;
    }
    // EPERM: another user's process.
    io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn alive_os(pid: u32) -> bool {
    use std::ffi::c_void;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const STILL_ACTIVE: u32 = 259;
    const ERROR_ACCESS_DENIED: i32 = 5;
    unsafe extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
        fn GetExitCodeProcess(process: *mut c_void, code: *mut u32) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
    }
    // SAFETY: plain Win32 calls on a handle this function owns and closes.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            // Refused means it exists; any other error means it is gone.
            return io::Error::last_os_error().raw_os_error() == Some(ERROR_ACCESS_DENIED);
        }
        let mut code = 0u32;
        let running = GetExitCodeProcess(handle, &mut code) == 0 || code == STILL_ACTIVE;
        CloseHandle(handle);
        running
    }
}

/// Remove what a build of `output` killed part-way left beside it; returns
/// the names. A real directory under a `.link-` name is the old pack between
/// the exchange and its rename: it becomes a version stamped by its mtime.
pub fn remove_abandoned(output: &Path) -> Result<Vec<String>, PackError> {
    let name = name_of(output)?;
    let parent = output.parent().unwrap_or(Path::new("."));
    let mut removed = Vec::new();
    for entry in entries(parent)? {
        let Some(entry_name) = entry
            .file_name()
            .and_then(|n| n.to_str())
            .map(str::to_owned)
        else {
            continue;
        };
        let Some((kind, pid)) = classify(&name, &entry_name) else {
            continue;
        };
        if kind == Kind::Version || alive(pid) {
            continue;
        }
        if kind == Kind::Link && entry.is_dir() && !is_symlink(&entry) {
            let mtime = entry
                .metadata()
                .and_then(|m| m.modified())
                .map_err(|e| io_error("read", &entry, e))?;
            let kept = parent.join(format!(".{name}.v{}-{pid}", stamp_of(mtime)));
            std::fs::rename(&entry, &kept).map_err(|e| io_error("rename", &entry, e))?;
        } else {
            remove(&entry)?;
        }
        removed.push(entry_name);
    }
    Ok(removed)
}

/// `output`'s installed versions beside it, oldest first.
pub fn versions(output: &Path) -> Result<Vec<PathBuf>, PackError> {
    let name = name_of(output)?;
    let parent = output.parent().unwrap_or(Path::new("."));
    Ok(entries(parent)?
        .into_iter()
        .filter(|entry| {
            entry
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| classify(&name, n))
                .is_some_and(|(kind, _)| kind == Kind::Version)
        })
        .collect())
}

/// Remove all but the newest `keep` versions of `output`, never the one
/// `output` names nor one another live process made; returns their names.
pub fn prune(output: &Path, keep: usize) -> Result<Vec<String>, PackError> {
    prune_with(output, keep, alive)
}

/// [`prune`] with the liveness check given, so a test can stand in for a
/// concurrent build.
fn prune_with(
    output: &Path,
    keep: usize,
    alive: impl Fn(u32) -> bool,
) -> Result<Vec<String>, PackError> {
    let name = name_of(output)?;
    let installed = versions(output)?;
    let mut removed = Vec::new();
    for path in &installed[..installed.len().saturating_sub(keep)] {
        let entry_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_owned();
        let pid = classify(&name, &entry_name).map_or(0, |(_, pid)| pid);
        if pid != std::process::id() && alive(pid) {
            continue; // another build's version, perhaps not yet installed
        }
        // Read after the liveness check: only a version's own build installs
        // it, so once that build is gone, `output` already names it if it
        // ever will. A snapshot from before the loop could be stale by now.
        let current = std::fs::canonicalize(output).ok();
        if std::fs::canonicalize(path).ok() == current {
            continue;
        }
        std::fs::remove_dir_all(path).map_err(|e| io_error("remove", path, e))?;
        removed.push(entry_name);
    }
    Ok(removed)
}

/// Removes the staging directory unless the build reached its version.
struct Staging<'a>(Option<&'a Path>);

impl Drop for Staging<'_> {
    fn drop(&mut self) {
        if let Some(path) = self.0
            && path.exists()
        {
            let _ = std::fs::remove_dir_all(path);
        }
    }
}

/// What an install returns: the build's value, and what went wrong in the
/// cleanup after `output` already named the new pack (keeping or removing
/// the old one, pruning). Those are warnings, not failures: the pack is
/// installed, and the next build removes or keeps what was left.
#[derive(Debug)]
pub struct Installed<T> {
    pub value: T,
    pub warnings: Vec<String>,
}

/// Run `build(staging)` into a new sibling directory of `output`, then make
/// `output` name it in one atomic step. An error means `output` is
/// untouched: on any failure before that step, a panic included, the
/// staging directory is removed. After it, nothing is an error
/// ([`Installed::warnings`]).
pub fn install_atomically<T>(
    output: &Path,
    build: impl FnOnce(&Path) -> Result<T, PackError>,
) -> Result<Installed<T>, PackError> {
    let output = std::path::absolute(output).map_err(|e| io_error("resolve", output, e))?;
    let name = name_of(&output)?;
    let parent = output
        .parent()
        .expect("an absolute path with a file name has a parent");
    std::fs::create_dir_all(parent).map_err(|e| io_error("create", parent, e))?;
    if output.exists() && !output.is_dir() {
        return Err(PackError(format!(
            "{} exists and is not a directory",
            output.display()
        )));
    }
    remove_abandoned(&output)?;
    // Taken first, so a real directory replaced below sorts before the new
    // version (`versions` orders by name).
    let before = stamp();
    let token = token();
    let pid = std::process::id();
    let staging = parent.join(format!(".{name}.build-{token}"));
    std::fs::create_dir(&staging).map_err(|e| io_error("create", &staging, e))?;
    let mut guard = Staging(Some(&staging));
    let result = build(&staging)?;
    let version = parent.join(format!(".{name}.v{}-{pid}", stamp()));
    std::fs::rename(&staging, &version).map_err(|e| io_error("rename", &staging, e))?;
    guard.0 = None;
    let link = parent.join(format!(".{name}.link-{token}"));
    // The commit point is inside each: an error is returned only before it.
    let mut warnings = if LINKS {
        link_to(
            &output,
            &version,
            &link,
            &parent.join(format!(".{name}.v{before}-{pid}")),
        )?
    } else {
        replace_directory(&output, &version, &link)?
    };
    if let Err(error) = prune(&output, if LINKS { KEEP_VERSIONS } else { 0 }) {
        warnings.push(format!(
            "installed, but pruning old versions failed: {error}"
        ));
    }
    Ok(Installed {
        value: result,
        warnings,
    })
}

/// Make `output` a link to `version` through the new link `link`; a real
/// directory at `output` is kept as the version `kept`. Committed once the
/// exchange or the rename succeeds; keeping the old directory after that
/// can only warn (it stays under the `.link-` name, which the next build
/// turns into a version).
#[cfg(unix)]
fn link_to(
    output: &Path,
    version: &Path,
    link: &Path,
    kept: &Path,
) -> Result<Vec<String>, PackError> {
    let target = version.file_name().expect("a version has a name");
    std::os::unix::fs::symlink(target, link).map_err(|e| io_error("link", link, e))?;
    let committed = if output.is_dir() && !is_symlink(output) {
        exchange(link, output).map(|()| true)
    } else {
        std::fs::rename(link, output)
            .map(|()| false)
            .map_err(|e| io_error("replace", output, e))
    };
    match committed {
        Err(error) => {
            if is_symlink(link) {
                let _ = std::fs::remove_file(link);
            }
            Err(error)
        }
        // `link` is the old real directory now.
        Ok(true) => Ok(match std::fs::rename(link, kept) {
            Ok(()) => Vec::new(),
            Err(e) => vec![format!(
                "installed, but the old pack stays at {}: {e}",
                link.display()
            )],
        }),
        Ok(false) => Ok(Vec::new()),
    }
}

#[cfg(not(unix))]
fn link_to(_: &Path, _: &Path, _: &Path, _: &Path) -> Result<Vec<String>, PackError> {
    unreachable!("LINKS is false off unix")
}

#[cfg(target_os = "linux")]
#[allow(unsafe_code)]
fn exchange(first: &Path, second: &Path) -> Result<(), PackError> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt as _;
    let a = CString::new(first.as_os_str().as_bytes()).expect("a path without NUL");
    let b = CString::new(second.as_os_str().as_bytes()).expect("a path without NUL");
    // SAFETY: two NUL-terminated paths that outlive the call.
    let status = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            libc::AT_FDCWD,
            a.as_ptr(),
            libc::AT_FDCWD,
            b.as_ptr(),
            libc::RENAME_EXCHANGE,
        )
    };
    if status != 0 {
        return Err(PackError(format!(
            "cannot exchange {} and {}: {}",
            first.display(),
            second.display(),
            io::Error::last_os_error()
        )));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
fn exchange(first: &Path, second: &Path) -> Result<(), PackError> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt as _;
    let a = CString::new(first.as_os_str().as_bytes()).expect("a path without NUL");
    let b = CString::new(second.as_os_str().as_bytes()).expect("a path without NUL");
    // SAFETY: two NUL-terminated paths that outlive the call.
    if unsafe { libc::renamex_np(a.as_ptr(), b.as_ptr(), libc::RENAME_SWAP) } != 0 {
        return Err(PackError(format!(
            "cannot exchange {} and {}: {}",
            first.display(),
            second.display(),
            io::Error::last_os_error()
        )));
    }
    Ok(())
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
fn exchange(_: &Path, second: &Path) -> Result<(), PackError> {
    Err(PackError(format!(
        "this system has no atomic exchange; cannot swap a real directory at {}",
        second.display()
    )))
}

/// Rename `version` to `output`, moving a directory already there to `old`
/// first and removing it after.
fn replace_directory(output: &Path, version: &Path, old: &Path) -> Result<Vec<String>, PackError> {
    if lexists(output) {
        std::fs::rename(output, old).map_err(|e| io_error("rename", output, e))?;
    }
    if let Err(error) = std::fs::rename(version, output) {
        if lexists(old) {
            let _ = std::fs::rename(old, output);
        }
        return Err(io_error("rename", version, error));
    }
    // Committed: removing the old directory can only warn.
    if lexists(old)
        && let Err(error) = remove(old)
    {
        return Ok(vec![format!("installed, but {error}")]);
    }
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::scratch;

    fn names(dir: &Path) -> Vec<String> {
        let mut found: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        found.sort();
        found
    }

    fn write_marker(dir: &Path, text: &str) -> Result<(), PackError> {
        std::fs::write(dir.join("marker"), text).map_err(|e| io_error("write", dir, e))
    }

    #[test]
    fn names_match_the_python_patterns() {
        assert_eq!(classify("p", ".p.build-12-0af3"), Some((Kind::Build, 12)));
        assert_eq!(classify("p", ".p.link-7-ff"), Some((Kind::Link, 7)));
        assert_eq!(
            classify("p", ".p.v20261028T120000123456Z-99"),
            Some((Kind::Version, 99))
        );
        for other in [
            ".p.build-12-0AF3",
            ".p.build-12-",
            ".p.build--ab",
            ".p.v20261028T1200001234Z-99",
            ".p.v20261028T120000123456Z-",
            ".q.v20261028T120000123456Z-1",
            "p",
            ".pp.build-1-a",
        ] {
            assert_eq!(classify("p", other), None, "{other}");
        }
    }

    #[test]
    fn stamp_is_utc_with_microseconds() {
        let when = UNIX_EPOCH + std::time::Duration::from_micros(1_761_652_800_000_123);
        assert_eq!(stamp_of(when), "20251028T120000000123Z");
        assert_eq!(stamp_of(UNIX_EPOCH), "19700101T000000000000Z");
        let name = format!(".p.v{}-1", stamp());
        assert_eq!(classify("p", &name), Some((Kind::Version, 1)));
    }

    #[test]
    fn installs_and_keeps_three_versions() {
        let root = scratch("install");
        let output = root.join("pack");
        for i in 0..5 {
            install_atomically(&output, |dir| write_marker(dir, &i.to_string())).unwrap();
            assert_eq!(
                std::fs::read_to_string(output.join("marker")).unwrap(),
                i.to_string()
            );
        }
        if LINKS {
            assert!(is_symlink(&output));
            assert_eq!(versions(&output).unwrap().len(), KEEP_VERSIONS);
        } else {
            assert!(versions(&output).unwrap().is_empty());
        }
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn failure_leaves_output_and_no_staging() {
        let root = scratch("failure");
        let output = root.join("pack");
        install_atomically(&output, |dir| write_marker(dir, "old")).unwrap();
        let before = names(&root);
        let error = install_atomically(&output, |dir| -> Result<(), PackError> {
            write_marker(dir, "new")?;
            Err(PackError("boom".into()))
        })
        .unwrap_err();
        assert_eq!(error.0, "boom");
        assert_eq!(names(&root), before);
        assert_eq!(
            std::fs::read_to_string(output.join("marker")).unwrap(),
            "old"
        );
        let panicked = std::panic::catch_unwind(|| {
            install_atomically(&output, |dir| -> Result<(), PackError> {
                write_marker(dir, "new")?;
                panic!("a section bug")
            })
        });
        assert!(panicked.is_err());
        assert_eq!(names(&root), before);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn real_directory_is_exchanged_and_kept() {
        let root = scratch("exchange");
        let output = root.join("pack");
        std::fs::create_dir_all(&output).unwrap();
        std::fs::write(output.join("marker"), "real").unwrap();
        install_atomically(&output, |dir| write_marker(dir, "new")).unwrap();
        assert!(is_symlink(&output));
        let kept = versions(&output).unwrap();
        assert_eq!(kept.len(), 2);
        assert_eq!(
            std::fs::read_to_string(kept[0].join("marker")).unwrap(),
            "real"
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn abandoned_entries_of_dead_processes_go() {
        let root = scratch("abandoned");
        let output = root.join("pack");
        install_atomically(&output, |dir| write_marker(dir, "1")).unwrap();
        // No process has this id: Linux caps pids at 2^22, macOS at 99,999,
        // and a Windows id is a multiple of 4.
        let dead = 4_194_305u32;
        assert!(!alive(dead));
        let me = std::process::id();
        std::fs::create_dir(root.join(format!(".pack.build-{dead}-ab"))).unwrap();
        std::fs::create_dir(root.join(format!(".pack.build-{me}-cd"))).unwrap();
        std::fs::create_dir(root.join(format!(".pack.link-{dead}-ef"))).unwrap();
        std::fs::write(root.join(format!(".pack.link-{dead}-01")), "").unwrap();
        let mut removed = remove_abandoned(&output).unwrap();
        removed.sort();
        assert_eq!(
            removed,
            vec![
                format!(".pack.build-{dead}-ab"),
                format!(".pack.link-{dead}-01"),
                format!(".pack.link-{dead}-ef"),
            ]
        );
        assert!(root.join(format!(".pack.build-{me}-cd")).exists());
        // The real directory under a link name became a version, not deleted.
        let expected = if LINKS { 2 } else { 1 };
        assert_eq!(versions(&output).unwrap().len(), expected);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn prune_spares_a_live_process_pending_version() {
        let root = scratch("prune");
        let output = root.join("pack");
        // pid 1 is alive on every unix; on Windows the System process is 4.
        let live = if cfg!(windows) { 4 } else { 1 };
        std::fs::create_dir_all(root.join(format!(".pack.v20000101T000000000000Z-{live}")))
            .unwrap();
        for _ in 0..4 {
            install_atomically(&output, |dir| write_marker(dir, "x")).unwrap();
        }
        assert!(
            root.join(format!(".pack.v20000101T000000000000Z-{live}"))
                .exists()
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn prune_rereads_the_current_version_after_a_build_ends() {
        // Another build made an old version, paused, then installed it and
        // exited while this prune ran: pruning must not delete what
        // `output` names by then.
        let root = scratch("race");
        let output = root.join("pack");
        let other = 4_194_305u32;
        for _ in 0..KEEP_VERSIONS {
            install_atomically(&output, |dir| write_marker(dir, "x")).unwrap();
        }
        // Oldest by name, so the next prune considers it.
        let late = root.join(format!(".pack.v20000101T000000000000Z-{other}"));
        std::fs::create_dir_all(&late).unwrap();
        let installs_then_exits = |pid: u32| {
            if pid == other && LINKS {
                let link = root.join(".pack.link-1-ab");
                #[cfg(unix)]
                std::os::unix::fs::symlink(late.file_name().unwrap(), &link).unwrap();
                std::fs::rename(&link, &output).unwrap();
            }
            false
        };
        prune_with(&output, KEEP_VERSIONS, installs_then_exits).unwrap();
        if LINKS {
            assert!(late.exists(), "the newly current version survives");
            assert_eq!(
                std::fs::canonicalize(&output).unwrap(),
                std::fs::canonicalize(&late).unwrap()
            );
        }
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_after_the_commit_point_warns() {
        use std::os::unix::fs::PermissionsExt as _;
        let root = scratch("commit");
        let output = root.join("pack");
        install_atomically(&output, |dir| write_marker(dir, "1")).unwrap();
        // An old version the pruner cannot empty: a read-only directory
        // holding a file.
        let stuck = root.join(".pack.v20000101T000000000000Z-1");
        std::fs::create_dir_all(stuck.join("locked")).unwrap();
        std::fs::write(stuck.join("locked").join("f"), "").unwrap();
        std::fs::set_permissions(stuck.join("locked"), std::fs::Permissions::from_mode(0o555))
            .unwrap();
        // pid 1 is alive, so rename the version to a dead build's.
        let dead = root.join(".pack.v20000101T000000000000Z-4194305");
        std::fs::rename(&stuck, &dead).unwrap();
        let mut warned = Vec::new();
        for i in 0..KEEP_VERSIONS {
            let installed =
                install_atomically(&output, |dir| write_marker(dir, &i.to_string())).unwrap();
            warned.extend(installed.warnings);
        }
        let privileged = std::fs::remove_dir_all(dead.join("locked")).is_ok() && warned.is_empty();
        if !privileged {
            assert!(
                warned
                    .iter()
                    .any(|w| w.starts_with("installed, but pruning")),
                "{warned:?}"
            );
            assert_eq!(
                std::fs::read_to_string(output.join("marker")).unwrap(),
                (KEEP_VERSIONS - 1).to_string()
            );
        }
        let _ =
            std::fs::set_permissions(dead.join("locked"), std::fs::Permissions::from_mode(0o755));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_file_at_output_is_refused() {
        let root = scratch("file");
        std::fs::create_dir_all(&root).unwrap();
        let output = root.join("pack");
        std::fs::write(&output, "").unwrap();
        let error = install_atomically(&output, |_| Ok(())).unwrap_err();
        assert!(
            error.0.ends_with("exists and is not a directory"),
            "{}",
            error.0
        );
        std::fs::remove_dir_all(&root).unwrap();
    }
}
