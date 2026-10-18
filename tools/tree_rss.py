#!/usr/bin/env python3
"""tools/tree_rss.py - measure a command's peak process-tree memory (#165).

    python3 tools/tree_rss.py [--interval 0.1] [--log FILE] [--json FILE] -- <command...>

Runs the command and, every `--interval` seconds (default 0.1), sums the
resident set size (`VmRSS`) of the command and every process descended from
it, walked through `/proc/<pid>/stat`'s parent ids. The peak of that sum is the
figure the heavy lock's classes are sized by (docs/DEVELOPMENT.md, "Heavy-run
lock"): a build's compiler and linker children, or a test runner's workers,
hold memory the parent's own RSS (and `/usr/bin/time -v`, which reports the
largest single process) never shows. It also samples the live CPU clock
(`/proc/cpuinfo`, mean over the cores and the fastest core) and the 1-minute
load average once a second, because a wall time means nothing without them.

It prints one line, `exit=... wall=...s peak_tree_rss=...MB mhz_mean=...
mhz_max=... load_1m_max=... cmd=...`, and exits with the command's exit code.
`--log` sends the command's output to a file instead of the terminal;
`--json` also writes the result, with the command, the UTC start, the sampling
interval and the cores, as a JSON object. Run it inside the heavy lock in the
class being measured (`python3 tools/heavy_lock.py run --class exclusive --
python3 tools/tree_rss.py -- <job>`), so nothing else locked shares the run.

Linux only (it reads /proc). Stdlib only, Python 3.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

#: Seconds between tree samples: #140's and #157's tables were measured at 0.1 s.
DEFAULT_INTERVAL_S = 0.1

#: Seconds between clock and load samples.
CLOCK_INTERVAL_S = 1.0


def core_clocks(cpuinfo=Path("/proc/cpuinfo")):
    """Every core's current clock in MHz, from `cpuinfo`; [] where it has none."""
    try:
        return [float(line.split(":", 1)[1]) for line in cpuinfo.read_text().splitlines()
                if line.startswith("cpu MHz")]
    except (OSError, ValueError, IndexError):
        return []


def cpu_mhz(cpuinfo=Path("/proc/cpuinfo")):
    """The mean current clock over every core, in MHz, or None where the file has none.

    Cargo's default jobs and test threads scale with the cores and the clock,
    so a timing is read beside the clock it ran at (the host boosts above its
    base clock under load; DEVELOPMENT.md's measurement table).
    """
    values = core_clocks(cpuinfo)
    return sum(values) / len(values) if values else None


def load_1m(loadavg=Path("/proc/loadavg")):
    """The 1-minute load average, or None where the file is absent.

    The heavy lock excludes locked jobs only; an unlocked headless run (a
    resume check's parallel `psiv-campaign` resumes) still competes for the
    cores, and a timing taken beside one is read with its load.
    """
    try:
        return float(loadavg.read_text().split()[0])
    except (OSError, ValueError, IndexError):
        return None


def _children(proc):
    """{parent pid: [child pids]} for every process `proc` lists now."""
    children = {}
    for entry in os.listdir(proc):
        if not entry.isdigit():
            continue
        try:
            stat = (proc / entry / "stat").read_text()
        except OSError:  # it exited between the listing and the read
            continue
        # `pid (comm) state ppid ...`; comm may hold spaces and parentheses.
        fields = stat[stat.rfind(")") + 2:].split()
        if len(fields) > 1 and fields[1].isdigit():
            children.setdefault(int(fields[1]), []).append(int(entry))
    return children


def _rss_kb(proc, pid):
    """`pid`'s VmRSS in KiB, 0 once it has exited (or for a zombie, which has none)."""
    try:
        for line in (proc / str(pid) / "status").read_text().splitlines():
            if line.startswith("VmRSS:"):
                return int(line.split()[1])
    except (OSError, ValueError, IndexError):
        pass
    return 0


def tree_rss_kb(root, proc=Path("/proc"), descendants=True):
    """The summed VmRSS, in KiB, of `root` and (with `descendants`) every process below it.

    `descendants=False` reads `root` alone: the sampler that misses children,
    kept for the negative control that shows why the tree is summed.
    """
    if not descendants:
        return _rss_kb(proc, root)
    children = _children(proc)
    total, stack, seen = 0, [root], set()
    while stack:
        pid = stack.pop()
        if pid in seen:
            continue
        seen.add(pid)
        total += _rss_kb(proc, pid)
        stack.extend(children.get(pid, ()))
    return total


def measure(command, interval=DEFAULT_INTERVAL_S, stdout=None, descendants=True):
    """Run `command` (an argv list) and return its measurement as a dict.

    `stdout` is a file object for the command's output and errors (None: this
    process's own). The keys: `exit_code`, `wall_s`, `peak_tree_rss_mb`,
    `tree_samples`, `mhz_mean` and `mhz_max` (None without /proc/cpuinfo),
    `load_1m_max` (None without /proc/loadavg), `command`, `start_utc`,
    `interval_s` and `cores`.
    """
    started_utc = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    started = time.monotonic()
    peak_kb, samples, clocks, fastest, loads = 0, 0, [], [], []
    next_clock = started
    process = subprocess.Popen(command, stdout=stdout,
                               stderr=subprocess.STDOUT if stdout else None)
    try:
        while True:
            peak_kb = max(peak_kb, tree_rss_kb(process.pid, descendants=descendants))
            samples += 1
            now = time.monotonic()
            if now >= next_clock:
                cores = core_clocks()
                if cores:
                    clocks.append(sum(cores) / len(cores))
                    fastest.append(max(cores))
                load = load_1m()
                if load is not None:
                    loads.append(load)
                next_clock = now + CLOCK_INTERVAL_S
            try:
                process.wait(timeout=interval)
                break
            except subprocess.TimeoutExpired:
                continue
    except BaseException:
        process.kill()
        process.wait()
        raise
    return {
        "exit_code": process.returncode,
        "wall_s": round(time.monotonic() - started, 1),
        "peak_tree_rss_mb": round(peak_kb / 1024),
        "tree_samples": samples,
        "mhz_mean": round(sum(clocks) / len(clocks)) if clocks else None,
        "mhz_max": round(max(fastest)) if fastest else None,
        "load_1m_max": max(loads) if loads else None,
        "command": list(command),
        "start_utc": started_utc,
        "interval_s": interval,
        "cores": os.cpu_count(),
    }


def summary_line(result):
    """The one line the CLI prints for `result`."""
    return (f"exit={result['exit_code']} wall={result['wall_s']}s "
            f"peak_tree_rss={result['peak_tree_rss_mb']}MB mhz_mean={result['mhz_mean']} "
            f"mhz_max={result['mhz_max']} load_1m_max={result['load_1m_max']} "
            f"cmd={' '.join(result['command'])}")


def main(argv=None):
    """`python3 tools/tree_rss.py [--interval S] [--log FILE] [--json FILE] -- CMD...`."""
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--interval", type=float, default=DEFAULT_INTERVAL_S,
                        help="seconds between tree samples (default 0.1)")
    parser.add_argument("--log", type=Path, help="write the command's output here")
    parser.add_argument("--json", type=Path, help="also write the result here as JSON")
    parser.add_argument("command", nargs=argparse.REMAINDER,
                        help="-- then the command to run")
    args = parser.parse_args(argv)
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command:
        parser.error("no command: give it after --")
    if args.log:
        with args.log.open("w") as handle:
            result = measure(command, args.interval, stdout=handle)
    else:
        result = measure(command, args.interval)
    print(summary_line(result), flush=True)
    if args.json:
        args.json.write_text(json.dumps(result, indent=2) + "\n")
    code = result["exit_code"]
    return 128 - code if code < 0 else code  # killed by signal N: the shell's 128 + N


if __name__ == "__main__":
    sys.exit(main())
