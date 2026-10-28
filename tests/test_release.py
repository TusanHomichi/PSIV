"""The release archives hold exactly the release, and their notices name every
third-party part with its license.

`tools/package/release.py` builds an archive per platform and refuses one that
holds anything else (a pack, a ROM, a file from `oracle/`) or whose programs
lost their executable bit; `tools/package/notices.py` writes the notices it
carries. These tests run both on stand-ins: no Godot, no cargo, no ROM. What
the real archives hold, and that each starts on its own system, is the
release workflow's to show (`.github/workflows/release.yml`).

    PYTHONPATH=. python3 -m unittest tests.test_release -v
"""
import io
import json
import os
import pathlib
import re
import stat
import sys
import tarfile
import tempfile
import unittest
import zipfile
from unittest import mock

from tools.package import notices, release
from tools.package.notices import FILES as NOTICE_FILES
from tools.package.notices import NoticeError, Texts
from tools.package.release import PLATFORMS, ReleaseError

ROOT = pathlib.Path(__file__).resolve().parent.parent
VERSION = "1.2.3"


def tar_archive(path: pathlib.Path, files: dict[str, bool]) -> pathlib.Path:
    """A `.tar.gz` holding `files`, each executable or not."""
    with tarfile.open(path, "w:gz") as handle:
        for name, executable in files.items():
            member = tarfile.TarInfo(name)
            member.mode = 0o755 if executable else 0o644
            member.size = 1
            handle.addfile(member, io.BytesIO(b"x"))
    return path


def zip_archive(path: pathlib.Path, files: dict[str, bool]) -> pathlib.Path:
    with zipfile.ZipFile(path, "w") as handle:
        for name, executable in files.items():
            entry = zipfile.ZipInfo(name)
            entry.external_attr = ((0o755 if executable else 0o644) | stat.S_IFREG) << 16
            handle.writestr(entry, b"x")
    return path


def release_files(platform) -> dict[str, bool]:
    """What `assemble` puts in `platform`'s archive, as `archive_files` reads it."""
    folder = platform.archive_name(VERSION)
    return {f"{folder}/{name}": name in platform.programs() for name in platform.files()}


class CheckRefusesAnythingButTheRelease(unittest.TestCase):
    def setUp(self):
        self.dir = pathlib.Path(self.enterContext(tempfile.TemporaryDirectory()))

    def archive(self, platform, files):
        path = self.dir / f"{platform.archive_name(VERSION)}{platform.suffix}"
        if platform.suffix == ".tar.gz":
            return tar_archive(path, files)
        return zip_archive(path, files)

    def test_each_platforms_release_passes(self):
        for platform in PLATFORMS.values():
            with self.subTest(platform.name):
                self.assertEqual(release.check(self.archive(platform, release_files(platform)),
                                               platform), [])

    def test_a_pack_rom_or_oracle_file_is_refused(self):
        for platform in PLATFORMS.values():
            folder = platform.archive_name(VERSION)
            for extra in ("runtime-pack/manifest.json", "Phantasy Star IV (USA).md",
                          "oracle/core/genesis_plus_gx_libretro.so"):
                with self.subTest(platform.name, extra=extra):
                    files = {**release_files(platform), f"{folder}/{extra}": False}
                    self.assertEqual(release.check(self.archive(platform, files), platform),
                                     [f"unexpected: {folder}/{extra}"])

    def test_a_program_that_lost_its_executable_bit_is_refused(self):
        for platform in (PLATFORMS["linux"], PLATFORMS["macos"]):
            folder = platform.archive_name(VERSION)
            for program in sorted(platform.programs()):
                with self.subTest(platform.name, program=program):
                    files = {**release_files(platform), f"{folder}/{program}": False}
                    self.assertEqual(release.check(self.archive(platform, files), platform),
                                     [f"not executable: {folder}/{program}"])

    def test_a_missing_file_is_refused(self):
        platform = PLATFORMS["windows"]
        folder = platform.archive_name(VERSION)
        files = release_files(platform)
        del files[f"{folder}/{NOTICE_FILES[0]}"]
        self.assertEqual(release.check(self.archive(platform, files), platform),
                         [f"missing: {folder}/{NOTICE_FILES[0]}"])

    def test_another_platforms_archive_is_refused_by_name(self):
        linux, windows = PLATFORMS["linux"], PLATFORMS["windows"]
        archive = self.archive(windows, release_files(windows))
        self.assertEqual(len(release.check(archive.rename(archive.with_name(
            archive.name.replace(".zip", ".tar.gz"))), linux)), 1)

    def test_the_command_exits_non_zero_on_a_refusal(self):
        platform = PLATFORMS["linux"]
        self.enterContext(mock.patch("sys.stderr", io.StringIO()))
        good = self.archive(platform, release_files(platform))
        self.assertEqual(release.main(["check", str(good), "--platform", "linux"]), 0)
        folder = platform.archive_name(VERSION)
        bad = self.archive(platform, {**release_files(platform), f"{folder}/saves/x": False})
        self.assertEqual(release.main(["check", str(bad), "--platform", "linux"]), 1)


class AssembleMakesWhatCheckAccepts(unittest.TestCase):
    """`assemble` from stand-ins for the export, the build and the notices;
    none is executable, as an artifact download leaves them."""

    def setUp(self):
        self.dir = pathlib.Path(self.enterContext(tempfile.TemporaryDirectory()))
        self.build = self.dir / "build"
        self.notices = self.dir / "notices"
        for name in NOTICE_FILES:
            (self.notices / name).parent.mkdir(parents=True, exist_ok=True)
            (self.notices / name).write_text("notice\n")

    def assemble(self, platform):
        export = self.dir / "export" / platform.name
        export.mkdir(parents=True)
        made = [platform.export] if platform.name == "macos" else [platform.executable,
                                                                   platform.library]
        for name in made:
            (export / name).write_bytes(b"\0")
        self.build.mkdir(exist_ok=True)
        (self.build / platform.builder).write_bytes(b"\0")
        return release.assemble(platform, export, self.build, self.notices, VERSION,
                                self.dir / "archives")

    def test_linux_and_windows_archives_pass_the_check(self):
        for name in ("linux", "windows"):
            with self.subTest(name):
                platform = PLATFORMS[name]
                archive = self.assemble(platform)
                self.assertEqual(archive.name, f"PSIV-{VERSION}-{name}-x86_64{platform.suffix}")
                self.assertEqual(release.check(archive, platform), [])

    def test_the_archive_carries_the_guide_and_the_notices(self):
        archive = self.assemble(PLATFORMS["linux"])
        with tarfile.open(archive) as handle:
            folder = PLATFORMS["linux"].archive_name(VERSION)
            guide = handle.extractfile(f"{folder}/README.txt").read()
            notice = handle.extractfile(f"{folder}/{NOTICE_FILES[0]}").read()
        self.assertEqual(guide, (ROOT / "docs" / "PLAYING.md").read_bytes())
        self.assertEqual(notice, b"notice\n")

    @unittest.skipIf(sys.platform == "darwin", "macOS assembles its own archive")
    def test_macos_is_assembled_on_macos_only(self):
        with self.assertRaisesRegex(ReleaseError, "on macOS"):
            self.assemble(PLATFORMS["macos"])


class BuildMakesEachPlatformsBinaries(unittest.TestCase):
    def setUp(self):
        self.dir = pathlib.Path(self.enterContext(tempfile.TemporaryDirectory()))
        self.calls = []
        self.enterContext(mock.patch.dict(os.environ, {"CARGO_TARGET_DIR": str(self.dir / "t")}))

    def runner(self, command, env=None):
        self.calls.append((command, env))

    def built(self, platform):
        for target in platform.targets:
            made = self.dir / "t" / target / "release"
            made.mkdir(parents=True, exist_ok=True)
            for name in (platform.library, platform.builder):
                (made / name).write_bytes(target.encode())
        return release.build(platform, self.dir / "out", runner=self.runner)

    def test_windows_links_the_c_runtime_statically(self):
        self.built(PLATFORMS["windows"])
        (command, env), = self.calls
        self.assertEqual(env["CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS"],
                         "-C target-feature=+crt-static")
        self.assertIn("psiv-pack", command)

    def test_linux_builds_its_one_target_with_no_extra_flags(self):
        made = self.built(PLATFORMS["linux"])
        (command, env), = self.calls
        self.assertFalse([key for key in env if key.endswith("_RUSTFLAGS")
                          and key not in os.environ])
        self.assertEqual([path.read_bytes() for path in made], [b"x86_64-unknown-linux-gnu"] * 2)

    def test_macos_joins_both_architectures(self):
        self.built(PLATFORMS["macos"])
        builds = [command for command, _ in self.calls if command[0] == "cargo"]
        joins = [command for command, _ in self.calls if command[0] == "lipo"]
        self.assertEqual(len(builds), 2)
        self.assertEqual(len(joins), 2)
        for join in joins:
            self.assertEqual(sum("apple-darwin" in part for part in join), 2)


class PlatformsAgreeWithTheGameAndTheProject(unittest.TestCase):
    """The names `release.py` uses are the ones the game, the export presets
    and the extension file use."""

    def test_the_builder_is_the_one_the_setup_screen_runs(self):
        install = (ROOT / "rust" / "psiv-godot" / "src" / "install.rs").read_text()
        name = re.search(r'const BUILDER_NAME: &str = "([^"]+)";', install).group(1)
        self.assertEqual(name, release.BUILDER_PACKAGE)
        self.assertIn('format!("{BUILDER_NAME}.exe")', install)
        for platform in PLATFORMS.values():
            expected = f"{name}.exe" if platform.name == "windows" else name
            self.assertEqual(platform.builder, expected)

    def test_every_preset_exists_and_writes_what_release_expects(self):
        presets = (ROOT / "godot" / "export_presets.cfg").read_text()
        found = dict(re.findall(r'^name="([^"]+)"\n(?:.*\n)*?export_path="([^"]*)"', presets,
                                re.M))
        for platform in PLATFORMS.values():
            with self.subTest(platform.name):
                self.assertIn(platform.preset, found)
                self.assertEqual(pathlib.PurePosixPath(found[platform.preset]).name,
                                 platform.export)

    def test_the_extension_file_names_every_platforms_library(self):
        extension = (ROOT / "godot" / "psiv.gdextension").read_text()
        for platform, key in (("linux", "linux.release.x86_64"),
                              ("windows", "windows.release.x86_64"), ("macos", "macos.release")):
            with self.subTest(platform):
                self.assertIn(f'{key} = "res://../rust/target/release/'
                              f'{PLATFORMS[platform].library}"', extension)

    def test_the_smoke_test_waits_for_lines_the_setup_screen_writes(self):
        setup = (ROOT / "rust" / "psiv-godot" / "src" / "pack_setup.rs").read_text()
        for text in (release.SETUP_SHOWN.rstrip(), release.BUILD_STARTED.rstrip(),
                     release.NOT_THE_ROM):
            self.assertIn(text, setup)

    def test_the_records_script_prints_the_mark_export_reads(self):
        self.assertIn(f'"{release.RECORDS_MARK}"', release.GODOT_RECORDS_SCRIPT.read_text())

    def test_a_first_run_sees_none_of_the_games_variables(self):
        with mock.patch.dict(os.environ, {"PSIV_RUNTIME_PACK": "/pack", "PSIV_SAVE_DIR": "/s"}):
            environment = release.first_run_environment(pathlib.Path("/home/first"))
        self.assertFalse([key for key in environment if key.startswith("PSIV_")])
        self.assertEqual(environment["APPDATA"], str(pathlib.Path("/home/first/AppData")))


# --- notices ---------------------------------------------------------------

def package(name, *, kinds=("lib",), source=notices.CRATES_IO, manifest="/x/Cargo.toml",
            license="MIT", repository=None):
    return {"id": name, "name": name, "version": "1.0.0", "source": source,
            "targets": [{"kind": list(kinds)}], "manifest_path": manifest,
            "license": license, "license_file": None, "repository": repository}


def dependency(target, *kinds):
    return {"pkg": target, "dep_kinds": [{"kind": kind, "target": None} for kind in kinds]}


def metadata(packages, edges, members):
    """`cargo metadata`'s shape: `edges` maps a package to its dependencies."""
    return {"packages": packages, "workspace_members": members,
            "resolve": {"nodes": [{"id": item["id"], "deps": edges.get(item["id"], [])}
                                  for item in packages]}}


class LinkedCratesAreTheShippedOnes(unittest.TestCase):
    def setUp(self):
        self.metadata = metadata(
            [package("psiv-godot", source=None), package("psiv-core", source=None),
             package("psiv-campaign", source=None),
             package("linked"), package("deeper"), package("from-core"),
             package("test-only"), package("build-only"), package("campaign-only"),
             package("derive", kinds=("proc-macro",)), package("macro-helper")],
            {"psiv-godot": [dependency("psiv-core", None), dependency("linked", None),
                            dependency("test-only", "dev"), dependency("derive", None)],
             "linked": [dependency("deeper", None), dependency("build-only", "build")],
             "psiv-core": [dependency("from-core", None)],
             "psiv-campaign": [dependency("campaign-only", None)],
             "derive": [dependency("macro-helper", None)]},
            ["psiv-godot", "psiv-core", "psiv-campaign"])

    def test_normal_dependencies_through_the_workspace_are_linked(self):
        self.assertEqual([item["name"] for item in notices.linked(self.metadata, ["psiv-godot"])],
                         ["deeper", "from-core", "linked"])

    def test_a_root_missing_from_the_workspace_fails(self):
        with self.assertRaisesRegex(NoticeError, "psiv-pack"):
            notices.linked(self.metadata, ["psiv-godot", "psiv-pack"])


class EveryCrateBringsItsLicense(unittest.TestCase):
    def setUp(self):
        self.dir = pathlib.Path(self.enterContext(tempfile.TemporaryDirectory()))

    def crate(self, name, license, files=()):
        directory = self.dir / name
        directory.mkdir()
        (directory / "Cargo.toml").write_text("")
        (directory / "README.md").write_text("not a license")
        for file in files:
            (directory / file).write_text(f"{file} of {name}\n")
        return package(name, manifest=str(directory / "Cargo.toml"), license=license)

    def test_a_crates_own_files_come_first(self):
        crate = self.crate("dual", "MIT OR Apache-2.0", ["LICENSE-MIT", "LICENSE-APACHE"])
        self.assertEqual([title for title, _ in notices.crate_licenses(crate)],
                         ["LICENSE-APACHE", "LICENSE-MIT"])

    def test_a_crate_with_no_file_gets_the_kept_text_for_its_license(self):
        crate = self.crate("mpl", "MPL-2.0")
        (title, text), = notices.crate_licenses(crate)
        self.assertEqual(title, "MPL-2.0")
        self.assertIn("Mozilla Public License Version 2.0", text)

    def test_a_crate_with_no_file_and_no_kept_text_fails(self):
        for license in ("MIT OR Apache-2.0", "", "BSD-3-Clause"):
            with self.subTest(license=license), self.assertRaisesRegex(NoticeError, "ships no"):
                notices.crate_licenses(self.crate(f"bare{len(license)}", license))

    def test_identical_texts_are_written_once(self):
        texts = Texts([])
        first = texts.cite("LICENSE-MIT", "same\n")
        self.assertEqual(texts.cite("LICENSE", "same"), first)
        self.assertNotEqual(texts.cite("LICENSE-MIT", "other"), first)
        self.assertEqual(len(texts.entries), 2)


GODOT_RECORDS = {
    "version": {"major": 4, "minor": 7, "patch": 1, "status": "stable",
                "string": "4.7.1-stable (official)"},
    "license": "Copyright (c) Godot Engine contributors.\nPermission is hereby granted",
    "copyright": [{"name": "Godot Engine", "parts": [
        {"files": ["*"], "copyright": ["2014-present, Godot Engine contributors"],
         "license": "Expat"}]}],
    "licenses": {"Expat": "Expat text"},
}

NUKED_SOURCE = """#ifdef HAVE_YM3438_CORE
/*
 * Copyright (C) 2017-2022 Alexey Khokholov (Nuke.YKT)
 *
 * This file is part of Nuked OPN2.
 */
#include "ym3438.h"
"""


class TheNoticesNameEveryPart(unittest.TestCase):
    def setUp(self):
        self.dir = pathlib.Path(self.enterContext(tempfile.TemporaryDirectory()))
        self.sound = self.dir / "sound"
        self.sound.mkdir()
        (self.sound / "ym3438.c").write_text(NUKED_SOURCE)
        (self.sound / "ym3438.h").write_text("/* header */\n")
        self.sysroot = self.dir / "sysroot"
        licenses = self.sysroot / "share" / "doc" / "rust" / "licenses"
        licenses.mkdir(parents=True)
        (licenses / "MIT.txt").write_text("MIT text\n")
        (licenses / "Apache-2.0.txt").write_text("Apache text\n")
        crate = self.dir / "crate"
        crate.mkdir()
        (crate / "LICENSE-MIT").write_text("crate MIT text\n")
        self.metadata = metadata(
            [package("psiv-godot", source=None),
             package("serde", manifest=str(crate / "Cargo.toml"),
                     repository="https://github.com/serde-rs/serde")],
            {"psiv-godot": [dependency("serde", None)]}, ["psiv-godot"])
        self.godot = self.dir / "godot.json"
        self.godot.write_text(json.dumps(GODOT_RECORDS))

    def write(self, **overrides):
        options = dict(targets=[], roots=["psiv-godot"], pinned="4.7.1",
                       metadata=self.metadata, rust=("rustc 9.9.9", self.sysroot),
                       sound=self.sound, commit="c" * 40)
        options.update(overrides)
        return notices.write_notices(self.godot, self.dir / "out", **options)

    def test_every_file_is_written_and_names_its_parts(self):
        written = self.write()
        self.assertEqual(written, [self.dir / "out" / name for name in NOTICE_FILES])
        text = written[0].read_text()
        for part in ("Godot Engine 4.7.1-stable (official)", "Expat text", "Nuked OPN2",
                     "c" * 40, "Alexey Khokholov", "serde 1.0.0",
                     "https://crates.io/crates/serde/1.0.0", "crate MIT text", "rustc 9.9.9",
                     "GNU LESSER GENERAL PUBLIC LICENSE"):
            self.assertIn(part, text)
        self.assertEqual((self.dir / "out" / "nuked-opn2" / "ym3438.c").read_text(),
                         NUKED_SOURCE)

    def test_another_godots_records_are_refused(self):
        with self.assertRaisesRegex(NoticeError, "not the pinned 4.7.2-stable"):
            self.write(pinned="4.7.2")

    def test_a_missing_or_foreign_sound_source_is_refused(self):
        (self.sound / "ym3438.c").write_text("/* something else */\n")
        with self.assertRaisesRegex(NoticeError, "Nuked OPN2's notice"):
            self.write()
        (self.sound / "ym3438.c").unlink()
        with self.assertRaisesRegex(NoticeError, "build_core.sh --sources"):
            self.write()

    def test_the_commit_is_the_one_build_core_pins(self):
        script = (ROOT / "oracle" / "build_core.sh").read_text()
        self.assertIn(f"\nCOMMIT={notices.pinned_gpgx()}\n", script)


if __name__ == "__main__":
    unittest.main()
