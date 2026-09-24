"""ds-lane's finalize commit: which paths it excludes, and what a git failure leaves.

    PYTHONPATH=. python3 -m unittest discover -s tests -p 'test_ds_lane*.py' -v

Finalize stages and commits what the worker left, keeping the linked inputs and
the worker's own host state out of the commit. Naming a path git already
ignores in that `:(exclude)` list is what git refuses - lane or-O1's finalize
died on a linked ROM matching `Phantasy Star IV*.md` (2026-09-24) - so only the
links without an ignore rule are named. A git step that fails anyway is recorded
as `finalize_error`, with the worker's result and receipt preserved, instead of
a bare SUPERVISOR ERROR and no run.json at all.
"""
import os
import shutil
import subprocess

try:  # `unittest discover -s tests` imports this module top level
    from .ds_lane_support import DS, LaneFixture, RECEIPT_RESULT
except ImportError:  # `python -m unittest tests.test_ds_lane_finalize`
    from ds_lane_support import DS, LaneFixture, RECEIPT_RESULT


FAILING_GIT = '''#!/bin/sh
# A `git` for the finalize-failure cases: the real one, except that the finalize
# step named by DS_LANE_FAIL_GIT fails. `add` is the ignored-path refusal that
# killed lane or-O1's finalize (2026-09-24); `commit` fails after the staging
# step, which leaves the run staged and uncommitted.
case "${DS_LANE_FAIL_GIT:-}:$1" in
  add:add) echo "The following paths are ignored by one of your .gitignore files:" >&2; exit 1 ;;
  commit:commit) echo "fatal: unable to write new index file" >&2; exit 1 ;;
esac
exec "@REAL_GIT@" "$@"
'''


class FinalizeCase(LaneFixture):
    """The finalize commit end to end: real CLI, detached supervisor, fake worker."""

    rom_name = "Phantasy Star IV (USA).md"
    rom_text = "rom bytes\n"

    # -- scaffolding

    def incident_inputs(self):
        """The or-O1 repo shape: an ignored ROM file and an ignored reference tree.

        `Phantasy Star IV*.md` matches the linked ROM *file* at its own name,
        while `reference/` cannot match a directory link: git records a symlink
        as a file (mode 120000), not as a directory. Both inputs are untracked
        and ignored in the repo, so a lane only ever sees them as links.
        """
        (self.repo / ".gitignore").write_text("generated/\nbuild/\nruntime-pack/\n"
                                              "reference/\nPhantasy Star IV*.md\n")
        (self.repo / self.rom_name).write_text(self.rom_text)
        notes = self.repo / "reference" / "notes"
        notes.mkdir(parents=True)
        (notes / "gpgx.txt").write_text("reference sources\n")
        self.git("add", "-A")
        self.git("commit", "-q", "-m", "base: ignored inputs")
        self.base_sha = self.git("rev-parse", "HEAD")

    def failing_git(self, mode):
        """An environment whose PATH puts a `git` that fails the finalize step `mode` first."""
        bindir = self.work / f"failing-git-{mode}"
        bindir.mkdir(parents=True, exist_ok=True)
        script = bindir / "git"
        script.write_text(FAILING_GIT.replace("@REAL_GIT@", shutil.which("git")))
        script.chmod(0o755)
        return {"PATH": f"{bindir}{os.pathsep}{os.environ['PATH']}", "DS_LANE_FAIL_GIT": mode}


    # -- 1. an ignored file link needs no exclusion, a directory link still does

    def test_ignored_file_link_and_directory_link_finalize(self):
        self.incident_inputs()
        spec = {"files": {"tools/new.txt": "owned\n"}, "result": RECEIPT_RESULT}
        self.start("Write tools/new.txt.\n", "t1", spec,
                   extra=["--link", self.rom_name, "--link", "reference"])
        run = self.run_json("t1")
        self.assertEqual(run["exit_code"], 0)
        self.assertIsNone(run["finalize_error"])
        self.assertTrue(run["committed"])

        wt = self.lane_wt("t1")
        # The decision: only the link git does not already ignore is named.
        self.assertTrue(DS.is_ignored(wt, self.rom_name))
        self.assertFalse(DS.is_ignored(wt, "reference"))
        self.assertEqual(DS.commit_excludes(wt, self.lane_json("t1")),
                         [":(exclude)reference", f":(exclude){DS.HOST_STATE_PATHS[0]}"])
        # Only the owned file is committed; neither link reaches the diff.
        self.assertEqual(self.git("show", "--name-only", "--format=", "HEAD", cwd=wt).splitlines(),
                         ["tools/new.txt"])
        self.assertEqual(self.git("diff", "--name-only", f"{self.base_sha}..{run['head_sha']}",
                                  cwd=wt).splitlines(), ["tools/new.txt"])
        self.assertIsNone(run["write_set_violations"])
        # Both links stay readable in the worktree, and neither is tracked: the
        # ROM is ignored and the reference link is simply left alone.
        self.assertEqual((wt / self.rom_name).read_text(), self.rom_text)
        self.assertEqual((wt / "reference" / "notes" / "gpgx.txt").read_text(),
                         "reference sources\n")
        status = self.git("status", "--porcelain", cwd=wt)
        self.assertIn("?? reference", status)
        self.assertNotIn("Phantasy Star IV", status)

        # The negative control: the old unconditional exclusion list still makes
        # this worktree refuse the whole add, and leaves the index untouched.
        links = self.lane_json("t1")["links"]
        old = [f":(exclude){rel}" for rel in links + list(DS.HOST_STATE_PATHS)]
        refused = subprocess.run(["git", "add", "-A", "--", ".", *old], cwd=wt, text=True,
                                 stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        self.assertEqual(refused.returncode, 1, refused.stdout)
        self.assertIn("ignored by one of your .gitignore files", refused.stderr)
        self.assertEqual(self.git("status", "--porcelain", cwd=wt), status)


    # -- 2. the same rule for the worker's host state

    def test_host_state_exclusion_follows_the_repos_ignore_rules(self):
        """`.reasonix` is named only when the repo does not ignore it either.

        The host-state exclusion follows the rule the links do: a repo that
        ignores the directory needs no pathspec, and one that does not still
        gets its exclusion. Filing it with the receipt is independent of both.
        """
        host_state = DS.HOST_STATE_PATHS[0]
        (self.repo / ".gitignore").write_text("generated/\nbuild/\nruntime-pack/\n"
                                              f"{host_state}/\n")
        self.git("add", "-A")
        self.git("commit", "-q", "-m", "base: host state ignored")
        self.base_sha = self.git("rev-parse", "HEAD")

        spec = {"files": {"tools/new.txt": "owned\n",
                          f"{host_state}/tasks/x/events.jsonl": "event\n"},
                "result": RECEIPT_RESULT}
        self.start("Write tools/new.txt.\n", "t1", spec)
        run = self.run_json("t1")
        self.assertEqual(run["exit_code"], 0)
        self.assertIsNone(run["finalize_error"])
        wt = self.lane_wt("t1")
        self.assertTrue(DS.is_ignored(wt, host_state))
        self.assertEqual(DS.commit_excludes(wt, self.lane_json("t1")), [])
        self.assertEqual(self.git("show", "--name-only", "--format=", "HEAD", cwd=wt).splitlines(),
                         ["tools/new.txt"])
        self.assertNotIn(host_state, self.git("status", "--porcelain", cwd=wt))
        # Whatever the rules say, the run's receipt still files what it held.
        filed = self.lane_state("t1", "run-1", "host-state", host_state, "tasks", "x",
                                "events.jsonl")
        self.assertEqual(filed.read_text(), "event\n")


    # -- 3. a git failure in finalize is recorded, not fatal to the run

    def test_failed_finalize_is_recorded_with_the_worker_result(self):
        """A failed git step keeps the run's record: run.json, result and receipt.

        The ignored-path refusal left lane or-O1 with a bare SUPERVISOR ERROR
        and no run.json, though the worker had finished (2026-09-24).
        """
        brief = ("# Brief\n\nTouch only tools/owned.txt.\n\n"
                 "```write-set\ntools/owned.txt\n```\n")
        spec = {"files": {"tools/owned.txt": "owned\n"}, "result": RECEIPT_RESULT}
        self.start(brief, "t1", spec, env=self.failing_git("add"))
        run = self.run_json("t1")
        self.assertEqual(run["exit_code"], 0)          # the worker's own result stands
        self.assertFalse(run["committed"])
        self.assertTrue(run["finalize_error"].startswith("git add -A -- ."),
                        run["finalize_error"])
        self.assertIn("failed (1)", run["finalize_error"])
        self.assertIn("ignored by one of your .gitignore files", run["finalize_error"])
        self.assertEqual(run["head_sha"], self.base_sha)  # nothing was committed
        self.assertFalse(self.lane_state("t1", "run-1", "failed").exists())
        # The checks that read the diff report nothing: there is no diff to read,
        # so the empty lists are the absence of a measurement, not a clean bill.
        self.assertEqual((run["write_set_violations"], run["oversize_files"]), ([], []))

        # The worker's result and receipt are preserved, with the failure explained.
        self.assertIn("Wrote the owned file.",
                      self.lane_state("t1", "run-1", "result.md").read_text())
        summary = self.summary("t1")
        self.assertIn("WARNING: finalize failed: git add -A -- .", summary)
        self.assertIn("the worker's result and receipt are preserved below", summary)
        self.assertIn("(finalize did not complete)", summary)
        self.assertNotIn("SUPERVISOR ERROR", summary)
        self.assertTrue(summary.rstrip().endswith("- Open issues: none"))
        # Its files stay in the worktree, and the run counts as finished: `wait`
        # reports the worker's exit code rather than dying without a result.
        self.assertEqual((self.lane_wt("t1") / "tools" / "owned.txt").read_text(), "owned\n")
        self.assertEqual(self.cli("wait", "t1", "--repo", self.repo, check=0).returncode, 0)

        # A failure after the staging step leaves the run staged but uncommitted.
        self.start(brief, "t2", spec, env=self.failing_git("commit"))
        run2 = self.run_json("t2")
        self.assertEqual(run2["exit_code"], 0)
        self.assertIn("git commit -q --no-verify -m ds-lane t2 run-1", run2["finalize_error"])
        self.assertEqual(run2["head_sha"], self.base_sha)
        self.assertNotIn("SUPERVISOR ERROR", self.summary("t2"))
        self.assertIn("tools/owned.txt",
                      self.git("diff", "--cached", "--name-only", cwd=self.lane_wt("t2")))
