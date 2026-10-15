"""tools/resume_check.py: every resume of a route finishes or halts where the
manifest says.

    PYTHONPATH=. python3 -m unittest tests.test_resume_check -v

The cases run the check on a synthetic four-chapter route with a fake campaign
binary: a script that plays a plan (which resume completes, which halts where,
which crashes), checks that it was handed the right predecessor save, and
writes the report the real runner writes. So the check's classification,
manifest rules, refusals and evaluate mode are tested without running the game.
The negative controls are the manifest's own: an entry removed fails naming the
halt it covered, an entry for a halt that does not occur fails as stale, an
entry without an issue link or a date fails, and a halt that differs from its
entry in any one part of the key (objective, objective kind, halt kind,
battle) is unlisted. A resume counts as completed or halted only with the exit
status that goes with its report. `CheckedInManifest` holds the repository's
manifest to the same rules against the real route.
"""
import contextlib
import io
import json
import os
import stat
import sys
import tempfile
import unittest
from pathlib import Path

from tools import resume_check as rc

ROOT = Path(__file__).resolve().parents[1]
IDS = ["a", "b", "c", "d"]
ISSUE = "https://github.com/TusanHomichi/PSIV/issues/107"

FAKE = r'''
import json, sys
from pathlib import Path
args = sys.argv[1:]
opt = lambda name: args[args.index(name) + 1]
plan = json.loads(Path(__file__).with_name("plan.json").read_text())
with open(Path(__file__).with_name("calls.log"), "a") as calls:
    calls.write(opt("--from-chapter") + "\n")
ids = [c["id"] for c in json.loads(Path(args[1]).read_text())["chapters"]]
start = ids.index(opt("--from-chapter"))
save = Path(opt("--save-dir")) / f"{start - 1:02d}-{ids[start - 1]}" / "slot_1.sram"
if not save.is_file() or save.read_text() != f"save of {ids[start - 1]}":
    sys.exit(1)
step = plan[ids[start]]
if step == "crash":
    sys.exit(1)
report = Path(opt("--report"))
standing = [{"name": "Chaz", "hp": 9, "max_hp": 9, "status": 0, "standing": True}]
down = [{"name": "Chaz", "hp": 0, "max_hp": 9, "status": 4, "standing": False}]
if step == "completed" or step.get("report") == "completed":
    report.write_text(json.dumps({"result": "completed", "event_battles": [
        {"chapter": "d", "id": 1, "party_at_end": standing}]}))
    sys.exit(0 if step == "completed" else step["exit"])
battles = [{"kind": "event", "id": 1, "end_frame": 5, "party_at_end": standing}]
if step["kind"] == "lost_battle":
    battle = step["battle"]
    if battle == "random":
        battles.append({"kind": "encounter", "id": 40, "end_frame": 9, "party_at_end": down})
    else:
        battles.append({"kind": "event", "id": int(battle.split(":")[1]), "end_frame": 9,
                        "party_at_end": down})
report.write_text(json.dumps({"result": "halted", "chapter": step["chapter"],
    "objective_index": step["objective"], "objective_kind": step["objective_kind"],
    "halt": {"kind": step["kind"], "detail": "what happened"}, "battles": battles}))
sys.exit(step.get("exit", 2))
'''

LOST = {"chapter": "d", "objective": 2, "objective_kind": "go_to", "kind": "lost_battle",
        "battle": "event:2"}
STUCK = {"chapter": "d", "objective": 0, "objective_kind": "go_to", "kind": "unreachable"}
PLAN = {"b": "completed", "c": LOST, "d": STUCK}


def entry(halt, resumes, **extra):
    return {**halt, "resumes": resumes, "issue": ISSUE, "date": "2026-10-07", **extra}


MATCHING = [entry(LOST, ["c"]), entry(STUCK, ["d"])]


class Check(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="resume-check-")
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name)
        self.route = self.root / "route.json"
        self.route.write_text(json.dumps({"chapters": [{"id": i} for i in IDS]}))
        self.seed = self.root / "seed"
        chapters = []
        for index, chapter in enumerate(IDS):
            save = self.seed / f"{index:02d}-{chapter}" / "slot_1.sram"
            save.parent.mkdir(parents=True)
            save.write_text(f"save of {chapter}")
            chapters.append({"index": index, "id": chapter, "save": str(save)})
        (self.seed / "report.json").write_text(json.dumps(
            {"result": "completed", "chapters": chapters}))
        self.binary = self.root / "bin" / "psiv-campaign"
        self.binary.parent.mkdir()
        self.binary.write_text(f"#!{sys.executable}\n{FAKE}")
        self.binary.chmod(self.binary.stat().st_mode | stat.S_IXUSR)
        self.set_plan(PLAN)
        self.manifest = self.root / "manifest.json"
        self.write_manifest(MATCHING)
        self.runs = 0

    def set_plan(self, plan):
        (self.binary.parent / "plan.json").write_text(json.dumps(plan))

    def write_manifest(self, halts):
        self.manifest.write_text(json.dumps({"format": 1, "halts": halts}))

    def calls(self):
        log = self.binary.parent / "calls.log"
        return log.read_text().split() if log.is_file() else []

    def check(self, *extra, evaluate=None):
        self.runs += 1
        argv = ["--route", str(self.route), "--manifest", str(self.manifest)]
        if evaluate is None:
            argv += ["--run", str(self.seed), "--binary", str(self.binary),
                     "--out", str(self.root / f"out{self.runs}")]
        else:
            argv += ["--evaluate", str(evaluate)]
        out = io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(out):
            code = rc.main(argv + list(extra))
        return code, out.getvalue()

    def summary(self, run=None, name="summary.json"):
        return json.loads((self.root / f"out{run or self.runs}" / name).read_text())

    def test_outcomes_that_match_the_manifest_pass(self):
        code, out = self.check()
        self.assertEqual(code, rc.EXIT_OK, out)
        self.assertEqual(self.calls(), ["b", "c", "d"], "every resume, in order, once")
        summary = self.summary()
        self.assertEqual(summary["result"], "ok")
        self.assertEqual(summary["completed"], 1)
        self.assertEqual([(r["start"], r["result"]) for r in summary["resumes"]],
                         [("b", "completed"), ("c", "halted"), ("d", "halted")])
        self.assertEqual(summary["fights"], {"1": {"won": ["b", "c", "d"], "lost": []},
                                             "2": {"won": [], "lost": ["c"]}})
        self.assertIn("1 of 3 resumes completed", out)

    def test_an_unlisted_halt_fails_naming_it(self):
        self.write_manifest([entry(STUCK, ["d"])])
        code, out = self.check()
        self.assertEqual(code, rc.EXIT_PROBLEMS, out)
        self.assertEqual(self.summary()["problems"], [
            "resume from c halts at chapter d objective 2 go_to (lost_battle in event:2), which "
            "the manifest does not list: what happened"])

    def test_an_entry_for_a_halt_that_does_not_occur_is_stale(self):
        self.write_manifest(MATCHING + [entry({"chapter": "c", "objective": 1,
                                               "objective_kind": "talk",
                                               "kind": "wrong_object"}, ["b"])])
        code, out = self.check()
        self.assertEqual(code, rc.EXIT_PROBLEMS, out)
        (problem,) = self.summary()["problems"]
        self.assertTrue(problem.startswith("stale: the manifest lists chapter c objective 1 "
                                           "talk (wrong_object) from b"), problem)
        self.assertTrue(problem.endswith("and that resume completes"), problem)

    def test_a_listed_halt_at_another_resume_point_fails_both_ways(self):
        self.write_manifest([entry(LOST, ["b"]), entry(STUCK, ["d"])])
        code, out = self.check()
        self.assertEqual(code, rc.EXIT_PROBLEMS, out)
        problems = self.summary()["problems"]
        self.assertEqual(len(problems), 2, problems)
        self.assertIn("stale: the manifest lists chapter d objective 2 go_to "
                      "(lost_battle in event:2) from b", problems[0])
        self.assertIn("which the manifest lists only from ['b']", problems[1])

    def test_an_entry_without_an_issue_link_or_a_date_fails(self):
        no_issue = entry(LOST, ["c"])
        del no_issue["issue"]
        bad_date = entry(STUCK, ["d"], date="2026-13-40")
        self.write_manifest([no_issue, bad_date])
        code, out = self.check()
        self.assertEqual(code, rc.EXIT_PROBLEMS, out)
        problems = self.summary()["problems"]
        self.assertEqual(len(problems), 2, problems)
        self.assertIn("no issue link", problems[0])
        self.assertIn("no date", problems[1])

    def test_a_malformed_entry_fails(self):
        self.write_manifest([entry(LOST, ["c", "c"]), entry(STUCK, ["a"], isue="x")])
        code, out = self.check()
        self.assertEqual(code, rc.EXIT_PROBLEMS, out)
        problems = " | ".join(self.summary()["problems"])
        self.assertIn("resume point c is also in entry 0", problems)
        self.assertIn("unknown keys ['isue']", problems)
        self.assertIn("resume point 'a' is not a chapter after the first", problems)

    def test_a_resume_that_did_not_run_fails(self):
        self.set_plan({**PLAN, "b": "crash"})
        code, out = self.check()
        self.assertEqual(code, rc.EXIT_PROBLEMS, out)
        self.assertEqual(self.summary()["problems"],
                         ["resume from b did not run: exit 1 and no report"])

    def assert_unlisted(self, halt, start="c"):
        """`halt` from `start`, against the manifest's entry for that resume
        point (LOST from c, STUCK from d), differing in one part of the key:
        unlisted, and the entry stale."""
        self.set_plan({**PLAN, start: halt})
        code, out = self.check()
        self.assertEqual(code, rc.EXIT_PROBLEMS, out)
        problems = self.summary()["problems"]
        self.assertEqual(len(problems), 2, problems)
        self.assertIn("which the manifest does not list", problems[0])
        self.assertIn("that resume halts elsewhere", problems[1])

    def test_a_halt_at_another_objective_is_unlisted(self):
        self.assert_unlisted({**LOST, "objective": 3})

    def test_a_halt_at_another_objective_kind_is_unlisted(self):
        self.assert_unlisted({**LOST, "objective_kind": "step_onto"})

    def test_another_halt_kind_is_unlisted(self):
        self.assert_unlisted({**STUCK, "kind": "budget"}, start="d")

    def test_a_loss_in_another_battle_is_unlisted(self):
        self.assert_unlisted({**LOST, "battle": "event:3"})
        self.assert_unlisted({**LOST, "battle": "random"})

    def test_a_random_loss_matches_an_entry_that_names_random(self):
        self.set_plan({**PLAN, "c": {**LOST, "battle": "random"}})
        self.write_manifest([entry({**LOST, "battle": "random"}, ["c"]), entry(STUCK, ["d"])])
        code, out = self.check()
        self.assertEqual(code, rc.EXIT_OK, out)

    def test_a_lost_battle_entry_names_its_battle(self):
        no_battle = entry({k: v for k, v in LOST.items() if k != "battle"}, ["c"])
        self.write_manifest([no_battle, entry(STUCK, ["d"])])
        code, out = self.check()
        self.assertEqual(code, rc.EXIT_PROBLEMS, out)
        self.assertIn("a lost battle names its battle", self.summary()["problems"][0])

    def test_only_a_lost_battle_entry_names_a_battle(self):
        stray = entry({**STUCK, "battle": "event:2"}, ["d"])
        self.write_manifest([entry(LOST, ["c"]), stray])
        code, out = self.check()
        self.assertEqual(code, rc.EXIT_PROBLEMS, out)
        self.assertIn("only a lost battle names a battle", self.summary()["problems"][0])

    def test_a_halted_report_with_exit_0_is_an_error(self):
        self.set_plan({**PLAN, "d": {**STUCK, "exit": 0}})
        code, out = self.check()
        self.assertEqual(code, rc.EXIT_PROBLEMS, out)
        self.assertEqual(self.summary()["problems"], [
            "resume from d did not run: exit 0, report result 'halted'"])

    def test_a_completed_report_with_exit_2_is_an_error(self):
        self.set_plan({**PLAN, "b": {"report": "completed", "exit": 2}})
        code, out = self.check()
        self.assertEqual(code, rc.EXIT_PROBLEMS, out)
        self.assertEqual(self.summary()["problems"], [
            "resume from b did not run: exit 2, report result 'completed'"])

    def test_a_crash_beside_a_report_is_an_error(self):
        self.set_plan({**PLAN, "b": {"report": "completed", "exit": 101},
                       "d": {**STUCK, "exit": 101}})
        code, out = self.check()
        self.assertEqual(code, rc.EXIT_PROBLEMS, out)
        self.assertEqual(self.summary()["problems"], [
            "resume from b did not run: exit 101 beside a 'completed' report: the runner "
            "stopped without its verdict",
            "resume from d did not run: exit 101 beside a 'halted' report: the runner "
            "stopped without its verdict"])

    def test_evaluate_judges_an_earlier_check_without_running(self):
        self.check()
        first = self.root / "out1"
        self.write_manifest([entry(STUCK, ["d"])])
        code, out = self.check(evaluate=first)
        self.assertEqual(code, rc.EXIT_PROBLEMS, out)
        self.assertEqual(self.calls(), ["b", "c", "d"], "evaluate runs nothing")
        evaluated = json.loads((first / "evaluate-summary.json").read_text())
        self.assertTrue(evaluated["evaluated_only"])
        self.assertIn("which the manifest does not list", evaluated["problems"][0])
        self.assertEqual(json.loads((first / "summary.json").read_text())["result"], "ok",
                         "the run's own summary is kept")

    def test_refusals(self):
        self.write_manifest(MATCHING)
        report = json.loads((self.seed / "report.json").read_text())
        (self.seed / "report.json").write_text(json.dumps({**report, "result": "halted"}))
        code, out = self.check()
        self.assertEqual(code, rc.EXIT_REFUSED, out)
        self.assertIn("not a completed run", out)
        (self.seed / "report.json").write_text(json.dumps(
            {**report, "chapters": report["chapters"][:3]}))
        code, out = self.check()
        self.assertEqual(code, rc.EXIT_REFUSED, out)
        self.assertIn("a run of another route", out)
        (self.seed / "report.json").write_text(json.dumps(report))
        self.manifest.write_text("{")
        code, out = self.check()
        self.assertEqual(code, rc.EXIT_REFUSED, out)
        self.assertIn("cannot read the manifest", out)
        self.write_manifest(MATCHING)
        busy = self.root / "busy"
        busy.mkdir()
        (busy / "x").write_text("x")
        out_io = io.StringIO()
        with contextlib.redirect_stdout(out_io), contextlib.redirect_stderr(out_io):
            code = rc.main(["--route", str(self.route), "--manifest", str(self.manifest),
                            "--run", str(self.seed), "--binary", str(self.binary),
                            "--out", str(busy)])
        self.assertEqual(code, rc.EXIT_REFUSED, out_io.getvalue())
        self.assertIn("is not empty", out_io.getvalue())
        self.assertEqual(self.calls(), [], "a refused check runs nothing")


class CheckedInManifest(unittest.TestCase):
    """The repository's manifest is well formed against the real route: every
    entry names a chapter of the route, a resume point after the first chapter,
    an issue link and a date."""

    def test_the_manifest_is_well_formed(self):
        ids = rc.route_ids(ROOT / rc.ROUTE)
        entries, problems = rc.load_manifest(ROOT / rc.MANIFEST, ids)
        self.assertEqual(problems, [])
        self.assertTrue(entries, "the manifest lists today's halts")


if __name__ == "__main__":
    unittest.main()
