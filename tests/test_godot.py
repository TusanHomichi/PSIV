"""The Godot resolver: `tools/godot.py`, the one lookup every launcher uses.

    PYTHONPATH=. python3 -m unittest tests.test_godot -v

`PSIV_GODOT` wins, `godot` on PATH is the fallback, and either must report the
version `.godot-version` pins. Each case stands a small script in for Godot
that prints what a real `godot --version` prints. The negative controls are a
wrong version (a patch release, a two-digit patch that shares the pinned
prefix, an older minor), the pinned version as a release candidate, beta,
dev or unlabelled build, a `PSIV_GODOT` that names nothing (which must not
fall back to PATH), no Godot anywhere, a Godot whose `--version` fails, and a
malformed version file. The last cases pin that the launchers ask the resolver
instead of naming a host path.
"""
import os
import re
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from tools import godot
from tools.repo_files import repo_files

ROOT = Path(__file__).resolve().parents[1]

FAKE = "#!/bin/sh\n[ \"$1\" = --version ] || exit 9\nprintf '%s\\n' '{output}'\nexit {code}\n"


class FakeGodot(unittest.TestCase):

    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="godot-resolver-")
        self.addCleanup(tmp.cleanup)
        self.dir = Path(tmp.name)
        self.version_file = self.dir / ".godot-version"
        self.version_file.write_text("4.7.1\n")

    def fake(self, name, output="4.7.1.stable.official.a13da4feb", code=0):
        path = self.dir / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(FAKE.format(output=output, code=code))
        path.chmod(path.stat().st_mode | stat.S_IXUSR)
        return path

    def resolve(self, env):
        return godot.resolve(env=env, version_file=self.version_file)

    def assert_refused(self, env, message):
        with self.assertRaises(godot.GodotError) as caught:
            self.resolve(env)
        self.assertIn(message, str(caught.exception))


class Resolution(FakeGodot):

    def test_the_environment_variable_names_the_executable(self):
        named = self.fake("named/psiv-godot")
        self.fake("bin/godot", output="4.6.1.stable.official.x")
        env = {"PSIV_GODOT": str(named), "PATH": str(self.dir / "bin")}
        self.assertEqual(self.resolve(env), named)

    def test_godot_on_path_is_the_fallback(self):
        found = self.fake("bin/godot")
        self.assertEqual(self.resolve({"PATH": str(self.dir / "bin")}), found)

    def test_an_empty_variable_falls_back_to_path(self):
        found = self.fake("bin/godot")
        self.assertEqual(self.resolve({"PSIV_GODOT": "", "PATH": str(self.dir / "bin")}), found)

    def test_a_dot_zero_release_prints_without_its_patch(self):
        self.version_file.write_text("4.7.0\n")
        found = self.fake("bin/godot", output="4.7.stable.official.abc")
        self.assertEqual(self.resolve({"PATH": str(self.dir / "bin")}), found)


class NegativeControls(FakeGodot):

    def test_a_wrong_version_is_refused(self):
        for output in ("4.7.2.stable.official.x", "4.7.10.stable.official.x",
                       "4.6.1.stable.official.x", "4.7.stable.official.x"):
            with self.subTest(output=output):
                named = self.fake("named/godot", output=output)
                version = output.split(".stable")[0]
                self.assert_refused({"PSIV_GODOT": str(named)},
                                    f"is Godot {version}; PSIV needs 4.7.1")

    def test_a_build_that_is_not_stable_is_refused(self):
        for output, status in (("4.7.1.rc1.official.x", "rc1"),
                               ("4.7.1.beta2.custom_build.x", "beta2"),
                               ("4.7.1.dev.custom_build", "dev"),
                               ("4.7.1", "with no build status")):
            with self.subTest(output=output):
                named = self.fake("named/godot", output=output)
                self.assert_refused({"PSIV_GODOT": str(named)},
                                    f"is Godot 4.7.1 {status}; PSIV needs the stable 4.7.1 release")

    def test_a_variable_naming_nothing_does_not_fall_back(self):
        self.fake("bin/godot")
        self.assert_refused({"PSIV_GODOT": str(self.dir / "missing"),
                             "PATH": str(self.dir / "bin")},
                            "is not an executable file")

    def test_no_godot_anywhere(self):
        self.assert_refused({"PATH": str(self.dir / "empty")}, "no Godot: set PSIV_GODOT")

    def test_a_failing_version_query(self):
        named = self.fake("named/godot", code=1)
        self.assert_refused({"PSIV_GODOT": str(named)}, "reported no version (exit 1)")

    def test_output_that_is_not_a_version(self):
        named = self.fake("named/godot", output="Godot Engine")
        self.assert_refused({"PSIV_GODOT": str(named)}, "reported no version")

    def test_a_malformed_version_file(self):
        for text in ("", "4.7.x", "4.7.1\n4.7.2", "v4.7.1"):
            with self.subTest(text=text):
                self.version_file.write_text(text)
                self.assert_refused({"PATH": ""}, "must hold one version")


class CommandLine(FakeGodot):
    """`python3 tools/godot.py`, the form the shell launchers run."""

    def run_cli(self, env):
        return subprocess.run([sys.executable, str(ROOT / "tools/godot.py")],
                              capture_output=True, text=True, env=env)

    def test_prints_the_path_or_refuses(self):
        # The CLI reads the tracked version file, so the fake reports that version.
        pinned = godot.required_version()
        named = self.fake("named/godot", output=f"{pinned}.stable.official.x")
        ok = self.run_cli({"PSIV_GODOT": str(named), "PATH": os.environ["PATH"]})
        self.assertEqual((ok.returncode, ok.stdout.strip()), (0, str(named)), ok.stderr)
        wrong = self.fake("named/old", output="3.6.stable.official.x")
        refused = self.run_cli({"PSIV_GODOT": str(wrong), "PATH": os.environ["PATH"]})
        self.assertEqual(refused.returncode, 1)
        self.assertEqual(refused.stdout, "")
        self.assertIn(f"is Godot 3.6; PSIV needs {pinned}", refused.stderr)


IMPORTER = """#!/bin/sh
# Stands in for Godot's headless editor import: logs the call, then writes
# the extension index (or not) under the project --path names.
echo "$@" >> "{calls}"
[ "$1" = --headless ] || exit 9
project=$3
if [ -n "{writes}" ]; then
    mkdir -p "$project/.godot"
    printf '%s\\n' "{writes}" > "$project/.godot/extension_list.cfg"
fi
exit {code}
"""


class Prepare(unittest.TestCase):
    """`prepare`: a fresh clone's project gets its extension index, once."""

    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="godot-prepare-")
        self.addCleanup(tmp.cleanup)
        self.dir = Path(tmp.name)
        self.project = self.dir / "godot"
        self.project.mkdir()
        (self.project / "project.godot").write_text("[application]\n")
        self.calls = self.dir / "calls"

    def importer(self, writes="res://psiv.gdextension", code=0):
        path = self.dir / "importer"
        path.write_text(IMPORTER.format(calls=self.calls, writes=writes, code=code))
        path.chmod(0o755)
        return path

    def calls_made(self):
        return self.calls.read_text().splitlines() if self.calls.exists() else []

    def test_a_fresh_project_gets_the_index_once(self):
        importer = self.importer()
        self.assertFalse(godot.prepared(self.project))  # the fresh clone's state
        log = self.dir / "import.log"
        self.assertTrue(godot.prepare(importer, self.project, log=log))
        self.assertTrue(godot.prepared(self.project))
        self.assertEqual(self.calls_made(),
                         [f"--headless --path {self.project} --editor --quit"])
        self.assertTrue(log.is_file())
        self.assertFalse(godot.prepare(importer, self.project))  # idempotent
        self.assertEqual(len(self.calls_made()), 1)

    def test_an_index_without_the_extension_is_imported_again(self):
        (self.project / ".godot").mkdir()
        (self.project / ".godot/extension_list.cfg").write_text("res://other.gdextension\n")
        self.assertTrue(godot.prepare(self.importer(), self.project))
        self.assertTrue(godot.prepared(self.project))

    def test_an_import_that_writes_nothing_is_refused(self):
        with self.assertRaises(godot.GodotError) as caught:
            godot.prepare(self.importer(writes=""), self.project)
        self.assertIn("import (exit 0) wrote no res://psiv.gdextension", str(caught.exception))
        self.assertFalse(godot.prepared(self.project))

    def test_a_failing_import_is_refused(self):
        log = self.dir / "import.log"
        with self.assertRaises(godot.GodotError) as caught:
            godot.prepare(self.importer(code=1), self.project, log=log)
        self.assertIn(f"import (exit 1) wrote no res://psiv.gdextension", str(caught.exception))
        self.assertIn(f"log={log}", str(caught.exception))

    def test_the_real_godot_prepares_a_fresh_copy_of_the_project(self):
        """The real import, on the tracked files of `godot/` alone (what a
        clone has); skipped on a host without the pinned Godot."""
        try:
            real = godot.resolve()
        except godot.GodotError as error:
            self.skipTest(f"no pinned Godot: {error}")
        for name in repo_files(ROOT, ("godot/*",)):
            target = self.dir / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes((ROOT / name).read_bytes())
        self.assertFalse(godot.prepared(self.project))
        self.assertTrue(godot.prepare(real, self.project, log=self.dir / "import.log"))
        self.assertTrue(godot.prepared(self.project))


class OnePlace(unittest.TestCase):
    """The version is written once, and the launchers ask the resolver."""

    def test_the_tracked_version_file_parses(self):
        self.assertRegex(godot.required_version(), r"\A\d+\.\d+(\.\d+)?\Z")

    def test_launchers_call_the_resolver(self):
        for launcher in ("godot/run.sh", "tools/package/deploy-deck.sh"):
            with self.subTest(launcher=launcher):
                self.assertIn("tools/godot.py", (ROOT / launcher).read_text())

    def test_every_way_to_start_the_game_prepares_the_project(self):
        for name in ("godot/run.sh", "README.md", "docs/DEVELOPMENT.md", "docs/MODDING.md"):
            with self.subTest(name=name):
                self.assertIn("python3 tools/godot.py prepare", (ROOT / name).read_text())
        for name in ("tools/certify.py", "tools/verify_native_tape.py"):
            with self.subTest(name=name):
                self.assertRegex((ROOT / name).read_text(), r"\bprepare(_godot)?\(")

    def test_only_the_resolver_knows_the_extension_index(self):
        code = [name for name in repo_files(ROOT, ("*.py", "*.sh", "*.gd"))
                if name.startswith(("tools/", "godot/", "oracle/", "psiv_tools/"))
                and name != "tools/godot.py"]
        hits = [name for name in code
                if "extension_list" in (ROOT / name).read_text(encoding="utf-8", errors="replace")]
        self.assertEqual(hits, [])

    #: A host's own Godot: anything under `~/.local/bin` named for Godot, an
    #: official download's file name, or a path carrying the pinned version.
    @staticmethod
    def host_godot_pattern(version):
        return re.compile(rf"\.local/bin/[\w.-]*godot|Godot_v[\w.]*-stable|godot-{re.escape(version)}",
                          re.IGNORECASE)

    def test_the_pattern_catches_host_godot_paths(self):
        pattern = self.host_godot_pattern("4.7.1")
        for line in (
            # tools/certify.py:64 before the resolver (commit 20a76de).
            'GODOT = os.environ.get("PSIV_GODOT", str(Path.home() / '
            '".local/bin/psiv-godot-4.7.1"))',
            'GODOT="$HOME/.local/bin/godot"',
            "/opt/Godot_v4.7.1-stable_linux.x86_64",
            "Godot_v4.7.1-stable_mono_linux_x86_64",
            "/srv/engines/godot-4.7.1/bin",
        ):
            with self.subTest(line=line):
                self.assertRegex(line, pattern)
        for line in ('PATH="$HOME/.local/bin:$PATH" cargo zigbuild',
                     "res://psiv.gdextension", "python3 tools/godot.py"):
            with self.subTest(line=line):
                self.assertNotRegex(line, pattern)

    def test_no_code_names_a_host_godot_or_its_version(self):
        pattern = self.host_godot_pattern(godot.required_version())
        code = [name for name in repo_files(ROOT)
                if name.endswith((".py", ".sh", ".gd", ".rs"))
                and name != "tests/test_godot.py"]
        hits = [f"{name}:{number}" for name in code
                for number, text in enumerate(
                    (ROOT / name).read_text(encoding="utf-8", errors="replace").splitlines(), 1)
                if pattern.search(text)]
        self.assertEqual(hits, [])


if __name__ == "__main__":
    unittest.main()
