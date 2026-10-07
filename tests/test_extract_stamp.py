"""`generated/` refuses an extract the current extractor did not write (#103).

    PYTHONPATH=. python3 -m unittest tests.test_extract_stamp -v

The loader, the stamp and the producers live in `psiv_tools/extract_stamp.py`.
Fixtures here are tiny synthetic directories the tests write themselves: no
ROM-derived table or pixel is committed. Where a producer needs the ROM the
test skips without it, the way the other extractor tests do.
"""
from __future__ import annotations

import ast
import hashlib
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

    def test_a_table_the_stamp_does_not_list_is_refused(self):
        # An old table left in a freshly stamped directory (one a later extractor
        # dropped or renamed) must not load under the new stamp.
        write_json(self.directory / "enemies.json", [])
        stamp.write_table_stamp(self.directory)
        write_json(self.directory / "old_table.json", {"from": "an older extract"})
        with self.assertRaises(StaleExtractError) as caught:
            load_table(self.directory, "old_table")
        message = str(caught.exception)
        self.assertIn(str(self.directory / "old_table.json"), message)
        self.assertIn("not one of the files", message)
        self.assertIn("python3 -m psiv_tools regenerate", message)
        self.assertEqual(load_table(self.directory, "enemies"), [], "the listed table still loads")

    def test_a_table_whose_bytes_changed_after_stamping_is_refused(self):
        # A stale table copied over a current one, under a current stamp.
        write_json(self.directory / "formations.json", {"inline_formations": [1]})
        stamp.write_table_stamp(self.directory)
        write_json(self.directory / "formations.json", {})
        with self.assertRaises(StaleExtractError) as caught:
            load_table(self.directory, "formations")
        message = str(caught.exception)
        self.assertIn(str(self.directory / "formations.json"), message)
        self.assertIn("changed since the extract was stamped", message)
        self.assertIn("python3 -m psiv_tools regenerate", message)
        with self.assertRaises(StaleExtractError):
            stamp.table_sha256(self.directory, "formations")

    def test_metadata_json_is_bound_by_its_contents_too(self):
        write_json(self.directory / "metadata.json", {"hashes": {"sha256": "aa"}})
        stamp.write_table_stamp(self.directory)
        self.assertEqual(load_table(self.directory, "metadata")["hashes"], {"sha256": "aa"})
        document = json.loads((self.directory / "metadata.json").read_text())
        document["hashes"]["sha256"] = "bb"
        write_json(self.directory / "metadata.json", document)
        with self.assertRaises(StaleExtractError):
            load_table(self.directory, "metadata")

    def test_the_stamp_lists_the_hash_a_provenance_record_can_use(self):
        write_json(self.directory / "characters.json", [1, 2])
        stamp.write_table_stamp(self.directory)
        self.assertEqual(stamp.table_sha256(self.directory, "characters"),
                         hashlib.sha256((self.directory / "characters.json").read_bytes()).hexdigest())

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
    def png(self, name: str, content: bytes = b"png") -> pathlib.Path:
        path = self.directory / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
        return path

    def test_a_stamped_directory_passes_and_a_foreign_one_is_refused(self):
        stamp.write_png_stamp(self.directory, "gfx", [self.png("a.png"), self.png("sub/b.png")])
        stamp.check_png_directory(self.directory, "gfx")
        with self.assertRaises(StaleExtractError) as caught:
            stamp.check_png_directory(self.directory, "planes")
        self.assertIn(str(self.directory / stamp.STAMP_FILE), str(caught.exception))
        self.assertIn("python3 -m psiv_tools regenerate", str(caught.exception))
        record = json.loads((self.directory / stamp.STAMP_FILE).read_text())
        self.assertEqual(sorted(record["files"]), ["a.png", "sub/b.png"])
        write_json(self.directory / stamp.STAMP_FILE, {**record, "stamp": "f" * 64})
        with self.assertRaises(StaleExtractError):
            stamp.check_png_directory(self.directory, "gfx")
        (self.directory / stamp.STAMP_FILE).unlink()
        with self.assertRaises(StaleExtractError):
            stamp.check_png_directory(self.directory, "gfx")

    def test_an_orphan_image_is_refused(self):
        # An image from an earlier run that this run no longer writes.
        stamp.write_png_stamp(self.directory, "gfx", [self.png("a.png")])
        self.png("orphan.png")
        with self.assertRaises(StaleExtractError) as caught:
            stamp.check_png_directory(self.directory, "gfx")
        self.assertIn("orphan.png", str(caught.exception))
        self.assertIn("does not list", str(caught.exception))

    def test_a_changed_or_missing_listed_image_is_refused(self):
        stamp.write_png_stamp(self.directory, "gfx", [self.png("a.png"), self.png("b.png")])
        self.png("a.png", b"another run's pixels")
        with self.assertRaises(StaleExtractError) as caught:
            stamp.check_png_directory(self.directory, "gfx")
        self.assertIn("a.png", str(caught.exception))
        self.png("a.png")
        stamp.check_png_directory(self.directory, "gfx")
        (self.directory / "b.png").unlink()
        with self.assertRaises(StaleExtractError) as caught:
            stamp.check_png_directory(self.directory, "gfx")
        self.assertIn("missing b.png", str(caught.exception))

    def test_an_export_removes_the_images_its_last_stamp_listed_and_only_those(self):
        stamp.write_png_stamp(self.directory, "gfx", [self.png("a.png"), self.png("dropped.png")])
        keep = self.directory / "notes.txt"
        keep.write_text("not the exporter's")
        stamp.begin_png_export(self.directory)
        self.assertFalse((self.directory / "dropped.png").exists())
        self.assertFalse((self.directory / "a.png").exists())
        self.assertFalse((self.directory / stamp.STAMP_FILE).exists(), "half way means unvouched")
        self.assertTrue(keep.exists(), "a file no stamp listed is never deleted")
        # The next run writes only a.png: dropped.png is gone for good.
        stamp.write_png_stamp(self.directory, "gfx", [self.png("a.png")])
        self.assertFalse((self.directory / "dropped.png").exists())
        with self.assertRaises(StaleExtractError):  # notes.txt is not listed
            stamp.check_png_directory(self.directory, "gfx")

    def test_a_stamp_cannot_make_an_export_delete_outside_its_directory(self):
        outside = self.root / "precious.txt"
        outside.write_text("keep")
        write_json(self.directory / stamp.STAMP_FILE,
                   {"files": {"../precious.txt": "0", str(outside): "0"}})
        stamp.begin_png_export(self.directory)
        self.assertTrue(outside.exists())

    def test_a_directory_that_grows_one_image_at_a_time_cannot_inherit_a_stamp(self):
        self.png("old.png")
        with self.assertRaises(StaleExtractError):
            stamp.check_accumulating_png_directory(self.directory, "layouts", {"new.png"})
        stamp.check_accumulating_png_directory(self.directory, "layouts", {"old.png"})
        stamp.write_png_stamp(self.directory, "layouts", [self.directory / "old.png"])
        stamp.check_accumulating_png_directory(self.directory, "layouts", {"new.png"})
        stamp.write_png_stamp(self.directory, "layouts", [self.png("new.png")], keep_previous=True)
        stamp.check_png_directory(self.directory, "layouts")
        record = json.loads((self.directory / stamp.STAMP_FILE).read_text())
        self.assertEqual(sorted(record["files"]), ["new.png", "old.png"])


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
                written = export(self.data, target)
                stamp.check_png_directory(target, kind)
                record = json.loads((target / stamp.STAMP_FILE).read_text())
                self.assertEqual(len(record["files"]), len(written))
                (target / "orphan.png").write_bytes(b"x")
                with self.assertRaises(StaleExtractError):
                    stamp.check_png_directory(target, kind)


class WriteExtract(TempDirectory):
    """`write_extract` stamps `metadata.json` with every table's hash, and writes it last."""

    def fake_result(self, without=()):
        return {key: {"key": key} for key in ["metadata", *core.TABLE_KEYS] if key not in without}

    def test_the_written_extract_carries_the_stamp_and_loads(self):
        with mock.patch.object(core, "extract_all", return_value=self.fake_result()):
            core.write_extract(b"", self.directory)
        metadata = json.loads((self.directory / "metadata.json").read_text())
        record = metadata["extract_stamp"]
        self.assertEqual({k: record[k] for k in ("kind", "stamp", "modules")}, stamp.table_stamp())
        self.assertEqual(sorted(record["files"]), sorted(["metadata.json", *(f"{k}.json" for k in core.TABLE_KEYS)]))
        for key in core.TABLE_KEYS:
            self.assertEqual(record["files"][f"{key}.json"],
                             hashlib.sha256((self.directory / f"{key}.json").read_bytes()).hexdigest())
        self.assertEqual(load_table(self.directory, "enemies"), {"key": "enemies"})
        self.assertEqual(load_table(self.directory, "metadata")["key"], "metadata")

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

    def test_a_rerun_that_drops_a_table_removes_the_old_file(self):
        with mock.patch.object(core, "extract_all", return_value=self.fake_result()):
            core.write_extract(b"", self.directory)
        (self.directory / "notes.txt").write_text("not a table")
        write_json(self.directory / "unlisted.json", {"from": "nobody's stamp"})
        kept = [key for key in core.TABLE_KEYS if key != "vehicles"]
        with mock.patch.object(core, "TABLE_KEYS", kept), \
                mock.patch.object(core, "extract_all", return_value=self.fake_result(without=("vehicles",))):
            core.write_extract(b"", self.directory)
        self.assertFalse((self.directory / "vehicles.json").exists(), "the table the new extractor dropped")
        self.assertEqual(load_table(self.directory, "enemies"), {"key": "enemies"})
        with self.assertRaises(FileNotFoundError):
            load_table(self.directory, "vehicles")
        self.assertTrue((self.directory / "notes.txt").exists(), "a file no stamp listed is never deleted")
        with self.assertRaises(StaleExtractError):
            load_table(self.directory, "unlisted")


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
    its receiver and its arguments are. A tainted name is a variable (`base`) or an
    attribute chain (`self.root`).
    """
    if isinstance(node, ast.Call):
        parts = [*node.args, *(k.value for k in node.keywords)]
        if isinstance(node.func, ast.Attribute):
            parts.append(node.func.value)
        return any(mentions(part, tainted) for part in parts)
    if GENERATED_NAMES.search(ast.unparse(node)):
        return True
    return any(isinstance(sub, (ast.Name, ast.Attribute)) and ast.unparse(sub) in tainted
               for sub in ast.walk(node))


PATH_WRAPPERS = {"Path", "PurePath", "str", "join", "resolve", "joinpath", "absolute",
                 "glob", "rglob", "iterdir", "sorted", "list", "tuple", "set", "reversed",
                 "enumerate"}


def is_path_expression(node: ast.AST) -> bool:
    """A value that is still a path (`dir / name`, `Path(dir)`, `dir.glob(...)`, a pair of them), not what was read from one."""
    if isinstance(node, ast.BinOp):
        return isinstance(node.op, ast.Div)
    if isinstance(node, ast.Call):
        function = node.func
        name = function.attr if isinstance(function, ast.Attribute) else getattr(function, "id", "")
        return name in PATH_WRAPPERS
    if isinstance(node, (ast.Tuple, ast.List, ast.Set)):
        return any(is_path_expression(element) for element in node.elts)
    if isinstance(node, ast.NamedExpr):
        return is_path_expression(node.value)
    return isinstance(node, (ast.Name, ast.Attribute, ast.Constant, ast.JoinedStr))


def target_names(target: ast.AST) -> set[str]:
    """The variables and attribute chains a target binds (`a`, `self.root`, each of `a, (b, c)`)."""
    if isinstance(target, (ast.Name, ast.Attribute)):
        return {ast.unparse(target)}
    if isinstance(target, (ast.Tuple, ast.List)):
        return set().union(*(target_names(element) for element in target.elts), set())
    if isinstance(target, ast.Starred):
        return target_names(target.value)
    return set()


def own_nodes(scope: ast.AST):
    """Every node of `scope` outside the functions, lambdas and classes nested in it."""
    stack = list(ast.iter_child_nodes(scope))
    while stack:
        node = stack.pop()
        yield node
        if not isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.Lambda, ast.ClassDef)):
            stack.extend(ast.iter_child_nodes(node))


def bindings(nodes) -> list[tuple[int, set[str], ast.AST]]:
    """`(line, names bound, value)` for each assignment shape: `=`, `x: T = v`, `for x in v`, `(x := v)`."""
    found = []
    for node in nodes:
        if isinstance(node, ast.Assign):
            for target in node.targets:
                if (isinstance(target, (ast.Tuple, ast.List)) and isinstance(node.value, (ast.Tuple, ast.List))
                        and len(target.elts) == len(node.value.elts)):
                    # `a, b = x, y` binds pairwise; anything else binds every name to the whole value.
                    found.extend((node.lineno, target_names(t), v) for t, v in zip(target.elts, node.value.elts))
                else:
                    found.append((node.lineno, target_names(target), node.value))
        elif isinstance(node, ast.AnnAssign) and node.value is not None:
            found.append((node.lineno, target_names(node.target), node.value))
        elif isinstance(node, (ast.For, ast.AsyncFor)):
            found.append((node.lineno, target_names(node.target), node.iter))
        elif isinstance(node, ast.comprehension):
            found.append((node.iter.lineno, target_names(node.target), node.iter))
        elif isinstance(node, ast.NamedExpr):
            found.append((node.lineno, target_names(node.target), node.value))
    return sorted(found, key=lambda item: item[0])


def bare_reads(source: str) -> list[tuple[int, str]]:
    """`(line, code)` of every read of a path that is, or derives from, the extract directory.

    A read is `json.load`/`json.loads`, `.read_text()`, `.read_bytes()`,
    `.open()` or `open(...)` for reading, or a call to a helper defined in the
    same file whose body performs one (`read = lambda p: json.loads(p.read_text())`).
    A path is the extract directory's when its source names it (`generated`,
    `GENERATED`, `data_dir`, ...), or names a variable or attribute that was
    bound from one: by assignment, annotated assignment, tuple unpacking, a
    `for` target, a comprehension target, a walrus, a parameter default, or
    `self.root = data_dir` in any method of the class.

    What it cannot see is a read through a generic name that never touched one
    of those (`path.read_text()` on a parameter the caller fills), a helper in
    another module, or a copy (`shutil.copy`): the loader is the door and this
    is the deterrent. `EveryReaderRefusesAStaleExtract` calls each reader with
    an extract the stamp does not vouch for.
    """
    found: list[tuple[int, str]] = []

    def visit(scope: ast.AST, inherited: set[str], helpers: set[str], report: bool = True) -> set[str]:
        """Visit one scope; return the `self.x` attributes it tainted."""
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
        for node in nodes:  # local reader helpers
            if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and any(
                    isinstance(sub, ast.Call) and is_read(sub) for sub in ast.walk(node)):
                helpers.add(node.name)
            if isinstance(node, ast.Assign) and isinstance(node.value, ast.Lambda) and any(
                    isinstance(sub, ast.Call) and is_read(sub) for sub in ast.walk(node.value)):
                helpers.update(t.id for t in node.targets if isinstance(t, ast.Name))
        for _, names, value in bindings(nodes):  # taint, in source order
            if is_path_expression(value) and mentions(value, tainted):
                tainted.update(names)
        if report:
            for node in nodes:
                if isinstance(node, ast.Call) and mentions(node, tainted) and (
                        is_read(node) or (isinstance(node.func, ast.Name) and node.func.id in helpers)):
                    found.append((node.lineno, ast.unparse(node)))
        attributes = {name for name in tainted - inherited if name.startswith("self.")}
        children = [n for n in nodes if isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef, ast.Lambda, ast.ClassDef))]
        if isinstance(scope, ast.ClassDef):
            # `self.root = data_dir` in one method taints `self.root` in every method.
            for child in children:
                attributes |= visit(child, tainted, helpers, report=False)
            tainted |= attributes
        for child in children:
            attributes |= visit(child, tainted, helpers, report)
        return attributes

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
            "a for-loop target": (
                'for table in (data_dir / "a.json", data_dir / "b.json"):\n'
                '    json.loads(table.read_text())'),
            "a for-loop over a glob": (
                'for table in sorted(data_dir.glob("*.json")):\n'
                '    json.loads(table.read_text())'),
            "an annotated assignment": (
                'base: pathlib.Path = args.generated\n'
                'json.loads((base / "enemies.json").read_text())'),
            "tuple unpacking": (
                'first, second = data_dir / "a.json", ROOT / "report.json"\n'
                'json.loads(first.read_text())'),
            "a walrus": 'json.loads((table := data_dir / "a.json").read_text())',
            "a walrus bound then read": (
                'if (table := data_dir / "a.json").is_file():\n'
                '    json.loads(table.read_text())'),
            "an attribute set in __init__ and read in another method": (
                'class Reader:\n'
                '    def __init__(self, data_dir):\n'
                '        self.root = data_dir\n'
                '    def formations(self):\n'
                '        return json.loads((self.root / "formations.json").read_text())'),
            "a comprehension": '[json.loads(t.read_text()) for t in GENERATED.glob("*.json")]',
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
            'for table in report_dir.glob("*.json"):\n    json.loads(table.read_text())',
            'class R:\n    def __init__(self, root):\n        self.root = root\n'
            '    def go(self):\n        return json.loads((self.root / "r.json").read_text())',
            'first, second = data_dir / "a.json", ROOT / "report.json"\njson.loads(second.read_text())',
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
