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

A run holds two repository-wide locks, both resolved by tools/heavy_lock.py so
every worktree shares one file each, always in this order (heavy_lock's
docstring owns it; a gate taking them the other way deadlocks against one that
does not): the gate lock, `<main checkout>/build/gate/.lock`, for the whole
run, and inside it the heavy lock in the exclusive class, through
`hold_heavy_lock` (heavy runs are serialized by project rule; a run under
verify_candidate already holds it, so that step is a no-op there, and
`PSIV_LOCK_PRIORITY=1` queues the gate as a priority waiter). A gate nested
under a heavy holder takes the gate lock without waiting, `--wait` or not: the
wait would hold the heavy lock while waiting for the gate lock.
Two guards refuse to start, both with exit 2 and nothing run: another gate
holding the gate lock (a second gate exits at once instead of racing it;
`--wait` waits for it instead, naming the holder once), and any process with
`rust/target/debug/libpsiv_godot.so` mapped - the library
godot/psiv.gdextension loads, which the cargo commands rebuild. A raw
`flock(1)` wrapper around the gate is refused too (heavy_lock exits 2 rather
than wait on its own parent).

`--fast` runs the per-commit fast tier (`FAST_COMMANDS`) with the same receipt,
marked `tier: "fast"`. `--list` prints the commands and exits 0. Stdlib only,
Python 3.
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
import unittest
from contextlib import contextmanager
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from heavy_lock import (HELD_ENV, gate_lock_path, heavy_lock_path,  # noqa: E402
                        hold_heavy_lock, lock_holders)

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

#: The per-commit fast tier, `python3 tools/gate.py --fast`: docs/DEVELOPMENT.md's
#: second `## Run checks` block, in order, verbatim. It is not the integration
#: bar; the full gate above is.
FAST_COMMANDS = [
    "python3 tools/check_docs.py",
    "cargo fmt --manifest-path rust/Cargo.toml --all --check",
    "CARGO_BUILD_JOBS=1 cargo clippy --manifest-path rust/Cargo.toml --workspace --all-targets -- -D warnings",
    "PYTHONPATH=. python3 tools/gate.py --quick-unittest",
    "CARGO_BUILD_JOBS=1 cargo test --manifest-path rust/Cargo.toml --workspace -- --test-threads=1",
]

#: Python test modules the fast tier leaves to the full gate (2026-10-16, CPU
#: capped at 2.3 GHz; values are seconds). First, every module that took 6 s or
#: more in one in-process run of the whole suite (202 s in all). Then a fixed
#: point over the fast tier's own run: the first module in it that builds a pack
#: pays the ~10 s one-time emit of the map-independent sections (#130), so each
#: such module at >= 6 s there was moved here in turn until the payer was
#: test_title_boot, whose shared PackFixtureCase most fast-tier modules need
#: (fast tier 77 s -> 66 s). Every name must be a module under tests/
#: (tests/test_gate.py checks), so a rename cannot leave a stale entry here.
SLOW_PYTHON_MODULES = {
    # whole suite
    "test_pack_maps": 36.5,
    "test_reload_frames": 31.4,
    "test_pack_manifest": 13.7,
    "test_battle_art_pack": 12.3,
    "test_crash_landing": 11.2,
    "test_map_patches": 11.0,
    "test_oracle_host_binary": 10.3,
    "test_sprites": 6.6,
    "test_extract_stamp": 6.6,
    # fast tier, as the first pack builder
    "test_map_effects": 12.3,
    "test_newgame": 12.5,
    "test_npc_commands": 11.9,
    "test_pack_game_start": 12.7,
    "test_pack_sprites": 15.2,
    "test_render": 12.3,
    "test_shop_flow": 13.8,
}

#: Where receipts go, relative to the root. The lock is shared: see heavy_lock.
GATE_DIR = Path("build") / "gate"

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
    if "unittest" in words or "--quick-unittest" in words:
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


@contextmanager
def gate_lock(lock_file, wait=False):
    """Hold `lock_file` exclusively for the run.

    When someone else holds it: refuse (GateRefused), or with `wait` print one
    line naming the holder and block until it is free.
    """
    lock_file = Path(lock_file)
    lock_file.parent.mkdir(parents=True, exist_ok=True)
    handle = os.open(lock_file, os.O_RDWR | os.O_CREAT, 0o644)
    try:
        try:
            fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError as exc:
            holders = ", ".join(str(pid) for pid in lock_holders(lock_file)) or "unknown"
            if not wait:
                raise GateRefused(
                    f"gate refuses to start: another gate (pid {holders}) holds {lock_file} "
                    f"({exc.strerror or exc}); heavy runs are serialized one at a time"
                ) from exc
            print(f"gate: waiting for {lock_file} held by pid {holders}", flush=True)
            fcntl.flock(handle, fcntl.LOCK_EX)
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


def _summary(records, receipt_dir, root, tier="full"):
    """The one line printed at the end and stored as the receipt's `summary`.

    It opens with what ran: `gate OK` / `gate FAILED` for the full gate, `fast
    tier OK` / `fast tier FAILED` for the fast tier, which is not a gate result.
    """
    label = "gate" if tier == "full" else f"{tier} tier"
    total = len(records)
    failed = [record for record in records if record["exit_code"] != 0]
    seconds = sum(record["duration_s"] for record in records)
    if failed:
        names = ", ".join(f"{Path(record['log']).stem} exit={record['exit_code']}"
                          for record in failed)
        head = f"{label} FAILED: {len(failed)}/{total} commands failed in {seconds:.1f}s ({names})"
    else:
        head = f"{label} OK: {total}/{total} commands passed in {seconds:.1f}s"
    counts = "; ".join(f"{Path(record['log']).stem} {_counts_text(record['counts'])}"
                       for record in records if record["counts"])
    tail = f"; receipt={display_path(receipt_dir, root)}"
    return "; ".join(part for part in (head, counts) if part) + tail


def _module_name(suite):
    """The module a discovered per-module suite came from, or None when empty."""
    for test in _flat_tests(suite):
        return test.id().split(".")[0]
    return None


def _flat_tests(suite):
    """Every test case in `suite`, depth first, in discovery order."""
    for item in suite:
        if isinstance(item, unittest.TestSuite):
            yield from _flat_tests(item)
        else:
            yield item


def quick_unittest(start="tests", excluded=SLOW_PYTHON_MODULES, stream=None):
    """The fast tier's Python suite: `unittest discover -s tests` minus `excluded`.

    Discovery is the full gate's, so a new module joins the fast tier unless
    it is named in SLOW_PYTHON_MODULES; a module that fails to import still
    reports its error here. Returns 0 when every test passed, 1 otherwise, and
    prints unittest's own `Ran N tests` / `OK` / `FAILED` lines.
    """
    # The import path `python3 -m unittest discover -s tests` gives the tests:
    # the cwd, not this script's tools/ (discover then puts the start first).
    tools_dir = str(Path(__file__).resolve().parent)
    saved = list(sys.path)
    sys.path[:] = [os.getcwd()] + [entry for entry in sys.path if entry != tools_dir]
    try:
        start = str(Path(start).resolve())
        suite = unittest.TestLoader().discover(start, top_level_dir=start)
        kept = unittest.TestSuite(module for module in suite
                                  if _module_name(module) not in excluded)
        result = unittest.TextTestRunner(stream=stream or sys.stderr).run(kept)
    finally:
        sys.path[:] = saved
    return EXIT_OK if result.wasSuccessful() else EXIT_FAILED


def run_gate(commands, root, receipt_dir, tier="full", wait=False, heavy_lock=None):
    """Run `commands` in order under `root`, writing a receipt into `receipt_dir`.

    Returns the receipt, whose `exit_code` is 0 when every command exited 0 and
    1 otherwise; the summary line is printed and stored. Holds `root`'s gate
    lock and heavy lock (`heavy_lock`, default `heavy_lock_path(root)`) for the
    run. Raises GateRefused (exit 2) without running anything when another gate
    holds the gate lock and `wait` is false, or a process has the debug
    extension mapped.
    """
    root = Path(root).resolve()
    receipt_dir = Path(receipt_dir).resolve()
    lock_file = gate_lock_path(root)
    heavy_lock = heavy_lock or heavy_lock_path(root)
    # Under a heavy holder (verify_candidate, `heavy_lock.py run`) the heavy lock
    # is already taken, so waiting here for the gate lock would invert the one
    # lock order (heavy_lock's docstring) and can deadlock: refuse instead.
    nested = os.environ.get(HELD_ENV) == str(Path(heavy_lock).resolve())
    with gate_lock(lock_file, wait and not nested), \
            hold_heavy_lock(heavy_lock, report=sys.stdout):
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
            "tier": tier,
            "commands": records,
            "summary": _summary(records, receipt_dir, root, tier),
        }
        if "git_error" in info:
            receipt["git_error"] = info["git_error"]
        (receipt_dir / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
        print(receipt["summary"], flush=True)
        return receipt


def main(argv=None):
    """`python3 tools/gate.py [--fast] [--wait] [--list]`, run from the repository root."""
    parser = argparse.ArgumentParser(
        prog="python3 tools/gate.py",
        description="Run the repository gate and write its receipt under build/gate/.")
    parser.add_argument("--fast", action="store_true",
                        help="run the per-commit fast tier instead of the full gate")
    parser.add_argument("--wait", action="store_true",
                        help="wait for a running gate to finish instead of refusing")
    parser.add_argument("--list", action="store_true",
                        help="print the commands and exit without running them")
    parser.add_argument("--quick-unittest", action="store_true",
                        help="run the fast tier's Python suite (tests/ minus "
                             "SLOW_PYTHON_MODULES) in this process; no receipt")
    args = parser.parse_args(argv)
    if args.quick_unittest:
        return quick_unittest()
    commands, tier = (FAST_COMMANDS, "fast") if args.fast else (GATE_COMMANDS, "full")
    if args.list:
        for command in commands:
            print(command)
        return EXIT_OK
    root = Path.cwd()
    # The directory is named for the candidate; run_gate re-reads git for the receipt.
    stamp = utc_stamp(datetime.now(timezone.utc))
    suffix = "-fast" if args.fast else ""
    receipt_dir = root / GATE_DIR / f"{stamp}-{git_info(root)['short_sha']}{suffix}"
    try:
        receipt = run_gate(commands, root, receipt_dir, tier, wait=args.wait)
    except GateRefused as refusal:
        print(str(refusal), file=sys.stderr)
        return EXIT_REFUSED
    return receipt["exit_code"]


if __name__ == "__main__":
    sys.exit(main())
