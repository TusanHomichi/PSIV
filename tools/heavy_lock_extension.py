#!/usr/bin/env python3
"""tools/heavy_lock_extension.py - can a command relink the GDExtension library?

`can_relink_extension(argv, cwd)` is the one owner of that decision. A cargo
command that compiles `psiv-godot` rewrites
`rust/target/<profile>/libpsiv_godot.so`, which a Godot run in the same
checkout maps, so `tools/heavy_lock.py run` refuses such a command in a shared
class and asks for `--class exclusive`. The per-checkout extension lock
(#144) reuses this function to decide which commands take that lock
exclusively.

It is conservative: it allows only what it can read with certainty, and
refuses the rest, from the command alone (nothing is run).

- Prefixes it sees through: `VAR=value` assignments; `env` (`-i`, `-u NAME`,
  `-C`/`--chdir DIR` followed as a directory change, `-S STRING` re-split);
  `exec`, `command`, `time` (`-f`/`-o` take values), `nice` (`-n N`),
  `timeout` (`-s`/`-k` take values, then the duration); and, in scripts, `!`,
  `if`, `while`, `until`, `do`, `then`, `else` and `elif`.
- Shells (`bash`, `sh`, `dash`, `zsh`, `ksh`): options are parsed up to `-c`
  (`-euo pipefail`, `-o pipefail`, `-l`, `--norc`), and the script is split on
  `&&`, `||`, `;`, `|`, `&` and newlines. A literal `cd DIR` or `pushd DIR` is
  followed and `popd` undone; a script with command substitution, a subshell
  or group, a `cd` whose target expands (`~`, `$VAR`, `-`), a variable in a
  cargo argument or a quoting error is unreadable.
- cargo: only the subcommands in `NON_LINKING` are allowed whatever they
  select; any other subcommand (build, test, run, bench, rustc, an alias from
  `--config`, a plugin such as nextest or zigbuild) is refused when its
  package selection can reach psiv-godot: a `-p`/`--package` spec matching it
  (a name, a glob such as `psiv-*`, or a package-id spec such as
  `psiv-godot@0.1.0`), `--workspace`/`--all` without an `--exclude` matching
  it, or no `-p` where the manifest (`--manifest-path`, else the nearest
  `Cargo.toml` above the directory) is psiv-godot's own or a workspace whose
  default members include it. A global option it does not know is unreadable.
- Anything else that still names `cargo` in any argument (`xargs cargo`,
  `stdbuf`, `setsid`, `ionice`, `taskset`, `python3 -c "...cargo..."`) is
  unreadable.

An unreadable command is refused with a request to state the class
explicitly. Known gap: a script file (`bash run.sh`) and `make` are not
opened, so what they run is their author's class to state; the per-checkout
extension lock (#144) is the structural fix. Stdlib only, Python 3.11+.
"""

from __future__ import annotations

import fnmatch
import os
import re
import shlex
import tomllib
from pathlib import Path

#: The package whose cdylib Godot maps.
EXTENSION_PACKAGE = "psiv-godot"
#: Cargo subcommands that never link a package: they type-check, format,
#: document, inspect, fetch or delete. Everything else may link.
NON_LINKING = {"check", "c", "clippy", "fmt", "doc", "d", "tree", "metadata", "fetch",
               "clean", "version", "help", "pkgid", "locate-project", "verify-project",
               "read-manifest", "generate-lockfile", "update", "search"}
#: Cargo options before the subcommand, with whether each takes a value.
CARGO_GLOBALS = {"-v": False, "-vv": False, "--verbose": False, "-q": False, "--quiet": False,
                 "--locked": False, "--offline": False, "--frozen": False, "--color": True,
                 "--config": True, "-Z": True, "-C": True}
SHELLS = {"bash", "sh", "dash", "zsh", "ksh"}
#: Shell options that take the next argument.
SHELL_VALUE_OPTIONS = {"--rcfile", "--init-file"}
OPERATORS = {"&&", "||", ";", "|", "&", ";;", "|&"}
#: Script keywords that run the command after them.
KEYWORDS = {"!", "if", "while", "until", "do", "then", "else", "elif"}
#: Wrapper commands: the options of theirs that take a value, and how many
#: positional arguments of their own they take before the command.
WRAPPERS = {"exec": ({"-a"}, 0), "command": (set(), 0),
            "time": ({"-f", "--format", "-o", "--output"}, 0), "nice": ({"-n", "--adjustment"}, 0),
            "timeout": ({"-s", "--signal", "-k", "--kill-after"}, 1)}
#: Subcommands whose package selection this module reads like cargo's own
#: (`-p`, `--workspace`, the manifest); for any other linking-capable one (a
#: plugin) a selection it cannot find is unreadable.
KNOWN_SELECTION = {"build", "b", "test", "t", "run", "r", "bench", "rustc"}
#: `cargo`, or a `cargo-<plugin>` binary, inside any argument.
CARGO_WORD = re.compile(r"(?<![\w-])cargo(?![\w])")


class Unreadable(ValueError):
    """A command that cannot be read with certainty."""


def can_relink_extension(argv, cwd=None):
    """Why `argv`, run in `cwd`, can relink the psiv-godot extension, or None
    when it certainly cannot. The one owner of this decision (#144 reuses it).

    A command that cannot be read with certainty answers with the reason it
    was not read: the caller refuses it rather than guess.
    """
    try:
        reason = _command(list(argv), Path(cwd or os.getcwd()))
    except Unreadable as unreadable:
        return (f"cannot tell whether this command compiles {EXTENSION_PACKAGE} "
                f"({unreadable}); state the class explicitly: --class exclusive if it "
                f"builds or tests {EXTENSION_PACKAGE}, or run its cargo commands one by one")
    if reason is None:
        return None
    return (f"{reason}, which relinks the GDExtension library a Godot run in this "
            "checkout maps; use --class exclusive")


def _command(argv, cwd):
    argv, cwd = _unwrap(argv, cwd)
    if not argv:
        return None
    name = Path(argv[0]).name
    if name in SHELLS:
        script = _shell_script(argv)
        return None if script is None else _script(script, cwd)
    if name == "cargo":
        return _cargo(argv[1:], cwd)
    if name.startswith("cargo-"):  # a plugin run directly: `cargo-nextest nextest run`
        plugin, rest = name[len("cargo-"):], argv[1:]
        return _cargo([plugin, *(rest[1:] if rest[:1] == [plugin] else rest)], cwd)
    if any(CARGO_WORD.search(arg) for arg in argv):
        raise Unreadable(f"`{name}` runs something that names cargo")
    return None


def _options(argv, valued):
    """`argv` past its leading options, skipping the value of each in `valued`."""
    while argv and argv[0].startswith("-") and argv[0] != "-":
        option = argv[0]
        if option == "--":
            return argv[1:]
        argv = argv[2:] if option in valued else argv[1:]
    return argv


def _unwrap(argv, cwd):
    """`(argv, cwd)` without the command's assignment, env and wrapper prefixes."""
    while argv:
        head = Path(argv[0]).name
        if "=" in argv[0] and argv[0].split("=", 1)[0].isidentifier():
            argv = argv[1:]
        elif argv[0] in KEYWORDS:
            argv = argv[1:]
        elif head == "env":
            argv = argv[1:]
            while argv and (argv[0].startswith("-") or "=" in argv[0]):
                option, argv = argv[0], argv[1:]
                if option in ("-C", "--chdir") or option.startswith("--chdir="):
                    target = option.split("=", 1)[1] if "=" in option else argv.pop(0)
                    cwd = _directory(cwd, target)
                elif option in ("-S", "--split-string") or option.startswith("--split-string="):
                    text = option.split("=", 1)[1] if "=" in option else argv.pop(0)
                    argv = _split(text) + argv
                elif option in ("-u", "--unset"):
                    argv = argv[1:]
                elif option == "--":
                    break
        elif head in WRAPPERS:
            valued, positional = WRAPPERS[head]
            argv = _options(argv[1:], valued)[positional:]
        else:
            return argv, cwd
    return argv, cwd


def _split(text):
    try:
        return shlex.split(text)
    except ValueError as error:
        raise Unreadable(str(error)) from None


def _directory(cwd, target):
    """`cwd` after a change to the literal directory `target`."""
    if not target or target == "-" or target.startswith("~") or "$" in target:
        raise Unreadable(f"a directory change it cannot follow ({target or 'home'})")
    return (cwd / target).resolve()


def _shell_script(argv):
    """The `-c` script of a shell command line, or None for a script file."""
    args, wants_script = argv[1:], False
    while args:
        arg = args.pop(0)
        if arg == "--":
            break
        if arg in SHELL_VALUE_OPTIONS:
            args = args[1:]
        elif arg.startswith("--"):
            continue
        elif arg[:1] in ("-", "+") and len(arg) > 1:
            letters = arg[1:]
            wants_script = wants_script or "c" in letters
            args = args[letters.count("o"):]  # each -o/+o takes an option name
        else:
            args.insert(0, arg)
            break
    if not wants_script:
        return None  # a script file, or an interactive shell: not opened
    if not args:
        raise Unreadable("-c without a script")
    return args[0]


def _script(script, cwd):
    """The first reason any command of `script` can relink, or None."""
    if "$(" in script or "`" in script or "<(" in script:
        raise Unreadable("command substitution")
    lexer = shlex.shlex(script.replace("\n", " ; "), posix=True, punctuation_chars=True)
    lexer.whitespace_split = True
    try:
        tokens = list(lexer)
    except ValueError as error:
        raise Unreadable(str(error)) from None
    segment, stack = [], []
    for token in tokens + [";"]:
        if token in ("{", "}") or token.startswith(("(", ")")):
            raise Unreadable("a subshell or group")
        if token not in OPERATORS:
            segment.append(token)
            continue
        command, segment = segment, []
        command, cwd = _unwrap(command, cwd)
        if command[:1] in (["cd"], ["pushd"]):
            if len(command) != 2:
                raise Unreadable(f"a {command[0]} it cannot follow")
            if command[0] == "pushd":
                stack.append(cwd)
            cwd = _directory(cwd, command[1])
            continue
        if command[:1] == ["popd"]:
            if not stack:
                raise Unreadable("a popd it cannot follow")
            cwd = stack.pop()
            continue
        if command and Path(command[0]).name == "cargo" and any("$" in arg for arg in command):
            raise Unreadable("a variable in a cargo command")
        reason = _command(command, cwd)
        if reason:
            return reason
    return None


def _cargo(args, cwd):
    """Why `cargo <args>` in `cwd` can relink the extension, or None."""
    if args and args[0].startswith("+"):
        args = args[1:]
    while args and args[0].startswith("-"):
        option, value = (args[0].split("=", 1) + [None])[:2]
        if option not in CARGO_GLOBALS:
            if option in ("-V", "--version", "-h", "--help", "--list"):
                return None
            raise Unreadable(f"cargo option {option} before the subcommand")
        if CARGO_GLOBALS[option] and value is None:
            if len(args) < 2:
                raise Unreadable(f"cargo {option} without a value")
            value, args = args[1], args[1:]
        if option == "-C":
            cwd = _directory(cwd, value)
        args = args[1:]
    if not args:
        return None
    subcommand, options = args[0], args[1:]
    if subcommand in NON_LINKING:
        return None
    if "--" in options:
        options = options[:options.index("--")]
    packages, excluded, manifest, workspace = [], [], None, False
    while options:
        arg = options.pop(0)
        option, value = (arg.split("=", 1) + [None])[:2] if arg.startswith("--") else (arg, None)
        if option.startswith("-p") and not option.startswith("--") and len(option) > 2:
            option, value = "-p", option[2:].lstrip("=")
        if option in ("-p", "--package", "--exclude", "--manifest-path"):
            if value is None:
                if not options:
                    raise Unreadable(f"cargo {option} without a value")
                value = options.pop(0)
            if option == "--exclude":
                excluded.append(value)
            elif option == "--manifest-path":
                manifest = value
            else:
                packages.append(value)
        elif option in ("--workspace", "--all"):
            workspace = True
    what = f"cargo {subcommand}"
    if any(_spec_matches(spec) for spec in packages):
        return f"{what} -p {' -p '.join(packages)} selects {EXTENSION_PACKAGE}"
    if packages:
        return None
    path = (cwd / manifest).resolve() if manifest else _nearest_manifest(cwd)
    if path is None or not path.is_file():
        if subcommand not in KNOWN_SELECTION:
            raise Unreadable(f"cargo {subcommand} with a selection it cannot find")
        return None
    if workspace:
        root = _workspace_root(path)
        if root is None or any(_spec_matches(spec) for spec in excluded):
            return None
        if EXTENSION_PACKAGE in _members(root):
            return f"{what} --workspace selects {EXTENSION_PACKAGE} (no --exclude)"
        return None
    if _package_name(path) == EXTENSION_PACKAGE:
        return f"{what} in {path.parent} builds {EXTENSION_PACKAGE} itself"
    if _package_name(path) is None and EXTENSION_PACKAGE in _members(path, default=True):
        return (f"{what} with no -p at the workspace root {path} builds its default "
                f"members, {EXTENSION_PACKAGE} among them")
    return None


def _spec_matches(spec):
    """Whether a `-p`/`--exclude` spec can name the extension package: a name,
    a glob, or a package-id spec (`name@version`, `name:version`,
    `url#name@version`, `url#version` with the name in the URL's path)."""
    url, _, fragment = spec.partition("#")
    candidates = [fragment or url]
    if "/" in url:
        candidates.append(url.rstrip("/").rsplit("/", 1)[-1])
    names = [re.split(r"[@:]", candidate, maxsplit=1)[0] for candidate in candidates]
    return any(fnmatch.fnmatchcase(EXTENSION_PACKAGE, name) for name in names if name)


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
