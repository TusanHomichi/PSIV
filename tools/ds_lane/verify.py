"""`ds-lane verify ID -- CMD...`: record the orchestrator's own checks as evidence.

A worker claims what it checked; this is how the orchestrator checks for itself
and leaves the raw output where a reader can weigh it. CMD runs in the lane's
worktree, so it sees the code the lane commits, with CARGO_BUILD_JOBS pinned to
the lane's own cap; its output is streamed to the caller's stdout and saved
under `<state>/verify/NNN.log` with a header naming the command, the UTC start,
the lane head, the exit code and the duration. The command's exit code is
ds-lane's, so a script can branch on the result, and the number in the log name
is one past the highest already there, so a lane's checks read in the order
they were run.
"""
import datetime as dt
import os
import subprocess
import sys
import time
from pathlib import Path

from .config import CARGO_JOBS, sh
from .lanes import find_lane


def cmd_verify(a):
    """Run CMD in the lane's worktree, save the log, and exit with CMD's code."""
    lane = find_lane(a)
    cmd = list(a.cmd)
    if cmd and cmd[0] == "--":
        cmd = cmd[1:]
    if not cmd:
        sys.exit("ds-lane: verify needs a command, e.g. `ds-lane verify ID -- cargo test`")
    wt = Path(lane["worktree"])
    if not wt.is_dir():
        sys.exit(f"ds-lane: worktree {wt} is gone")
    vdir = Path(lane["state_dir"]) / "verify"
    vdir.mkdir(parents=True, exist_ok=True)
    n = max([int(p.stem) for p in vdir.glob("*.log") if p.stem.isdigit()] or [0]) + 1
    log = vdir / f"{n:03d}.log"
    head = sh(["git", "-C", str(wt), "rev-parse", "HEAD"])
    started, t0 = dt.datetime.now(dt.timezone.utc), time.monotonic()
    env = dict(os.environ)
    env["CARGO_BUILD_JOBS"] = str(CARGO_JOBS)
    body = []
    p = subprocess.Popen(cmd, cwd=wt, env=env, text=True,
                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    for line in p.stdout:
        sys.stdout.write(line)
        sys.stdout.flush()
        body.append(line)
    rc = p.wait()
    duration = round(time.monotonic() - t0, 1)
    log.write_text("\n".join([f"command: {' '.join(cmd)}",
                              f"utc_start: {started.isoformat(timespec='seconds')}",
                              f"lane: {lane['id']}  head: {head}",
                              f"exit_code: {rc}",
                              f"duration_s: {duration}",
                              ""]) + "\n" + "".join(body))
    print(f"ds-lane: verify {log} exit={rc} duration={duration}s")
    return rc
