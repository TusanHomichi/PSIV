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
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import unittest
import zipfile
from unittest import mock

from tools.package import notices, release, symbols
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

    def test_linux_builds_its_one_target_with_explicit_build_id_flag(self):
        made = self.built(PLATFORMS["linux"])
        (command, env), = self.calls
        self.assertEqual(env["CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS"],
                         "-C link-arg=-Wl,--build-id=sha1")
        self.assertIn("--config", command)
        self.assertIn('profile.release.debug="line-tables-only"', command)
        self.assertEqual([path.read_bytes() for path in made], [b"x86_64-unknown-linux-gnu"] * 2)

    def test_linux_build_preserves_existing_target_rustflags(self):
        with mock.patch.dict(os.environ,
                             {"CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS": "-C lto=thin"}):
            self.built(PLATFORMS["linux"])
        (_, env), = self.calls
        self.assertEqual(env["CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS"],
                         "-C lto=thin -C link-arg=-Wl,--build-id=sha1")

    def test_non_linux_builds_do_not_enable_release_debug_override(self):
        for name in ("windows", "macos"):
            with self.subTest(name):
                self.calls.clear()
                self.built(PLATFORMS[name])
                for command, _ in self.calls:
                    self.assertNotIn('profile.release.debug="line-tables-only"', command)

    def test_macos_joins_both_architectures(self):
        self.built(PLATFORMS["macos"])
        builds = [command for command, _ in self.calls if command[0] == "cargo"]
        joins = [command for command, _ in self.calls if command[0] == "lipo"]
        self.assertEqual(len(builds), 2)
        self.assertEqual(len(joins), 2)
        for join in joins:
            self.assertEqual(sum("apple-darwin" in part for part in join), 2)


class LinuxGlibcCeiling(unittest.TestCase):
    def check(self, versions):
        def readelf(command):
            self.assertEqual(command[:3], ["readelf", "--version-info", "--wide"])
            return subprocess.CompletedProcess(command, 0, versions[pathlib.Path(command[-1]).name])
        release.check_glibc([pathlib.Path("libpsiv_godot.so"), pathlib.Path("psiv-pack")],
                            runner=readelf)

    def test_both_binaries_allow_the_ceiling_and_compare_versions_numerically(self):
        self.check({"libpsiv_godot.so": "Name: GLIBC_2.9\nName: GLIBC_2.41\n",
                    "psiv-pack": "Name: GLIBC_2.34\n"})

    def test_either_binary_requiring_newer_glibc_fails(self):
        for binary in ("libpsiv_godot.so", "psiv-pack"):
            versions = dict.fromkeys(("libpsiv_godot.so", "psiv-pack"), "Name: GLIBC_2.34\n")
            versions[binary] += "Name: GLIBC_2.42\n"
            with self.subTest(binary=binary), self.assertRaisesRegex(ReleaseError, binary):
                self.check(versions)

    def test_missing_or_private_version_requirements_fail(self):
        for text in ("", "Name: GLIBC_2.34\nName: GLIBC_PRIVATE\n"):
            with self.subTest(text=text), self.assertRaisesRegex(ReleaseError, "missing public"):
                self.check(dict.fromkeys(("libpsiv_godot.so", "psiv-pack"), text))


def release_profile_symbol_problems(text: str, linux_rustflags: tuple[str, ...]) -> list[str]:
    """Check that Linux release flags keep build-id and Cargo does not strip."""
    problems = []
    joined = " ".join(linux_rustflags)
    if "-Wl,--build-id=" not in joined:
        problems.append("Linux release rustflags must pass -Wl,--build-id=")
    profile = tomllib.loads(text).get("profile", {}).get("release", {})
    if isinstance(profile, dict):
        strip = profile.get("strip")
        if strip not in (None, False, "none"):
            problems.append(f"[profile.release] must not set strip = {strip!r}")
    return problems


class ReleaseProfilePreservesDebugSymbols(unittest.TestCase):
    def test_linux_release_flags_keep_debug_line_tables_and_cargo_does_not_strip(self):
        manifest = (ROOT / "rust" / "Cargo.toml").read_text(encoding="utf-8")
        self.assertEqual(
            release_profile_symbol_problems(manifest, PLATFORMS["linux"].rustflags),
            [],
        )
        for non_linux in ("windows", "macos"):
            self.assertNotIn("debuginfo=", " ".join(PLATFORMS[non_linux].rustflags))

    def test_negative_controls_catch_missing_debug_or_stripping(self):
        manifest = (ROOT / "rust" / "Cargo.toml").read_text(encoding="utf-8")
        self.assertTrue(release_profile_symbol_problems(manifest, ()))
        self.assertTrue(release_profile_symbol_problems(
            manifest + '\n[profile.release]\nstrip = "debuginfo"\n',
            PLATFORMS["linux"].rustflags,
        ))


class LinuxBuildSplitsAndChecksSymbols(unittest.TestCase):
    def setUp(self):
        self.dir = pathlib.Path(self.enterContext(tempfile.TemporaryDirectory()))
        self.lib = self.dir / "libpsiv_godot.so"
        self.builder = self.dir / "psiv-pack"
        self.lib.write_bytes(b"\x7fELFlib")
        self.builder.write_bytes(b"\x7fELFpack")
        self.state = {
            self.lib.name: {"build_id": "aa" * 20, "sections": {".note.gnu.build-id", ".debug_line", ".symtab"}},
            self.builder.name: {"build_id": "bb" * 20, "sections": {".note.gnu.build-id", ".debug_line", ".symtab"}},
        }
        self.commands = []

    def runner(self, command):
        self.commands.append(command)
        tool = command[0]
        if tool == "readelf" and "--notes" in command:
            info = self.state.get(pathlib.Path(command[-1]).name, {})
            bid = info.get("build_id")
            stdout = f"  GNU  0x14  NT_GNU_BUILD_ID\n    Build ID: {bid}\n" if bid else ""
            return subprocess.CompletedProcess(command, 0, stdout, "")
        if tool == "readelf" and "--section-headers" in command:
            sections = sorted(self.state.get(pathlib.Path(command[-1]).name, {}).get("sections", ()))
            stdout = "\n".join(f"  [{i:2d}] {name:<20} PROGBITS" for i, name in enumerate(sections, 1))
            return subprocess.CompletedProcess(command, 0, stdout, "")
        if tool == "objcopy" and command[1] == "--only-keep-debug":
            src, dst = pathlib.Path(command[2]), pathlib.Path(command[3])
            dst.write_bytes(b"debug:" + src.read_bytes())
            self.state[dst.name] = {
                "build_id": self.state[src.name]["build_id"],
                "sections": {".note.gnu.build-id", ".debug_line"},
            }
            return subprocess.CompletedProcess(command, 0, "", "")
        if tool == "objcopy" and command[1] == "--strip-debug":
            target = pathlib.Path(command[2])
            self.state[target.name]["sections"].discard(".debug_line")
            target.write_bytes(b"stripped:" + target.read_bytes())
            return subprocess.CompletedProcess(command, 0, "", "")
        if tool == "objcopy" and command[1].startswith("--add-gnu-debuglink="):
            target = pathlib.Path(command[2])
            self.state[target.name]["sections"].add(".gnu_debuglink")
            return subprocess.CompletedProcess(command, 0, "", "")
        raise AssertionError(f"unexpected command: {command}")

    def test_splits_debug_files_preserves_build_ids_and_writes_manifest(self):
        debug_paths, manifest_path = release.preserve_linux_symbols(
            [self.lib, self.builder], out=self.dir, runner=self.runner,
        )
        self.assertEqual([p.name for p in debug_paths],
                         ["libpsiv_godot.so.debug", "psiv-pack.debug"])
        self.assertEqual(manifest_path.name, symbols.BUILD_SYMBOLS_FILE)
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        self.assertEqual(manifest["binaries"]["libpsiv_godot.so"]["build_id"], "aa" * 20)
        self.assertEqual(manifest["binaries"]["psiv-pack"]["build_id"], "bb" * 20)
        self.assertEqual(manifest["binaries"]["libpsiv_godot.so"]["debug_sha256"],
                         symbols.sha256(self.dir / "libpsiv_godot.so.debug"))
        self.assertNotIn(".debug_line", self.state["libpsiv_godot.so"]["sections"])
        self.assertIn(".gnu_debuglink", self.state["libpsiv_godot.so"]["sections"])

    def test_negative_control_missing_build_id_fails(self):
        self.state["libpsiv_godot.so"]["build_id"] = None
        with self.assertRaisesRegex(ReleaseError, "missing ELF GNU build-id"):
            release.preserve_linux_symbols([self.lib], out=self.dir, runner=self.runner)

    def test_negative_control_missing_debug_line_fails(self):
        self.state["libpsiv_godot.so"]["sections"].discard(".debug_line")
        with self.assertRaisesRegex(ReleaseError, "missing DWARF debug symbols"):
            release.preserve_linux_symbols([self.lib], out=self.dir, runner=self.runner)

    def test_negative_control_mismatched_debug_build_id_fails(self):
        def broken_objcopy(command):
            result = self.runner(command)
            if command[0] == "objcopy" and command[1] == "--only-keep-debug":
                self.state[pathlib.Path(command[3]).name]["build_id"] = "cc" * 20
            return result
        with self.assertRaisesRegex(ReleaseError, "does not match"):
            release.preserve_linux_symbols([self.lib], out=self.dir, runner=broken_objcopy)


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

    def test_a_first_run_sees_only_its_own_save_directory(self):
        with mock.patch.dict(os.environ, {"PSIV_RUNTIME_PACK": "/pack", "PSIV_SAVE_DIR": "/s"}):
            environment = release.first_run_environment(pathlib.Path("/home/first"),
                                                        pathlib.Path("/scratch/saves"))
        self.assertEqual({key: value for key, value in environment.items()
                          if key.startswith("PSIV_")},
                         {"PSIV_SAVE_DIR": str(pathlib.Path("/scratch/saves"))})
        self.assertEqual(environment["APPDATA"], str(pathlib.Path("/home/first/AppData")))


class ExportWritesWhereTheCallerAsked(unittest.TestCase):
    """`export` with Godot stood in for: it writes each export where its
    command line says, reading a relative path from the project, as Godot
    does."""

    def setUp(self):
        self.dir = pathlib.Path(self.enterContext(tempfile.TemporaryDirectory())).resolve()
        self.tmp = self.dir / "tmp"
        self.tmp.mkdir()
        self.enterContext(mock.patch.object(tempfile, "tempdir", str(self.tmp)))
        self.libraries = self.dir / "libraries"
        self.libraries.mkdir()
        for platform in PLATFORMS.values():
            (self.libraries / platform.library).write_bytes(b"\0")
        self.project = self.dir / "run" / "godot"
        self.commands = []
        self.enterContext(mock.patch("tools.godot.require"))
        self.enterContext(mock.patch("tools.godot.create_run_root", self.create_run_root))
        self.enterContext(mock.patch("tools.godot.remove_run_root"))
        self.enterContext(mock.patch("tools.godot.library_path",
                                     lambda root, profile: root / "lib" / profile / "lib.so"))
        self.enterContext(mock.patch("tools.godot.godot_command",
                                     lambda made, *args: ["godot", *args]))
        self.enterContext(mock.patch.object(release, "run", self.godot))
        here = os.getcwd()
        os.chdir(self.dir)
        self.addCleanup(os.chdir, here)

    def create_run_root(self, root, *, profile, library):
        made = mock.Mock(root=root, library=root / "lib" / profile / library.name)
        made.library.parent.mkdir(parents=True)
        shutil.copy2(library, made.library)
        return made

    def godot(self, command):
        self.commands.append(command)
        if "--export-release" in command:
            preset = command[command.index("--export-release") + 1]
            platform, = [item for item in PLATFORMS.values() if item.preset == preset]
            target = self.project / command[-1]
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(b"\0")
            if platform.name != "macos":
                (target.parent / platform.library).write_bytes(b"\0")
            return mock.Mock(stdout="")
        return mock.Mock(stdout=f"{release.RECORDS_MARK}{{}}\n")

    def test_a_relative_destination_is_written_where_the_caller_is(self):
        exports = release.export(self.libraries, pathlib.Path("out"))
        for name, platform in PLATFORMS.items():
            with self.subTest(name):
                self.assertEqual(exports[name], self.dir / "out" / name)
                self.assertTrue((self.dir / "out" / name / platform.export).is_file())
        self.assertTrue((self.dir / "out" / "godot.json").is_file())
        self.assertTrue((self.dir / "out" / symbols.EXPORT_METADATA_FILE).is_file())
        self.assertEqual(list(self.tmp.iterdir()), [])

    def test_export_records_template_version_sha_and_copies_sidecar_symbols(self):
        (self.libraries / "libpsiv_godot.so.debug").write_bytes(b"dwarf-symbols")
        (self.libraries / symbols.BUILD_SYMBOLS_FILE).write_text('{"binaries":{}}\n')
        xdg = self.dir / "xdg"
        pinned = release.pinned_godot_info()
        tpl_dir = xdg / "godot" / "export_templates" / f"{pinned['version']}.stable"
        tpl_dir.mkdir(parents=True)
        (tpl_dir / "version.txt").write_text(f"{pinned['version']}.stable\n")
        (tpl_dir / PLATFORMS["linux"].template).write_bytes(b"linux-template-bytes")
        with mock.patch.dict(os.environ, {"XDG_DATA_HOME": str(xdg)}):
            release.export(self.libraries, pathlib.Path("out"))
        meta = json.loads((self.dir / "out" / symbols.EXPORT_METADATA_FILE).read_text())
        self.assertEqual(meta["godot_version"], pinned["version"])
        self.assertEqual(meta["templates"]["sha512"], pinned["templates"]["sha512"])
        self.assertEqual(meta["templates"]["version_txt"], f"{pinned['version']}.stable")
        self.assertEqual(meta["templates"]["files"]["linux"]["sha256"],
                         symbols.sha256(tpl_dir / PLATFORMS["linux"].template))
        self.assertEqual(sorted(item["file"] for item in meta["symbols"]),
                         [symbols.BUILD_SYMBOLS_FILE, "libpsiv_godot.so.debug"])
        self.assertTrue((self.dir / "out" / "symbols" / "libpsiv_godot.so.debug").is_file())

    def test_export_retains_failure_dir_on_crash_and_cleans_up_when_disabled(self):
        def crash(command):
            raise ReleaseError("godot --export-release crashed")
        kept_dir = self.dir / "export-fail"
        with mock.patch.object(release, "run", crash):
            with self.assertRaisesRegex(ReleaseError, "retained failure directory:"):
                release.export(self.libraries, pathlib.Path("out"), failure_dir=kept_dir)
        report = json.loads((kept_dir / symbols.FAILURE_REPORT_FILE).read_text())
        self.assertEqual(report["stage"], "export")
        self.assertEqual(report["godot"]["version"], release.pinned_godot_info()["version"])
        with mock.patch.object(release, "run", crash):
            with self.assertRaises(ReleaseError) as ctx:
                release.export(self.libraries, pathlib.Path("out"), keep_on_failure=False)
        self.assertNotIn("retained failure directory:", str(ctx.exception))
        self.assertEqual(list(self.tmp.iterdir()), [])


class SmokeNeedsACleanExit(unittest.TestCase):
    """`smoke` with the game stood in for: it writes the lines a first run
    logs, then exits with the code given."""

    def setUp(self):
        self.dir = pathlib.Path(self.enterContext(tempfile.TemporaryDirectory())).resolve()
        self.tmp = self.dir / "tmp"
        self.tmp.mkdir()
        self.enterContext(mock.patch.object(tempfile, "tempdir", str(self.tmp)))
        platform = PLATFORMS["linux"]
        self.archive = tar_archive(self.dir / f"{platform.archive_name(VERSION)}.tar.gz",
                                   release_files(platform))
        self.environments = []
        self.commands = []
        self.enterContext(mock.patch.dict(os.environ, {"PSIV_SAVE_DIR": "/developer/saves"}))

    def game(self, code, *, logged=""):
        def run(command, *, env, **options):
            self.environments.append(env)
            self.commands.append(command)
            log = pathlib.Path(command[command.index("--log-file") + 1])
            log.write_text(f"{logged}{release.SETUP_SHOWN}x\n{release.BUILD_STARTED}x\n"
                           f"{release.NOT_THE_ROM_NAME} {release.NOT_THE_ROM}\n")
            return subprocess.CompletedProcess(command, code, "", "Segmentation fault\n")
        return mock.patch.object(release.subprocess, "run", run)

    def test_a_clean_exit_with_the_lines_passes(self):
        with self.game(0):
            shown = release.smoke(self.archive, PLATFORMS["linux"])
        self.assertTrue(shown)
        self.assertEqual(list(self.tmp.iterdir()), [])

    def test_a_first_run_that_logs_an_error_fails(self):
        logged = ("ERROR: runtime pack failed to load from /p: could not read "
                  "/p/manifest.json: No such file or directory (os error 2)\n"
                  "   at: <psiv_godot::Field as INode2D>::ready (psiv-godot/src/lib.rs:371)\n")
        with self.game(0, logged=logged), \
                self.assertRaisesRegex(ReleaseError, "setup run logged an error"):
            release.smoke(self.archive, PLATFORMS["linux"])

    def test_a_warning_is_not_an_error(self):
        logged = ("WARNING: All audio drivers failed, falling back to the dummy driver.\n"
                  "     at: initialize (servers/audio/audio_server.cpp:258)\n")
        with self.game(0, logged=logged):
            self.assertTrue(release.smoke(self.archive, PLATFORMS["linux"]))

    def test_a_crash_after_the_lines_fails_and_retains_unpacked_export_logs_and_symbols(self):
        sym_dir = self.dir / "syms"
        sym_dir.mkdir()
        (sym_dir / "libpsiv_godot.so.debug").write_bytes(b"dwarf-bytes")
        with self.game(-11), self.assertRaisesRegex(ReleaseError, "setup run exited -11") as ctx:
            release.smoke(self.archive, PLATFORMS["linux"], symbols_dir=sym_dir)
        match = re.search(r"retained failure directory: (.+)$", str(ctx.exception), re.M)
        self.assertIsNotNone(match, str(ctx.exception))
        kept = pathlib.Path(match.group(1))
        self.assertTrue(kept.is_dir())
        run_root = kept / "first run (portabilité)"
        report = json.loads((run_root / symbols.FAILURE_REPORT_FILE).read_text())
        self.assertEqual(report["returncode"], -11)
        self.assertEqual(report["signal"], "SIGSEGV")
        self.assertEqual(report["godot"]["templates"]["sha512"],
                         release.pinned_godot_info()["templates"]["sha512"])
        self.assertTrue(pathlib.Path(report["executable"]).is_file())
        self.assertTrue(pathlib.Path(report["library"]).is_file())
        self.assertTrue(pathlib.Path(report["log_file"]).is_file())
        self.assertIn("Segmentation fault", pathlib.Path(report["stderr_file"]).read_text())
        self.assertTrue((run_root / "symbols" / "libpsiv_godot.so.debug").is_file())

    def test_smoke_moves_retained_artifacts_to_requested_failure_dir(self):
        target = self.dir / "kept-crash"
        with self.game(-11), self.assertRaisesRegex(ReleaseError, str(target)):
            release.smoke(self.archive, PLATFORMS["linux"], failure_dir=target)
        report = json.loads((target / "first run (portabilité)" / symbols.FAILURE_REPORT_FILE).read_text())
        self.assertEqual(report["retained_dir"], str(target))
        self.assertTrue(report["executable"].startswith(str(target)))
        self.assertTrue(pathlib.Path(report["executable"]).is_file())
        self.assertEqual(list(self.tmp.iterdir()), [])

    def test_smoke_refuses_non_empty_failure_dir_without_deleting_it(self):
        existing = self.dir / "prior-crash"
        existing.mkdir()
        kept_file = existing / "earlier.log"
        kept_file.write_text("earlier evidence\n")
        with self.game(-11), self.assertRaisesRegex(ReleaseError, "non-empty failure directory"):
            release.smoke(self.archive, PLATFORMS["linux"], failure_dir=existing)
        self.assertEqual(kept_file.read_text(), "earlier evidence\n")

    def test_move_failure_dir_refuses_non_empty_dir_and_keeps_both(self):
        existing = self.dir / "prior"
        existing.mkdir()
        (existing / "keep.txt").write_text("old\n")
        scratch = self.dir / "scratch-new"
        scratch.mkdir()
        (scratch / "new.txt").write_text("new\n")
        with self.assertRaisesRegex(symbols.SymbolError, "refusing to overwrite"):
            symbols.move_failure_dir(scratch, existing)
        self.assertEqual((existing / "keep.txt").read_text(), "old\n")
        self.assertEqual((scratch / "new.txt").read_text(), "new\n")

    def test_negative_control_smoke_cleans_up_crash_when_keep_on_failure_is_false(self):
        with self.game(-11), self.assertRaisesRegex(ReleaseError, "setup run exited -11") as ctx:
            release.smoke(self.archive, PLATFORMS["linux"], keep_on_failure=False)
        self.assertNotIn("retained failure directory:", str(ctx.exception))
        self.assertEqual(list(self.tmp.iterdir()), [])

    def test_a_clean_exit_without_the_setup_screen_fails(self):
        def no_setup(command, **options):
            return subprocess.CompletedProcess(command, 0, "", "")
        with mock.patch.object(release.subprocess, "run", no_setup):
            with self.assertRaisesRegex(ReleaseError, "never showed"):
                release.smoke(self.archive, PLATFORMS["linux"])

    def test_executable_rom_and_saves_exercise_non_ascii_paths(self):
        with self.game(0):
            release.smoke(self.archive, PLATFORMS["linux"])
        for command in self.commands:
            self.assertIn("first run (portabilité)", command[0])
        self.assertEqual(pathlib.Path(self.commands[-1][-1]).name,
                         "not the game (portabilité).md")

    def test_each_run_has_its_own_scratch_save_directory(self):
        with self.game(0):
            release.smoke(self.archive, PLATFORMS["linux"])
        self.assertEqual(len(self.environments), 2)
        for environment in self.environments:
            saves = pathlib.Path(environment["PSIV_SAVE_DIR"])
            self.assertEqual(saves.name, "saves")
            self.assertEqual(saves.parent.name, "first run (portabilité)")
            self.assertTrue(saves.parent.parent.name.startswith("psiv-smoke-"))


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
                       sound=self.sound, commit="c" * 40, psiv_commit="d" * 40)
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
        self.assertIn(f"https://github.com/TusanHomichi/PSIV/tree/{'d' * 40}", text)
        self.assertIn("oracle/build_core.sh --sources", text)
        self.assertEqual((self.dir / "out" / "nuked-opn2" / "ym3438.c").read_text(),
                         NUKED_SOURCE)

    def test_another_godots_records_are_refused(self):
        with self.assertRaisesRegex(NoticeError, "not the pinned 4.7.2-stable"):
            self.write(pinned="4.7.2")

    def test_a_moving_source_reference_is_refused(self):
        with self.assertRaisesRegex(NoticeError, "full commit hash"):
            self.write(psiv_commit="main")

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


class ReleaseWorkflowPublishesRealNotes(unittest.TestCase):
    """Tagged releases include generated per-release notes plus player/modding pointers."""

    def test_publish_step_combines_generated_notes_with_quick_start(self):
        workflow = (ROOT / ".github" / "workflows" / "release.yml").read_text()
        match = re.search(r"- name: Publish the release\b.*", workflow, re.S)
        self.assertIsNotNone(match, "missing Publish the release step")
        step = match.group(0)
        self.assertTrue(
            "releases/generate-notes" in step or "--generate-notes" in step,
            "Publish the release step must include generated per-release changes",
        )
        self.assertIn("docs/PLAYING.md", step)
        self.assertIn("docs/MODDING.md", step)


if __name__ == "__main__":
    unittest.main()
