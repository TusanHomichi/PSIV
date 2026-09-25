#!/usr/bin/env python3
"""tools/gate.py - the repository's one entry point for the integration gate.

`GATE_COMMANDS` is the ordered gate: exactly the `## Run checks` block of
docs/DEVELOPMENT.md, so check and documentation cannot drift (tests/test_gate.py
fails when they do). `python3 tools/gate.py`, run from the root, executes each
string with `bash -c <string>`, one at a time, so the documented env prefixes
(`PYTHONPATH=.`, `CARGO_BUILD_JOBS=1`) apply exactly as written. A failing
command does not stop the run: one receipt shows every failure, and the exit
code is 1 when any command failed, 0 otherwise.

The run writes `build/gate/<UTC YYYYmmddTHHMMSSZ>-<short sha>/`: one log per
command plus `receipt.json`, holding the candidate SHA, whether the tree was
dirty and its `git status --porcelain` paths, and for each command its string,
UTC start and end, duration, exit code, log path and parsed counts (a
`Ran N tests`/`OK`/`FAILED (...)` summary for a unittest command, `test result:`
sums for `cargo test`; a count the run never reported is null, not zero). A gate
result is reported from that receipt.

Two guards refuse to start, both with exit 2 and nothing run: `build/gate/.lock`,
held exclusively with `fcntl.flock` for the whole run (a second gate exits at
once instead of racing it; heavy runs are serialized by project rule), and any
process with `rust/target/debug/libpsiv_godot.so` mapped - the library
godot/psiv.gdextension loads, which the cargo commands rebuild.

`--list` prints the commands and exits 0. Stdlib only, Python 3.
"""

from __future__ import annotations

import argparse
import fcntl
import json
import os
import re
import shlex
import subprocess
import sys
import time
from contextlib import contextmanager
from datetime import datetime, timezone
from pathlib import Path

EXIT_OK = 0
EXIT_FAILED = 1
EXIT_REFUSED = 2

#: docs/DEVELOPMENT.md's `## Run checks` block, in order, verbatim.
GATE_COMMANDS = [
    "python3 tools/check_docs.py",
    "PYTHONPATH=. python3 -m unittest discover -s tests",
    "cargo fmt --manifest-path rust/Cargo.toml --all --check",
    "CARGO_BUILD_JOBS=1 cargo test --manifest-path rust/Cargo.toml --workspace -- --test-threads=1",
    "CARGO_BUILD_JOBS=1 cargo clippy --manifest-path rust/Cargo.toml --workspace --all-targets -- -D warnings",
]

#: Where receipts go, relative to the root; the lock lives in this directory.
GATE_DIR = Path("build") / "gate"
LOCK_NAME = ".lock"

#: The library godot/psiv.gdextension loads, relative to the root.
EXTENSION_PATH = Path("rust") / "target" / "debug" / "libpsiv_godot.so"


class GateRefused(Exception):
    """The gate declined to start; `exit_code` is what the command line returns."""

    exit_code = EXIT_REFUSED


def utc_stamp(when):
    """`YYYYmmddTHHMMSSZ`, the UTC stamp a receipt directory is named with."""
    return when.strftime("%Y%m%dT%H%M%SZ")


def _utc(when):
    """`YYYY-MM-DDTHH:MM:SSZ`, a receipt field."""
    return when.strftime("%Y-%m-%dT%H:%M:%SZ")


def display_path(path, root):
    """`path` relative to `root` when it is inside, else the path itself."""
    path, root = Path(path), Path(root)
    return str(path.relative_to(root)) if path.is_relative_to(root) else str(path)


def _command_words(command):
    """The command's words, with leading `NAME=value` env prefixes dropped."""
    try:
        words = shlex.split(command)
    except ValueError:  # unbalanced quotes: not our command, but keep the lines
        words = command.split()
    while words and re.match(r"^[A-Za-z_][A-Za-z0-9_]*=", words[0]):
        words.pop(0)
    return words


def _operand(words, program):
    """The first non-flag word after `program` in `words`, or None."""
    tail = words[words.index(program) + 1:] if program in words else []
    return next((word for word in tail if not word.startswith("-")), None)


def command_slug(command):
    """A short file-name-safe label for `command`: `cargo-test`, `python3-unittest`."""
    words = [word for word in _command_words(command) if not word.startswith("-")]
    parts = [re.sub(r"[^A-Za-z0-9_.]+", "-", word)[:24] for word in words[:2]]
    return "-".join(part for part in parts if part) or "command"


# Ran N tests / OK / FAILED (...) from unittest, `test result:` sums from cargo.
_UNITTEST_RAN_RE = re.compile(r"^Ran (\d+) tests? in ", re.M)
_UNITTEST_STATUS_RE = re.compile(r"^(OK|FAILED)(?: \((.*)\))?[ \t\r]*$", re.M)
_UNITTEST_METRIC_RE = re.compile(r"^(failures|errors|skipped)=(\d+)$")
_CARGO_RESULT_RE = re.compile(
    r"^test result: \S+\. (\d+) passed; (\d+) failed; (\d+) ignored;", re.M)


def parse_unittest(text):
    """Counts from a unittest run: `Ran N tests` and the trailing status line.

    Skipped tests are part of N; the parenthetical on `OK`/`FAILED` names the
    failures, errors and skips, and `OK` alone means none. A number the run
    never printed is null: it died before reporting, which is not zero.
    """
    ran = _UNITTEST_RAN_RE.findall(text)
    statuses = list(_UNITTEST_STATUS_RE.finditer(text))
    counts = {"tests_run": int(ran[-1]) if ran else None,
              "failures": None, "errors": None, "skipped": None, "status": None}
    if statuses:
        last = statuses[-1]  # the runner's own line comes after anything a test printed
        counts["status"] = last.group(1)
        listed = {}
        for entry in (last.group(2) or "").split(","):
            metric = _UNITTEST_METRIC_RE.match(entry.strip())
            if metric:
                listed[metric.group(1)] = int(metric.group(2))
        # "expected failures=2" is a different metric and must not read as failures
        for metric in ("failures", "errors", "skipped"):
            counts[metric] = listed.get(metric, 0)
    return counts


def parse_cargo_test(text):
    """`passed`, `failed` and `ignored` summed over every `test result:` line.

    One line per suite: the library, each test binary and doc-tests. With no
    such line (the build failed first) the sums are null, not zero.
    """
    matches = _CARGO_RESULT_RE.findall(text)
    if not matches:
        return {"passed": None, "failed": None, "ignored": None, "suites": 0}
    totals = {"passed": 0, "failed": 0, "ignored": 0, "suites": len(matches)}
    for passed, failed, ignored in matches:
        totals["passed"] += int(passed)
        totals["failed"] += int(failed)
        totals["ignored"] += int(ignored)
    return totals


def parse_counts(command, text):
    """Parsed counts for `command`'s log text; `{}` when it has nothing to count."""
    words = _command_words(command)
    if "unittest" in words:
        return parse_unittest(text)
    if _operand(words, "cargo") == "test":
        return parse_cargo_test(text)
    return {}


def _counts_text(counts):
    """One line's worth of counts, for progress output and the summary."""
    if "tests_run" in counts:
        if counts["status"] is None:  # e.g. "NO TESTS RAN", or a run that died first
            ran = counts["tests_run"] if counts["tests_run"] is not None else "?"
            return f"Ran {ran} tests, no OK/FAILED line"
        return (f"Ran {counts['tests_run']} tests, failures={counts['failures']}, "
                f"errors={counts['errors']}, skipped={counts['skipped']}")
    if counts["passed"] is None:
        return "no test result: line"
    return (f"passed={counts['passed']}, failed={counts['failed']}, "
            f"ignored={counts['ignored']} over {counts['suites']} suites")


def _git(root, *args):
    """(exit code, stdout, stderr) of a git command run in `root`."""
    completed = subprocess.run(["git", *args], cwd=str(root), stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE, text=True)
    return completed.returncode, completed.stdout.strip(), completed.stderr.strip()


def git_info(root):
    """The candidate SHA, dirty flag and `git status --porcelain` paths of `root`.

    A root git cannot describe (a test fixture, an exported tree) still yields a
    receipt: sha null, dirty null, no paths, and the reason in `git_error`.
    """
    code, head, err = _git(root, "rev-parse", "HEAD")
    if code != 0 or not head:
        return {"candidate_sha": None, "short_sha": "nogit", "dirty": None,
                "dirty_paths": [], "git_error": err or "git cannot describe this tree"}
    _, short, _ = _git(root, "rev-parse", "--short", "HEAD")
    code, status, err = _git(root, "status", "--porcelain")
    if code != 0:
        return {"candidate_sha": head, "short_sha": short or head[:7], "dirty": None,
                "dirty_paths": [], "git_error": err or "git status failed"}
    paths = status.splitlines()
    return {"candidate_sha": head, "short_sha": short or head[:7],
            "dirty": bool(paths), "dirty_paths": paths}


def lock_path(receipt_dir):
    """The lock guarding a receipt directory's siblings: `build/gate/.lock`."""
    return Path(receipt_dir).parent / LOCK_NAME


@contextmanager
def gate_lock(lock_file):
    """Hold `lock_file` exclusively for the run; refuse when someone else does."""
    lock_file = Path(lock_file)
    lock_file.parent.mkdir(parents=True, exist_ok=True)
    handle = os.open(lock_file, os.O_RDWR | os.O_CREAT, 0o644)
    try:
        try:
            fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError as exc:
            raise GateRefused(
                f"gate refuses to start: another gate holds {lock_file} "
                f"({exc.strerror or exc}); heavy runs are serialized one at a time"
            ) from exc
        yield handle
    finally:
        os.close(handle)


def maps_path(line):
    """The file name in one /proc/<pid>/maps line, or None when there is none."""
    parts = line.split(None, 5)
    if len(parts) < 6:
        return None
    path = parts[5].strip()
    if path.endswith(" (deleted)"):
        path = path[: -len(" (deleted)")]
    return path or None


def loaded_extension_pids(extension, proc_dir=Path("/proc")):
    """PIDs with `extension` mapped, lowest first.

    The file is matched by its resolved path, because that is what a map
    records. /proc entries that cannot be read (another user's process, one
    that exited mid-scan) are skipped.
    """
    target = os.path.realpath(extension)
    pids = set()
    try:
        entries = sorted(proc_dir.iterdir())
    except OSError:
        return []
    for entry in entries:
        if not entry.name.isdigit():
            continue
        try:
            maps = (entry / "maps").read_text(errors="replace")
        except OSError:
            continue
        for line in maps.splitlines():
            path = maps_path(line)
            if path and os.path.realpath(path) == target:
                pids.add(int(entry.name))
                break
    return sorted(pids)


def _run_command(command, index, total, root, receipt_dir):
    """Run one command, log it, and return its receipt record."""
    log_file = receipt_dir / f"{index:02d}-{command_slug(command)}.log"
    print(f"[{index}/{total}] {command}", flush=True)
    started_wall = datetime.now(timezone.utc)
    started = time.monotonic()
    with log_file.open("wb") as handle:
        completed = subprocess.run(["bash", "-c", command], cwd=str(root),
                                   stdout=handle, stderr=subprocess.STDOUT)
    ended_wall = datetime.now(timezone.utc)
    duration = round(time.monotonic() - started, 3)
    counts = parse_counts(command, log_file.read_text(errors="replace"))
    print(f"[{index}/{total}] exit {completed.returncode} in {duration:.1f}s -> {log_file.name}"
          + (f"; {_counts_text(counts)}" if counts else ""), flush=True)
    return {
        "command": command,
        "start_utc": _utc(started_wall),
        "end_utc": _utc(ended_wall),
        "duration_s": duration,
        "exit_code": completed.returncode,
        "log": display_path(log_file, root),
        "counts": counts,
    }


def _summary(records, receipt_dir, root):
    """The one line printed at the end and stored as the receipt's `summary`."""
    total = len(records)
    failed = [record for record in records if record["exit_code"] != 0]
    seconds = sum(record["duration_s"] for record in records)
    if failed:
        names = ", ".join(f"{Path(record['log']).stem} exit={record['exit_code']}"
                          for record in failed)
        head = f"gate FAILED: {len(failed)}/{total} commands failed in {seconds:.1f}s ({names})"
    else:
        head = f"gate OK: {total}/{total} commands passed in {seconds:.1f}s"
    counts = "; ".join(f"{Path(record['log']).stem} {_counts_text(record['counts'])}"
                       for record in records if record["counts"])
    tail = f"; receipt={display_path(receipt_dir, root)}"
    return "; ".join(part for part in (head, counts) if part) + tail


def run_gate(commands, root, receipt_dir):
    """Run `commands` in order under `root`, writing a receipt into `receipt_dir`.

    Returns the receipt, whose `exit_code` is 0 when every command exited 0 and
    1 otherwise; the summary line is printed and stored. Raises GateRefused
    (exit 2) without running anything when another gate holds the lock or a
    process has the debug extension mapped.
    """
    root = Path(root).resolve()
    receipt_dir = Path(receipt_dir).resolve()
    lock_file = lock_path(receipt_dir)
    with gate_lock(lock_file):
        extension = root / EXTENSION_PATH
        loaded = loaded_extension_pids(extension)
        if loaded:
            holders = ", ".join(str(pid) for pid in loaded)
            raise GateRefused(
                f"gate refuses to start: {extension} is loaded by PID(s) {holders}; "
                "close them first, because the cargo commands rebuild that library"
            )
        info = git_info(root)
        started = datetime.now(timezone.utc)
        receipt_dir.mkdir(parents=True, exist_ok=True)
        print(f"gate: {len(commands)} commands -> {display_path(receipt_dir, root)}",
              flush=True)
        records = [_run_command(command, index, len(commands), root, receipt_dir)
                   for index, command in enumerate(commands, start=1)]
        finished = datetime.now(timezone.utc)
        receipt = {
            "candidate_sha": info["candidate_sha"],
            "short_sha": info["short_sha"],
            "dirty": info["dirty"],
            "dirty_paths": info["dirty_paths"],
            "started_utc": _utc(started),
            "finished_utc": _utc(finished),
            "duration_s": round(sum(r["duration_s"] for r in records), 3),
            "exit_code": EXIT_FAILED if any(r["exit_code"] != 0 for r in records) else EXIT_OK,
            "receipt_dir": display_path(receipt_dir, root),
            "commands": records,
            "summary": _summary(records, receipt_dir, root),
        }
        if "git_error" in info:
            receipt["git_error"] = info["git_error"]
        (receipt_dir / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
        print(receipt["summary"], flush=True)
        return receipt


def main(argv=None):
    """`python3 tools/gate.py [--list]`, run from the repository root."""
    parser = argparse.ArgumentParser(
        prog="python3 tools/gate.py",
        description="Run the repository gate and write its receipt under build/gate/.")
    parser.add_argument("--list", action="store_true",
                        help="print the gate commands and exit without running them")
    args = parser.parse_args(argv)
    if args.list:
        for command in GATE_COMMANDS:
            print(command)
        return EXIT_OK
    root = Path.cwd()
    # The directory is named for the candidate; run_gate re-reads git for the receipt.
    stamp = utc_stamp(datetime.now(timezone.utc))
    receipt_dir = root / GATE_DIR / f"{stamp}-{git_info(root)['short_sha']}"
    try:
        receipt = run_gate(GATE_COMMANDS, root, receipt_dir)
    except GateRefused as refusal:
        print(str(refusal), file=sys.stderr)
        return EXIT_REFUSED
    return receipt["exit_code"]


if __name__ == "__main__":
    sys.exit(main())
