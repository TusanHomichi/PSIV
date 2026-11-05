"""Godot's leak reports at exit fail a native verification run (#123).

    PYTHONPATH=. python3 -m unittest tests.test_godot_exit_leaks -v

`tools/godot.py` `exit_leaks` and `logged_exit_leaks` are the one check; the
verifiers that run Godot (certify, the native tape and the flash-attack
verifier) fail a run on any line they return. The report lines below are typed
by hand from Godot 4.7.1's own format strings (core/object/object.cpp
`ObjectDB::cleanup`, core/io/resource.cpp `ResourceCache::clear`,
core/templates/rid_owner.h, core/templates/paged_allocator.h), not from the
pattern under test. The negative controls are each report form, the singular
ObjectDB line a plural-only pattern would miss, a leaked class other than the
one #123 saw, and a verifier that runs Godot without the check.
"""
import ast
import tempfile
import unittest
from pathlib import Path

from tools import godot
from tools.repo_files import repo_files

ROOT = Path(__file__).resolve().parents[1]

#: The tail of a clean headless boot, with the lines a leak pattern must not
#: take for a report: other warnings, a verbose StringName count, and words
#: like "leak" and "exit" in the game's own output.
CLEAN = """\
PSIV field ready: map 0x000, party at (12, 9)
WARNING: some unrelated warning
     at: push_warning (core/variant/variant_utility.cpp:1034)
native tape: no leak measured; exit at frame 240
StringName: 12 unclaimed string names at exit.
"""

#: The #123 report as a verbose headless boot printed it (2026-11-05).
AUDIO_LEAK = """\
WARNING: 1 ObjectDB instance was leaked at exit (run with `--verbose` for details).
     at: cleanup (core/object/object.cpp:2536)
Leaked instance: AudioStreamGeneratorPlayback:9223372118341717473 - Reference count: 1
Hint: Leaked instances typically happen when nodes are removed from the scene tree (with `remove_child()`) but not freed (with `free()` or `queue_free()`).
"""


class ExitLeaks(unittest.TestCase):

    def test_a_clean_exit_reports_nothing(self):
        self.assertEqual(godot.exit_leaks(CLEAN), [])

    def test_the_audio_playback_leak_fails(self):
        self.assertEqual(godot.exit_leaks(CLEAN + AUDIO_LEAK), [
            "WARNING: 1 ObjectDB instance was leaked at exit (run with `--verbose` for details).",
            "Leaked instance: AudioStreamGeneratorPlayback:9223372118341717473 - Reference count: 1",
        ])

    def test_negative_control_the_singular_line_alone_fails(self):
        # Without --verbose only this line prints; "instances leaked" misses it.
        line = "WARNING: 1 ObjectDB instance was leaked at exit (run with `--verbose` for details)."
        self.assertEqual(godot.exit_leaks(line), [line])

    def test_negative_control_every_report_form_fails(self):
        for line in (
                "WARNING: 3 ObjectDB instances were leaked at exit (run with `--verbose` for details).",
                "Leaked instance: Sprite2D:31826181399 - Node name: Body",
                "Leaked instance: Image:-9223371990122233314",
                "ERROR: 2 resources still in use at exit (run with --verbose for details).",
                "ERROR: 2 resources still in use at exit.",
                "Resource still in use: res://field.tscn (PackedScene)",
                "ERROR: 4 RID allocations of type 'N10RendererRD12TextureStorage7TextureE' "
                "were leaked at exit.",
                "ERROR: Pages in use exist at exit in PagedAllocator: N13GodotBody2DE",
        ):
            with self.subTest(line=line):
                self.assertEqual(godot.exit_leaks(CLEAN + line + "\n"), [line])


class LoggedExitLeaks(unittest.TestCase):

    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="godot-exit-leaks-")
        self.addCleanup(tmp.cleanup)
        self.dir = Path(tmp.name)

    def write(self, name, text):
        path = self.dir / name
        path.write_text(text)
        return path

    def test_clean_logs_and_missing_logs_pass(self):
        self.assertEqual(godot.logged_exit_leaks(self.write("run.log", CLEAN),
                                                 self.dir / "never-written.log"), [])

    def test_a_report_in_either_log_fails_once(self):
        # Godot writes its report to stderr and to --log-file: one line each.
        run = self.write("run.log", CLEAN + AUDIO_LEAK)
        log = self.write("godot.log", AUDIO_LEAK)
        self.assertEqual(len(godot.logged_exit_leaks(run, log)), 2)
        self.assertEqual(godot.logged_exit_leaks(self.write("only.log", CLEAN), log),
                         godot.exit_leaks(AUDIO_LEAK))


#: Callers of `godot_command` that do not judge Godot's output, and why.
UNJUDGED = {
    # `run` hands Godot the terminal: its output is the player's, not captured.
    "tools/godot.py",
    # The release build exports with the editor and smoke-runs the release
    # template: packaging, not native verification (#123 scope).
    "tools/package/release.py",
}


def calls(tree, name):
    return any(isinstance(node, ast.Call) and (getattr(node.func, "id", None) == name
                                               or getattr(node.func, "attr", None) == name)
               for node in ast.walk(tree))


def unchecked_runs(name, text):
    """`name` when it starts Godot (`godot_command`) without `logged_exit_leaks`."""
    if name in UNJUDGED:
        return []
    tree = ast.parse(text)
    return [name] if calls(tree, "godot_command") and not calls(tree, "logged_exit_leaks") else []


class EveryVerifierChecks(unittest.TestCase):

    def test_every_tool_that_runs_godot_checks_its_exit(self):
        names = [name for name in repo_files(ROOT, ("*.py",)) if name.startswith("tools/")]
        problems = [problem for name in names
                    for problem in unchecked_runs(name, (ROOT / name).read_text())]
        self.assertEqual(problems, [])

    def test_the_census_sees_the_verifiers(self):
        """Not vacuous: the three verifiers run Godot and are judged."""
        for name in ("tools/certify.py", "tools/verify_native_tape.py",
                     "tools/verify_native_flash_attack.py"):
            with self.subTest(name=name):
                tree = ast.parse((ROOT / name).read_text())
                self.assertTrue(calls(tree, "godot_command"))
                self.assertTrue(calls(tree, "logged_exit_leaks"))

    def test_negative_control_a_run_without_the_check_fails(self):
        text = ("from tools.godot import godot_command\n"
                "command = godot_command(run_root, '--headless')\n")
        self.assertEqual(unchecked_runs("a-new-verifier", text), ["a-new-verifier"])
        self.assertEqual(unchecked_runs("a-new-verifier",
                                        text + "failures = logged_exit_leaks(log)\n"), [])


if __name__ == "__main__":
    unittest.main()
