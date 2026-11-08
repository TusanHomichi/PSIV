//! Extract real Rust docs, then let CommonMark own code-block boundaries.
use super::{DocStyle, LiteralKind, Token, TokenKind, ident, tokens, trivia};
use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};
use serde::Serialize;

type Mapped = Vec<(char, usize)>;
struct Fragment {
    chars: Mapped,
    guards: Vec<String>,
}
struct LiteralDoc<'a> {
    token: Token<'a>,
    guards: Vec<String>,
}

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
fn literal_docs<'a>(parts: &[Token<'a>], guards: &[String], docs: &mut Vec<LiteralDoc<'a>>) {
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
            docs.push(LiteralDoc {
                token: parts[2],
                guards: guards.to_vec(),
            })
        }
        "cfg_attr"
            if parts.len() > 3 && parts[1].text == "(" && parts.last().unwrap().text == ")" =>
        {
            let mut depth = 0;
            let mut start = 2;
            let mut condition: Option<String> = None;
            for i in 2..parts.len() {
                let text = parts[i].text;
                if (text == "," && depth == 0) || i == parts.len() - 1 {
                    if let Some(condition) = &condition {
                        let mut nested = guards.to_vec();
                        nested.push(condition.clone());
                        literal_docs(&parts[start..i], &nested, docs);
                    } else {
                        // Equal predicates on one item must select their fragments
                        // together. Include token lengths to avoid concatenation collisions.
                        condition = Some(
                            parts[start..i]
                                .iter()
                                .map(|t| format!("{}:{}", t.text.len(), t.text))
                                .collect::<String>(),
                        );
                    }
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

fn alternatives(group: &[Fragment]) -> Result<Vec<Mapped>, String> {
    let mut guards = Vec::new();
    for fragment in group {
        for guard in &fragment.guards {
            if !guards.contains(guard) {
                guards.push(guard.clone());
            }
        }
    }
    // Bound work without silently dropping configurations. Oversized items fail
    // closed so they must be simplified before this static check can certify them.
    if guards.len() > 8 {
        return Err("more than 256 conditional documentation alternatives on one item".into());
    }
    let mut result = Vec::new();
    for mask in 0..(1usize << guards.len()) {
        let mut chars = Vec::new();
        for fragment in group {
            if fragment
                .guards
                .iter()
                .all(|guard| mask & (1 << guards.iter().position(|g| g == guard).unwrap()) != 0)
            {
                chars.extend_from_slice(&fragment.chars);
            }
        }
        if !result.contains(&chars) {
            result.push(chars);
        }
    }
    Ok(result)
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
fn documentation(source: &str) -> Result<Vec<Vec<Fragment>>, String> {
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
                doc = Some((
                    attachment,
                    vec![Fragment {
                        chars,
                        guards: Vec::new(),
                    }],
                ));
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
                doc = Some((
                    attachment,
                    vec![Fragment {
                        chars,
                        guards: Vec::new(),
                    }],
                ));
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
                    literal_docs(&attr[1..attr.len() - 1], &[], &mut literals);
                    let mut fragments = Vec::new();
                    for literal in literals {
                        let token = literal.token;
                        let mut chars = string_doc(token)?;
                        chars.push((
                            '\n',
                            token.line + token.text.bytes().filter(|&b| b == b'\n').count(),
                        ));
                        fragments.push(Fragment {
                            chars,
                            guards: literal.guards,
                        });
                    }
                    if fragments.is_empty() {
                        i += 1;
                        continue;
                    }
                    doc = Some((
                        if inner {
                            DocStyle::Inner
                        } else {
                            DocStyle::Outer
                        },
                        fragments,
                    ));
                }
            }
            _ => {}
        }
        if let Some((attachment, fragments)) = doc {
            if !group.is_empty() && style != Some(attachment) {
                groups.push(std::mem::take(&mut group));
            }
            style = Some(attachment);
            group.extend(fragments);
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
    // Rustdoc TagIterator grammar: parenthesized comments and CSS groups do
    // not contribute language tokens; quoted language names remain atomic.
    let mut names = Vec::new();
    let mut word = String::new();
    let mut braces = 0;
    let mut quoted = false;
    let mut comment = false;
    for ch in info.chars().chain(std::iter::once(',')) {
        if comment {
            if ch == ')' {
                comment = false;
            }
            continue;
        }
        if ch == '"' {
            quoted = !quoted;
        }
        if !quoted && ch == '(' {
            if !word.is_empty() {
                names.push(std::mem::take(&mut word));
            }
            comment = true;
            continue;
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
    if comment || quoted || braces != 0 {
        return false;
    }
    let mut rust = false;
    let mut other = false;
    let mut custom = false;
    for name in names {
        match name.as_str() {
            "rust" => rust = true,
            "custom" => custom = true,
            "no_run" | "ignore" | "should_panic" => rust = !other,
            "compile_fail" | "test_harness" | "standalone_crate" => rust |= !other,
            value if value.starts_with("ignore-") => rust = !other,
            value if value.starts_with("edition") => {}
            value
                if value.len() == 5
                    && value.starts_with('E')
                    && value[1..].bytes().all(|c| c.is_ascii_digit()) =>
            {
                rust |= !other
            }
            _ => other = true,
        }
    }
    !custom && (!other || rust)
}

pub fn examples(source: &str) -> Result<Vec<Example>, String> {
    let mut result = Vec::new();
    for fragments in documentation(source)? {
        for group in alternatives(&fragments)? {
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
    }
    let mut seen = std::collections::HashSet::new();
    result.retain(|example| seen.insert((example.source.clone(), example.lines.clone())));
    Ok(result)
}
