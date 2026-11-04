#!/usr/bin/env python3
"""tools/gate.py - the repository's one entry point for the integration gate.

`GATE_COMMANDS` is the ordered gate: exactly the `## Run checks` block of
docs/DEVELOPMENT.md, so check and documentation cannot drift (tests/test_gate.py
fails when they do). `python3 tools/gate.py`, run from the root, executes each
string with `bash -c <string>`, one at a time, so the documented env prefix
(`PYTHONPATH=.`) applies exactly as written. Every command runs with
`CARGO_BUILD_JOBS` and `RUST_TEST_THREADS` set to the cores available, capped
at `GATE_MEASURED_PARALLELISM`, the configuration measured to fit the gate's
lock class. A failing command does not stop the run: one receipt shows every
failure, and the exit code is 1 when any command failed, 0 otherwise.

The run writes `build/gate/<UTC YYYYmmddTHHMMSSZ>-<short sha>/` (or the directory
`PSIV_GATE_RECEIPT` names, `RECEIPT_ENV`): one log per
command plus `receipt.json`, holding the candidate SHA, whether the tree was
dirty and its `git status --porcelain` paths, which of `GATE_INPUTS` were
missing (`missing_inputs`: a gate without one is partial), the cores it could use
(`cpu_count`), the `cargo_build_jobs` and `rust_test_threads` it gave cargo,
and for each command its string, UTC start and end, duration, exit code, the CPU clock
sampled every second while it ran (`cpu_mhz`: mean, min, max and samples, null
without /proc/cpuinfo) and the highest 1-minute load average seen then
(`load_1m_max`, null without /proc/loadavg), log path and parsed counts (a `Ran N tests`/`OK`/
`FAILED (...)` summary for a unittest command, `test result:` sums for
`cargo test`; a count the run never reported is null, not zero). A gate result
is reported from that receipt.

A run holds two repository-wide locks, both resolved by tools/heavy_lock.py so
every worktree shares one file each, always in this order (heavy_lock's
docstring owns it; a gate taking them the other way deadlocks against one that
does not): the gate lock, `<main checkout>/build/gate/.lock`, for the whole
run, and inside it the heavy lock in `GATE_LOCK_CLASS`, exclusive, through
`hold_heavy_lock` for every command (the gate's receipt is a timed,
reproducible run, so nothing shares the machine with it; the receipt names the class at the top and on each command; a run under
`heavy_lock.py run --class exclusive` already holds it, so that step is a no-op there; the
full gate queues ahead of every other waiter, so a pull request's gate never waits behind
queued lane work, and `PSIV_LOCK_PRIORITY=1` queues the fast tier as a priority waiter). A gate nested
under a heavy holder takes the gate lock without waiting, `--wait` or not: the
wait would hold the heavy lock while waiting for the gate lock.
Another gate holding the gate lock refuses the run with exit 2 and nothing
run (a second gate exits at once instead of racing it; `--wait` waits for it
instead, naming the holder, and says how long it has waited every
heavy_lock `WAIT_REPORT_S`). A Godot that has the checkout's
`rust/target/debug/libpsiv_godot.so` mapped does not: cargo replaces that file
with a new one rather than writing it in place, a non-editor Godot never
re-reads it, the editor reloads it on focus by design, and every Godot a
repository tool starts loads its own copy (tools/godot.py `create_run_root`).
A raw `flock(1)` wrapper around the gate is refused too (heavy_lock exits 2
rather than wait on its own parent).

Before it takes a lock, the gate checks that the environment its commands
would run in has the toolchain they need (`toolchain_missing`: the Python
packages and Rust toolchain of docs/DEVELOPMENT.md, "Current environment",
each run, not only found).
Without it, the gate exits 126 (`EXIT_UNEQUIPPED`) with nothing run, naming
what is missing: the machine failing, not the commit, which
tools/pr_gate.py passes through for its runner to try again, where every
command failing for it would be a red verdict on the commit.

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
import shutil
import subprocess
import sys
import time
from contextlib import contextmanager
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from heavy_lock import (EXCLUSIVE, HELD_ENV, RUNNER_WAIT_ENV, gate_lock_path,  # noqa: E402
                        heavy_lock_path, hold_heavy_lock, lock_holders, wait_reporting)
from tree_rss import cpu_mhz, load_1m  # noqa: E402

EXIT_OK = 0
EXIT_FAILED = 1
EXIT_REFUSED = 2
#: The environment lacks the gate's toolchain, so nothing ran: the 126 a shell
#: gives a command it can't execute, which tools/pr_gate.py and its runners
#: read as the machine failing, never as a verdict on the commit.
EXIT_UNEQUIPPED = 126

#: docs/DEVELOPMENT.md's `## Run checks` block, in order, verbatim.
#: The last gate step: the certified pixel pairs, through the dev shell because
#: that is what holds the pinned Godot and Xvfb the pairs are defined against.
#: Certify takes the heavy lock by class, a nested no-op under the gate's
#: exclusive hold, and a failing pair exits 1 (#269: camp-root sat at 5.071417
#: for three days because no merge ran it).
CERTIFY_COMMAND = "nix develop --command python3 tools/certify.py"

GATE_COMMANDS = [
    "python3 tools/check_docs.py",
    "PYTHONPATH=. python3 tools/py_suite.py",
    "cargo fmt --manifest-path rust/Cargo.toml --all --check",
    "cargo test --manifest-path rust/Cargo.toml --workspace",
    "cargo clippy --manifest-path rust/Cargo.toml --workspace --all-targets -- -D warnings",
    "python3 tools/pack_parity.py",
    CERTIFY_COMMAND,
]

#: The per-commit fast tier, `python3 tools/gate.py --fast`: docs/DEVELOPMENT.md's
#: second `## Run checks` block, in order, verbatim. It is not the integration
#: bar; the full gate above is.
FAST_COMMANDS = [
    "python3 tools/check_docs.py",
    "cargo fmt --manifest-path rust/Cargo.toml --all --check",
    "cargo clippy --manifest-path rust/Cargo.toml --workspace --all-targets -- -D warnings",
    "PYTHONPATH=. python3 tools/py_suite.py --fast",
    "cargo test --manifest-path rust/Cargo.toml --workspace",
]

#: The local inputs a full gate needs, relative to the root: the ROM, the
#: disassembly and the pack (docs/DEVELOPMENT.md, "Gate and coverage"). The
#: tests that read one skip without it, so a gate without one is partial: its
#: receipt's `missing_inputs` names each one absent, and a pull request's gate
#: counts a partial gate as failed (tools/pr_gate.py).
GATE_INPUTS = ("Phantasy Star IV (USA).md", "reference/ps4disasm", "runtime-pack")

#: What the gate's commands need from the environment they run in, beyond the
#: standard library (docs/DEVELOPMENT.md, "Current environment", whose
#: `nix develop` shell has all of it). Each Python module the commands import,
#: as the entry point that loads its native extension (Pillow's `PIL.Image`
#: loads `_imaging`; `numpy` its core), with the name it is installed as; and
#: what `cargo fmt`, `cargo test` and `cargo clippy` need, each as a command
#: that runs the toolchain they would get, rustup's shims and
#: rust-toolchain.toml included: a shim is on PATH whether or not its
#: toolchain or component is installed.
GATE_PYTHON_MODULES = {"yaml": "PyYAML", "PIL.Image": "Pillow", "numpy": "NumPy"}
GATE_RUST_PROBES = {"cargo": ("cargo", "--version"), "rustfmt": ("cargo", "fmt", "--version"),
                    "clippy": ("cargo", "clippy", "--version")}

#: Imports each module named on its command line and prints the ones that fail.
_IMPORT_PROBE = ("import importlib, sys\n"
                 "for name in sys.argv[1:]:\n"
                 "    try:\n"
                 "        importlib.import_module(name)\n"
                 "    except Exception:\n"
                 "        print(name)\n")

#: Where a caller asks for this run's receipt: a directory only this run
#: writes, which the caller names (tools/pr_gate.py, so it judges this run's
#: receipt and no other in the shared `build/gate/`). Unset, the receipt goes
#: under GATE_DIR, named for the stamp and the candidate. The commands never
#: see it.
RECEIPT_ENV = "PSIV_GATE_RECEIPT"

#: The heavy-lock class every gate command runs under. The gate's receipt is a
#: timed, reproducible run, so it holds the machine for the whole run: the one
#: exclusive job (docs/DEVELOPMENT.md, "Heavy-run lock").
GATE_LOCK_CLASS = EXCLUSIVE

#: Seconds between a command's clock samples (its receipt's `cpu_mhz`).
CPU_SAMPLE_S = 1.0

#: The most build jobs and test threads the gate gives cargo: the configuration
#: measured to fit the exclusive class, cargo's default on the 8-core host (#157,
#: 2026-10-18: a cold `cargo test --workspace --no-run` at 8 jobs peaked at
#: 4,979 MB tree RSS, 39% under the class's 8,192 MB; the built suite at 8
#: threads at 364 MB; docs/DEVELOPMENT.md, "Heavy-run lock"). Cargo's defaults
#: grow with the cores and the budget does not, so a host with more cores still
#: runs 8 until the peak is measured again at a higher count and this is raised.
GATE_MEASURED_PARALLELISM = 8

#: Where receipts go, relative to the root. The lock is shared: see heavy_lock.
GATE_DIR = Path("build") / "gate"


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
    if "unittest" in words or "tools/py_suite.py" in words:
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


def missing_inputs(root):
    """The `GATE_INPUTS` absent under `root` (a link to nothing is absent)."""
    return [relative for relative in GATE_INPUTS if not (Path(root) / relative).exists()]


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

    When someone else holds it: refuse (GateRefused), or with `wait` print a
    line naming the holder and block until it is free, saying how long it has
    waited and on whom every `WAIT_REPORT_S` (heavy_lock's `wait_reporting`).
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
            os.close(handle)
            handle = None
            handle = wait_reporting(lock_file, fcntl.LOCK_EX, sys.stdout,
                                    "PSIV's gate lock, while another gate runs")
        yield handle
    finally:
        if handle is not None:
            os.close(handle)




def _clock_summary(clocks):
    """A command's `cpu_mhz` receipt field: mean, min and max of its samples, or None."""
    if not clocks:
        return None
    return {"mean": round(sum(clocks) / len(clocks)), "min": round(min(clocks)),
            "max": round(max(clocks)), "samples": len(clocks)}


def available_cores():
    """The cores this process may run on (what cargo's defaults count), at least 1."""
    try:
        return max(1, len(os.sched_getaffinity(0)))
    except AttributeError:  # not Linux
        return max(1, os.cpu_count() or 1)


def cargo_parallelism(cores):
    """Build jobs and test threads for `cores` cores: cargo's default, capped at the measurement."""
    return max(1, min(cores, GATE_MEASURED_PARALLELISM))


def gate_environment(environ, cores):
    """`environ` with `CARGO_BUILD_JOBS` and `RUST_TEST_THREADS` set to `cargo_parallelism`,
    and without a job runner's wait directory (`RUNNER_WAIT_ENV`) or the receipt
    a caller asked for (`RECEIPT_ENV`), which is this run's alone.

    Set, not defaulted: a caller's own `CARGO_BUILD_JOBS=1` would make the run
    a different measurement from the one the receipt describes. The gate's
    commands run under its locks, so any wait in them (a test's own holder)
    is the gate's work, never a wait in line its runner should stop counting.
    """
    count = str(cargo_parallelism(cores))
    kept = {key: value for key, value in environ.items() if key not in (RUNNER_WAIT_ENV, RECEIPT_ENV)}
    return {**kept, "CARGO_BUILD_JOBS": count, "RUST_TEST_THREADS": count}


def _probe(argv, env, root):
    """`argv` run from env's PATH in `root`, or None when it isn't there or won't start
    (a missing interpreter or loader, a file the kernel can't execute)."""
    program = shutil.which(argv[0], path=env.get("PATH", ""))
    if program is None:
        return None
    try:
        return subprocess.run([program, *argv[1:]], cwd=root, env=env,
                              stdin=subprocess.DEVNULL, capture_output=True, text=True)
    except OSError:
        return None


def toolchain_missing(env, root, modules=None, rust=None):
    """What of the gate's toolchain its commands couldn't use in `env`, run from
    `root`, by name, in the order it is checked: `python3` when it isn't on
    env's PATH or won't start, else each of `modules` (default
    `GATE_PYTHON_MODULES`) it can't import, by its installed name; then each of
    `rust` (default `GATE_RUST_PROBES`) whose command doesn't exit 0. Empty
    when nothing is missing.

    Everything is run, not only found, by the programs the commands get, with
    the same environment and directory: a package built for another
    interpreter is found and still fails its import, and a rustup shim is
    found and still fails without its toolchain or component.
    """
    modules = GATE_PYTHON_MODULES if modules is None else modules
    rust = GATE_RUST_PROBES if rust is None else rust
    imported = _probe(["python3", "-c", _IMPORT_PROBE, *modules], env, root)
    if imported is None:
        missing = ["python3"]
    else:
        # A probe that dies (an extension that crashes the interpreter)
        # couldn't say which import failed, so none of them counts.
        failed = set(imported.stdout.split()) if imported.returncode == 0 else set(modules)
        missing = [installed for name, installed in modules.items() if name in failed]
    for name, argv in rust.items():
        ran = _probe(list(argv), env, root)
        if ran is None or ran.returncode != 0:
            missing.append(name)
    return missing


def _run_command(command, index, total, root, receipt_dir, env=None):
    """Run one command, log it, and return its receipt record."""
    log_file = receipt_dir / f"{index:02d}-{command_slug(command)}.log"
    print(f"[{index}/{total}] {command}", flush=True)
    started_wall = datetime.now(timezone.utc)
    started = time.monotonic()
    clocks, loads = [], []
    with log_file.open("wb") as handle:
        process = subprocess.Popen(["bash", "-c", command], cwd=str(root), env=env,
                                   stdout=handle, stderr=subprocess.STDOUT)
        try:
            while True:
                clock = cpu_mhz()
                if clock is not None:
                    clocks.append(clock)
                load = load_1m()
                if load is not None:
                    loads.append(load)
                try:
                    process.wait(timeout=CPU_SAMPLE_S)
                    break
                except subprocess.TimeoutExpired:
                    continue
        except BaseException:
            process.kill()
            process.wait()
            raise
    ended_wall = datetime.now(timezone.utc)
    duration = round(time.monotonic() - started, 3)
    counts = parse_counts(command, log_file.read_text(errors="replace"))
    print(f"[{index}/{total}] exit {process.returncode} in {duration:.1f}s -> {log_file.name}"
          + (f"; {_counts_text(counts)}" if counts else ""), flush=True)
    return {
        "command": command,
        "start_utc": _utc(started_wall),
        "end_utc": _utc(ended_wall),
        "duration_s": duration,
        "exit_code": process.returncode,
        "log": display_path(log_file, root),
        "counts": counts,
        "lock_class": GATE_LOCK_CLASS,
        "cpu_mhz": _clock_summary(clocks),
        "load_1m_max": max(loads) if loads else None,
    }


def _summary(records, receipt_dir, root, tier="full", missing=()):
    """The one line printed at the end and stored as the receipt's `summary`.

    It opens with what ran: `gate OK` / `gate FAILED` for the full gate, `fast
    tier OK` / `fast tier FAILED` for the fast tier, which is not a gate result;
    then, when an input was missing, `partial:` and which.
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
    partial = f"partial: {', '.join(missing)} missing" if missing else ""
    tail = f"; receipt={display_path(receipt_dir, root)}"
    return "; ".join(part for part in (head, partial, counts) if part) + tail


def run_gate(commands, root, receipt_dir, tier="full", wait=False, heavy_lock=None):
    """Run `commands` in order under `root`, writing a receipt into `receipt_dir`.

    Returns the receipt, whose `exit_code` is 0 when every command exited 0 and
    1 otherwise; the summary line is printed and stored. Holds `root`'s gate
    lock and heavy lock (`heavy_lock`, default `heavy_lock_path(root)`) for the
    run. Raises GateRefused (exit 2) without running anything when another gate
    holds the gate lock and `wait` is false.
    """
    root = Path(root).resolve()
    receipt_dir = Path(receipt_dir).resolve()
    lock_file = gate_lock_path(root)
    heavy_lock = heavy_lock or heavy_lock_path(root)
    # Under a heavy holder (`heavy_lock.py run --class ...`) the heavy lock
    # is already taken, so waiting here for the gate lock would invert the one
    # lock order (heavy_lock's docstring) and can deadlock: refuse instead.
    nested = os.environ.get(HELD_ENV) == str(Path(heavy_lock).resolve())
    with gate_lock(lock_file, wait and not nested), \
            hold_heavy_lock(heavy_lock, report=sys.stdout, lock_class=GATE_LOCK_CLASS,
                            gate=tier == "full"):
        info = git_info(root)
        missing = missing_inputs(root)
        started = datetime.now(timezone.utc)
        receipt_dir.mkdir(parents=True, exist_ok=True)
        print(f"gate: {len(commands)} commands -> {display_path(receipt_dir, root)}",
              flush=True)
        cores = available_cores()
        env = gate_environment(os.environ, cores)
        records = [_run_command(command, index, len(commands), root, receipt_dir, env)
                   for index, command in enumerate(commands, start=1)]
        finished = datetime.now(timezone.utc)
        receipt = {
            "candidate_sha": info["candidate_sha"],
            "short_sha": info["short_sha"],
            "dirty": info["dirty"],
            "dirty_paths": info["dirty_paths"],
            "missing_inputs": missing,
            "started_utc": _utc(started),
            "finished_utc": _utc(finished),
            "duration_s": round(sum(r["duration_s"] for r in records), 3),
            "exit_code": EXIT_FAILED if any(r["exit_code"] != 0 for r in records) else EXIT_OK,
            "receipt_dir": display_path(receipt_dir, root),
            "tier": tier,
            "heavy_lock_class": GATE_LOCK_CLASS,
            "cpu_count": cores,
            "cargo_build_jobs": int(env["CARGO_BUILD_JOBS"]),
            "rust_test_threads": int(env["RUST_TEST_THREADS"]),
            "commands": records,
            "summary": _summary(records, receipt_dir, root, tier, missing),
        }
        if "git_error" in info:
            receipt["git_error"] = info["git_error"]
        (receipt_dir / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
        print(receipt["summary"], flush=True)
        return receipt


def receipt_dir_for(root, fast, environ):
    """Where this run's receipt goes: the directory `RECEIPT_ENV` names (relative to
    the root), else one under GATE_DIR named for the stamp and the candidate (run_gate
    re-reads git for the receipt itself)."""
    if environ.get(RECEIPT_ENV):
        return Path(root) / environ[RECEIPT_ENV]
    stamp = utc_stamp(datetime.now(timezone.utc))
    suffix = "-fast" if fast else ""
    return Path(root) / GATE_DIR / f"{stamp}-{git_info(root)['short_sha']}{suffix}"


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
    args = parser.parse_args(argv)
    commands, tier = (FAST_COMMANDS, "fast") if args.fast else (GATE_COMMANDS, "full")
    if args.list:
        for command in commands:
            print(command)
        return EXIT_OK
    root = Path.cwd()
    missing = toolchain_missing(gate_environment(os.environ, available_cores()), root)
    if missing:
        print(f"gate: this environment can't run the gate, nothing was run: no {', '.join(missing)} "
              "(docs/DEVELOPMENT.md, \"Current environment\"; `nix develop` has all of it)",
              file=sys.stderr)
        return EXIT_UNEQUIPPED
    receipt_dir = receipt_dir_for(root, args.fast, os.environ)
    try:
        receipt = run_gate(commands, root, receipt_dir, tier, wait=args.wait)
    except GateRefused as refusal:
        print(str(refusal), file=sys.stderr)
        return EXIT_REFUSED
    return receipt["exit_code"]


if __name__ == "__main__":
    sys.exit(main())
