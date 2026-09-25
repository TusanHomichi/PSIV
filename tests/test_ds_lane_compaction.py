"""ds-lane compaction: dedupe a lane's evidence, compress the bulky text.

    PYTHONPATH=. python3 -m unittest discover -s tests -p 'test_ds_lane*.py' -v

Every run's receipt keeps its own copy of the worktree's `build/lane-evidence/`,
so a lane's receipts repeat each other and the raw captures in them are large
repetitive text - one sweep lane reached 8.7 GB (owner decision 2026-09-24:
deduplicate and compress). These cases cover the passes end to end: a file an
earlier run already keeps becomes a hardlink to it (same inode) while a changed
file stays its own, a file over the size threshold is stored as `.xz` and the
index's sha256 verifies its content, small files, binaries and already
compressed files are left alone, and a run another pass holds, or a lane with a
run in flight, is left untouched. `compact` over pre-change receipts saves
space, is idempotent, and a `--dry-run` changes nothing; an interrupted pass
leaves every file readable in one form or the other, and a failed pass is
recorded in `run.json` without disturbing the run's own record.
"""
import fcntl
import hashlib
import json
import lzma
import os
import unittest
from pathlib import Path
from unittest import mock

try:  # `unittest discover -s tests` imports this module top level
    from .ds_lane_support import DS, LaneFixture, RECEIPT_RESULT
except ImportError:  # `python -m unittest tests.test_ds_lane_compaction`
    from ds_lane_support import DS, LaneFixture, RECEIPT_RESULT

TINY = "a tiny capture\n"  # far under the threshold: never compressed


def big_text(rows=46000, tag="row"):
    """Over the 1 MiB default threshold, and repetitive like a real capture."""
    return "".join(f"{tag} {i:07d} payload {i % 977}\n" for i in range(rows))


def spec_file(root, name, spec):
    """A fake-worker spec on disk: the content is too big for the environment."""
    path = Path(root) / name
    path.write_text(json.dumps(spec))
    return path


def snapshot(root):
    """Every file under `root`: size, inode and mtime, keyed by relative path."""
    out = {}
    for path in sorted(Path(root).rglob("*")):
        if path.is_file():
            st = path.stat()
            out[str(path.relative_to(root))] = (st.st_size, st.st_ino, st.st_mtime_ns)
    return out


def storage(root):
    """Bytes `root` really occupies: storage shared by hardlinks counted once."""
    seen, total = set(), 0
    for path in sorted(Path(root).rglob("*")):
        if path.is_file():
            st = path.stat()
            if (st.st_dev, st.st_ino) not in seen:
                seen.add((st.st_dev, st.st_ino))
                total += st.st_size
    return total


def inode(path):
    st = Path(path).stat()
    return (st.st_dev, st.st_ino)


class CompactionCase(LaneFixture):
    """Dedupe, compression and the index: real CLI, detached supervisor, fake worker."""

    # -- scaffolding

    def evidence(self, lane_id, run):
        return self.lane_state(lane_id, f"run-{run}", "evidence")

    def index(self, lane_id, run):
        path = self.evidence(lane_id, run) / DS.INDEX_NAME
        return json.loads(path.read_text()) if path.exists() else None

    def entry(self, lane_id, run, rel):
        return next(e for e in self.index(lane_id, run)["files"] if e["path"] == rel)

    def compacted(self, lane_id, run, timeout=120):
        """The run's record once its compaction has been recorded in run.json."""
        path = self.lane_state(lane_id, f"run-{run}", "run.json")

        def attempt():
            try:
                run_json = json.loads(path.read_text())
            except (OSError, json.JSONDecodeError):
                return None
            return run_json if "compaction" in run_json else None
        return self.wait_for(attempt, f"{path} compaction", timeout=timeout)

    def compact(self, lane_id, *extra, check=0):
        return self.cli("compact", "--repo", self.repo, lane_id, *extra, check=check)

    def prechange_receipts(self, lane_id, contents):
        """A lane whose receipts predate compaction: evidence and no INDEX.json.

        `contents` is {run number: {evidence-relative path: text or bytes}}, so a
        case says exactly what each run captured and which of it is identical -
        the state `ds-lane compact ID` exists to clean up.
        """
        state = self.lane_state(lane_id)
        state.mkdir(parents=True, exist_ok=True)
        (state / "lane.json").write_text(json.dumps({
            "id": lane_id, "title": "pre-change receipts", "repo": str(self.repo),
            "base_ref": "HEAD", "base_sha": self.base_sha, "branch": f"ds/{lane_id}",
            "worktree": str(self.lane_wt(lane_id)), "state_dir": str(state), "links": [],
            "link_sources": {}, "add_dirs": [], "created": DS.now(), "model": DS.MODEL,
            "effort": DS.EFFORT, "read_only": False, "write_set": None, "runs": [],
        }, indent=2) + "\n")
        for run_number, files in sorted(contents.items()):
            run = state / f"run-{run_number}"
            (run / "evidence").mkdir(parents=True)
            (run / "run.json").write_text(json.dumps({"run": run_number, "exit_code": 0}) + "\n")
            for rel, text in files.items():
                path = run / "evidence" / rel
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(text if isinstance(text, bytes) else text.encode())
        return state

    # -- 1. dedupe: an identical file is linked, a changed one is not

    def test_identical_evidence_is_hardlinked_across_runs(self):
        text = big_text()
        spec = {"files": {"build/lane-evidence/shared.txt": TINY,
                          "build/lane-evidence/trace.csv": text,
                          "build/lane-evidence/report.txt": "capture 1\n"},
                "result": RECEIPT_RESULT}
        self.start("Capture two runs.\n", "t1", spec_file(self.work, "two-runs.json", spec))
        self.compacted("t1", 1)
        # The worktree keeps run 1's evidence, so run 2 re-copies it and changes one file.
        again = {"files": {"build/lane-evidence/trace.csv": text,
                           "build/lane-evidence/report.txt": "capture 2\n"},
                 "result": RECEIPT_RESULT}
        self.resume("t1", "Capture again.\n",
                    spec=spec_file(self.work, "two-runs-again.json", again))
        self.compacted("t1", 2)

        first, second = self.evidence("t1", 1), self.evidence("t1", 2)
        # The capture is compressed once, in run 1, and run 2 keeps run 1's
        # bytes: same inode, no second compression, no second copy of the file.
        self.assertFalse((first / "trace.csv").exists())
        self.assertTrue((second / "trace.csv.xz").exists())
        self.assertEqual(inode(second / "trace.csv.xz"), inode(first / "trace.csv.xz"))
        self.assertEqual(self.entry("t1", 1, "trace.csv.xz")["stored"], "xz")
        self.assertEqual(self.entry("t1", 2, "trace.csv.xz")["stored"], "hardlink")
        self.assertEqual(self.entry("t1", 2, "trace.csv.xz")["link"], "run-1/evidence/trace.csv.xz")
        self.assertEqual(lzma.open(second / "trace.csv.xz", "rb").read().decode(), text)
        # A small identical file is linked as it stands; a changed one is its own.
        self.assertEqual(inode(second / "shared.txt"), inode(first / "shared.txt"))
        self.assertEqual(self.entry("t1", 2, "shared.txt")["stored"], "hardlink")
        self.assertNotEqual(inode(second / "report.txt"), inode(first / "report.txt"))
        self.assertEqual(self.entry("t1", 2, "report.txt")["stored"], "plain")
        self.assertEqual((second / "shared.txt").read_text(), TINY)
        self.assertEqual((second / "report.txt").read_text(), "capture 2\n")
        # The run's own record notes the pass and its failures (there are none).
        run = self.compacted("t1", 2)
        self.assertIsNone(run["compaction_error"])
        self.assertEqual(run["compaction"]["files"], 3)
        self.assertEqual(run["compaction"]["linked"], 2)
        self.assertEqual(run["compaction"]["compressed"], 0)

    # -- 2. a large capture is compressed, and the index verifies it

    def test_a_large_csv_is_compressed_and_the_index_verifies_it(self):
        text = big_text()
        spec = {"files": {"build/lane-evidence/trace.csv": text}, "result": RECEIPT_RESULT}
        self.start("Capture a big trace.\n", "t1", spec_file(self.work, "big.json", spec))
        run = self.compacted("t1", 1)
        self.assertIsNone(run["compaction_error"])
        self.assertEqual(run["compaction"]["compressed"], 1)

        evidence = self.evidence("t1", 1)
        self.assertFalse((evidence / "trace.csv").exists())
        packed = evidence / "trace.csv.xz"
        self.assertEqual(lzma.open(packed, "rb").read().decode(), text)  # `xz -dc` equivalent
        entry = self.entry("t1", 1, "trace.csv.xz")
        self.assertEqual(entry["size"], len(text.encode()))
        self.assertEqual(entry["sha256"], hashlib.sha256(text.encode()).hexdigest())
        self.assertEqual(entry["stored"], "xz")
        self.assertEqual(entry["stored_size"], packed.stat().st_size)
        # The index's own accounting: what was captured, what it costs now.
        self.assertEqual(self.index("t1", 1)["original_bytes"], len(text.encode()))
        self.assertEqual(self.index("t1", 1)["stored_bytes"], packed.stat().st_size)
        self.assertLess(packed.stat().st_size, len(text.encode()) // 4)

    # -- 3. small, binary and already-compressed files are left alone

    def test_small_binary_and_compressed_files_are_left_alone(self):
        blob = bytes(range(256)) * 5000  # 1.25 MiB of nothing a compressor likes
        packed = lzma.compress(b"already compressed\n")
        files = {"small.txt": "small, and like nothing else here\n", "blob.log": blob,
                 "notes.csv.xz": packed}
        self.prechange_receipts("t9", {1: files})
        self.compact("t9")

        evidence = self.evidence("t9", 1)
        self.assertEqual(sorted(p.name for p in evidence.iterdir()),
                         sorted(["INDEX.json", *files]))
        self.assertEqual((evidence / "small.txt").read_text(), files["small.txt"])
        self.assertEqual((evidence / "blob.log").read_bytes(), blob)
        self.assertEqual((evidence / "notes.csv.xz").read_bytes(), packed)
        for rel in files:
            self.assertEqual(self.entry("t9", 1, rel)["stored"], "plain")
        self.assertEqual(self.entry("t9", 1, "blob.log")["size"], len(blob))

    # -- 4. compact over pre-change receipts: space, then nothing, then dry

    def test_compact_saves_space_is_idempotent_and_a_dry_run_changes_nothing(self):
        text = big_text()
        contents = {1: {"trace.csv": text, "shared.txt": TINY, "note.txt": "one\n"},
                    2: {"trace.csv": text, "shared.txt": TINY, "note.txt": "two\n"}}
        state = self.prechange_receipts("t9", contents)
        before = snapshot(state)
        self.assertEqual([p.name for p in DS.run_dirs(state)], ["run-1", "run-2"])
        self.assertFalse((self.evidence("t9", 1) / DS.INDEX_NAME).exists())

        # The dry run reports the work and the bytes, and touches nothing.
        dry = self.compact("t9", "--dry-run")
        self.assertIn("would save", dry.stdout)
        self.assertIn("run-2", dry.stdout)
        self.assertNotIn("would save 0 B", dry.stdout)
        self.assertEqual(snapshot(state), before)

        captured = storage(state)
        real = self.compact("t9")
        self.assertIn("saved", real.stdout)
        self.assertLess(storage(state), captured)
        self.assertEqual(inode(self.evidence("t9", 2) / "trace.csv.xz"),
                         inode(self.evidence("t9", 1) / "trace.csv.xz"))
        self.assertEqual(self.entry("t9", 1, "trace.csv.xz")["stored"], "xz")
        self.assertEqual(self.entry("t9", 2, "trace.csv.xz")["stored"], "hardlink")
        self.assertEqual(self.entry("t9", 2, "note.txt")["stored"], "plain")
        self.assertEqual((self.evidence("t9", 2) / "note.txt").read_text(), "two\n")
        self.assertTrue((self.evidence("t9", 1) / "trace.csv.xz").exists())

        # A second pass says so, and leaves every file - mtimes included - alone.
        settled = snapshot(state)
        again = self.compact("t9")
        self.assertIn("saved 0 B", again.stdout)
        self.assertIn("0 linked, 0 compressed", again.stdout)
        self.assertEqual(snapshot(state), settled)
        # Nor does a dry run over the settled receipts misread them: nothing is
        # compressed again, and every link is still a link.
        dry = self.compact("t9", "--dry-run")
        self.assertIn("would save 0 B", dry.stdout)
        self.assertIn("0 compressed", dry.stdout)
        self.assertEqual(snapshot(state), settled)

    # -- 5. a lane with a run in flight is refused

    def test_compact_refuses_a_lane_with_a_run_in_flight(self):
        self.prechange_receipts("t9", {1: {"trace.csv": TINY}})
        pending = self.lane_state("t9", "run-2")
        pending.mkdir()
        (pending / "supervisor.pid").write_text(str(os.getpid()))  # alive, no run.json
        before = snapshot(self.lane_state("t9"))
        refused = self.compact("t9", check=1)
        self.assertIn("run-2 is still in flight", refused.stdout)
        self.assertIn("ds-lane wait t9", refused.stdout)
        self.assertEqual(snapshot(self.lane_state("t9")), before)
        # A run whose supervisor is gone is not in flight: the pass proceeds.
        (pending / "supervisor.pid").unlink()
        self.compact("t9")
        self.assertTrue((self.evidence("t9", 1) / DS.INDEX_NAME).exists())

    # -- 6. a run another pass holds is skipped, not fought over

    def test_compact_skips_a_run_another_pass_holds(self):
        self.prechange_receipts("t9", {1: {"trace.csv": TINY}})
        lock = open(self.lane_state("t9", "run-1", DS.compaction.LOCK_NAME), "w")
        self.addCleanup(lock.close)
        fcntl.flock(lock, fcntl.LOCK_EX)
        held = self.compact("t9")
        self.assertIn("run-1: skipped (another compaction holds this run)", held.stdout)
        self.assertFalse((self.evidence("t9", 1) / DS.INDEX_NAME).exists())
        fcntl.flock(lock, fcntl.LOCK_UN)
        self.compact("t9")
        self.assertTrue((self.evidence("t9", 1) / DS.INDEX_NAME).exists())

    # -- 7. an interrupted compression leaves everything readable

    def test_an_interrupted_compression_leaves_every_file_readable(self):
        text = big_text()
        state = self.prechange_receipts("t9", {1: {"trace.csv": text, "keep.txt": TINY}})
        lane = json.loads((state / "lane.json").read_text())

        def interrupted(src, dst, dry_run=False):
            """What a host that dies mid-write leaves: a scratch file, no `.xz`."""
            Path(str(dst) + DS.PART_SUFFIX).write_bytes(b"\xfd7zXZ\x00truncated")
            raise OSError("simulated interruption")

        with mock.patch.object(DS.compaction, "compress_file", interrupted):
            report = DS.compact_runs(lane, DS.run_dirs(state))
        self.assertTrue(report["errors"], report)
        self.assertIn("simulated interruption", report["errors"][0])
        evidence = self.evidence("t9", 1)
        self.assertEqual((evidence / "trace.csv").read_text(), text)  # the original is intact
        self.assertFalse((evidence / "trace.csv.xz").exists())
        self.assertEqual((evidence / "keep.txt").read_text(), TINY)
        scratch = evidence / ("trace.csv.xz" + DS.PART_SUFFIX)
        self.assertTrue(scratch.exists())

        # The next pass clears the scratch and stores the capture properly.
        self.compact("t9")
        self.assertFalse(scratch.exists())
        self.assertFalse((evidence / "trace.csv").exists())
        self.assertEqual(DS.evidence.xz_content(evidence / "trace.csv.xz"),
                         (hashlib.sha256(text.encode()).hexdigest(), len(text.encode())))
        self.assertEqual(self.entry("t9", 1, "trace.csv.xz")["stored"], "xz")
        self.assertEqual(self.entry("t9", 1, "keep.txt")["stored"], "plain")

    # -- 8. a settled pair converges; a foreign `.xz` is left where it is

    def test_a_settled_pair_converges_and_a_foreign_xz_stays(self):
        text = big_text()
        settled = lzma.compress(text.encode(), preset=0)  # any stream of that content
        foreign = lzma.compress(b"something else entirely\n")
        state = self.prechange_receipts("t9", {1: {"a.csv": text, "b.csv": text,
                                                  "c.csv": "tiny text\n",
                                                  "d.txt": "left alone\n"}})
        evidence = self.evidence("t9", 1)
        (evidence / "b.csv.xz").write_bytes(settled)  # an interrupted pass's own `.xz`
        (evidence / "d.txt.xz").write_bytes(foreign)  # a worker's `.xz`, not ours
        self.compact("t9")

        self.assertTrue((evidence / "a.csv.xz").exists())
        self.assertFalse((evidence / "a.csv").exists())
        # `b.csv`'s pair settles on the `.xz` the interrupted pass wrote, byte for
        # byte; `d.txt` keeps both files, because that `.xz` is other content.
        self.assertEqual((evidence / "b.csv.xz").read_bytes(), settled)
        self.assertFalse((evidence / "b.csv").exists())
        self.assertEqual((evidence / "d.txt").read_text(), "left alone\n")
        self.assertEqual((evidence / "d.txt.xz").read_bytes(), foreign)
        self.assertEqual([e["path"] for e in self.index("t9", 1)["files"]],
                         ["a.csv.xz", "b.csv.xz", "c.csv", "d.txt", "d.txt.xz"])
        self.assertEqual(self.entry("t9", 1, "b.csv.xz")["sha256"],
                         hashlib.sha256(text.encode()).hexdigest())
        # Everything readable in one form, and a second pass changes nothing.
        once = snapshot(evidence)
        self.compact("t9")
        self.assertEqual(snapshot(evidence), once)

    # -- 9. a pass that fails is recorded, and the run's record stands

    def test_a_failed_compaction_is_recorded_in_run_json(self):
        text = big_text()
        state = self.prechange_receipts("t9", {1: {"trace.csv": text}})
        run_dir = state / "run-1"
        (run_dir / "run.json").write_text(json.dumps(
            {"run": 1, "exit_code": 0, "committed": True, "session_id": "sess-1"}) + "\n")
        (run_dir / "result.md").write_text(RECEIPT_RESULT)
        lane = json.loads((state / "lane.json").read_text())

        def broken(src, dst, dry_run=False):
            raise OSError("no space left on device")

        with mock.patch.object(DS.compaction, "compress_file", broken):
            DS.compact_finished_run(lane, run_dir, DS.run_dirs(state))
        recorded = json.loads((run_dir / "run.json").read_text())
        self.assertIn("no space left on device", recorded["compaction_error"])
        self.assertEqual(recorded["compaction"]["compressed"], 0)
        self.assertEqual(recorded["compaction"]["files"], 1)
        # The run's own record is untouched by it, and its evidence is readable.
        self.assertEqual((recorded["exit_code"], recorded["committed"]), (0, True))
        self.assertEqual(recorded["session_id"], "sess-1")
        self.assertIn("Wrote the owned file.", (run_dir / "result.md").read_text())
        self.assertEqual((run_dir / "evidence" / "trace.csv").read_text(), text)
        # A run with nothing to compact says so, and is not an error.
        empty = state / "run-2"
        empty.mkdir()
        (empty / "run.json").write_text(json.dumps({"run": 2}) + "\n")
        DS.compact_finished_run(lane, empty, DS.run_dirs(state))
        skipped = json.loads((empty / "run.json").read_text())
        self.assertIsNone(skipped["compaction_error"])
        self.assertEqual(skipped["compaction"]["skipped"], "no evidence")
        # A pass that cannot run at all is recorded too, and raises nothing: the
        # run it belongs to has succeeded and must not look like a crash.
        def unrunnable(*args, **kwargs):
            raise PermissionError("receipts are not readable")

        with mock.patch.object(DS.compaction, "compact_runs", unrunnable):
            DS.compact_finished_run(lane, run_dir, DS.run_dirs(state))
        crashed = json.loads((run_dir / "run.json").read_text())
        self.assertIn("PermissionError", crashed["compaction_error"])
        self.assertEqual(crashed["exit_code"], 0)
        self.assertFalse((run_dir / "failed").exists())
