"""Tests for the Motavia sweep runner, `oracle/sweep/`.

The runner is two subprocesses per formation and a record that is rewritten
after each one, so what these tests pin is that shape: which command each stage
gets, what the record says about a formation that worked and about one that did
not (either stage), and that a second run of the same sweep skips what is
already on disk. `oracle.sweep.jobs.run` - the one seam every stage goes
through - is replaced, so no emulator is started.

The list is checked against a hand-built pack here; the real one's 83
formations are computed from `generated/formation_indexes.json` and recorded in
`sweep_motavia.json` (`oracle/sweep/motavia_formations.json` is the committed
list).
"""
import json
import os
import pathlib
import subprocess
import sys
import unittest
from unittest import mock

from oracle.force.pack import Pack
from oracle.sweep import Options, Sweep, list_formations, main, record_path
from oracle.sweep import jobs as sweep_jobs
from oracle.sweep.plan import write_list
from tests.test_oracle_force_battle import ENCOUNTERS, PackFixture

#: The pack the sweep tests run against: every Motavia group is present, so the
#: list is what `list_formations` makes of them - formation 5 in the on-foot
#: groups, and 6 only in the vehicle table 8 (`kind: vehicle`).
GROUPS = {group: [5] * 32 for group in range(11)}
GROUPS[8] = [6] * 32
#: The two formations those groups hold, ascending.
MINI_LIST = [5, 6]


class SweepFixture(PackFixture):
    """A sweep over the mini pack, with the emulator replaced."""

    def setUp(self):
        super().setUp()
        self.pack_from(GROUPS, ENCOUNTERS)
        self.out = pathlib.Path(self.dir.name) / "sweep"
        self.fixtures = pathlib.Path(self.dir.name) / "fixtures"
        self.options = Options(work=self.out, fixtures=self.fixtures)
        self.calls = []

    def pack_of(self):
        return Pack.load(pathlib.Path(self.pack_dir))

    def report(self, formation: int) -> dict:
        """The report the force tool would have written for this formation."""
        return {
            "formation": formation, "outcome": "victory", "cut_frame": None,
            "max_rounds": 5, "abilities": {"e1=0x02": 25000},
            "enemies": [{"slot": 1, "id": 10, "maxhp": 25}],
            "selector": {"group": 0, "kind": "map", "entry": 0,
                         "label": "map 7 (TestCave)"},
            "durable": {"hp": 999, "frame": 100, "cells": ["chaz_hp"],
                        "skipped": []},
            "battle_first": 40, "battle_last": 200, "start_frame": 41,
            "log_sha256": "a" * 64, "trace_sha256": "b" * 64,
            "trace": "trace.csv", "log": "capture.csv", "tape": "tape.txt",
            "patches": [],
        }

    def fixture_document(self, formation: int) -> dict:
        return {"format_version": 1, "rounds": [{"round": 1}, {"round": 2}],
                "outcome": {"victory": True, "truncated": False,
                            "rounds_captured": 2}}

    def fake_run(self, force_fail=(), extract_fail=()):
        """A `jobs.run` that writes each stage's own output.

        `force_fail` and `extract_fail` are formation ids that stage fails for,
        with a message in the stage's log so the record's error can be checked.
        """
        def run(argv, log):
            self.calls.append([str(part) for part in argv])
            stem = pathlib.Path(argv[argv.index("--out") + 1])
            formation = int(stem.name.split("_")[1].split(".")[0], 16)
            log.parent.mkdir(parents=True, exist_ok=True)
            if "oracle.force" in argv:
                if formation in force_fail:
                    log.write_text("force_battle: the probe built something "
                                   "else\n")
                    return subprocess.CompletedProcess(argv, 2)
                log.write_text("capture: two runs byte-identical\n")
                (log.parent / "report.json").write_text(
                    json.dumps(self.report(formation)))
                return subprocess.CompletedProcess(argv, 0)
            if formation in extract_fail:
                log.write_text("fixture: the trace has no rolls in 40..200\n")
                return subprocess.CompletedProcess(argv, 1)
            log.write_text("wrote the fixture\n")
            out = pathlib.Path(argv[argv.index("--out") + 1])
            out.parent.mkdir(parents=True, exist_ok=True)
            out.write_text(json.dumps(self.fixture_document(formation)))
            return subprocess.CompletedProcess(argv, 0)
        return run

    def forced(self) -> set[int]:
        """The formations whose *capture* was run in this test."""
        return {int(call[call.index("--formation") + 1], 16)
                for call in self.calls if "--formation" in call}

    def sweep(self) -> Sweep:
        return Sweep(self.out, self.options, list_formations(self.pack_of()),
                     {"formations": "the mini pack"})

    def record(self) -> dict:
        return json.loads(record_path(self.out).read_text())

    def entries(self) -> dict:
        return {entry["formation"]: entry for entry in self.record()["formations"]}


class FormationList(SweepFixture):
    def test_the_list_is_the_union_of_the_motavia_groups(self):
        formations = list_formations(self.pack_of())
        self.assertEqual([entry.formation for entry in formations], MINI_LIST)
        self.assertEqual({entry.kind for entry in formations},
                         {"foot", "vehicle"})

    def test_a_formation_only_a_vehicle_table_holds_is_captured_mounted(self):
        formations = {entry.formation: entry
                      for entry in list_formations(self.pack_of())}
        self.assertEqual(formations[5].kind, "foot")
        self.assertIsNone(formations[5].vehicle)
        self.assertEqual(formations[6].kind, "vehicle")
        # The Motavia tables seat the Land Rover by default (`selectors.REGION
        # _DEFAULT_VEHICLE`), and the sweep says so in the command it records.
        self.assertEqual(formations[6].vehicle, 1)

    def test_the_committed_list_is_this_lists_own_shape(self):
        pack = self.pack_of()
        path = pathlib.Path(self.dir.name) / "list.json"
        write_list(path, list_formations(pack), {"formations": "the mini pack"})
        document = json.loads(path.read_text())
        self.assertEqual(document["count"], len(MINI_LIST))
        self.assertEqual([entry["formation"] for entry in document["formations"]],
                         MINI_LIST)
        self.assertEqual(document["formations"][0]["hex"], "0x05")


class Commands(SweepFixture):
    """What each stage is told to run: the two tools, and their options."""

    def test_the_capture_runs_the_force_tool_with_the_sweeps_own_options(self):
        entry = list_formations(self.pack_of())[0]
        argv = sweep_jobs.force_argv(entry, self.options)
        # The paved invocation: a module of the `oracle` package, run from the
        # repository root (`oracle/README.md`, "Layout").
        self.assertEqual(argv[:3], [sys.executable, "-m", "oracle.force"])
        self.assertEqual(argv[argv.index("--formation") + 1], "0x05")
        self.assertIn("--durable", argv)
        self.assertEqual(argv[argv.index("--max-rounds") + 1], "5")
        self.assertEqual(argv[argv.index("--scout") + 1],
                         str(self.options.scout))
        self.assertNotIn("--vehicle", argv)

    def test_the_extraction_caps_the_rounds_and_declares_the_hp_patch(self):
        entry = list_formations(self.pack_of())[0]
        argv = sweep_jobs.extractor_argv(self.report(entry.formation), entry,
                                        self.options)
        self.assertEqual(argv[:3], [sys.executable, "-m", "oracle.fixture"])
        self.assertEqual(argv[argv.index("--max-rounds") + 1], "5")
        self.assertEqual(argv[argv.index("--hp-patch") + 1], "999")
        self.assertIn("--minified", argv)
        self.assertEqual(argv[argv.index("--battle-last") + 1], "200")
        self.assertTrue(argv[argv.index("--out") + 1].endswith(
            "formation_05.json"))

    def test_the_record_names_the_list_the_run_was_given(self):
        with mock.patch.object(sweep_jobs, "run", self.fake_run()):
            self.sweep().run(jobs_count=1)
        document = self.record()
        self.assertEqual(document["source"], {"formations": "the mini pack"})
        self.assertEqual(document["options"]["max_rounds"], 5)
        self.assertTrue(document["options"]["durable"])
        self.assertEqual(document["summary"]["formations"], len(MINI_LIST))


class Recording(SweepFixture):
    def test_a_captured_formation_records_its_fixture_and_outcome(self):
        with mock.patch.object(sweep_jobs, "run", self.fake_run()):
            self.sweep().run(jobs_count=1)
        entry = self.entries()[5]
        self.assertEqual(entry["status"], "captured")
        self.assertEqual(entry["outcome"], "victory")
        self.assertFalse(entry["truncated"])
        self.assertEqual(entry["rounds"], 2)
        self.assertEqual(entry["abilities"], {"e1=0x02": 25000})
        self.assertEqual(entry["selector"]["kind"], "map")
        self.assertEqual(entry["capture"]["log_sha256"], "a" * 64)
        self.assertTrue(entry["fixture"]["path"].endswith("formation_05.json"))
        fixture = pathlib.Path(sweep_jobs.ROOT) / entry["fixture"]["path"]
        self.assertEqual(entry["fixture"]["bytes"], fixture.stat().st_size)
        self.assertEqual(self.record()["summary"]["captured"], len(MINI_LIST))
        # Both commands are in the record, so a formation's capture can be
        # re-run exactly as the sweep ran it.
        self.assertEqual(entry["commands"]["force"][-1], "--durable")
        self.assertEqual(entry["commands"]["extract"][-1], "999")

    def test_a_capture_that_fails_is_recorded_with_its_stage_and_error(self):
        with mock.patch.object(sweep_jobs, "run", self.fake_run(force_fail=(5,))):
            document = self.sweep().run(jobs_count=1)
        entry = self.entries()[5]
        self.assertEqual(entry["status"], "failed")
        self.assertEqual(entry["stage"], "capture")
        self.assertEqual(entry["force_status"], 2)
        self.assertIn("the probe built something else", entry["error"])
        self.assertNotIn("extract", entry["commands"])
        self.assertEqual(document["summary"]["failed"], 1)
        self.assertEqual(document["summary"]["failures"][0]["formation"], 5)

    def test_an_extraction_that_fails_is_recorded_too(self):
        with mock.patch.object(sweep_jobs, "run",
                               self.fake_run(extract_fail=(6,))):
            document = self.sweep().run(jobs_count=2)
        entry = self.entries()[6]
        self.assertEqual(entry["status"], "failed")
        self.assertEqual(entry["stage"], "extract")
        self.assertEqual(entry["extract_status"], 1)
        self.assertIn("no rolls", entry["error"])
        # The capture's own output is still in the record: only the fixture is
        # missing, and the record says which stage to re-run.
        self.assertEqual(entry["commands"]["extract"][-1], "999")
        self.assertEqual(document["summary"]["captured"], len(MINI_LIST) - 1)

    def test_a_failed_formation_does_not_stop_the_others(self):
        with mock.patch.object(sweep_jobs, "run",
                               self.fake_run(force_fail=(5,), extract_fail=(6,))):
            document = self.sweep().run(jobs_count=3)
        self.assertEqual(document["summary"]["formations"], len(MINI_LIST))
        self.assertEqual(document["summary"]["captured"], 0)
        self.assertEqual(document["summary"]["failed"], 2)
        self.assertEqual(sorted(entry["formation"]
                                for entry in document["summary"]["failures"]),
                         [5, 6])

    def test_a_truncated_capture_says_so(self):
        fixture = {"format_version": 1, "rounds": [{"round": 1}],
                   "outcome": {"victory": False, "truncated": True,
                               "rounds_captured": 5}}

        def run(argv, log):
            if "oracle.force" in argv:
                return self.fake_run()(argv, log)
            out = pathlib.Path(argv[argv.index("--out") + 1])
            out.parent.mkdir(parents=True, exist_ok=True)
            out.write_text(json.dumps(fixture))
            log.write_text("wrote the fixture\n")
            return subprocess.CompletedProcess(argv, 0)

        with mock.patch.object(sweep_jobs, "run", run):
            self.sweep().run(jobs_count=1)
        self.assertTrue(self.entries()[5]["truncated"])
        self.assertEqual(self.entries()[5]["rounds_captured"], 5)


class Resume(SweepFixture):
    def test_a_second_run_skips_the_formations_whose_fixtures_are_on_disk(self):
        with mock.patch.object(sweep_jobs, "run", self.fake_run()):
            self.sweep().run(jobs_count=2)
        self.calls.clear()
        with mock.patch.object(sweep_jobs, "run", self.fake_run()):
            document = self.sweep().run(jobs_count=2)
        self.assertEqual(self.calls, [])
        self.assertEqual(document["summary"]["captured"], len(MINI_LIST))
        self.assertEqual(document["summary"]["recorded"], len(MINI_LIST))

    def test_a_failed_formation_is_run_again(self):
        with mock.patch.object(sweep_jobs, "run", self.fake_run(force_fail=(5,))):
            self.sweep().run(jobs_count=1)
        self.calls.clear()
        with mock.patch.object(sweep_jobs, "run", self.fake_run()):
            document = self.sweep().run(jobs_count=1)
        self.assertEqual(self.forced(), {5})
        self.assertEqual(document["summary"]["captured"], len(MINI_LIST))

    def test_a_fixture_that_changed_since_the_record_is_captured_again(self):
        with mock.patch.object(sweep_jobs, "run", self.fake_run()):
            self.sweep().run(jobs_count=1)
        fixture = self.fixtures / "formation_06.json"
        fixture.write_text(json.dumps({"format_version": 1, "rounds": [],
                                       "outcome": {}}))
        self.calls.clear()
        with mock.patch.object(sweep_jobs, "run", self.fake_run()):
            self.sweep().run(jobs_count=1)
        self.assertEqual(self.forced(), {6})

    def test_force_runs_every_formation_again(self):
        with mock.patch.object(sweep_jobs, "run", self.fake_run()):
            self.sweep().run(jobs_count=1)
        self.calls.clear()
        with mock.patch.object(sweep_jobs, "run", self.fake_run()):
            self.sweep().run(jobs_count=2, force=True)
        self.assertEqual(len(self.calls), 2 * len(MINI_LIST))

    def test_the_command_line_writes_the_list_and_the_record(self):
        paths = ["--out", str(self.out), "--fixtures", str(self.fixtures),
                 "--data-dir", self.pack_dir, "--ram-map", self.ram_map,
                 "--list", str(self.out / "list.json"), "--jobs", "1",
                 "--base-tape", os.path.join(self.dir.name, "base.tape")]
        with mock.patch.object(sweep_jobs, "run", self.fake_run()):
            status = main(paths)
        self.assertEqual(status, 0)
        self.assertEqual(self.record()["summary"]["captured"], len(MINI_LIST))
        listed = json.loads((self.out / "list.json").read_text())
        self.assertEqual([entry["formation"] for entry in listed["formations"]],
                         MINI_LIST)

    def test_a_run_with_a_failure_is_its_own_exit_status(self):
        paths = ["--out", str(self.out), "--fixtures", str(self.fixtures),
                 "--data-dir", self.pack_dir, "--ram-map", self.ram_map,
                 "--jobs", "1",
                 "--base-tape", os.path.join(self.dir.name, "base.tape")]
        with mock.patch.object(sweep_jobs, "run",
                               self.fake_run(force_fail=(5, 6))):
            self.assertEqual(main(paths), 1)

    def test_only_and_limit_run_a_subset(self):
        with mock.patch.object(sweep_jobs, "run", self.fake_run()):
            self.sweep().run(jobs_count=1, only={6})
        self.assertEqual(sorted(self.entries()), [6])
        self.calls.clear()
        with mock.patch.object(sweep_jobs, "run", self.fake_run()):
            self.sweep().run(jobs_count=1, limit=1)
        self.assertEqual(len(self.calls), 2)


if __name__ == "__main__":
    unittest.main()
