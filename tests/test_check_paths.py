"""The code path guard: `tools/check_paths.py`, run inside `tools/check_docs.py`.

    PYTHONPATH=. python3 -m unittest tests.test_check_paths -v

A tracked source that names a deleted doc, a retired driver or a renamed
section reads as a pointer to nothing (issue #114). The cases build a
throwaway git repository, run the checker the way the gate does, and read the
`path:line: message` lines it prints. The negative controls are the three the
issue names (a deleted file, a renamed anchor, a path inside a JSON route's
note string) and the forms around them: a bare retired file name, a quoted
section name, the far end of a range, a climbing `../` path, a path built on
`CARGO_MANIFEST_DIR`, a `res://` path, a missing root file and a stale
`EXEMPT` entry. The
accepted cases pin what the checker lets through on purpose.
"""
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from tools import check_paths

ROOT = Path(__file__).resolve().parents[1]
CHECKER = ROOT / "tools" / "check_paths.py"
DOCS_CHECKER = ROOT / "tools" / "check_docs.py"


def git_env(home):
    """The environment that keeps a case's Git answers its own."""
    return dict(os.environ, HOME=str(home), GIT_CONFIG_NOSYSTEM="1",
                GIT_CONFIG_GLOBAL=os.devnull)


class RepoCase(unittest.TestCase):
    """A throwaway repository with one doc, `docs/a.md`, and whatever a case adds."""

    DOC = "# A\n\n## The northern bridge after Rika joins\n\n**Fade correction:** sixteen frames.\n"

    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="check-paths-")
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name)
        self.home = self.root / "home"
        self.home.mkdir()
        self.write({"docs/a.md": self.DOC, ".gitignore": "/home\n/runtime-pack\n"})
        self.git("init", "-q", "-b", "main")

    def git(self, *args):
        subprocess.run(["git", "-C", str(self.root), *args], capture_output=True,
                       text=True, check=True, env=git_env(self.home))

    def write(self, files):
        for name, text in files.items():
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding="utf-8")

    def run_tool(self, tool=CHECKER):
        self.git("add", "-A")
        proc = subprocess.run([sys.executable, str(tool)], cwd=str(self.root),
                              capture_output=True, text=True, env=git_env(self.home))
        return proc.returncode, proc.stdout + proc.stderr

    def check(self, files):
        self.write(files)
        return self.run_tool()

    def assert_fails(self, files, *messages):
        code, out = self.check(files)
        self.assertEqual(code, 1, out)
        for message in messages:
            self.assertIn(message, out)

    def assert_passes(self, files):
        code, out = self.check(files)
        self.assertEqual(code, 0, out)
        self.assertIn("0 problem(s)", out)


class RealTree(unittest.TestCase):
    """The repository's own code, checked by the checker and through check_docs."""

    def test_repository_tree_passes(self):
        proc = subprocess.run([sys.executable, str(CHECKER)], cwd=str(ROOT),
                              capture_output=True, text=True)
        self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
        self.assertRegex(proc.stdout, r"checked \d{3,} code paths; 0 problem\(s\)")

    def test_every_exempt_entry_names_a_tracked_file(self):
        files = set(check_paths.repo_files(ROOT))
        for source in check_paths.EXEMPT:
            self.assertIn(source, files)


class NegativeControls(RepoCase):

    def test_a_deleted_file(self):
        self.write({"docs/b.md": "# B\n", "src/lib.rs": "//! See `docs/b.md`.\n"})
        self.assertEqual(self.run_tool()[0], 0)
        (self.root / "docs/b.md").unlink()
        code, out = self.run_tool()
        self.assertEqual(code, 1, out)
        self.assertIn("src/lib.rs:1: path not found: docs/b.md", out)

    def test_a_renamed_anchor(self):
        self.assert_fails(
            {"tool.py": "# docs/a.md#post-rika-northern-crossing is the rule\n"},
            "tool.py:1: missing heading anchor: docs/a.md #post-rika-northern-crossing",
        )

    def test_a_renamed_quoted_section(self):
        self.assert_fails(
            {"lib.rs": '/// (`docs/a.md`, "Post-Rika northern crossing")\n'},
            'lib.rs:1: missing heading anchor: docs/a.md "Post-Rika northern crossing"',
        )

    def test_a_path_inside_a_json_note_string(self):
        route = ('{"chapters": [{"objectives": [{"do": "talk", '
                 '"note": "played (docs/campaign/RUNNER_LOG.md H12)"}]}]}\n')
        self.assert_fails(
            {"rust/psiv-campaign/routes/main.json": route},
            "rust/psiv-campaign/routes/main.json:1: path not found: docs/campaign/RUNNER_LOG.md",
        )

    def test_a_quoted_section_wrapped_across_comment_lines(self):
        self.assert_fails(
            {"lib.rs": "/// The rule is `docs/a.md` \"Post-Rika northern\n/// crossing\".\n"},
            'lib.rs:1: missing heading anchor: docs/a.md "Post-Rika northern crossing"',
        )
        self.assert_fails(
            {"tool.py": "# see `docs/a.md`,\n# \"Post-Rika crossing\"\nX = 1\n"},
            'tool.py:1: missing heading anchor: docs/a.md "Post-Rika crossing"',
        )

    def test_a_json_escaped_quoted_section(self):
        route = '{"source": "docs/a.md \\"Post-Rika northern crossing\\""}\n'
        self.assert_fails(
            {"rust/psiv-campaign/routes/main.json": route},
            'missing heading anchor: docs/a.md "Post-Rika northern crossing"',
        )

    def test_a_bare_retired_file_name(self):
        self.assert_fails(
            {"mod.rs": "/// the drivers' loops (`native_alshline.gd`) read it\n"},
            "mod.rs:1: file name matches no file: native_alshline.gd",
        )

    def test_the_far_end_of_a_range(self):
        self.assert_fails(
            {"docs/scenes/03_A.md": "# A\n",
             "lib.rs": "// docs/scenes/03_A.md..11_B.md\n"},
            "lib.rs:1: path not found: docs/scenes/11_B.md",
        )

    def test_a_shell_or_c_source(self):
        self.assert_fails({"oracle/verify.sh": "# see docs/gone.md\n",
                           "oracle/host/rng.c": "/* docs/gone.md */\n"},
                          "oracle/verify.sh:1: path not found: docs/gone.md",
                          "oracle/host/rng.c:1: path not found: docs/gone.md")

    def test_a_path_in_markdown_prose_or_inline_code(self):
        self.assert_fails({"docs/b.md": "# B\n\nsee docs/gone.md and `tests/old.rs`\n"},
                          "docs/b.md:3: path not found: docs/gone.md",
                          "docs/b.md:3: path not found: tests/old.rs")

    def test_check_docs_carries_the_check(self):
        self.write({"lib.rs": "// docs/gone.md\n"})
        code, out = self.run_tool(DOCS_CHECKER)
        self.assertEqual(code, 1, out)
        self.assertIn("lib.rs:1: path not found: docs/gone.md", out)
        self.assertRegex(out, r"1 code paths; 1 problem\(s\)")

    def test_a_climbing_path(self):
        self.assert_fails(
            {"docs/battle/b.md": "# B\n\nsee `../gone.md`\n",
             "rust/c/Cargo.toml": "[package]\n",
             "rust/c/tests/t.rs": 'const TAPES: &str = "../../oracle/tapes";\n'},
            "docs/battle/b.md:3: path not found: ../gone.md",
            "rust/c/tests/t.rs:1: path not found: ../../oracle/tapes")

    def test_a_path_on_the_crate_directory(self):
        self.assert_fails(
            {"rust/c/Cargo.toml": "[package]\n",
             "rust/c/tests/t.rs": 'const R: &str = concat!(env!("CARGO_MANIFEST_DIR"), '
                                  '"/routes/gone.json");\n'
                                  'fn f() {\n    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))\n'
                                  '        .join("../../docs/gone.md");\n}\n'},
            "rust/c/tests/t.rs:1: path not found: rust/c/routes/gone.json",
            "rust/c/tests/t.rs:4: path not found: rust/c/../../docs/gone.md")

    def test_a_godot_resource_path(self):
        self.assert_fails({"godot/field.gd": 'const S = preload("res://gone.tscn")\n'},
                          "godot/field.gd:1: path not found: godot/gone.tscn")

    def test_a_missing_root_file(self):
        self.assert_fails({"lib.rs": "// pinned in .python-version and pyproject.toml\n"},
                          "lib.rs:1: file name matches no file: .python-version",
                          "lib.rs:1: file name matches no file: pyproject.toml")

    def test_an_escaped_dot_path_to_a_missing_file(self):
        self.assert_fails({"tools/t.py": 'PATTERN = re.compile(r"python3 tools/gone\\.py run")\n'},
                          "tools/t.py:1: path not found: tools/gone.py")

    def test_a_stale_exempt_entry(self):
        self.write({"tests/test_x.py": "# writes scratch.md\n"})
        self.git("add", "-A")
        problems, _ = check_paths.check(self.root, {
            "tests/test_x.py": frozenset({"scratch.md", "gone.md"}),
            "tests/test_removed.py": None,
        })
        self.assertIn("tools/check_paths.py: EXEMPT token not in tests/test_x.py: gone.md",
                      problems)
        self.assertIn("tools/check_paths.py: EXEMPT names a missing file: "
                      "tests/test_removed.py", problems)
        self.assertEqual(len(problems), 2, problems)


class AcceptedCases(RepoCase):

    def test_existing_files_directories_and_crate_relative_paths(self):
        self.assert_passes({
            "rust/c/Cargo.toml": "[package]\n",
            "rust/c/tests/runner.rs": "// tests/runner.rs and rust/c/tests/ and docs/a.md\n",
        })

    def test_templates_check_their_directory(self):
        self.assert_passes({"tool.py": "OUT = 'docs/{name}.md'  # and docs/*.md\n"})

    def test_ignored_local_inputs(self):
        self.assert_passes({"tool.py": "PACK = 'runtime-pack/battle/enemies.json'\n"})

    def test_a_wrapped_section_that_exists(self):
        self.assert_passes({"lib.rs": "//! See `docs/a.md`'s \"The northern\n//! bridge after Rika joins\".\n"})

    def test_a_section_without_its_number_or_leading_the(self):
        self.write({"docs/b.md": "# B\n\n## 2. How a shop opens\n\n## The per-frame tick structure\n"})
        self.assert_passes({"lib.rs": '// `docs/b.md` "How a shop opens", `docs/b.md` "per-frame tick structure"\n'})

    def test_a_section_by_heading_prefix_or_bold_label(self):
        self.assert_passes({
            "lib.rs": '// `docs/a.md` "The northern bridge"\n'
                      '// `docs/a.md`\'s "Fade correction"\n'
                      "// docs/a.md#the-northern-bridge-after-rika-joins\n",
        })

    def test_a_bare_name_that_some_file_ends_with(self):
        self.assert_passes({"lib.rs": "// `a.md` and `docs/a.md` and lib.rs\n"})

    def test_a_retired_file_cited_by_commit(self):
        self.assert_passes({"lib.rs": "// RUNNER_LOG (commit 0faa91b^) H16\n"})

    def test_an_exempt_file_is_skipped(self):
        # tests/test_check_docs.py is exempt as a whole in EXEMPT; the table
        # is not verified here, since this is not the checker's own tree.
        self.assert_passes({"tests/test_check_docs.py": "NAME = 'docs/gone.md'\n"})

    def test_climbing_crate_resource_and_root_paths_that_exist(self):
        self.assert_passes({
            ".godot-version": "4.7.1\n",
            "rust-toolchain.toml": "[toolchain]\n",
            "tools/t.gd": "extends Node\n",
            "godot/field.tscn": "[gd_scene]\n",
            "godot/main.gd": 'const A = "res://field.tscn"\nconst B = "res://../tools/t.gd"\n',
            "rust/c/Cargo.toml": "[package]\n",
            "rust/c/routes/main.json": "{}\n",
            "rust/c/tests/t.rs": (
                'const R: &str = concat!(\n    env!("CARGO_MANIFEST_DIR"),\n    "/routes/main.json"\n);\n'
                'const P: &str = "../../runtime-pack/battle";\n'
                'const D: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/a.md");\n'
                "// .godot-version and rust-toolchain.toml\n"),
            "tool.py": "OUTSIDE = '../outside-the-repository'\n",
        })

    def test_a_crate_relative_spelling_of_an_ignored_output(self):
        self.write({".gitignore": "/home\n/build\n"})
        self.assert_passes({"rust/c/Cargo.toml": "[package]\n",
                            "rust/c/src/lib.rs": "// leaves the dump under `rust/c/build/`\n"})

    def test_a_crate_relative_output_the_root_does_not_ignore(self):
        self.assert_fails({"rust/c/Cargo.toml": "[package]\n",
                           "rust/c/src/lib.rs": "// leaves the dump under `rust/c/build/`\n"},
                          "rust/c/src/lib.rs:1: path not found: rust/c/build")

    def test_an_escaped_dot_path_to_a_file_that_exists(self):
        self.assert_passes({"tools/godot.py": "# the resolver\n",
                            "tools/t.py": 'PATTERN = re.compile(r"python3 tools/godot\\.py prepare")\n'
                                          'PLAIN = "tools/godot\\\\.py"\n'})

    def test_a_path_resolves_beside_the_file_that_names_it(self):
        self.assert_passes({"docs/battle/b.md": "# B\n\nsee `../a.md` and `c.md`\n",
                            "docs/battle/c.md": "# C\n"})


if __name__ == "__main__":
    unittest.main()
