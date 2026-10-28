#!/usr/bin/env python3
"""Build PSIV's release archives: the game for Linux, Windows and macOS, each
with its pack builder beside it and the papers a player needs.

`.github/workflows/release.yml` runs these steps on GitHub's runners, in this
order. Each runs the same on a developer's machine of the platform it needs.

    python3 tools/package/release.py build --platform P --out DIR

builds the game's extension and the pack builder (`psiv-pack`) for platform P
(`linux`, `windows` or `macos`), release profile, on that platform. macOS
builds both architectures and joins each pair into one universal binary with
`lipo`. The workflow builds Linux on the oldest runner GitHub offers, so the
binaries' glibc floor suits the Steam Deck and older distributions.

    python3 tools/package/release.py install-godot --dir DIR [--cache DIR]

downloads the pinned Godot's Linux editor and export templates
(`tools/godot-downloads.json`), checks their SHA-512, unpacks the editor into
DIR and the templates where Godot looks for them, and prints the editor's
path. Linux only: every platform is exported from Linux.

    python3 tools/package/release.py export --libraries DIR --out DIR

exports the game for every platform from one release run root
(`tools/godot.py`). `--libraries` holds each platform's extension from
`build`, and the run root holds them where `godot/psiv.gdextension` names
them. `OUT/<platform>/` gets the export: the Linux and Windows executables
with the extension beside them, and the macOS app, zipped. `OUT/godot.json`
gets Godot's own license and copyright records, for the notices.

    python3 tools/package/release.py notices --godot FILE --out DIR

writes the third-party notices, and the source they say travels with the game,
into DIR (`tools/package/notices.py`). It needs the Genesis Plus GX checkout
(`oracle/build_core.sh --sources`).

    python3 tools/package/release.py assemble --platform P --export DIR \\
        --build DIR --notices DIR --version V --out DIR

makes platform P's archive. It holds the export, with `psiv-pack` beside the
executable (in the macOS app, `Contents/MacOS`), the player's guide
(`docs/PLAYING.md`) as `README.txt`, `LICENSE.txt`, and what `notices` wrote.
Windows builds link the C runtime statically, so the game needs no Visual C++
redistributable. On macOS
it signs the app ad hoc (`codesign`; PSIV has no paid signing identity) and
zips it with `ditto`, which keeps the signature, so it runs on macOS only.

    python3 tools/package/release.py check ARCHIVE --platform P

refuses an archive that holds anything but what `assemble` puts there (no
pack, no ROM, nothing from `oracle/`), or whose programs lost their
executable bit.

    python3 tools/package/release.py smoke ARCHIVE --platform P

unpacks the archive and starts the game in it as a player's first run does,
headless, on the platform it is for. With no pack it must show the setup
screen. Given a file that is not the ROM on its command line, it must run the
`psiv-pack` beside it and show the builder's refusal. What a real ROM builds
is the full gate's to prove, on the machine that holds one.

Stdlib only, Python 3. `tools/godot.py` is imported only where Godot runs,
since it needs `fcntl`, which Windows lacks.
"""

from __future__ import annotations

import argparse
import hashlib
import os
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import zipfile
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(ROOT))

from tools.package.notices import FILES as NOTICE_FILES  # noqa: E402
from tools.package.notices import NoticeError, write_notices  # noqa: E402

#: The workspace packages a release ships: the extension and the builder.
EXTENSION_PACKAGE = "psiv-godot"
BUILDER_PACKAGE = "psiv-pack"

#: What every archive holds beside the game and the notices, by name in the
#: archive.
PAPERS = {"README.txt": ROOT / "docs" / "PLAYING.md",
          "LICENSE.txt": ROOT / "LICENSE"}

#: The macOS app the macOS export holds, and its parts the archive adds to or
#: runs.
APP = "PSIV.app"
APP_EXECUTABLE = f"{APP}/Contents/MacOS/PSIV"
APP_BUILDER_DIRECTORY = f"{APP}/Contents/MacOS"

#: Everything else Godot's macOS export puts in the app, and the seal the
#: ad hoc signature adds. A new file here fails `check` until it is listed.
APP_FILES = frozenset({
    f"{APP}/Contents/Info.plist",
    f"{APP}/Contents/PkgInfo",
    f"{APP}/Contents/Resources/PSIV.pck",
    f"{APP}/Contents/Resources/icon.icns",
    f"{APP}/Contents/Resources/PrivacyInfo.xcprivacy",
    f"{APP}/Contents/Frameworks/libpsiv_godot.dylib",
    f"{APP}/Contents/_CodeSignature/CodeResources",
})

#: The first run's lines the smoke test waits for (`pack_setup.rs`), and the
#: builder's refusal of a file that is not the ROM (its wording is the
#: builder's own, kept by `psiv-pack` from `psiv_tools/core.py`).
SETUP_SHOWN = "pack setup: no usable runtime pack at "
BUILD_STARTED = "pack setup: building "
NOT_THE_ROM = "is not the US retail ROM PSIV builds from"

#: The file the smoke test offers in place of the ROM.
NOT_THE_ROM_NAME = "not-the-game"

#: Frames a smoke run lasts, at `--max-fps 60`, and its time limit in seconds.
SMOKE_FRAMES = {"setup": "120", "refusal": "300"}
SMOKE_TIMEOUT = 180

#: Bytes read at a time from a download.
CHUNK = 1 << 20
DOWNLOAD_TIMEOUT = 120


class ReleaseError(RuntimeError):
    """A release step could not do its part; the message says why."""


@dataclass(frozen=True)
class Platform:
    """One platform a release is built for."""

    name: str
    #: The Rust targets it builds, joined with `lipo` when there are two.
    targets: tuple[str, ...]
    #: The extension's file name, as `godot/psiv.gdextension` names it.
    library: str
    #: The pack builder's file name, as `install::builder_command` runs it.
    builder: str
    #: Its preset in `godot/export_presets.cfg`, and what that writes.
    preset: str
    export: str
    #: Its release template, in Godot's export templates.
    template: str
    #: The game's executable in the archive's folder.
    executable: str
    arch: str
    suffix: str
    #: Flags for its targets' builds.
    rustflags: tuple[str, ...] = ()

    def archive_name(self, version: str) -> str:
        """The archive's folder: `PSIV-<version>-<platform>-<arch>`."""
        return f"PSIV-{version}-{self.name}-{self.arch}"

    def files(self) -> frozenset[str]:
        """What the archive holds below its folder, files only."""
        papers = {*PAPERS, *NOTICE_FILES}
        if self.name == "macos":
            return frozenset({*papers, *APP_FILES, APP_EXECUTABLE,
                              f"{APP_BUILDER_DIRECTORY}/{self.builder}"})
        return frozenset({*papers, self.executable, self.library, self.builder})

    def programs(self) -> frozenset[str]:
        """The archive's files that must stay executable (none on Windows)."""
        if self.name == "windows":
            return frozenset()
        if self.name == "macos":
            return frozenset({APP_EXECUTABLE, f"{APP_BUILDER_DIRECTORY}/{self.builder}"})
        return frozenset({self.executable, self.builder})


PLATFORMS = {
    "linux": Platform("linux", ("x86_64-unknown-linux-gnu",), "libpsiv_godot.so",
                      "psiv-pack", "Linux", "psiv.x86_64", "linux_release.x86_64",
                      "psiv.x86_64", "x86_64", ".tar.gz"),
    "windows": Platform("windows", ("x86_64-pc-windows-msvc",), "psiv_godot.dll",
                        "psiv-pack.exe", "Windows", "PSIV.exe", "windows_release_x86_64.exe",
                        "PSIV.exe", "x86_64", ".zip", ("-C", "target-feature=+crt-static")),
    "macos": Platform("macos", ("aarch64-apple-darwin", "x86_64-apple-darwin"),
                      "libpsiv_godot.dylib", "psiv-pack", "macOS", "PSIV.zip", "macos.zip",
                      APP_EXECUTABLE, "universal", ".zip"),
}


def run(command: list[str], **options) -> subprocess.CompletedProcess:
    """Run `command`, raising `ReleaseError` with its output if it fails."""
    result = subprocess.run(command, capture_output=True, text=True, encoding="utf-8",
                            errors="replace", check=False, **options)
    if result.returncode != 0:
        output = (result.stdout + result.stderr).strip().splitlines()
        raise ReleaseError(f"{' '.join(command[:4])} ... exited {result.returncode}:\n"
                           + "\n".join(output[-40:]))
    return result


# --- build -----------------------------------------------------------------

def target_directory() -> Path:
    return Path(os.environ.get("CARGO_TARGET_DIR") or ROOT / "rust" / "target")


def build(platform: Platform, out: Path, runner: Callable = run) -> list[Path]:
    """Build the extension and the builder for `platform` into `out`."""
    out.mkdir(parents=True, exist_ok=True)
    for target in platform.targets:
        environment = dict(os.environ)
        if platform.rustflags:
            variable = f"CARGO_TARGET_{target.upper().replace('-', '_')}_RUSTFLAGS"
            environment[variable] = " ".join(platform.rustflags)
        runner(["cargo", "build", "--release", "--locked",
                "--manifest-path", str(ROOT / "rust" / "Cargo.toml"), "--target", target,
                "-p", EXTENSION_PACKAGE, "-p", BUILDER_PACKAGE], env=environment)
    made = []
    for name in (platform.library, platform.builder):
        parts = [target_directory() / target / "release" / name for target in platform.targets]
        if len(parts) == 1:
            shutil.copy2(parts[0], out / name)
        else:
            runner(["lipo", "-create", "-output", str(out / name), *map(str, parts)])
        made.append(out / name)
    return made


# --- install-godot ---------------------------------------------------------

def sha512(path: Path) -> str:
    digest = hashlib.sha512()
    with path.open("rb") as handle:
        while chunk := handle.read(CHUNK):
            digest.update(chunk)
    return digest.hexdigest()


def fetch(url: str, expected: str, path: Path) -> Path:
    """`path`, downloaded from `url` unless it is already there, once its
    SHA-512 is `expected`. A download that doesn't match is removed."""
    if path.is_file() and sha512(path) == expected:
        return path
    import urllib.request
    path.parent.mkdir(parents=True, exist_ok=True)
    partial = path.with_name(path.name + ".part")
    digest = hashlib.sha512()
    try:
        with urllib.request.urlopen(url, timeout=DOWNLOAD_TIMEOUT) as response, \
                partial.open("wb") as handle:
            while chunk := response.read(CHUNK):
                digest.update(chunk)
                handle.write(chunk)
    except OSError as error:
        partial.unlink(missing_ok=True)
        raise ReleaseError(f"cannot download {url}: {error}") from error
    if digest.hexdigest() != expected:
        partial.unlink()
        raise ReleaseError(f"{url} has SHA-512 {digest.hexdigest()}, not the pinned {expected}")
    partial.replace(path)
    return path


def templates_directory(version_text: str) -> Path:
    """Where Godot on Linux looks for the export templates of a version
    (`version.txt`'s `4.7.1.stable`)."""
    data = os.environ.get("XDG_DATA_HOME") or str(Path.home() / ".local" / "share")
    return Path(data) / "godot" / "export_templates" / version_text


def install_godot(directory: Path, cache: Path) -> Path:
    """Download and unpack the pinned editor and templates; the editor's path."""
    if not sys.platform.startswith("linux"):
        raise ReleaseError("the release exports from Linux; install-godot runs there only")
    from tools.godot import GodotError, download
    try:
        editor, templates = download("editor"), download("templates")
    except GodotError as error:
        raise ReleaseError(str(error)) from error
    editor_zip = fetch(editor.url, editor.sha512, cache / editor.file)
    templates_zip = fetch(templates.url, templates.sha512, cache / templates.file)
    directory.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(editor_zip) as archive:
        members = [name for name in archive.namelist() if not name.endswith("/")]
        if len(members) != 1:
            raise ReleaseError(f"{editor.file} holds {len(members)} files, not the one editor")
        program = Path(archive.extract(members[0], directory))
    program.chmod(program.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    with zipfile.ZipFile(templates_zip) as archive:
        version_text = archive.read("templates/version.txt").decode("utf-8").strip()
        target = templates_directory(version_text)
        target.mkdir(parents=True, exist_ok=True)
        for name in ["version.txt", *(platform.template for platform in PLATFORMS.values())]:
            (target / name).write_bytes(archive.read(f"templates/{name}"))
    return program


# --- export ----------------------------------------------------------------

#: The SceneTree script `export` runs in the editor for Godot's own license
#: and copyright records (an exported release game runs no script).
GODOT_RECORDS_SCRIPT = Path(__file__).resolve().parent / "godot_records.gd"
RECORDS_MARK = "PSIV-GODOT-RECORDS "


def export(libraries: Path, out: Path) -> dict[str, Path]:
    """Export every platform from one release run root; their exports."""
    from tools.godot import GodotError
    try:
        return _export(libraries, out)
    except GodotError as error:
        raise ReleaseError(str(error)) from error


def _export(libraries: Path, out: Path) -> dict[str, Path]:
    from tools.godot import create_run_root, godot_command, library_path, remove_run_root, require
    require()
    linux = PLATFORMS["linux"]
    missing = [name for name in (platform.library for platform in PLATFORMS.values())
               if not (libraries / name).is_file()]
    if missing:
        raise ReleaseError(f"{libraries} lacks {', '.join(missing)}; run `build` for each platform")
    exports = {}
    with tempfile.TemporaryDirectory(prefix="psiv-export-") as scratch:
        made = create_run_root(Path(scratch) / "run", profile="release",
                               library=libraries / linux.library)
        try:
            for platform in PLATFORMS.values():
                if platform is not linux:
                    shutil.copy2(libraries / platform.library,
                                 made.library.parent / platform.library)
            # The editor that exports is a debug build of Godot and loads the
            # extension's debug entry; with the Linux library there too, the
            # export runs with the game's classes loaded.
            debug = library_path(made.root, "debug")
            debug.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(made.library, debug)
            for platform in PLATFORMS.values():
                target = out / platform.name / platform.export
                target.parent.mkdir(parents=True, exist_ok=True)
                run(godot_command(made, "--headless", "--export-release", platform.preset,
                                  str(target)))
                beside = [] if platform.name == "macos" else [target.parent / platform.library]
                for made_file in (target, *beside):
                    if not made_file.is_file():
                        raise ReleaseError(f"the {platform.preset} export wrote no {made_file}")
                exports[platform.name] = target.parent
            records = run(godot_command(made, "--headless", "--script",
                                        str(GODOT_RECORDS_SCRIPT))).stdout
        finally:
            remove_run_root(made.root)
    lines = [line for line in records.splitlines() if line.startswith(RECORDS_MARK)]
    if len(lines) != 1:
        raise ReleaseError("Godot printed no license records")
    (out / "godot.json").write_text(lines[0][len(RECORDS_MARK):] + "\n", encoding="utf-8",
                                    newline="\n")
    return exports


# --- assemble --------------------------------------------------------------

def assemble(platform: Platform, export_dir: Path, build_dir: Path, notices: Path,
             version: str, out: Path) -> Path:
    """Make `platform`'s archive in `out`; its path."""
    name = platform.archive_name(version)
    out.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="psiv-assemble-") as scratch:
        folder = Path(scratch) / name
        folder.mkdir()
        if platform.name == "macos":
            if sys.platform != "darwin":
                raise ReleaseError("the macOS archive is signed and zipped on macOS")
            run(["ditto", "-x", "-k", str(export_dir / platform.export), str(folder)])
            beside = folder / APP_BUILDER_DIRECTORY
        else:
            for part in (platform.executable, platform.library):
                shutil.copy2(export_dir / part, folder / part)
            beside = folder
        shutil.copy2(build_dir / platform.builder, beside / platform.builder)
        for archived, source in PAPERS.items():
            shutil.copyfile(source, folder / archived)
        for notice in NOTICE_FILES:
            (folder / notice).parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(notices / notice, folder / notice)
        for program in platform.programs():
            path = folder / program
            path.chmod(path.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
        archive = out / f"{name}{platform.suffix}"
        archive.unlink(missing_ok=True)
        if platform.name == "macos":
            run(["codesign", "--force", "--deep", "--sign", "-", str(folder / APP)])
            run(["codesign", "--verify", "--deep", "--strict", str(folder / APP)])
            run(["ditto", "-c", "-k", "--norsrc", "--keepParent", str(folder), str(archive)])
        elif platform.suffix == ".tar.gz":
            with tarfile.open(archive, "w:gz") as handle:
                handle.add(folder, arcname=name)
        else:
            with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as handle:
                for path in sorted(folder.rglob("*")):
                    handle.write(path, Path(name, path.relative_to(folder)).as_posix())
    return archive


# --- check -----------------------------------------------------------------

def archive_files(archive: Path) -> dict[str, bool]:
    """The archive's files (not directories), each with whether it is
    executable."""
    if archive.name.endswith(".tar.gz"):
        with tarfile.open(archive) as handle:
            return {member.name: bool(member.mode & stat.S_IXUSR)
                    for member in handle.getmembers() if not member.isdir()}
    with zipfile.ZipFile(archive) as handle:
        return {entry.filename: bool((entry.external_attr >> 16) & stat.S_IXUSR)
                for entry in handle.infolist() if not entry.is_dir()}


def check(archive: Path, platform: Platform) -> list[str]:
    """What is wrong with `archive` as `platform`'s release; empty when
    nothing is."""
    name = archive.name.removesuffix(platform.suffix)
    if not name.startswith("PSIV-") or not name.endswith(f"-{platform.name}-{platform.arch}"):
        return [f"{archive.name} is not named as {platform.name}'s archive"]
    files = archive_files(archive)
    expected = {f"{name}/{part}" for part in platform.files()}
    problems = [f"unexpected: {path}" for path in sorted(set(files) - expected)]
    problems += [f"missing: {path}" for path in sorted(expected - set(files))]
    problems += [f"not executable: {name}/{program}" for program in sorted(platform.programs())
                 if files.get(f"{name}/{program}") is False]
    return problems


# --- smoke -----------------------------------------------------------------

def unpack(archive: Path, into: Path) -> None:
    if archive.name.endswith(".tar.gz"):
        with tarfile.open(archive) as handle:
            handle.extractall(into, filter="data")
    elif sys.platform == "darwin":
        run(["ditto", "-x", "-k", str(archive), str(into)])
    else:
        with zipfile.ZipFile(archive) as handle:
            handle.extractall(into)


def first_run_environment(home: Path) -> dict[str, str]:
    """The environment of a player who has never run PSIV: an empty user
    data folder on every platform (`install.rs`), and none of the variables
    that point the game elsewhere."""
    environment = {key: value for key, value in os.environ.items()
                   if not key.startswith("PSIV_")}
    environment.update(XDG_DATA_HOME=str(home / "data"), APPDATA=str(home / "AppData"),
                       HOME=str(home))
    return environment


def smoke(archive: Path, platform: Platform) -> list[str]:
    """Run the archive's game as a first run does; the lines it showed."""
    with tempfile.TemporaryDirectory(prefix="psiv-smoke-") as scratch:
        scratch_path = Path(scratch)
        unpack(archive, scratch_path / "game")
        folder = scratch_path / "game" / archive.name.removesuffix(platform.suffix)
        game = folder / platform.executable
        not_the_rom = scratch_path / NOT_THE_ROM_NAME
        not_the_rom.write_bytes(bytes(4096))
        home = scratch_path / "home"
        home.mkdir()
        shown = []
        for step, extra, wanted in (
                ("setup", [], [SETUP_SHOWN]),
                ("refusal", [str(not_the_rom)], [SETUP_SHOWN, BUILD_STARTED,
                                                 f"{NOT_THE_ROM_NAME} {NOT_THE_ROM}"])):
            log = scratch_path / f"{step}.log"
            try:
                subprocess.run([str(game), "--headless", "--max-fps", "60", "--quit-after",
                                SMOKE_FRAMES[step], "--log-file", str(log), *extra],
                               env=first_run_environment(home), capture_output=True,
                               timeout=SMOKE_TIMEOUT, check=False)
            except subprocess.TimeoutExpired as error:
                raise ReleaseError(f"the {step} run did not end in {SMOKE_TIMEOUT} s") from error
            lines = log.read_text(encoding="utf-8", errors="replace").splitlines() \
                if log.is_file() else []
            missing = [text for text in wanted if not any(text in line for line in lines)]
            if missing:
                raise ReleaseError(f"the {step} run never showed {missing}; it logged:\n"
                                   + "\n".join(lines[-40:]))
            shown += [line for line in lines if line.startswith(("pack setup:", "pack builder:"))]
        return shown


# --- command line ----------------------------------------------------------

def notices(godot_json: Path, out: Path) -> list[Path]:
    """Write the notices for every platform's build into `out`."""
    from tools.godot import GodotError, required_version
    try:
        pinned = required_version()
    except GodotError as error:
        raise ReleaseError(str(error)) from error
    try:
        return write_notices(godot_json, out, pinned=pinned,
                             targets=[target for platform in PLATFORMS.values()
                                      for target in platform.targets],
                             roots=[EXTENSION_PACKAGE, BUILDER_PACKAGE])
    except NoticeError as error:
        raise ReleaseError(str(error)) from error


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="python3 tools/package/release.py",
                                     description="Build PSIV's release archives.")
    commands = parser.add_subparsers(dest="command", required=True)
    step = commands.add_parser("build")
    step.add_argument("--platform", choices=PLATFORMS, required=True)
    step.add_argument("--out", type=Path, required=True)
    step = commands.add_parser("install-godot")
    step.add_argument("--dir", type=Path, required=True)
    step.add_argument("--cache", type=Path, default=Path(tempfile.gettempdir()) / "psiv-godot")
    step = commands.add_parser("export")
    step.add_argument("--libraries", type=Path, required=True)
    step.add_argument("--out", type=Path, required=True)
    step = commands.add_parser("notices")
    step.add_argument("--godot", type=Path, required=True)
    step.add_argument("--out", type=Path, required=True)
    step = commands.add_parser("assemble")
    step.add_argument("--platform", choices=PLATFORMS, required=True)
    step.add_argument("--export", type=Path, required=True)
    step.add_argument("--build", type=Path, required=True)
    step.add_argument("--notices", type=Path, required=True)
    step.add_argument("--version", required=True)
    step.add_argument("--out", type=Path, required=True)
    for name in ("check", "smoke"):
        step = commands.add_parser(name)
        step.add_argument("archive", type=Path)
        step.add_argument("--platform", choices=PLATFORMS, required=True)
    args = parser.parse_args(argv)
    try:
        if args.command == "build":
            for path in build(PLATFORMS[args.platform], args.out):
                print(f"release: built {path}", file=sys.stderr)
        elif args.command == "install-godot":
            print(install_godot(args.dir, args.cache))
        elif args.command == "export":
            for name, path in export(args.libraries, args.out).items():
                print(f"release: exported {name} to {path}", file=sys.stderr)
        elif args.command == "notices":
            for path in notices(args.godot, args.out):
                print(f"release: wrote {path}", file=sys.stderr)
        elif args.command == "assemble":
            archive = assemble(PLATFORMS[args.platform], args.export, args.build, args.notices,
                               args.version, args.out)
            print(archive)
        elif args.command == "check":
            problems = check(args.archive, PLATFORMS[args.platform])
            for problem in problems:
                print(f"release: {args.archive.name}: {problem}", file=sys.stderr)
            if problems:
                return 1
            print(f"release: {args.archive.name} holds exactly the release", file=sys.stderr)
        elif args.command == "smoke":
            for line in smoke(args.archive, PLATFORMS[args.platform]):
                print(f"release: {line}", file=sys.stderr)
            print(f"release: {args.archive.name} starts and runs its pack builder",
                  file=sys.stderr)
    except ReleaseError as error:
        print(f"release: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
