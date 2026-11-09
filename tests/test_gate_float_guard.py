"""The gate covers float syntax even where Clippy suppresses its lint."""
from pathlib import Path
import tempfile
import unittest

from tools.float_guard import CRATES, float_sites, violations


class FloatGuardTests(unittest.TestCase):
    def test_retail_crates(self):
        root = Path(__file__).resolve().parents[1]
        for crate in CRATES:
            self.assertTrue((root / "rust" / crate / "src/lib.rs").is_file())
        self.assertEqual(violations(root), [])

    def test_negative_controls(self):
        for token in ('f32', 'f64', '1.5', '1.', '1e-3', '1E+3', '1_f32', '1f64', '1.0_f64'):
            with self.subTest(token=token):
                self.assertTrue(float_sites(f'let expected = 3 as {token};'))

    def test_non_float_syntax(self):
        self.assertEqual(float_sites('''
            // 1.5 and f32
            /* nested /* f64 */ 1.0 */
            let text = r###"1.5 f64"###;
            let text = "escaped \\" f32";
            let ch = 'x'; let borrow = &'a value;
            let x = (pair.0.1, 0..5, 1..=8, 0x1E3, 1.count_ones(), 3_u32);
        '''), [])

    def test_scans_unit_integration_and_doctest_negative_controls(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for crate in CRATES:
                for relative, source in (
                    ('src/lib.rs', '#[test]\nfn expected() { let x = 1.5; }'),
                    (str((Path('tests') / 'new').with_suffix('.rs')), '#[test]\nfn expected() { let x = 2 as f64; }'),
                    (str((Path('src') / 'docs').with_suffix('.rs')), '/// ```rust\n/// let x = 1e3;\n/// ```'),
                ):
                    path = root / 'rust' / crate / relative
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_text(source)
            self.assertEqual(len(violations(root)), 9)

    def test_doc_fences_and_block_docs(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for crate in CRATES:
                lib = root / 'rust' / crate / 'src/lib.rs'
                lib.parent.mkdir(parents=True)
                lib.write_text('')
            path = root / 'rust' / CRATES[0] / 'src/lib.rs'
            path.write_text("""/// ```text
/// 1.5 f64
/// ```
/// prose 1.5
/**
 * ```rust
 * let x = 1.5;
 * ```
 */
""")
            self.assertEqual(len(violations(root)), 1)
