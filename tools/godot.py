#!/usr/bin/env python3
"""The Godot executable PSIV runs: one resolver for every launcher and verifier.

    python3 tools/godot.py

prints the executable's path, or the reason there is none on stderr with exit
status 1. `PSIV_GODOT` names the executable; when it is unset or empty, `godot`
on `PATH` is used. Either way the executable must report the version that
`.godot-version` at the repository root pins (`godot --version` prints
`4.7.1.stable.official.<commit>`) and be a `stable` build, so a different
Godot, or a release candidate of the pinned one, is refused rather than run.
That file is the one place the version is written, so changing the version is
a one-line edit there. `python3 tools/godot.py check` checks the same without
printing the path.

    python3 tools/godot.py prepare

prepares the Godot project for its first run. Godot finds the game's
GDExtension through `godot/.godot/extension_list.cfg`, which its editor
import writes and Git ignores, so a fresh clone has none: the game then
starts on a black screen with "Cannot get class 'Field'". `prepare` writes
that index, one line naming `res://psiv.gdextension`, beside any extension it
already lists; when the game's extension is listed it does nothing. Every
launcher and verifier calls it before starting the game.

It writes the file rather than running the editor import
(`--headless --editor --quit`) because, on a fresh project whose extension is
already built (a clone after `cargo build`), the pinned Godot's headless
editor aborts at exit, after writing the index. The game needs nothing else
from the import: the project has no assets to import, and the title, a battle
and the camp menu capture identically from an index-only project and a fully
imported one.

    python3 tools/godot.py run [--dir DIR [--replace]] [--keep] [--profile P]
        [--library FILE] [-- <godot arguments>]

runs the game from a fresh run root (`create_run_root`), removed when Godot
exits unless `--keep`; environment variables (`PSIV_SAVE_DIR`, the debug
selectors) pass through. Every Godot run a repository tool starts, and every
documented launch, runs from a run root, never from the checkout's `godot/`,
and its command line comes from `godot_command`, the one argv builder, because the project loads its extension
from `res://../rust/target/<profile>/libpsiv_godot.so`: from the checkout,
that is the file every `cargo build` of psiv-godot relinks. A run root holds

- `godot/`: a copy of the project's files (`repo_files`), prepared, and with
  `projects=N`, `godot-1/` ... `godot-<N-1>/` beside it, one per worker;
- `rust/target/<profile>/libpsiv_godot.so`: a reflink of the built library,
  or a plain copy where the filesystem cannot reflink, never a hard link
  (cargo's own uplift hard-links that name), read-only; every project
  resolves the same `res://../` to it, so all of one run's workers load one
  snapshot;
- `runtime-pack`: a symbolic link to the pack's real path, resolved once at
  creation, so a pack swapped in while the run lasts (`psiv_tools pack`) is
  not seen half-way;
- `run-root.json`: what it holds, with the library's sha256.

So a build can relink the checkout's library while a run lasts, and the run
records exactly which library it ran. A non-editor Godot never re-reads a
loaded extension: Godot 4.7.1 enables extension reloading only with
`--editor` (main/main.cpp, `set_extension_reloading_enabled`), and only the
editor's focus-in calls `GDExtensionManager::reload_extensions`. The owner
removes the bulk when the run ends (`RunRoot.release`), keeping
`run-root.json`; `run` removes its run root when Godot exits.

Stdlib only, Python 3.
"""

from __future__ import annotations

import argparse
import fcntl
import hashlib
import json
import os
import re
import shutil
import signal
import stat
import subprocess
import sys
import time
from collections.abc import Callable, Mapping
from dataclasses import dataclass
from pathlib import Path

try:  # imported as `tools.godot` by tests and verifiers
    from tools.repo_files import RepoFilesError, repo_files
except ModuleNotFoundError:  # direct `python3 tools/godot.py`, or `from godot import`
    from repo_files import RepoFilesError, repo_files

ROOT = Path(__file__).resolve().parent.parent

#: The pinned version, one line such as `4.7.1`.
VERSION_FILE = ROOT / ".godot-version"

#: The environment variable that names the executable.
ENV = "PSIV_GODOT"

#: Seconds `godot --version` may take.
VERSION_TIMEOUT = 30

#: The Godot project, the extension index its import writes, and the entry
#: that index must hold for the game's classes to load.
PROJECT = ROOT / "godot"
EXTENSION_INDEX = Path(".godot/extension_list.cfg")
EXTENSION_ENTRY = "res://psiv.gdextension"

#: The library the project's `psiv.gdextension` loads, per build profile,
#: relative to the directory that holds the project.
LIBRARY_NAME = "libpsiv_godot.so"
PROFILES = ("debug", "release")

#: A run root's record of itself, and the pack link inside it.
RUN_ROOT_RECORD = "run-root.json"
RUN_ROOT_PACK = "runtime-pack"

#: Linux FICLONE: make the destination share the source's extents.
FICLONE = 0x40049409

_VERSION_RE = re.compile(r"\A\d+\.\d+(?:\.\d+)?\Z")


class GodotError(RuntimeError):
    """No usable Godot: none named or found, not runnable, or the wrong version."""


def required_version(path: Path = VERSION_FILE) -> str:
    """The version `.godot-version` pins, such as `4.7.1`."""
    try:
        text = path.read_text(encoding="utf-8").strip()
    except OSError as error:
        raise GodotError(f"cannot read the pinned Godot version: {error}") from error
    if not _VERSION_RE.match(text):
        raise GodotError(f"{path} must hold one version such as 4.7.1, not {text!r}")
    return text


def reported_version(output: str) -> tuple[str, str] | None:
    """The version and build status in `godot --version` output: its leading
    numeric fields and the field after them.

    `4.7.1.stable.official.a13da4feb` is (4.7.1, stable); a `.0` patch release
    prints as `4.7.stable...`, which is 4.7; a release candidate prints
    `4.7.1.rc1.official...`, status `rc1`.
    """
    words = output.strip().split()
    if not words:
        return None
    fields = words[0].split(".")
    numbers = []
    for field in fields:
        if not field.isdigit():
            break
        numbers.append(field)
    if len(numbers) < 2:
        return None
    status = fields[len(numbers)] if len(fields) > len(numbers) else ""
    return ".".join(numbers), status


def same_version(left: str, right: str) -> bool:
    """True when two versions name the same release (`4.7` is `4.7.0`)."""
    def fields(version: str) -> list[int]:
        values = [int(part) for part in version.split(".")]
        return values + [0] * (3 - len(values))
    return fields(left) == fields(right)


def find(env: Mapping[str, str] | None = None,
         which: Callable[..., str | None] = shutil.which) -> Path | None:
    """The executable `PSIV_GODOT` names, else `godot` on PATH, unchecked.

    Cheap and silent, so a tool can look it up at import; `check` it before
    running it. None when neither names one.
    """
    env = os.environ if env is None else env
    named = env.get(ENV, "")
    if named:
        return Path(named).expanduser()
    found = which("godot", path=env.get("PATH"))
    return Path(found) if found else None


def check(path: Path | None, version_file: Path = VERSION_FILE) -> Path:
    """`path`, once it is an executable reporting the pinned version.

    Raises `GodotError` naming the cause otherwise.
    """
    required = required_version(version_file)
    if path is None:
        raise GodotError(f"no Godot: set {ENV} to the Godot {required} executable, "
                         "or put it on PATH as `godot`")
    if not (path.is_file() and os.access(path, os.X_OK)):
        raise GodotError(f"{path} is not an executable file ({ENV}, or `godot` on PATH)")
    try:
        proc = subprocess.run([str(path), "--version"], capture_output=True, text=True,
                              timeout=VERSION_TIMEOUT, check=False)
    except (OSError, subprocess.TimeoutExpired) as error:
        raise GodotError(f"cannot run {path} --version: {error}") from error
    reported = reported_version(proc.stdout)
    if proc.returncode != 0 or reported is None:
        raise GodotError(f"{path} --version reported no version (exit {proc.returncode})")
    version, status = reported
    if status != "stable":
        raise GodotError(f"{path} is Godot {version} {status or 'with no build status'}; "
                         f"PSIV needs the stable {required} release ({version_file.name})")
    if not same_version(version, required):
        raise GodotError(f"{path} is Godot {version}; PSIV needs {required} "
                         f"({version_file.name})")
    return path


def resolve(env: Mapping[str, str] | None = None,
            which: Callable[..., str | None] = shutil.which,
            version_file: Path = VERSION_FILE) -> Path:
    """The Godot executable to run: `find`, then `check`."""
    return check(find(env, which), version_file)


def prepared(project: Path = PROJECT) -> bool:
    """True when `project`'s extension index lists the game's extension."""
    try:
        return EXTENSION_ENTRY in (project / EXTENSION_INDEX).read_text(encoding="utf-8")
    except OSError:
        return False


def prepare(project: Path = PROJECT) -> bool:
    """Make `project` ready to run: list the game's extension in its index.

    Returns True when it wrote the index and False when the extension was
    already listed. Other extensions the index lists are kept.
    """
    if prepared(project):
        return False
    index = project / EXTENSION_INDEX
    try:
        kept = [line for line in index.read_text(encoding="utf-8").splitlines() if line.strip()]
    except OSError:
        kept = []
    try:
        index.parent.mkdir(parents=True, exist_ok=True)
        index.write_text("".join(f"{line}\n" for line in [*kept, EXTENSION_ENTRY]),
                         encoding="utf-8")
    except OSError as error:
        raise GodotError(f"cannot write {index}: {error}") from error
    return True


def library_path(source: Path = ROOT, profile: str = "debug") -> Path:
    """The library a project beside `source`'s `rust/` loads for `profile`."""
    if profile not in PROFILES:
        raise GodotError(f"unknown build profile {profile!r}; one of {', '.join(PROFILES)}")
    return source / "rust" / "target" / profile / LIBRARY_NAME


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def snapshot_file(source: Path, target: Path) -> str:
    """Copy `source` to the new file `target`: a reflink where the filesystem
    can share extents, else a plain copy; never a hard link, so nothing that
    later writes `source` reaches `target`. Returns "reflink" or "copy"."""
    with source.open("rb") as reader, target.open("xb") as writer:
        try:
            fcntl.ioctl(writer.fileno(), FICLONE, reader.fileno())
            return "reflink"
        except OSError:
            writer.truncate(0)
            shutil.copyfileobj(reader, writer, 1 << 20)
            return "copy"


@dataclass(frozen=True)
class RunRoot:
    """A run root `create_run_root` made; `projects[0]` is the one a single
    run passes to `--path`."""

    root: Path
    projects: tuple[Path, ...]
    library: Path
    library_sha256: str
    record: dict

    @property
    def project(self) -> Path:
        return self.projects[0]

    @property
    def pack(self) -> Path:
        return self.root / RUN_ROOT_PACK

    def release(self) -> None:
        """Remove what the run root holds but its record: the projects, the
        library snapshot and the pack link. The run's evidence policy keeps
        `run-root.json` (the library's path and sha256), not the 200 MB copy."""
        remove_run_root(self.root, keep_record=True)


def create_run_root(root: Path, *, profile: str = "debug", projects: int = 1,
                    library: Path | None = None, pack: Path | None = None,
                    source: Path = ROOT) -> RunRoot:
    """Make a run root at `root`, a directory that must not exist yet.

    `library` defaults to `source`'s built library for `profile`, `pack` to
    `source`'s `runtime-pack`; a missing default pack makes no link (the game
    reports it), while a named pack must exist. Raises `GodotError`."""
    started = time.monotonic()
    if projects < 1:
        raise GodotError(f"a run root needs at least one project, not {projects}")
    built = library_path(source, profile) if library is None else Path(library)
    try:
        built = built.resolve(strict=True)
    except OSError as error:
        raise GodotError(f"no built extension at {built}: {error}; build it first") from error
    if pack is not None and not Path(pack).exists():
        raise GodotError(f"no runtime pack at {pack}")
    try:
        files = repo_files(source, ("godot/*",))
    except RepoFilesError as error:
        raise GodotError(f"cannot list the project's files: {error}") from error
    if "godot/project.godot" not in files:
        raise GodotError(f"{source / 'godot'} holds no project.godot")
    root = Path(root).absolute()
    try:
        root.mkdir(parents=True)
    except OSError as error:
        raise GodotError(f"cannot create the run root {root}: {error}") from error
    try:
        # A record first, so a run root that fails part-way is still one that
        # `remove_run_root` (and `run-root --replace`) will remove.
        (root / RUN_ROOT_RECORD).write_text(json.dumps({"root": str(root),
                                                        "incomplete": True}) + "\n")
        snapshot = library_path(root, profile)
        snapshot.parent.mkdir(parents=True)
        method = snapshot_file(built, snapshot)
        snapshot.chmod(stat.S_IRUSR | stat.S_IXUSR | stat.S_IRGRP | stat.S_IXGRP
                       | stat.S_IROTH | stat.S_IXOTH)
        digest = sha256(snapshot)
        pack_source = source / RUN_ROOT_PACK if pack is None else Path(pack)
        pack_target = (os.path.realpath(pack_source)
                       if pack is not None or pack_source.exists() else None)
        if pack_target is not None:
            (root / RUN_ROOT_PACK).symlink_to(pack_target, target_is_directory=True)
        made = []
        for index in range(projects):
            project = root / ("godot" if index == 0 else f"godot-{index}")
            for name in files:
                target = project / Path(name).relative_to("godot")
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(source / name, target, follow_symlinks=True)
            prepare(project)
            made.append(project)
    except BaseException as error:
        remove_run_root(root)
        if isinstance(error, OSError):
            raise GodotError(f"cannot create the run root {root}: {error}") from error
        raise
    record = {
        "root": str(root), "projects": [str(project) for project in made],
        "profile": profile, "library": str(snapshot), "library_sha256": digest,
        "library_source": str(built), "library_copy": method,
        "pack": str(root / RUN_ROOT_PACK) if pack_target else None,
        "pack_target": pack_target, "created_s": round(time.monotonic() - started, 3),
    }
    (root / RUN_ROOT_RECORD).write_text(json.dumps(record, indent=2, sort_keys=True) + "\n")
    return RunRoot(root, tuple(made), snapshot, digest, record)


#: What `create_run_root` puts in a run root besides its record: the project
#: copies, the library snapshot's tree and the pack link.
_RUN_ROOT_ENTRY = re.compile(r"\A(?:godot(?:-\d+)?|rust|" + re.escape(RUN_ROOT_PACK) + r")\Z")


def remove_run_root(root: Path, keep_record: bool = False) -> None:
    """Remove what `create_run_root` made at `root`: the projects, the library
    snapshot and the pack link, and (unless `keep_record`) `run-root.json` and
    the directory. Anything else in it is left, and then the directory too.
    Refuses a directory that is not a run root."""
    root = Path(root)
    if not (root / RUN_ROOT_RECORD).is_file():
        raise GodotError(f"{root} is not a run root (no {RUN_ROOT_RECORD}); not removing it")
    for entry in root.iterdir():
        if not _RUN_ROOT_ENTRY.match(entry.name):
            continue
        if entry.is_dir() and not entry.is_symlink():
            # The library snapshot is read-only; its directory is not.
            shutil.rmtree(entry)
        else:
            entry.unlink()
    if not keep_record:
        (root / RUN_ROOT_RECORD).unlink()
        left = sorted(entry.name for entry in root.iterdir())
        if left:
            raise GodotError(f"{root} holds files a run root does not make ({', '.join(left)}); "
                             "removed the run root's own, left those")
        root.rmdir()


def require() -> None:
    """Raise `GodotError` unless the pinned Godot resolves: a launcher's early
    check, before it does any work. It returns nothing, since `godot_command`
    resolves Godot itself."""
    resolve()


def godot_command(run_root: RunRoot, *args: str, project: Path | None = None,
                  prefix: tuple[str, ...] = ()) -> list[str]:
    """The one Godot command line a repository tool runs: `prefix`, the
    resolved Godot, `--path` at one of `run_root`'s projects (default the
    first), then `args`.

    Godot is resolved here, so no launcher holds its path or builds its own
    command: tests/test_run_root.py's census refuses any other code that
    resolves Godot, names `--path` or a `godot` project, or starts a bare
    `godot`. Raises `GodotError`."""
    project = run_root.project if project is None else Path(project)
    if project not in run_root.projects:
        raise GodotError(f"{project} is not a project of the run root {run_root.root}")
    return [*prefix, str(resolve()), "--path", str(project), *args]


def run_main(argv: list[str]) -> int:
    """`run`: Godot from a fresh run root, with the arguments after `--`."""
    parser = argparse.ArgumentParser(
        prog="python3 tools/godot.py run",
        description="Run the game from a fresh run root: "
                    "python3 tools/godot.py run [options] [-- <godot arguments>]")
    parser.add_argument("--dir", type=Path,
                        help="the run root (default: a new one under build/run-roots/)")
    parser.add_argument("--replace", action="store_true",
                        help="remove the run root already at --dir first")
    parser.add_argument("--keep", action="store_true",
                        help="keep the run root when Godot exits (default: remove it)")
    parser.add_argument("--profile", choices=PROFILES, default="debug")
    parser.add_argument("--library", type=Path,
                        help="the library to snapshot (default: rust/target/<profile>/)")
    own, godot_args = (argv[:argv.index("--")], argv[argv.index("--") + 1:]) \
        if "--" in argv else (argv, [])
    args = parser.parse_args(own)
    stamp = time.strftime("%Y%m%dT%H%M%SZ", time.gmtime())
    root = args.dir or ROOT / "build" / "run-roots" / f"{stamp}-{os.getpid()}"
    try:
        if args.replace and root.exists():
            remove_run_root(root)
        made = create_run_root(root, profile=args.profile, library=args.library)
        command = godot_command(made, *godot_args)
    except GodotError as error:
        print(f"godot: {error}", file=sys.stderr)
        return 1
    print(f"godot: run root {made.root}, library sha256 {made.library_sha256}", file=sys.stderr)
    process = subprocess.Popen(command)

    def forward(signum, _frame):  # Ctrl-C and a shell's TERM reach the game
        try:
            process.send_signal(signum)
        except ProcessLookupError:  # the game exited first
            pass

    forwarded = (signal.SIGINT, signal.SIGTERM, signal.SIGHUP)
    previous = {number: signal.signal(number, forward) for number in forwarded}
    code = 1
    try:
        code = process.wait()
    finally:
        for number, handler in previous.items():
            signal.signal(number, handler)
        if not args.keep:
            try:
                remove_run_root(made.root)
            except GodotError as error:  # the game's exit code still stands
                print(f"godot: could not remove {made.root}: {error}", file=sys.stderr)
    return code


def main(argv: list[str] | None = None) -> int:
    argv = sys.argv[1:] if argv is None else argv
    if argv[:1] == ["run"]:
        return run_main(argv[1:])
    if argv not in ([], ["prepare"], ["check"]):
        print("usage: python3 tools/godot.py [prepare | check | run [options] -- <args>]",
              file=sys.stderr)
        return 2
    try:
        godot = resolve()
        if argv == ["prepare"]:
            ran = prepare()
            print(f"godot: {PROJECT.name}/ {'prepared' if ran else 'already prepared'}",
                  file=sys.stderr)
        elif argv == ["check"]:
            print(f"godot: {godot} is the pinned Godot", file=sys.stderr)
        else:
            print(godot)
    except GodotError as error:
        print(f"godot: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
