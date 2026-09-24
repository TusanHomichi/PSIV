"""Paths, environment and the small helpers every other module shares.

Environment is read per call rather than cached at import, so one hermetic
test case can point at its own home, worker binary, lane cap or stall poll
without disturbing the others.
"""
import datetime as dt
import json
import os
import subprocess
import sys
from pathlib import Path

MODEL = "deepseek-flash"
EFFORT = "max"
PERMISSION_MODE = "acceptEdits"
MAX_LANES = 3   # machine-wide concurrent workers; the 13GB laptop OOM'd at 5 (2026-08-16)
CARGO_JOBS = 2  # per-worker cargo parallelism cap, same incident
DEFAULT_TIMEOUT = 5400        # per-run wall clock, seconds
DEFAULT_STALL_TIMEOUT = 900   # per-run silence budget, seconds (0 disables the watchdog)
DEFAULT_STALL_RETRIES = 1     # automatic resumes of a stalled run
STALL_POLL = 15.0             # seconds between stall samples; a whole silent window is a stall
KILL_GRACE = 30               # seconds between SIGTERM and SIGKILL on stop/timeout/stall
TIMEOUT_EXIT = 124
STALL_EXIT = 125              # stalled: the worker is alive but nothing has progressed
STOPPED_EXIT = 143            # `ds-lane stop`: SIGTERM'd on request
STOP_WAIT = 60                # seconds `stop` waits for the run to finish finalizing
WT_ROOT = Path.home() / ".cache/ds-lane/wt"
STATE_ROOT = Path.home() / ".local/state/ds-lane"

# The executable entry point (`tools/ds-lane`), which the detached supervisor
# re-enters as `<python> <entry> _exec <state> <n>`. Resolved from this file
# rather than from sys.argv[0]: ds-lane is normally invoked through the
# ~/.local/bin/ds-lane symlink, and the package always sits next to the entry.
PKG_DIR = Path(__file__).resolve().parent
ENTRY = PKG_DIR.parent / "ds-lane"


# --------------------------------------------------------------- environment

def ds_lane_home():
    h = os.environ.get("DS_LANE_HOME")
    return Path(h).expanduser() if h else None


def wt_root():
    h = ds_lane_home()
    return (h / "wt") if h else WT_ROOT


def state_root():
    h = ds_lane_home()
    return (h / "state") if h else STATE_ROOT


def reasonix_bin():
    return os.environ.get("DS_LANE_REASONIX") or "reasonix"


def max_lanes():
    try:
        return int(os.environ["DS_LANE_MAX_LANES"])
    except (KeyError, ValueError):
        return MAX_LANES


def stall_poll():
    """Seconds between stall samples (the suite lowers it to keep cases short).

    A non-positive value would make the sampling loop spin, so it falls back.
    """
    try:
        value = float(os.environ["DS_LANE_STALL_POLL"])
    except (KeyError, ValueError):
        return STALL_POLL
    return value if value > 0 else STALL_POLL


def sh(args, cwd=None, check=True, capture=True):
    r = subprocess.run(args, cwd=cwd, text=True,
                       stdout=subprocess.PIPE if capture else None,
                       stderr=subprocess.PIPE if capture else None)
    if check and r.returncode != 0:
        sys.exit(f"ds-lane: {' '.join(map(str, args))} failed ({r.returncode}):\n{r.stderr}")
    return r.stdout.strip() if capture else ""


def repo_root(path):
    return Path(sh(["git", "-C", str(path), "rev-parse", "--show-toplevel"]))


def lane_paths(repo, lane_id):
    return wt_root() / repo.name / lane_id, state_root() / repo.name / lane_id


# ----------------------------------------------------------- lane receipts

def load_receipt(state):
    f = Path(state) / "lane.json"
    if not f.exists():
        sys.exit(f"ds-lane: no lane at {state}")
    return json.loads(f.read_text())


def save_receipt(state, data):
    """Persist lane.json by rename, so a concurrent reader never sees it half written."""
    f = Path(state) / "lane.json"
    tmp = f.with_name("lane.json.tmp")
    tmp.write_text(json.dumps(data, indent=2) + "\n")
    os.replace(tmp, f)


def now():
    return dt.datetime.now().astimezone().isoformat(timespec="seconds")


def pid_alive(pidfile):
    try:
        os.kill(int(Path(pidfile).read_text().strip()), 0)
        return True
    except (OSError, ValueError):
        return False


def supervisor_alive(run_dir):
    return pid_alive(Path(run_dir) / "supervisor.pid")
