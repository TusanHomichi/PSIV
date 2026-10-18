"""tools/heavy_lock_extension.py and `heavy_lock.py run`'s refusal of a shared
class for a command that can relink the GDExtension library.

    PYTHONPATH=. python3 -m unittest tests.test_heavy_lock_extension -v

Hermetic: a throwaway cargo workspace (`rust/` with psiv-core and psiv-godot
members) in a temp directory; nothing is compiled. `CanRelinkCase` asks
`can_relink_extension` about each command form: every linking form that can
compile psiv-godot is named, the ones that cannot (another package, an
`--exclude`, `check`, `clippy`) are not, and a shell script it cannot read is
refused rather than guessed. `RunRefusalCase` runs `heavy_lock.main` with a fake
`cargo` on PATH: `--class build -- cargo test --workspace` exits 2 naming
`--class exclusive` and runs nothing, while the negative control, the old
behaviour (no check), lets the same command through.
"""

import contextlib
import io
import os
import re
import shlex
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from tools import heavy_lock as heavy_lock_module
from tools.heavy_lock import HEAVY_LOCK_ENV, HELD_CLASS_ENV, HELD_ENV, HELD_SLOTS_ENV, PRIORITY_ENV
from tools.heavy_lock_extension import can_relink_extension

REPO_ROOT = Path(__file__).resolve().parents[1]

#: `heavy_lock.py run [--priority] --class C -- <command>` in a document.
DOCUMENTED = re.compile(r"heavy_lock\.py run (?:--priority )?--class (?P<cls>\w+) -- "
                        r"(?P<cmd>[^`\n]+)")

#: A fake cargo: appends its arguments to $FAKE_CARGO_LOG.
FAKE_CARGO = '#!/bin/sh\necho "$@" >> "$FAKE_CARGO_LOG"\n'


def make_workspace(base):
    """`base/rust`: a virtual workspace of psiv-core and psiv-godot."""
    rust = base / "rust"
    for member in ("psiv-core", "psiv-godot"):
        (rust / member).mkdir(parents=True)
        (rust / member / "Cargo.toml").write_text(
            f'[package]\nname = "{member}"\nversion = "0.1.0"\n')
    (rust / "Cargo.toml").write_text(
        '[workspace]\nresolver = "2"\nmembers = ["psiv-core", "psiv-godot"]\n')
    return rust


class CanRelinkCase(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.base = Path(tmp.name).resolve()
        self.rust = make_workspace(self.base)

    def relinks(self, *argv, cwd=None):
        return can_relink_extension(list(argv), cwd or self.base)

    def test_every_linking_form_that_compiles_psiv_godot_is_named(self):
        manifest = ("--manifest-path", "rust/Cargo.toml")
        for subcommand in ("build", "b", "test", "t", "run", "r", "bench", "rustc"):
            for argv in (("cargo", subcommand, *manifest, "-p", "psiv-godot"),
                         ("cargo", subcommand, *manifest, "--package=psiv-godot"),
                         ("cargo", subcommand, *manifest, "-ppsiv-godot"),
                         ("cargo", subcommand, *manifest, "-p=psiv-godot"),
                         ("cargo", subcommand, *manifest, "--workspace"),
                         ("cargo", subcommand, *manifest, "--all"),
                         ("cargo", subcommand, *manifest),
                         ("cargo", subcommand, "--manifest-path=rust/psiv-godot/Cargo.toml"),
                         ("cargo", "+stable", subcommand, *manifest, "--release"),
                         ("cargo", "--locked", "-v", subcommand, *manifest)):
                with self.subTest(argv=argv):
                    reason = self.relinks(*argv)
                    self.assertIsNotNone(reason)
                    self.assertIn("use --class exclusive", reason)

    def test_package_globs_and_id_specs_are_matched(self):
        manifest = ("--manifest-path", "rust/Cargo.toml")
        for spec in ("psiv-*", "psiv-go?ot", "*", "psiv-godot@0.1.0", "psiv-godot:0.1.0",
                     "path+file:///work/rust/psiv-godot#0.1.0",
                     "https://example.invalid/psiv#psiv-godot@0.1.0"):
            with self.subTest(spec=spec):
                self.assertIsNotNone(self.relinks("cargo", "test", *manifest, "-p", spec))
        self.assertIsNone(self.relinks("cargo", "test", *manifest, "--workspace",
                                       "--exclude", "psiv-g*"))

    def test_subcommands_outside_the_allowlist_are_refused(self):
        """rustc, plugins and aliases may link: only known non-linking ones pass."""
        manifest = ("--manifest-path", "rust/Cargo.toml")
        for argv in (("cargo", "zigbuild", *manifest, "--workspace"),
                     ("cargo", "nextest", "run", *manifest, "-p", "psiv-godot"),
                     ("cargo", "--config", "alias.xb='build'", "xb", *manifest),
                     ("cargo", "my-plugin", *manifest),
                     ("cargo-nextest", "nextest", "run", *manifest, "--workspace")):
            with self.subTest(argv=argv):
                self.assertIsNotNone(self.relinks(*argv))
        for argv in (("cargo", "nextest", "run"), ("cargo", "install", "--path", "rust/x"),
                     ("cargo", "--unknown-global", "check")):
            with self.subTest(argv=argv):  # a selection it cannot read
                self.assertIn("state the class explicitly", self.relinks(*argv))

    def test_the_directory_decides_when_no_manifest_is_given(self):
        self.assertIsNotNone(self.relinks("cargo", "test", cwd=self.rust))
        self.assertIsNotNone(self.relinks("cargo", "build", cwd=self.rust / "psiv-godot"))
        self.assertIsNone(self.relinks("cargo", "test", cwd=self.rust / "psiv-core"))
        self.assertIsNotNone(self.relinks("cargo", "test", "--workspace",
                                          cwd=self.rust / "psiv-core"))
        self.assertIsNotNone(self.relinks("cargo", "-C", "rust", "test"))
        self.assertIsNotNone(self.relinks("env", "-C", "rust", "cargo", "test"))
        self.assertIsNotNone(self.relinks("env", "--chdir=rust/psiv-godot", "cargo", "build"))

    def test_prefixes_and_shell_scripts_are_seen_through(self):
        command = "cargo test --manifest-path rust/Cargo.toml --workspace"
        for argv in (("env", "CARGO_BUILD_JOBS=1", *command.split()),
                     ("env", "-u", "RUSTFLAGS", "A=1", *command.split()),
                     ("env", "-S", f"A=1 {command}"),
                     ("env", f"--split-string=A=1 {command}"),
                     ("timeout", "-s", "KILL", "600", "nice", "-n", "5", *command.split()),
                     ("time", "-f", "%M", "-o", "rss.txt", *command.split()),
                     ("bash", "-c", f"CARGO_BUILD_JOBS=1 {command} -- --test-threads=1"),
                     ("bash", "-euo", "pipefail", "-c", command),
                     ("bash", "-e", "-o", "pipefail", "-c", command),
                     ("bash", "-l", "-c", command),
                     ("bash", "--norc", "-ec", command),
                     ("sh", "-c", "echo start; cd rust && exec cargo test 2>&1 | tail -5"),
                     ("bash", "-lc", f"true\n{command}"),
                     ("bash", "-c", f"if true; then {command}; fi"),
                     ("bash", "-c", f"for i in 1 2; do {command}; done"),
                     ("bash", "-c", f"true || ! {command}"),
                     ("bash", "-c", f"if false; then true; elif {command}; then true; fi"),
                     ("bash", "-c", "pushd rust && cargo test; popd"),
                     ("bash", "-c", "pushd rust/psiv-core && popd && cd rust && cargo build")):
            with self.subTest(argv=argv):
                self.assertIsNotNone(self.relinks(*argv))

    def test_a_command_it_cannot_read_is_refused_not_guessed(self):
        for argv in (("bash", "-c", "cargo test $(cat packages.txt)"),
                     ("bash", "-c", "cargo test `echo -p`"),
                     ("bash", "-c", "(cd rust && cargo check)"),
                     ("bash", "-c", "{ cargo check; }"),
                     ("bash", "-c", "cargo test -p $PKG"),
                     ("bash", "-c", "cd $DIR && cargo check"),
                     ("bash", "-c", "cd ~/work/rust && cargo check"),
                     ("bash", "-c", "cd - && cargo check"),
                     ("bash", "-c", "popd; cargo check"),
                     ("bash", "-c", "cargo test 'unterminated"),
                     ("bash", "-c", "python3 -c 'import subprocess; subprocess.run([\"cargo\"])'"),
                     ("xargs", "cargo", "test"),
                     ("stdbuf", "-oL", "cargo", "test", "--workspace"),
                     ("setsid", "cargo", "build"),
                     ("ionice", "-c3", "cargo", "build"),
                     ("taskset", "-c", "0-3", "cargo", "build"),
                     ("python3", "-c", "import subprocess; subprocess.run(['cargo', 'build'])"),
                     ("env", "-S", "cargo test 'unterminated"),
                     ("env", "-C", "~/rust", "cargo", "test")):
            with self.subTest(argv=argv):
                reason = self.relinks(*argv)
                self.assertIsNotNone(reason)
                self.assertIn("state the class explicitly", reason)

    def test_negative_control_commands_that_cannot_relink_are_allowed(self):
        manifest = ("--manifest-path", "rust/Cargo.toml")
        for argv in (("cargo", "test", *manifest, "-p", "psiv-core"),
                     ("cargo", "test", *manifest, "--workspace", "--exclude", "psiv-godot"),
                     ("cargo", "build", *manifest, "--all", "--exclude=psiv-godot"),
                     ("cargo", "clippy", *manifest, "--workspace", "--all-targets"),
                     ("cargo", "clippy", *manifest, "-p", "psiv-godot"),
                     ("cargo", "check", *manifest, "--workspace"),
                     ("cargo", "check", *manifest, "-p", "psiv-godot"),
                     ("cargo", "fmt", *manifest, "--all", "--check"),
                     ("cargo", "doc", *manifest, "--workspace"),
                     ("cargo", "tree", *manifest), ("cargo", "--version"),
                     ("env", "CARGO_BUILD_JOBS=1", "cargo", "clippy", *manifest, "--workspace"),
                     ("bash", "-euo", "pipefail", "-c",
                      "cargo clippy --manifest-path rust/Cargo.toml --workspace"),
                     ("bash", "build/evidence/run.sh"),
                     ("make", "all"),
                     ("python3", "-m", "unittest", "discover", "-s", "tests"),
                     ("cargo", "test", *manifest, "-p", "psiv-core", "--", "-p", "psiv-godot")):
            with self.subTest(argv=argv):
                self.assertIsNone(self.relinks(*argv))

    def test_the_repositorys_own_workspace_is_read(self):
        self.assertIsNotNone(can_relink_extension(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--workspace"], REPO_ROOT))
        self.assertIsNone(can_relink_extension(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "-p", "psiv-runtime"],
            REPO_ROOT))

    def test_every_documented_shared_class_command_is_allowed(self):
        """The repository's own `heavy_lock.py run --class <shared> -- ...` lines
        (backslash continuations joined) pass the guard."""
        found = []
        for doc in [REPO_ROOT / "AGENTS.md", *sorted((REPO_ROOT / "docs").rglob("*.md"))]:
            text = doc.read_text().replace("\\\n", " ")
            for match in DOCUMENTED.finditer(text):
                command = match["cmd"].strip()
                if match["cls"] != "exclusive" and "..." not in command and "<" not in command:
                    found.append((doc.name, command))  # placeholders are not commands
        self.assertTrue(found, "no documented shared-class command found")
        for name, command in found:
            with self.subTest(doc=name, command=command):
                self.assertIsNone(can_relink_extension(shlex.split(command), REPO_ROOT))


class RunRefusalCase(unittest.TestCase):
    """`heavy_lock.main(["run", ...])` in the throwaway workspace, fake cargo on PATH."""

    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        base = Path(tmp.name).resolve()
        self.rust = make_workspace(base)
        bin_dir = base / "bin"
        bin_dir.mkdir()
        (bin_dir / "cargo").write_text(FAKE_CARGO)
        (bin_dir / "cargo").chmod(0o755)
        self.log = base / "cargo.log"
        environ = {key: value for key, value in os.environ.items()
                   if key not in (HELD_ENV, HELD_CLASS_ENV, HELD_SLOTS_ENV, PRIORITY_ENV)}
        environ.update({HEAVY_LOCK_ENV: str(base / "heavy.lock"), "FAKE_CARGO_LOG": str(self.log),
                        "PATH": f"{bin_dir}{os.pathsep}{os.environ.get('PATH', '')}"})
        patcher = mock.patch.dict(os.environ, environ, clear=True)
        patcher.start()
        self.addCleanup(patcher.stop)
        self.addCleanup(os.chdir, os.getcwd())
        os.chdir(self.rust)

    def run_main(self, *argv):
        err = io.StringIO()
        with contextlib.redirect_stderr(err):
            code = heavy_lock_module.main(["run", *argv])
        ran = self.log.read_text().splitlines() if self.log.exists() else []
        return code, err.getvalue(), ran

    def test_a_build_class_workspace_test_is_refused_and_runs_nothing(self):
        code, err, ran = self.run_main("--class", "build", "--", "cargo", "test", "--workspace")
        self.assertEqual(code, 2, err)
        self.assertIn("refused --class build", err)
        self.assertIn("use --class exclusive", err)
        self.assertEqual(ran, [])

    def test_the_reviewers_end_to_end_bypass_is_refused(self):
        """`bash -euo pipefail -c` once hid the -c script, and cargo ran."""
        code, err, ran = self.run_main(
            "--class", "build", "--", "bash", "-euo", "pipefail", "-c",
            "cargo build --manifest-path Cargo.toml -p psiv-godot")
        self.assertEqual(code, 2, err)
        self.assertIn("use --class exclusive", err)
        self.assertEqual(ran, [])

    def test_every_shared_class_is_refused(self):
        for lock_class in ("godot", "python"):
            code, err, ran = self.run_main("--class", lock_class, "--", "cargo", "build",
                                           "-p", "psiv-godot")
            self.assertEqual(code, 2, err)
            self.assertEqual(ran, [])

    def test_the_same_command_runs_in_the_exclusive_class(self):
        code, err, ran = self.run_main("--class", "exclusive", "--", "cargo", "test",
                                       "--workspace")
        self.assertEqual(code, 0, err)
        self.assertEqual(ran, ["test --workspace"])

    def test_a_build_class_test_of_another_package_runs(self):
        code, err, ran = self.run_main("--class", "build", "--", "cargo", "test", "-p",
                                       "psiv-core")
        self.assertEqual(code, 0, err)
        self.assertEqual(ran, ["test -p psiv-core"])

    def test_negative_control_the_old_behaviour_lets_the_workspace_test_through(self):
        """Without the check (as before this guard), the race is open."""
        with mock.patch.object(heavy_lock_module, "can_relink_extension", lambda *a, **k: None):
            code, err, ran = self.run_main("--class", "build", "--", "cargo", "test",
                                           "--workspace")
        self.assertEqual(code, 0, err)
        self.assertEqual(ran, ["test --workspace"])


if __name__ == "__main__":
    unittest.main()
