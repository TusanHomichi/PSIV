"""Write the third-party notices every release archive carries.

`python3 tools/package/release.py notices` calls `write_notices`, which fills
a directory with `FILES`: `THIRD-PARTY-NOTICES.txt`, naming what besides
PSIV's own code the game and its pack builder contain, who holds it and under
which license, with every license's full text; and Nuked OPN2's source, which
its license asks to travel with the game. Three parts, one section each:

1. Godot: the engine's own license and its copyright records, as the engine
   reports them (`godot_records.gd`, which `release.py export` runs). Their
   version must be the pinned one.
2. Nuked OPN2, the sound chip emulation the extension compiles in
   (`rust/psiv-sound/build.rs`), from the Genesis Plus GX checkout at the
   commit `oracle/build_core.sh` pins (`--sources` fetches it), under the
   LGPL-2.1-or-later.
3. The Rust crates linked into the extension and the builder on any release
   platform (`cargo metadata`), and the Rust standard library. A crate is
   linked when a root reaches it through normal dependencies; build scripts,
   proc-macro crates and what only they use are not. Each crate brings the
   license files it ships; one that ships none gets the text kept in
   `tools/package/licenses/` for its license, when that is a single SPDX id
   kept there, and any other crate fails the step, so no release ships a
   crate without its license.

Identical license texts are written once. Stdlib only, Python 3.
"""

from __future__ import annotations

import json
import re
import subprocess
from collections.abc import Callable, Iterable
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent

NOTICES = "THIRD-PARTY-NOTICES.txt"
NUKED_DIRECTORY = "nuked-opn2"
NUKED_NAMES = ("ym3438.c", "ym3438.h")
#: What `write_notices` puts in its directory, which every archive holds.
FILES = (NOTICES, *(f"{NUKED_DIRECTORY}/{name}" for name in NUKED_NAMES))

#: License texts for crates that ship none, by SPDX id (`<id>.txt`).
KEPT_LICENSES = Path(__file__).resolve().parent / "licenses"
NUKED_LICENSE = "LGPL-2.1-or-later"

GPGX_SCRIPT = ROOT / "oracle" / "build_core.sh"
GPGX_SOUND = ROOT / "oracle" / "gpgx-src" / "core" / "sound"
GPGX_REPOSITORY = "https://github.com/libretro/Genesis-Plus-GX"
PSIV_REPOSITORY = "https://github.com/TusanHomichi/PSIV"
CRATES_IO = "registry+https://github.com/rust-lang/crates.io-index"

#: A crate's own license files: top-level names starting with these.
LICENSE_FILE_RE = re.compile(r"\A(?:LICEN[CS]E|COPYING|COPYRIGHT|NOTICE|UNLICENSE)", re.I)
_COMMIT_RE = re.compile(r"^COMMIT=([0-9a-f]{40})$", re.M)
_COMMENT_RE = re.compile(r"/\*.*?\*/", re.S)

RULE = "=" * 78


class NoticeError(RuntimeError):
    """The notices could not be written; the message says why."""


@dataclass
class Texts:
    """License texts, each written once and cited by its number."""

    entries: list[tuple[str, str]]

    def cite(self, title: str, text: str) -> int:
        text = text.strip("\n")
        for index, (_, known) in enumerate(self.entries, start=1):
            if known == text:
                return index
        self.entries.append((title, text))
        return len(self.entries)


def _cites(numbers: Iterable[int]) -> str:
    return ", ".join(f"[{number}]" for number in sorted(set(numbers)))


# --- Godot -----------------------------------------------------------------

def godot_section(records: dict, pinned: str, texts: Texts) -> str:
    """Godot's license and copyright records; refuses another version's."""
    version = records.get("version", {})
    number = ".".join(str(version.get(part)) for part in ("major", "minor", "patch"))
    if not (number == pinned or number == f"{pinned}.0") or version.get("status") != "stable":
        raise NoticeError(f"the Godot records are {version.get('string', 'of no version')}, "
                          f"not the pinned {pinned}-stable")
    lines = [f"Godot Engine {version['string']}, https://godotengine.org", "",
             records["license"].strip("\n"), "",
             "Godot contains these components; their licenses are cited by name below.", ""]
    for component in records["copyright"]:
        lines.append(component["name"])
        for part in component["parts"]:
            lines.append(f"  Files: {', '.join(part['files'])}")
            for index, holder in enumerate(part["copyright"]):
                lines.append(f"  {'Copyright:' if index == 0 else '          '} {holder}")
            lines.append(f"  License: {part['license']}")
        lines.append("")
    cited = [f"{name} {_cites([texts.cite(name, text)])}"
             for name, text in sorted(records["licenses"].items())]
    lines.append("Godot's licenses: " + "; ".join(cited) + ".")
    return "\n".join(lines)


# --- Nuked OPN2 ------------------------------------------------------------

def pinned_gpgx(script: Path = GPGX_SCRIPT) -> str:
    match = _COMMIT_RE.search(script.read_text(encoding="utf-8"))
    if not match:
        raise NoticeError(f"{script} pins no COMMIT")
    return match.group(1)


def nuked_section(sound: Path, commit: str, texts: Texts) -> str:
    """Nuked OPN2's notice: its own header, where its source is and how to
    use a changed copy."""
    source = sound / NUKED_NAMES[0]
    if not source.is_file():
        raise NoticeError(f"no {source}; run oracle/build_core.sh --sources first")
    header = _COMMENT_RE.search(source.read_text(encoding="utf-8"))
    if not header or "Nuked OPN2" not in header.group(0):
        raise NoticeError(f"{source} does not open with Nuked OPN2's notice")
    license_text = (KEPT_LICENSES / f"{NUKED_LICENSE}.txt").read_text(encoding="utf-8")
    cite = _cites([texts.cite(NUKED_LICENSE, license_text)])
    return "\n".join([
        "Nuked OPN2, the Yamaha YM3438 emulation, is compiled into the game's",
        f"extension unchanged, from Genesis Plus GX commit {commit}",
        f"({GPGX_REPOSITORY}/tree/{commit}/core/sound). Its notice:",
        "",
        header.group(0),
        "",
        f"License: {NUKED_LICENSE} {cite}. Its source, {', '.join(NUKED_NAMES)}, is in",
        f"this archive's {NUKED_DIRECTORY} folder. To use a changed copy: PSIV's",
        f"complete source is at {PSIV_REPOSITORY}; its developer guide",
        "(docs/DEVELOPMENT.md) builds the extension, which compiles",
        "oracle/gpgx-src/core/sound/ym3438.c. Put your copy there, build, and",
        "replace this archive's extension with the one built.",
    ])


def copy_nuked(sound: Path, out: Path) -> None:
    (out / NUKED_DIRECTORY).mkdir(parents=True, exist_ok=True)
    for name in NUKED_NAMES:
        (out / NUKED_DIRECTORY / name).write_bytes((sound / name).read_bytes())


# --- Rust ------------------------------------------------------------------

def cargo_metadata(targets: Iterable[str], manifest: Path = ROOT / "rust" / "Cargo.toml",
                   runner: Callable = subprocess.run) -> dict:
    command = ["cargo", "metadata", "--locked", "--format-version", "1",
               "--manifest-path", str(manifest)]
    for target in targets:
        command += ["--filter-platform", target]
    result = runner(command, capture_output=True, text=True, check=False)
    if result.returncode != 0:
        raise NoticeError(f"cargo metadata exited {result.returncode}: {result.stderr.strip()}")
    return json.loads(result.stdout)


def linked(metadata: dict, roots: Iterable[str]) -> list[dict]:
    """The third-party packages `roots` (workspace package names) link."""
    packages = {package["id"]: package for package in metadata["packages"]}
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    members = set(metadata["workspace_members"])
    by_name = {packages[member]["name"]: member for member in members}
    missing = [root for root in roots if root not in by_name]
    if missing:
        raise NoticeError(f"the workspace has no {', '.join(missing)}")
    seen: set[str] = set()
    pending = [by_name[root] for root in roots]
    while pending:
        current = pending.pop()
        if current in seen:
            continue
        seen.add(current)
        for dependency in nodes[current]["deps"]:
            normal = any(kind["kind"] is None for kind in dependency["dep_kinds"])
            package = packages[dependency["pkg"]]
            macro = any("proc-macro" in target["kind"] for target in package["targets"])
            if normal and not macro:
                pending.append(dependency["pkg"])
    return sorted((packages[package] for package in seen - members),
                  key=lambda package: (package["name"], package["version"]))


def crate_licenses(package: dict, kept: Path = KEPT_LICENSES) -> list[tuple[str, str]]:
    """A crate's license texts, as (title, text)."""
    directory = Path(package["manifest_path"]).parent
    files = {path.resolve() for path in directory.iterdir()
             if path.is_file() and LICENSE_FILE_RE.match(path.name)}
    if package.get("license_file"):
        files.add((directory / package["license_file"]).resolve())
    if files:
        return [(path.name, path.read_text(encoding="utf-8", errors="replace"))
                for path in sorted(files)]
    license_id = package.get("license") or ""
    if (kept / f"{license_id}.txt").is_file():
        return [(license_id, (kept / f"{license_id}.txt").read_text(encoding="utf-8"))]
    raise NoticeError(f"{package['name']} {package['version']} ships no license file and its "
                      f"license, {license_id or 'none'}, has no text in {kept}")


def crate_source(package: dict) -> str:
    source = package.get("source") or ""
    if source == CRATES_IO:
        where = f"https://crates.io/crates/{package['name']}/{package['version']}"
    else:
        where = source
    repository = package.get("repository")
    return f"{where} (repository {repository})" if repository else where


def rust_section(metadata: dict, roots: Iterable[str], rustc: str, sysroot: Path,
                 texts: Texts) -> str:
    lines = ["These Rust crates are linked into the extension and psiv-pack,",
             "unchanged; each archive holds the ones its platform uses.", ""]
    for package in linked(metadata, roots):
        numbers = [texts.cite(title, text) for title, text in crate_licenses(package)]
        lines += [f"{package['name']} {package['version']}",
                  f"  License: {package.get('license') or 'see its files'} {_cites(numbers)}",
                  f"  Source: {crate_source(package)}", ""]
    standard = sysroot / "share" / "doc" / "rust" / "licenses"
    try:
        numbers = [texts.cite(name, (standard / f"{name}.txt").read_text(encoding="utf-8"))
                   for name in ("MIT", "Apache-2.0")]
    except OSError as error:
        raise NoticeError(f"the Rust toolchain's license texts are missing: {error}") from error
    lines += [f"The Rust Standard Library, {rustc}, is linked into both. Copyrights in",
              "it are retained by their contributors (The Rust Project), and it is",
              "licensed under the MIT License or the Apache License 2.0, at your option",
              f"{_cites(numbers)}: https://www.rust-lang.org/policies/licenses"]
    return "\n".join(lines)


def toolchain(runner: Callable = subprocess.run) -> tuple[str, Path]:
    """The active toolchain's `rustc --version` and sysroot."""
    answers = []
    for command in (["rustc", "--version"], ["rustc", "--print", "sysroot"]):
        result = runner(command, capture_output=True, text=True, check=False,
                        cwd=ROOT / "rust")
        if result.returncode != 0:
            raise NoticeError(f"{' '.join(command)} exited {result.returncode}")
        answers.append(result.stdout.strip())
    return answers[0], Path(answers[1])


# --- the file --------------------------------------------------------------

def render(sections: list[tuple[str, str]], texts: Texts) -> str:
    lines = ["PSIV third-party notices", "",
             "PSIV's own code is under the MIT License, in LICENSE.txt. The game and",
             "its pack builder, psiv-pack, also contain the third-party software",
             "below, each under its own license. Bracketed numbers cite the license",
             "texts at the end.", ""]
    for number, (title, body) in enumerate(sections, start=1):
        lines += [RULE, f"{number}. {title}", RULE, "", body, ""]
    lines += [RULE, f"{len(sections) + 1}. License texts", RULE, ""]
    for number, (title, text) in enumerate(texts.entries, start=1):
        lines += [f"[{number}] {title}", "-" * 78, text, ""]
    return "\n".join(lines)


def write_notices(godot_json: Path, out: Path, *, targets: Iterable[str],
                  roots: Iterable[str], pinned: str, metadata: dict | None = None,
                  rust: tuple[str, Path] | None = None, sound: Path = GPGX_SOUND,
                  commit: str | None = None) -> list[Path]:
    """Write `FILES` into `out`; their paths. `metadata`, `rust` (rustc's
    version and sysroot) and `commit` default to this checkout's."""
    records = json.loads(Path(godot_json).read_text(encoding="utf-8"))
    roots = list(roots)
    texts = Texts([])
    sections = [
        ("Godot Engine", godot_section(records, pinned, texts)),
        ("Nuked OPN2", nuked_section(sound, commit or pinned_gpgx(), texts)),
        ("Rust", rust_section(metadata if metadata is not None else cargo_metadata(targets),
                              roots, *(rust or toolchain()), texts)),
    ]
    out.mkdir(parents=True, exist_ok=True)
    (out / NOTICES).write_text(render(sections, texts), encoding="utf-8", newline="\n")
    copy_nuked(sound, out)
    return [out / name for name in FILES]
