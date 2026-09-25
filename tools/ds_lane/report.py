"""What a compaction pass reports: the caller's lines, and the run's record.

The pass itself is `compaction`; this is the two shapes its report takes. The
lines `ds-lane compact ID` prints say what happened - or, with `--dry-run`, what
would - per run and for the lane, and `record_compaction` writes the same
figures into the finished run's `run.json` beside its own record, where
`compaction_error` is what a reader looks for when a pass went wrong.

`human` renders bytes the way the summary's sizes read: 1.5 GiB, 12.3 MiB, 900
B. The report is a plain dict assembled by `compaction.compact_runs` - the
per-run counts and notes, the lane's byte totals and its errors - so nothing
here depends on how a pass was run.
"""
import json
from pathlib import Path

RECORDED = ("files", "linked", "compressed", "kept", "removed_parts", "skipped")  # run.json's view


def human(count):
    """Bytes as a short human string (1.5 GiB, 12.3 MiB, 900 B)."""
    for unit, step in (("GiB", 1 << 30), ("MiB", 1 << 20), ("KiB", 1 << 10)):
        if count >= step:
            return f"{count / step:.1f} {unit}"
    return f"{count} B"


def record_compaction(run_dir, report):
    """Note a run's compaction in its run.json, after the fact and by rename.

    run.json is what marks a run complete, so this updates the record a reader
    may already hold rather than replacing it: written under a temporary name
    and renamed into place. `compaction_error` is the pass's failures, and None
    when it was clean.
    """
    path = Path(run_dir) / "run.json"
    run = json.loads(path.read_text())
    run["compaction"] = {key: report.get(key) for key in RECORDED}
    run["compaction"]["notes"] = report.get("notes") or []
    run["compaction"]["bytes_before"] = report.get("bytes_before")
    run["compaction"]["bytes_after"] = report.get("bytes_after")
    run["compaction_error"] = "; ".join(report.get("errors") or []) or None
    scratch = path.with_name("run.json.tmp")
    scratch.write_text(json.dumps(run, indent=2) + "\n")
    scratch.replace(path)


def print_report(report):
    """Print what a pass did (or would do): one line per run, one for the lane."""
    lane = report.get("lane") or "lane"
    dry = " (dry run)" if report["dry_run"] else ""
    exts = "/".join(f".{e}" for e in report["exts"]) or "no extensions"
    print(f"{lane}{dry}: {len(report['runs'])} run(s); {exts} at or over "
          f"{human(report['min_bytes'])} are stored as .xz")
    for run in report["runs"]:
        if run["skipped"]:
            print(f"  run-{run['run']}: skipped ({run['skipped']})")
            continue
        line = (f"  run-{run['run']}: {run['files']} file(s): {run['linked']} linked, "
                f"{run['compressed']} compressed, {run['kept']} kept")
        if run["removed_parts"]:
            line += f", {run['removed_parts']} interrupted write(s) removed"
        print(line)
        for note in run["notes"]:
            print(f"    note: {note}")
    saved = report["saved"]
    if saved >= 0:
        change = f"{'would save' if dry else 'saved'} {human(saved)}"
    else:  # a pass never aims for this: it is reported rather than hidden
        change = f"{'would grow' if dry else 'grew'} by {human(-saved)}"
    print(f"  {'would store' if dry else 'stored'} {human(report['bytes_after'])} instead of "
          f"{human(report['bytes_before'])}: {change}")
    shown = {note for run in report["runs"] for note in run["notes"]}
    for note in report["notes"]:  # a run's own notes are printed under its line
        if note not in shown:
            print(f"  note: {note}")
    for error in report["errors"]:
        print(f"  ERROR: {error}")
    return report
