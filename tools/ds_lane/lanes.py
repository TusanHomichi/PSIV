"""The lane commands and the run-launch path they share.

`prepare_run` is the one way a run is created - the CLI's `start` and `resume`
and the watchdog's automatic resume after a stall all go through it, so every
run gets the same spec, the same detached supervisor and the same receipts.
"""
import datetime as dt
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import time
from pathlib import Path

from .config import (CARGO_JOBS, DEFAULT_STALL_RETRIES, DEFAULT_STALL_TIMEOUT, DEFAULT_TIMEOUT,
                     EFFORT, ENTRY, MODEL, PERMISSION_MODE, STOP_WAIT, lane_paths, load_receipt,
                     now, pid_alive, reasonix_bin, repo_root, save_receipt, sh, state_root,
                     supervisor_alive, wt_root)
from .preflight import PREAMBLE, READ_ONLY_CLAUSE, parse_write_set, preflight_phrasing
from .receipts import print_summary, run_finished, wait_run
from .trajectory import scan_trajectory, trajectory_tail


def run_dirs(state):
    """A lane's run directories, oldest first.

    Only `run-<digits>` counts as a run; the number is what orders them, so a
    name that is not one (a stray file, `run-x`) is not a run and does not
    decide what follows it.
    """
    dirs = [p for p in Path(state).glob("run-*") if p.name.split("-", 1)[1].isdigit()]
    return sorted(dirs, key=lambda p: int(p.name.split("-", 1)[1]))


def lane_runs(lane):
    return run_dirs(lane["state_dir"])


def next_run_number(state):
    """One more than the highest existing run directory.

    `len(lane["runs"])` is not the same thing: a run whose supervisor died
    before it recorded itself leaves its directory behind with no entry in
    lane.json, and numbering from the record made `resume` try to create that
    same directory again (FileExistsError), which left the lane stuck until an
    orchestrator cleared it by hand (2026-09-24).
    """
    dirs = run_dirs(state)
    return int(dirs[-1].name.split("-", 1)[1]) + 1 if dirs else 1


def latest_session(lane):
    """The session a `resume` continues: the last one recorded, else a leftover run's.

    Every run that finalizes records its session in lane.json, and so does a
    crashed one (receipts.record_crashed_run). A supervisor killed outright -
    a host crash, an OOM kill - writes nothing at all, and the only remaining
    word on that conversation is the trajectory it left behind. Resuming
    without it would start a fresh session and lose the lane's context, which
    is half of what a resume is for.
    """
    recorded = next((r["session_id"] for r in reversed(lane["runs"]) if r.get("session_id")), None)
    if recorded:
        return recorded
    for run_dir in reversed(lane_runs(lane)):
        session_id = scan_trajectory(run_dir / "trajectory.jsonl")[0]
        if session_id:
            return session_id
    return None


def find_lane(a):
    repo = repo_root(a.repo)
    _, state = lane_paths(repo, a.id)
    return load_receipt(state)


def followup_prompt(followup):
    """The wrapper every resumed run's prompt gets."""
    return ("# Follow-up from the orchestrator\n\n" + followup +
            "\n\nEnd with the same ## Receipt format as before.")


# What a run's command line holds in the prompt's place. The supervisor feeds
# the run's `prompt.md` to the worker's stdin, so no brief text is an argv word
# and `command.json` names the file instead of quoting it.
STDIN_PROMPT = "<stdin: prompt.md>"


# ----------------------------------------------------------- runs & receipts

def prepare_run(lane, prompt_text, *, max_steps=0, timeout=DEFAULT_TIMEOUT,
                stall_timeout=DEFAULT_STALL_TIMEOUT, stall_retries_left=DEFAULT_STALL_RETRIES,
                resume_session=None, run_number=None, resumed_after_stall=False):
    """Write the run spec, then hand it to a detached supervisor.

    The supervisor runs in its own session (setsid) so a host that reaps
    background shells cannot kill a worker mid-run or skip the commit step.
    `run_number` normally defaults to the next free one, counted from the run
    directories rather than the lane record (next_run_number: a crashed run
    leaves its directory without an entry there); the watchdog passes its
    successor's number explicitly rather than lean on the lane record it has
    just updated.
    """
    wt, state = Path(lane["worktree"]), Path(lane["state_dir"])
    for r in sorted(state.glob("run-*")):
        if not (r / "run.json").exists() and supervisor_alive(r):
            sys.exit(f"ds-lane: lane {lane['id']} {r.name} is still in flight; `ds-lane wait {lane['id']}`")
    n = run_number or next_run_number(state)
    run_dir = state / f"run-{n}"
    run_dir.mkdir(parents=True)
    (run_dir / "prompt.md").write_text(prompt_text)
    traj = run_dir / "trajectory.jsonl"
    traj.touch()  # reasonix refuses to create it
    cmd = [reasonix_bin(), "run", "--dir", str(wt), "--model", MODEL, "--effort", EFFORT,
           "--permission-mode", PERMISSION_MODE, "--output-format", "json",
           "--trajectory", str(traj), "--metrics", str(run_dir / "metrics.json")]
    if max_steps:
        cmd += ["--max-steps", str(max_steps)]
    for d in lane.get("add_dirs", []):
        cmd += ["--add-dir", d]
    if resume_session:
        cmd += ["--resume", resume_session]
    spec = {"run": n, "cmd": cmd, "resume_session": resume_session, "timeout": timeout,
            "max_steps": max_steps, "stall_timeout": stall_timeout,
            "stall_retries_left": stall_retries_left, "resumed_after_stall": bool(resumed_after_stall)}
    (run_dir / "spec.json").write_text(json.dumps(spec, indent=2) + "\n")
    (run_dir / "command.json").write_text(json.dumps(cmd + [STDIN_PROMPT], indent=2) + "\n")
    log = open(run_dir / "supervisor.log", "w")
    subprocess.Popen([sys.executable, str(ENTRY), "_exec", str(state), str(n)],
                     stdin=subprocess.DEVNULL, stdout=log, stderr=log, start_new_session=True)
    return run_dir


def launch_resume(lane, followup, session, *, max_steps=0, timeout=DEFAULT_TIMEOUT,
                  stall_timeout=DEFAULT_STALL_TIMEOUT, stall_retries_left=DEFAULT_STALL_RETRIES,
                  run_number=None, resumed_after_stall=False):
    """Start a follow-up run of `lane` on `session`; called by `resume` and by the watchdog."""
    return prepare_run(lane, followup_prompt(followup), max_steps=max_steps, timeout=timeout,
                       stall_timeout=stall_timeout, stall_retries_left=stall_retries_left,
                       resume_session=session, run_number=run_number,
                       resumed_after_stall=resumed_after_stall)


def wait_or_detach(lane, run_dir, no_wait):
    if no_wait:
        print(f"lane {lane['id']} {run_dir.name}: launched detached; `ds-lane wait {lane['id']}`")
        return 0
    return wait_run(run_dir)


def cmd_start(a):
    repo = repo_root(a.repo)
    brief = Path(a.brief).read_text()
    if not a.read_only:  # a read-only lane wants the ban; --allow-phrasing overrides the check
        preflight_phrasing(brief, "brief", allow=a.allow_phrasing)
    write_set = parse_write_set(brief)
    title = next((l.lstrip("# ").strip() for l in brief.splitlines() if l.strip()), "lane")
    lane_id = a.id or dt.datetime.now().strftime("%m%d-%H%M%S")
    if not re.fullmatch(r"[A-Za-z0-9._-]+", lane_id):
        sys.exit("ds-lane: --id must be [A-Za-z0-9._-]+")
    wt, state = lane_paths(repo, lane_id)
    if wt.exists() or state.exists():
        sys.exit(f"ds-lane: lane {lane_id} already exists")
    base_sha = sh(["git", "-C", str(repo), "rev-parse", a.base])
    dirty = sh(["git", "-C", str(repo), "status", "--porcelain", "--untracked-files=no"])
    if dirty:
        print(f"ds-lane: note: {repo} has uncommitted tracked changes; the lane starts from "
              f"{a.base} ({base_sha[:10]}) and will NOT see them.", file=sys.stderr)

    links = {}  # worktree-relative path -> absolute source
    for spec in a.link:
        rel, _, src = spec.partition("=")
        src = (repo / (src or rel)).resolve()
        if not src.exists():
            sys.exit(f"ds-lane: --link {spec}: {src} does not exist")
        if Path(rel).is_absolute() or ".." in Path(rel).parts:
            sys.exit(f"ds-lane: --link {spec}: PATH must be repo-relative")
        if subprocess.run(["git", "-C", str(repo), "check-ignore", "-q", "--no-index", rel.rstrip("/") + "/"]).returncode != 0:
            sys.exit(f"ds-lane: --link {spec}: only gitignored paths may be linked")
        links[rel.rstrip("/")] = str(src)

    wt.parent.mkdir(parents=True, exist_ok=True)
    state.mkdir(parents=True)
    sh(["git", "-C", str(repo), "worktree", "add", "-q", "-b", f"ds/{lane_id}", str(wt), base_sha])
    for rel, src in links.items():
        dst = wt / rel
        dst.parent.mkdir(parents=True, exist_ok=True)
        dst.symlink_to(src)
    shutil.copy(a.brief, state / "brief.md")

    lane = {
        "id": lane_id, "title": title, "repo": str(repo), "base_ref": a.base, "base_sha": base_sha,
        "branch": f"ds/{lane_id}", "worktree": str(wt), "state_dir": str(state), "links": list(links), "link_sources": links,
        "add_dirs": [str(Path(d).resolve()) for d in a.add_dir], "created": now(),
        "model": MODEL, "effort": EFFORT, "reasonix_version": sh([reasonix_bin(), "--version"]),
        "read_only": a.read_only, "write_set": write_set, "runs": [],
    }
    save_receipt(state, lane)
    print(f"lane {lane_id}: worktree {wt}")
    head = READ_ONLY_CLAUSE if a.read_only else ""
    run_dir = prepare_run(lane, head + PREAMBLE + brief, max_steps=a.max_steps, timeout=a.timeout,
                          stall_timeout=a.stall_timeout, stall_retries_left=a.stall_retries_left)
    return wait_or_detach(lane, run_dir, a.no_wait)


def adopt_write_set(lane, followup):
    """A follow-up's own write-set block becomes the lane's from this run on.

    `resume` is the only place a lane's write set changes: a follow-up without a
    block leaves the current one in place, so the run inherits what the brief or
    an earlier follow-up declared. Lane ab-H run-3 was resumed with a follow-up
    naming two new paths and the run was still checked against the brief's set
    (orchestrator decision 2026-09-24). The new set lands in lane.json before
    the run is launched, so the supervisor reads it when it finalizes, and that
    run's run.json records the set it was actually checked against.
    """
    declared = parse_write_set(followup)
    if declared is None:  # no block: the lane keeps the set it already has
        return
    lane["write_set"] = declared
    save_receipt(lane["state_dir"], lane)


def cmd_resume(a):
    lane = find_lane(a)
    followup = Path(a.followup).read_text()
    if not lane.get("read_only"):
        preflight_phrasing(followup, "follow-up", allow=a.allow_phrasing)
    adopt_write_set(lane, followup)
    run_dir = launch_resume(lane, followup, latest_session(lane), max_steps=a.max_steps,
                            timeout=a.timeout, stall_timeout=a.stall_timeout,
                            stall_retries_left=a.stall_retries_left)
    return wait_or_detach(lane, run_dir, a.no_wait)


def cmd_wait(a):
    lane = find_lane(a)
    runs = lane_runs(lane)
    if not runs:
        sys.exit("ds-lane: lane has no runs")
    return wait_run(runs[-1])


def cmd_stop(a):
    """Stop a lane's in-flight run by signalling its supervisor, then wait.

    Only PIDs recorded in the run directory are used: the supervisor turns the
    SIGTERM into a stop of the worker's process group and finalizes the run.
    """
    lane = find_lane(a)
    pending = [r for r in lane_runs(lane) if not run_finished(r)]
    if not pending:
        print(f"lane {lane['id']}: no run in flight")
        return 0
    run_dir = pending[-1]
    pid_file = run_dir / "supervisor.pid"
    t0 = time.monotonic()  # `start --no-wait` can race the supervisor's first write
    while not pid_file.exists() and not run_finished(run_dir) and time.monotonic() - t0 < 10:
        time.sleep(0.1)
    if run_finished(run_dir):
        print_summary(run_dir)
        return 0
    if not pid_file.exists():
        print(f"lane {lane['id']} {run_dir.name}: no supervisor pid recorded; nothing to signal")
        return 0
    try:
        pid = int(pid_file.read_text().strip())
    except ValueError:
        sys.exit(f"ds-lane: {pid_file} is not a pid")
    if not pid_alive(pid_file):
        print(f"lane {lane['id']} {run_dir.name}: supervisor {pid} is gone; nothing to signal")
        return 0
    print(f"lane {lane['id']} {run_dir.name}: stopping supervisor {pid} (worker group)")
    try:
        os.kill(pid, signal.SIGTERM)
    except ProcessLookupError:
        print(f"lane {lane['id']} {run_dir.name}: supervisor {pid} exited meanwhile")
    deadline = time.monotonic() + STOP_WAIT
    while not run_finished(run_dir):
        if time.monotonic() > deadline:
            print(f"ds-lane: {run_dir} is still finalizing after {STOP_WAIT}s; "
                  f"see {run_dir / 'supervisor.log'}")
            return 1
        time.sleep(0.25 if time.monotonic() - t0 < 5 else 1)
    print_summary(run_dir)
    return 0


def cmd_tail(a):
    lane = find_lane(a)
    runs = lane_runs(lane)
    if not runs:
        sys.exit("ds-lane: lane has no runs")
    run_dir = runs[-1]
    done_file = run_dir / "run.json"
    if done_file.exists() or (run_dir / "failed").exists():
        state = "finished"
        elapsed = json.loads(done_file.read_text())["duration_s"] if done_file.exists() else None
    else:
        started = (run_dir / "spec.json").stat().st_mtime
        elapsed = round(time.time() - started, 1)
        state = "running" if pid_alive(run_dir / "worker.slot") else \
            ("queued" if supervisor_alive(run_dir) else "no supervisor")
    print(f"lane {lane['id']} {run_dir.name}: {state}" +
          (f" elapsed={elapsed}s" if elapsed is not None else ""))
    calls, message = trajectory_tail(run_dir / "trajectory.jsonl")
    if message:
        print(f"last message: {' '.join(message.split())[:300]}")
    print(f"tool calls (last {min(a.n, len(calls))} of {len(calls)}):")
    for i, (name, args, st) in enumerate(calls[-max(a.n, 0):], start=max(1, len(calls) - a.n + 1)):
        print(f"  {i}. {name} [{st}] {' '.join(str(args).split())[:100]}")


def cmd_verify(a):
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


def cmd_list(a):
    repo = repo_root(a.repo)
    root = state_root() / repo.name
    for f in sorted(root.glob("*/lane.json")) if root.exists() else []:
        l = json.loads(f.read_text())
        last = l["runs"][-1] if l["runs"] else {}
        alive = "live" if Path(l["worktree"]).exists() else "removed"
        pending = [r for r in f.parent.glob("run-*") if not (r / "run.json").exists()
                   and not (r / "failed").exists() and supervisor_alive(r)]
        if pending:
            alive = "running" if any(pid_alive(r / "worker.slot") for r in pending) else "queued"
        print(f"{l['id']:<24} {alive:<8} runs={len(l['runs'])} cost=${l.get('total_cost_usd', 0)} "
              f"exit={last.get('exit_code')} {l['title'][:50]}")


def cmd_show(a):
    print(json.dumps(find_lane(a), indent=2))


def cmd_rm(a):
    lane = find_lane(a)
    repo, wt = lane["repo"], lane["worktree"]
    if Path(wt).exists():
        for rel in lane.get("links", []):
            p = Path(wt) / rel
            if p.is_symlink():
                p.unlink()
        sh(["git", "-C", repo, "worktree", "remove", "--force", wt])
    subprocess.run(["git", "-C", repo, "branch", "-D", lane["branch"]], capture_output=True)
    if a.purge:
        shutil.rmtree(lane["state_dir"])
    print(f"lane {lane['id']}: worktree and branch removed" + ("; receipts purged" if a.purge else
          f"; receipts kept at {lane['state_dir']}"))
