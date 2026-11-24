"""rust/Cargo.toml's profiles keep overflow checks and debug assertions on.

    PYTHONPATH=. python3 -m unittest tests.test_build_profiles -v

Retail integer widths rely on a wrap panicking instead of passing silently.
`[profile.test]` builds the gate's tests and `[profile.dev]` builds the
GDExtension that certify and the native tape drivers load, so both must set
`overflow-checks` and `debug-assertions` to true explicitly, and no profile or
per-package override anywhere may set either false. The runtime half of the
guard is rust/psiv-core/tests/build_profile.rs; this file covers the profiles
that test does not run under. The negative controls mutate a copy of the real
manifest for both keys, both profiles and a per-package override.
"""
import tomllib
import unittest
from pathlib import Path

CARGO = Path(__file__).resolve().parents[1] / "rust" / "Cargo.toml"
KEYS = ("overflow-checks", "debug-assertions")
REQUIRED = ("dev", "test")


def profile_problems(text):
    """Every way the manifest `text` weakens the checks, as readable strings."""
    profiles = tomllib.loads(text).get("profile", {})
    problems = []
    for name, table in profiles.items():
        scopes = [(f"[profile.{name}]", table)]
        scopes += [(f"[profile.{name}.package.{package}]", override)
                   for package, override in table.get("package", {}).items()]
        for label, scope in scopes:
            for key in KEYS:
                if scope.get(key) is False:
                    problems.append(f"{label} sets {key} = false")
    for name in REQUIRED:
        table = profiles.get(name, {})
        for key in KEYS:
            if table.get(key) is not True:
                problems.append(f"[profile.{name}] must set {key} = true")
    return problems


class ProfileCase(unittest.TestCase):
    def test_the_manifest_keeps_both_checks_on(self):
        self.assertEqual(profile_problems(CARGO.read_text()), [])

    def mutated(self, old, new):
        text = CARGO.read_text()
        self.assertEqual(text.count(old), 1, f"anchor {old!r} must occur once")
        return profile_problems(text.replace(old, new))

    def test_negative_control_each_key_in_each_required_profile(self):
        text = CARGO.read_text()
        for profile in REQUIRED:
            for key in KEYS:
                with self.subTest(profile=profile, key=key):
                    header = text.index(f"[profile.{profile}]\n")
                    end = text.find("\n[", header + 1)
                    table = text[header:end if end != -1 else len(text)]
                    line = f"{key} = true\n"
                    self.assertEqual(table.count(line), 1, table)
                    at = header + table.index(line)
                    broken = text[:at] + f"{key} = false\n" + text[at + len(line):]
                    self.assertIn(f"[profile.{profile}] sets {key} = false",
                                  profile_problems(broken))
                    missing = text[:at] + text[at + len(line):]
                    self.assertIn(f"[profile.{profile}] must set {key} = true",
                                  profile_problems(missing))

    def test_negative_control_a_per_package_override(self):
        for key in KEYS:
            with self.subTest(key=key):
                problems = self.mutated(
                    "[profile.dev.package.psiv-sound]\nopt-level = 3\n",
                    f"[profile.dev.package.psiv-sound]\nopt-level = 3\n{key} = false\n")
                self.assertEqual(problems,
                                 [f"[profile.dev.package.psiv-sound] sets {key} = false"])

    def test_negative_control_another_profile(self):
        problems = profile_problems(CARGO.read_text()
                                    + "\n[profile.release]\noverflow-checks = false\n")
        self.assertEqual(problems, ["[profile.release] sets overflow-checks = false"])


if __name__ == "__main__":
    unittest.main()
