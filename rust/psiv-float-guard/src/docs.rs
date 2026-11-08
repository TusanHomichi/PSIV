//! Extract real Rust docs, then let CommonMark own code-block boundaries.
use super::{DocStyle, LiteralKind, Token, TokenKind, ident, tokens, trivia};
use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};
use serde::Serialize;

type Mapped = Vec<(char, usize)>;
#[derive(Serialize)]
pub struct Example {
    pub source: String,
    pub lines: Vec<usize>,
}

fn raw_chars(text: &str, mut line: usize) -> Mapped {
    text.chars()
        .map(|ch| {
            let result = (ch, line);
            line += usize::from(ch == '\n');
            result
        })
        .collect()
}
fn string_doc(token: Token<'_>) -> Result<Mapped, String> {
    if let TokenKind::Literal {
        kind: LiteralKind::RawStr {
            n_hashes: Some(hashes),
        },
        ..
    } = token.kind
    {
        let start = token.text.find('"').ok_or("invalid raw doc string")? + 1;
        return Ok(raw_chars(
            &token.text[start..token.text.len() - usize::from(hashes) - 1],
            token.line,
        ));
    }
    let mut chars = token.text[1..token.text.len() - 1].chars().peekable();
    let mut line = token.line;
    let mut result = Vec::new();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            result.push((ch, line));
            line += usize::from(ch == '\n');
            continue;
        }
        let escaped = chars.next().ok_or("invalid doc escape")?;
        let value = match escaped {
            '\n' | '\r' => {
                line += usize::from(escaped == '\n');
                while chars.peek().is_some_and(|ch| ch.is_whitespace()) {
                    line += usize::from(chars.next() == Some('\n'));
                }
                continue;
            }
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            '0' => '\0',
            '\\' => '\\',
            '"' => '"',
            '\'' => '\'',
            'x' => {
                let code: String = chars.by_ref().take(2).collect();
                char::from_u32(u32::from_str_radix(&code, 16).map_err(|e| e.to_string())?)
                    .ok_or("invalid doc byte")?
            }
            'u' => {
                if chars.next() != Some('{') {
                    return Err("invalid doc unicode escape".into());
                }
                let code: String = chars
                    .by_ref()
                    .take_while(|&ch| ch != '}')
                    .filter(|&ch| ch != '_')
                    .collect();
                char::from_u32(u32::from_str_radix(&code, 16).map_err(|e| e.to_string())?)
                    .ok_or("invalid doc unicode scalar")?
            }
            _ => return Err("invalid doc escape".into()),
        };
        result.push((value, line));
    }
    Ok(result)
}
fn literal_docs<'a>(parts: &[Token<'a>], docs: &mut Vec<Token<'a>>) {
    let Some(first) = parts.first() else {
        return;
    };
    match ident(first) {
        "doc"
            if parts.len() == 3
                && parts[1].text == "="
                && matches!(
                    parts[2].kind,
                    TokenKind::Literal {
                        kind: LiteralKind::Str { terminated: true }
                            | LiteralKind::RawStr { n_hashes: Some(_) },
                        ..
                    }
                ) =>
        {
            docs.push(parts[2])
        }
        "cfg_attr"
            if parts.len() > 3 && parts[1].text == "(" && parts.last().unwrap().text == ")" =>
        {
            let mut depth = 0;
            let mut start = 2;
            let mut condition = true;
            for i in 2..parts.len() {
                let text = parts[i].text;
                if (text == "," && depth == 0) || i == parts.len() - 1 {
                    if !condition {
                        literal_docs(&parts[start..i], docs);
                    }
                    condition = false;
                    start = i + 1;
                } else if matches!(text, "(" | "[" | "{") {
                    depth += 1;
                } else if matches!(text, ")" | "]" | "}") {
                    depth -= 1;
                }
            }
        }
        _ => {}
    }
}
fn decorate_block(chars: Mapped) -> Mapped {
    let rows: Vec<_> = chars.split_inclusive(|&(ch, _)| ch == '\n').collect();
    let starred = rows
        .iter()
        .skip(1)
        .filter(|row| row.iter().any(|(ch, _)| !ch.is_whitespace()))
        .all(|row| {
            let text: String = row.iter().map(|(ch, _)| ch).collect();
            text.trim_start()
                .strip_prefix('*')
                .is_some_and(|tail| tail.is_empty() || tail.starts_with(char::is_whitespace))
        });
    if !starred {
        return chars;
    }
    let mut result = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let prefix = row.iter().take_while(|(ch, _)| *ch == ' ').count();
        let mut start = 0;
        if index > 0 && row.get(prefix).is_some_and(|(ch, _)| *ch == '*') {
            start = prefix + 1;
            if row.get(start).is_some_and(|(ch, _)| *ch == ' ') {
                start += 1;
            }
        }
        result.extend_from_slice(&row[start..]);
    }
    result
}
fn documentation(source: &str) -> Result<Vec<Mapped>, String> {
    let stream = tokens(source);
    let mut groups = Vec::new();
    let mut group = Vec::new();
    let mut style = None;
    let mut i = 0;
    while i < stream.len() {
        let token = stream[i];
        let mut doc = None;
        match token.kind {
            TokenKind::LineComment {
                doc_style: Some(attachment),
            } => {
                let body = token.text[3..]
                    .strip_prefix(' ')
                    .unwrap_or(&token.text[3..]);
                let mut chars = raw_chars(body, token.line);
                chars.push(('\n', token.line));
                doc = Some((attachment, chars));
            }
            TokenKind::BlockComment {
                doc_style: Some(attachment),
                terminated: true,
            } => {
                let mut chars =
                    decorate_block(raw_chars(&token.text[3..token.text.len() - 2], token.line));
                chars.push((
                    '\n',
                    token.line + token.text.bytes().filter(|&b| b == b'\n').count(),
                ));
                doc = Some((attachment, chars));
            }
            TokenKind::Pound => {
                let mut j = i + 1;
                let mut parts = Vec::new();
                let mut depth = 0;
                while j < stream.len() {
                    let part = stream[j];
                    j += 1;
                    if trivia(part.kind) {
                        continue;
                    }
                    parts.push(part);
                    match part.text {
                        "[" => depth += 1,
                        "]" => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        "!" if depth == 0 => {}
                        _ if depth == 0 => break,
                        _ => {}
                    }
                }
                if parts.last().is_some_and(|t| t.text == "]") {
                    i = j - 1;
                    let inner = parts[0].text == "!";
                    let attr = if inner { &parts[1..] } else { &parts[..] };
                    let mut literals = Vec::new();
                    literal_docs(&attr[1..attr.len() - 1], &mut literals);
                    let mut chars = Vec::new();
                    for literal in literals {
                        chars.extend(string_doc(literal)?);
                        chars.push((
                            '\n',
                            literal.line + literal.text.bytes().filter(|&b| b == b'\n').count(),
                        ));
                    }
                    if chars.is_empty() {
                        i += 1;
                        continue;
                    }
                    doc = Some((
                        if inner {
                            DocStyle::Inner
                        } else {
                            DocStyle::Outer
                        },
                        chars,
                    ));
                }
            }
            _ => {}
        }
        if let Some((attachment, chars)) = doc {
            if !group.is_empty() && style != Some(attachment) {
                groups.push(std::mem::take(&mut group));
            }
            style = Some(attachment);
            group.extend(chars);
        } else if !trivia(token.kind) {
            if !group.is_empty() {
                groups.push(std::mem::take(&mut group));
            }
            style = None;
        }
        i += 1;
    }
    if !group.is_empty() {
        groups.push(group);
    }
    Ok(groups)
}
fn rust_info(info: &str) -> bool {
    let mut names = Vec::new();
    let mut word = String::new();
    let mut braces = 0;
    let mut quoted = false;
    for ch in info.chars().chain(std::iter::once(',')) {
        if ch == '"' {
            quoted = !quoted;
        }
        if !quoted && ch == '{' {
            braces += 1;
        }
        if !quoted && ch == '}' {
            braces -= 1;
            continue;
        }
        if braces > 0 {
            continue;
        }
        if !quoted && (ch == ',' || ch.is_whitespace()) {
            if !word.is_empty() {
                names.push(std::mem::take(&mut word));
            }
        } else if ch != '"' {
            word.push(ch);
        }
    }
    if names.iter().any(|s| s == "custom") {
        return false;
    }
    names.iter().any(|s| s == "rust")
        || names.iter().all(|s| {
            matches!(
                s.as_str(),
                "no_run"
                    | "ignore"
                    | "should_panic"
                    | "compile_fail"
                    | "test_harness"
                    | "standalone_crate"
                    | "edition2015"
                    | "edition2018"
                    | "edition2021"
                    | "edition2024"
            ) || s.starts_with("ignore-")
                || (s.len() == 5
                    && s.starts_with('E')
                    && s[1..].bytes().all(|c| c.is_ascii_digit()))
        })
}

pub fn examples(source: &str) -> Result<Vec<Example>, String> {
    let mut result = Vec::new();
    for group in documentation(source)? {
        let text: String = group.iter().map(|(ch, _)| ch).collect();
        let mut source_lines = vec![group.first().map_or(1, |(_, line)| *line)];
        for pair in group.windows(2) {
            if pair[0].0 == '\n' {
                source_lines.push(pair[1].1);
            }
        }
        let mut current: Option<(String, Mapped)> = None;
        for (event, range) in Parser::new(&text).into_offset_iter() {
            match event {
                Event::Start(Tag::CodeBlock(kind)) => {
                    let enabled = match kind {
                        CodeBlockKind::Indented => true,
                        CodeBlockKind::Fenced(info) => rust_info(&info),
                    };
                    if enabled {
                        current = Some((String::new(), Vec::new()));
                    }
                }
                Event::Text(value) => {
                    if let Some((code, mapping)) = &mut current {
                        let line = text[..range.start].bytes().filter(|&b| b == b'\n').count();
                        let mut index = line;
                        for ch in value.chars() {
                            mapping.push((
                                ch,
                                *source_lines.get(index).ok_or("invalid doc source map")?,
                            ));
                            index += usize::from(ch == '\n');
                        }
                        code.push_str(&value);
                    }
                }
                Event::End(TagEnd::CodeBlock) => {
                    if let Some((code, mapping)) = current.take() {
                        let mut lines = Vec::new();
                        let mut new_line = true;
                        for (ch, line) in mapping {
                            if new_line {
                                lines.push(line);
                            }
                            new_line = ch == '\n';
                        }
                        result.push(Example {
                            source: code,
                            lines,
                        });
                    }
                }
                _ => {}
            }
        }
    }
    Ok(result)
}
