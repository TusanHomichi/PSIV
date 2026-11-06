//! The byte ranges the scene documents claim, read off `docs/scenes`: each
//! scene document's `**Retail bytes:**` and `| Retail range |` lines, given to
//! the nearest scene heading above them, and the registry rows of
//! `docs/scenes/README.md`. The dialogue-entry census
//! (`dialogue_callers_image.rs`) ties each image caller to a scene through
//! them.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Scene name -> every `(start, end_exclusive)` its documents claim.
pub(super) type Ranges = BTreeMap<String, Vec<(usize, usize)>>;

pub(super) fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Six upper-case hex digits at the start of `text`, and the rest.
fn hex6(text: &str) -> Option<(usize, &str)> {
    let digits = text.get(..6)?;
    if !digits
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b))
    {
        return None;
    }
    Some((usize::from_str_radix(digits, 16).ok()?, &text[6..]))
}

/// The scene a heading line names: `#`..`######`, then optionally a document
/// number and a dash, then `` `Name` ``.
fn heading(line: &str) -> Option<(usize, String)> {
    let level = line.bytes().take_while(|b| *b == b'#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let rest = &line[level..];
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let rest = rest.trim_start();
    let named = |text: &str| -> Option<String> {
        let name = text.strip_prefix('`')?;
        let end = name.find('`')?;
        let name = &name[..end];
        (!name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
            .then(|| name.to_owned())
    };
    let name = named(rest).or_else(|| {
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }
        let after = rest[digits..].trim_start();
        let dash = after
            .chars()
            .next()
            .filter(|c| matches!(c, '—' | '–' | '-'))?;
        named(after[dash.len_utf8()..].trim_start())
    })?;
    Some((level, name))
}

/// `**Retail bytes:** `$XXXXXX..$YYYYYY`` at `at`: the inclusive span.
fn retail_bytes(text: &str) -> Option<(usize, usize)> {
    let rest = text.strip_prefix("**Retail bytes:**")?.trim_start();
    let (start, rest) = hex6(rest.strip_prefix("`$")?)?;
    let (end, rest) = hex6(rest.strip_prefix("..$")?)?;
    rest.starts_with('`').then_some((start, end))
}

/// `| Retail range | **`$XXXXXX` .. `$YYYYYY`**` from its `Retail range`.
fn retail_range(text: &str) -> Option<(usize, usize)> {
    let rest = text.strip_prefix("Retail range")?.trim_start();
    let rest = rest.strip_prefix('|')?.trim_start();
    let (start, rest) = hex6(rest.strip_prefix("**`$")?)?;
    let rest = rest.strip_prefix('`')?.trim_start();
    let rest = rest.strip_prefix("..")?.trim_start();
    let (end, rest) = hex6(rest.strip_prefix("`$")?)?;
    rest.starts_with("`**").then_some((start, end))
}

/// Every range one scene document claims, each given to the nearest scene
/// heading above it, so the five `Event_VahalFort*` sections of document 110
/// keep their own bytes instead of all landing on the title's scene. A
/// document whose title names no scene claims nothing.
fn document_ranges(text: &str) -> Vec<(String, usize, usize)> {
    let mut found = Vec::new();
    if !text
        .lines()
        .any(|line| heading(line).is_some_and(|(level, _)| level == 1))
    {
        return found;
    }
    let mut owner: Option<String> = None;
    for line in text.lines() {
        let mut spans = Vec::new();
        for (at, _) in line.match_indices("**Retail bytes:**") {
            spans.extend(retail_bytes(&line[at..]));
        }
        for (at, _) in line.match_indices("Retail range") {
            if line[..at].trim_end().ends_with('|') {
                spans.extend(retail_range(&line[at..]));
            }
        }
        if let Some((_, name)) = heading(line) {
            owner = Some(name);
        }
        if let Some(name) = &owner {
            for (start, end) in spans {
                found.push((name.clone(), start, end + 1));
            }
        }
    }
    found
}

/// The registry rows of `docs/scenes/README.md`: `` | `Name` | `$EV` | doc |
/// `$XXXXXX..$YYYYYY` | ``.
fn registry_ranges(readme: &str) -> Vec<(String, usize, usize)> {
    let mut found = Vec::new();
    for line in readme.lines() {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        // A row is `| a | b | c | d |`: the first and last pieces are outside.
        if cells.len() < 6 {
            continue;
        }
        let Some(name) = cells[1].strip_prefix('`').and_then(|c| c.strip_suffix('`')) else {
            continue;
        };
        let event = cells[2]
            .strip_prefix("`$")
            .and_then(|c| c.strip_suffix('`'));
        if !event.is_some_and(|e| {
            !e.is_empty()
                && e.bytes()
                    .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b))
        }) {
            continue;
        }
        let span = cells[4]
            .strip_prefix("`$")
            .and_then(hex6)
            .and_then(|(start, rest)| {
                let (end, rest) = hex6(rest.strip_prefix("..$")?)?;
                (rest == "`").then_some((start, end))
            });
        if let Some((start, end)) = span
            && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            found.push((name.to_owned(), start, end + 1));
        }
    }
    found
}

/// Scene name -> every range the scene documents and the registry claim.
pub(super) fn documented_ranges() -> Ranges {
    let dir = repo().join("docs/scenes");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|x| x == "md"))
        .collect();
    paths.sort();
    let mut ranges = Ranges::new();
    let mut add = |rows: Vec<(String, usize, usize)>| {
        for (name, start, end) in rows {
            ranges.entry(name).or_default().push((start, end));
        }
    };
    for path in &paths {
        add(document_ranges(&std::fs::read_to_string(path).unwrap()));
    }
    add(registry_ranges(
        &std::fs::read_to_string(dir.join("README.md")).unwrap(),
    ));
    ranges
}

#[test]
fn a_section_range_belongs_to_its_section() {
    // Document 111 holds `Event_Burstroc` then `## `Event_PosiBolt``. Each
    // section's `Retail bytes` line (`$0724A4..$07251F`, `$072520..$07259D`)
    // belongs to its own heading; attributing both to the title would put
    // PosiBolt's bytes in Burstroc.
    let ranges = documented_ranges();
    assert!(!ranges["Event_Burstroc"].contains(&(0x072520, 0x07259E)));
    assert!(ranges["Event_Burstroc"].contains(&(0x0724A4, 0x072520)));
    assert!(ranges["Event_PosiBolt"].contains(&(0x072520, 0x07259E)));
}

#[test]
fn the_document_readers_take_each_form() {
    let text = "# 53 — `Event_A`\n\
                | Retail range | **`$070000` .. `$07000F`** (16 bytes) |\n\
                ## `Event_B`\n\
                - **Retail bytes:** `$070010..$07001F` inclusive\n\
                - **Retail bytes:** `$07002..$07003F` (five digits: not a range)\n";
    assert_eq!(
        document_ranges(text),
        vec![
            ("Event_A".to_owned(), 0x070000, 0x070010),
            ("Event_B".to_owned(), 0x070010, 0x070020),
        ]
    );
    // No scene in the title: nothing is claimed.
    assert!(
        document_ranges("# Notes\n## `Event_B`\n**Retail bytes:** `$070010..$07001F`\n").is_empty()
    );
    assert_eq!(
        registry_ranges(
            "| Scene | Event | Doc | Retail bytes |\n\
             |---|---:|---|---|\n\
             | `Event_C` | `$2A` | doc | `$070040..$07004F` |\n\
             | `Event_D` | `$2B` | doc | pending |\n"
        ),
        vec![("Event_C".to_owned(), 0x070040, 0x070050)]
    );
}
