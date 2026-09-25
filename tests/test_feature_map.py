"""Guard for `docs/FEATURE_MAP.md`: the feature map is a checked artifact.

    PYTHONPATH=. python3 -m unittest tests.test_feature_map -v

The feature map answers, per player-facing feature, which paths own it, how a
player reaches it in the native build, which focused check proves it and which
ledger records its evidence. A map that silently rots teaches wrong paths, so
this module parses its tables and fails on every way they can go stale:

- a table whose columns are not the five the map defines, or a row with an
  empty cell;
- an owning path no longer in the repository - a glob must still match at least
  one tracked file, and a directory must still hold one;
- a focused-test command naming a crate that is not a `rust/Cargo.toml`
  workspace member, a `--test` target with no `rust/<crate>/tests/<name>.rs`,
  a `tests.test_x` module with no `tests/test_x.py`, or a `--lib <module>::`
  filter whose module the crate does not have;
- a ledger link that no longer resolves, points outside `docs/`, or carries an
  anchor the target no longer defines;
- a `rust/psiv-runtime/tests/*.rs` target or a `tools/native/` driver with no
  row in the map. That coverage is half the map's value: a new integration
  target or driver fails here until its row lands beside it.

`check_map(path=...)` does the work and takes the map to read, so a case can
point it at a copy: `NegativeControl` renames a referenced test target in a
copy of the map and shows the renamed target named in the failure.
"""
from __future__ import annotations

import fnmatch
import re
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FEATURE_MAP = ROOT / "docs" / "FEATURE_MAP.md"
CARGO_MANIFEST = ROOT / "rust" / "Cargo.toml"

COLUMNS = ("Feature", "Owning paths", "Player reach", "Focused test", "Ledger")
DOCS_PREFIX = "docs/"
RUNTIME_TESTS_PREFIX = "rust/psiv-runtime/tests/"
NATIVE_DRIVERS_PREFIX = "tools/native/"

ROW_RE = re.compile(r"\A\s*\|(?P<cells>.*)\|\s*\Z")
SEPARATOR_RE = re.compile(r"\A\s*\|[\s:|-]+\|\s*\Z")
INLINE_CODE_RE = re.compile(r"`([^`]+)`")
LINK_RE = re.compile(r"\[[^\]]*\]\((?P<target>[^()\s]+)\)")
CRATE_RE = re.compile(r"(?:\A|\s)-p[ \t]+(?P<crate>[A-Za-z0-9_.-]+)")
TARGET_RE = re.compile(r"(?:\A|\s)--test[ \t]+(?P<name>[A-Za-z0-9_.-]+)")
MODULE_RE = re.compile(r"\btests\.(?P<name>test_[A-Za-z0-9_]+)")
FILTER_RE = re.compile(r"(?:\A|\s)(?P<module>[A-Za-z0-9_]+(?:::[A-Za-z0-9_]+)*)::")
MEMBERS_RE = re.compile(r"members\s*=\s*\[(?P<list>[^\]]*)\]")


# --------------------------------------------------------------------- inputs

def git_paths(*patterns: str) -> list[str]:
    """Tracked paths at the repository root, optionally filtered by pattern."""
    proc = subprocess.run(
        ["git", "ls-files", "-z", *patterns],
        cwd=ROOT, capture_output=True, text=True, check=True,
    )
    return [name for name in proc.stdout.split("\0") if name]


def workspace_members() -> set[str]:
    """The `members` list of `rust/Cargo.toml`, as crate directory names."""
    match = MEMBERS_RE.search(CARGO_MANIFEST.read_text(encoding="utf-8"))
    return set(re.findall(r'"([^"]+)"', match["list"])) if match else set()


def has_glob(token: str) -> bool:
    """True when `token` is a pattern rather than one path."""
    return "*" in token or "?" in token


def resolves(token: str, tracked: list[str], names: set[str]) -> bool:
    """True when `token` names tracked content: a file, a directory or a glob."""
    if has_glob(token):
        return any(fnmatch.fnmatchcase(name, token) for name in tracked)
    if token in names:
        return True
    prefix = token.rstrip("/") + "/"
    return any(name.startswith(prefix) for name in tracked)


def covers(pattern: str, path: str) -> bool:
    """True when a map token names `path`, literally or as a glob."""
    return pattern == path or (has_glob(pattern) and fnmatch.fnmatchcase(path, pattern))


def heading_slugs(text: str) -> set[str]:
    """GitHub's anchors for one Markdown file, the way `tools/check_docs.py` reads them."""
    slugs = set()
    fence = None
    for line in text.splitlines():
        if fence is not None:
            if re.match(rf"\A {{0,3}}{re.escape(fence)}+\s*\Z", line):
                fence = None
            continue
        opened = re.match(r"\A {0,3}(`{3,}|~{3,})", line)
        if opened:
            fence = opened[1]
            continue
        heading = re.match(r"\A {0,3}#{1,6}[ \t]+(?P<text>.*?)[ \t]*#*\s*\Z", line)
        if heading is None:
            continue
        kept = [char for char in heading["text"].lower() if char.isalnum() or char in " -_"]
        slug = "".join(kept).replace(" ", "-")
        if slug:
            slugs.add(slug)
    return slugs


# -------------------------------------------------------------------- parsing

def table_rows(text: str) -> tuple[list[tuple[int, dict[str, str]]], list[str]]:
    """`(rows, problems)`: feature-table rows as `(line, {column: cell})`."""
    lines = text.splitlines()
    rows: list[tuple[int, dict[str, str]]] = []
    problems: list[str] = []
    index = 0
    while index < len(lines):
        header = ROW_RE.match(lines[index])
        if not header or index + 1 >= len(lines) or not SEPARATOR_RE.match(lines[index + 1]):
            index += 1
            continue
        columns = [cell.strip() for cell in header["cells"].split("|")]
        expected = tuple(columns) == COLUMNS
        if "Feature" in columns and not expected:
            problems.append(
                f"line {index + 1}: unexpected table columns: {' | '.join(columns)}"
            )
        index += 2
        while index < len(lines) and ROW_RE.match(lines[index]):
            if expected:
                cells = [cell.strip() for cell in ROW_RE.match(lines[index])["cells"].split("|")]
                if len(cells) != len(COLUMNS):
                    problems.append(
                        f"line {index + 1}: {len(cells)} cells, expected {len(COLUMNS)}"
                    )
                else:
                    rows.append((index + 1, dict(zip(COLUMNS, cells))))
            index += 1
    return rows, problems


def inline_code(text: str) -> list[str]:
    """The contents of one cell's code spans, in order."""
    return INLINE_CODE_RE.findall(text)


def module_file(module: str, crates: set[str], names: set[str]) -> bool:
    """True when some crate of `crates` has the Rust module `module`."""
    parts = module.split("::")
    for crate in crates:
        for cut in range(len(parts), 0, -1):
            base = f"rust/{crate}/src/{'/'.join(parts[:cut])}"
            if f"{base}.rs" in names or f"{base}/mod.rs" in names:
                return True
    return False


def command_problems(command: str, members: set[str], names: set[str]) -> list[str]:
    """Every way one focused-test command names something that is not there."""
    problems: list[str] = []
    crates = set(CRATE_RE.findall(command))
    for crate in sorted(crates):
        if crate not in members:
            problems.append(f"crate is not a rust/Cargo.toml member: -p {crate}")
    for name in TARGET_RE.findall(command):
        for crate in sorted(crates):
            target = f"rust/{crate}/tests/{name}.rs"
            if target not in names:
                problems.append(f"no such test target: {target}")
    for name in MODULE_RE.findall(command):
        module = f"tests/{name}.py"
        if module not in names:
            problems.append(f"no such Python test module: {module}")
    for module in FILTER_RE.findall(command):
        if not module_file(module, crates, names):
            problems.append(f"filter names a module the crate does not have: {module}::")
    if not crates and not MODULE_RE.search(command):
        problems.append(f"names no crate or Python module to check: {command}")
    return problems


def link_problems(target: str, text: str, documents: Path, names: set[str]) -> list[str]:
    """Every way one ledger link fails to resolve under `documents`."""
    path, _, fragment = target.partition("#")
    if not path:  # an anchor in the map itself
        if fragment and fragment not in heading_slugs(text):
            return [f"ledger anchor is gone: #{fragment}"]
        return []
    resolved = (documents / path).resolve()
    if not resolved.is_file():
        return [f"ledger link does not resolve: {target}"]
    try:
        relative = str(resolved.relative_to(ROOT.resolve()))
    except ValueError:
        return []  # a copy read from another directory: the link resolves there
    if relative not in names:
        return [f"ledger link is not tracked: {target}"]
    if not relative.startswith(DOCS_PREFIX):
        return [f"ledger link is outside {DOCS_PREFIX}: {target}"]
    if fragment and fragment not in heading_slugs(resolved.read_text(encoding="utf-8")):
        return [f"ledger anchor is gone: {target}"]
    return []


def ledger_root(path: Path) -> Path:
    """Where `path`'s relative links resolve: its own directory, or `docs/` for a copy."""
    directory = Path(path).resolve().parent
    try:
        directory.relative_to(ROOT.resolve())
    except ValueError:
        return ROOT / DOCS_PREFIX  # a copy outside the checkout keeps the repository's docs
    return directory


# --------------------------------------------------------------------- checks

def check_map(path: Path = FEATURE_MAP, tracked: list[str] | None = None) -> list[str]:
    """`(problems)` for the feature tables in `path`, empty when the map is true."""
    if tracked is None:
        tracked = git_paths()
    names = set(tracked)
    text = Path(path).read_text(encoding="utf-8")
    documents = ledger_root(path)
    rows, problems = table_rows(text)

    def problem(line: int, message: str) -> None:
        """Record one problem against the row (`line`) that has it."""
        problems.append(f"{path}:{line}: {message}")

    if not rows:
        problems.append(f"{path}: no table with the expected columns: {' | '.join(COLUMNS)}")
    members = workspace_members()
    features: set[str] = set()
    targets_seen: set[str] = set()
    tokens_seen: set[str] = set(inline_code(text))
    for line, cells in rows:
        for column in COLUMNS:
            if not cells.get(column):
                problem(line, f"{column} is empty")
        for token in inline_code(cells.get("Owning paths", "")):
            if not resolves(token, tracked, names):
                problem(line, f"owning path is not in the repository: {token}")
        commands = inline_code(cells.get("Focused test", ""))
        if not commands:
            problem(line, "Focused test names no command")
        for command in commands:
            for message in command_problems(command, members, names):
                problem(line, message)
            targets_seen |= set(TARGET_RE.findall(command))
        links = LINK_RE.findall(cells.get("Ledger", ""))
        if not links:
            problem(line, "Ledger names no link")
        for target in links:
            for message in link_problems(target, text, documents, names):
                problem(line, message)
        if cells.get("Feature") in features:
            problem(line, f"duplicate feature row: {cells['Feature']}")
        features.add(cells.get("Feature", ""))
    for target in sorted(name for name in tracked if name.startswith(RUNTIME_TESTS_PREFIX)):
        stem = Path(target).stem
        if stem not in targets_seen:
            problems.append(f"{path}: {target}: no row runs this target (--test {stem})")
    for driver in sorted(name for name in tracked if name.startswith(NATIVE_DRIVERS_PREFIX)):
        if not any(covers(token, driver) for token in tokens_seen):
            problems.append(f"{path}: {driver}: no row lists this native driver")
    return problems


class RealMap(unittest.TestCase):
    """The committed map: the case the gate runs on every change."""

    def test_the_committed_map_has_no_problems(self):
        problems = check_map()
        self.assertEqual(problems, [], "\n".join(problems))

    def test_every_runtime_test_target_is_covered(self):
        rows, _ = table_rows(FEATURE_MAP.read_text(encoding="utf-8"))
        targets = set()
        for _, cells in rows:
            targets |= set(TARGET_RE.findall(cells["Focused test"]))
        expected = {
            Path(name).stem
            for name in git_paths(f"{RUNTIME_TESTS_PREFIX}*.rs")
        }
        self.assertTrue(expected, "no psiv-runtime integration targets found")
        self.assertEqual(expected - targets, set())

    def test_the_map_is_a_table_per_area_with_rows(self):
        rows, _ = table_rows(FEATURE_MAP.read_text(encoding="utf-8"))
        self.assertGreaterEqual(len(rows), 40)
        self.assertEqual(len({cells["Feature"] for _, cells in rows}), len(rows))


class Wiring(unittest.TestCase):
    """The map is reachable from the documents that send readers to it."""

    def test_the_documentation_index_links_the_map(self):
        index = (ROOT / "docs" / "README.md").read_text(encoding="utf-8")
        self.assertIn("FEATURE_MAP.md", index)

    def test_the_repository_instructions_point_at_the_map(self):
        instructions = (ROOT / "AGENTS.md").read_text(encoding="utf-8")
        self.assertIn("docs/FEATURE_MAP.md", instructions)

    def test_the_map_stays_under_the_file_size_limit(self):
        self.assertLessEqual(len(FEATURE_MAP.read_text(encoding="utf-8").splitlines()), 1000)


class NegativeControl(unittest.TestCase):
    """A rotted copy is named by the check rather than accepted.

    Each case writes a copy of the committed map, points `check_map` at it
    through its `path` argument, and asserts the specific thing it broke is in
    the failure. `RenamedTarget` is the control the map's coverage claim needs:
    a test target renamed in the map must be named in the result.
    """

    def copy_with(self, replace: tuple[str, str]) -> list[str]:
        """`check_map`'s problems for a copy of the map with one replacement."""
        text = FEATURE_MAP.read_text(encoding="utf-8")
        broken = text.replace(*replace)
        self.assertNotEqual(text, broken, f"the map no longer contains {replace[0]!r}")
        with tempfile.TemporaryDirectory() as directory:
            copy = Path(directory) / "FEATURE_MAP.md"
            copy.write_text(broken, encoding="utf-8")
            return check_map(copy)

    def test_renaming_a_referenced_test_target_fails_and_names_it(self):
        problems = self.copy_with(("--test chest_slots", "--test chest_slots_renamed"))
        self.assertTrue(problems)
        self.assertTrue(
            any("rust/psiv-runtime/tests/chest_slots_renamed.rs" in item for item in problems),
            problems,
        )
        self.assertTrue(
            any("rust/psiv-runtime/tests/chest_slots.rs" in item for item in problems),
            problems,
        )

    def test_a_broken_ledger_link_is_reported(self):
        problems = self.copy_with(("field/CHESTS.md", "field/CHESTS_MOVED.md"))
        self.assertTrue(any("field/CHESTS_MOVED.md" in item for item in problems), problems)

    def test_a_moved_owning_path_is_reported(self):
        problems = self.copy_with(("rust/psiv-core/src/chest.rs", "rust/psiv-core/src/chests.rs"))
        self.assertTrue(
            any("rust/psiv-core/src/chests.rs" in item for item in problems), problems
        )

    def test_an_unlisted_native_driver_is_reported(self):
        problems = self.copy_with(("native_chests.gd", "native_chests_replaced.gd"))
        self.assertTrue(
            any("tools/native/native_chests.gd" in item for item in problems), problems
        )


class Parsing(unittest.TestCase):
    """The small rules the checks are built on, on data built here."""

    def test_rows_are_read_from_a_table_with_the_expected_columns(self):
        text = (
            "| Feature | Owning paths | Player reach | Focused test | Ledger |\n"
            "| --- | --- | --- | --- | --- |\n"
            "| Chests | `rust/psiv-core/src/chest.rs` | Open one | `tests.test_shops` | [x](README.md) |\n"
        )
        rows, problems = table_rows(text)
        self.assertEqual(problems, [])
        self.assertEqual([line for line, _ in rows], [3])
        self.assertEqual(rows[0][1]["Feature"], "Chests")

    def test_a_table_with_other_columns_is_a_problem_not_rows(self):
        text = "| Feature | Paths |\n| --- | --- |\n| Chests | x |\n"
        rows, problems = table_rows(text)
        self.assertEqual(rows, [])
        self.assertEqual(len(problems), 1)

    def test_a_short_row_is_reported_with_its_cell_count(self):
        text = (
            "| Feature | Owning paths | Player reach | Focused test | Ledger |\n"
            "| --- | --- | --- | --- | --- |\n"
            "| Chests | `rust/psiv-core/src/chest.rs` | Open one |\n"
        )
        rows, problems = table_rows(text)
        self.assertEqual(rows, [])
        self.assertTrue(any("3 cells, expected 5" in problem for problem in problems), problems)

    def test_paths_resolve_as_files_directories_and_globs(self):
        tracked = ["docs/README.md", "rust/psiv-core/src/chest.rs"]
        names = set(tracked)
        self.assertTrue(resolves("docs/README.md", tracked, names))
        self.assertTrue(resolves("rust/psiv-core/src/", tracked, names))
        self.assertTrue(resolves("rust/psiv-core/src/*.rs", tracked, names))
        self.assertFalse(resolves("rust/psiv-core/src/gone.rs", tracked, names))
        self.assertFalse(resolves("rust/psiv-core/src/*.py", tracked, names))

    def test_commands_name_a_crate_a_target_and_a_module(self):
        names = {"rust/psiv-core/tests/warps.rs", "tests/test_shops.py"}
        members = {"psiv-core", "psiv-runtime"}
        good = (
            "CARGO_BUILD_JOBS=1 cargo test --manifest-path rust/Cargo.toml "
            "-p psiv-core --test warps -- --test-threads=1"
        )
        self.assertEqual(command_problems(good, members, names), [])
        self.assertEqual(command_problems("PYTHONPATH=. python3 -m unittest tests.test_shops", members, names), [])
        broken = command_problems("-p psiv-godot --test warps", members, names)
        self.assertTrue(any("rust/psiv-godot/tests/warps.rs" in item for item in broken), broken)
        self.assertTrue(any("not a rust/Cargo.toml member" in item for item in broken), broken)
        self.assertTrue(command_problems("python3 -m unittest tests.test_gone", members, names))

    def test_a_lib_filter_names_a_module_the_crate_has(self):
        names = {"rust/psiv-godot/src/camp.rs", "rust/psiv-godot/src/battle/layout.rs"}
        members = {"psiv-godot"}
        command = "cargo test --manifest-path rust/Cargo.toml -p psiv-godot --lib camp:: -- --test-threads=1"
        self.assertEqual(command_problems(command, members, names), [])
        command = "cargo test --manifest-path rust/Cargo.toml -p psiv-godot --lib battle::layout::"
        self.assertEqual(command_problems(command, members, names), [])
        command = "cargo test --manifest-path rust/Cargo.toml -p psiv-godot --lib camps::"
        self.assertTrue(command_problems(command, members, names))


if __name__ == "__main__":
    unittest.main()
