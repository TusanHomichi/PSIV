"""The Godot resolver: `tools/godot.py`, the one lookup every launcher uses.

    PYTHONPATH=. python3 -m unittest tests.test_godot -v

`PSIV_GODOT` wins, `godot` on PATH is the fallback, and either must report the
version `.godot-version` pins. Each case stands a small script in for Godot
that prints what a real `godot --version` prints. The negative controls are a
wrong version (a patch release, a two-digit patch that shares the pinned
prefix, an older minor), a `PSIV_GODOT` that names nothing (which must not
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

    def test_a_variable_naming_nothing_does_not_fall_back(self):
        self.fake("bin/godot")
        self.assert_refused({"PSIV_GODOT": str(self.dir / "missing"),
                             "PATH": str(self.dir / "bin")},
                            "which is not an executable file")

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


class OnePlace(unittest.TestCase):
    """The version is written once, and the launchers ask the resolver."""

    def test_the_tracked_version_file_parses(self):
        self.assertRegex(godot.required_version(), r"\A\d+\.\d+(\.\d+)?\Z")

    def test_launchers_call_the_resolver(self):
        for launcher in ("godot/run.sh", "tools/package/deploy-deck.sh"):
            with self.subTest(launcher=launcher):
                self.assertIn("tools/godot.py", (ROOT / launcher).read_text())

    #: Python tools still on their own lookup, awaiting the decision on how
    #: far into their files the resolver may reach (the lanes that own them).
    PENDING = {"tools/certify.py", "tools/verify_native_tape.py"}

    def test_no_code_names_a_host_godot_or_its_version(self):
        version = re.escape(godot.required_version())
        pattern = re.compile(rf"\.local/bin/[\w.-]*godot-\d|godot-{version}")
        code = [name for name in repo_files(ROOT)
                if name.endswith((".py", ".sh", ".gd", ".rs"))
                and name not in self.PENDING | {"tests/test_godot.py"}]
        hits = [name for name in code
                if pattern.search((ROOT / name).read_text(encoding="utf-8", errors="replace"))]
        self.assertEqual(hits, [])


if __name__ == "__main__":
    unittest.main()
