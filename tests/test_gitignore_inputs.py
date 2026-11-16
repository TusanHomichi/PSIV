"""Local inputs stay ignored whether they are directories or symlinks.

A `dir/` pattern matches only a directory. Worktrees symlink the local
inputs in (`tools/local_inputs.py`, which owns the list), so a trailing-slash
pattern let `git add -A` stage the link; a committed self-referencing
`oracle/gpgx-src` link then replaced the main
checkout's real directory on checkout (2026-10-02). `git check-ignore` with a
path that does not exist treats it as a file, which is exactly the symlink case.

    PYTHONPATH=. python3 -m unittest tests.test_gitignore_inputs -v
"""
import pathlib
import subprocess
import unittest

from tests import scratch_git
from tools.local_inputs import LINKED_INPUT_GLOBS, LINKED_INPUTS

ROOT = pathlib.Path(__file__).resolve().parent.parent


def ignored_as_file(path: str) -> bool:
    """Whether git ignores `path` when it is not a directory (a symlink)."""
    result = subprocess.run(
        ["git", "check-ignore", "--no-index", "-q", path],
        cwd=ROOT, capture_output=True)
    return result.returncode == 0


class LinkedInputsAreIgnored(unittest.TestCase):
    def test_every_linked_input_is_ignored_as_a_symlink(self):
        for path in LINKED_INPUTS:
            with self.subTest(path=path):
                self.assertTrue(ignored_as_file(path), f"{path} would be staged as a symlink")

    def test_every_linked_file_pattern_is_ignored(self):
        for pattern in LINKED_INPUT_GLOBS:
            path = pattern.replace("*", "any")
            with self.subTest(path=path):
                self.assertTrue(ignored_as_file(path), f"{path} would be staged as a symlink")

    def test_negative_control_a_trailing_slash_pattern_misses_a_symlink(self):
        # The mechanism itself, in a scratch repository: `foo/` does not
        # ignore a non-directory `foo` (the bug), `/foo` does (the fix).
        import tempfile
        with tempfile.TemporaryDirectory() as scratch:
            scratch_git.init(scratch, branch=None)
            probe = pathlib.Path(scratch)
            def ignored(pattern: str) -> bool:
                (probe / ".gitignore").write_text(pattern + "\n")
                return subprocess.run(
                    ["git", "check-ignore", "--no-index", "-q", "foo"],
                    cwd=probe, capture_output=True).returncode == 0
            self.assertFalse(ignored("foo/"))
            self.assertTrue(ignored("/foo"))



class PackVersionsAreIgnored(unittest.TestCase):
    """The checkout's pack versions and staging are local inputs, never staged."""

    @staticmethod
    def ignored(name):
        return subprocess.run(["git", "check-ignore", "--no-index", "-q", name],
                              cwd=ROOT).returncode == 0

    def test_versions_staging_and_links_beside_any_pack_are_ignored(self):
        for name in (".runtime-pack.v20261018T114840029832Z-1549809",
                     ".runtime-pack.build-1549809-0a1b2c3d", ".runtime-pack.link-1-0a1b2c3d",
                     ".my-mod-pack.v20261018T114840029832Z-1549809",
                     ".my-mod-pack.build-1549809-0a1b2c3d",
                     ".my-mod-pack.link-1549809-0a1b2c3d"):
            with self.subTest(name=name):
                self.assertTrue(self.ignored(name))

    def test_negative_control_tracked_dotfiles_and_other_names_are_not_ignored(self):
        for name in (".runtime-packs.v1", ".my-mod-pack.v2", ".gitignore", ".godot-version",
                     ".github/ISSUE_TEMPLATE/bug.yml", ".github/pull_request_template.md",
                     ".my-mod-pack.v20261018T114840029832Z", ".my-mod-pack.build-1-0a1b2c3"):
            with self.subTest(name=name):
                self.assertFalse(self.ignored(name))


if __name__ == "__main__":
    unittest.main()
