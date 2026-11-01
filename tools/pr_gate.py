#!/usr/bin/env python3
"""The full gate on one commit, in the gate's kept worktree, with a summary
fit to post on its pull request.

    python3 tools/pr_gate.py DIR COMMIT [--summary FILE] -- GATE...

This is what the machine that holds the local inputs runs for a pull
request's `gate` status (docs/DEVELOPMENT.md, "Gate and coverage"): one
command a runner starts, with no agent in it. Run it anywhere in a checkout
of the repository that has COMMIT and whose `origin/main` is fetched. Under
DIR's lock (`DIR.lock`, held until the gate's last process ends, so two runs
never move the one worktree under each other):

1. main's `tools/gate_worktree.py` moves the kept worktree DIR to COMMIT;
2. main's `tools/local_inputs.py link` links the local inputs into it;
3. GATE runs there (`python3 tools/gate.py --wait`, or a wrapper around it
   such as burrow's `flaky run ... --`), its output passing through;
4. the receipt that run wrote for COMMIT is read back (one an earlier run
   left, in the main checkout's `build/gate/` that every worktree links,
   never counts), and FILE gets a Markdown summary of it: each command's
   exit, duration and test counts.

The summary never quotes a log, a path outside the repository, or anything
taken from the cartridge: it is built from the receipt's command strings,
exit codes, durations, counts and missing inputs alone, so a runner may post
it as it is. A step that fails before the gate writes a summary saying which,
and nothing else.

The steward runs main's copy on any commit (`git show
origin/main:tools/pr_gate.py | python3 - DIR COMMIT ...`), as it does the two
tools it calls, so this file imports nothing from `tools/`.

Exit 0 when the full gate passed on COMMIT: every command exited 0, on a
clean tree that had every input (the receipt's `missing_inputs` is empty:
a partial gate is a failure). 1 when it failed, a step before it failed, or
it left no full receipt for COMMIT (a `--fast` run is a failure). A gate
killed by a signal exits 128 plus the signal, and one that can't start, or
exits 126 or more itself, passes that through: the machine failing, not the
commit, so a runner can try again. 2 for a call this can't read.
"""

from __future__ import annotations

import argparse
import fcntl
import json
import subprocess
import sys
from pathlib import Path

EXIT_PASSED = 0
EXIT_FAILED = 1
EXIT_USAGE = 2

#: A gate that couldn't start, as a shell says it.
EXIT_NOT_STARTED = 127

#: The ref whose copies of the worktree and input tools run, whatever COMMIT is.
TOOLS_FROM = "origin/main"

#: Where the gate writes its receipts, relative to the worktree's root
#: (tools/gate.py `GATE_DIR`).
GATE_DIR = Path("build") / "gate"


class StepFailed(Exception):
    """A step before the gate failed; the message says which, fit to post."""


def resolve(here: Path, commit: str) -> str | None:
    """COMMIT's full SHA, or None if this checkout doesn't have it."""
    shown = subprocess.run(["git", "rev-parse", "--verify", "--quiet", f"{commit}^{{commit}}"],
                           cwd=here, capture_output=True, text=True, check=False)
    return shown.stdout.strip() if shown.returncode == 0 else None


def main_copy(here: Path, tool: str) -> str:
    """The source of `tool` as TOOLS_FROM has it."""
    shown = subprocess.run(["git", "show", f"{TOOLS_FROM}:tools/{tool}"], cwd=here,
                           capture_output=True, text=True, check=False)
    if shown.returncode != 0:
        raise StepFailed(f"{TOOLS_FROM}'s `tools/{tool}` can't be read here")
    return shown.stdout


def run_main_copy(here: Path, cwd: Path, tool: str, *args: str) -> int:
    """main's copy of `tool`, run on stdin with `args` in cwd, its output passing through."""
    return subprocess.run([sys.executable, "-", *args], cwd=cwd, input=main_copy(here, tool),
                          text=True, check=False).returncode


def receipts(worktree: Path) -> dict[str, Path]:
    """Every receipt under the worktree's GATE_DIR, by its directory's name."""
    return {path.parent.name: path for path in (worktree / GATE_DIR).glob("*/receipt.json")}


def receipt_for(worktree: Path, commit: str, before: set[str]) -> dict | None:
    """The newest receipt for commit whose directory isn't in `before`, or None.

    Directories are named for the UTC second the gate started, so the newest
    sorts last."""
    for name, path in sorted(receipts(worktree).items(), reverse=True):
        if name in before:
            continue
        try:
            receipt = json.loads(path.read_text())
        except (OSError, ValueError):
            continue
        if receipt.get("candidate_sha") == commit:
            return receipt
    return None


def _seconds(value: float) -> str:
    whole = int(round(value))
    return f"{whole // 60}m {whole % 60:02d}s" if whole >= 60 else f"{whole}s"


def _counts(counts: dict) -> str:
    """A command's tests, as its receipt counted them; blank when it counts none."""
    if not counts:
        return ""
    if "tests_run" in counts:
        ran = counts["tests_run"] if counts["tests_run"] is not None else "?"
        return (f"{ran} run, {counts.get('failures') or 0} failed, {counts.get('errors') or 0} errors, "
                f"{counts.get('skipped') or 0} skipped")
    if counts.get("passed") is None:
        return "no test result"
    return f"{counts['passed']} passed, {counts.get('failed') or 0} failed, {counts.get('ignored') or 0} ignored"


def _cell(text: object) -> str:
    """Text a table cell can hold as code: no pipes or backticks of its own."""
    return str(text).replace("|", "\\|").replace("`", "'")


def summary(commit: str, receipt: dict | None, verdict: str) -> str:
    """The Markdown a runner posts: the verdict, then a row a command."""
    if receipt is None:
        return f"**Full gate on {commit[:12]}: {verdict}.**\n"
    lines = [f"**Full gate on {commit[:12]}: {verdict}, in {_seconds(receipt.get('duration_s') or 0)}.**",
             "", "| Command | Exit | Time | Tests |", "| --- | --- | --- | --- |"]
    for record in receipt.get("commands", []):
        lines.append(f"| `{_cell(record.get('command', ''))}` | {record.get('exit_code')} "
                     f"| {_seconds(record.get('duration_s') or 0)} | {_counts(record.get('counts') or {})} |")
    return "\n".join(lines) + "\n"


def judge(commit: str, receipt: dict | None, gate_exit: int) -> tuple[int, str]:
    """The exit and the verdict's words for a gate that exited gate_exit."""
    if gate_exit < 0:
        return 128 - gate_exit, f"stopped by signal {-gate_exit}, so it didn't finish"
    if gate_exit >= 126:
        return gate_exit, f"it couldn't run (exit {gate_exit})"
    if receipt is None:
        return EXIT_FAILED, f"failed: the gate left no receipt for {commit[:12]} (exit {gate_exit})"
    if receipt.get("tier") != "full":
        return EXIT_FAILED, f"failed: that was the {receipt.get('tier')} tier, not the full gate"
    if receipt.get("dirty") is not False:
        return EXIT_FAILED, "failed: the tree it ran on wasn't clean"
    missing = receipt.get("missing_inputs")
    if missing is None:
        return EXIT_FAILED, ("failed: this commit's gate doesn't say which inputs it had "
                             "(it predates `missing_inputs`; merge main into the branch)")
    if missing:
        return EXIT_FAILED, f"failed: partial, without {', '.join(f'`{_cell(m)}`' for m in missing)}"
    commands = receipt.get("commands", [])
    failed = [r for r in commands if r.get("exit_code") != 0]
    if failed or receipt.get("exit_code") != 0:
        return EXIT_FAILED, f"failed, {len(failed)} of {len(commands)} commands"
    if gate_exit != 0:
        return EXIT_FAILED, f"failed: every command passed, but the gate exited {gate_exit}"
    return EXIT_PASSED, "passed"


def run_gate(gate: list[str], worktree: Path) -> int:
    """GATE's exit in the worktree, its output passing through: negative for a signal."""
    try:
        return subprocess.run(gate, cwd=worktree, check=False).returncode
    except OSError as error:
        print(f"pr gate: {gate[0]} couldn't start: {error.strerror}", flush=True)
        return EXIT_NOT_STARTED


def pr_gate(here: Path, worktree: Path, commit: str, gate: list[str]) -> tuple[int, str]:
    """The four steps under the worktree's lock: the exit, and the summary."""
    full = resolve(here, commit)
    if full is None:
        return EXIT_FAILED, summary(commit, None, "failed before it ran: the commit isn't in this checkout")
    lock_path = worktree.with_name(worktree.name + ".lock")
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    with open(lock_path, "a") as lock:
        print(f"pr gate: waiting for {lock_path.name}", flush=True)
        fcntl.flock(lock, fcntl.LOCK_EX)
        try:
            if run_main_copy(here, here, "gate_worktree.py", str(worktree), full) != 0:
                raise StepFailed("the gate's worktree couldn't be prepared")
            if run_main_copy(here, worktree, "local_inputs.py", "link") != 0:
                raise StepFailed("the local inputs couldn't be prepared")
        except StepFailed as error:
            return EXIT_FAILED, summary(full, None, f"failed before it ran: {error}")
        before = set(receipts(worktree))
        gate_exit = run_gate(gate, worktree)
        receipt = receipt_for(worktree, full, before)
        code, verdict = judge(full, receipt, gate_exit)
        return code, summary(full, receipt, verdict)


def main(argv: list[str] | None = None) -> int:
    argv = sys.argv[1:] if argv is None else argv
    if "--" not in argv:
        print("pr gate: usage: pr_gate.py DIR COMMIT [--summary FILE] -- GATE...", file=sys.stderr)
        return EXIT_USAGE
    split = argv.index("--")
    parser = argparse.ArgumentParser(
        prog="python3 tools/pr_gate.py",
        description="Run the full gate on COMMIT in the gate's kept worktree, and summarize its receipt.")
    parser.add_argument("dir", type=Path, help="the gate's kept worktree (tools/gate_worktree.py)")
    parser.add_argument("commit", help="the commit the gate runs on")
    parser.add_argument("--summary", type=Path, help="where the Markdown summary goes")
    args = parser.parse_args(argv[:split])
    gate = argv[split + 1:]
    if not gate:
        print("pr gate: nothing to run after --", file=sys.stderr)
        return EXIT_USAGE
    code, said = pr_gate(Path.cwd(), args.dir.absolute(), args.commit, gate)
    if args.summary:
        args.summary.write_text(said)
    print(f"pr gate: {said.splitlines()[0]}", flush=True)
    return code


if __name__ == "__main__":
    sys.exit(main())
