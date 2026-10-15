"""tools/route_sweep.py: a route with its training chapters changed, played and
summarized the same way every time.

    PYTHONPATH=. python3 -m unittest tests.test_route_sweep -v

`Variant` checks the route changes (a patrol's level and money set, a chapter
dropped) and their refusals: an unknown chapter, a chapter with no patrol.
`Sweep` runs the tool with a fake campaign binary that records its command
line and writes the lines the real runner prints, so the variant file, the
seed's save, the flags and the summary (training chapters, every scripted
battle's end, a halt's last battle) are checked without running the game.
`Summary` reads a run's output the way the sweep reads it: from the runner's
`event battle` lines, not the chapter's closing party.
"""
import contextlib
import io
import json
import stat
import sys
import tempfile
import unittest
from pathlib import Path

from tools import route_sweep as rs

ROUTE = {"chapters": [
    {"id": "start", "objectives": [{"do": "go_to"}]},
    {"id": "town-training", "objectives": [
        {"do": "patrol", "until": {"party_level_at_least": 10}}]},
    {"id": "boss", "objectives": [{"do": "fight_scripted"}]},
]}

FAKE = r'''
import json, sys
from pathlib import Path
args = sys.argv[1:]
Path(__file__).with_name("command.json").write_text(json.dumps(args))
opt = lambda name: args[args.index(name) + 1] if name in args else None
print("chapter town-training: 120 frames, 3 battles, saved x; party Chaz L12 50/50")
print("chapter boss: 40 frames, 1 battles, saved y; party Chaz L12 0/50")
print("  event battle 4: frames 130..160; party Chaz 0/50 down; down: Chaz")
print("tape t (160 frames); digest 00ff")
if opt("--from-chapter") == "boss":
    print("HALTED in chapter boss at objective 0 (fight_scripted): lost_battle -- defeated")
    Path(opt("--report")).write_text(json.dumps({"result": "halted", "battles": [
        {"kind": "event", "id": 4, "party_at_end": [
            {"name": "Chaz", "hp": 0, "max_hp": 50, "standing": False}]}]}))
    sys.exit(2)
print("COMPLETED")
'''


class Variant(unittest.TestCase):
    def test_a_patrol_takes_a_level_and_money_and_a_chapter_drops(self):
        changed = rs.variant(ROUTE, {"town-training": 8}, {"town-training": 500}, ["start"])
        self.assertEqual([c["id"] for c in changed["chapters"]], ["town-training", "boss"])
        self.assertEqual(changed["chapters"][0]["objectives"][0]["until"],
                         {"party_level_at_least": 8, "money_at_least": 500})
        self.assertEqual(ROUTE["chapters"][1]["objectives"][0]["until"],
                         {"party_level_at_least": 10}, "the route itself is not changed")

    def test_an_unknown_chapter_or_one_without_a_patrol_is_refused(self):
        with self.assertRaisesRegex(rs.Refused, "no chapter 'nowhere'"):
            rs.variant(ROUTE, {"nowhere": 3}, {}, [])
        with self.assertRaisesRegex(rs.Refused, "boss has 0 patrols"):
            rs.variant(ROUTE, {"boss": 3}, {}, [])
        with self.assertRaisesRegex(rs.Refused, "CHAPTER=N"):
            rs.pairs(["boss:3"], "level")


class Sweep(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="route-sweep-")
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name)
        self.route = self.root / "route.json"
        self.route.write_text(json.dumps(ROUTE))
        self.binary = self.root / "bin" / "psiv-campaign"
        self.binary.parent.mkdir()
        self.binary.write_text(f"#!{sys.executable}\n{FAKE}")
        self.binary.chmod(self.binary.stat().st_mode | stat.S_IXUSR)

    def sweep(self, *argv):
        out = io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(out):
            code = rs.main([*argv, "--route", str(self.route), "--binary", str(self.binary),
                            "--out-root", str(self.root / "sweep")])
        return code, out.getvalue()

    def command(self):
        return json.loads((self.binary.parent / "command.json").read_text())

    def test_a_full_run_plays_the_variant_and_summarizes_it(self):
        code, out = self.sweep("try-8", "--level", "town-training=8")
        self.assertEqual(code, 0, out)
        run = self.root / "sweep" / "try-8"
        variant = json.loads((run / "route.json").read_text())
        self.assertEqual(variant["chapters"][1]["objectives"][0]["until"],
                         {"party_level_at_least": 8})
        command = self.command()
        self.assertEqual(command[:2], ["run", str(run / "route.json")])
        self.assertNotIn("--from-chapter", command)
        self.assertEqual(out.splitlines()[:3], [
            "== try-8: 160 frames, digest 00ff; completed",
            "   town-training: 120 frames, 3 battles; Chaz L12 50/50",
            "   boss event battle 4: frames 130..160; party Chaz 0/50 down; down: Chaz"])

    def test_a_seeded_run_copies_the_save_before_its_first_chapter(self):
        seed = self.root / "seed"
        (seed / "01-town-training").mkdir(parents=True)
        (seed / "01-town-training" / "slot_1.sram").write_text("save")
        code, out = self.sweep("snap", "--seed", str(seed), "--from-chapter", "boss")
        self.assertEqual(code, 2, out)
        run = self.root / "sweep" / "snap"
        self.assertEqual((run / "01-town-training" / "slot_1.sram").read_text(), "save")
        self.assertEqual(self.command()[self.command().index("--from-chapter") + 1], "boss")
        self.assertIn("HALTED in chapter boss", out)
        self.assertIn("last battle: event 4; party Chaz 0/50", out)

    def test_refusals_run_nothing(self):
        cases = [
            (("again", "--from-chapter", "boss"), "go together"),
            (("x", "--level", "nowhere=3"), "no chapter 'nowhere'"),
            (("x", "--seed", str(self.root / "none"), "--from-chapter", "boss"),
             "the seed has no save"),
        ]
        for argv, message in cases:
            code, out = self.sweep(*argv)
            self.assertEqual(code, rs.EXIT_REFUSED, out)
            self.assertIn(message, out)
        self.assertFalse((self.binary.parent / "command.json").exists())

    def test_an_absolute_name_is_refused(self):
        code, out = self.sweep(str(self.root / "elsewhere"), "--replace")
        self.assertEqual(code, rs.EXIT_REFUSED, out)
        self.assertIn("not one plain path component", out)

    def test_dot_names_are_refused(self):
        for name in (".", ".."):
            code, out = self.sweep(name, "--replace")
            self.assertEqual(code, rs.EXIT_REFUSED, out)
            self.assertIn("not one plain path component", out)

    def test_a_link_inside_the_out_root_that_points_elsewhere_is_refused(self):
        outside = self.root / "outside"
        outside.mkdir()
        (outside / "keep.txt").write_text("evidence")
        (self.root / "sweep").mkdir()
        (self.root / "sweep" / "link").symlink_to(outside)
        code, out = self.sweep("link", "--replace")
        self.assertEqual(code, rs.EXIT_REFUSED, out)
        self.assertIn("outside", out)
        self.assertEqual((outside / "keep.txt").read_text(), "evidence")

    def test_an_earlier_run_is_kept_unless_replaced(self):
        self.assertEqual(self.sweep("same")[0], 0)
        code, out = self.sweep("same")
        self.assertEqual(code, rs.EXIT_REFUSED, out)
        self.assertIn("holds an earlier run", out)
        self.assertEqual(self.sweep("same", "--replace")[0], 0)


class Summary(unittest.TestCase):
    def test_a_run_without_output_says_so(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.assertEqual(rs.summarize(Path(tmp)), [f"== {Path(tmp).name}: no out.log"])


if __name__ == "__main__":
    unittest.main()
