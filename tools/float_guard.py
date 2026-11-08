"""Reject float syntax in retail crates, including Clippy's test blind spots.

This is a conservative source guard, not Rust type inference. There are no
allowed float sites. Strings/comments are ignored; fenced Rust doc examples
are scanned separately. External macro expansions remain Clippy's concern.
"""
import re

CRATES = ("psiv-core", "psiv-data", "psiv-runtime")
# Consume whole identifiers and numeric tokens so hex addresses, tuple fields,
# ranges and method calls cannot be mistaken for float literals.
TOKEN = re.compile(
    r'(?P<raw>(?:br|r)\#*\")|(?P<string>b?"(?:\\.|[^"\\])*" )'
    r"|(?P<char>b?'(?:\\(?:u\{[0-9a-fA-F_]+\}|x[0-9a-fA-F]{2}|.)|[^'\\])')"
    r'|(?P<line>//[^\n]*)|(?P<block>/\*)'
    r'|(?P<number>(?<![\w.])(?:0[xob][0-9a-fA-F_]+(?:[iu][0-9]+)?|'
    r'[0-9][0-9_]*(?:\.(?!\.|[A-Za-z_])[0-9_]*)?'
    r'(?:[eE][+-]?[0-9_]+)?(?:[A-Za-z_][A-Za-z_0-9]*)?))'
    r'|(?P<ident>\b[A-Za-z_][A-Za-z_0-9]*)',
    re.VERBOSE,
)
FLOAT = re.compile(r'^(?:f32|f64)$|^[0-9][0-9_]*(?:\.(?!\.)|[eE]|_?f(?:32|64)$)')


def float_sites(source):
    """Return (line, token) pairs; skip nested comments and Rust strings."""
    sites = []
    pos = 0
    while match := TOKEN.search(source, pos):
        pos = match.end()
        kind = match.lastgroup
        if kind == 'raw':
            hashes = match.group().count('#')
            end = source.find('"' + '#' * hashes, pos)
            pos = len(source) if end < 0 else end + 1 + hashes
        elif kind == 'block':
            depth = 1
            while depth and pos < len(source):
                marker = re.search(r'/\*|\*/', source[pos:])
                if marker is None:
                    pos = len(source)
                    break
                depth += 1 if marker.group() == '/*' else -1
                pos += marker.end()
        elif kind in ('number', 'ident') and FLOAT.search(match.group()):
            sites.append((source.count('\n', 0, match.start()) + 1, match.group()))
    return sites


def violations(root):
    """Scan all crate Rust files, plus fenced Rust line/block-doc examples."""
    found = []
    for crate in CRATES:
        for path in sorted((root / 'rust' / crate).rglob('*.rs')):
            source = path.read_text()
            snippets = [source]
            # Keep original line numbers. Non-Rust fenced examples are excluded.
            doc = []
            active = False
            fenced = False
            block_doc = False
            for line in source.splitlines():
                match = re.match(r'\s*//[/!] ?(.*)', line)
                text = match.group(1) if match else ''
                stripped = line.strip()
                if stripped.startswith(('/**', '/*!')):
                    block_doc = True
                    text = stripped[3:]
                elif block_doc:
                    text = stripped.removeprefix('*').lstrip()
                if block_doc and '*/' in line:
                    text = text.split('*/')[0]
                    block_doc = False
                if text.startswith('```'):
                    active = not fenced and text[3:].split(',')[0] in ('', 'rust', 'no_run', 'ignore', 'should_panic', 'compile_fail')
                    fenced = not fenced
                    doc.append('')
                else:
                    doc.append(text if active else '')
            snippets.append('\n'.join(doc))
            for snippet in snippets:
                found.extend(f'{path.relative_to(root)}:{line}: float syntax {token}'
                             for line, token in float_sites(snippet))
    return found
