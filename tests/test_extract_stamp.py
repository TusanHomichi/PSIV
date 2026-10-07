"""`generated/` refuses an extract the current extractor did not write (#103).

    PYTHONPATH=. python3 -m unittest tests.test_extract_stamp -v

The loader, the stamp and the producers live in `psiv_tools/extract_stamp.py`.
Fixtures here are tiny synthetic directories the tests write themselves: no
ROM-derived table or pixel is committed. Where a producer needs the ROM the
test skips without it, the way the other extractor tests do.
"""
from __future__ import annotations

import ast
import json
import pathlib
import re
import tempfile
import unittest
from unittest import mock

from oracle.force.errors import ForceError
from oracle.force.pack import Pack
from oracle.sweep import coverage, replay_pack, route_abilities
from psiv_tools import core, dialogue_census
from psiv_tools import extract_stamp as stamp
from psiv_tools.extract_stamp import StaleExtractError, load_table

ROOT = pathlib.Path(__file__).resolve().parent.parent
ROM = ROOT / "Phantasy Star IV (USA).md"


def write_json(path: pathlib.Path, value) -> None:
    path.write_text(json.dumps(value) + "\n", encoding="utf-8")


class TempDirectory(unittest.TestCase):
    def setUp(self):
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        self.root = pathlib.Path(scratch.name)
        self.directory = self.root / "generated"
        self.directory.mkdir()


class Loader(TempDirectory):
    def test_a_matching_stamp_loads_the_table(self):
        write_json(self.directory / "enemies.json", [{"id": 1}])
        stamp.write_table_stamp(self.directory)
        self.assertEqual(load_table(self.directory, "enemies"), [{"id": 1}])
        self.assertEqual(stamp.load_table_file(self.directory / "enemies.json"), [{"id": 1}])

    def test_a_mismatched_stamp_is_refused_naming_the_file_and_the_rebuild_command(self):
        write_json(self.directory / "enemies.json", [])
        old = stamp.table_stamp()
        old = {**old, "stamp": "0" * 64,
               "modules": {**old["modules"], "psiv_tools/formations.py": "1" * 64}}
        write_json(self.directory / "metadata.json", {"extract_stamp": old})
        with self.assertRaises(StaleExtractError) as caught:
            load_table(self.directory, "enemies")
        message = str(caught.exception)
        self.assertIn(str(self.directory / "enemies.json"), message)
        self.assertIn("0" * 16, message, "the stamp it found")
        self.assertIn(stamp.table_stamp()["stamp"][:16], message, "the stamp it expected")
        self.assertIn("psiv_tools/formations.py", message, "the module that changed")
        self.assertIn("python3 -m psiv_tools regenerate", message)
        self.assertEqual(caught.exception.command, stamp.REBUILD_COMMAND)

    def test_an_extract_from_before_stamping_is_refused(self):
        write_json(self.directory / "enemies.json", [])
        for metadata in (None, {}, {"extract_stamp": "x"}):
            with self.subTest(metadata=metadata):
                path = self.directory / "metadata.json"
                path.unlink(missing_ok=True)
                if metadata is not None:
                    write_json(path, metadata)
                with self.assertRaises(StaleExtractError) as caught:
                    load_table(self.directory, "enemies")
                self.assertIn("no stamp", str(caught.exception))

    def test_a_directory_or_table_that_is_absent_keeps_its_skip_behaviour(self):
        # The gate runs with no generated/: readers that skip on this error
        # must keep skipping, and must not be told the extract is stale.
        with self.assertRaises(FileNotFoundError):
            load_table(self.root / "nowhere", "enemies")
        stamp.write_table_stamp(self.directory)
        with self.assertRaises(FileNotFoundError):
            load_table(self.directory, "enemies")
        self.assertFalse(issubclass(StaleExtractError, OSError))

    def test_the_stamp_follows_the_extractor_source(self):
        # A synthetic package: `core` reaches `a` by a relative import, `b` by a
        # lazy one inside a function and `sub.c` through a package; `unused`
        # is reached by nothing, and `extract_stamp` is the stamp's own module.
        package = self.root / "psiv_tools"
        (package / "sub").mkdir(parents=True)
        files = {
            "__init__.py": "",
            "core.py": "from .a import x\n\ndef run():\n    from . import b\n    from .sub.c import z\n",
            "a.py": "x = 1\n", "b.py": "y = 1\n", "unused.py": "u = 1\n",
            "extract_stamp.py": "s = 1\n",
            "sub/__init__.py": "", "sub/c.py": "z = 1\n",
        }
        for name, text in files.items():
            (package / name).write_text(text)
        reached = stamp.source_modules("psiv_tools.core", package)
        self.assertEqual(sorted(reached), ["psiv_tools", "psiv_tools.a", "psiv_tools.b",
                                           "psiv_tools.core", "psiv_tools.sub",
                                           "psiv_tools.sub.c"])
        before = stamp.source_stamp("tables", package)
        (package / "unused.py").write_text("u = 2\n")
        (package / "extract_stamp.py").write_text("s = 2\n")
        self.assertEqual(stamp.source_stamp("tables", package), before,
                         "nothing the extractor cannot reach changes the stamp")
        for name in ("a.py", "b.py", "sub/c.py", "core.py"):
            with self.subTest(changed=name):
                text = (package / name).read_text()
                (package / name).write_text(text + "# changed\n")
                after = stamp.source_stamp("tables", package)
                self.assertNotEqual(after["stamp"], before["stamp"])
                self.assertEqual(stamp.describe_difference(before, after),
                                 [f"psiv_tools/{name}"])
                (package / name).write_text(text)
        self.assertEqual(stamp.source_stamp("tables", package), before)

    def test_the_real_extractor_graph_reaches_what_the_tables_come_from(self):
        reached = set(stamp.source_stamp("tables")["modules"])
        for module in ("core", "formations", "gfx", "planes", "shops", "text",
                       "symbols", "nemesis", "kosinski", "enigma", "maps/__init__"):
            self.assertIn(f"psiv_tools/{module}.py", reached)
        self.assertNotIn("psiv_tools/extract_stamp.py", reached)
        for kind in stamp.KIND_ENTRY_MODULES:
            with self.subTest(kind=kind):
                self.assertEqual(stamp.source_stamp(kind), stamp.source_stamp(kind),
                                 "a stamp is deterministic")


class PngDirectories(TempDirectory):
    def test_a_stamped_directory_passes_and_a_foreign_one_is_refused(self):
        stamp.write_png_stamp(self.directory, "gfx")
        stamp.check_png_directory(self.directory, "gfx")
        with self.assertRaises(StaleExtractError) as caught:
            stamp.check_png_directory(self.directory, "planes")
        self.assertIn(str(self.directory / stamp.STAMP_FILE), str(caught.exception))
        self.assertIn("python3 -m psiv_tools regenerate", str(caught.exception))
        write_json(self.directory / stamp.STAMP_FILE,
                   {**stamp.source_stamp("gfx"), "stamp": "f" * 64})
        with self.assertRaises(StaleExtractError):
            stamp.check_png_directory(self.directory, "gfx")
        (self.directory / stamp.STAMP_FILE).unlink()
        with self.assertRaises(StaleExtractError):
            stamp.check_png_directory(self.directory, "gfx")

    def test_a_directory_that_grows_one_image_at_a_time_cannot_inherit_a_stamp(self):
        (self.directory / "old.png").write_bytes(b"png")
        with self.assertRaises(StaleExtractError):
            stamp.check_accumulating_png_directory(self.directory, "layouts", {"new.png"})
        stamp.check_accumulating_png_directory(self.directory, "layouts", {"old.png"})
        stamp.write_png_stamp(self.directory, "layouts")
        stamp.check_accumulating_png_directory(self.directory, "layouts", {"new.png"})

    def test_discarding_a_stamp_leaves_an_interrupted_export_unvouched(self):
        stamp.write_png_stamp(self.directory, "gfx")
        stamp.discard_png_stamp(self.directory)
        with self.assertRaises(StaleExtractError):
            stamp.check_png_directory(self.directory, "gfx")


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class Producers(TempDirectory):
    """Each exporter stamps the directory it writes (the real ROM, no committed pixels)."""

    @classmethod
    def setUpClass(cls):
        cls.data = ROM.read_bytes()

    def test_the_art_planes_and_battle_art_exporters_stamp_their_directories(self):
        from psiv_tools.battle_art import export_battle_art_pngs
        from psiv_tools.gfx import export_art_pngs
        from psiv_tools.planes import export_plane_pngs

        for kind, export in (("gfx", export_art_pngs), ("planes", export_plane_pngs),
                             ("battle_art", export_battle_art_pngs)):
            with self.subTest(kind=kind):
                target = self.root / kind
                export(self.data, target)
                stamp.check_png_directory(target, kind)


class WriteExtract(TempDirectory):
    """`write_extract` stamps `metadata.json`, and writes it last."""

    KEYS = ["metadata", "layout_validation", "tables", "characters", "techniques", "skills",
            "combos", "vehicles", "items", "enemies", "enemy_skills", "progression",
            "formations", "formation_indexes", "shops", "graphics", "names", "dialogue",
            "maps", "encounters", "planes"]

    def fake_result(self, without=()):
        return {key: {"key": key} for key in self.KEYS if key not in without}

    def test_the_written_extract_carries_the_stamp_and_loads(self):
        with mock.patch.object(core, "extract_all", return_value=self.fake_result()):
            core.write_extract(b"", self.directory)
        metadata = json.loads((self.directory / "metadata.json").read_text())
        self.assertEqual(metadata["extract_stamp"], stamp.table_stamp())
        self.assertEqual(load_table(self.directory, "enemies"), {"key": "enemies"})

    def test_an_extract_that_stops_half_way_is_not_vouched_for(self):
        with mock.patch.object(core, "extract_all", return_value=self.fake_result()):
            core.write_extract(b"", self.directory)
        # A second run dies before it reaches `planes`; the first run's stamp
        # must not survive over the tables the second run half rewrote.
        with mock.patch.object(core, "extract_all",
                               return_value=self.fake_result(without=("planes",))):
            with self.assertRaises(KeyError):
                core.write_extract(b"", self.directory)
        self.assertFalse((self.directory / "metadata.json").exists())
        with self.assertRaises(StaleExtractError):
            load_table(self.directory, "enemies")


class EveryReaderRefusesAStaleExtract(TempDirectory):
    """The negative control, reader by reader: a table with no valid stamp."""

    def setUp(self):
        super().setUp()
        for name in ("formations", "formation_indexes", "encounters", "enemies",
                     "enemy_skills", "characters", "progression", "maps", "dialogue"):
            write_json(self.directory / f"{name}.json", {})
        write_json(self.directory / "metadata.json", {"extract_stamp": {"stamp": "0" * 64}})

    def test_the_battle_forcing_pack(self):
        with self.assertRaises(ForceError) as caught:
            Pack.load(self.directory)
        self.assertIn("python3 -m psiv_tools regenerate", str(caught.exception))

    def test_the_route_abilities_tables(self):
        with self.assertRaises(StaleExtractError):
            route_abilities.Data.load(self.directory)

    def test_the_replay_pack_builder(self):
        with self.assertRaises(StaleExtractError):
            replay_pack.build(self.directory, self.root, self.root)

    def test_the_sweep_coverage_enemy_names(self):
        with self.assertRaises(StaleExtractError):
            coverage.enemy_names(self.directory / "enemies.json")

    def test_the_dialogue_census(self):
        with self.assertRaises(StaleExtractError):
            dialogue_census.load(self.directory / "dialogue.json")


# ---------------------------------------------------------------------------
# The guard: nothing but the loader opens a generated/ table
# ---------------------------------------------------------------------------
#: What the repository calls the extract directory in code that reads it.
GENERATED_NAMES = re.compile(r"generated|GENERATED|data_dir|DATA_DIR")
READ_METHODS = {"read_text", "read_bytes", "open"}
SKIPPED_DIRECTORIES = ("rust/", "build/", "runtime-pack/", "reference/", "oracle/gpgx-src/",
                       "godot/", "saves/", "docs/")
#: The loader owns the door; this file quotes the shapes it forbids.
EXEMPT_FILES = {"psiv_tools/extract_stamp.py", "tests/test_extract_stamp.py"}


def opens_for_reading(call: ast.Call, mode_position: int) -> bool:
    """`open(path)` and `open(path, "rb")` read; a literal mode with `w`, `a` or `x` writes."""
    mode = call.args[mode_position] if len(call.args) > mode_position else next(
        (k.value for k in call.keywords if k.arg == "mode"), None)
    return not (isinstance(mode, ast.Constant) and isinstance(mode.value, str)
                and set(mode.value) & set("wax"))


def is_read(call: ast.Call) -> bool:
    """`json.load(s)`, `<path>.read_text()`/`.read_bytes()`/`.open()` or `open(...)`."""
    function = call.func
    if isinstance(function, ast.Name):
        return function.id == "open" and opens_for_reading(call, 1)
    if not isinstance(function, ast.Attribute):
        return False
    if function.attr in ("load", "loads"):
        return isinstance(function.value, ast.Name) and function.value.id == "json"
    if function.attr == "open":
        return opens_for_reading(call, 0)
    return function.attr in READ_METHODS


def mentions(node: ast.AST, tainted: set[str]) -> bool:
    """Whether `node` names the extract directory: by its usual names, or by a name derived from one.

    The name of the function a call invokes is not a mention (`write_generated(...)`);
    its receiver and its arguments are.
    """
    if isinstance(node, ast.Call):
        parts = [*node.args, *(k.value for k in node.keywords)]
        if isinstance(node.func, ast.Attribute):
            parts.append(node.func.value)
        return any(mentions(part, tainted) for part in parts)
    if GENERATED_NAMES.search(ast.unparse(node)):
        return True
    return any(isinstance(sub, ast.Name) and sub.id in tainted for sub in ast.walk(node))


def is_path_expression(node: ast.AST) -> bool:
    """A value that is still a path (`dir / name`, `Path(dir)`, `str(dir)`), not what was read from one."""
    if isinstance(node, ast.BinOp):
        return isinstance(node.op, ast.Div)
    if isinstance(node, ast.Call):
        function = node.func
        name = function.attr if isinstance(function, ast.Attribute) else getattr(function, "id", "")
        return name in {"Path", "PurePath", "str", "join", "resolve", "joinpath", "absolute"}
    return isinstance(node, (ast.Name, ast.Attribute, ast.Constant, ast.JoinedStr))


def own_nodes(scope: ast.AST):
    """Every node of `scope` outside the functions, lambdas and classes nested in it."""
    stack = list(ast.iter_child_nodes(scope))
    while stack:
        node = stack.pop()
        yield node
        if not isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.Lambda, ast.ClassDef)):
            stack.extend(ast.iter_child_nodes(node))


def bare_reads(source: str) -> list[tuple[int, str]]:
    """`(line, code)` of every read of a path that is, or derives from, the extract directory.

    A read is `json.load`/`json.loads`, `.read_text()`, `.read_bytes()`,
    `.open()` or `open(...)`, or a call to a helper defined in the same file
    whose body performs one (`read = lambda p: json.loads(p.read_text())`). A
    path is the extract directory's when its source names it (`generated`,
    `GENERATED`, `data_dir`, ...), or names a variable of the enclosing
    functions that was assigned from one, or defaulted from one.

    What it cannot see is a read through a generic name that never touched one
    of those (`path.read_text()` on a parameter the caller fills): the other
    half of the guard is that the loader is the readers' one import, and
    `EveryReaderRefusesAStaleExtract` calls each reader with a stale table.
    """
    found: list[tuple[int, str]] = []

    def visit(scope: ast.AST, inherited: set[str], helpers: set[str]) -> None:
        tainted = set(inherited)
        helpers = set(helpers)
        if isinstance(scope, (ast.FunctionDef, ast.AsyncFunctionDef, ast.Lambda)):
            arguments = scope.args
            positional = arguments.posonlyargs + arguments.args
            pairs = list(zip(reversed(positional), reversed(arguments.defaults)))
            pairs += [(arg, default) for arg, default in zip(arguments.kwonlyargs, arguments.kw_defaults)
                      if default is not None]
            for arg, default in pairs:
                if mentions(default, tainted):
                    tainted.add(arg.arg)
            for arg in positional + arguments.kwonlyargs:
                if GENERATED_NAMES.search(arg.arg):
                    tainted.add(arg.arg)
        nodes = list(own_nodes(scope))
        for node in nodes:  # local reader helpers, then taint, in source order
            if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and any(
                    isinstance(sub, ast.Call) and is_read(sub) for sub in ast.walk(node)):
                helpers.add(node.name)
            if isinstance(node, ast.Assign) and isinstance(node.value, ast.Lambda) and any(
                    isinstance(sub, ast.Call) and is_read(sub) for sub in ast.walk(node.value)):
                helpers.update(t.id for t in node.targets if isinstance(t, ast.Name))
        for node in sorted((n for n in nodes if isinstance(n, ast.Assign)), key=lambda n: n.lineno):
            if is_path_expression(node.value) and mentions(node.value, tainted):
                tainted.update(t.id for t in node.targets if isinstance(t, ast.Name))
        for node in nodes:
            if isinstance(node, ast.Call) and mentions(node, tainted) and (
                    is_read(node) or (isinstance(node.func, ast.Name) and node.func.id in helpers)):
                found.append((node.lineno, ast.unparse(node)))
            if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.Lambda, ast.ClassDef)):
                visit(node, tainted, helpers)

    visit(ast.parse(source), set(), set())
    return sorted(set(found))


def python_files() -> list[str]:
    from tools.repo_files import repo_files

    return [path for path in repo_files(ROOT, ("*.py",))
            if not path.startswith(SKIPPED_DIRECTORIES) and path not in EXEMPT_FILES]


class NothingReadsGeneratedBare(unittest.TestCase):
    def test_no_module_reads_a_generated_path_outside_the_loader(self):
        offenders = []
        for path in python_files():
            for line, code in bare_reads((ROOT / path).read_text(encoding="utf-8")):
                offenders.append(f"{path}:{line}: {code}")
        self.assertEqual(
            offenders, [],
            "read generated/ tables with psiv_tools.extract_stamp.load_table, which "
            "refuses an extract the current extractor did not write")

    def test_negative_control_every_bare_shape_is_seen(self):
        shapes = {
            "path built from the directory name": 'json.loads((data_dir / "formations.json").read_text())',
            "open on a constant": 'json.load(open(GENERATED / "enemies.json"))',
            "read_text with no json": 'text = (ROOT / "generated" / "maps.json").read_text()',
            "attribute on args": 'json.loads(pathlib.Path(args.generated / "characters.json").read_text())',
            "read_bytes through a module": 'blob = (ra.GENERATED / "maps.json").read_bytes()',
            "a module constant the file derived": (
                'ENEMIES = ROOT / "generated" / "enemies.json"\n'
                'names = json.loads(ENEMIES.read_text())'),
            "a parameter defaulted from the directory, read in a closure": (
                'GENERATED = ROOT / "generated"\n'
                'def load(directory=GENERATED):\n'
                '    def read(name):\n'
                '        return json.loads((directory / f"{name}.json").read_text())\n'
                '    return read("formations")'),
            "a local that holds the directory": (
                'def go(args):\n'
                '    base = pathlib.Path(args.data_dir)\n'
                '    return json.loads((base / "enemies.json").read_text())'),
            "a lambda that reads, called with the directory": (
                'read = lambda p: json.loads(p.read_text())\n'
                'characters = read(args.generated / "characters.json")'),
            "a helper function that reads, called with the directory": (
                'def read(p):\n    return json.loads(p.read_text())\n'
                'forms = read(DATA_DIR / "formations.json")'),
            "a reader opened explicitly for reading": 'open(data_dir / "x.json", "r")',
        }
        for name, code in shapes.items():
            with self.subTest(shape=name):
                self.assertTrue(bare_reads(code), code)

    def test_negative_control_the_loader_and_unrelated_reads_pass(self):
        for code in (
            'load_table(data_dir, "formations")',
            'load_table(args.generated, "characters")',
            'json.loads(ram_map.read_text())',
            'json.loads((work / "report.json").read_text())',
            'hashlib.sha256(p.read_bytes())',
            'write_generated(census, registry)',
            'open(data_dir / "enemies.json", "w")',
            'data = Data.load(arguments.data_dir)\nuse(data)',
            'def f(directory):\n    return json.loads((directory / "report.json").read_text())',
        ):
            with self.subTest(code=code):
                self.assertEqual(bare_reads(code), [], code)

    def test_the_scan_covers_the_readers(self):
        files = set(python_files())
        for reader in ("oracle/force/pack.py", "oracle/sweep/route_abilities.py",
                       "oracle/sweep/replay_pack.py", "oracle/sweep/coverage.py",
                       "oracle/sweep/player_abilities.py", "psiv_tools/dialogue_census.py"):
            self.assertIn(reader, files)


if __name__ == "__main__":
    unittest.main()
