"""Command-line parsing and presentation for heavy_lock.py."""

import argparse
import subprocess
import sys


def run_parser(lock):
    parser = argparse.ArgumentParser(
        prog="python3 tools/heavy_lock.py run",
        description="Run a command holding the heavy lock.")
    parser.add_argument("--priority", action="store_true",
                        help="go ahead of every normal waiter (speedup lanes, a yield)")
    parser.add_argument("--class", dest="lock_class", choices=lock.LOCK_CLASSES,
                        help="required; see CLASS_HELP")
    return parser


#: What `run` prints when no class is given, one line per class.
CLASS_HELP = {
    'exclusive': "the whole machine: the opt-level-3 extension build",
    'build': "a cargo build, test or clippy run that does not compile psiv-godot",
    'extension': "a cargo build, test or clippy run that compiles psiv-godot (or the workspace)",
    'godot': "one Godot/Xvfb run",
    'python': "the Python suite, or part of it",
    'pack': "a pack build with a built psiv-pack (`cargo run` is build)",
}


def class_usage(lock):
    """`run`'s usage line and one line per class with its units."""
    lines = ["usage: python3 tools/heavy_lock.py run [--priority] --class CLASS "
             "-- <command...>", "classes (docs/DEVELOPMENT.md, \"Heavy-run lock\"):"]
    for name in lock.LOCK_CLASSES:
        units = "every unit" if name == lock.EXCLUSIVE else f"{lock.CLASS_UNITS[name]} unit(s)"
        lines.append(f"  {name:<9} {units:<12} {CLASS_HELP[name]}")
    return "\n".join(lines)


def main(lock, argv=None):
    """Print a lock path or the lock's status, or `run --class CLASS -- <cmd>`
    under the heavy lock (its exit code)."""
    argv = list(sys.argv[1:] if argv is None else argv)
    if argv[:1] == ["run"]:
        options, command = (argv[1:argv.index("--")], argv[argv.index("--") + 1:]) \
            if "--" in argv else ([], argv[1:])
        args = run_parser(lock).parse_args(options)
        if not command or args.lock_class is None:
            print(class_usage(lock), file=sys.stderr)
            return 2
        with lock.hold_heavy_lock(lock_class=args.lock_class, priority=args.priority or None):
            return subprocess.run(command).returncode
    if argv[:1] == ["status"]:
        lock.status()
        return 0
    parser = argparse.ArgumentParser(
        prog="python3 tools/heavy_lock.py",
        description="Print the repository-wide heavy-run lock path; `status` for its "
                    "holders and waiters, or `run -- <command>` holding it.")
    parser.add_argument("--gate", action="store_true",
                        help="print the gate lock instead of the heavy lock")
    args = parser.parse_args(argv)
    print(lock.gate_lock_path() if args.gate else lock.heavy_lock_path())
    return 0

