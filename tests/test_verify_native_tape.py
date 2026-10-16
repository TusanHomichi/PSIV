"""The native verifier must reject changed code, even with a passing game."""

from pathlib import Path
import hashlib
import os
import signal
import subprocess
import sys
import tempfile
import time
import unittest

from unittest import mock
import contextlib
import io
import json

from tools import verify_native_tape
from tools.heavy_lock import (HEAVY_LOCK_ENV, HELD_CLASS_ENV, HELD_ENV, HELD_SLOTS_ENV,
                              PRIORITY_ENV)
from tests.test_native_rss import samples
from tools.verify_native_tape import (file_identity, host_power, identities_stable,
                                      source_identity, terminate_process_group)


#: Every case's heavy lock is a throwaway file, never the repository's own: the
#: verifiers take a godot slot themselves, and a held real lock must not stall
#: (or be stalled by) these cases.
_LOCK_DIR = tempfile.TemporaryDirectory(prefix="native-lock-")
_LOCK_ENV = mock.patch.dict(os.environ, {HEAVY_LOCK_ENV: str(Path(_LOCK_DIR.name) / "heavy.lock")})


def setUpModule():
    _LOCK_ENV.start()  # first, so its stop restores the markers popped below
    for key in (HELD_ENV, HELD_CLASS_ENV, HELD_SLOTS_ENV, PRIORITY_ENV):
        os.environ.pop(key, None)


def tearDownModule():
    _LOCK_ENV.stop()
    _LOCK_DIR.cleanup()


class NativeTapeIdentityGuard(unittest.TestCase):
    def test_direct_cli_imports_the_file_list_owner_without_pythonpath(self):
        root = Path(__file__).resolve().parents[1]
        env = dict(os.environ)
        env.pop("PYTHONPATH", None)
        run = subprocess.run([sys.executable, str(root / "tools/verify_native_tape.py"),
                              "--help"], cwd=root, env=env,
                             capture_output=True, text=True)
        self.assertEqual(run.returncode, 0, run.stderr)
        self.assertIn("--tape", run.stdout)

    def test_changed_extension_or_driver_cannot_keep_a_stable_receipt(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            paths = {"extension": root / "libpsiv_godot.so",
                     "driver": root / "native_tape.gd"}
            for name, path in paths.items():
                path.write_bytes(f"stub {name}".encode())
            before = {name: file_identity(path) for name, path in paths.items()}
            git = {"head": "fixed", "dirty_paths": [], "diff_sha256": "fixed"}
            self.assertTrue(identities_stable(before, before, git, git,
                                              "pack", "pack", "tape", "tape"))
            for changed in paths:
                with self.subTest(changed=changed):
                    before = {name: file_identity(path) for name, path in paths.items()}
                    original = paths[changed].read_bytes()
                    paths[changed].write_bytes(original + b" changed")
                    after = {name: file_identity(path) for name, path in paths.items()}
                    self.assertNotEqual(before[changed]["sha256"], after[changed]["sha256"])
                    self.assertFalse(identities_stable(before, after, git, git,
                                                       "pack", "pack", "tape", "tape"))
                    paths[changed].write_bytes(original)

    def test_untracked_directory_symlink_is_hashed_without_following_it(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            subprocess.run(["git", "-C", str(root), "-c", "user.name=Test",
                            "-c", "user.email=test@example.invalid", "commit",
                            "--allow-empty", "-qm", "base"], check=True)
            link = root / "runtime-pack"
            first = root / "missing-protected-assets"
            link.symlink_to(first, target_is_directory=True)
            before = source_identity(root)
            self.assertEqual(before["repository_entries"]["runtime-pack"]["kind"],
                             "symlink")
            self.assertEqual(before["repository_entries"]["runtime-pack"]["link_sha256"],
                             hashlib.sha256(os.fsencode(str(first))).hexdigest())
            link.unlink()
            link.symlink_to(root / "another-missing-directory", target_is_directory=True)
            after = source_identity(root)
            self.assertNotEqual(before["repository_entries"], after["repository_entries"])
            self.assertFalse(identities_stable({}, {}, before, after,
                                               "pack", "pack", "tape", "tape"))

    def test_quoted_and_newline_filename_keeps_its_actual_path(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            subprocess.run(["git", "-C", str(root), "-c", "user.name=Test",
                            "-c", "user.email=test@example.invalid", "commit",
                            "--allow-empty", "-qm", "base"], check=True)
            name = 'new "quote"\nline.txt'
            path = root / name
            path.write_bytes(b"first")
            before = source_identity(root)
            self.assertEqual(before["repository_entries"][name]["sha256"],
                             hashlib.sha256(b"first").hexdigest())
            self.assertIn({"status": "??", "path": name}, before["dirty_status"])
            path.write_bytes(b"second")
            after = source_identity(root)
            self.assertNotEqual(before["repository_entries"], after["repository_entries"])
            self.assertFalse(identities_stable({}, {}, before, after,
                                               "pack", "pack", "tape", "tape"))

    def test_timeout_cleanup_kills_owned_child_not_only_shell(self):
        with tempfile.TemporaryDirectory() as directory:
            pid_file = Path(directory) / "child.pid"
            parent_code = (
                "import pathlib, subprocess, sys, time; "
                "child=subprocess.Popen([sys.executable, '-c', "
                "'import signal,time; signal.signal(signal.SIGTERM, signal.SIG_IGN); time.sleep(60)']); "
                "pathlib.Path(sys.argv[1]).write_text(str(child.pid)); time.sleep(60)"
            )
            parent = subprocess.Popen([sys.executable, "-c", parent_code,
                                       str(pid_file)], start_new_session=True)
            child_pid = None
            try:
                deadline = time.monotonic() + 5
                while not pid_file.is_file() and time.monotonic() < deadline:
                    time.sleep(0.02)
                self.assertTrue(pid_file.is_file())
                child_pid = int(pid_file.read_text())
                terminate_process_group(parent, grace_s=0.2)
                self.assertIsNotNone(parent.returncode)
                child_stat = Path(f"/proc/{child_pid}/stat")
                deadline = time.monotonic() + 2
                while True:
                    try:
                        state = child_stat.read_text().split(") ", 1)[1][0]
                    except FileNotFoundError:
                        break
                    if state in {"Z", "X"}:
                        break
                    if time.monotonic() >= deadline:
                        self.fail(f"owned child {child_pid} survived timeout cleanup: {state}")
                    time.sleep(0.02)
            finally:
                if parent.poll() is None:
                    terminate_process_group(parent, grace_s=0.1)
                if child_pid is not None:
                    try:
                        state = Path(f"/proc/{child_pid}/stat").read_text().split(") ", 1)[1][0]
                    except FileNotFoundError:
                        pass
                    else:
                        if state not in {"Z", "X"}:
                            os.kill(child_pid, signal.SIGKILL)


class NativeTapeHostPower(unittest.TestCase):
    def test_host_power_reads_supplies_the_cpu_cap_and_the_load(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "sys/class/power_supply/AC").mkdir(parents=True)
            (root / "sys/class/power_supply/AC/type").write_text("Mains\n")
            (root / "sys/class/power_supply/AC/online").write_text("1\n")
            cpufreq = root / "sys/devices/system/cpu/cpu0/cpufreq"
            cpufreq.mkdir(parents=True)
            (cpufreq / "bios_limit").write_text("2300000\n")
            (root / "proc").mkdir()
            (root / "proc/loadavg").write_text("2.68 2.41 2.31 3/912 130360\n")
            power = host_power(root / "sys", root / "proc")
        self.assertEqual(power["power_supply"]["AC"]["online"], "1")
        self.assertIsNone(power["power_supply"]["AC"]["status"])
        self.assertEqual(power["cpu0_bios_limit_khz"], "2300000")
        self.assertIsNone(power["cpu0_scaling_max_freq_khz"])
        self.assertEqual(power["loadavg"], "2.68 2.41 2.31 3/912 130360")


FAKE_GODOT = """#!{python}
# Stands in for Godot: writes the driver's report and slot snapshot.
import json, os, sys
samples = json.loads(os.environ["FAKE_RSS_SAMPLES"])
with open(os.environ["PSIV_TAPE_SNAPSHOT_OUT"], "wb") as out:
    out.write(b"slot")
with open(os.environ["PSIV_TAPE_REPORT"], "w") as out:
    json.dump({{"result": "pass", "rss_samples": samples}}, out)
"""


class NativeTapeReceiptRss(unittest.TestCase):
    """`main` itself: a failing RSS verdict fails the receipt, everything else
    being a pass (Godot, the process map and Git are stood in for)."""

    def run_main(self, rows, extra=()):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "godot/.godot").mkdir(parents=True)
            (root / "godot/.godot/extension_list.cfg").write_text("res://psiv.gdextension\n")
            library = root / "rust/target/debug/libpsiv_godot.so"
            library.parent.mkdir(parents=True)
            library.write_bytes(b"extension")
            (root / "tools/native").mkdir(parents=True)
            (root / "tools/native/native_tape.gd").write_text("# driver\n")
            pack = root / "pack"
            pack.mkdir()
            (pack / "manifest.json").write_text("{}")
            tape = root / "run.tape"
            tape.write_text("tape")
            godot = root / "godot-bin"
            godot.write_text(FAKE_GODOT.format(python=sys.executable))
            godot.chmod(0o755)
            mapped = {"path": str(library.resolve()), "inode": library.stat().st_ino}
            env = {"FAKE_RSS_SAMPLES": json.dumps(samples(*rows))}
            with mock.patch.object(verify_native_tape, "ROOT", root), \
                    mock.patch.object(verify_native_tape, "GODOT", godot), \
                    mock.patch.object(verify_native_tape, "mapped_extension_in_process_tree",
                                      return_value=mapped), \
                    mock.patch.object(verify_native_tape, "source_identity",
                                      return_value={"head": "test"}), \
                    mock.patch.dict(os.environ, env), \
                    contextlib.redirect_stdout(io.StringIO()):
                code = verify_native_tape.main([
                    "--tape", str(tape), "--pack", str(pack), "--out", str(root / "out"),
                    "--timeout", "60", "--rss-every", "1000", *extra])
                receipt = json.loads((root / "out/receipt.json").read_text())
        return code, receipt

    def test_a_flat_run_passes_and_a_failing_rss_verdict_fails_the_receipt(self):
        bound = ("--rss-warmup-frames", "2000", "--rss-bound-mb", "16")
        flat = [(300, 100, 10, 1)] * 6
        code, receipt = self.run_main(flat, bound)
        self.assertEqual(code, 0, receipt.get("rss"))
        self.assertIs(receipt["pass"], True)
        growing = [(300, 100, 10, 1)] * 4 + [(400, 100, 10, 1), (500, 100, 10, 1)]
        code, receipt = self.run_main(growing, bound)
        self.assertEqual(code, 1)
        self.assertIs(receipt["rss"]["pass"], False)
        self.assertIs(receipt["pass"], False)

    def test_a_bound_without_a_warmup_of_one_sample_is_refused(self):
        for extra in (("--rss-bound-mb", "16"),
                      ("--rss-warmup-frames", "999", "--rss-bound-mb", "16")):
            with self.assertRaises(SystemExit), contextlib.redirect_stderr(io.StringIO()):
                self.run_main([(300, 100, 10, 1)] * 6, extra)

class TakesItsGodotSlot(unittest.TestCase):
    """The replay holds one godot-class slot of the heavy lock itself."""

    def held_while_replaying(self, *wrapper):
        seen = {}

        def replay(args):
            seen.update({key: os.environ.get(key) for key in (HELD_ENV, HELD_CLASS_ENV)})
            return 0

        with mock.patch.object(verify_native_tape, "replay", replay), wrapper[0] if wrapper \
                else contextlib.nullcontext():
            self.assertEqual(verify_native_tape.main(["--tape", "t", "--out", "o"]), 0)
        return seen

    def test_the_replay_holds_a_godot_slot_of_the_heavy_lock(self):
        lock = str(Path(os.environ[HEAVY_LOCK_ENV]).resolve())
        self.assertEqual(self.held_while_replaying(), {HELD_ENV: lock, HELD_CLASS_ENV: "godot"})

    def test_negative_control_under_an_exclusive_holder_it_takes_nothing_new(self):
        lock = str(Path(os.environ[HEAVY_LOCK_ENV]).resolve())
        outer = mock.patch.dict(os.environ, {HELD_ENV: lock, HELD_CLASS_ENV: "exclusive"})
        self.assertEqual(self.held_while_replaying(outer),
                         {HELD_ENV: lock, HELD_CLASS_ENV: "exclusive"})


if __name__ == "__main__":
    unittest.main()
