#!/usr/bin/env python3
"""tools/heavy_lock_extension.py - can a command relink the GDExtension library?

`can_relink_extension(argv, cwd)` is the one owner of that decision. A cargo
build, test, run or bench that compiles `psiv-godot` rewrites
`rust/target/<profile>/libpsiv_godot.so`, which a Godot run in the same
checkout maps, so `tools/heavy_lock.py run` refuses such a command in a shared
class and asks for `--class exclusive`. The per-checkout extension lock
(#144) reuses this function to decide which commands take that lock
exclusively.

What it reads, from the command alone (nothing is run):

- a cargo invocation, after `env [-i] [-u NAME] [VAR=value ...]`, `exec`,
  `command`, `time`, `nice [-n N]` or `timeout [opts] DURATION` prefixes and
  shell `VAR=value` assignments;
- `bash -c`/`sh -c` scripts, split on `&&`, `||`, `;`, `|`, `&` and newlines,
  with a literal `cd DIR` followed; a script it cannot read with certainty
  (command substitution, a subshell, a variable in a cargo argument, a
  quoting error) is not guessed at: the answer is a refusal asking for an
  explicit class;
- cargo's subcommand: `build`/`b`, `test`/`t`, `run`/`r` and `bench` link;
  `check`, `clippy` and every other subcommand do not;
- the packages it selects: `-p psiv-godot`, `--workspace`/`--all` without
  `--exclude psiv-godot`, or no `-p` where the manifest (`--manifest-path`,
  else the nearest `Cargo.toml` above the directory) is psiv-godot's own or a
  workspace whose default members include it.

A script file (`bash run.sh`) is not opened: what it runs is its author's
class to state. Stdlib only, Python 3.11+.
"""

from __future__ import annotations

import os
import shlex
import tomllib
from pathlib import Path

#: The package whose cdylib Godot maps.
EXTENSION_PACKAGE = "psiv-godot"
#: Cargo subcommands (and their aliases) that compile and link the package.
LINKING = {"build", "b", "test", "t", "run", "r", "bench"}
#: Command prefixes that run the rest of the line, with how many arguments of
#: their own they take after any options.
WRAPPERS = {"exec": 0, "command": 0, "time": 0, "nice": 0, "timeout": 1}
SHELLS = {"bash", "sh"}
OPERATORS = {"&&", "||", ";", "|", "&", ";;", "|&"}


class Unreadable(ValueError):
    """A shell script whose commands cannot be read with certainty."""


def can_relink_extension(argv, cwd=None):
    """Why `argv`, run in `cwd`, can relink the psiv-godot extension, or None
    when it cannot. The one owner of this decision (#144 reuses it).

    A `bash -c` script that cannot be read with certainty answers with the
    reason it was not read: the caller refuses it rather than guess.
    """
    try:
        reason = _command(list(argv), Path(cwd or os.getcwd()))
    except Unreadable as unreadable:
        return (f"cannot tell whether this shell script compiles {EXTENSION_PACKAGE} "
                f"({unreadable}); state the class explicitly: --class exclusive if it "
                f"builds or tests {EXTENSION_PACKAGE}, or run its cargo commands one by one")
    if reason is None:
        return None
    return (f"{reason}, which relinks the GDExtension library a Godot run in this "
            "checkout maps; use --class exclusive")


def _command(argv, cwd):
    argv = _unwrap(argv)
    if not argv:
        return None
    name = Path(argv[0]).name
    if name in SHELLS:
        script = _shell_script(argv)
        return None if script is None else _script(script, cwd)
    if name == "cargo":
        return _cargo(argv[1:], cwd)
    return None


def _unwrap(argv):
    """`argv` without its env, assignment and wrapper prefixes."""
    while argv:
        head = Path(argv[0]).name
        if "=" in argv[0] and not argv[0].startswith("-") and argv[0].split("=")[0].isidentifier():
            argv = argv[1:]
        elif head == "env":
            argv = argv[1:]
            while argv and (argv[0].startswith("-") or "=" in argv[0]):
                takes = argv[0] in ("-u", "--unset", "-C", "--chdir", "-S")
                argv = argv[2:] if takes else argv[1:]
        elif head in WRAPPERS:
            argv = argv[1:]
            while argv and argv[0].startswith("-"):
                takes = argv[0] in ("-n", "-s", "-k", "--signal", "--kill-after")
                argv = argv[2:] if takes else argv[1:]
            argv = argv[WRAPPERS[head]:]
        else:
            return argv
    return argv


def _shell_script(argv):
    """The `-c` script of a shell command line, or None for a script file."""
    for index, arg in enumerate(argv[1:], start=1):
        if arg.startswith("-") and not arg.startswith("--") and "c" in arg:
            if index + 1 >= len(argv):
                raise Unreadable("-c without a script")
            return argv[index + 1]
        if not arg.startswith("-"):
            return None
    return None


def _script(script, cwd):
    """The first reason any command of `script` can relink, or None."""
    if "$(" in script or "`" in script:
        raise Unreadable("command substitution")
    lexer = shlex.shlex(script.replace("\n", " ; "), posix=True, punctuation_chars=True)
    lexer.whitespace_split = True
    try:
        tokens = list(lexer)
    except ValueError as error:
        raise Unreadable(str(error)) from None
    segment = []
    for token in tokens + [";"]:
        if token in ("(", ")", "{", "}") or token.startswith(("(", ")")):
            raise Unreadable("a subshell or group")
        if token not in OPERATORS:
            segment.append(token)
            continue
        command, segment = _unwrap(segment), []
        if command[:1] == ["cd"]:
            if len(command) != 2 or "$" in command[1]:
                raise Unreadable("a cd it cannot follow")
            cwd = (cwd / command[1]).resolve()
            continue
        if command and Path(command[0]).name == "cargo" \
                and any("$" in arg for arg in command):
            raise Unreadable("a variable in a cargo command")
        reason = _command(command, cwd)
        if reason:
            return reason
    return None


def _cargo(args, cwd):
    """Why `cargo <args>` in `cwd` can relink the extension, or None."""
    if args and args[0].startswith("+"):
        args = args[1:]
    globals_with_value = {"--config", "-Z", "-C", "--color"}
    index = 0
    while index < len(args) and args[index].startswith("-"):
        if args[index] == "-C" and index + 1 < len(args):
            cwd = (cwd / args[index + 1]).resolve()
        index += 2 if args[index] in globals_with_value else 1
    if index >= len(args) or args[index] not in LINKING:
        return None
    subcommand = args[index]
    options = args[index + 1:]
    if "--" in options:
        options = options[:options.index("--")]
    packages, excluded, manifest, workspace = [], [], None, False
    index = 0
    while index < len(options):
        arg = options[index]
        value = options[index + 1] if index + 1 < len(options) else None
        for flag, into in (("-p", packages), ("--package", packages), ("--exclude", excluded)):
            if arg == flag and value is not None:
                into.append(value)
                index += 1
            elif arg.startswith(flag + "=") or (flag == "-p" and arg.startswith("-p")
                                                 and len(arg) > 2 and arg[2] != "="):
                into.append(arg.split("=", 1)[1] if "=" in arg else arg[2:])
        if arg == "--manifest-path" and value is not None:
            manifest = value
            index += 1
        elif arg.startswith("--manifest-path="):
            manifest = arg.split("=", 1)[1]
        elif arg in ("--workspace", "--all"):
            workspace = True
        index += 1
    what = f"cargo {subcommand}"
    if EXTENSION_PACKAGE in packages:
        return f"{what} -p {EXTENSION_PACKAGE} compiles {EXTENSION_PACKAGE}"
    if packages:
        return None
    path = (cwd / manifest).resolve() if manifest else _nearest_manifest(cwd)
    if path is None or not path.is_file():
        return None
    root = _workspace_root(path)
    if workspace:
        if EXTENSION_PACKAGE in excluded or root is None:
            return None
        if EXTENSION_PACKAGE in _members(root):
            return f"{what} --workspace compiles {EXTENSION_PACKAGE} (no --exclude)"
        return None
    if _package_name(path) == EXTENSION_PACKAGE:
        return f"{what} in {path.parent} builds {EXTENSION_PACKAGE} itself"
    if _package_name(path) is None and EXTENSION_PACKAGE in _members(path, default=True):
        return (f"{what} with no -p at the workspace root {path} builds its default "
                f"members, {EXTENSION_PACKAGE} among them")
    return None


def _toml(path):
    try:
        return tomllib.loads(Path(path).read_text())
    except (OSError, tomllib.TOMLDecodeError):
        return {}


def _package_name(manifest):
    return _toml(manifest).get("package", {}).get("name")


def _members(manifest, default=False):
    """Package names of a workspace manifest's members (its `default-members`
    when `default` and it has them), read from each member's Cargo.toml."""
    workspace = _toml(manifest).get("workspace", {})
    paths = workspace.get("default-members") if default else None
    names = []
    for member in paths or workspace.get("members", []):
        name = _package_name(Path(manifest).parent / member / "Cargo.toml")
        names.append(name or Path(member).name)
    return names


def _nearest_manifest(cwd):
    for directory in (cwd, *cwd.parents):
        if (directory / "Cargo.toml").is_file():
            return directory / "Cargo.toml"
    return None


def _workspace_root(manifest):
    """The workspace manifest `manifest` belongs to (itself when it has one)."""
    for directory in (manifest.parent, *manifest.parent.parents):
        candidate = directory / "Cargo.toml"
        if candidate.is_file() and "workspace" in _toml(candidate):
            return candidate
    return None
