"""Deduplicate and compress the evidence a finished run leaves in its receipt.

The finalize step copies the worktree's `build/lane-evidence/` into the run's
receipt as `run-N/evidence/`. Across a lane those copies mostly repeat each
other - run 2 re-copies what run 1 held - and the raw captures in them (CSV
logs, traces) are large and highly repetitive, so one sweep lane's receipts
reached 8.7 GB (owner decision 2026-09-24: deduplicate and compress; pruning is
a later lane). Two passes, oldest run first:

1. **Dedupe.** A file whose content an earlier run of the same lane already
   keeps becomes a hardlink to that copy, so every path stays readable exactly
   as before and the bytes are stored once. A link that fails - across
   filesystems, say - falls back to compressing or copying, and an entry is
   recorded from what actually happened at the path, never from the attempt.
2. **Compress.** A file of at least `DS_LANE_COMPRESS_MIN_BYTES` (default
   1 MiB) whose extension is in `DS_LANE_COMPRESS_EXTS` (default .csv .log
   .jsonl .txt .tsv) is written to `name.ext.xz` with `lzma` preset 6 and the
   original is removed. Files already compressed, binaries and files under the
   threshold are left alone. An earlier run's compressed copy of the same
   content is what this run would compress to, so the pass links to it instead
   of spending CPU of its own: run 2's `traces.csv.xz` is run 1's, by hardlink.

Order matters: a file an earlier run stores plain is linked rather than
compressed, and only bytes no earlier run holds are compressed, so a pass never
spends CPU on what it can share. A file another run links to is left as it is
too: compressing it would keep the shared bytes alive *and* write the
compressed copy, which costs space instead of saving it.

`evidence/INDEX.json` records what happened to each file - its path, the size
and sha256 of its content as captured, whether it is stored `plain`, `xz` or as
a `hardlink` to an earlier run, and the bytes it takes - and is what makes a
second pass a no-op: an already-compressed file is not compressed again, an
already-linked file is not copied again, and an unchanged index is not
rewritten. How those files are written - and that an interrupted pass leaves
the original, the finished `.xz` or both, never neither - is `evidence`'s
business.

Compaction runs off the critical path: the supervisor starts it after the run's
result is recorded and its machine-wide slot is released
(`compact_finished_run`), so it never delays another lane's start, and a
failure is noted in `run.json` as `compaction_error` without disturbing the
rest of the receipt. `ds-lane compact ID` applies the same passes to a lane's
existing receipts, including lanes finalized before this existed, refusing a
lane with a run in flight; it is idempotent, and `--dry-run` reports what it
would do and the bytes it would save without changing anything.
"""
import fcntl
import json
import lzma
from pathlib import Path

from .config import compress_exts, compress_min_bytes
from .evidence import (XZ_SUFFIX, clear_parts, compress_file, compressible, content_of,
                       entry_for, evidence_files, hardlink_entry, linked_targets, measure_store,
                       prev_form, read_index, share_the_bytes, write_index, xz_content, xz_entry)
from .report import record_compaction

LOCK_NAME = "compaction.lock"  # one compaction per run directory at a time


# ------------------------------------------------------------------- donors

def add_donors(donors, forms, run_dir, entries):
    """Record one run's entries as the link targets of the runs after it.

    Keyed by the sha256 of the content, with the oldest run that holds it
    winning, so every pass picks the same donor and a second pass records the
    same link. Content is looked up by form: a plain copy can be linked onto a
    name directly, a compressed one only onto the `.xz` name this pass would
    use. An entry whose link cannot be resolved - an index edited by hand - is
    left out: a donor this pass cannot describe is one it must not link to.
    """
    for e in entries:
        form = e.get("stored")
        if form == "hardlink":
            form = forms.get(e.get("link"))
        if form not in ("plain", "xz"):
            continue
        display = f"{Path(run_dir).name}/evidence/{e['path']}"
        donors[form].setdefault(e["sha256"], (Path(run_dir) / "evidence" / e["path"], display))
        forms[display] = form


def scan_run(donors, forms, run_dir, *, lock=True):
    """Add a run's evidence to the donor maps, from its index or by measuring it.

    Runs are scanned oldest first, so a link always points at a run whose form
    is already known. A run finalized before compaction existed has no index to
    read, and its files are hashed as they are on disk: what it captured is its
    content. Returns notes for the report. The run is held shared while it is
    read, so no other pass moves a file out from under the map; a dry run takes
    no lock, because it may not leave a lock file in a receipt it reports on.
    """
    run_dir, notes = Path(run_dir), []
    evidence = run_dir / "evidence"
    if not evidence.is_dir():
        return notes
    if not lock:  # a dry run reads without locking, and leaves no lock file
        add_run(donors, forms, run_dir)
        return notes
    hold = lock_run_dir(run_dir, exclusive=False, blocking=False)
    if hold is None:  # a pass is working here: wait for it, its files are moving
        notes.append(f"{run_dir.name}: another compaction holds it; waiting for it")
        hold = lock_run_dir(run_dir, exclusive=False, blocking=True)
    if hold is None:
        notes.append(f"{run_dir.name}: not scanned, its compaction lock is unavailable")
        return notes
    try:
        add_run(donors, forms, run_dir)
    finally:
        hold.close()
    return notes


def add_run(donors, forms, run_dir):
    """Add one run's evidence to the donor maps: from its index, or by measuring it."""
    evidence = Path(run_dir) / "evidence"
    entries = read_index(evidence)
    if entries:
        add_donors(donors, forms, run_dir, entries.values())
    else:
        add_captured(donors, forms, run_dir)


def add_captured(donors, forms, run_dir):
    """Record a run's files as they stand: what it captured is its content.

    A receipt finalized before compaction existed has no index to read, so its
    files are hashed and entered as plain copies of exactly those bytes.
    """
    evidence = Path(run_dir) / "evidence"
    for path in evidence_files(evidence):
        sha, _size, _binary = content_of(path)
        display = f"{Path(run_dir).name}/evidence/{path.relative_to(evidence).as_posix()}"
        donors["plain"].setdefault(sha, (path, display))
        forms[display] = "plain"


# ------------------------------------------------------------------ one run


def lock_run_dir(run_dir, *, exclusive, blocking):
    """Hold this run's compaction lock, or return None when someone else does.

    One pass per run directory: two passes over the same files would fight over
    names and could record a link to a file the other pass is moving. A run
    being compacted is held exclusive for the whole pass, a donor run shared
    for its scan, and locks are always taken own-run first and donors - older
    runs - after, so the two orders cannot close a cycle.
    """
    handle = open(Path(run_dir) / LOCK_NAME, "a")  # truncating would touch the mtime
    flags = (fcntl.LOCK_EX if exclusive else fcntl.LOCK_SH) | (0 if blocking else fcntl.LOCK_NB)
    try:
        fcntl.flock(handle, flags)
    except OSError:  # BlockingIOError: another compaction holds this run
        handle.close()
        return None
    return handle


def compact_file(evidence, rel, prev_entry, donors, forms, linked_to, min_bytes, exts, dry_run):
    """Dedupe and, where it pays, compress one evidence file.

    Returns (entry, consumed, notes): the file's index entry, the path of a
    second file this call settled (the `.xz` an interrupted pass had already
    written beside its original, so the caller does not index it twice) and
    anything worth telling the reader.
    """
    path, notes, consumed = Path(evidence) / rel, [], None
    run_name = Path(evidence).parent.name  # the receipt this file belongs to
    form = prev_form(prev_entry, forms) if prev_entry else None
    if form:
        sha, size, binary = prev_entry["sha256"], prev_entry["size"], False
    else:
        sha, size, binary = content_of(path)
        form = "plain"

    if form == "xz":  # already the compressed form: share it if an older run has it
        share = donors["xz"].get(sha)
        if share and share_the_bytes(path, share, dry_run):
            return hardlink_entry(rel, share, sha, size), None, notes
        return xz_entry(evidence, rel, sha, size), None, notes

    share = donors["plain"].get(sha)
    if share and share_the_bytes(path, share, dry_run):  # an older run keeps these bytes
        return hardlink_entry(rel, share, sha, size), None, notes

    display = f"{run_name}/evidence/{rel}"
    if not compressible(rel, size, binary, min_bytes, exts):
        return entry_for(rel, sha, size, "plain", size), None, notes
    if display in linked_to:  # another run's copy is this storage: leave it where it is
        notes.append(f"{rel}: kept plain, another run links to it")
        return entry_for(rel, sha, size, "plain", size), None, notes

    # store the compressed form, at the name this pass would give it
    xz_rel = rel + XZ_SUFFIX
    xz_path = evidence / xz_rel
    original = path
    if xz_path.exists():
        if xz_content(xz_path) != (sha, size):
            # not a compressed copy of this file: both are somebody's evidence,
            # so both stay exactly where they are
            notes.append(f"{rel}: {xz_rel} is a different file, so both stay")
            return entry_for(rel, sha, size, "plain", size), None, notes
        # an interrupted pass compressed this file and never removed the
        # original; the `.xz` holds the same content, so the plain copy goes
        notes.append(f"{rel}: kept the .xz an interrupted pass wrote")
        consumed, original = xz_rel, None
        if not dry_run:
            path.unlink()
    share = donors["xz"].get(sha)
    if share and share_the_bytes(xz_path, share, dry_run):
        if original is not None and not dry_run:
            original.unlink()
        return hardlink_entry(xz_rel, share, sha, size), consumed, notes
    if original is None:  # that interrupted `.xz` is the stored form; nothing to compress
        return xz_entry(evidence, xz_rel, sha, size), consumed, notes
    measured = compress_file(path, xz_path, dry_run)
    if not dry_run:
        original.unlink()
    return xz_entry(evidence, xz_rel, sha, size, measured=measured), consumed, notes


def compact_run(run_dir, donors, forms, linked_to, *, min_bytes, exts, dry_run=False):
    """Dedupe and compress one run's evidence; returns (report, entries).

    Every file the run holds at the start gets one entry, whether the pass
    touched it or not, so the index always describes the directory beside it -
    only a file this pass could not read is left out, and the report says so.
    A failure on one file is never the end of the pass: the rest of the receipt
    still gets compacted.
    """
    run_dir = Path(run_dir)
    run_number = run_number_or_zero(run_dir) or None
    report = {"run": run_number, "files": 0, "linked": 0, "compressed": 0, "kept": 0,
              "bytes_before": 0, "bytes_after": 0, "removed_parts": 0, "notes": [], "errors": [],
              "skipped": None}
    evidence = run_dir / "evidence"
    if not evidence.is_dir():
        report["skipped"] = "no evidence"
        return report, []
    hold = None
    if not dry_run:  # a dry run reports and changes nothing, lock file included
        hold = lock_run_dir(run_dir, exclusive=True, blocking=False)
        if hold is None:
            report["skipped"] = "another compaction holds this run"
            return report, []
    try:
        prev = read_index(evidence)
        report["removed_parts"] = clear_parts(evidence, dry_run, report["notes"])
        entries, consumed = [], set()
        for path in evidence_files(evidence):
            rel = path.relative_to(evidence).as_posix()
            if rel in consumed:
                continue
            report["files"] += 1
            before = prev.get(rel)
            try:
                # what this run's own evidence cost in bytes: a copy another run
                # shares already costs this receipt nothing
                report["bytes_before"] += path.stat().st_size \
                    if (before or {}).get("stored") != "hardlink" else 0
                entry, also, notes = compact_file(evidence, rel, before, donors, forms, linked_to,
                                                  min_bytes, exts, dry_run)
            except (OSError, lzma.LZMAError) as e:  # one bad file is not the end of the pass
                report["errors"].append(f"{rel}: {e}")
                try:  # describe the file as it stands: it is still evidence
                    sha, size, _binary = content_of(path)
                    entries.append(entry_for(rel, sha, size, "plain", size))
                    report["kept"] += 1
                except OSError:  # unreadable: the one thing the index cannot describe
                    pass
                continue
            entries.append(entry)
            report["notes"] += notes
            if also:
                consumed.add(also)
            was = prev_form(before, forms) if before else None
            if entry["stored"] == "hardlink":
                report["linked"] += 1
            elif entry["stored"] == "xz" and was != "xz":
                report["compressed"] += 1
            else:
                report["kept"] += 1
            report["bytes_after"] += 0 if entry["stored"] == "hardlink" else entry["stored_size"]
        entries.sort(key=lambda e: e["path"])
        write_index(evidence, run_number, entries, dry_run)
        return report, entries
    finally:
        if hold is not None:
            hold.close()


# ------------------------------------------------------------------- a lane

def compact_runs(lane, runs, *, only=None, dry_run=False, min_bytes=None, exts=None):
    """Dedupe and compress a lane's runs oldest first; returns the lane's report.

    Donors come from the runs already passed, so a lane is compacted in its own
    order: run 2 links to run 1 whether or not run 1 had been indexed before.
    `only` restricts the pass to one run's number - the finalize path compacting
    the run that just finished - while every older run is still read as a donor
    source. `bytes_before` and `bytes_after` measure what the lane's receipts
    really occupy, shared storage counted once; a dry run cannot measure the
    second one, so it reports what the entries the pass would leave add up to.
    """
    min_bytes = compress_min_bytes() if min_bytes is None else min_bytes
    exts = compress_exts() if exts is None else exts
    ordered = sorted((Path(r) for r in runs), key=lambda p: run_number_or_zero(p))
    report = {"lane": lane.get("id"), "dry_run": dry_run, "runs": [], "notes": [], "errors": [],
              "files": 0, "linked": 0, "compressed": 0, "kept": 0,
              "min_bytes": min_bytes, "exts": list(exts), "only": only}
    report["bytes_before"] = measure_store(ordered)
    donors, forms = {"plain": {}, "xz": {}}, {}
    linked_to = linked_targets(ordered)
    projected, all_runs = 0, True
    for run_dir in ordered:
        if only is not None and run_number_or_zero(run_dir) != only:
            report["notes"] += scan_run(donors, forms, run_dir, lock=not dry_run)
            all_runs = False
            continue
        run_report, entries = compact_run(run_dir, donors, forms, linked_to,
                                          min_bytes=min_bytes, exts=exts, dry_run=dry_run)
        report["runs"].append(run_report)
        report["notes"] += run_report["notes"]
        report["errors"] += [f"run-{run_report['run']} {e}" for e in run_report["errors"]]
        for key in ("files", "linked", "compressed", "kept"):
            report[key] += run_report[key]
        add_donors(donors, forms, run_dir, entries)
        # storage one pass over the whole lane leaves: every file is either its
        # own bytes or a link to an earlier run's, which cost nothing here
        projected += sum(0 if e["stored"] == "hardlink" else e["stored_size"] for e in entries)
    report["bytes_after"] = projected if dry_run and all_runs else measure_store(ordered)
    report["saved"] = report["bytes_before"] - report["bytes_after"]
    return report


def run_number_or_zero(run_dir):
    """The `N` of a `run-N` directory, or 0 for a name that is not a run."""
    tail = Path(run_dir).name.split("-", 1)[-1]
    return int(tail) if tail.isdigit() else 0


def compact_finished_run(lane, run_dir, runs):
    """Compact one finished run, off the critical path; never raises.

    The supervisor calls this after the run's record has landed and its
    machine-wide slot has been released, so a pass that takes minutes costs
    this lane alone and never another lane's start. Only this run's evidence is
    compacted - earlier runs are read as donor sources and left for
    `ds-lane compact ID`. Whatever goes wrong lands in run.json as
    `compaction_error`, because the evidence is readable either way, and this
    must not raise: the run it belongs to has succeeded, and an exception here
    would have the supervisor mark it as a crashed run.
    """
    number = run_number_or_zero(run_dir)
    run_report = {"run": number, "files": 0, "linked": 0, "compressed": 0, "kept": 0,
                  "removed_parts": 0, "notes": [], "errors": [], "skipped": None}
    before = after = None
    try:
        report = compact_runs(lane, runs, only=number)
        before, after = report["bytes_before"], report["bytes_after"]
        run_report = next((r for r in report["runs"] if r["run"] == number), run_report)
    except Exception as e:  # a pass that cannot run at all costs the run nothing
        run_report["errors"].append(f"compaction failed: {e!r}")
    errors = list(run_report.get("errors") or [])
    skipped = run_report.get("skipped")
    if skipped and skipped != "no evidence":  # say why nothing was compacted
        errors.append(skipped)
    try:
        record_compaction(run_dir, {**run_report, "errors": errors,
                                    "bytes_before": before, "bytes_after": after})
    except (OSError, ValueError) as e:  # the run's record is on disk and stands
        print(f"ds-lane: could not record the compaction of {run_dir}: {e}", flush=True)
    return run_report
