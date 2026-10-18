"""Every tracked Python file compiles without a warning.

    PYTHONPATH=. python3 -m unittest tests.test_python_compiles_clean -v

An invalid escape in an ordinary string (`"tools/godot\\.py"` outside a raw
string) is a SyntaxWarning today, printed on every fresh compile, and a
SyntaxError in a later Python. The case compiles each file of the change with
warnings promoted to errors and names every offender; the negative control is
the docstring `tools/check_paths.py` carried at 7327922.
"""
import unittest
import warnings
from pathlib import Path

from tools.repo_files import repo_files

ROOT = Path(__file__).resolve().parents[1]


def compile_problem(source: str, name: str) -> str | None:
    """The warning or error compiling `source` raises, or None when it is clean."""
    with warnings.catch_warnings():
        warnings.simplefilter("error")
        try:
            compile(source, name, "exec", dont_inherit=True)
        except (SyntaxError, SyntaxWarning, DeprecationWarning) as error:
            return f"{name}:{getattr(error, 'lineno', '?')}: {error}"
    return None


class CompilesClean(unittest.TestCase):

    def test_every_tracked_python_file(self):
        offenders = []
        for name in repo_files(ROOT, ("*.py",)):
            try:
                source = (ROOT / name).read_text(encoding="utf-8")
            except (OSError, UnicodeDecodeError):
                continue
            problem = compile_problem(source, name)
            if problem:
                offenders.append(problem)
        self.assertEqual(offenders, [], "\n".join(offenders))

    def test_negative_control_an_invalid_escape_in_a_docstring(self):
        # tools/check_paths.py's docstring at 7327922, line 44.
        old = '"""Paths.\n\n`r"tools/godot\\.py"` names `tools/godot.py`.\n"""\n'
        self.assertIn('invalid escape sequence', compile_problem(old, "check_paths.py") or "")


if __name__ == "__main__":
    unittest.main()
