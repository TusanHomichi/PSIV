//! Source-level float guard. Rust tokens and CommonMark blocks have one owner.
//! No type inference, macro expansion, or generated/included documentation.
#![forbid(unsafe_code)]

mod docs;
pub use docs::{Example, examples};
use ra_ap_rustc_lexer::{DocStyle, FrontmatterAllowed, LiteralKind, TokenKind};
use serde::Serialize;
use std::{fs, path::Path};

pub const CRATES: [&str; 3] = ["psiv-core", "psiv-data", "psiv-runtime"];
pub type Site = (usize, String);

#[derive(Clone, Copy)]
struct Token<'a> {
    kind: TokenKind,
    text: &'a str,
    line: usize,
}
fn tokens(source: &str) -> Vec<Token<'_>> {
    let mut offset = 0;
    let mut line = 1;
    ra_ap_rustc_lexer::tokenize(source, FrontmatterAllowed::No)
        .map(|token| {
            let end = offset + token.len as usize;
            let text = &source[offset..end];
            let result = Token {
                kind: token.kind,
                text,
                line,
            };
            offset = end;
            line += text.bytes().filter(|&b| b == b'\n').count();
            result
        })
        .collect()
}
fn trivia(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Whitespace | TokenKind::LineComment { .. } | TokenKind::BlockComment { .. }
    )
}
fn ident<'a>(token: &Token<'a>) -> &'a str {
    token.text.strip_prefix("r#").unwrap_or(token.text)
}

pub fn float_sites(source: &str) -> Vec<Site> {
    let tokens: Vec<_> = tokens(source)
        .into_iter()
        .filter(|t| !trivia(t.kind))
        .collect();
    let mut sites = Vec::new();
    let mut macros = Vec::new();
    for (i, token) in tokens.iter().enumerate() {
        if matches!(token.text, "(" | "[" | "{") {
            let call = i >= 2
                && tokens[i - 1].text == "!"
                && matches!(tokens[i - 2].kind, TokenKind::Ident | TokenKind::RawIdent);
            let definition =
                i >= 3 && tokens[i - 3].text == "macro_rules" && tokens[i - 2].text == "!";
            macros.push(macros.last().copied().unwrap_or(false) || call || definition);
        } else if matches!(token.text, ")" | "]" | "}") {
            macros.pop();
        }
        let is_float = match token.kind {
            TokenKind::Ident | TokenKind::RawIdent => matches!(ident(token), "f32" | "f64"),
            TokenKind::Literal {
                kind: LiteralKind::Float { .. },
                ..
            } => {
                // Chained tuple fields lex as a float. Macro token trees can
                // instead bind that same token as a literal: use (pair.0).1.
                let integer_chain = token.text.split_once('.').is_some_and(|(a, b)| {
                    !a.is_empty()
                        && !b.is_empty()
                        && a.bytes().all(|c| c.is_ascii_digit())
                        && b.bytes().all(|c| c.is_ascii_digit())
                });
                let receiver = i >= 2
                    && tokens[i - 1].text == "."
                    && (matches!(
                        tokens[i - 2].kind,
                        TokenKind::Ident | TokenKind::RawIdent | TokenKind::Literal { .. }
                    ) || matches!(tokens[i - 2].text, ")" | "]"));
                !(integer_chain && receiver && !macros.last().copied().unwrap_or(false))
            }
            TokenKind::Literal {
                kind: LiteralKind::Int { .. },
                suffix_start,
            } => matches!(&token.text[suffix_start as usize..], "f32" | "f64"),
            _ => false,
        };
        if is_float {
            sites.push((token.line, token.text.into()));
        }
    }
    sites
}

#[derive(Serialize)]
pub struct Violation {
    pub path: String,
    pub line: usize,
    pub token: String,
}
pub fn scan(root: &Path) -> Result<Vec<Violation>, String> {
    fn walk(root: &Path, path: &Path, found: &mut Vec<Violation>) -> Result<(), String> {
        let mut paths: Vec<_> = fs::read_dir(path)
            .map_err(|e| format!("{}: {e}", path.display()))?
            .map(|entry| entry.map(|e| e.path()))
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?;
        paths.sort();
        for path in paths {
            if path.is_dir() {
                walk(root, &path, found)?;
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let source = fs::read_to_string(&path).map_err(|e| e.to_string())?;
                let mut sites = float_sites(&source);
                for example in examples(&source)? {
                    for (line, token) in float_sites(&example.source) {
                        sites.push((example.lines[line - 1], token));
                    }
                }
                found.extend(sites.into_iter().map(|(line, token)| {
                    Violation {
                        path: path
                            .strip_prefix(root)
                            .unwrap()
                            .to_string_lossy()
                            .replace('\\', "/"),
                        line,
                        token,
                    }
                }));
            }
        }
        Ok(())
    }
    let mut found = Vec::new();
    for name in CRATES {
        let path = root.join("rust").join(name);
        // Missing source roots must fail closed. Fixtures can provide empty crates.
        if !path.join("src/lib.rs").is_file() {
            return Err(format!("missing {}", path.join("src/lib.rs").display()));
        }
        walk(root, &path, &mut found)?;
    }
    Ok(found)
}
