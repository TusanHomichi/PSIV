"""What a run left to read: its trajectory and the worker's own JSON result.

`receipts` builds a run's record from these, and `lanes` reads a session id
back out of a trajectory to resume it. Nothing here writes anything.
"""
import json
import re
from pathlib import Path

from .preflight import CONSTRAINT_BLOCK


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
