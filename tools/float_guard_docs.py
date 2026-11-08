"""Documentation tokens and independent Rustdoc examples for the float guard."""
import re

from markdown_it import MarkdownIt

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
            current_style = '!' if text[2] == '!' else '/'
            line = source.count('\n', 0, token.start) + 1
            chars = [(ch, line) for ch in text[3:].removeprefix(' ')]
            chars.append(('\n', source.count('\n', 0, token.start) + 1))
        elif token.kind == 'block' and (text.startswith('/*!') or
                (text.startswith('/**') and not text.startswith(('/***', '/**/')))):
            current_style = '!' if text[2] == '!' else '/'
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
            docs = list(literal_docs(attr[1:-1])) if attr and attr[0].text == '[' else []
            if docs:
                chars = []
                for literal in docs:
                    chars.extend(string_doc(literal, source))
                    chars.append(('\n', source.count('\n', 0, literal.end) + 1))
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


def literal_docs(parts):
    """Extract only doc literals, recursively descending cfg_attr attribute lists.

    Conditions are deliberately not evaluated: all source feature branches must
    obey the float guard, even when they aren't enabled on this machine.
    """
    if not parts:
        return
    name = parts[0].text.removeprefix('r#')
    if (name == 'doc' and len(parts) == 3 and parts[1].text == '='
            and parts[2].kind in ('raw', 'string')):
        yield parts[2]
    elif name == 'cfg_attr' and len(parts) > 3 and parts[1].text == '(':
        arguments, current, depth = [], [], 0
        for part in parts[2:-1]:
            if part.text == ',' and depth == 0:
                arguments.append(current)
                current = []
                continue
            depth += part.text in ('(', '[', '{')
            depth -= part.text in (')', ']', '}')
            current.append(part)
        arguments.append(current)
        for argument in arguments[1:]:
            yield from literal_docs(argument)


def rust_info(info):
    """Class groups decorate a block; custom overrides its Rust language tag."""
    # Quoted names are single words. Braced CSS classes are not language tags.
    words = re.findall(r'\{(?:[^"}]|"[^"]*")*\}|"[^"]*"|[^\s,]+', info)
    attrs = [word.strip('"') for word in words if not word.startswith('{')]
    if 'custom' in attrs:
        return False
    known = lambda attr: (attr in ('rust', 'no_run', 'ignore', 'should_panic',
                                  'compile_fail', 'test_harness', 'standalone_crate')
                          or re.fullmatch(r'edition(?:2015|2018|2021|2024)', attr)
                          or attr.startswith('ignore-') or re.fullmatch(r'E\d{4}', attr))
    return not attrs or 'rust' in attrs or all(known(attr) for attr in attrs)


def doc_examples(source):
    """Use CommonMark container/fence semantics and reset the lexer per block."""
    parser = MarkdownIt('commonmark')
    for chars in documentation(source):
        text = ''.join(ch for ch, _ in chars)
        lines = []
        offset = 0
        for row in text.splitlines(keepends=True):
            lines.append(chars[offset][1])
            offset += len(row)
        for block in parser.parse(text):
            if block.type not in ('fence', 'code_block') or not rust_info(block.info):
                continue
            start = block.map[0] + (block.type == 'fence')
            count = len(block.content.splitlines())
            yield block.content, lines[start:start + count]
