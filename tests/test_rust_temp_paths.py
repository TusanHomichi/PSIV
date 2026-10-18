"""The Rust test suites' temp directories are safe on parallel threads (#157).

    PYTHONPATH=. python3 -m unittest tests.test_rust_temp_paths -v

The gate runs `cargo test` at cargo's default test threads (docs/DEVELOPMENT.md,
`## Run checks`), so tests in one binary run side by side, and several test
processes share the system temp directory. This guard reads every Rust source
under rust/ (no build, no cargo) and holds each `temp_dir()` call to a process
key, a per-call key and a unique prefix; the one-helper replacement is #168.
"""

import re
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]


def temp_dir_findings(text, name):
    """Every `temp_dir()` call in Rust source `text` that parallel tests can collide on.

    The gate runs each test binary's tests on parallel threads, and several
    test processes (other checkouts' suites) share the system temp directory.
    So a test's directory under `temp_dir()` is keyed by `std::process::id()`
    in the expression that builds it, and, unless that expression sits directly
    in one `#[test]` function, also by a per-call key: an atomic counter
    (`fetch_add`) or a caller-supplied name (a second format placeholder or a
    `.join(name)`), never a clock reading, which two threads can read alike.
    The prefix after `temp_dir()` must also be unique across the tree, so two
    call sites never share one directory. Returns `(path:line, reason)` pairs
    and the prefixes seen, for the tree-wide uniqueness check.
    """
    lines = text.splitlines()
    findings, prefixes = [], []
    for index, line in enumerate(lines):
        if "temp_dir()" not in line or line.lstrip().startswith("//"):
            continue
        where = f"{name}:{index + 1}"
        statement = []
        for following in lines[index:index + 8]:
            statement.append(following)
            if ";" in following or following.strip() == "}":
                break
        expression = " ".join(part.strip() for part in statement)
        expression = expression[expression.index("temp_dir()"):]
        prefix = re.search(r'"(psiv-[^"{]*)', expression)
        if prefix is None:
            findings.append((where, "no literal `psiv-` prefix after temp_dir()"))
        else:
            prefixes.append((prefix.group(1).rstrip("-"), where))
        if "process::id()" not in expression:
            findings.append((where, "not keyed by std::process::id()"))
            continue
        header = next((i for i in range(index, -1, -1)
                       if re.match(r"\s*(pub(\([a-z]+\))? )?fn ", lines[i])), None)
        attributes = []
        for above in range(header - 1 if header is not None else -1, -1, -1):
            stripped = lines[above].strip()
            if not (stripped.startswith("#[") or stripped.startswith("///")):
                break
            attributes.append(stripped)
        if "#[test]" in attributes:
            continue
        body = "\n".join(lines[header:index + len(statement)]) if header is not None else ""
        format_string = re.search(r'format!\(\s*"([^"]*)"', expression)
        placeholders = len(re.findall(r"\{[^}]*\}", format_string.group(1))) if format_string else 0
        per_call = ("fetch_add" in body or placeholders >= 2
                    or re.search(r"\)\s*\.join\(\s*[a-z_]\w*\s*\)", expression))
        clock = "SystemTime" in body or "as_nanos" in body
        if not per_call or (clock and "fetch_add" not in body):
            findings.append((where, "outside a #[test] fn without a per-call key "
                                    "(an atomic counter or a caller's name, not a clock)"))
    return findings, prefixes


def temp_dir_tree_findings(sources):
    """`temp_dir_findings` over `(name, text)` pairs, plus every prefix two sites share."""
    findings, seen, sites = [], {}, 0
    for name, text in sources:
        found, prefixes = temp_dir_findings(text, name)
        findings += found
        sites += len(prefixes)
        for prefix, where in prefixes:
            seen.setdefault(prefix, []).append(where)
    findings += [(", ".join(wheres), f"prefix {prefix!r} shared")
                 for prefix, wheres in seen.items() if len(wheres) > 1]
    return findings, sites


def rust_sources():
    """`(path, text)` for every Rust file under rust/, outside build output, calling temp_dir()."""
    rust = REPO_ROOT / "rust"
    for path in sorted(rust.rglob("*.rs")):
        if "target" in path.relative_to(rust).parts:
            continue
        text = path.read_text()
        if "temp_dir()" in text:
            yield str(path.relative_to(REPO_ROOT)), text


class ParallelTestPathsCase(unittest.TestCase):
    """Rust tests run on parallel threads (#157): no two share a temp directory."""

    def test_every_temp_dir_is_keyed_by_process_and_call(self):
        findings, sites = temp_dir_tree_findings(rust_sources())
        self.assertGreater(sites, 20, "too few temp_dir() sites found to be reading the tree")
        self.assertEqual(findings, [])

    def check(self, source):
        return [reason for _, reason in temp_dir_findings(source, "sample-source")[0]]

    def test_negative_control_an_unkeyed_name_is_found(self):
        # The pre-#157 psiv-core/src/battle/replay/data.rs line, collided on by
        # two suites at once ("cannot create .../lane-evidence: No such file").
        old = ('#[test]\nfn t() {\n'
               '    let dir = std::env::temp_dir().join("psiv-empty-manifest-dump");\n}\n')
        self.assertEqual(self.check(old), ["not keyed by std::process::id()"])

    def helper(self, *body, signature="fn scratch() -> PathBuf"):
        """A non-test helper whose body is `body`, one statement per line."""
        return signature + " {\n" + "".join(f"    {line}\n" for line in body) + "}\n"

    def test_negative_control_a_shared_helper_needs_a_per_call_key(self):
        root = "std::env::temp_dir().join(format!("
        pid_only = self.helper(root + '"psiv-x-{}", std::process::id()))')
        self.assertEqual(len(self.check(pid_only)), 1)
        clock = self.helper("let nanos = SystemTime::now().as_nanos();",
                            root + '"psiv-x-{}-{nanos}", std::process::id()))')
        self.assertEqual(len(self.check(clock)), 1)
        counter = self.helper("let n = NEXT.fetch_add(1, Ordering::Relaxed);",
                              root + '"psiv-x-{}-{n}", std::process::id()))')
        self.assertEqual(self.check(counter), [])
        named = self.helper(root + '"psiv-x-{}", std::process::id())).join(name)',
                            signature="fn scratch(name: &str) -> PathBuf")
        self.assertEqual(self.check(named), [])

    def test_negative_control_one_prefix_at_two_sites_is_found(self):
        site = ('#[test]\nfn t() {\n    let d = std::env::temp_dir()'
                '.join(format!("psiv-same-{}", std::process::id()));\n}\n')
        sources = [("first-source", site), ("second-source", site)]
        findings, sites = temp_dir_tree_findings(sources)
        self.assertEqual(sites, 2)
        self.assertEqual(findings,
                         [("first-source:3, second-source:3", "prefix 'psiv-same' shared")])


if __name__ == "__main__":
    unittest.main()
