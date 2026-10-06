#!/usr/bin/env python3
"""tools/verify_candidate.py - verify a frozen integration candidate end to end.

    python3 tools/verify_candidate.py [--expect-digest HEX] [--expect-chapters N]

Run from the repository root on a clean tree (the candidate). It runs, one at a
time and in this order:

1. `cargo build --release -p psiv-campaign` (`CARGO_BUILD_JOBS=2`);
2. the campaign route, `psiv-campaign run rust/psiv-campaign/routes/main.json`,
   into the receipt directory, and reads its `report.json`: the result must be
   `completed`, and `--expect-digest` / `--expect-chapters` must hold when given;
3. `python3 tools/certify.py`, the certified pixel pairs, under the shared heavy
   lock (`PSIV_HEAVY_LOCK`, default `build/continuation-heavy.lock`): Godot and
   Xvfb runs from every worktree take that lock;
4. `python3 tools/gate.py`, the integration gate.

A failing step does not stop the later ones; the exit code is 1 when any step
failed or an expectation did not hold, 0 otherwise, and 2 when the run is
refused before anything runs (not the root, or a dirty tree without
`--allow-dirty`). Everything lands in `build/candidate/<UTC stamp>-<short sha>/`:
one log per step, the route's tape, saves and report, and `receipt.json` with
the candidate SHA, each step's command, exit code and duration, the route's
digest, chapter count, frames and tape SHA-256, and the certify and gate
receipt paths their logs name. A candidate result is reported from that
receipt. Stdlib only, Python 3.
"""

from __future__ import annotations

import argparse
import fcntl
import hashlib
import json
import os
import re
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

EXIT_OK = 0
EXIT_FAILED = 1
EXIT_REFUSED = 2

ROUTE = "rust/psiv-campaign/routes/main.json"
CAMPAIGN = "rust/target/release/psiv-campaign"
CANDIDATE_DIR = Path("build") / "candidate"
DEFAULT_HEAVY_LOCK = Path("build") / "continuation-heavy.lock"


def steps(out: Path) -> list[tuple[str, str, bool]]:
    """`(name, command, needs the heavy lock)` for each step, in order."""
    route = out / "route"
    return [
        ("build", "CARGO_BUILD_JOBS=2 cargo build --release --manifest-path "
                  "rust/Cargo.toml -p psiv-campaign", False),
        ("route", f"{CAMPAIGN} run {ROUTE} --save-dir {route} --tape {route}/run.tape "
                  f"--report {route}/report.json", False),
        ("certify", "python3 tools/certify.py", True),
        ("gate", "python3 tools/gate.py", False),
    ]


def git(*args: str) -> str:
    return subprocess.run(["git", *args], capture_output=True, text=True,
                          check=True).stdout.strip()


def run_step(command: str, log: Path, lock: Path | None) -> int:
    """Run one step with `bash -c`, its output in `log`; hold `lock` if given."""
    with open(log, "w") as handle:
        if lock is None:
            return subprocess.run(["bash", "-c", command], stdout=handle,
                                  stderr=subprocess.STDOUT).returncode
        lock.parent.mkdir(parents=True, exist_ok=True)
        with open(lock, "a") as held:
            fcntl.flock(held, fcntl.LOCK_EX)
            return subprocess.run(["bash", "-c", command], stdout=handle,
                                  stderr=subprocess.STDOUT).returncode


CHAPTER_RE = re.compile(r"^chapter (?P<id>\S+): (?P<frames>\d+) frames", re.MULTILINE)


def route_facts(out: Path) -> dict:
    """The route's result and digest (its report), chapters and frames (the run's
    per-chapter lines), and the tape's SHA-256."""
    route = out / "route"
    facts: dict = {"result": None, "digest": None, "chapters": None, "frames": None,
                   "tape_sha256": None}
    report = route / "report.json"
    if report.is_file():
        data = json.loads(report.read_text())
        facts["result"] = data.get("result")
        facts["digest"] = data.get("digest")
    log = out / "route.log"
    if log.is_file():
        chapters = CHAPTER_RE.findall(log.read_text(errors="replace"))
        facts["chapters"] = len(chapters)
        facts["frames"] = sum(int(frames) for _, frames in chapters)
    tape = route / "run.tape"
    if tape.is_file():
        facts["tape_sha256"] = hashlib.sha256(tape.read_bytes()).hexdigest()
    return facts


def expectation_failures(facts: dict, digest: str | None, chapters: int | None) -> list[str]:
    """What the route's facts contradict, one line each."""
    failures = []
    if facts["result"] != "completed":
        failures.append(f"route result {facts['result']!r}, not 'completed'")
    if digest is not None and facts["digest"] != digest:
        failures.append(f"route digest {facts['digest']} != expected {digest}")
    if chapters is not None and facts["chapters"] != chapters:
        failures.append(f"route chapters {facts['chapters']} != expected {chapters}")
    return failures


RECEIPT_RE = re.compile(r"receipt=(\S+)")


def named_receipt(log: Path) -> str | None:
    """The last `receipt=<path>` a step's log printed (certify and gate do)."""
    if not log.is_file():
        return None
    found = RECEIPT_RE.findall(log.read_text(errors="replace"))
    return found[-1] if found else None


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--expect-digest", help="the route digest the candidate must reproduce")
    parser.add_argument("--expect-chapters", type=int, help="the route's chapter count")
    parser.add_argument("--allow-dirty", action="store_true",
                        help="verify a tree with uncommitted changes (the receipt says so)")
    return parser


def main(argv: list[str] | None = None, runner=run_step) -> int:
    args = build_parser().parse_args(argv)
    try:
        top = Path(git("rev-parse", "--show-toplevel")).resolve()
    except (subprocess.CalledProcessError, FileNotFoundError):
        print("verify_candidate: not in a git repository", file=sys.stderr)
        return EXIT_REFUSED
    if top != Path.cwd().resolve():
        print(f"verify_candidate: run from the repository root ({top})", file=sys.stderr)
        return EXIT_REFUSED
    dirty = git("status", "--porcelain")
    if dirty and not args.allow_dirty:
        print("verify_candidate: the tree is dirty; freeze the candidate first "
              "(or --allow-dirty)", file=sys.stderr)
        return EXIT_REFUSED
    sha = git("rev-parse", "HEAD")
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    out = CANDIDATE_DIR / f"{stamp}-{sha[:7]}"
    out.mkdir(parents=True)
    lock = Path(os.environ.get("PSIV_HEAVY_LOCK", DEFAULT_HEAVY_LOCK))

    records = []
    for name, command, heavy in steps(out):
        log = out / f"{name}.log"
        start = time.monotonic()
        code = runner(command, log, lock if heavy else None)
        records.append({"step": name, "command": command, "exit": code,
                        "seconds": round(time.monotonic() - start, 1), "log": str(log),
                        "receipt": named_receipt(log)})
        print(f"[{name}] exit {code} in {records[-1]['seconds']}s -> {log}", flush=True)

    facts = route_facts(out)
    failures = [f"step {r['step']} exited {r['exit']}" for r in records if r["exit"]]
    failures += expectation_failures(facts, args.expect_digest, args.expect_chapters)
    receipt = {"sha": sha, "dirty": dirty.splitlines(), "steps": records, "route": facts,
               "expect": {"digest": args.expect_digest, "chapters": args.expect_chapters},
               "failures": failures}
    (out / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    verdict = "OK" if not failures else "FAILED: " + "; ".join(failures)
    print(f"candidate {sha[:7]} {verdict}; route {facts['result']} {facts['chapters']} "
          f"chapters digest {facts['digest']}; receipt={out / 'receipt.json'}")
    return EXIT_FAILED if failures else EXIT_OK


if __name__ == "__main__":
    sys.exit(main())
