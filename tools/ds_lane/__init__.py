"""ds-lane: a DeepSeek worker (via Reasonix) in an isolated git worktree lane.

The implementation is split by cohesion - `config` (paths, environment,
constants), `confine` (the read boundary a worker and its children run under),
`preflight` (brief phrasing and write sets), `trajectory` (what a run left to
read), `receipts` (a finished run's record and the wait chain), `evidence` (how
a receipt's evidence is stored and described), `compaction` (deduplicating and
compressing it), `report` (the lines and the run.json block a pass leaves),
`lanes` (the lane commands and the run-launch path), `verify` (the
orchestrator's own checks as evidence), `supervisor` (slot, worker process
group, watchdog, exec) and `cli` (arguments) - and is stdlib-only.
`tools/ds-lane`, the executable entry point, exposes `main`. Usage:
`ds-lane --help`, or the `cli` module docstring.

This module also re-exports the pieces the test suite and ad-hoc tooling use as
`ds_lane.<name>`, so they need not know the module split.
"""
from .compaction import compact_finished_run, compact_run, compact_runs, scan_run
from .confine import (bwrap_argv, bwrap_path, confine_home, lane_reasonix_home, local_bin_targets,
                      node_prefix, read_paths, require_bwrap, seed_names, seed_reasonix_home, summary,
                      wrap, worker_roots)
from .config import (BINARY_SNIFF_BYTES, CARGO_JOBS, COMPRESS_PRESET, DEFAULT_COMPRESS_EXTS,
                     DEFAULT_COMPRESS_MIN_BYTES, DEFAULT_SIZE_EXEMPT, DEFAULT_STALL_CPU_PCT,
                     DEFAULT_STALL_RETRIES, DEFAULT_STALL_TIMEOUT, DEFAULT_TIMEOUT, EFFORT, ENTRY,
                     HOST_STATE_PATHS, KILL_GRACE, MAX_FILE_LINES, MAX_LANES, MODEL,
                     PERMISSION_MODE, STALL_EXIT, STALL_POLL, STATE_ROOT, STOPPED_EXIT, STOP_WAIT,
                     TIMEOUT_EXIT, WORKER_ENV_ALLOW, WT_ROOT, compress_exts, compress_min_bytes, ds_lane_home,
                     lane_paths, load_receipt, max_file_lines, max_lanes, now, pid_alive,
                     reasonix_bin, repo_root, save_receipt, sh, size_exempt, stall_cpu_pct,
                     stall_poll, state_root, supervisor_alive, worker_env, wt_root)
from .evidence import (INDEX_NAME, PART_SUFFIX, XZ_SUFFIX, compress_file, content_of,
                       evidence_files, link_file, read_index, same_file, write_index,
                       xz_content, xz_size)
from .preflight import (CONSTRAINT_BLOCK, PREAMBLE, READ_ONLY_CLAUSE, parse_write_set,
                        phrasing_violations, preflight_phrasing, strip_code, write_set_match,
                        write_set_violations)
from .report import human, print_report, record_compaction
from .receipts import (FinalizeError, commit_excludes, commit_run, count_lines, finalize_run,
                       git_step, head_sha, inflight_runs, is_ignored, lines_at, oversize_files,
                       print_summary, record_crashed_run, run_finished, wait_run)
from .supervisor import STALL_FOLLOWUP, StallWatcher, exec_run, group_ticks, run_worker
from .trajectory import receipt_block, result_of, scan_trajectory, trajectory_tail
from .verify import cmd_verify
from .lanes import lane_runs, latest_session, next_run_number, run_dirs
