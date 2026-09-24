"""ds-lane: a DeepSeek worker (via Reasonix) in an isolated git worktree lane.

The implementation is split by cohesion - `config` (paths, environment,
constants), `preflight` (brief phrasing and write sets), `receipts` (a
finished run's record and the wait chain), `lanes` (the lane commands and the
run-launch path), `supervisor` (slot, worker process group, watchdog, exec) and
`cli` (arguments) - and is stdlib-only. `tools/ds-lane`, the executable entry
point, exposes `main`. Usage: `ds-lane --help`, or the `cli` module docstring.

This module also re-exports the pieces the test suite and ad-hoc tooling use as
`ds_lane.<name>`, so they need not know the module split.
"""
from .config import (BINARY_SNIFF_BYTES, CARGO_JOBS, DEFAULT_STALL_CPU_PCT,
                     DEFAULT_STALL_RETRIES, DEFAULT_STALL_TIMEOUT, DEFAULT_TIMEOUT, EFFORT, ENTRY,
                     HOST_STATE_PATHS, KILL_GRACE, MAX_FILE_LINES, MAX_LANES, MODEL,
                     PERMISSION_MODE, STALL_EXIT, STALL_POLL, STATE_ROOT, STOPPED_EXIT, STOP_WAIT,
                     TIMEOUT_EXIT, WT_ROOT, ds_lane_home, lane_paths, load_receipt, max_file_lines,
                     max_lanes, now, pid_alive, reasonix_bin, repo_root, save_receipt, sh,
                     stall_cpu_pct, stall_poll, state_root, supervisor_alive, wt_root)
from .preflight import (CONSTRAINT_BLOCK, PREAMBLE, READ_ONLY_CLAUSE, parse_write_set,
                        phrasing_violations, preflight_phrasing, strip_code, write_set_match,
                        write_set_violations)
from .receipts import (count_lines, finalize_run, lines_at, oversize_files, print_summary,
                       receipt_block, run_finished, scan_trajectory, wait_run)
from .supervisor import STALL_FOLLOWUP, StallWatcher, exec_run, group_ticks, run_worker
