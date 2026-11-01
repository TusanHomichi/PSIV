#!/usr/bin/env python3
"""The full gate on one commit, in the gate's kept worktree, with a summary
fit to post on its pull request.

    python3 tools/pr_gate.py DIR COMMIT [--summary FILE] -- GATE...

This is what the machine that holds the local inputs runs for a pull
request's `gate` status (docs/DEVELOPMENT.md, "Gate and coverage"): one
command a runner starts, with no agent in it. Run it anywhere in a checkout
of the repository that has COMMIT and whose `origin/main` is fetched. Under
DIR's lock (`DIR.lock`), so two runs never move the one worktree under each
other:

1. main's `tools/gate_worktree.py` moves the kept worktree DIR to COMMIT;
2. main's `tools/local_inputs.py link` links the local inputs into it;
3. GATE runs there (`python3 tools/gate.py --wait`, or a wrapper around it
   such as burrow's `flaky run ... --`), its output passing through, told
   where to write its receipt (`PSIV_GATE_RECEIPT`, tools/gate.py): a
   directory of this run's own, since every worktree links the main
   checkout's `build/gate/`, which holds every other run's;
4. that receipt is read back, and FILE gets a Markdown summary of it: each
   command's exit, duration and test counts.

Each step runs in a systemd scope of the user's manager, one name for DIR
as its real path has it (`psiv-pr-gate-HASH.scope`, beside the lock, so
every spelling of DIR shares both), so the kernel's cgroup holds every
process it starts, whatever its process group, session or descriptors. When
the step ends, or this is stopped (SIGTERM, SIGINT, SIGHUP), the scope is
stopped (SIGTERM, then SIGKILL after GRACE_S), and its cgroup must then be
gone or empty, however the stop went, before the next step or the lock. A
run killed outright leaves its scope; the next run on DIR stops it the same
way, once it holds the lock, before it touches DIR. A scope stops with the
user unit this runs in, if it runs in one, as the steward's runs do. Its
arguments reach the step as they are (no `${VAR}` expansion).

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
it wrote no full receipt for COMMIT where it was asked to (a `--fast` run,
or a branch whose `tools/gate.py` is older than this, is a failure). A gate
killed by a signal, or this stopped by one, exits 128 plus the signal, and
one that can't start, or exits 126 or more itself, passes that through: the
machine failing, not the commit, so a runner can try again; so does a step
that couldn't be run or stopped in its scope (no user systemd manager, a
scope that never ran its command, processes that outlived its stop), exit
126. 2 for a call
this can't read.
"""

from __future__ import annotations

import argparse
import fcntl
import hashlib
import json
import os
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import uuid
from pathlib import Path

EXIT_PASSED = 0
EXIT_FAILED = 1
EXIT_USAGE = 2

#: A gate that couldn't start, as a shell says it.
EXIT_NOT_STARTED = 127

#: A step that couldn't be run, or stopped, in its scope.
EXIT_UNSUPERVISED = 126

#: The ref whose copies of the worktree and input tools run, whatever COMMIT is.
TOOLS_FROM = "origin/main"

#: Where the gate writes its receipts, relative to the worktree's root
#: (tools/gate.py `GATE_DIR`).
GATE_DIR = Path("build") / "gate"

#: What tells the gate where this run's receipt goes (tools/gate.py `RECEIPT_ENV`).
RECEIPT_ENV = "PSIV_GATE_RECEIPT"

#: The signals that stop a run, its steps first.
STOPS = (signal.SIGTERM, signal.SIGINT, signal.SIGHUP)

#: Seconds a step's scope has to end after SIGTERM, before SIGKILL.
GRACE_S = 10

#: Where the kernel shows cgroups, systemd's ControlGroup= relative to it.
CGROUPS = Path("/sys/fs/cgroup")

#: What a step's scope runs: it marks that the scope started it, then becomes
#: the step's command, so a command that can't start (127) is told apart from
#: a scope that never ran (Unsupervised).
TRAMPOLINE = """import os, sys
open(sys.argv[1], "w").close()
try:
    os.execvp(sys.argv[2], sys.argv[2:])
except OSError as error:
    print(f"pr gate: {sys.argv[2]} couldn't start: {error.strerror}", flush=True)
    os._exit(127)
"""


class StepFailed(Exception):
    """A step before the gate failed; the message says which, fit to post."""


class Unsupervised(Exception):
    """A step couldn't be run, or stopped, in its scope: the machine failing,
    not the commit. The message says which, fit to post."""


class Stopped(Exception):
    """This run got one of STOPS."""

    def __init__(self, signum: int):
        super().__init__(signum)
        self.signum = signum


def _stop(signum, _frame):
    raise Stopped(signum)


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


def scope_for(worktree: Path) -> str:
    """The scope every step on worktree runs in, one at a time."""
    return f"psiv-pr-gate-{hashlib.sha256(str(worktree).encode()).hexdigest()[:16]}.scope"


def caller_unit(cgroups: Path = Path("/proc/self/cgroup")) -> str | None:
    """The unit of the user's manager this runs in, if it runs in one."""
    try:
        lines = cgroups.read_text().splitlines()
    except OSError:
        return None
    for line in lines:
        if line.startswith("0::"):
            parts = line[3:].split("/")
            if (any(p.startswith("user@") and p.endswith(".service") for p in parts[:-1])
                    and parts[-1].endswith((".service", ".scope"))):
                return parts[-1]
    return None


def systemctl(*args: str) -> subprocess.CompletedProcess:
    """`systemctl --user ARGS`, its output captured."""
    try:
        return subprocess.run(["systemctl", "--user", *args], capture_output=True, text=True, check=False)
    except OSError as error:
        raise Unsupervised("no user systemd manager ran its steps") from error


def populated(group: str, cgroups: Path = CGROUPS) -> bool:
    """Whether the cgroup systemd calls `group` still holds a process: one
    that's gone holds none, and one that can't be read isn't known to be
    empty."""
    if not group:
        return False
    try:
        events = (cgroups / group.lstrip("/") / "cgroup.events").read_text()
    except FileNotFoundError:
        return False
    except OSError:
        return True
    return "populated 0" not in events.splitlines()


def stop_scope(scope: str, cgroups: Path = CGROUPS) -> None:
    """Stop the scope, if it's there, then make sure nothing of it is left:
    stopped, its cgroup gone or empty. A stop can end with processes that
    outlived SIGKILL (the scope `failed`), so how it ended proves nothing."""
    systemctl("stop", scope)
    shown = systemctl("show", "--property=ActiveState", "--property=ControlGroup", scope)
    if shown.returncode != 0:
        raise Unsupervised("no user systemd manager ran its steps")
    props = dict(line.partition("=")[::2] for line in shown.stdout.splitlines())
    if props.get("ActiveState") not in ("inactive", "failed") or populated(props.get("ControlGroup", ""), cgroups):
        raise Unsupervised("what a step started outlived its stop")


def run_step(command: list[str], cwd: Path, scope: str, source: str | None = None,
             env: dict[str, str] | None = None) -> int:
    """COMMAND's exit in cwd (negative for a signal), in the scope, `source` on
    its stdin; its output passes through. The scope is stopped before this
    returns, however it returns."""
    marks = Path(tempfile.mkdtemp(prefix="pr-gate-"))
    started = marks / "started"
    unit = caller_unit()
    bind = [f"--property=BindsTo={unit}"] if unit else []
    try:
        try:
            process = subprocess.Popen(
                ["systemd-run", "--user", "--scope", "--quiet", "--collect", "--expand-environment=no",
                 f"--unit={scope}", f"--property=TimeoutStopSec={GRACE_S}s", *bind, "--",
                 sys.executable, "-c", TRAMPOLINE, str(started), *command],
                cwd=cwd, env=env, text=True, stdin=subprocess.PIPE if source is not None else None)
        except OSError as error:
            raise Unsupervised("no user systemd manager ran its steps") from error
        try:
            if source is not None:
                try:
                    process.stdin.write(source)
                    process.stdin.close()
                except BrokenPipeError:
                    pass
            code = process.wait()
        finally:
            # A second stop waits till this one's cleanup is done.
            signal.pthread_sigmask(signal.SIG_BLOCK, STOPS)
            try:
                stop_scope(scope)
                process.wait()
            finally:
                signal.pthread_sigmask(signal.SIG_UNBLOCK, STOPS)
        if not started.exists():
            raise Unsupervised("a step's scope never ran its command")
        return code
    finally:
        shutil.rmtree(marks, ignore_errors=True)


def run_main_copy(here: Path, cwd: Path, scope: str, tool: str, *args: str) -> int:
    """main's copy of `tool`, run on stdin with `args` in cwd, as a step."""
    return run_step([sys.executable, "-", *args], cwd, scope, source=main_copy(here, tool))


def read_receipt(path: Path, commit: str) -> dict | None:
    """The receipt at `path` if it is one for commit, else None."""
    try:
        receipt = json.loads((path / "receipt.json").read_text())
    except (OSError, ValueError):
        return None
    return receipt if isinstance(receipt, dict) and receipt.get("candidate_sha") == commit else None


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
        return EXIT_FAILED, (f"failed: the gate wrote no receipt for {commit[:12]} where it was asked to "
                             f"(exit {gate_exit}; a branch older than tools/pr_gate.py needs main merged in)")
    if receipt.get("tier") != "full":
        return EXIT_FAILED, f"failed: that was the {receipt.get('tier')} tier, not the full gate"
    if receipt.get("dirty") is not False:
        return EXIT_FAILED, "failed: the tree it ran on wasn't clean"
    missing = receipt.get("missing_inputs")
    if not isinstance(missing, list):
        return EXIT_FAILED, "failed: its receipt doesn't say which inputs it had"
    if missing:
        return EXIT_FAILED, f"failed: partial, without {', '.join(f'`{_cell(m)}`' for m in missing)}"
    commands = receipt.get("commands", [])
    failed = [r for r in commands if r.get("exit_code") != 0]
    if failed or receipt.get("exit_code") != 0:
        return EXIT_FAILED, f"failed, {len(failed)} of {len(commands)} commands"
    if gate_exit != 0:
        return EXIT_FAILED, f"failed: every command passed, but the gate exited {gate_exit}"
    return EXIT_PASSED, "passed"


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
        scope = scope_for(worktree)
        try:
            # What a run killed outright left running.
            stop_scope(scope)
            try:
                if run_main_copy(here, here, scope, "gate_worktree.py", str(worktree), full) != 0:
                    raise StepFailed("the gate's worktree couldn't be prepared")
                if run_main_copy(here, worktree, scope, "local_inputs.py", "link") != 0:
                    raise StepFailed("the local inputs couldn't be prepared")
            except StepFailed as error:
                return EXIT_FAILED, summary(full, None, f"failed before it ran: {error}")
            stamp = time.strftime("%Y%m%dT%H%M%SZ", time.gmtime())
            asked = worktree / GATE_DIR / f"{stamp}-{full[:7]}-pr-{uuid.uuid4().hex[:12]}"
            gate_exit = run_step(gate, worktree, scope, env={**os.environ, RECEIPT_ENV: str(asked)})
        except Unsupervised as error:
            return EXIT_UNSUPERVISED, summary(full, None, f"it couldn't run: {error}")
        receipt = read_receipt(asked, full)
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
    # From here to the end, a stop is a stop: the commit, the lock, the
    # steps, the receipt and the summary alike.
    try:
        for signum in STOPS:
            signal.signal(signum, _stop)
        # Its real path: every spelling of it shares one lock and one scope.
        code, said = pr_gate(Path.cwd(), args.dir.resolve(), args.commit, gate)
        post(args.summary, said)
        return code
    except Stopped as stop:
        signal.pthread_sigmask(signal.SIG_BLOCK, STOPS)
        post(args.summary, summary(args.commit, None, f"stopped by signal {stop.signum}, so it didn't finish"))
        return 128 + stop.signum


def post(path: Path | None, said: str) -> None:
    """Write the summary to path, if there is one, and say its first line."""
    if path:
        path.write_text(said)
    print(f"pr gate: {said.splitlines()[0]}", flush=True)


if __name__ == "__main__":
    sys.exit(main())
