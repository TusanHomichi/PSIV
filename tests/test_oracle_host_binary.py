"""The oracle host binary is never stale (issue #61).

    PYTHONPATH=. python3 -m unittest tests.test_oracle_host_binary -v

Stdlib plus a C compiler: no ROM, no emulation core, no tape. The cases come in
four groups.

* **The fingerprint.** A copy of `oracle/host/` written in another file order,
  with other mtimes, hashes the same; one flipped byte in any single source -
  `.c` or `.h` - hashes differently; a directory with no sources is an error
  rather than the digest of nothing.
* **The helper.** `oracle/host_binary.py` replaces a hand-compiled host (one
  that answers `unknown`, which is what the three-week-old binary of #61 looked
  like) and one whose sources have since changed, and leaves a current one
  alone; with `PSIV_ORACLE_NO_REBUILD=1` it refuses instead, naming both
  fingerprints.
* **The compile list.** Every `*.c` of `oracle/host/` is in the entry point's
  link line exactly once, so a new source cannot be fingerprinted but never
  compiled.
* **The wiring.** Only the helper names the host binary's path, every launcher
  asks the helper for it, and `NegativeControlCase` shows that check can fail.
  These cases are deliberately source-level: they pin the shape of the call,
  not its behaviour, which the compile cases above cover.
"""
from __future__ import annotations

import os
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile
import unittest

from oracle import build_host, host_binary
from tools.repo_files import repo_files

ROOT = pathlib.Path(__file__).resolve().parents[1]
VERIFY_SH = ROOT / "oracle" / "verify.sh"
HELPER = ROOT / "oracle" / "host_binary.py"

#: The repository's code files: Python and shell, tracked or not.
CODE_PATTERNS = ("*.py", "*.sh")

#: The one file allowed to spell the host's path - it is the helper that hands
#: a launcher the path of a binary built from the sources in front of it.
PATH_OWNER = "oracle/host_binary.py"

#: This module defines the spellings the check must catch, so it is not its own
#: first finding; everything else in the tree is scanned, and
#: `NegativeControlCase` shows the check can fail on a file that is not exempt.
CHECK_OWNER = "tests/test_oracle_host_binary.py"
EXEMPT_FILES = (PATH_OWNER, CHECK_OWNER)

#: `bin` ... `/` ... `psiv_oracle`, however the pieces are quoted or spaced:
#: `BIN=$ORACLE/bin/psiv_oracle`, `O / "bin" / "psiv_oracle"`,
#: `["oracle/bin/psiv_oracle", ...]`.
BINARY_PATH_RE = re.compile(r"""bin["'\s]*/["'\s]*psiv_oracle""")
HOST_NAME = "psiv_oracle"
#: What may follow the host's name without it being a launch site: the source
#: file (`psiv_oracle.c`), the state document kind (`psiv_oracle_state`), or a
#: flag named beside the binary in a help string (`psiv_oracle --rng-trace`).
BENIGN_AFTER = (".c", "_state", " --")

#: Every entry point that runs the host. `oracle/fixture`, `oracle/sweep`,
#: `oracle/route.py` and `oracle/rng_trace.py` are not here: they read a run's
#: logs or plan a tape, and launch nothing.
LAUNCHERS = (
    "oracle/force/runs.py",
    "oracle/scripts/anim_sweep.py",
    "oracle/scripts/damage_census.py",
    "oracle/scripts/find_battle.py",
    "oracle/scripts/navigate.py",
    "oracle/scripts/sweep_encounter.py",
    "tools/certify.py",
    "oracle/verify.sh",
)

needs_gcc = unittest.skipUnless(shutil.which("gcc"),
                                "compiling the host needs gcc")


#: An inline-code span, which is how this repository's prose names a path or a
#: flag - ``psiv_oracle``, `oracle/bin/psiv_oracle`, `psiv_oracle --rng-trace`.
#: Stripped before the scan: a path named in documentation is not a call to it.
INLINE_CODE = re.compile(r"`+[^`]*`+")


def offending(line: str) -> bool:
    """True when `line` names the host *executable* rather than talking about it.

    Two spells to catch: the path (`.../bin/psiv_oracle`, however it is quoted
    or joined together) and a bare invocation (`["psiv_oracle", "--core"]`).
    Everything else the scanner meets is left alone - the source file
    (`psiv_oracle.c`), the state document kind (`psiv_oracle_state`), a flag
    named beside the binary in a help string (`psiv_oracle --rng-trace`), and
    anything inside an inline-code span - so the check stays about launching
    and does not police how the host is documented.
    """
    code = INLINE_CODE.sub(" ", line)
    paths = list(BINARY_PATH_RE.finditer(code))
    if paths:
        return True
    for match in re.finditer(HOST_NAME, code):
        if code[match.end():].startswith(BENIGN_AFTER):
            continue
        return True
    return False


def offenders(root: pathlib.Path = ROOT, files: list[str] | None = None) -> list[str]:
    """`path:line: text` for every code line that names the host binary.

    `files` is what to scan, repository-relative; the default is every Python
    and shell file of the change - `tools/repo_files.py`'s answer, so a
    launcher added to the tree, staged or not, is scanned the next time this
    module runs. `root` with `files` points the check at a tree of its own,
    which is how `NegativeControlCase` makes it fail on purpose.
    """
    if files is None:
        files = repo_files(root, CODE_PATTERNS)
    found = []
    for relative in files:
        if relative in EXEMPT_FILES:
            continue
        try:
            text = (pathlib.Path(root) / relative).read_text(
                encoding="utf-8", errors="replace")
        except OSError:
            continue
        for number, line in enumerate(text.splitlines(), 1):
            if offending(line):
                found.append(f"{relative}:{number}: {line.strip()}")
    return found


class MentionCase(unittest.TestCase):
    """The boundary of the check, one spelling at a time."""

    LAUNCH_SITES = (
        'BIN = ROOT / "bin" / "psiv_oracle"',
        "BIN=$ORACLE/bin/psiv_oracle",
        "        BIN=O / 'bin' / 'psiv_oracle'",
        'command = ["oracle/bin/psiv_oracle", "--core", CORE]',
        'subprocess.run(["psiv_oracle", "--core", CORE])',
        "cmd = [str(O / 'bin' / 'psiv_oracle')]",
    )
    MENTIONS_ONLY = (
        "BIN=$(python3 -m oracle.host_binary --path)",
        'argv = [str(host_binary.ensure()), "--core", str(CORE)]',
        "``psiv_oracle`` - the headless host - in prose",
        "`--trace` is the CSV `psiv_oracle --rng-trace` writes: one row per call",
        'help="CSV from psiv_oracle --rng-trace")',
        'source = read(ROOT / "oracle" / "host" / "psiv_oracle.c")',
        'self.assertEqual(state["kind"], "psiv_oracle_state")',
    )

    def test_launch_sites_are_offenders(self):
        for line in self.LAUNCH_SITES:
            with self.subTest(line=line):
                self.assertTrue(offending(line))

    def test_prose_and_other_subjects_are_not(self):
        for line in self.MENTIONS_ONLY:
            with self.subTest(line=line):
                self.assertFalse(offending(line))


class HostTreeCase(unittest.TestCase):
    """A throwaway `oracle/` tree: a copy of the sources and a place to build."""

    def setUp(self):
        self._dir = tempfile.TemporaryDirectory(prefix="psiv-host-")
        self.addCleanup(self._dir.cleanup)
        self.tree = pathlib.Path(self._dir.name)
        self.host = self.tree / "host"
        self.binary = self.tree / "bin" / "psiv_oracle"
        shutil.copytree(host_binary.HOST_DIR, self.host)

    def sources(self) -> list[pathlib.Path]:
        return host_binary.host_sources(self.host)

    def ensure(self, **kwargs):
        return host_binary.ensure(binary=self.binary, host_dir=self.host, **kwargs)

    def hand_build(self) -> None:
        """Compile the sources the way a lane without the entry point would.

        No `-DPSIV_ORACLE_BUILD_ID`, so the binary answers `unknown`: the shape
        of the host #61 found, built by hand and free to fall behind.
        """
        command = [arg for arg in build_host.compile_command(
            self.binary, self.host, "unused")
            if not arg.startswith(f"-D{host_binary.BUILD_ID_DEFINE}")]
        self.binary.parent.mkdir(parents=True, exist_ok=True)
        built = subprocess.run(command, capture_output=True, text=True)
        self.assertEqual(0, built.returncode, built.stderr)

    def flip_a_byte(self, path: pathlib.Path) -> bytes:
        original = path.read_bytes()
        changed = bytearray(original)
        changed[len(changed) // 2] ^= 0x01
        path.write_bytes(bytes(changed))
        return original


class FingerprintCase(HostTreeCase):
    """The number the build takes from the sources, and what moves it."""

    def test_a_copy_in_another_order_with_other_mtimes_hashes_the_same(self):
        """Nothing but the bytes decides the fingerprint.

        The copy is written in reverse name order with mtimes an hour apart, so
        a filesystem handing entries over in its own order, and a checkout
        unpacked at another time, still hash to the same value - which is what
        the check rests on: two trees agree about which host is current only if
        the fingerprint is a function of the sources alone.
        """
        other = self.tree / "elsewhere"
        other.mkdir()
        for path in reversed(self.sources()):
            shutil.copy2(path, other / path.name)
            os.utime(other / path.name, (1_000_000, 1_000_000))
        self.assertEqual(host_binary.source_fingerprint(self.host),
                         host_binary.source_fingerprint(other))
        self.assertEqual(host_binary.source_fingerprint(),
                         host_binary.source_fingerprint(other))

    def test_any_single_source_byte_moves_the_fingerprint(self):
        """Every `.c` and `.h` is in the digest, one byte at a time."""
        want = host_binary.source_fingerprint(self.host)
        for path in self.sources():
            with self.subTest(source=path.name):
                original = self.flip_a_byte(path)
                try:
                    self.assertNotEqual(want,
                                        host_binary.source_fingerprint(self.host))
                finally:
                    path.write_bytes(original)
                self.assertEqual(want, host_binary.source_fingerprint(self.host))

    def test_a_tree_with_no_sources_is_an_error(self):
        """A digest of nothing would match a binary built from nothing."""
        empty = self.tree / "empty"
        empty.mkdir()
        with self.assertRaises(host_binary.HostBinaryError):
            host_binary.source_fingerprint(empty)
        with self.assertRaises(host_binary.HostBinaryError):
            host_binary.source_fingerprint(self.tree / "absent")


class RebuildSwitchCase(unittest.TestCase):
    """`PSIV_ORACLE_NO_REBUILD`: the documented refusal and its spellings."""

    def test_the_switch_reads_the_values_the_readme_documents(self):
        for value in ("1", "yes", "true", "on", " TRUE "):
            with self.subTest(value=value):
                self.assertFalse(host_binary.rebuilding_allowed(
                    {host_binary.NO_REBUILD_ENV: value}))
        for value in ("", "0", "no", "false", "off"):
            with self.subTest(value=value):
                self.assertTrue(host_binary.rebuilding_allowed(
                    {host_binary.NO_REBUILD_ENV: value}))
        self.assertTrue(host_binary.rebuilding_allowed({}))


class HelperCase(HostTreeCase):
    """What a launcher gets back from `ensure()`."""

    @needs_gcc
    def test_a_hand_built_host_is_replaced(self):
        self.hand_build()
        self.assertEqual("unknown", host_binary.reported_build_id(self.binary))
        self.assertEqual(self.binary, self.ensure(rebuild=True))
        self.assertEqual(host_binary.source_fingerprint(self.host),
                         host_binary.reported_build_id(self.binary))

    @needs_gcc
    def test_a_host_from_changed_sources_is_replaced(self):
        self.ensure(rebuild=True)
        first = host_binary.reported_build_id(self.binary)
        self.flip_a_byte(self.host / "tape.c")
        self.assertEqual(self.binary, self.ensure(rebuild=True))
        second = host_binary.reported_build_id(self.binary)
        self.assertNotEqual(first, second)
        self.assertEqual(host_binary.source_fingerprint(self.host), second)

    @needs_gcc
    def test_a_current_host_is_returned_untouched(self):
        """The check is cheap, and the rebuild only happens on a mismatch: the
        binary's own mtime is what says no second compile ran."""
        self.ensure(rebuild=True)
        stamp = self.binary.stat().st_mtime_ns
        self.ensure(rebuild=True)
        self.assertEqual(stamp, self.binary.stat().st_mtime_ns)

    def test_rebuilding_off_refuses_and_names_both_fingerprints(self):
        """The escape hatch for a tree that must not be written to."""
        want = host_binary.source_fingerprint(self.host)
        with self.assertRaises(host_binary.StaleHostError) as caught:
            self.ensure(rebuild=False)
        message = str(caught.exception)
        self.assertIn(want, message)
        self.assertIn(host_binary.NO_REBUILD_ENV, message)
        self.assertFalse(self.binary.exists(), "a refusal builds nothing")

    @needs_gcc
    def test_the_environment_can_turn_rebuilding_off(self):
        self.hand_build()
        proc = self.helper_run("--path", NO_REBUILD="1")
        self.assertEqual(1, proc.returncode, proc.stdout + proc.stderr)
        self.assertIn(host_binary.source_fingerprint(self.host), proc.stderr)
        self.assertIn("unknown", proc.stderr)
        self.assertEqual("unknown", host_binary.reported_build_id(self.binary))

    @needs_gcc
    def test_the_helper_prints_the_path_of_the_host_it_kept_current(self):
        proc = self.helper_run("--path")
        self.assertEqual(0, proc.returncode, proc.stderr)
        self.assertEqual([str(self.binary)], proc.stdout.split())
        self.assertEqual(host_binary.source_fingerprint(self.host),
                         host_binary.reported_build_id(self.binary))

    @needs_gcc
    def test_the_helper_prints_the_build_id_the_host_reports(self):
        proc = self.helper_run("--build-id")
        self.assertEqual(0, proc.returncode, proc.stderr)
        self.assertEqual([host_binary.source_fingerprint(self.host)],
                         proc.stdout.split())

    def helper_run(self, *args, NO_REBUILD=None) -> subprocess.CompletedProcess:
        """`python3 -m oracle.host_binary` over this case's own tree."""
        env = {**os.environ, "PYTHONPATH": str(ROOT)}
        if NO_REBUILD is not None:
            env[host_binary.NO_REBUILD_ENV] = NO_REBUILD
        else:
            env.pop(host_binary.NO_REBUILD_ENV, None)
        return subprocess.run(
            [sys.executable, "-m", "oracle.host_binary", *args,
             "--binary", str(self.binary), "--host-dir", str(self.host)],
            cwd=ROOT, env=env, capture_output=True, text=True)


class BuildEntryPointCase(unittest.TestCase):
    """The one place the compile command lives."""

    def test_the_compile_list_names_every_host_source_once(self):
        """A `.c` added to `oracle/host/` without a line in the list would be
        fingerprinted but never compiled: the build id would move while the
        binary's behaviour stayed behind."""
        compiled = list(build_host.SOURCES)
        self.assertEqual(len(compiled), len(set(compiled)))
        self.assertEqual(sorted(compiled),
                         sorted(path.name for path in host_binary.host_sources()
                                if path.suffix == ".c"))

    def test_the_entry_point_is_what_the_helper_rebuilds_through(self):
        """`ensure()` runs the entry point as a module, so the host a launcher
        gets is the host a person gets from the command line."""
        text = HELPER.read_text()
        self.assertRegex(text, r"""["']oracle\.build_host["']""")
        self.assertRegex(text, r"""["']-m["'],\s*["']oracle\.build_host["']""")


class LauncherWiringCase(unittest.TestCase):
    """Every launcher asks the helper, and only the helper names the path."""

    def test_no_file_but_the_helper_names_the_host_binary(self):
        found = offenders()
        self.assertEqual(
            [], found,
            "the host binary's path is named outside "
            f"{PATH_OWNER}, so that file can launch a host older than the "
            "sources it was built from:\n  " + "\n  ".join(found)
            + "\nAsk oracle.host_binary for the path (`ensure()`, or "
              "`python3 -m oracle.host_binary --path` from a shell script).")

    def test_the_known_launchers_ask_the_helper(self):
        for relative in LAUNCHERS:
            with self.subTest(launcher=relative):
                self.assertIn("host_binary", (ROOT / relative).read_text(),
                              f"{relative} does not go through the helper")

    def test_verify_builds_through_the_entry_point(self):
        text = VERIFY_SH.read_text()
        self.assertIn("python3 -m oracle.build_host", text)
        self.assertIn("python3 -m oracle.host_binary --path", text)
        self.assertNotIn("gcc", text, "verify.sh still compiles the host inline")


class NegativeControlCase(unittest.TestCase):
    """A launcher that names the path is caught - the check can fail."""

    def test_a_new_launcher_is_caught_with_its_line(self):
        body = ('BIN = ROOT / "oracle" / "bin" / "psiv_oracle"\n'
                + 'subprocess.run([str(BIN), "--core", CORE])\n')
        with tempfile.TemporaryDirectory(prefix="psiv-host-wiring-") as name:
            launcher = pathlib.Path(name) / "oracle" / "new_launcher.py"
            launcher.parent.mkdir(parents=True)
            launcher.write_text(body)
            found = offenders(root=pathlib.Path(name),
                              files=["oracle/new_launcher.py"])
        self.assertEqual(1, len(found), found)
        self.assertIn("oracle/new_launcher.py:1", found[0])
        self.assertIn("bin", found[0])
        self.assertIn("psiv_oracle", found[0])
