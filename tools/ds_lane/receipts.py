"""A finished run's record: commit, receipt, write set, size rule, summary, run.json.

run.json's presence is what marks a run complete (see `run_finished`), so
everything a reader may need - the exit code, the turn error, the write-set
verdict, the files over the line limit and the number of the run the stall
watchdog starts in this one's place - is written before it lands. What the
worker left to read (its trajectory and result) is `trajectory`.
"""
import json
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

from .config import (BINARY_SNIFF_BYTES, EFFORT, HOST_STATE_PATHS, MODEL, PERMISSION_MODE,
                     max_file_lines, save_receipt, size_exempt, supervisor_alive)
from .preflight import CONSTRAINT_BLOCK, write_set_match, write_set_violations
from .trajectory import receipt_block, result_of, scan_trajectory


def count_lines(data):
    """Lines in file content, or None when it is binary (a NUL byte in the first 8 KiB).

    Content is bytes, not text: a lane may commit any encoding and only a line
    count is wanted. The count is the one an editor shows, so a final line
    without a newline still counts.
    """
    if b"\0" in data[:BINARY_SNIFF_BYTES]:
        return None
    return len(data.splitlines())


def lines_at(wt, rev, path):
    """Lines in `path` at `rev` in the lane worktree, or None when it is gone or binary.

    The count comes from the blob the revision records, so a run's numbers are
    what it committed, not what the worktree happens to hold afterwards.
    """
    r = subprocess.run(["git", "show", f"{rev}:{path}"], cwd=wt, stdout=subprocess.PIPE,
                       stderr=subprocess.DEVNULL)
    return count_lines(r.stdout) if r.returncode == 0 else None


def oversize_files(wt, base_sha, head, paths, limit, exempt=()):
    """[{path, lines, base_lines}] for the changed paths over `limit` lines at `head`.

    `paths` is a run's changed set (tracked, base..head). A path gone at the
    new head is skipped - a deleted file has no size - and so is anything
    binary there, since counting lines in a blob is meaningless; `exempt` is
    the size rule's skip list (generated and data files), matched the way a
    write set matches a path. `base_lines` is the count at the lane's base,
    None for a path that is new there (or was binary), so a reader sees how
    much of the size this lane added. The base is only read for a path already
    over the limit at the head, which keeps this to one `git show` per changed
    file in the common case.
    """
    out = []
    for path in paths:
        if write_set_match(path, exempt):
            continue
        lines = lines_at(wt, head, path)
        if lines is None or lines <= limit:
            continue
        out.append({"path": path, "lines": lines, "base_lines": lines_at(wt, base_sha, path)})
    return out


# ------------------------------------------------------- the finalize commit

class FinalizeError(RuntimeError):
    """A git step of the finalize commit failed; the run is recorded anyway."""


def git_step(wt, *args):
    """Run one git step of the finalize commit in the worktree.

    `config.sh` exits the process on failure, which is too blunt here: the
    refusal in the staging step left lane or-O1's finished run with nothing but
    a bare SUPERVISOR ERROR record, no run.json and no summary of what the
    worker did (2026-09-24). A failure raises FinalizeError instead, so the
    caller can still record the run.
    """
    r = subprocess.run(["git", *args], cwd=wt, text=True,
                       stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if r.returncode != 0:
        raise FinalizeError(f"git {' '.join(args)} failed ({r.returncode}): "
                            f"{' '.join(r.stderr.split()) or 'no output'}")
    return r.stdout.strip()


def is_ignored(wt, rel):
    """True when the worktree's ignore rules already cover `rel`.

    Such a path needs no `:(exclude)` pathspec: `git add -A` skips an ignored
    path by itself, while naming one makes git refuse the whole add ("The
    following paths are ignored by one of your .gitignore files"), which is how
    lane or-O1's finalize died on a linked ROM matching `Phantasy Star IV*.md`
    (2026-09-24). A `dir/` rule does not match a directory *symlink* - git
    records a link as a file, mode 120000 - so a linked directory still needs
    its exclusion. `--no-index` answers about the rules alone, whatever the
    index happens to track; any other answer, including git refusing to read a
    path beyond a symbolic link, falls back to naming it, which is what the
    harness did before this check.
    """
    r = subprocess.run(["git", "check-ignore", "-q", "--no-index", rel], cwd=wt,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return r.returncode == 0


def commit_excludes(wt, lane):
    """The `:(exclude)` pathspecs the finalize commit needs for this lane.

    The linked local inputs and the worker's own host state are kept out of the
    commit; the ones git already ignores are left unnamed, because naming them
    is exactly what git refuses.
    """
    rels = list(lane.get("links", [])) + list(HOST_STATE_PATHS)
    return [f":(exclude){rel}" for rel in rels if not is_ignored(wt, rel)]


def head_sha(wt):
    """HEAD in the worktree, or None when not even that can be read."""
    r = subprocess.run(["git", "rev-parse", "HEAD"], cwd=wt, text=True,
                       stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    return (r.stdout.strip() or None) if r.returncode == 0 else None


def commit_run(wt, lane, n):
    """Stage and commit the worker's output as one reviewable commit.

    Returns {"committed", "head", "changed", "diff", "stat"}; `changed` is the
    run's changed set, base..head, which the write-set and size checks read.
    Every step goes through git_step, so a failure raises FinalizeError naming
    the command and carrying git's own message.
    """
    git_step(wt, "add", "-A", "--", ".", *commit_excludes(wt, lane))
    committed = subprocess.run(["git", "diff", "--cached", "--quiet"], cwd=wt).returncode != 0
    if committed:
        title = lane["title"][:60]
        git_step(wt, "commit", "-q", "--no-verify",
                 "-m", f"ds-lane {lane['id']} run-{n}: {title}")
    head = git_step(wt, "rev-parse", "HEAD")
    span = f"{lane['base_sha']}..{head}"
    return {"committed": committed, "head": head,
            "changed": git_step(wt, "diff", "--name-only", span).splitlines(),
            "diff": git_step(wt, "diff", span) + "\n",
            "stat": git_step(wt, "diff", "--stat", span)}


def finalize_run(lane, run_dir, spec, rc, started, duration, outcome=None, stall_resume=None):
    """Record a finished run: commit, receipt, write set, size rule, summary, run.json.

    `outcome` is None for a normal run, "timeout", "stopped" or "stalled"; a
    run stopped before it ever started has no stdout.log and no turn.
    `stall_resume` is the run number the watchdog starts in this run's place
    (supervisor.exec_run); it is fixed here, before run.json lands, so a
    `wait` blocked on this run always learns what follows from one read.
    A git step that fails is recorded as `finalize_error` rather than raised:
    the run still gets its run.json, with the worker's exit code, result and
    receipt readable, and the checks that need git report nothing.
    """
    wt, state, n = Path(lane["worktree"]), Path(lane["state_dir"]), spec["run"]
    result = result_of(run_dir)
    (run_dir / "result.md").write_text(result.get("result", "") + "\n")

    # Commit on the worker's behalf (outside the sandbox) so each run is one reviewable
    # commit. The linked local inputs and the worker's own host state are excluded - the
    # ones git already ignores are left unnamed, since naming one refuses the whole add.
    finalize_error = None
    try:
        work = commit_run(wt, lane, n)
    except FinalizeError as e:
        finalize_error = str(e)
        work = {"committed": False, "head": None, "changed": [], "diff": "",
                "stat": "(finalize did not complete)"}
    head = work["head"] or head_sha(wt) or lane["base_sha"]
    committed = work["committed"]

    evidence = wt / "build/lane-evidence"
    if evidence.is_dir():
        shutil.copytree(evidence, run_dir / "evidence", dirs_exist_ok=True)
    for rel in HOST_STATE_PATHS:  # excluded from the commit, kept reviewable in the receipt
        host_state = wt / rel
        if host_state.is_dir():
            shutil.copytree(host_state, run_dir / "host-state" / host_state.name, dirs_exist_ok=True)
    (run_dir / "diff.patch").write_text(work["diff"])
    stat = work["stat"]

    # Write-set enforcement: every path this lane has, base..new head. The set
    # is the lane's current one, which a resumed run's follow-up may have
    # replaced (lanes.adopt_write_set); the run records what it was checked
    # against, so a reader never has to reconstruct that from lane.json.
    write_set = lane.get("write_set")
    changed = work["changed"]
    violations = write_set_violations(changed, write_set) if write_set else None

    # The owner's 1,000-line rule, reported like a write-set violation: a file
    # this lane changed and left too big to touch again. Generated and data
    # files are exempt (size_exempt), and the run records the list it used, so
    # a reader can tell a skipped path from an unflagged one. Linked inputs and
    # the worker's host state never reach a commit, so they are never counted.
    limit = max_file_lines()
    exempt = size_exempt()
    oversize = oversize_files(wt, lane["base_sha"], head, changed, limit, exempt)

    session_id, turn_err, constraint_blocks, sandbox_blocks = scan_trajectory(
        run_dir / "trajectory.jsonl")
    session_id = result.get("session_id") or session_id
    if outcome == "timeout":
        turn_err = f"timeout after {spec.get('timeout'):g} s"
    elif outcome == "stopped":
        turn_err = "stopped by orchestrator"
    elif outcome == "stalled":
        turn_err = f"stalled: no progress for {spec.get('stall_timeout'):g} s"

    run = {
        "run": n, "started": started, "duration_s": duration, "exit_code": rc,
        "model": MODEL, "effort": EFFORT, "permission_mode": PERMISSION_MODE,
        "session_id": session_id, "turn_error": turn_err, "is_error": result.get("is_error"),
        "num_turns": result.get("num_turns"), "cost_usd": result.get("total_cost_usd"),
        "usage": result.get("usage"), "committed": committed, "head_sha": head,
        "finalize_error": finalize_error,
        "resumed_session": spec.get("resume_session"), "read_only": lane.get("read_only", False),
        "constraint_blocks": constraint_blocks, "sandbox_blocks": sandbox_blocks,
        "timeout_s": spec.get("timeout"), "timed_out": outcome == "timeout",
        "outcome": outcome or "completed", "write_set_violations": violations,
        "write_set": write_set, "oversize_files": oversize, "max_file_lines": limit,
        "size_exempt": exempt,
        "stall_timeout_s": spec.get("stall_timeout"),
        "stall_retries_left": spec.get("stall_retries_left"),
        "resumed_after_stall": bool(spec.get("resumed_after_stall")),
        "stalled": outcome == "stalled", "stall_resumed_run": stall_resume,
    }
    lane["runs"].append(run)
    lane["head_sha"] = head
    lane["total_cost_usd"] = round(sum(r.get("cost_usd") or 0 for r in lane["runs"]), 6)
    save_receipt(state, lane)

    status = re.search(r"-\s*Status:\s*(\S+)", result.get("result", ""))
    out = [f"lane {lane['id']} run-{n}: exit={rc} worker_status={status.group(1) if status else 'no-receipt'} "
           f"cost=${run['cost_usd']} turns={run['num_turns']} time={duration}s committed={committed}"]
    if turn_err:
        out.append(f"turn ended: {turn_err}")
    if finalize_error:
        out.append(f"WARNING: finalize failed: {finalize_error}")
        out.append("the worker's result and receipt are preserved below; its files stay "
                   "in the worktree, uncommitted or partly staged")
    if stall_resume:
        out.append(f"auto-resumed after the stall as run-{stall_resume}")
    if constraint_blocks and not lane.get("read_only"):
        out.append(f"WARNING: {constraint_blocks} tool call(s) hit a Reasonix prompt-constraint block "
                   f"('{CONSTRAINT_BLOCK} ...'). Negated-mutation wording in the brief likely banned "
                   f"writes; rephrase positively and resume.")
    if sandbox_blocks:
        out.append(f"note: {sandbox_blocks} sandbox block(s) (write outside worktree attempted)")
    out.append(f"branch ds/{lane['id']} head={head[:10]} base={lane['base_sha'][:10]}")
    out.append(stat or "(no changes)")
    for p in violations or []:
        out.append(f"WARNING: outside write set: {p}")
    for f in oversize:  # one line per file, beside the write-set warnings
        was = "new" if f["base_lines"] is None else f["base_lines"]
        out.append(f"WARNING: over {limit} lines: {f['path']} ({f['lines']}, was {was})")
    out.append(f"receipts: {run_dir}")
    receipt = receipt_block(result.get("result", ""))
    if receipt:
        out.append(receipt)
    (run_dir / "summary.txt").write_text("\n".join(out) + "\n")
    # run.json last: its presence marks the run complete.
    (run_dir / "run.json").write_text(json.dumps(run, indent=2) + "\n")


def record_crashed_run(lane, run_dir, spec, error, *, started=None, duration=None, exit_code=None):
    """Record a run whose supervisor died before run.json could land.

    A supervisor that raises - inside the run, or inside finalize before the
    record was written - left the run directory in place with nothing in
    lane.json to say it had happened: `resume` then numbered the next run from
    `len(lane["runs"])` and tried to create that same directory again
    (FileExistsError), and the session id the dead run's trajectory holds was
    lost with it (2026-09-24). Whatever is known is recorded here instead - the
    error, the session id read from the trajectory, and the worker's turn
    error, exit code and cost when its result arrived - so a later `resume`
    continues that session.

    This never writes run.json: its presence is what marks a run complete.
    `failed` is the terminal marker instead, so `wait` reports the run as
    failed and prints the error from the summary written here. A run finalize
    had already appended to lane.json is updated in place, so a failure after
    that point cannot list one run twice.
    """
    run_dir = Path(run_dir)
    result = result_of(run_dir)
    session_id, turn_err, constraint_blocks, sandbox_blocks = scan_trajectory(
        run_dir / "trajectory.jsonl")
    run = {
        "run": spec.get("run"), "started": started, "duration_s": duration,
        "exit_code": exit_code if exit_code is not None else result.get("exit_code"),
        "model": MODEL, "effort": EFFORT, "permission_mode": PERMISSION_MODE,
        "session_id": result.get("session_id") or session_id, "turn_error": turn_err,
        "is_error": result.get("is_error"), "num_turns": result.get("num_turns"),
        "cost_usd": result.get("total_cost_usd"), "usage": result.get("usage"),
        "resumed_session": spec.get("resume_session"), "read_only": lane.get("read_only", False),
        "constraint_blocks": constraint_blocks, "sandbox_blocks": sandbox_blocks,
        "timeout_s": spec.get("timeout"), "timed_out": False,
        "stall_timeout_s": spec.get("stall_timeout"),
        "stall_retries_left": spec.get("stall_retries_left"),
        "resumed_after_stall": bool(spec.get("resumed_after_stall")),
        "stalled": False, "stall_resumed_run": None, "outcome": "error", "error": error,
    }
    recorded = next((r for r in lane["runs"] if r.get("run") == run["run"]), None)
    if recorded is None:
        lane["runs"].append(run)
    else:  # finalize reached its record first: its fields stand, only fill the gaps
        for key, value in run.items():
            if key not in recorded and value is not None:
                recorded[key] = value
        run = recorded
    run["outcome"] = "error"  # no run.json, so no run this reader can call completed
    run["error"] = error
    lane["total_cost_usd"] = round(sum(r.get("cost_usd") or 0 for r in lane["runs"]), 6)
    save_receipt(lane["state_dir"], lane)

    known = [f"exit={run['exit_code']}" if run["exit_code"] is not None else "exit unknown",
             f"cost=${run['cost_usd']}" if run["cost_usd"] is not None else "cost unknown",
             f"session={run['session_id'] or 'unknown'}"]
    (run_dir / "summary.txt").write_text("\n".join([
        f"lane {lane['id']} run-{run['run']}: FAILED supervisor error ({' '.join(known)})",
        f"SUPERVISOR ERROR {error}",
        "the run has no run.json: what the supervisor knew of it before it died is in "
        f"{Path(lane['state_dir']) / 'lane.json'}, and its files stay in the worktree",
        f"see {run_dir / 'supervisor.log'}",
        f"receipts: {run_dir}",
    ]) + "\n")
    return run


def clear_stall_resume(run_dir, error):
    """Undo run.json's stall pointer after a failed auto-resume, and say why."""
    path = Path(run_dir) / "run.json"
    run = json.loads(path.read_text())
    run["stall_resumed_run"] = None
    run["stall_resume_error"] = error
    tmp = path.with_name("run.json.tmp")
    tmp.write_text(json.dumps(run, indent=2) + "\n")
    tmp.replace(path)
    summary = Path(run_dir) / "summary.txt"
    if summary.exists():
        summary.write_text(summary.read_text() + f"stall resume failed: {error}\n")


def run_finished(run_dir):
    run_dir = Path(run_dir)
    return (run_dir / "run.json").exists() or (run_dir / "failed").exists()


def print_summary(run_dir):
    summary = Path(run_dir) / "summary.txt"
    print(summary.read_text() if summary.exists() else f"(no summary in {run_dir})", end="")


def wait_run(run_dir, grace=30):
    """Wait for a run, then follow the stall chain to the lane's final run.

    A stalled run records the number of the run the watchdog started in its
    place, so a waiter that attached before the stall still reports the
    outcome of the run that finished the lane's work.
    """
    run_dir = Path(run_dir)
    t0 = time.monotonic()
    while not run_finished(run_dir):
        if time.monotonic() - t0 > grace and not supervisor_alive(run_dir):
            sys.exit(f"ds-lane: supervisor for {run_dir} died without a result; see supervisor.log")
        time.sleep(0.25 if time.monotonic() - t0 < 5 else 3)
    print_summary(run_dir)
    if (run_dir / "failed").exists():
        return 1
    run = json.loads((run_dir / "run.json").read_text())
    successor = run.get("stall_resumed_run")
    if successor:
        return wait_run(run_dir.parent / f"run-{successor}", grace=grace)
    return run["exit_code"]
