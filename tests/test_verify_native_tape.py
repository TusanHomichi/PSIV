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
from tools.verify_native_tape import (file_identity, host_power, identities_stable, rss_verdict,
                                      source_identity, terminate_process_group)


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


def samples(*rows, every=1000, step_ms=1000):
    """One sample per row of (rss_mb, static_mb, objects, map) every `every`
    frames; the first row is frame `every`."""
    return [{"frame": every * (i + 1), "elapsed_ms": step_ms * (i + 1),
             "rss_kb": int(rss * 1024), "static_kb": int(static * 1024), "objects": objects,
             "map": map_id}
            for i, (rss, static, objects, map_id) in enumerate(rows)]


# Warm-up is frames < 2000: the first row of every sample list below.
WARMUP = 2000


class NativeTapeRssBound(unittest.TestCase):
    """#92: memory past warm-up stays bounded, with Godot's map-sized static
    memory compared one map at a time."""

    def test_a_map_swing_with_no_leak_passes(self):
        # Map 1's images take 110 MB of Godot static memory; map 2's, none.
        verdict = rss_verdict(samples((360, 210, 2800, 1), (250, 100, 2800, 2),
                                      (360, 210, 2900, 1), (252, 100, 2850, 2),
                                      (362, 212, 2900, 1)), WARMUP, 16)
        self.assertTrue(verdict["pass"], verdict)
        self.assertEqual(verdict["extension"]["growth_mb"], 2.0)
        self.assertEqual((verdict["godot_static"]["worst_map"],
                          verdict["godot_static"]["growth_mb"]), ("1", 2.0))
        self.assertEqual(verdict["maps_unjudged"], [])

    def test_a_leak_inside_godot_cannot_hide_under_a_bigger_maps_images(self):
        # The reviewer's probe: a 210 MB map in warm-up, then a 100 MB map
        # whose static memory climbs 3 MB a sample (105 MB leaked). Judged
        # against the warm-up peak, this passed.
        rows = [(360, 210, 2800, 1)] + [(250 + 3 * i, 100 + 3 * i, 2800, 2)
                                        for i in range(36)]
        verdict = rss_verdict(samples(*rows), WARMUP, 16)
        self.assertFalse(verdict["pass"])
        self.assertTrue(verdict["extension"]["pass"])
        self.assertFalse(verdict["godot_static"]["pass"])
        self.assertEqual((verdict["godot_static"]["worst_map"],
                          verdict["godot_static"]["growth_mb"]), ("2", 105.0))

    def test_a_map_sampled_once_is_listed_unjudged(self):
        verdict = rss_verdict(samples((300, 100, 10, 1), (300, 100, 10, 1),
                                      (500, 300, 900, 7), (300, 100, 10, 1)), WARMUP, 16)
        self.assertTrue(verdict["pass"], verdict)
        self.assertEqual(verdict["maps_unjudged"], [7])
        self.assertEqual(verdict["maps"]["7"]["samples"], 1)

    def test_a_window_with_no_map_sampled_twice_fails(self):
        verdict = rss_verdict(samples((300, 100, 10, 1), (300, 100, 10, 2),
                                      (300, 100, 10, 3)), WARMUP, 16)
        self.assertFalse(verdict["pass"])
        self.assertIn("no map was sampled twice", verdict["reason"])

    def test_warmup_growth_is_not_counted(self):
        self.assertTrue(rss_verdict(samples((100, 50, 10, 1), (900, 50, 10, 1),
                                            (905, 50, 10, 1)), WARMUP, 16)["pass"])

    def test_a_growing_extension_heap_fails_whatever_the_map(self):
        # The shipped #92 shape: the Rust heap grows while maps swing.
        verdict = rss_verdict(samples((250, 100, 2800, 2), (250, 100, 2800, 2),
                                      (380, 210, 2800, 1), (290, 100, 2800, 2),
                                      (420, 210, 2800, 1)), WARMUP, 16)
        self.assertFalse(verdict["pass"])
        self.assertFalse(verdict["extension"]["pass"])
        self.assertTrue(verdict["godot_static"]["pass"])
        self.assertEqual(verdict["extension"]["growth_mb"], 60.0)
        self.assertEqual(verdict["extension"]["slope_kb_per_kframe"], 20480.0)

    def test_a_transient_extension_peak_counts_even_if_it_falls_back(self):
        self.assertFalse(rss_verdict(samples((250, 100, 1, 1), (250, 100, 1, 1),
                                             (300, 100, 1, 1), (250, 100, 1, 1)),
                                     WARMUP, 16)["pass"])

    def test_objects_climbing_on_one_map_fail(self):
        verdict = rss_verdict(samples((300, 200, 10, 1), (300, 200, 10, 1),
                                      (300, 200, 300, 1)), WARMUP, 16)
        self.assertFalse(verdict["pass"])
        self.assertFalse(verdict["objects"]["pass"])
        self.assertEqual(verdict["objects"]["growth"], 290)

    def test_the_verdict_leaves_the_report_samples_untouched(self):
        rows = samples((300, 200, 10, 1), (300, 200, 10, 1), (300, 200, 10, 1))
        before = [dict(item) for item in rows]
        rss_verdict(rows, WARMUP, 16)
        self.assertEqual(rows, before)

    def test_missing_or_unreadable_samples_fail_rather_than_pass(self):
        self.assertFalse(rss_verdict(None, 0, 16)["pass"])
        # No warm-up sample, too few samples past warm-up, an unreadable RSS.
        self.assertFalse(rss_verdict(samples((470, 1, 1, 1), (480, 1, 1, 1)), 0, 16)["pass"])
        self.assertFalse(rss_verdict(samples((470, 1, 1, 1), (480, 1, 1, 1), (490, 1, 1, 1)),
                                     3000, 16)["pass"])
        unreadable = samples((470, 1, 1, 1), (480, 1, 1, 1), (490, 1, 1, 1))
        unreadable[2]["rss_kb"] = -1
        self.assertFalse(rss_verdict(unreadable, WARMUP, 16)["pass"])
        for key in ("static_kb", "map"):
            missing = samples((470, 1, 1, 1), (480, 1, 1, 1), (490, 1, 1, 1))
            del missing[1][key]
            verdict = rss_verdict(missing, WARMUP, 16)
            self.assertFalse(verdict["pass"], key)
            self.assertIn("unreadable", verdict["reason"])

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
        flat = [(300, 100, 10, 1)] * 4
        code, receipt = self.run_main(flat, bound)
        self.assertEqual(code, 0, receipt.get("rss"))
        self.assertIs(receipt["pass"], True)
        growing = [(300, 100, 10, 1), (300, 100, 10, 1), (400, 100, 10, 1), (500, 100, 10, 1)]
        code, receipt = self.run_main(growing, bound)
        self.assertEqual(code, 1)
        self.assertIs(receipt["rss"]["pass"], False)
        self.assertIs(receipt["pass"], False)

    def test_a_bound_without_a_warmup_of_one_sample_is_refused(self):
        for extra in (("--rss-bound-mb", "16"),
                      ("--rss-warmup-frames", "999", "--rss-bound-mb", "16")):
            with self.assertRaises(SystemExit), contextlib.redirect_stderr(io.StringIO()):
                self.run_main([(300, 100, 10, 1)] * 4, extra)

if __name__ == "__main__":
    unittest.main()
