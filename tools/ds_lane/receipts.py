"""A finished run's record: commit, receipt, write set, summary, run.json.

run.json's presence is what marks a run complete (see `run_finished`), so
everything a reader may need - the exit code, the turn error, the write-set
verdict and the number of the run the stall watchdog starts in this one's
place - is written before it lands.
"""
import json
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

from .config import (EFFORT, HOST_STATE_PATHS, MODEL, PERMISSION_MODE, save_receipt, sh,
                     supervisor_alive)
from .preflight import CONSTRAINT_BLOCK, write_set_violations


def scan_trajectory(path):
    """(session_id, turn_error, constraint_blocks, sandbox_blocks) from a trajectory.

    `reasonix run` omits session_id from its JSON result; the trajectory has it.
    """
    session_id, turn_err, constraint_blocks, sandbox_blocks = None, None, 0, 0
    for line in Path(path).read_text(errors="replace").splitlines():
        try:
            ev = json.loads(line).get("event", {})
        except json.JSONDecodeError:
            continue
        session_id = session_id or ev.get("sessionId")
        if ev.get("kind") == "tool_result":
            err = str((ev.get("tool") or {}).get("err") or "")
            constraint_blocks += err.startswith(CONSTRAINT_BLOCK)
            sandbox_blocks += "outside the writable roots" in err
        if ev.get("kind") == "turn_done" and ev.get("status") != "completed":
            turn_err = ev.get("err") or ev.get("status")
    return session_id, turn_err, constraint_blocks, sandbox_blocks


def receipt_block(text):
    """The worker's final `## Receipt` block, when its result carries one."""
    lines = (text or "").splitlines()
    heads = [i for i, l in enumerate(lines) if re.match(r"^#{1,6}\s*Receipt\b", l.strip())]
    return "\n".join(lines[heads[-1]:]).strip() if heads else None


def trajectory_tail(path):
    """(tool calls, latest assistant message text) from a trajectory.jsonl."""
    started, calls, message = {}, [], ""
    if not Path(path).exists():
        return calls, message
    for line in Path(path).read_text(errors="replace").splitlines():
        try:
            ev = json.loads(line).get("event", {})
        except json.JSONDecodeError:
            continue
        kind, tool = ev.get("kind"), ev.get("tool") or {}
        if kind == "tool_started":
            started[tool.get("id")] = tool.get("name")
        elif kind == "tool_result":
            state = tool.get("runState") or ("err" if tool.get("err") else "ok")
            calls.append((tool.get("name") or started.get(tool.get("id")) or "?",
                          tool.get("args") or "", state))
        elif kind == "message" and ev.get("text"):
            message = ev["text"]
    return calls, message


def result_of(run_dir):
    """The last JSON line `reasonix run` printed, or {}."""
    stdout_file = Path(run_dir) / "stdout.log"
    if not stdout_file.exists():
        return {}
    for line in reversed(stdout_file.read_text().strip().splitlines()):
        try:
            return json.loads(line)
        except json.JSONDecodeError:
            continue
    return {}


def finalize_run(lane, run_dir, spec, rc, started, duration, outcome=None, stall_resume=None):
    """Record a finished run: commit, receipt, write set, summary, run.json.

    `outcome` is None for a normal run, "timeout", "stopped" or "stalled"; a
    run stopped before it ever started has no stdout.log and no turn.
    `stall_resume` is the run number the watchdog starts in this run's place
    (supervisor.exec_run); it is fixed here, before run.json lands, so a
    `wait` blocked on this run always learns what follows from one read.
    """
    wt, state, n = Path(lane["worktree"]), Path(lane["state_dir"]), spec["run"]
    result = result_of(run_dir)
    (run_dir / "result.md").write_text(result.get("result", "") + "\n")

    # Commit on the worker's behalf (outside the sandbox) so each run is one reviewable commit.
    # The linked local inputs and the worker's own host state are excluded: neither is lane output.
    excludes = [f":(exclude){rel}" for rel in list(lane.get("links", [])) + list(HOST_STATE_PATHS)]
    sh(["git", "add", "-A", "--", "."] + excludes, cwd=wt)
    committed = subprocess.run(["git", "diff", "--cached", "--quiet"], cwd=wt).returncode != 0
    if committed:
        title = lane["title"][:60]
        sh(["git", "commit", "-q", "--no-verify", "-m", f"ds-lane {lane['id']} run-{n}: {title}"], cwd=wt)
    head = sh(["git", "rev-parse", "HEAD"], cwd=wt)
    evidence = wt / "build/lane-evidence"
    if evidence.is_dir():
        shutil.copytree(evidence, run_dir / "evidence", dirs_exist_ok=True)
    for rel in HOST_STATE_PATHS:  # excluded from the commit, kept reviewable in the receipt
        host_state = wt / rel
        if host_state.is_dir():
            shutil.copytree(host_state, run_dir / "host-state" / host_state.name, dirs_exist_ok=True)
    (run_dir / "diff.patch").write_text(sh(["git", "diff", f"{lane['base_sha']}..{head}"], cwd=wt) + "\n")
    stat = sh(["git", "diff", "--stat", f"{lane['base_sha']}..{head}"], cwd=wt)

    # Write-set enforcement: every path this lane has, base..new head. The set
    # is the lane's current one, which a resumed run's follow-up may have
    # replaced (lanes.adopt_write_set); the run records what it was checked
    # against, so a reader never has to reconstruct that from lane.json.
    write_set = lane.get("write_set")
    violations = None
    if write_set:
        changed = sh(["git", "diff", "--name-only", f"{lane['base_sha']}..{head}"], cwd=wt).splitlines()
        violations = write_set_violations(changed, write_set)

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
        "resumed_session": spec.get("resume_session"), "read_only": lane.get("read_only", False),
        "constraint_blocks": constraint_blocks, "sandbox_blocks": sandbox_blocks,
        "timeout_s": spec.get("timeout"), "timed_out": outcome == "timeout",
        "outcome": outcome or "completed", "write_set_violations": violations,
        "write_set": write_set,
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
    out.append(f"receipts: {run_dir}")
    receipt = receipt_block(result.get("result", ""))
    if receipt:
        out.append(receipt)
    (run_dir / "summary.txt").write_text("\n".join(out) + "\n")
    # run.json last: its presence marks the run complete.
    (run_dir / "run.json").write_text(json.dumps(run, indent=2) + "\n")


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
