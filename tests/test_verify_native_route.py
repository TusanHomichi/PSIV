"""The route verifier's own rules: what it refuses, what it asks the tape
driver to compare, and what it reports for a chapter that did not match."""

import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

from tools import verify_native_route as route
from tools.verify_native_tape import checkpoint_save_hashes, checkpoints_passed


def write_run(root: Path, chapters=(("academy", b"save-0", 10), ("holt", b"save-1", 5),
                                    ("rune-dorin", b"save-2", 7))) -> Path:
    """A completed run directory as `psiv-campaign run --report` leaves it."""
    run = root / "run"
    records = []
    for index, (name, data, frames) in enumerate(chapters):
        directory = run / f"{index:02d}-{name}"
        directory.mkdir(parents=True)
        (directory / "slot_1.sram").write_bytes(data)
        records.append({"index": index, "id": name, "frames": frames, "battles": index,
                        "save_fnv": f"{route.fnv1a64(data):016x}"})
    (run / "run.tape").write_text("psiv-tape 1\nstart new-game\nframes 22\npads\n00 22\n")
    (run / "report.json").write_text(json.dumps({
        "result": "completed", "digest": "d", "chapters": records,
        "frames": sum(frames for _, _, frames in chapters)}))
    return run


class SaveHash(unittest.TestCase):
    def test_fnv_matches_the_published_vectors_the_tape_codec_uses(self):
        self.assertEqual(route.fnv1a64(b""), 0xCBF29CE484222325)
        self.assertEqual(route.fnv1a64(b"a"), 0xAF63DC4C8601EC8C)
        self.assertEqual(route.fnv1a64(b"foobar"), 0x85944171F73967E8)


class LoadChapters(unittest.TestCase):
    def test_chapters_carry_their_cumulative_end_frames(self):
        with tempfile.TemporaryDirectory() as directory:
            _, chapters, total = route.load_chapters(write_run(Path(directory)))
        self.assertEqual(total, 3)
        self.assertEqual([c["end_frame"] for c in chapters], [10, 15, 22])
        self.assertEqual([c["id"] for c in chapters], ["academy", "holt", "rune-dorin"])

    def test_a_prefix_reads_only_the_saves_it_replays(self):
        with tempfile.TemporaryDirectory() as directory:
            run = write_run(Path(directory))
            (run / "02-rune-dorin" / "slot_1.sram").unlink()
            _, chapters, total = route.load_chapters(run, "holt")
        self.assertEqual([c["id"] for c in chapters], ["academy", "holt"])
        self.assertEqual(total, 3, "the run's own chapter count still decides a full run")

    def test_a_save_that_changed_since_the_run_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            run = write_run(Path(directory))
            (run / "01-holt" / "slot_1.sram").write_bytes(b"edited")
            with self.assertRaisesRegex(route.RouteError, "stale or edited"):
                route.load_chapters(run)

    def test_a_missing_save_an_unknown_chapter_and_a_foreign_report_are_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            run = write_run(Path(directory))
            with self.assertRaisesRegex(route.RouteError, "no chapter 'nowhere'"):
                route.load_chapters(run, "nowhere")
            (run / "00-academy" / "slot_1.sram").unlink()
            with self.assertRaisesRegex(route.RouteError, "cannot read chapter save"):
                route.load_chapters(run)
            report = json.loads((run / "report.json").read_text())
            report["frames"] += 1
            (run / "report.json").write_text(json.dumps(report))
            with self.assertRaisesRegex(route.RouteError, "do not add up"):
                route.load_chapters(run)
            (run / "report.json").write_text(json.dumps({"result": "halted"}))
            with self.assertRaisesRegex(route.RouteError, "not a completed run"):
                route.load_chapters(run)
            (run / "report.json").write_text("{")
            with self.assertRaisesRegex(route.RouteError, "cannot read"):
                route.load_chapters(run)


class Receipts(unittest.TestCase):
    def chapters(self):
        with tempfile.TemporaryDirectory() as directory:
            return route.load_chapters(write_run(Path(directory)))[1]

    def test_checkpoints_name_each_chapter_at_the_frame_it_ended_on(self):
        listed = route.checkpoint_list(self.chapters())
        self.assertEqual([(c["label"], c["frame"]) for c in listed],
                         [("00-academy", 10), ("01-holt", 15), ("02-rune-dorin", 22)])
        self.assertTrue(all(c["expect_save"].endswith("slot_1.sram") for c in listed))

    def test_each_chapter_gets_its_own_wall_time_from_the_running_clock(self):
        native = {"checkpoints": [
            {"label": "00-academy", "frame": 10, "elapsed_ms": 2000, "match": True,
             "snapshot_sha256": "aa"},
            {"label": "01-holt", "frame": 15, "elapsed_ms": 2500, "match": True,
             "snapshot_sha256": "bb"},
            {"label": "02-rune-dorin", "frame": 22, "elapsed_ms": 3900, "match": True,
             "snapshot_sha256": "cc"}]}
        receipts = route.chapter_receipts(self.chapters(), native)
        self.assertEqual([r["wall_s"] for r in receipts], [2.0, 0.5, 1.4])
        self.assertEqual([r["frames_per_s"] for r in receipts], [5.0, 10.0, 5.0])
        self.assertEqual([(r["start_frame"], r["end_frame"]) for r in receipts],
                         [(0, 10), (10, 15), (15, 22)])
        self.assertTrue(all(r["match"] and r["reached"] for r in receipts))

    def test_a_divergence_keeps_its_first_byte_and_later_chapters_are_not_reached(self):
        native = {"checkpoints": [
            {"label": "00-academy", "frame": 10, "elapsed_ms": 1000, "match": True,
             "snapshot_sha256": "aa"},
            {"label": "01-holt", "frame": 15, "elapsed_ms": 1500, "match": False,
             "snapshot_sha256": "bb", "first_differing_byte": 44,
             "divergent_snapshot": "x.divergence-01-holt"}]}
        receipts = route.chapter_receipts(self.chapters(), native)
        self.assertEqual([(r["reached"], r["match"]) for r in receipts],
                         [(True, True), (True, False), (False, False)])
        self.assertEqual(receipts[1]["first_differing_byte"], 44)
        table = route.summary_table(receipts)
        self.assertIn("DIFFERS at frame 15 (first byte 44)", table)
        self.assertIn("not reached", table)

    def test_no_native_report_means_nothing_matched(self):
        receipts = route.chapter_receipts(self.chapters(), None)
        self.assertFalse(any(r["reached"] or r["match"] for r in receipts))


class Main(unittest.TestCase):
    """`main` against a stand-in for the tape driver, which needs Godot."""

    @staticmethod
    def fake_driver(checkpoint_matches, code=0, record=None):
        def run(argv):
            options = dict(zip(argv[::2], argv[1::2]))
            if record is not None:
                record.append(argv)
            out = Path(options["--out"])
            out.mkdir(parents=True)
            asked = json.loads(Path(options["--checkpoints"]).read_text())
            reached = [{"label": want["label"], "frame": want["frame"],
                        "elapsed_ms": 100 * (i + 1), "match": ok, "snapshot_sha256": "00"}
                       for i, (want, ok) in enumerate(zip(asked, checkpoint_matches))]
            (out / "receipt.json").write_text(json.dumps({
                "pass": code == 0 and all(checkpoint_matches), "native_report": {
                    "checkpoints": reached, "camp_save_acks": [3, 0, 0]},
                "identity_stable": True, "mapped_library_matches_preflight": True,
                "code_identity_before": {"extension": {"sha256": "ext"}},
                "pack_manifest_sha256": "pack", "ordinary_save_acknowledged": True,
                "ordinary_save_matches": True}))
            return code
        return run

    def run_main(self, root: Path, extra=(), driver=None):
        run = write_run(root)
        out = root / "out"
        argv = ["--run-dir", str(run), "--out", str(out), *extra]
        with mock.patch.object(route.verify_native_tape, "main", driver), \
                contextlib.redirect_stdout(io.StringIO()):
            code = route.main(argv)
        return code, out

    def test_every_chapter_matching_passes_and_writes_a_receipt_per_chapter(self):
        asked = []
        with tempfile.TemporaryDirectory() as directory:
            code, out = self.run_main(Path(directory), driver=self.fake_driver(
                [True, True, True], record=asked))
            summary = json.loads((out / "summary.json").read_text())
            names = sorted(path.name for path in (out / "chapters").iterdir())
        self.assertEqual(code, 0)
        self.assertTrue(summary["pass"])
        self.assertEqual((summary["chapters_matched"], summary["frames"]), (3, 22))
        self.assertEqual(summary["scope"], "full route")
        self.assertEqual(names, ["00-academy.json", "01-holt.json", "02-rune-dorin.json"])
        self.assertNotIn("--stop-at", asked[0])
        self.assertIn("--checkpoints", asked[0])

    def test_one_chapter_differing_fails_the_whole_route(self):
        with tempfile.TemporaryDirectory() as directory:
            code, out = self.run_main(Path(directory), driver=self.fake_driver(
                [True, False], code=1))
            summary = json.loads((out / "summary.json").read_text())
        self.assertEqual(code, 1)
        self.assertFalse(summary["pass"])
        self.assertEqual(summary["chapters_matched"], 1)

    def test_a_driver_pass_cannot_hide_a_chapter_that_did_not_match(self):
        with tempfile.TemporaryDirectory() as directory:
            code, _ = self.run_main(Path(directory), driver=self.fake_driver(
                [True, True, False], code=0))
        self.assertEqual(code, 1)

    def test_a_prefix_replays_only_its_frames_and_says_so(self):
        asked = []
        with tempfile.TemporaryDirectory() as directory:
            code, out = self.run_main(
                Path(directory), ("--until-chapter", "holt"),
                self.fake_driver([True, True], record=asked))
            summary = json.loads((out / "summary.json").read_text())
        self.assertEqual(code, 0)
        self.assertEqual(summary["scope"], "prefix through holt")
        self.assertEqual(asked[0][asked[0].index("--stop-at") + 1], "15")

    def test_a_stale_run_directory_is_refused_before_anything_is_replayed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            run = write_run(root)
            (run / "00-academy" / "slot_1.sram").write_bytes(b"edited")
            driver = mock.Mock()
            with mock.patch.object(route.verify_native_tape, "main", driver), \
                    contextlib.redirect_stdout(io.StringIO()) as said:
                code = route.main(["--run-dir", str(run), "--out", str(root / "out")])
            self.assertEqual(code, 1)
            driver.assert_not_called()
            self.assertFalse((root / "out").exists())
            self.assertIn("REFUSED", said.getvalue())


class ContinueProbes(unittest.TestCase):
    """`--continue-probes` against stand-ins for the probe-tape cutter and the
    tape driver, which need a built binary and Godot."""

    @staticmethod
    def fake_driver(results):
        outcomes = iter(results)

        def run(argv):
            options = dict(zip(argv[::2], argv[1::2]))
            out = Path(options["--out"])
            out.mkdir(parents=True)
            ok, unchanged = next(outcomes)
            (out / "receipt.json").write_text(json.dumps({
                "pass": ok, "source_sha256_before": "aa",
                "source_sha256_after": "aa" if unchanged else "bb",
                "snapshot_sha256": "aa" if ok else "cc"}))
            return 0 if ok else 1
        return run

    def run_probes(self, root: Path, results, cutter_code=0):
        run = write_run(root)
        binary = root / "psiv-campaign"
        binary.write_text("stub")
        out = root / "out"
        cut = mock.Mock(return_value=mock.Mock(returncode=cutter_code, stderr="no save",
                                               stdout=""))
        with mock.patch.object(route.verify_native_tape, "main", self.fake_driver(results)), \
                mock.patch.object(route.subprocess, "run", cut), \
                contextlib.redirect_stdout(io.StringIO()):
            code = route.main(["--run-dir", str(run), "--out", str(out), "--continue-probes",
                               "--campaign-bin", str(binary)])
        return code, out, cut

    def test_every_chapter_save_must_load_and_reencode_identically(self):
        with tempfile.TemporaryDirectory() as directory:
            code, out, cut = self.run_probes(Path(directory), [(True, True)] * 3)
            summary = json.loads((out / "continue-summary.json").read_text())
        self.assertEqual(code, 0)
        self.assertTrue(summary["pass"])
        self.assertEqual([item["chapter"] for item in summary["chapters"]],
                         ["00-academy", "01-holt", "02-rune-dorin"])
        self.assertEqual(cut.call_count, 3)
        self.assertEqual(cut.call_args.args[0][1], "save-probe-tape")
        self.assertEqual(cut.call_args.args[0][-1], "0", "a zero-pad probe: load only")

    def test_one_save_that_does_not_reencode_fails_the_run(self):
        with tempfile.TemporaryDirectory() as directory:
            code, out, _ = self.run_probes(Path(directory),
                                           [(True, True), (False, True), (True, True)])
            summary = json.loads((out / "continue-summary.json").read_text())
        self.assertEqual(code, 1)
        self.assertEqual([item["pass"] for item in summary["chapters"]], [True, False, True])

    def test_a_probe_that_touched_its_source_save_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            code, _, _ = self.run_probes(Path(directory), [(True, False)] * 3)
        self.assertEqual(code, 1)

    def test_a_missing_binary_is_refused_and_a_failed_cutter_is_reported(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            run = write_run(root)
            with contextlib.redirect_stdout(io.StringIO()) as said:
                code = route.main(["--run-dir", str(run), "--out", str(root / "out"),
                                   "--continue-probes",
                                   "--campaign-bin", str(root / "absent")])
            self.assertEqual(code, 1)
            self.assertIn("REFUSED", said.getvalue())
        with tempfile.TemporaryDirectory() as directory:
            code, out, _ = self.run_probes(Path(directory), [], cutter_code=1)
            summary = json.loads((out / "continue-summary.json").read_text())
        self.assertEqual(code, 1)
        self.assertEqual(summary["chapters"][0]["error"], "no save")


class DriverCheckpoints(unittest.TestCase):
    """The tape verifier's side of the contract."""

    def listed(self, root: Path, frames=(10, 15)):
        saves = []
        for index, frame in enumerate(frames):
            path = root / f"s{index}.sram"
            path.write_bytes(bytes([index]))
            saves.append({"frame": frame, "label": f"c{index}", "expect_save": str(path)})
        path = root / "checkpoints.json"
        path.write_text(json.dumps(saves))
        return path

    def test_every_checkpoint_must_be_reached_and_match(self):
        with tempfile.TemporaryDirectory() as directory:
            listed = self.listed(Path(directory))
            good = {"checkpoints": [{"label": "c0", "frame": 10, "match": True},
                                    {"label": "c1", "frame": 15, "match": True}]}
            self.assertTrue(checkpoints_passed(good, listed))
            self.assertTrue(checkpoints_passed(None, None))
            short = {"checkpoints": good["checkpoints"][:1]}
            wrong_frame = {"checkpoints": [dict(good["checkpoints"][0], frame=11),
                                           good["checkpoints"][1]]}
            differs = {"checkpoints": [good["checkpoints"][0],
                                       dict(good["checkpoints"][1], match=False)]}
            for bad in (None, {}, short, wrong_frame, differs):
                with self.subTest(bad=bad):
                    self.assertFalse(checkpoints_passed(bad, listed))

    def test_a_chapter_save_that_changes_during_the_replay_is_seen(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            listed = self.listed(root)
            before = checkpoint_save_hashes(listed)
            self.assertEqual(sorted(before), ["c0", "c1"])
            self.assertEqual(before, checkpoint_save_hashes(listed))
            (root / "s1.sram").write_bytes(b"changed")
            self.assertNotEqual(before, checkpoint_save_hashes(listed))
            self.assertEqual(checkpoint_save_hashes(None), {})


if __name__ == "__main__":
    unittest.main()
