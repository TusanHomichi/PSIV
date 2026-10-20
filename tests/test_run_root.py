"""Run roots: `tools/godot.py` `create_run_root`, and the census that every
repository tool starts Godot from one.

    PYTHONPATH=. python3 -m unittest tests.test_run_root -v

A run root holds a prepared project copy (one per worker), an independent
snapshot of the built extension and a link to the pack's real path. The cases
pin each against the way it can go wrong: a snapshot that a later in-place
write to the built library reaches (a hard link does), a pack link that follows
the checkout's link after a swap, workers sharing one project, a run root made
over an existing directory, a removal that takes something other than a run
root. The census reads every launcher whole (Python by its syntax tree) and
allows a Godot command line only from `godot_command` in tools/godot.py; its
negative controls are the launchers as they were before run roots and the
launch forms review found a per-line census missed. With the pinned Godot and a built extension, the last case shows a
real Godot mapping the run root's library, not the checkout's.
"""
import hashlib
import json
import os
import re
import subprocess
import tempfile
import time
import unittest
from pathlib import Path
from unittest import mock

from tools import godot
from tools.repo_files import repo_files

ROOT = Path(__file__).resolve().parents[1]


def sha256(data):
    return hashlib.sha256(data).hexdigest()


class Checkout(unittest.TestCase):
    """A minimal checkout: a Git work tree with a project, a built library and
    a pack reached through a link, as a lane worktree has."""

    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="run-root-")
        self.addCleanup(tmp.cleanup)
        self.dir = Path(tmp.name)
        self.source = self.dir / "checkout"
        (self.source / "godot").mkdir(parents=True)
        subprocess.run(["git", "init", "-q", str(self.source)], check=True)
        (self.source / "godot/project.godot").write_text("[application]\n")
        (self.source / "godot/psiv.gdextension").write_text("[configuration]\n")
        self.library = godot.library_path(self.source)
        self.library.parent.mkdir(parents=True)
        self.library.write_bytes(b"library v1")
        self.packs = self.dir / "packs"
        for version in ("v1", "v2"):
            (self.packs / version).mkdir(parents=True)
            (self.packs / version / "manifest.json").write_text(version)
        (self.source / "runtime-pack").symlink_to(self.packs / "v1")

    def create(self, name="run", **kwargs):
        return godot.create_run_root(self.dir / name, source=self.source, **kwargs)


class Layout(Checkout):

    def test_a_run_root_holds_a_prepared_project_the_library_and_the_pack(self):
        made = self.create()
        self.assertEqual(made.project, self.dir / "run/godot")
        self.assertTrue(godot.prepared(made.project))
        self.assertEqual((made.project / "project.godot").read_text(), "[application]\n")
        # The project's `res://../rust/target/debug/` is the snapshot.
        self.assertEqual(made.library, made.project.parent / "rust/target/debug/libpsiv_godot.so")
        self.assertEqual(made.library.read_bytes(), b"library v1")
        self.assertEqual(made.library_sha256, sha256(b"library v1"))
        self.assertEqual((made.pack / "manifest.json").read_text(), "v1")
        record = json.loads((made.root / godot.RUN_ROOT_RECORD).read_text())
        self.assertEqual(record["library_sha256"], sha256(b"library v1"))
        self.assertEqual(record["library_source"], str(self.library))
        self.assertEqual(record["pack_target"], str(self.packs / "v1"))
        self.assertIn(record["library_copy"], ("reflink", "copy"))

    def test_workers_get_their_own_projects_and_one_library(self):
        made = self.create(projects=3)
        self.assertEqual([project.name for project in made.projects],
                         ["godot", "godot-1", "godot-2"])
        self.assertEqual(len({project.parent for project in made.projects}), 1)
        self.assertTrue(all(godot.prepared(project) for project in made.projects))
        self.assertEqual(len(list((made.root / "rust/target/debug").iterdir())), 1)

    def test_the_release_profile_snapshots_a_named_library(self):
        named = self.dir / "zig/libpsiv_godot.so"
        named.parent.mkdir()
        named.write_bytes(b"deck build")
        made = self.create(profile="release", library=named)
        self.assertEqual(made.library, made.root / "rust/target/release/libpsiv_godot.so")
        self.assertEqual(made.library_sha256, sha256(b"deck build"))


class Snapshot(Checkout):
    """The snapshot is the run's library whatever later happens to the build."""

    def test_an_in_place_write_to_the_built_library_does_not_reach_the_run(self):
        made = self.create()
        self.assertNotEqual(made.library.stat().st_ino, self.library.stat().st_ino)
        self.assertEqual(made.library.stat().st_nlink, 1)
        with self.library.open("r+b") as handle:  # what `cp` over it does
            handle.write(b"LIBRARY v2")
        self.assertEqual(sha256(made.library.read_bytes()), made.library_sha256)

    def test_negative_control_a_hard_link_is_reached_by_the_write(self):
        """The check above fails a hard-linked snapshot: the expected value is
        the literal bytes, not anything the code under test computed."""
        linked = self.dir / "linked.so"
        os.link(self.library, linked)
        with self.library.open("r+b") as handle:
            handle.write(b"LIBRARY v2")
        self.assertNotEqual(sha256(linked.read_bytes()), sha256(b"library v1"))
        self.assertEqual(linked.stat().st_nlink, 2)

    def test_a_relink_of_the_built_library_does_not_reach_the_run(self):
        made = self.create()
        self.library.unlink()  # cargo's uplift: a new file under the old name
        self.library.write_bytes(b"library v2")
        self.assertEqual(made.library.read_bytes(), b"library v1")

    def test_the_snapshot_is_read_only(self):
        made = self.create()
        self.assertEqual(made.library.stat().st_mode & 0o222, 0)


class PackLink(Checkout):

    def test_the_pack_is_resolved_once(self):
        made = self.create()
        flipped = self.dir / "flip"
        flipped.symlink_to(self.packs / "v2")
        os.replace(flipped, self.source / "runtime-pack")  # a pack swap
        self.assertEqual((self.source / "runtime-pack/manifest.json").read_text(), "v2")
        self.assertEqual((made.pack / "manifest.json").read_text(), "v1")

    def test_negative_control_a_link_to_the_checkout_link_follows_the_swap(self):
        stale = self.dir / "stale"
        stale.symlink_to(self.source / "runtime-pack")
        flipped = self.dir / "flip"
        flipped.symlink_to(self.packs / "v2")
        os.replace(flipped, self.source / "runtime-pack")
        self.assertEqual((stale / "manifest.json").read_text(), "v2")

    def test_a_named_pack_must_exist_and_a_missing_default_makes_no_link(self):
        with self.assertRaises(godot.GodotError):
            self.create(pack=self.dir / "missing")
        (self.source / "runtime-pack").unlink()
        made = self.create("no-pack")
        self.assertFalse(os.path.lexists(made.pack))
        self.assertIsNone(made.record["pack"])


class Refusals(Checkout):

    def test_a_missing_library_is_refused_before_anything_is_made(self):
        self.library.unlink()
        with self.assertRaises(godot.GodotError) as caught:
            self.create()
        self.assertIn("no built extension", str(caught.exception))
        self.assertFalse((self.dir / "run").exists())

    def test_an_existing_directory_is_refused(self):
        (self.dir / "run").mkdir()
        with self.assertRaises(godot.GodotError):
            self.create()

    def test_release_keeps_the_record_and_removal_refuses_other_directories(self):
        made = self.create(projects=2)
        made.release()
        self.assertEqual([entry.name for entry in made.root.iterdir()], [godot.RUN_ROOT_RECORD])
        self.assertTrue(self.library.is_file())
        self.assertTrue((self.packs / "v1/manifest.json").is_file())  # a link, not followed
        godot.remove_run_root(made.root)
        self.assertFalse(made.root.exists())
        with self.assertRaises(godot.GodotError):
            godot.remove_run_root(self.source)
        self.assertTrue((self.source / "godot/project.godot").is_file())

    def test_a_run_root_that_fails_part_way_is_removed(self):
        """The reviewer's reproduction: `prepare` failing on the second project."""
        calls = []

        def failing(project):
            calls.append(project)
            if len(calls) == 2:
                raise godot.GodotError("cannot write the index")
            return True

        with mock.patch.object(godot, "prepare", failing), \
                self.assertRaises(godot.GodotError):
            self.create(projects=2)
        self.assertFalse((self.dir / "run").exists())
        self.create()  # the same path is free again

    def test_negative_control_a_half_root_without_its_record_is_refused(self):
        """What a failure left before the record came first: a directory that
        `remove_run_root`, and so `run-root --replace`, refuses."""
        (self.dir / "run/rust/target/debug").mkdir(parents=True)
        with self.assertRaises(godot.GodotError):
            godot.remove_run_root(self.dir / "run")

    def test_release_removes_only_what_the_run_root_made(self):
        made = self.create(projects=2)
        (made.root / "native.json").write_text("evidence")
        made.release()
        self.assertEqual(sorted(entry.name for entry in made.root.iterdir()),
                         ["native.json", godot.RUN_ROOT_RECORD])
        with self.assertRaises(godot.GodotError) as caught:
            godot.remove_run_root(made.root)
        self.assertIn("native.json", str(caught.exception))
        self.assertEqual((made.root / "native.json").read_text(), "evidence")

    def test_the_command_line_runs_from_and_replaces_its_own_run_root(self):
        """`run` makes a run root, keeps it only with `--keep`, takes over the
        one it made with `--replace`, and removes a one-shot root when the
        game exits. A stub stands in for Godot: the run root is the subject
        here, and the real engine is the next case's subject."""
        if not godot.library_path().is_file():
            self.skipTest("no built extension to snapshot")
        stub = self.dir / "godot-stub"
        stub.write_text(f'#!/bin/sh\necho "{godot.required_version()}.stable.official.test"\n')
        stub.chmod(0o755)
        env = {**os.environ, "PYTHONPATH": str(ROOT), "PSIV_GODOT": str(stub)}
        root = self.dir / "cli"
        keep = ["python3", str(ROOT / "tools/godot.py"), "run", "--dir", str(root), "--keep"]
        first = subprocess.run([*keep, "--", "--version"], capture_output=True, text=True,
                               env=env, cwd=ROOT)
        self.assertEqual(first.returncode, 0, first.stderr)
        self.assertIn(f"run root {root}", first.stderr)
        self.assertTrue((root / "godot/project.godot").is_file())
        self.addCleanup(godot.remove_run_root, root)
        replaced = subprocess.run([*keep, "--replace", "--", "--version"], capture_output=True,
                                  text=True, env=env, cwd=ROOT)
        self.assertEqual(replaced.returncode, 0, replaced.stderr)
        gone = self.dir / "gone"
        one_shot = ["python3", str(ROOT / "tools/godot.py"), "run", "--dir", str(gone),
                    "--", "--version"]
        removed = subprocess.run(one_shot, capture_output=True, text=True, env=env, cwd=ROOT)
        self.assertEqual(removed.returncode, 0, removed.stderr)
        self.assertFalse(gone.exists())


#: The resolver's names that hand out the Godot executable. Only tools/godot.py
#: uses them: every other launcher calls `godot_command` (or `require`).
RESOLVER_NAMES = {"resolve", "find", "check", "ENV"}
#: The subprocess and os calls that start a program.
STARTERS = {"run", "Popen", "call", "check_call", "check_output", "execv", "execvp",
            "execve", "execl", "execlp", "spawnv", "spawnvp"}


def python_problems(name, text):
    """What builds a Godot command line in the Python file `name` outside the
    one builder: the resolver's names, Godot's `--path`, the project file, the
    `PSIV_GODOT` variable, a program list or start call beginning with a bare
    `godot`, or a start call whose `cwd` is a `godot` project."""
    import ast
    problems = []
    tree = ast.parse(text)
    # A tool's own `--path` flag (oracle/host_binary.py) is not Godot's.
    own_flags = {id(arg) for node in ast.walk(tree) if isinstance(node, ast.Call)
                 and getattr(node.func, "attr", None) == "add_argument" for arg in node.args}
    for node in ast.walk(tree):
        line = f"{name}:{getattr(node, 'lineno', '?')}"
        if isinstance(node, ast.ImportFrom) and node.module in ("godot", "tools.godot"):
            for alias in node.names:
                if alias.name in RESOLVER_NAMES:
                    problems.append(f"{line}: imports the resolver's {alias.name}")
        elif isinstance(node, ast.Attribute) and node.attr in RESOLVER_NAMES \
                and isinstance(node.value, ast.Name) and node.value.id == "godot":
            problems.append(f"{line}: calls godot.{node.attr}")
        elif isinstance(node, ast.Constant) and isinstance(node.value, str):
            if node.value == "--path" and id(node) not in own_flags:
                problems.append(f"{line}: names Godot's --path")
            elif "project.godot" in node.value:
                problems.append(f"{line}: names a project.godot")
            elif node.value == "PSIV_GODOT":
                problems.append(f"{line}: reads PSIV_GODOT")
        elif isinstance(node, (ast.List, ast.Tuple)) and node.elts \
                and isinstance(node.elts[0], ast.Constant) and node.elts[0].value == "godot":
            problems.append(f"{line}: a command beginning with a bare godot")
        elif isinstance(node, ast.Call):
            func = node.func
            called = func.attr if isinstance(func, ast.Attribute) else getattr(func, "id", "")
            if called not in STARTERS:
                continue
            for keyword in node.keywords:
                if keyword.arg == "cwd" and "godot" in ast.unparse(keyword.value).replace(
                        "godot_", "").replace("_godot", ""):
                    problems.append(f"{line}: starts a program in a godot project (cwd=)")
    return problems


#: Shell text that builds a Godot command: the bare resolver (`$(python3
#: tools/godot.py)` prints the executable), `PSIV_GODOT`, `--path`, a project
#: file, or the word `godot` itself (a command, `cd godot`, `.../godot`).
SHELL_RULES = (
    (re.compile(r"""tools/godot\.py["']?(?=\s*(?:\)|$|[;&|>]))""", re.M),
     "prints the Godot executable (bare tools/godot.py)"),
    (re.compile(r"PSIV_GODOT"), "reads PSIV_GODOT"),
    (re.compile(r"--path(?![-\w])(?!.*host_binary)"), "names Godot's --path"),
    (re.compile(r"project\.godot"), "names a project.godot"),
    (re.compile(r"""(?<![\w.$-])godot(?![\w.-])(?!/)"""), "names a bare godot"),
)


def shell_code(text):
    """The shell text with comments dropped and continued lines joined."""
    lines = [re.sub(r"(?:^|\s)#.*$", "", line) for line in text.splitlines()]
    return re.sub(r"\\\n", " ", "\n".join(lines))


def shell_problems(name, text):
    code = shell_code(text)
    problems = []
    for number, line in enumerate(code.splitlines(), 1):
        for rule, what in SHELL_RULES:
            if rule.search(line) and not ("host_binary" in line and "--path" in what):
                problems.append(f"{name}:{number}: {what}: {line.strip()}")
    return problems


def launcher_problems(name, text):
    """Whole-file census: Godot's command line is built only by tools/godot.py."""
    if name == "tools/godot.py":
        return []
    return python_problems(name, text) if name.endswith(".py") else shell_problems(name, text)


def launchers():
    """Every tool, launcher and oracle script: code that could start Godot."""
    return [name for name in repo_files(ROOT, ("*.py", "*.sh"))
            if name.startswith(("tools/", "godot/", "oracle/", "psiv_tools/"))]


class Census(unittest.TestCase):

    def test_only_the_builder_makes_a_godot_command_line(self):
        problems = [problem for name in launchers()
                    for problem in launcher_problems(name, (ROOT / name).read_text())]
        self.assertEqual(problems, [])

    def test_the_launchers_use_the_builder(self):
        """Not vacuous: the four launchers go through tools/godot.py."""
        for name, needle in (("tools/certify.py", "godot_command("),
                             ("tools/verify_native_tape.py", "godot_command("),
                             ("godot/run.sh", "tools/godot.py run"),
                             ("tools/package/deploy-deck.sh", 'tools/godot.py" run')):
            with self.subTest(name=name):
                self.assertIn(needle, (ROOT / name).read_text())

    #: The launch forms the review found the line census missed (2026-10-18),
    #: and the launchers as they were before run roots.
    ESCAPES = {
        "tools/multi_line.py": 'from godot import resolve\n'
                               'cmd = [str(resolve()),\n    "--path",\n    str(ROOT / "godot")]\n',
        "tools/project_dir.py": 'from tools.godot import find\nPROJECT_DIR = ROOT / "godot"\n'
                                'subprocess.run([find(), "--path", str(PROJECT_DIR)])\n',
        "tools/cwd.py": 'import godot\nsubprocess.run([str(godot.resolve())], cwd=ROOT / "godot")\n',
        "tools/cwd_bare.py": 'subprocess.Popen(["godot", "--headless"], cwd=ROOT / "godot")\n',
        "tools/positional.py": 'subprocess.run([GODOT, str(ROOT / "godot/project.godot")])\n',
        "tools/cd.sh": 'cd godot && godot --headless\n',
        "tools/rev_parse.sh": '"$(python3 tools/godot.py)" \\\n'
                              '  --path "$(git rev-parse --show-toplevel)/godot"\n',
        "tools/positional.sh": '"$PSIV_GODOT" --headless "$PWD/godot/project.godot"\n',
        "tools/certify_old.py": 'command = ["timeout", "9s", str(godot), "--path", "godot"]\n',
        "tools/tape_old.py": 'from godot import find as find_godot\nGODOT = find_godot()\n',
        "godot/run_old.sh": 'psiv_godot=$(python3 tools/godot.py) || exit 1\n'
                            'nohup "$psiv_godot" --path godot &\n',
        "tools/deploy_old.sh": 'GODOT="$(python3 "$HERE/tools/godot.py")"\n'
                               '"$GODOT" --headless --path "$HERE/godot"\n',
    }

    def test_negative_control_every_escape_fails(self):
        for name, text in self.ESCAPES.items():
            with self.subTest(name=name):
                self.assertNotEqual(launcher_problems(name, text), [])

    def test_the_builder_its_callers_and_other_path_flags_pass(self):
        for name, text in (
                ("oracle/x.py", 'parser.add_argument("--path", action="store_true")\n'),
                ("oracle/x.sh", "BIN=$(python3 -m oracle.host_binary --path)\n"),
                ("tools/j.py", 'git("rev-parse", "--path-format=absolute", "--git-common-dir")\n'),
                ("tools/g.py", 'from godot import godot_command, require\n'
                               'command = godot_command(run_root, "--headless", prefix=("timeout", "9s"))\n'),
                ("tools/h.sh", 'python3 tools/godot.py check || exit 1\n'
                               'nohup python3 tools/godot.py run -- --headless &\n'),
                ("tools/k.py", 'GODOT = "godot"  # a lock class name\nLOCK_CLASSES = (GODOT,)\n')):
            with self.subTest(name=name):
                self.assertEqual(launcher_problems(name, text), [])


def godot_slot(test):
    """Hold a godot-class slot of the heavy lock for a case that starts a real
    Godot (a context manager). Under the gate's exclusive hold, or a godot
    holder, it takes nothing new; under a holder of another class (the Python
    suite run as `heavy_lock.py run --class python`), which reserved no slot,
    the case is skipped rather than run unaccounted."""
    from tools.heavy_lock import EXCLUSIVE, GODOT, HELD_CLASS_ENV, HELD_ENV, hold_heavy_lock
    held = os.environ.get(HELD_CLASS_ENV) if os.environ.get(HELD_ENV) else None
    if held not in (None, EXCLUSIVE, GODOT):
        test.skipTest(f"a real Godot needs a godot slot, and this {held}-class hold has none; "
                      "the gate (exclusive) runs it")
    return hold_heavy_lock(lock_class=GODOT)


class RealGodot(unittest.TestCase):
    """The pinned Godot, run from a run root, maps the run root's library."""

    def test_godot_maps_the_snapshot_not_the_checkout_library(self):
        try:
            godot.require()
        except godot.GodotError as error:
            self.skipTest(f"no pinned Godot: {error}")
        if not godot.library_path().is_file():
            self.skipTest("no built extension")
        tmp = tempfile.TemporaryDirectory(prefix="run-root-real-")
        self.addCleanup(tmp.cleanup)
        made = godot.create_run_root(Path(tmp.name) / "run")
        saves = Path(tmp.name) / "saves"
        saves.mkdir()
        slot = godot_slot(self)
        slot.__enter__()
        self.addCleanup(slot.__exit__, None, None, None)
        process = subprocess.Popen(
            godot.godot_command(made, "--headless", "--audio-driver", "Dummy",
                                "--quit-after", "100000"),
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            env={**os.environ, "PSIV_SAVE_DIR": str(saves)})
        try:
            mapped = set()
            deadline = time.monotonic() + 60
            while process.poll() is None and time.monotonic() < deadline and not mapped:
                try:
                    maps = Path(f"/proc/{process.pid}/maps").read_text()
                except OSError:
                    break
                mapped = {line.split(None, 5)[5] for line in maps.splitlines()
                          if line.count(" ") >= 5 and godot.LIBRARY_NAME in line}
                time.sleep(0.05)
        finally:
            process.kill()
            process.wait()
        self.assertEqual(mapped, {str(made.library)})


if __name__ == "__main__":
    unittest.main()
