"""Documentation tokens and independent Rustdoc examples for the float guard."""
import re

from tools.float_guard import tokens


def string_doc(token, source):
    """Decode a literal doc attribute, retaining a source line per character."""
    text = token.text
    line = source.count('\n', 0, token.start) + 1
    if token.kind == 'raw':
        start = text.index('"') + 1
        end = len(text) - (start - 1)  # r + hashes + quote equals closing width
        return [(ch, line + text[:start + i].count('\n'))
                for i, ch in enumerate(text[start:end])]
    result = []
    i = text.index('"') + 1
    escapes = {'n': '\n', 'r': '\r', 't': '\t', '0': '\0', '\\': '\\', '"': '"', "'": "'"}
    while i < len(text) - 1:
        ch = text[i]
        if ch == '\\':
            i += 1
            ch = text[i]
            if ch in '\r\n':
                while i < len(text) and text[i].isspace():
                    line += text[i] == '\n'
                    i += 1
                continue
            if ch == 'u':
                end = text.index('}', i)
                ch = chr(int(text[i + 2:end].replace('_', ''), 16))
                i = end
            elif ch == 'x':
                ch = chr(int(text[i + 1:i + 3], 16))
                i += 2
            else:
                ch = escapes.get(ch, ch)
        result.append((ch, line))
        line += text[i] == '\n'
        i += 1
    return result


def documentation(source):
    """Group real doc tokens by item; never inspect strings as comment text."""
    stream = list(tokens(source))
    group = []
    style = None
    i = 0
    while i < len(stream):
        token = stream[i]
        text = token.text
        chars = None
        current_style = None
        if token.kind == 'line' and (text.startswith('//!') or
                (text.startswith('///') and not text.startswith('////'))):
            current_style = text[2]
            line = source.count('\n', 0, token.start) + 1
            chars = [(ch, line) for ch in text[3:].removeprefix(' ')]
            chars.append(('\n', source.count('\n', 0, token.start) + 1))
        elif token.kind == 'block' and (text.startswith('/*!') or
                (text.startswith('/**') and not text.startswith('/***'))):
            current_style = text[2]
            body = text[3:-2]
            chars = []
            line = source.count('\n', 0, token.start) + 1
            for ch in body:
                chars.append((ch, line))
                line += ch == '\n'
            # Only strip decorative stars when the block consistently uses them.
            rows = body.splitlines(keepends=True)
            decorated = [row for row in rows[1:] if row.strip()]
            if decorated and all(re.match(r'\s*\*(?: |$)', row) for row in decorated):
                cleaned = []
                offset = 0
                for index, row in enumerate(rows):
                    prefix = re.match(r' *\* ?', row) if index else None
                    start = prefix.end() if prefix else 0
                    cleaned.extend(chars[offset + start:offset + len(row)])
                    offset += len(row)
                chars = cleaned
            chars.append(('\n', line))
        elif text == '#':
            # Parse attributes from significant tokens, including inline doc = literal.
            j = i + 1
            significant = []
            depth = 0
            while j < len(stream):
                part = stream[j]
                j += 1
                if part.kind in ('space', 'line', 'block'):
                    continue
                significant.append(part)
                if part.text == '[':
                    depth += 1
                elif part.text == ']':
                    depth -= 1
                    if depth == 0:
                        break
                elif depth == 0 and part.text != '!':
                    break
            parts = [part.text for part in significant]
            inner = parts[:1] == ['!']
            attr = significant[1:] if inner else significant
            if len(attr) == 5 and [part.text for part in attr[:3]] == ['[', 'doc', '='] and attr[-1].text == ']' and attr[3].kind in ('raw', 'string'):
                chars = string_doc(attr[3], source)
                chars.append(('\n', source.count('\n', 0, token.end) + 1))
                current_style = '!' if inner else '/'
            if parts and parts[-1] == ']':
                i = j - 1
                if chars is None:
                    i += 1
                    continue  # Other attributes can separate docs on the same item.
        if chars is not None:
            if group and style != current_style:
                yield group
                group = []
            style = current_style
            group.extend(chars)
        elif token.kind not in ('space', 'line', 'block'):
            if group:
                yield group
                group = []
            style = None
        i += 1
    if group:
        yield group


def rust_info(info):
    """Rustdoc defaults to Rust; known language tags override that default."""
    attrs = re.findall(r'[\w-]+', info)
    known = lambda attr: (attr in ('rust', 'no_run', 'ignore', 'should_panic',
                                  'compile_fail', 'test_harness', 'standalone_crate')
                          or attr.startswith(('edition', 'ignore-'))
                          or re.fullmatch(r'E\d{4}', attr))
    return not attrs or 'rust' in attrs or all(known(attr) for attr in attrs)


def doc_examples(source):
    """Extract fences (backticks/tildes) and indented code, one lexer per block."""
    for chars in documentation(source):
        # Strip doc decoration once per item, preserving indentation for Markdown.
        rows = []
        text = ''.join(ch for ch, _ in chars)
        offset = 0
        raw_rows = text.splitlines(keepends=True)
        for row in raw_rows:
            line = chars[offset][1] if offset < len(chars) else 1
            content = row.rstrip('\r\n')
            rows.append((content, line))
            offset += len(row)
        fence = None
        block = []
        enabled = False
        indented = False
        blank_before = True
        for row, line in rows:
            opening = re.match(r' {0,3}(`{3,}|~{3,})(.*)$', row)
            if fence:
                if re.fullmatch(r' {0,3}' + re.escape(fence[0]) + '{' + str(len(fence)) + r',}\s*', row):
                    if enabled:
                        yield '\n'.join(value for value, _ in block), [n for _, n in block]
                    block, fence = [], None
                else:
                    block.append((row, line))
            elif opening:
                if indented and block:
                    yield '\n'.join(value for value, _ in block), [n for _, n in block]
                block, indented = [], False
                fence = opening[1]
                enabled = rust_info(opening[2])
            elif (indented or blank_before) and (row.startswith('    ') or row.startswith('\t')):
                indented = True
                block.append((row[4:] if row.startswith('    ') else row[1:], line))
            elif indented and not row.strip():
                block.append(('', line))
            elif indented:
                yield '\n'.join(value for value, _ in block), [n for _, n in block]
                block, indented = [], False
            blank_before = not row.strip() or bool(opening) or fence is not None
        if block and (indented or enabled):
            yield '\n'.join(value for value, _ in block), [n for _, n in block]
