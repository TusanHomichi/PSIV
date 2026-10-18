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
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from tools import heavy_lock as heavy_lock_module
from tools.heavy_lock import HEAVY_LOCK_ENV, HELD_CLASS_ENV, HELD_ENV, HELD_SLOTS_ENV, PRIORITY_ENV
from tools.heavy_lock_extension import can_relink_extension

REPO_ROOT = Path(__file__).resolve().parents[1]

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
        for subcommand in ("build", "b", "test", "t", "run", "r", "bench"):
            for argv in (("cargo", subcommand, *manifest, "-p", "psiv-godot"),
                         ("cargo", subcommand, *manifest, "--package=psiv-godot"),
                         ("cargo", subcommand, *manifest, "--workspace"),
                         ("cargo", subcommand, *manifest, "--all"),
                         ("cargo", subcommand, *manifest),
                         ("cargo", subcommand, "--manifest-path=rust/psiv-godot/Cargo.toml"),
                         ("cargo", "+stable", subcommand, *manifest, "--release")):
                with self.subTest(argv=argv):
                    reason = self.relinks(*argv)
                    self.assertIsNotNone(reason)
                    self.assertIn("use --class exclusive", reason)

    def test_the_directory_decides_when_no_manifest_is_given(self):
        self.assertIsNotNone(self.relinks("cargo", "test", cwd=self.rust))
        self.assertIsNotNone(self.relinks("cargo", "build", cwd=self.rust / "psiv-godot"))
        self.assertIsNone(self.relinks("cargo", "test", cwd=self.rust / "psiv-core"))
        self.assertIsNotNone(self.relinks("cargo", "test", "--workspace",
                                          cwd=self.rust / "psiv-core"))

    def test_prefixes_and_shell_scripts_are_seen_through(self):
        command = "cargo test --manifest-path rust/Cargo.toml --workspace"
        for argv in (("env", "CARGO_BUILD_JOBS=1", *command.split()),
                     ("env", "-u", "RUSTFLAGS", "A=1", *command.split()),
                     ("timeout", "-s", "KILL", "600", "nice", "-n", "5", *command.split()),
                     ("bash", "-c", f"CARGO_BUILD_JOBS=1 {command} -- --test-threads=1"),
                     ("sh", "-c", "echo start; cd rust && exec cargo test 2>&1 | tail -5"),
                     ("bash", "-lc", f"true\n{command}")):
            with self.subTest(argv=argv):
                self.assertIsNotNone(self.relinks(*argv))

    def test_a_script_it_cannot_read_is_refused_not_guessed(self):
        for script in ("cargo test $(cat packages.txt)", "cargo test `echo -p`",
                       "(cd rust && cargo check)", "cargo test -p $PKG",
                       "cd $DIR && cargo check", "cargo test 'unterminated"):
            with self.subTest(script=script):
                reason = self.relinks("bash", "-c", script)
                self.assertIsNotNone(reason)
                self.assertIn("state the class explicitly", reason)

    def test_negative_control_commands_that_cannot_relink_are_allowed(self):
        manifest = ("--manifest-path", "rust/Cargo.toml")
        for argv in (("cargo", "test", *manifest, "-p", "psiv-core"),
                     ("cargo", "test", *manifest, "--workspace", "--exclude", "psiv-godot"),
                     ("cargo", "build", *manifest, "--all", "--exclude=psiv-godot"),
                     ("cargo", "clippy", *manifest, "--workspace", "--all-targets"),
                     ("cargo", "check", *manifest, "-p", "psiv-godot"),
                     ("env", "CARGO_BUILD_JOBS=1", "cargo", "clippy", *manifest, "--workspace"),
                     ("bash", "-c", "cargo clippy --manifest-path rust/Cargo.toml --workspace"),
                     ("bash", "build/evidence/run.sh"),
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
