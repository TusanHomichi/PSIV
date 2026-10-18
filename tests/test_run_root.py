"""Run roots: `tools/godot.py` `create_run_root`, and the census that every
repository tool starts Godot from one.

    PYTHONPATH=. python3 -m unittest tests.test_run_root -v

A run root holds a prepared project copy (one per worker), an independent
snapshot of the built extension and a link to the pack's real path. The cases
pin each against the way it can go wrong: a snapshot that a later in-place
write to the built library reaches (a hard link does), a pack link that follows
the checkout's link after a swap, workers sharing one project, a run root made
over an existing directory, a removal that takes something other than a run
root. The census reads every launcher for the old idiom, Godot's `--path` at
the checkout's `godot/`, and requires a file that passes `--path` to create a
run root; its negative controls are the launchers as they were written before
run roots. With the pinned Godot and a built extension, the last case shows a
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

    def test_the_command_line_replaces_its_own_run_root(self):
        env = {**os.environ, "PYTHONPATH": str(ROOT)}
        command = ["python3", str(ROOT / "tools/godot.py"), "run-root", str(self.dir / "cli")]
        # The tool snapshots the checkout it lives in, so it runs from ROOT;
        # a fresh clone without a built library refuses, which is also a pass.
        first = subprocess.run(command, capture_output=True, text=True, env=env, cwd=ROOT)
        if first.returncode != 0:
            self.assertIn("no built extension", first.stderr)
            return
        self.addCleanup(godot.remove_run_root, self.dir / "cli")
        self.assertEqual(first.stdout.strip(), str(self.dir / "cli/godot"))
        again = subprocess.run(command, capture_output=True, text=True, env=env, cwd=ROOT)
        self.assertNotEqual(again.returncode, 0)  # never over an existing one
        replaced = subprocess.run([*command, "--replace"], capture_output=True, text=True,
                                  env=env, cwd=ROOT)
        self.assertEqual(replaced.returncode, 0, replaced.stderr)


#: Godot's `--path` at the checkout's own project: `godot`, `./godot`,
#: `$HERE/godot` or `$PWD/godot` in a shell line, `"godot"`, `ROOT / "godot"`
#: or the resolver's `PROJECT` in a Python argument list.
OLD_PATH_RE = re.compile(
    r"""--path["']?\s*[,=]?\s*(?:str\(\s*)?"""
    r"""(?:(?:["']?\$\{?\w+\}?/)|(?:\w+\s*/\s*)|\./)?["']?godot\b(?![-.\w/])"""
    r"""|--path["']?\s*[,=]?\s*(?:str\(\s*)?(?:godot\.)?PROJECT\b""")
#: Godot's `--path` flag as a word, not `git rev-parse --path-format`.
PATH_FLAG_RE = re.compile(r"--path(?![-\w])")
#: Lines that use `--path` for something other than Godot: the oracle host
#: binary's own flag.
NOT_GODOT_RE = re.compile(r"""add_argument\(\s*["']--path["']|host_binary --path""")
#: How a Python and a shell launcher make a run root.
PY_RUN_ROOT_RE = re.compile(r"\bcreate_run_root\(")
SH_RUN_ROOT_RE = re.compile(r"""tools/godot\.py"?\s+run-root\b""")


def launcher_problems(name, text):
    """What is wrong with how `text` (the file `name`) starts Godot."""
    problems = [f"{name}:{number}: --path at the checkout's godot/: {line.strip()}"
                for number, line in enumerate(text.splitlines(), 1)
                if OLD_PATH_RE.search(line)]
    passes_path = any(PATH_FLAG_RE.search(line) and not NOT_GODOT_RE.search(line)
                      for line in text.splitlines())
    makes_root = (PY_RUN_ROOT_RE if name.endswith(".py") else SH_RUN_ROOT_RE).search(text)
    if passes_path and not makes_root and name != "tools/godot.py":
        problems.append(f"{name}: passes Godot a --path without creating a run root")
    return problems


def launchers():
    """Every tool, launcher and oracle script: code that could start Godot."""
    return [name for name in repo_files(ROOT, ("*.py", "*.sh"))
            if name.startswith(("tools/", "godot/", "oracle/", "psiv_tools/"))]


class Census(unittest.TestCase):

    def test_no_tool_starts_godot_at_the_checkout_project(self):
        problems = [problem for name in launchers()
                    for problem in launcher_problems(name, (ROOT / name).read_text())]
        self.assertEqual(problems, [])

    def test_the_census_reads_every_known_launcher(self):
        """Not vacuous: the four launchers pass `--path` and make a run root."""
        found = {name for name in launchers()
                 if any(PATH_FLAG_RE.search(line) and not NOT_GODOT_RE.search(line)
                        for line in (ROOT / name).read_text().splitlines())}
        self.assertLessEqual({"tools/certify.py", "tools/verify_native_tape.py",
                              "godot/run.sh", "tools/package/deploy-deck.sh"}, found)

    OLD_LAUNCHERS = {
        # The four launchers before run roots (main at 0b1e9bc).
        "tools/certify.py": '               "--path", "godot", "--quit-after", '
                            'str(tick + QUIT_MARGIN)]\ncreate_run_root(x)\n',
        "tools/verify_native_tape.py": '    command.extend(["--log-file", str(godot_log), '
                                       '"--path", "godot", "--script",\ncreate_run_root(x)\n',
        "godot/run.sh": 'nohup "$psiv_godot" --path godot \\\n'
                        'python3 tools/godot.py run-root build/play\n',
        "tools/package/deploy-deck.sh": '"$GODOT" --headless --path "$HERE/godot" \\\n'
                                        'python3 "$HERE/tools/godot.py" run-root x\n',
        # Other spellings of the same project.
        "tools/a.py": 'cmd = [godot, "--path", str(ROOT / "godot")]\ncreate_run_root(x)\n',
        "tools/b.py": 'cmd = [godot, "--path", str(PROJECT)]\ncreate_run_root(x)\n',
        "tools/c.sh": 'godot --path ./godot --quit-after 9\ntools/godot.py run-root x\n',
        "tools/d.sh": 'godot --path="$PWD/godot"\ntools/godot.py run-root x\n',
    }

    def test_negative_control_the_old_launchers_fail(self):
        """Each old line fails even beside a run root it does not use."""
        for name, text in self.OLD_LAUNCHERS.items():
            with self.subTest(name=name):
                problems = launcher_problems(name, text)
                self.assertEqual(len(problems), 1, problems)
                self.assertIn("--path at the checkout's godot/", problems[0])

    def test_negative_control_a_path_without_a_run_root_fails(self):
        for name, text in (("tools/e.py", 'cmd = [godot, "--path", str(project_dir)]\n'),
                           ("tools/f.sh", 'godot --path "$project" --headless\n')):
            with self.subTest(name=name):
                self.assertEqual(launcher_problems(name, text),
                                 [f"{name}: passes Godot a --path without creating a run root"])

    def test_the_host_binary_flag_and_run_root_projects_pass(self):
        for name, text in (
                ("tools/j.py", 'git("rev-parse", "--path-format=absolute", "--git-common-dir")\n'),
                ("oracle/x.py", 'parser.add_argument("--path", action="store_true")\n'),
                ("oracle/x.sh", "BIN=$(python3 -m oracle.host_binary --path)\n"),
                ("tools/g.py", 'create_run_root(x)\n[godot, "--path", str(run_root.project)]\n'),
                ("tools/h.sh", 'p=$(python3 tools/godot.py run-root d)\ngodot --path "$p"\n'),
                ("tools/i.py", 'create_run_root(x)\n["--path", str(project), "--script",'
                               ' str(ROOT / "tools/native/native_tape.gd")]\n')):
            with self.subTest(name=name):
                self.assertEqual(launcher_problems(name, text), [])


class RealGodot(unittest.TestCase):
    """The pinned Godot, run from a run root, maps the run root's library."""

    def test_godot_maps_the_snapshot_not_the_checkout_library(self):
        try:
            real = godot.resolve()
        except godot.GodotError as error:
            self.skipTest(f"no pinned Godot: {error}")
        if not godot.library_path().is_file():
            self.skipTest("no built extension")
        tmp = tempfile.TemporaryDirectory(prefix="run-root-real-")
        self.addCleanup(tmp.cleanup)
        made = godot.create_run_root(Path(tmp.name) / "run")
        saves = Path(tmp.name) / "saves"
        saves.mkdir()
        process = subprocess.Popen(
            [str(real), "--headless", "--audio-driver", "Dummy", "--path", str(made.project),
             "--quit-after", "100000"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
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
