"""Cases for tools/certify.py, the single entry point for the certified pairs.

    PYTHONPATH=. python3 -m unittest tests.test_certify -v

Hermetic: no Godot, no oracle host, no local game assets. They pin the shape of
the pair table (so a pair cannot silently lose its hash or its recipe), the
hash check that stops a regenerated or edited oracle frame from re-baselining a
certified pair, and the parallel run: N workers really overlap, never share a
display or project, take the longest pair first, and `main` takes the extension class for
the build and N godot slots for the captures. One case starts real Xvfb
servers when Xvfb is installed.
"""
import contextlib
import hashlib
import inspect
import io
import json
import os
import shutil
import signal
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from pathlib import Path
from unittest import mock

from tools import certify
from tools.heavy_lock import Hold

SIX = ["opening-p1", "opening-p2", "meeting-rika", "title", "battle-0x88", "camp-root"]
BATTLE = ["battle-status", "battle-status-2", "battle-fusion", "battle-strip", "battle-tech"]
NAMES = SIX + BATTLE + ["ship-menu"]
ROOT = Path(__file__).resolve().parent.parent


class PairTableCase(unittest.TestCase):
    def test_the_certified_pairs_are_present_once_in_order(self):
        self.assertEqual([pair[0] for pair in certify.PAIRS], NAMES)

    def test_the_six_original_pairs_keep_their_pins(self):
        # The battle pairs were added after these six; their ticks and frames
        # must not move with them.
        pins = {pair[0]: (pair[2], pair[3], pair[4]) for pair in certify.PAIRS}
        self.assertEqual(pins["battle-0x88"][:2], (200, "oracle/frames/frame_25000.png"))
        self.assertEqual(pins["camp-root"][:2], (60, "oracle/frames/frame_7675.png"))
        self.assertEqual(pins["title"][:2], (480, "oracle/frames/title/frame_450.png"))

    def test_battle_pairs_name_a_session_fixture_and_a_regenerable_frame(self):
        for name, env, _, frame, _, recipe in certify.PAIRS:
            if name in BATTLE:
                with self.subTest(pair=name):
                    self.assertIn("PSIV_DEBUG_BATTLE_WINDOW", env)
                    self.assertIsNotNone(recipe)
                    self.assertTrue((ROOT / recipe["tape"]).is_file())
                    # Oracle frames are local and ignored, never committed.
                    self.assertTrue(frame.startswith("build/certify/oracle/"))

    def test_the_ship_menu_pair_names_its_event_fixture_and_a_regenerable_frame(self):
        pair = next(p for p in certify.PAIRS if p[0] == "ship-menu")
        _, env, tick, frame, digest, recipe = pair
        # The clone starts the real scene (Cutscene_InsideSpaceship) through the
        # session's event fixture, not a hand-built menu.
        self.assertEqual(env, {"PSIV_DEBUG_EVENT": "0x800D"})
        self.assertIs(recipe, certify.TAPE_35)
        self.assertEqual(tick, 70)  # still settled with cursor and marker hidden
        self.assertEqual(frame, "build/certify/oracle/ship-menu/frame_7293.png")
        self.assertEqual(digest, "af4aaf169ea283c1784dad98822f02adcaa64684ccd3a5d037596df58f3300b8")
        self.assertTrue((ROOT / recipe["tape"]).is_file())
        self.assertTrue(frame.startswith("build/certify/oracle/"))
        # The fixture is tape 28's frame-7000 reload and a frame-7200 event
        # entry, with AlysFound (F101 bit 7), all spelled as patches.
        self.assertIn("7200:FFFFECA8:800D", recipe["patches"])
        self.assertIn("7000:FFFFEC28:00BF", recipe["patches"])
        self.assertIn("7000:FFFFF101:80", recipe["patches"])

    def test_battle_pairs_pin_the_enemy_clock_the_oracle_receipt_names(self):
        for name, env, _, _, _, _ in certify.PAIRS:
            if name in ("battle-strip", "battle-tech", "battle-status", "battle-status-2"):
                with self.subTest(pair=name):
                    self.assertIn("PSIV_DEBUG_BATTLE_PHASE", env)

    def test_every_pair_pins_a_sha256_and_a_positive_tick(self):
        for name, env, tick, frame, digest, _ in certify.PAIRS:
            with self.subTest(pair=name):
                self.assertRegex(digest, r"^[0-9a-f]{64}$")
                self.assertGreater(tick, 0)
                self.assertTrue(frame.endswith(".png"))
                self.assertIsInstance(env, dict)

    def test_opening_goes_through_the_real_title_start(self):
        # #44: a hand-built PSIV_DEBUG_EVENT=0x9f fixture rotted unnoticed.
        for name, env, _, _, _, recipe in certify.PAIRS:
            if name.startswith("opening"):
                with self.subTest(pair=name):
                    self.assertEqual(env.get("PSIV_DEBUG_TITLE_AUTOSTART"), "1")
                    self.assertEqual(env.get("PSIV_DEBUG_TITLE_SHOT"), "1")
                    self.assertNotIn("PSIV_DEBUG_EVENT", env)
                    self.assertIs(recipe, certify.TAPE_27)

    def test_opening_pairs_share_one_regeneration_run(self):
        frames = {Path(p[3]).name: p[5]["frames"] for p in certify.PAIRS
                  if p[0].startswith("opening")}
        self.assertEqual(sorted(frames), ["frame_4000.png", "frame_5200.png"])
        self.assertEqual(certify.TAPE_27["frames"], [4000, 5200])

    def test_recipes_name_their_own_frame(self):
        for name, _, _, frame, _, recipe in certify.PAIRS:
            if recipe is not None:
                with self.subTest(pair=name):
                    number = int(Path(frame).stem.split("_")[1])
                    self.assertIn(number, recipe["frames"])
                    self.assertTrue(recipe["tape"].startswith("oracle/tapes/"))


class OracleFrameHashCase(unittest.TestCase):
    def setUp(self):
        self._dir = tempfile.TemporaryDirectory()
        self.addCleanup(self._dir.cleanup)
        self.root = Path(self._dir.name)

    def write_frame(self, content):
        path = self.root / "frame.png"
        path.write_bytes(content)
        return path, hashlib.sha256(content).hexdigest()

    def test_a_frame_matching_its_pin_is_accepted(self):
        path, digest = self.write_frame(b"certified frame")
        found, error = certify.oracle_frame("p", str(path), digest, None, self.root)
        self.assertEqual((found, error), (path, None))

    def test_a_frame_differing_from_its_pin_is_refused(self):
        # Negative control: an edited or regenerated frame must not re-baseline.
        path, digest = self.write_frame(b"certified frame")
        path.write_bytes(b"edited frame")
        found, error = certify.oracle_frame("p", str(path), digest, None, self.root)
        self.assertIsNone(found)
        self.assertIn("!= certified", error)

    def test_a_missing_frame_without_a_recipe_is_refused(self):
        found, error = certify.oracle_frame("p", str(self.root / "gone.png"), "0" * 64,
                                            None, self.root)
        self.assertIsNone(found)
        self.assertIn("missing oracle frame", error)

    def test_a_missing_frame_is_regenerated_then_hash_checked(self):
        target = self.root / "out" / "frame_1.png"
        recipe = {"tape": "t.tape", "patches": [], "frames": [1]}

        def fake_regenerate(_recipe, frame_path, _log):
            frame_path.parent.mkdir(parents=True, exist_ok=True)
            frame_path.write_bytes(b"regenerated")
            return 0

        good = hashlib.sha256(b"regenerated").hexdigest()
        with mock.patch.object(certify, "regenerate", fake_regenerate):
            found, error = certify.oracle_frame("p", str(target), good, recipe, self.root)
            self.assertEqual((found, error), (target, None))
            target.unlink()
            found, error = certify.oracle_frame("p", str(target), "1" * 64, recipe, self.root)
            self.assertIsNone(found)
            self.assertIn("!= certified", error)

    def test_a_failing_regeneration_is_reported(self):
        recipe = {"tape": "t.tape", "patches": [], "frames": [1]}
        with mock.patch.object(certify, "regenerate", lambda *args: 7):
            found, error = certify.oracle_frame("p", str(self.root / "f.png"), "0" * 64,
                                                recipe, self.root)
        self.assertIsNone(found)
        self.assertIn("exited 7", error)


class BuildByDefaultTest(unittest.TestCase):
    """A capture must come from the checked-out sources (a stale extension once
    certified the wrong code), so building is the default."""

    def test_default_builds(self):
        self.assertFalse(certify.build_parser().parse_args([]).no_build)

    def test_no_build_is_explicit(self):
        self.assertTrue(certify.build_parser().parse_args(["--no-build"]).no_build)

    def test_legacy_build_flag_still_parses(self):
        self.assertFalse(certify.build_parser().parse_args(["--build"]).no_build)


class CaptureTimeoutTest(unittest.TestCase):
    """The capture budget scales with the tick it must reach."""

    def test_long_pairs_get_longer_budgets(self):
        self.assertGreater(certify.capture_timeout(4550), certify.capture_timeout(60))

    def test_the_longest_pair_outlasts_ten_frames_a_second(self):
        longest = max(tick for _, _, tick, *_ in certify.PAIRS)
        self.assertGreaterEqual(certify.capture_timeout(longest), (longest + certify.QUIT_MARGIN) / 10)


class JobCountTest(unittest.TestCase):
    def test_the_default_is_three_or_the_slot_cap_when_smaller(self):
        self.assertEqual(certify.job_count(None, 5), 3)
        self.assertEqual(certify.job_count(None, 2), 2)

    def test_more_workers_than_slots_is_refused(self):
        self.assertEqual(certify.job_count(3, 3), 3)
        for bad in (0, 4):
            with self.subTest(jobs=bad), self.assertRaises(ValueError):
                certify.job_count(bad, 3)


class RunCapturesTest(unittest.TestCase):
    """`run_captures` with stand-ins for Xvfb and Godot."""

    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.receipt = Path(tmp.name)
        self.opened = []
        self.in_use = set()
        self.started = []
        self.peak = 0
        self.projects_in_use = set()
        self.guard = threading.Lock()

    @contextlib.contextmanager
    def display(self, log):
        name = f":{len(self.opened)}"
        self.opened.append(log)
        yield name

    def take(self, barrier=None):
        def run(name, env, tick, receipt, display, project):
            with self.guard:
                self.assertNotIn(display, self.in_use, "two captures on one display")
                self.assertNotIn(project, self.projects_in_use, "two captures in one project")
                self.in_use.add(display)
                self.projects_in_use.add(project)
                self.started.append(name)
                self.peak = max(self.peak, len(self.in_use))
            if barrier is not None:
                barrier.wait()  # breaks unless `parties` captures overlap
            with self.guard:
                self.in_use.discard(display)
                self.projects_in_use.discard(project)
            return receipt / f"{name}.png", 0
        return run

    def captures(self, tasks, jobs, take, **kwargs):
        """`run_captures` with one stand-in project per possible worker."""
        projects = kwargs.pop("projects", tuple(Path(f"godot-{n}") for n in range(3)))
        return certify.run_captures(tasks, jobs, self.receipt, self.display, take,
                                    projects=projects, **kwargs)

    TASKS = [(f"pair-{tick}", {}, tick) for tick in (60, 4550, 200, 480, 3550, 160)]

    def test_n_workers_overlap_on_their_own_displays(self):
        shots = self.captures(self.TASKS, 3, self.take(threading.Barrier(3, timeout=10)))
        self.assertEqual(sorted(shots), sorted(name for name, _, _ in self.TASKS))
        self.assertEqual(len(self.opened), 3)
        self.assertEqual(self.peak, 3)

    def test_negative_control_one_worker_never_overlaps(self):
        self.captures(self.TASKS, 1, self.take())
        self.assertEqual(self.peak, 1)
        with self.assertRaises(threading.BrokenBarrierError):
            self.captures(self.TASKS[:2], 1, self.take(threading.Barrier(2, timeout=0.5)))

    def test_the_longest_pair_starts_first(self):
        self.captures(self.TASKS, 1, self.take())
        ticks = [int(name.split("-")[1]) for name in self.started]
        self.assertEqual(ticks, sorted(ticks, reverse=True))

    def test_a_failing_capture_cancels_the_queued_ones(self):
        def failing(name, env, tick, receipt, display, project):
            self.started.append(name)
            raise RuntimeError("godot launch failed")

        results = {}
        with self.assertRaises(RuntimeError):
            self.captures(self.TASKS, 1, failing, results=results)
        self.assertEqual(self.started, ["pair-4550"])  # the rest never started
        self.assertEqual(results, {})

    def test_the_capture_is_always_passed(self):
        """`capture` needs the resolved Godot, so no default `take` could run."""
        parameters = inspect.signature(certify.run_captures).parameters
        self.assertIs(parameters["take"].default, inspect.Parameter.empty)
        self.assertIs(parameters["display"].default, inspect.Parameter.empty)
        self.assertIs(parameters["projects"].default, inspect.Parameter.empty)
        self.assertIs(inspect.signature(certify.capture).parameters["godot"].default,
                      inspect.Parameter.empty)

    def test_no_more_workers_than_tasks(self):
        self.captures(self.TASKS[:2], 3, self.take())
        self.assertEqual(len(self.opened), 2)

    def test_each_worker_keeps_its_own_project(self):
        seen = {}

        def take(name, env, tick, receipt, display, project):
            with self.guard:
                seen.setdefault(display, set()).add(project)
            return receipt / f"{name}.png", 0

        self.captures(self.TASKS, 3, take)
        self.assertEqual(len(seen), 3)
        self.assertTrue(all(len(projects) == 1 for projects in seen.values()), seen)
        self.assertEqual(len(set().union(*seen.values())), 3)

    def test_negative_control_fewer_projects_than_workers_is_refused(self):
        with self.assertRaises(ValueError):
            self.captures(self.TASKS, 3, self.take(), projects=(Path("godot"),))


#: A stand-in X server: writes its pid to $FAKE_XVFB_PID, then reports display
#: 77 on its -displayfd, then runs until signalled.
FAKE_XVFB = """
import os, sys, time
open(os.environ["FAKE_XVFB_PID"] + ".tmp", "w").write(str(os.getpid()))
os.rename(os.environ["FAKE_XVFB_PID"] + ".tmp", os.environ["FAKE_XVFB_PID"])
os.write(int(sys.argv[sys.argv.index("-displayfd") + 1]), b"77\\n")  # after the pid
time.sleep(120)
"""

#: Opens one worker display with the stand-in server, says `ready`, waits to
#: be killed. With `argv[2] == "unarmed"`, the parent-death signal is left out.
DISPLAY_OWNER = f"""
import sys, time
sys.path.insert(0, {str(ROOT / "tools")!r})
import certify
certify.XVFB_COMMAND = [sys.executable, sys.argv[1]]
if sys.argv[2] == "unarmed":
    certify.die_with_parent = lambda: None
with certify.xvfb_display(sys.argv[3]) as display:
    print("ready", display, flush=True)
    time.sleep(120)
"""


class XvfbOrphanTest(unittest.TestCase):
    """A certify killed outright (SIGKILL: no handler, no finally) must not
    leave its X servers running."""

    def run_owner(self, mode):
        tmp = Path(self.enterContext(tempfile.TemporaryDirectory()))
        fake = tmp / "fake_xvfb.py"
        fake.write_text(FAKE_XVFB)
        pid_file = tmp / "xvfb.pid"
        owner = subprocess.Popen([sys.executable, "-c", DISPLAY_OWNER, str(fake), mode,
                                  str(tmp / "xvfb.log")], stdout=subprocess.PIPE, text=True,
                                 env={**os.environ, "FAKE_XVFB_PID": str(pid_file)})
        self.addCleanup(owner.wait, 30)
        self.addCleanup(owner.kill)
        self.assertEqual(owner.stdout.readline().split(), ["ready", ":77"])
        owner.stdout.close()
        server = int(pid_file.read_text())
        self.addCleanup(self.kill_quietly, server)
        owner.kill()
        owner.wait(timeout=30)
        return server

    @staticmethod
    def alive(pid):
        try:
            state = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()[0]
        except OSError:
            return False
        return state != "Z"

    @staticmethod
    def kill_quietly(pid):
        try:
            os.kill(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass

    def test_a_killed_certify_takes_its_x_server_with_it(self):
        server = self.run_owner("armed")
        for _ in range(100):  # bounded: 10 s for the kernel's signal to land
            if not self.alive(server):
                break
            time.sleep(0.1)
        self.assertFalse(self.alive(server), "the X server outlived certify")

    def test_negative_control_without_the_death_signal_the_server_is_orphaned(self):
        server = self.run_owner("unarmed")
        time.sleep(1.0)
        self.assertTrue(self.alive(server))


@unittest.skipUnless(shutil.which("Xvfb"), "Xvfb is not installed")
class XvfbDisplayTest(unittest.TestCase):
    def test_concurrent_servers_get_distinct_displays(self):
        with tempfile.TemporaryDirectory() as tmp, contextlib.ExitStack() as stack:
            names = [stack.enter_context(certify.xvfb_display(Path(tmp) / f"x{i}.log"))
                     for i in range(2)]
            self.assertRegex(names[0], r"^:\d+$")
            self.assertNotEqual(names[0], names[1])


class MainLocksTest(unittest.TestCase):
    """`main` takes the extension class for the build, then one godot slot per
    worker for the captures, and refuses more workers than slots up front."""

    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.tmp = Path(tmp.name)
        self.addCleanup(os.chdir, os.getcwd())
        self.holds = []

    @contextlib.contextmanager
    def hold(self, lock_class="exclusive", slots=1, **_):
        self.holds.append((lock_class, slots))
        yield Hold(self.tmp / "heavy.lock", lock_class, slots, nested=False)

    def run_main(self, argv, slots="3", interrupt_after=None, godot=Path("godot"),
                 prepare_error=None):
        frame = self.tmp / "frame.png"
        frame.write_bytes(b"f")

        def captures(tasks, jobs, receipt, display, take, results, projects):
            self.jobs = jobs
            self.take = take
            self.projects = projects
            for count, (name, _, _) in enumerate(tasks):
                if count == interrupt_after:
                    raise KeyboardInterrupt
                results[name] = (frame, 0, 0.0)
            return results

        def resolve():
            if isinstance(godot, Exception):
                raise godot
            return godot

        def run_root(root, projects, source):
            self.prepared.append((root, projects))
            if isinstance(prepare_error, Exception):
                raise prepare_error
            made = [root / ("godot" if n == 0 else f"godot-{n}") for n in range(projects)]
            return mock.Mock(root=root, projects=tuple(made), library_sha256="0" * 64,
                             record={"root": str(root), "created_s": 0.1})

        self.prepared = []
        with mock.patch.object(certify, "hold_heavy_lock", self.hold), \
                mock.patch.object(certify, "resolve_godot", resolve), \
                mock.patch.object(certify, "create_run_root", run_root), \
                mock.patch.dict(os.environ, {"PSIV_GODOT_SLOTS": slots}), \
                mock.patch.object(certify.subprocess, "run",
                                  return_value=mock.Mock(returncode=0)), \
                mock.patch.object(certify, "git_info", return_value={"short_sha": "abc"}), \
                mock.patch.object(certify, "receipt_dir", return_value=self.tmp / "receipt"), \
                mock.patch.object(certify, "oracle_frame", return_value=(frame, None)), \
                mock.patch.object(certify, "run_captures", captures), \
                mock.patch.object(certify, "compare", return_value=("0.000000", "")), \
                contextlib.redirect_stdout(io.StringIO()) as out:
            code = certify.main(argv)
        return code, out.getvalue()

    def test_the_build_is_extension_class_and_the_captures_hold_one_slot_per_worker(self):
        """The extension class, not exclusive: the captures load the run
        root's snapshot, so the build may share the machine (tools/godot.py)."""
        code, out = self.run_main(["--only", "title", "camp-root", "battle-0x88", "--jobs", "2"])
        self.assertEqual(code, 0, out)
        self.assertEqual(self.holds, [("extension", 1), ("godot", 2)])
        self.assertEqual(self.jobs, 2)

    def test_no_build_takes_only_the_godot_slots(self):
        code, out = self.run_main(["--no-build", "--only", "title", "camp-root"])
        self.assertEqual(code, 0, out)
        self.assertEqual(self.holds, [("godot", 2)])  # the default 3, capped by two pairs

    def test_an_interrupted_run_still_writes_its_receipt_then_reraises(self):
        with self.assertRaises(KeyboardInterrupt):
            self.run_main(["--no-build", "--only", "title", "camp-root"], interrupt_after=1)
        receipt = json.loads((self.tmp / "receipt" / "certify.json").read_text())
        self.assertEqual(receipt["interrupted"], "KeyboardInterrupt: ")
        pairs = {pair["pair"]: pair for pair in receipt["pairs"]}
        self.assertTrue(pairs["title"]["pass"])
        self.assertFalse(pairs["camp-root"]["pass"])
        self.assertIn("interrupted", pairs["camp-root"]["error"])

    def test_negative_control_a_finished_run_is_not_marked_interrupted(self):
        code, out = self.run_main(["--no-build", "--only", "title", "camp-root"])
        self.assertEqual(code, 0, out)
        receipt = json.loads((self.tmp / "receipt" / "certify.json").read_text())
        self.assertIsNone(receipt["interrupted"])

    def test_godot_is_resolved_once_and_the_project_prepared_before_captures(self):
        code, out = self.run_main(["--no-build", "--only", "title"], godot=Path("/x/godot"))
        self.assertEqual(code, 0, out)
        self.assertEqual(self.prepared, [(self.tmp / "receipt" / "run-root", 1)])
        self.assertEqual(self.take.keywords, {"godot": Path("/x/godot")})

    def test_the_captures_run_from_one_run_root_with_a_project_per_worker(self):
        code, out = self.run_main(["--no-build", "--only", "title", "camp-root", "battle-0x88",
                                   "--jobs", "3"])
        self.assertEqual(code, 0, out)
        self.assertEqual(self.prepared, [(self.tmp / "receipt" / "run-root", 3)])
        self.assertEqual(len(set(self.projects)), 3)
        receipt = json.loads((self.tmp / "receipt" / "certify.json").read_text())
        self.assertEqual(receipt["run_root"]["root"], str(self.tmp / "receipt" / "run-root"))

    def test_a_project_that_cannot_be_prepared_is_refused(self):
        error = certify.GodotError("cannot write godot/.godot/extension_list.cfg: denied")
        code, out = self.run_main(["--no-build", "--only", "title"], prepare_error=error)
        self.assertEqual(code, 2)
        self.assertIn("certify: refused, cannot write godot/.godot/extension_list.cfg", out)

    def test_more_jobs_than_slots_is_refused_before_any_lock(self):
        code, out = self.run_main(["--jobs", "4"], slots="3")
        self.assertEqual(code, 2)
        self.assertIn("slot cap is 3", out)
        self.assertEqual(self.holds, [])

    def test_a_missing_or_wrong_godot_is_refused_before_any_lock(self):
        error = certify.GodotError("/x/godot is Godot 4.6.1; PSIV needs 4.7.1 (.godot-version)")
        code, out = self.run_main(["--only", "title"], godot=error)
        self.assertEqual(code, 2)
        self.assertIn("certify: refused, /x/godot is Godot 4.6.1", out)
        self.assertEqual(self.holds, [])


if __name__ == "__main__":
    unittest.main()
