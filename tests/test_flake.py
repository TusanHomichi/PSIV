"""The Nix shell's Godot hash table covers the version `.godot-version` pins.

`flake.nix` downloads the official Godot release and needs its hash, keyed by
version in `godotHashes`. A bump of `.godot-version` without the new hash
passes every other check on a host without Nix and leaves the Nix shell
refusing to start, so this reads both files and fails at the bump instead.

    PYTHONPATH=. python3 -m unittest tests.test_flake -v
"""
import pathlib
import re
import tempfile
import unittest

from tools.godot import required_version

ROOT = pathlib.Path(__file__).resolve().parent.parent
FLAKE = ROOT / "flake.nix"

#: The `godotHashes = { ... };` attribute set in flake.nix.
HASHES_RE = re.compile(r"godotHashes\s*=\s*\{(?P<body>.*?)\};", re.S)
#: One `"<version>" = "sha256-...";` entry inside it.
ENTRY_RE = re.compile(r'"(?P<version>[^"]+)"\s*=\s*"sha256-[A-Za-z0-9+/]{43}=";')


def hashed_versions(flake_text: str) -> set[str]:
    """The Godot versions `flake.nix` holds a release hash for."""
    match = HASHES_RE.search(flake_text)
    if match is None:
        raise ValueError("flake.nix has no godotHashes attribute set")
    # A commented-out entry is no hash to Nix, so it is none here either.
    body = re.sub(r"#[^\n]*", "", match["body"])
    return {entry["version"] for entry in ENTRY_RE.finditer(body)}


class GodotHashCoversThePin(unittest.TestCase):
    def test_the_pinned_version_has_a_hash(self):
        versions = hashed_versions(FLAKE.read_text(encoding="utf-8"))
        self.assertIn(required_version(), versions,
                      "add the release zip's hash for the pinned Godot to godotHashes in flake.nix")

    def test_negative_control_a_version_without_a_hash_is_missing(self):
        text = 'godotHashes = {\n  "4.7.1" = "sha256-' + "A" * 43 + '=";\n};'
        self.assertEqual(hashed_versions(text), {"4.7.1"})
        self.assertNotIn("4.8.0", hashed_versions(text))

    def test_negative_control_a_bumped_pin_without_a_hash_fails_against_the_real_flake(self):
        with tempfile.TemporaryDirectory() as scratch:
            pin = pathlib.Path(scratch) / ".godot-version"
            pin.write_text("99.0.0\n", encoding="utf-8")
            self.assertNotIn(required_version(pin), hashed_versions(FLAKE.read_text(encoding="utf-8")))

    def test_negative_control_a_commented_out_entry_is_not_a_hash(self):
        text = 'godotHashes = {\n  # "4.8.0" = "sha256-' + "A" * 43 + '=";\n};'
        self.assertEqual(hashed_versions(text), set())

    def test_negative_control_a_malformed_hash_is_not_an_entry(self):
        self.assertEqual(hashed_versions('godotHashes = { "4.7.1" = "sha256-short="; };'), set())

    def test_negative_control_no_table_is_an_error(self):
        with self.assertRaises(ValueError):
            hashed_versions("{ }")


if __name__ == "__main__":
    unittest.main()
