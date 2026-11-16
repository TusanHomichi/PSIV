//! Optional local reference checks. The fork is not a retail authority: its
//! five script differences remain explicit and must not silently disappear.
use super::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn reference() -> Option<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference/ps4disasm");
    if !root.exists() {
        eprintln!("local reference absent: text source comparisons skipped");
        return None;
    }
    Some(root)
}
fn read(path: &Path) -> String {
    // The reference is ISO-8859-1, not UTF-8.
    std::fs::read(path)
        .unwrap()
        .into_iter()
        .map(char::from)
        .collect()
}
fn number(token: &str) -> u8 {
    if let Some(h) = token.trim().strip_prefix('$') {
        u8::from_str_radix(h, 16).unwrap()
    } else {
        token.trim().parse().unwrap()
    }
}
fn charset_source(path: &Path) -> BTreeMap<u8, char> {
    let mut table = BTreeMap::new();
    for line in read(path).lines().filter(|l| !l.trim().is_empty()) {
        let args = line
            .trim()
            .strip_prefix("charset")
            .expect("unknown charset directive");
        // A comma can itself be the quoted glyph.
        let mut args = args.trim();
        let mut values = Vec::new();
        while !args.is_empty() {
            if args.starts_with('\'') {
                let c = args.chars().nth(1).unwrap();
                let end = 1 + c.len_utf8();
                assert_eq!(args.as_bytes()[end], b'\'');
                values.push(c as u8);
                args = &args[end + 1..];
            } else {
                let end = args.find(',').unwrap_or(args.len());
                values.push(number(&args[..end]));
                args = &args[end..];
            }
            args = args.trim_start().trim_start_matches(',').trim_start();
        }
        match values.as_slice() {
            &[first, last, base] => {
                for (i, c) in (first..=last).enumerate() {
                    assert!(table.insert(base + i as u8, char::from(c)).is_none());
                }
            }
            &[c, code] => {
                assert!(table.insert(code, char::from(c)).is_none());
            }
            _ => panic!("unrecognised charset directive"),
        }
    }
    table
}
fn tables(root: &Path) -> [BTreeMap<u8, char>; 2] {
    [
        charset_source(&root.join("general/tables/wincharset.asm")),
        charset_source(&root.join("script/charset.asm")),
    ]
}

#[test]
fn charsets_match_every_local_directive() {
    let Some(root) = reference() else { return };
    for (actual, source) in [window_charset(), dialogue_charset()]
        .into_iter()
        .zip(tables(&root))
    {
        for b in 0..=255 {
            assert_eq!(actual.glyph(b), source.get(&b).copied(), "glyph {b:#x}");
        }
    }
}

pub(super) fn names_against_reference(names: &Value) {
    let Some(root) = reference() else { return };
    let [window, dialogue] = tables(&root);
    // Independently decode every name against the source directives, covering
    // the old selected-name pins without storing cartridge text or digests.
    for spec in NAME_TABLES {
        let font = if spec.charset == WINDOW {
            &window
        } else {
            &dialogue
        };
        for entry in names[spec.name].as_array().unwrap() {
            let raw = unhex_value(&entry["raw_hex"]);
            let expected: String = raw[..raw.len() - 1].iter().map(|b| font[b]).collect();
            assert_eq!(entry["name"], expected);
        }
    }
}

fn operands(mut args: &str, charset: &BTreeMap<char, u8>) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        args = args.trim_start_matches(|c: char| c.is_whitespace() || c == ',');
        if args.is_empty() || args.starts_with(';') {
            break;
        }
        if let Some(rest) = args.strip_prefix('"') {
            args = rest;
            loop {
                let c = args.chars().next().expect("unterminated string");
                args = &args[c.len_utf8()..];
                if c == '"' {
                    if let Some(rest) = args.strip_prefix('"') {
                        args = rest;
                    } else {
                        break;
                    }
                }
                out.push(*charset.get(&c).expect("unmapped source character"));
            }
        } else {
            let end = args
                .find(|c: char| c == ',' || c == ';' || c.is_whitespace())
                .unwrap_or(args.len());
            out.push(number(&args[..end]));
            args = &args[end..];
        }
    }
    out
}
fn assemble(source: &str, charset: &BTreeMap<char, u8>) -> Vec<u8> {
    let mut out = Vec::new();
    let mut active = Vec::<(bool, bool)>::new();
    for raw in source.lines() {
        let line = raw.trim();
        if let Some(condition) = line
            .strip_prefix("if ")
            .or_else(|| line.strip_prefix("if\t"))
        {
            let (name, n) = condition.split_once('=').unwrap();
            assert_eq!(name.trim(), "skip_opening");
            let take = active.iter().all(|p| p.0) && number(n) == 0;
            active.push((take, take));
        } else if line == "else" {
            let (old, _) = active.pop().unwrap();
            let take = active.iter().all(|p| p.0) && !old;
            active.push((take, true));
        } else if line == "endif" {
            active.pop().unwrap();
        } else if active.iter().all(|p| p.0) {
            if let Some(args) = line.strip_prefix("dc.b") {
                out.extend(operands(args, charset));
            } else if let Some(arg) = line.strip_prefix("dc.w") {
                let arg = arg.split(';').next().unwrap().trim();
                let value: u16 = if let Some(h) = arg.strip_prefix('$') {
                    u16::from_str_radix(h, 16).unwrap()
                } else {
                    arg.parse().unwrap()
                };
                out.extend(value.to_be_bytes());
            } else {
                assert!(
                    line.split(';').next().unwrap().trim().is_empty(),
                    "unknown script directive"
                );
            }
        }
    }
    assert!(active.is_empty());
    if !out.len().is_multiple_of(2) {
        out.push(0);
    }
    out
}

// Insertion/deletion distance, banded at two edits. A same-length mismatch
// at distance two is precisely one byte inserted and one deleted.
fn two_single_byte_slips(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() || a == b {
        return false;
    }
    let mut previous = vec![3usize; b.len() + 1];
    previous[0] = 0;
    for (j, v) in previous.iter_mut().enumerate().take(3) {
        *v = j;
    }
    for i in 1..=a.len() {
        let mut row = vec![3; b.len() + 1];
        if i <= 2 {
            row[0] = i;
        }
        for j in i.saturating_sub(2).max(1)..=(i + 2).min(b.len()) {
            row[j] = (previous[j] + 1).min(row[j - 1] + 1);
            if a[i - 1] == b[j - 1] {
                row[j] = row[j].min(previous[j - 1]);
            }
        }
        previous = row;
    }
    previous[b.len()] == 2
}

pub(super) fn dialogue_against_reference(
    data: &[u8],
    specs: &[TreeSpec],
    decodes: &crate::decode::Decodes,
) {
    let Some(root) = reference() else { return };
    let font: BTreeMap<char, u8> = tables(&root)[1].iter().map(|(&b, &c)| (c, b)).collect();
    let mut mismatches = BTreeSet::new();
    for spec in specs {
        let source = assemble(
            &read(&root.join(format!("script/dialogue {}.asm", spec.tree))),
            &font,
        );
        let decoded = decodes
            .get(crate::decode::Call::Kosinski { offset: spec.start })
            .unwrap();
        let blob = decoded.bytes();
        if blob != source {
            mismatches.insert(spec.tree);
        }
        match spec.tree {
            17 => assert_ne!(blob.len(), source.len()),
            19 | 29 | 31 | 34 => {
                assert!(
                    two_single_byte_slips(&blob, &source),
                    "tree {} source drift",
                    spec.tree
                );
                if spec.tree == 34 {
                    assert_eq!(blob[0x119E], 0xFB);
                    assert_ne!(source[0x119E], 0xFB);
                }
            }
            _ => assert_eq!(blob, source, "tree {} differs", spec.tree),
        }
    }
    assert_eq!(mismatches, [17, 19, 29, 31, 34].into());
    // Plain text after the compressed region has its own portrait word.
    let listing = read(&root.join("ps4.asm"));
    let plain = listing
        .rsplit_once("\nWinTiles_PlayerNothingMsg:")
        .unwrap()
        .1;
    let mut first = Vec::new();
    for line in plain.lines().skip(1) {
        let line = line.trim();
        if let Some(arg) = line.strip_prefix("dc.w") {
            let word = u16::from_str_radix(arg.trim().trim_start_matches('$'), 16).unwrap();
            first.extend(word.to_be_bytes());
        } else if let Some(args) = line.strip_prefix("dc.b") {
            first.extend(operands(args, &font));
            if first.last() == Some(&0xFF) {
                break;
            }
        } else {
            assert!(line.is_empty());
        }
    }
    assert_eq!(first, data[0x1FE660..0x1FE660 + first.len()]);
    // The first tree's selected entry is also checked through this byte-exact
    // source comparison; inline text follows the final aligned blob.
    assert_eq!(data[specs.last().unwrap().end..0x1FE660], [0; 11]);
}

#[test]
fn source_parser_and_edit_check_have_negative_controls() {
    let font = BTreeMap::from([('a', 1), ('"', 2)]);
    assert_eq!(
        assemble(
            "if skip_opening=1\n dc.b $00\nelse\n dc.b \"a\"\"a\", $FF\nendif",
            &font
        ),
        [1, 2, 1, 255]
    );
    assert!(std::panic::catch_unwind(|| assemble("dc.b \"z\"", &font)).is_err());
    assert!(std::panic::catch_unwind(|| assemble("unknown $01", &font)).is_err());
    assert!(two_single_byte_slips(&[1, 2, 3, 4], &[1, 3, 4, 0]));
    assert!(!two_single_byte_slips(&[1, 2, 3, 4], &[1, 7, 8, 4]));
}
