"""Source-level float guard for retail crates, including Clippy's test gaps.

No allowed float sites. This recognizes source tokens, not inferred types or
external macro expansions. Literal doc attributes and comment docs are scanned;
include_str!/generated documentation remains outside this source-only check.
"""
from dataclasses import dataclass
import re

CRATES = ("psiv-core", "psiv-data", "psiv-runtime")
LEX = re.compile(
    r'(?P<space>\s+)'
    r'|(?P<raw>(?:br|cr|r)\#*")'
    r'|(?P<string>[bc]?"(?:\\[\s\S]|[^"\\])*(?:"|\Z))'
    r"|(?P<char>b?'(?:\\(?:u\{[0-9a-fA-F_]+\}|x[0-9a-fA-F]{2}|.)|[^'\\\n])')"
    r'|(?P<line>//[^\n]*)|(?P<block>/\*)'
    r'|(?P<number>[0-9][0-9_]*(?:\.(?!\.|[^\W\d])[0-9_]*)?'
    r'(?:[eE][+-]?[0-9_]+)?(?:[^\W\d]\w*)?)'
    r'|(?P<ident>(?:r\#)?[^\W\d]\w*)'
    r'|(?P<range>\.\.(?:=|\.)?)|(?P<other>[\s\S])', re.UNICODE,
)


@dataclass
class Token:
    kind: str
    start: int
    text: str

    @property
    def end(self):
        return self.start + len(self.text)


def tokens(source):
    """Keep string/comment boundaries, even in incomplete compile_fail code."""
    pos = 0
    while pos < len(source):
        match = LEX.match(source, pos)
        kind = match.lastgroup
        end = match.end()
        if kind == 'raw':
            delimiter = '"' + '#' * match.group().count('#')
            close = source.find(delimiter, end)
            end = len(source) if close < 0 else close + len(delimiter)
        elif kind == 'block':
            depth = 1
            while depth and end < len(source):
                marker = re.search(r'/\*|\*/', source[end:])
                if marker is None:
                    end = len(source)
                    break
                depth += 1 if marker.group() == '/*' else -1
                end += marker.end()
        yield Token(kind, pos, source[pos:end])
        pos = end


def float_sites(source):
    """Return (one-based line, token), with range and tuple punctuation distinct."""
    sites = []
    history = []
    delimiters = []
    for token in tokens(source):
        if token.kind in ('space', 'line', 'block'):
            continue
        text = token.text
        if text in ('(', '[', '{'):
            invocation = len(history) >= 2 and history[-1].text == '!' and history[-2].kind == 'ident'
            definition = len(history) >= 3 and [t.text for t in history[-3:-1]] == ['macro_rules', '!']
            delimiters.append(bool((delimiters and delimiters[-1]) or invocation or definition))
        elif text in (')', ']', '}') and delimiters:
            delimiters.pop()
        is_float = token.kind == 'ident' and text.removeprefix('r#') in ('f32', 'f64')
        if token.kind == 'number':
            # Nondecimal literals are consumed with their full identifier suffix.
            is_float = not text.startswith(('0x', '0o', '0b')) and (
                '.' in text or re.search(r'[eE][+-]?[0-9_]+', text) is not None
                or text.endswith(('f32', 'f64')))
        # Rust tokenizes chained tuple fields as a decimal-looking token. Only
        # accept bare integer field chains with an expression receiver; suffixes
        # and exponents are never tuple indices. A macro's leading dot is not a
        # receiver (e.g. value!(. 1.5)). Within macro token trees even a.b.c
        # can match an ident, dot and float literal; require (a.b).c there.
        if (token.kind == 'number' and re.fullmatch(r'[0-9]+\.[0-9]+', text)
                and len(history) >= 2 and history[-1].text == '.'
                and not any(delimiters)):
            receiver = history[-2]
            if receiver.kind in ('ident', 'number') or receiver.text in (')', ']'):
                is_float = False
        if is_float:
            sites.append((source.count('\n', 0, token.start) + 1, text))
        history.append(token)
    return sites


def violations(root):
    from tools.float_guard_docs import doc_examples

    found = []
    for crate in CRATES:
        for path in sorted((root / 'rust' / crate).rglob('*.rs')):
            source = path.read_text()
            sites = float_sites(source)
            for example, lines in doc_examples(source):
                sites.extend((lines[line - 1], text) for line, text in float_sites(example))
            found.extend(f'{path.relative_to(root)}:{line}: float syntax {text}'
                         for line, text in sites)
    return found
