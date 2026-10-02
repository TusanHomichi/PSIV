"""Local inputs stay ignored whether they are directories or symlinks.

A `dir/` pattern matches only a directory. Lane worktrees symlink the local
inputs in, so a trailing-slash pattern let `git add -A` stage the link; a
committed self-referencing `oracle/gpgx-src` link then replaced the main
checkout's real directory on checkout (2026-10-02). `git check-ignore` with a
path that does not exist treats it as a file, which is exactly the symlink case.

    PYTHONPATH=. python3 -m unittest tests.test_gitignore_inputs -v
"""
import pathlib
import subprocess
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent

#: Every local input a lane or subagent may link into a worktree.
LINKED_INPUTS = (
    "runtime-pack", "reference", "generated", "saves", "build", "roms",
    "oracle/gpgx-src", "oracle/core", "oracle/bin", "oracle/frames",
    "oracle/states", "oracle/layouts", "oracle/logs",
    "Phantasy Star IV (USA).md",
)


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

    def test_negative_control_a_trailing_slash_pattern_misses_a_symlink(self):
        # The mechanism itself, in a scratch repository: `foo/` does not
        # ignore a non-directory `foo` (the bug), `/foo` does (the fix).
        import tempfile
        with tempfile.TemporaryDirectory() as scratch:
            subprocess.run(["git", "init", "-q", scratch], check=True)
            probe = pathlib.Path(scratch)
            def ignored(pattern: str) -> bool:
                (probe / ".gitignore").write_text(pattern + "\n")
                return subprocess.run(
                    ["git", "check-ignore", "--no-index", "-q", "foo"],
                    cwd=probe, capture_output=True).returncode == 0
            self.assertFalse(ignored("foo/"))
            self.assertTrue(ignored("/foo"))

if __name__ == "__main__":
    unittest.main()
