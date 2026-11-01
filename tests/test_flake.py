"""Godot's pinned downloads cover the version `.godot-version` pins.

`tools/godot-downloads.json` holds, per Godot version, the official editor zip
(the Nix shell's Godot, `flake.nix`) and the export templates (a release
build, `tools/package/release.py`), each with Godot's published SHA-512. A bump
of `.godot-version` without them passes every other check on a host without
Nix and leaves the Nix shell refusing to start and the release unable to
export, so this reads both files and fails at the bump instead; `python3
tools/godot.py pin` writes them from the release's published sums.

    PYTHONPATH=. python3 -m unittest tests.test_flake -v
"""
import json
import pathlib
import tempfile
import unittest

from tools.godot import (DOWNLOAD_FILES, DOWNLOAD_KINDS, DOWNLOADS_FILE, GodotError, download,
                         pin, pins_from_sums, required_version)

ROOT = pathlib.Path(__file__).resolve().parent.parent
FLAKE = ROOT / "flake.nix"
SUM = "0" * 128
#: A release's names for what PSIV downloads, and its SHA512-SUMS.txt as
#: Godot publishes it, in part: another download's line between them.
EDITOR = DOWNLOAD_FILES["editor"].format(version="9.9.9")
TEMPLATES = DOWNLOAD_FILES["templates"].format(version="9.9.9")
SUMS = (f"{'a' * 128}  {EDITOR}\n"
        f"{'b' * 128}  {EDITOR.replace('linux.x86_64', 'mono_linux_x86_64')}\n"
        f"{'c' * 128}  {TEMPLATES}\n")


class GodotDownloadsCoverThePin(unittest.TestCase):
    def setUp(self):
        self.dir = pathlib.Path(self.enterContext(tempfile.TemporaryDirectory()))

    def pins(self, document) -> pathlib.Path:
        path = self.dir / "pins.json"
        path.write_text(json.dumps(document), encoding="utf-8")
        return path

    def test_the_pinned_version_has_every_download(self):
        for kind in DOWNLOAD_KINDS:
            with self.subTest(kind=kind):
                pinned = download(kind)
                self.assertTrue(pinned.url.endswith(f"/{required_version()}-stable/{pinned.file}"))

    def test_the_flake_reads_the_pins_rather_than_its_own(self):
        text = FLAKE.read_text(encoding="utf-8")
        self.assertIn("./tools/godot-downloads.json", text)
        self.assertIn("godotDownloads.editor.sha512", text)
        self.assertNotIn("godotHashes", text)

    def test_negative_control_a_bumped_pin_without_downloads_fails_against_the_real_pins(self):
        version = self.dir / ".godot-version"
        version.write_text("99.0.0\n", encoding="utf-8")
        with self.assertRaises(GodotError):
            download("editor", required_version(version), DOWNLOADS_FILE)

    def test_negative_control_a_missing_kind_or_malformed_sum_is_no_pin(self):
        for document in ({"4.7.1": {"editor": {"file": "e.zip", "sha512": SUM}}},
                         {"4.7.1": {"editor": {"file": "e.zip", "sha512": "short"},
                                    "templates": {"file": "t.tpz", "sha512": SUM}}},
                         {"4.7.1": {"editor": {"file": "e.zip"},
                                    "templates": {"file": "t.tpz", "sha512": SUM}}},
                         ["4.7.1"]):
            with self.subTest(document=document):
                path = self.pins(document)
                with self.assertRaises(GodotError):
                    for kind in DOWNLOAD_KINDS:
                        download(kind, "4.7.1", path)

    def test_a_well_formed_pin_resolves(self):
        path = self.pins({"4.7.1": {"editor": {"file": "e.zip", "sha512": SUM}}})
        self.assertEqual(download("editor", "4.7.1", path).sha512, SUM)


class PinFromTheReleaseSums(unittest.TestCase):
    def test_pin_writes_the_editor_and_templates_from_the_published_sums(self):
        with tempfile.TemporaryDirectory() as scratch:
            path = pathlib.Path(scratch) / "pins.json"
            path.write_text(json.dumps({"1.0": {}}), encoding="utf-8")
            asked = []
            pin("9.9.9", path, fetch=lambda url: asked.append(url) or SUMS)
            self.assertEqual(asked, ["https://github.com/godotengine/godot/releases/download/"
                                     "9.9.9-stable/SHA512-SUMS.txt"])
            self.assertEqual(json.loads(path.read_text(encoding="utf-8")), {"9.9.9": {
                "editor": {"file": EDITOR, "sha512": "a" * 128},
                "templates": {"file": TEMPLATES, "sha512": "c" * 128}}})
            self.assertEqual(download("templates", "9.9.9", path).sha512, "c" * 128)

    def test_the_pins_file_is_what_pin_writes(self):
        version = required_version()
        written = {version: {kind: {"file": download(kind).file, "sha512": download(kind).sha512}
                             for kind in DOWNLOAD_KINDS}}
        self.assertEqual(DOWNLOADS_FILE.read_text(encoding="utf-8"),
                         json.dumps(written, indent=2) + "\n")

    def test_negative_control_sums_missing_a_file_or_with_a_bad_sum_pin_nothing(self):
        for sums in (SUMS.replace(TEMPLATES, TEMPLATES + ".old"),
                     SUMS.replace("a" * 128, "a" * 127),
                     ""):
            with self.subTest(sums=sums[:40]):
                with self.assertRaises(GodotError):
                    pins_from_sums("9.9.9", sums)


if __name__ == "__main__":
    unittest.main()
