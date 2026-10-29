"""The pack builder reads and writes text as UTF-8 with `\\n` line ends on every system.

    PYTHONPATH=. python3 -m unittest tests.test_text_files -v

Players run `psiv_tools` on Windows and macOS too. There a text file opened
without `encoding` uses the locale's code page, and one written without
`newline="\\n"` gets `\\r\\n` line ends, so the same ROM would build a pack
with different bytes. This census reads every module under `psiv_tools/` and
refuses a `read_text` without `encoding`, a `write_text` without `encoding`
and `newline`, and an `open` in text mode without them.
"""
import ast
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PACKAGE = ROOT / "psiv_tools"


def _keywords(call):
    return {keyword.arg for keyword in call.keywords}


def _mode(call, position):
    for keyword in call.keywords:
        if keyword.arg == "mode":
            return keyword.value.value if isinstance(keyword.value, ast.Constant) else None
    if len(call.args) > position and isinstance(call.args[position], ast.Constant):
        return call.args[position].value
    return "r" if len(call.args) <= position else None


def unsafe_text_calls(source, name="<source>"):
    """`name:line: call` for each text read or write that leaves the encoding
    or the line end to the system."""
    found = []
    for node in ast.walk(ast.parse(source, name)):
        if not isinstance(node, ast.Call):
            continue
        func = node.func
        attr = func.attr if isinstance(func, ast.Attribute) else getattr(func, "id", None)
        keywords = _keywords(node)
        if attr == "read_text":
            needed = {"encoding"}
        elif attr == "write_text":
            needed = {"encoding", "newline"}
        elif attr == "open":
            # builtins.open(file, mode) and Path.open(mode)
            mode = _mode(node, 1 if isinstance(func, ast.Name) else 0)
            if mode is None:
                found.append(f"{name}:{node.lineno}: open with a mode the census cannot read")
                continue
            if "b" in mode:
                continue
            needed = {"encoding"} | ({"newline"} if set(mode) & set("wax+") else set())
        else:
            continue
        missing = needed - keywords
        if missing:
            found.append(f"{name}:{node.lineno}: {attr} without {', '.join(sorted(missing))}")
    return found


class TextFiles(unittest.TestCase):

    def test_psiv_tools_names_the_encoding_and_line_end_of_every_text_file(self):
        found = []
        for path in sorted(PACKAGE.rglob("*.py")):
            found += unsafe_text_calls(path.read_text(encoding="utf-8"),
                                       path.relative_to(ROOT).as_posix())
        self.assertEqual(found, [])

    def test_negative_control_the_census_finds_each_unsafe_call(self):
        source = "\n".join([
            "path.read_text()",
            "path.write_text(text, encoding='utf-8')",
            "open(name, 'w', encoding='utf-8')",
            "path.open()",
            "open(name, mode)",
            # safe ones
            "path.read_text(encoding='utf-8')",
            "path.write_text(text, encoding='utf-8', newline='\\n')",
            "path.open('rb')",
            "open(name, 'r', encoding='utf-8')",
            "path.read_bytes()",
        ])
        self.assertEqual(unsafe_text_calls(source), [
            "<source>:1: read_text without encoding",
            "<source>:2: write_text without newline",
            "<source>:3: open without newline",
            "<source>:4: open without encoding",
            "<source>:5: open with a mode the census cannot read",
        ])


if __name__ == "__main__":
    unittest.main()
