"""tests/scratch_git.py: a test's throwaway repository runs no automatic
Git maintenance, and every test module starts its repositories through it.

    PYTHONPATH=. python3 -m unittest tests.test_scratch_git -v

`MaintenanceCase` traces a commit in a repository `init` started: Git starts
no `git maintenance run --auto` there, where its negative control, a plain
`git init`, does. `OnePathCase` reads every test module for a `git init` of
its own (#371: test_local_inputs's own init lost a race to the maintenance a
commit started); its negative controls are each spelling the tests used.
"""

import os
import re
import subprocess
import tempfile
import unittest
from pathlib import Path

from tests import scratch_git

TESTS = Path(__file__).resolve().parent
HELPER = TESTS / "scratch_git.py"

#: A `git init` written as an argument list: `"init",` or `'init',`, the form
#: of every call to git a test makes.
INIT = re.compile(r"""["']init["']\s*,""")


def traced_commit(repo: Path) -> str:
    """GIT_TRACE's lines for one commit in `repo`."""
    env = dict(os.environ, GIT_TRACE="1", GIT_CONFIG_NOSYSTEM="1",
               GIT_CONFIG_GLOBAL=os.devnull, HOME=str(repo))
    (repo / "file").write_text("one\n")
    for args in (["add", "-A"], ["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q",
                                 "-m", "one"]):
        done = subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True,
                              env=env, check=True)
    return done.stderr


class MaintenanceCase(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="scratch-git-")
        self.addCleanup(tmp.cleanup)
        self.tmp = Path(tmp.name)

    def test_a_commit_starts_no_maintenance(self):
        repo = scratch_git.init(self.tmp / "repo")
        self.assertNotIn("maintenance run", traced_commit(repo))
        self.assertEqual(subprocess.run(["git", "-C", str(repo), "branch", "--show-current"],
                                        capture_output=True, text=True, check=True).stdout,
                         "main\n")

    def test_negative_control_a_plain_init_starts_maintenance(self):
        repo = self.tmp / "plain"
        subprocess.run(["git", "init", "-q", "-b", "main", str(repo)], check=True,
                       capture_output=True)
        self.assertIn("maintenance run --auto", traced_commit(repo))

    def test_a_failed_init_says_why(self):
        (self.tmp / "file").write_text("not a directory\n")
        with self.assertRaisesRegex(RuntimeError, "exited"):
            scratch_git.init(self.tmp / "file" / "repo")


class OnePathCase(unittest.TestCase):
    def test_no_test_module_runs_its_own_git_init(self):
        own = [f"{path.name}:{number}"
               for path in sorted(TESTS.rglob("*.py")) if path != HELPER
               for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1)
               if INIT.search(line) and path != Path(__file__).resolve()]
        self.assertEqual(own, [], "start a test repository with tests/scratch_git.py's init")

    def test_negative_control_each_spelling_is_found(self):
        for line in ('        self.git("init", "-q", "-b", "main")',
                     '        git(self.tmp, "init", "-q", "-b", "main", str(self.repo))',
                     '            subprocess.run(["git", "init", "-q", scratch], check=True)',
                     "        for args in (['init', '-q'], ['add', '-A']):"):
            with self.subTest(line=line):
                self.assertTrue(INIT.search(line))
        self.assertIsNone(INIT.search('        self.git("commit", "-q", "-m", "init")'))


if __name__ == "__main__":
    unittest.main()
