"""Regression controls for Rust token boundaries and independent Rustdoc tests."""
from pathlib import Path
import tempfile
import unittest

from tools.float_guard import CRATES, doc_examples, float_sites, violations


def examples(source):
    return [(lines[line - 1], text) for code, lines in doc_examples(source)
            for line, text in float_sites(code)]


def line_doc(code, fence='```rust', close='```', prefix='/// '):
    return '\n'.join(prefix + row for row in [fence, *code.splitlines(), close])


class ReviewControls(unittest.TestCase):
    def assert_contexts(self, code, expected):
        """Exercise file discovery as well as the token scanner for each crate."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for crate in CRATES:
                for folder, name, source in (
                    ('src', 'lib', code),
                    ('src', 'unit', '#[test]\nfn test() {\n' + code + '\n}'),
                    ('tests', 'integration', '#[test]\nfn test() {\n' + code + '\n}'),
                    ('src', 'docs', line_doc(code) + '\nfn documented() {}'),
                ):
                    path = (root / 'rust' / crate / folder / name).with_suffix('.rs')
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_text(source)
            found = violations(root)
            self.assertEqual(len(found), len(CRATES) * 4 * expected, found)

    def test_strings_keep_following_code_visible(self):
        for prefix in ('', 'b', 'c'):
            for contents in ('float f64 1.5', 'float \\\n f32 1.5', 'quote \\" f64'):
                with self.subTest(prefix=prefix, contents=contents):
                    code = f'let s = {prefix}"{contents}";'
                    self.assert_contexts(code, 0)
                    self.assert_contexts(code + ' let x = 1.5; let end = "end";', 1)
        for prefix in ('r', 'br', 'cr'):
            for hashes in ('', '#', '###'):
                contents = 'f64 1.5' if not hashes else 'f64 " 1.5 "' + hashes[:-1]
                code = f'let s = {prefix}{hashes}"{contents}"{hashes};'
                with self.subTest(prefix=prefix, hashes=hashes):
                    self.assert_contexts(code, 0)
                    self.assert_contexts(code + ' let x = 1.5; let end = "end";', 1)

    def test_range_bounds(self):
        for operator in ('..', '..='):
            for literal in ('1.5', '1e3', '1e-3', '1_f64', '1f32'):
                with self.subTest(operator=operator, literal=literal):
                    self.assert_contexts(f'let bound = {operator}{literal};', 1)
        self.assert_contexts('let ints = (0..5, ..8, ..=9, pair.0.1, pair . 0 . 1, '
                             '1.count_ones(), 0x1E3, 0b1_0, 0o77);', 0)

    def test_rustdoc_forms_and_attributes(self):
        for prefix in ('/// ', '//! '):
            for fence, close in (('```', '```'), ('~~~~rust', '~~~~'),
                                 ('````rust', '````'), ('  ```rust', '  ```'),
                                 ('```edition2024', '```'),
                                 ('```no_run,edition2024', '```'),
                                 ('```compile_fail,E0308', '```'),
                                 ('```ignore-x86_64', '```')):
                for code in ('let x = 1.5;', '# let x = 1.5;'):
                    with self.subTest(prefix=prefix, fence=fence, code=code):
                        source = line_doc(code, fence, close, prefix)
                        self.assertEqual(examples(source), [(2, '1.5')])
                        block = '/**\n' + '\n'.join(' * ' + row for row in
                                [fence, code, close]) + '\n */'
                        self.assertEqual(examples(block), [(3, '1.5')])
        self.assertEqual(examples('/// prose\n///\n///     let x = 1.5;'), [(3, '1.5')])
        self.assertEqual(examples('/**\n * prose\n *\n *     let x = 1.5;\n */'), [(4, '1.5')])
        for language in ('text', 'python', 'json', 'sh'):
            self.assertEqual(examples(line_doc('f64 1.5', '~~~' + language, '~~~')), [])

    def test_real_doc_boundaries_and_locations(self):
        source = '/**\n * ```rust\n * /* nested */\n * let x = 1.5;\n * ```\n */'
        self.assertEqual(examples(source), [(4, '1.5')])
        for prefix in ('/**', '/*!'):
            self.assertEqual(examples(prefix + '\n```rust\n/* nested /* comment */ */\n'
                                      'let x = 1.5;\n```\n*/'), [(4, '1.5')])
        fake = line_doc('let x = 1.5;')
        for container in ('r###"' + fake + '"###', '/*\n' + fake + '\n*/',
                          '/*\n/**\n' + fake + '\n*/\n*/'):
            self.assertEqual(examples(container), [])
        for attribute in ('#[doc = r"```rust\nlet x = 1.5;\n```"]',
                          '#![doc = r###"```rust\nlet x = 1.5;\n```"###]',
                          '#[doc = "```rust\\nlet x = 1.5;\\n```"]',
                          '#[doc = "```rust\\nlet x = 1\\u{2e}5;\\n```"]'):
            with self.subTest(attribute=attribute):
                self.assertEqual(len(examples(attribute)), 1)
        # Escaped newlines in a cooked attribute map to its physical source line.
        self.assertEqual(examples('#[doc = "```rust\\nlet x = 1.5;\\n```"]'), [(1, '1.5')])

    def test_independent_examples_and_items(self):
        for malformed in ('/*', '"unterminated', 'r#"unterminated'):
            first = line_doc(malformed, '```compile_fail')
            second = line_doc('let x = 1.5;')
            self.assertEqual(examples(first + '\n' + second), [(5, '1.5')])
            self.assertEqual(examples(first + '\nfn first() {}\n' + second), [(6, '1.5')])
        # An unclosed fence ends at the documented item, never the following one.
        first = '/// ```text\n/// some text\nfn first() {}\n'
        self.assertEqual(examples(first + line_doc('let x = 1.5;')), [(5, '1.5')])
        # A shorter fence cannot close the surrounding non-Rust block.
        self.assertEqual(examples(line_doc('```\nf64 1.5', '````text', '````')), [])

    def test_dotted_macro_literals(self):
        macro = 'macro_rules! value { (. $x:literal) => { $x }; }\n'
        for literal in ('1.5', '1e3', '1f64', '1_f32'):
            with self.subTest(literal=literal):
                self.assert_contexts(macro + f'let x = value!(. {literal});', 1)
        self.assert_contexts('macro_rules! value { () => { . 1.5 }; }', 1)
        self.assert_contexts('macro_rules! value { ($a:ident . $x:literal) => { $x }; }\n'
                             'let x = value!(receiver . 1.5);', 1)
        self.assert_contexts('assert_eq!((pair.0).1, 0);', 0)
        self.assert_contexts('let x = (pair.0.1, (pair).0.1, values[0].0.1);', 0)

    def test_mixed_documentation_forms(self):
        for middle in ('/** let x = 1.5; */', '#[doc = "let x = 1.5;"]'):
            source = '/// ```rust\n' + middle + '\n/// ```\nfn item() {}'
            self.assertEqual(examples(source), [(2, '1.5')])
        self.assertEqual(examples('/// ```rust\n/**/\n/// let x = 1.5;\n'
                                  '/* ordinary */\n/// ```\nfn item() {}'), [(3, '1.5')])
        self.assertEqual(examples('//! ```rust\n/*! let x = 1.5; */\n//! ```'), [(2, '1.5')])
        self.assertEqual(examples('/// ```text\nfn first() {}\n/** ```rust\n'
                                  'let x = 1.5;\n``` */\nfn second() {}'), [(4, '1.5')])

    def test_commonmark_containers_and_info_strings(self):
        for rows, expected_line in (
            (['> ```rust', '> let x = 1.5;', '> ```'], 2),
            (['- ```rust', '  let x = 1.5;', '  ```'], 2),
            (['- outer', '  - inner', '', '    > ~~~~rust',
              '    > let x = 1.5;', '    > ~~~~'], 5),
            (['> prose', '>', '>     let x = 1.5;'], 3),
        ):
            source = '\n'.join('/// ' + row for row in rows)
            with self.subTest(rows=rows):
                self.assertEqual(examples(source), [(expected_line, '1.5')])
        for info in ('{class=example}', '{.example}', '{class="hello world"}',
                     'rust,{class="custom, text"}', '"rust",{.example}',
                     'no_run,{class="example, custom"}'):
            with self.subTest(info=info):
                self.assertEqual(examples(line_doc('let x = 1.5;', '```' + info)), [(2, '1.5')])
        for info in ('custom,{class=rust}', 'custom,rust', 'custom,{.rust}',
                     '"not rust" {."hello everyone"}', 'text,{class=rust}'):
            with self.subTest(info=info):
                self.assertEqual(examples(line_doc('let x = 1.5;', '```' + info)), [])

    def test_conditional_literal_doc_attributes(self):
        for spelling in ('cfg_attr', 'r#cfg_attr'):
            for doc in ('doc', 'r#doc'):
                for marker in ('#', '#!'):
                    source = (marker + '[' + spelling + '(all(), allow(dead_code), '
                              + doc + ' = "```rust\\nlet x = 1.5;\\n```", '
                              'cfg_attr(any(), doc = r"```rust\nlet y = 2f64;\n```"))]')
                    with self.subTest(source=source):
                        self.assertEqual(examples(source), [(1, '1.5'), (2, '2f64')])
        source = ('/// ```rust\n#[cfg_attr(all(), doc = "let x = 1.5;")]\n'
                  '/// ```\nfn item() {}')
        self.assertEqual(examples(source), [(2, '1.5')])
        self.assertEqual(examples('#[cfg_attr(all(), deprecated(note = "```rust\\n'
                                  'let x = 1.5;\\n```"))]'), [])
