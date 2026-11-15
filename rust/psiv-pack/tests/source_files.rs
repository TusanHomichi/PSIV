//! Every Rust source file in the workspace is compiled: each `.rs` file under
//! a crate's `src/`, `tests/`, `examples/` and `benches/` is reached from one
//! of the crate's roots through `mod` declarations (#549).
//!
//! Cargo says nothing about a file no `mod` names. A test module whose
//! declaration is missing, or lost in a merge, passes by never running. The
//! pack retirement (#115) moves the Python pack checks into new test files
//! across psiv-pack, so this holds each of them, and every other module, to
//! actually being built. It lives with psiv-pack's tests for that reason, and
//! walks every crate.
//!
//! A `mod` named only in a comment or a string does not count. A file reached
//! only through an inline `mod name { mod child; }` or a `#[path]` is resolved
//! as rustc resolves it; `include!` is not followed (the workspace has none).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Blank out comments and string and character literals, keeping every byte
/// offset, so a `mod` inside one is not read and an attribute's string can
/// still be read from the original text at the same offset.
fn code_only(text: &str) -> Vec<u8> {
    let src = text.as_bytes();
    let mut out = src.to_vec();
    let blank = |out: &mut Vec<u8>, from: usize, to: usize| {
        for byte in &mut out[from..to.min(src.len())] {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
    };
    let mut i = 0;
    while i < src.len() {
        let rest = &src[i..];
        if rest.starts_with(b"//") {
            let end = rest
                .iter()
                .position(|&b| b == b'\n')
                .map_or(src.len(), |n| i + n);
            blank(&mut out, i, end);
            i = end;
        } else if rest.starts_with(b"/*") {
            let (mut depth, mut j) = (0usize, i);
            while j < src.len() {
                if src[j..].starts_with(b"/*") {
                    depth += 1;
                    j += 2;
                } else if src[j..].starts_with(b"*/") {
                    depth -= 1;
                    j += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    j += 1;
                }
            }
            blank(&mut out, i, j);
            i = j;
        } else if let Some(hashes) = raw_string_start(rest) {
            let open = i + hashes + 2 + usize::from(rest[0] == b'b');
            let close: Vec<u8> = std::iter::once(b'"')
                .chain(std::iter::repeat_n(b'#', hashes))
                .collect();
            let end = src[open..]
                .windows(close.len())
                .position(|w| w == close)
                .map_or(src.len(), |n| open + n + close.len());
            blank(&mut out, i, end);
            i = end;
        } else if rest[0] == b'"' {
            let mut j = i + 1;
            while j < src.len() && src[j] != b'"' {
                j += if src[j] == b'\\' { 2 } else { 1 };
            }
            blank(&mut out, i + 1, j);
            i = j + 1;
        } else if rest[0] == b'\'' {
            // A character literal, or a lifetime, which is left alone.
            if rest.get(1) == Some(&b'\\') {
                let end = rest[2..]
                    .iter()
                    .position(|&b| b == b'\'')
                    .map_or(src.len(), |n| i + n + 3);
                blank(&mut out, i, end);
                i = end;
            } else if let Some(width) = text[i + 1..]
                .chars()
                .next()
                .map(char::len_utf8)
                .filter(|&w| rest.get(1 + w) == Some(&b'\''))
            {
                blank(&mut out, i, i + width + 2);
                i += width + 2;
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    out
}

/// `r"`, `r#"`, `br##"` and so on: the number of `#`s.
fn raw_string_start(rest: &[u8]) -> Option<usize> {
    let after = rest.strip_prefix(b"b").unwrap_or(rest).strip_prefix(b"r")?;
    let hashes = after.iter().take_while(|&&b| b == b'#').count();
    (after.get(hashes) == Some(&b'"')).then_some(hashes)
}

fn is_ident(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// The `#[path = "..."]` directly before offset `at`, if any.
fn path_attribute(code: &[u8], text: &str, at: usize) -> Option<String> {
    let before = std::str::from_utf8(&code[..at]).ok()?;
    let start = before.rfind("#[path")?;
    let between = &before[start..];
    let close = between.find(']')?;
    // Only other attributes and a visibility may stand between it and this
    // `mod`: an item boundary means it belonged to an earlier one.
    if between[close + 1..].contains([';', '{', '}']) {
        return None;
    }
    let attribute = &text[start..start + close];
    let open = attribute.find('"')?;
    let end = attribute[open + 1..].find('"')?;
    Some(attribute[open + 1..open + 1 + end].to_owned())
}

/// The files `file` declares as modules. `owns_dir` is whether its children
/// live beside it (a crate root, a `mod.rs`, a `#[path]` file) rather than in
/// a directory named after it.
fn declared(file: &Path, owns_dir: bool) -> Vec<(PathBuf, bool)> {
    let text = std::fs::read_to_string(file).unwrap();
    let code = code_only(&text);
    let here = file.parent().unwrap();
    let base = if owns_dir {
        here.to_path_buf()
    } else {
        here.join(file.file_stem().unwrap())
    };
    let mut found = Vec::new();
    // Inline modules open a directory level: (name, brace depth inside it).
    let mut inline: Vec<(String, usize)> = Vec::new();
    let mut depth = 0usize;
    let mut i = 0;
    while i < code.len() {
        match code[i] {
            b'{' => depth += 1,
            b'}' => {
                depth = depth.saturating_sub(1);
                while inline.last().is_some_and(|(_, d)| *d > depth) {
                    inline.pop();
                }
            }
            b'm' if code[i..].starts_with(b"mod")
                && (i == 0 || !is_ident(code[i - 1]))
                && code.get(i + 3).is_some_and(|b| b.is_ascii_whitespace()) =>
            {
                let mut j = i + 3;
                while code.get(j).is_some_and(|b| b.is_ascii_whitespace()) {
                    j += 1;
                }
                let name_start = j;
                while code.get(j).is_some_and(|&b| is_ident(b)) {
                    j += 1;
                }
                let name = String::from_utf8_lossy(&code[name_start..j])
                    .trim_start_matches("r#")
                    .to_owned();
                while code.get(j).is_some_and(|b| b.is_ascii_whitespace()) {
                    j += 1;
                }
                if name.is_empty() {
                    i = j;
                    continue;
                }
                let nested: PathBuf = inline.iter().map(|(n, _)| n.as_str()).collect();
                match code.get(j) {
                    Some(b';') => match path_attribute(&code, &text, i) {
                        Some(path) => found.push((here.join(&nested).join(path), true)),
                        None => {
                            let dir = base.join(&nested);
                            found.push((dir.join(format!("{name}.rs")), false));
                            found.push((dir.join(&name).join("mod.rs"), true));
                        }
                    },
                    Some(b'{') => {
                        depth += 1;
                        inline.push((name, depth));
                    }
                    _ => {}
                }
                i = j + 1;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    found
}

/// Every file reached from `roots`.
fn reached(roots: Vec<PathBuf>) -> BTreeSet<PathBuf> {
    let mut seen = BTreeSet::new();
    let mut stack: Vec<(PathBuf, bool)> = roots.into_iter().map(|r| (r, true)).collect();
    while let Some((file, owns_dir)) = stack.pop() {
        let Ok(file) = file.canonicalize() else {
            continue;
        };
        if seen.insert(file.clone()) {
            stack.extend(declared(&file, owns_dir));
        }
    }
    seen
}

fn rust_files(dir: &Path, out: &mut BTreeSet<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.insert(path.canonicalize().unwrap());
        }
    }
}

/// A crate's roots, as Cargo finds them by default.
fn roots(krate: &Path) -> Vec<PathBuf> {
    let mut roots = vec![
        krate.join("src/lib.rs"),
        krate.join("src/main.rs"),
        krate.join("build.rs"),
    ];
    for dir in ["src/bin", "tests", "examples", "benches"] {
        let Ok(entries) = std::fs::read_dir(krate.join(dir)) else {
            continue;
        };
        for entry in entries {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "rs") {
                roots.push(path);
            } else if path.is_dir() {
                roots.push(path.join("main.rs"));
            }
        }
    }
    roots
}

/// The `.rs` files under `krate` that no root reaches.
fn orphans(krate: &Path) -> Vec<PathBuf> {
    let mut files = BTreeSet::new();
    for dir in ["src", "tests", "examples", "benches"] {
        rust_files(&krate.join(dir), &mut files);
    }
    let reached = reached(roots(krate));
    files.difference(&reached).cloned().collect()
}

#[test]
fn every_rust_source_file_in_the_workspace_is_compiled() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut crates = 0;
    let mut missing = Vec::new();
    for entry in std::fs::read_dir(&workspace).unwrap() {
        let krate = entry.unwrap().path();
        if !krate.join("Cargo.toml").is_file() || krate.ends_with("target") {
            continue;
        }
        crates += 1;
        missing.extend(orphans(&krate));
    }
    assert!(
        crates > 5,
        "found {crates} crates under {}",
        workspace.display()
    );
    assert!(
        missing.is_empty(),
        "no `mod` declaration reaches these files, so they are never built and \
         their tests never run; declare each in its parent module (or delete it):\n{}",
        missing
            .iter()
            .map(|p| format!("  {}", p.display()))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The negative control: a synthetic crate with every way to reach a file,
/// and the ways that look like a declaration but are not.
#[test]
fn an_undeclared_file_is_found_and_every_declaration_form_is_followed() {
    let root = std::env::temp_dir().join(format!("psiv-source-files-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let files = [
        (
            "src/lib.rs",
            "//! Not `mod commented;` here.\n\
             mod plain;\n\
             pub(crate) mod dir;\n\
             #[cfg(test)]\n#[path = \"elsewhere/pathed.rs\"]\nmod pathed;\n\
             mod inline {\n    fn f() -> char { '}' }\n    mod nested;\n}\n\
             // mod commented;\n\
             /* mod blocked; /* nested */ */\n\
             const S: &str = \"mod quoted;\";\n\
             const R: &str = r#\"mod raw;\"#;\n\
             fn g<'a>(x: &'a str) -> &'a str { x }\n\
             mod after_lifetime;\n",
        ),
        ("src/plain.rs", "mod child;\n"),
        ("src/plain/child.rs", ""),
        ("src/dir/mod.rs", "mod beside;\n"),
        ("src/dir/beside.rs", ""),
        ("src/elsewhere/pathed.rs", "mod under;\n"),
        ("src/elsewhere/under.rs", ""),
        ("src/inline/nested.rs", ""),
        ("src/after_lifetime.rs", ""),
        ("src/commented.rs", ""),
        ("src/blocked.rs", ""),
        ("src/quoted.rs", ""),
        ("src/raw.rs", ""),
        ("src/orphan.rs", ""),
        ("tests/top.rs", "mod helper;\n"),
        ("tests/helper.rs", ""),
        ("tests/stray/inner.rs", ""),
    ];
    for (path, text) in files {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    let canonical = root.canonicalize().unwrap();
    // Joined with `/` from the components, so the names read the same on
    // Windows, where `display` separates them with a backslash.
    let found: Vec<String> = orphans(&root)
        .iter()
        .map(|p| {
            let relative = p.strip_prefix(&canonical).unwrap();
            let parts: Vec<_> = relative.iter().map(|c| c.to_str().unwrap()).collect();
            parts.join("/")
        })
        .collect();
    std::fs::remove_dir_all(&root).unwrap();
    assert_eq!(
        found,
        [
            "src/blocked.rs",
            "src/commented.rs",
            "src/orphan.rs",
            "src/quoted.rs",
            "src/raw.rs",
            "tests/stray/inner.rs",
        ]
    );
}
