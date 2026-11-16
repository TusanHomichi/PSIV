"""Keep PSIV's toolchain manifests independent of burrow's transitive lock.

The CI gate-test glob runs this without Nix or local game assets. Nix's own
flake evaluation checks that the chosen overlay actually supplies the Rust pin.
"""
import json
import re
import unittest
from pathlib import Path
from urllib.parse import parse_qs, urlsplit

ROOT = Path(__file__).resolve().parents[1]


class RustOverlayOwnership(unittest.TestCase):
    def assert_owned(self, flake, lock):
        declaration = re.search(r'inputs\.burrow\.inputs\.rust-overlay\.url\s*=\s*"([^"]+)";', flake)
        self.assertIsNotNone(declaration, "PSIV must override burrow's older rust-overlay")
        revision = parse_qs(urlsplit(declaration[1]).query).get("rev", [""])[0]
        self.assertRegex(revision, r"^[0-9a-f]{40}$")
        nodes = lock["nodes"]
        burrow = nodes[nodes[lock["root"]]["inputs"]["burrow"]]
        overlay = nodes[burrow["inputs"]["rust-overlay"]]
        self.assertEqual(overlay["original"]["rev"], revision)
        self.assertEqual(overlay["locked"]["rev"], revision)
        self.assertEqual(overlay["inputs"]["nixpkgs"], ["burrow", "nixpkgs"])

    def test_psiv_owns_the_overlay_revision_instead_of_inheriting_burrows_lock(self):
        self.assert_owned((ROOT / "flake.nix").read_text(),
                          json.loads((ROOT / "flake.lock").read_text()))

    def test_negative_control_an_inherited_overlay_is_rejected(self):
        flake = (ROOT / "flake.nix").read_text()
        inherited = re.sub(r'inputs\.burrow\.inputs\.rust-overlay\.url\s*=\s*"[^"]+";', "", flake)
        with self.assertRaisesRegex(AssertionError, "PSIV must override"):
            self.assert_owned(inherited, json.loads((ROOT / "flake.lock").read_text()))
