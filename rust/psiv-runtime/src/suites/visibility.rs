//! The boundary as a check: no `Runtime` mutator is public.
//!
//! Node S6's rule — a game is a [`Session`](crate::Session), and everything
//! that changes a runtime is crate-private — is what the two `compile_fail`
//! doctests on `Runtime` prove for a *caller*. This proves it for the *source*:
//! a method that takes `&mut self` and is declared `pub fn` inside an
//! `impl Runtime` block is a hole in that boundary whether or not a caller uses
//! it today, so a lane that adds one fails here with the method's name and
//! file.
//!
//! The scan reads the crate's own sources (`CARGO_MANIFEST_DIR`), the way
//! `psiv-campaign`'s `the_runner_reaches_no_runtime_mutator` reads the
//! runner's: the rule is about the shape of the tree, and the tree is the
//! input.

use std::path::{Path, PathBuf};

#[test]
fn no_runtime_mutator_is_public() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect(&root, &mut files);
    assert!(
        files.len() > 40,
        "the crate's sources were found: {files:?}"
    );
    let public: Vec<String> = files
        .iter()
        .flat_map(|path| public_mutators(path))
        .collect();
    assert!(
        public.is_empty(),
        "these Runtime methods take `&mut self` and are `pub`:\n  {}",
        public.join("\n  ")
    );
}

/// Every `.rs` file under `dir`, recursively.
fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("the crate's source directory") {
        let path = entry.expect("a source entry").path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// The `pub fn` methods that take `&mut self` inside an `impl Runtime` block of
/// `path`, as `file:line name`.
fn public_mutators(path: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(path).expect("a readable source file");
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    let mut receiver: Option<String> = None;
    let mut depth = 0usize;
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if depth == 0
            && let Some(rest) = trimmed.strip_prefix("impl")
            && let Some(name) = rest
                .trim_start()
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .next()
        {
            receiver = Some(name.to_owned());
        }
        if receiver.as_deref() == Some("Runtime")
            && let Some(name) = trimmed.strip_prefix("pub fn ")
            && signature(&lines, index).contains("&mut self")
        {
            let name = name
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .next()
                .unwrap_or(name);
            out.push(format!("{}:{} {name}", path.display(), index + 1));
        }
        depth = depth + line.matches('{').count() - line.matches('}').count();
    }
    out
}

/// The `fn` signature that starts on `line`, up to its argument list's close,
/// so a `&mut self` on a continuation line still counts.
fn signature(lines: &[&str], line: usize) -> String {
    let mut text = String::new();
    for line in lines.iter().skip(line) {
        text.push_str(line);
        if text.matches('(').count() <= text.matches(')').count() {
            break;
        }
    }
    text
}
