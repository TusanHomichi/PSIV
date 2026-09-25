"""What a receipt's evidence is kept as: its bytes, its index, its scratch names.

The finalize step copies the worktree's `build/lane-evidence/` into a run's
receipt as `run-N/evidence/`. This module is how those files are stored and
described - hashing a file's content, compressing it to `name.ext.xz`, sharing
its bytes with an earlier run by hardlink, reading and writing the index beside
it - while `compaction` decides what to do with each file.

The rules that make a pass safe live here:

* a file is written under a `.ds-lane.part` scratch name and renamed into
  place, so an interrupted pass leaves the original, the finished `.xz` or
  both, never a truncated `.xz` where a reader would look for evidence;
* the index is written by rename too, so a reader sees a whole index or the old
  one, and `INDEX.json` describes the files beside it: each one's path, the
  size and sha256 of its content as captured, whether that content is stored
  `plain`, `xz` or as a `hardlink` to an earlier run, the bytes the path
  occupies, and for a link where its storage comes from;
* a hardlink is recorded only when the link was made, so an entry never claims
  shared storage that did not happen.
"""
import hashlib
import json
import lzma
import os
from pathlib import Path

from .config import BINARY_SNIFF_BYTES, COMPRESS_PRESET

INDEX_NAME = "INDEX.json"      # evidence/INDEX.json, written by every pass
PART_SUFFIX = ".ds-lane.part"  # scratch name for a write that is not in place yet
XZ_SUFFIX = ".xz"
READ_CHUNK = 1 << 20


# ------------------------------------------------------------------ content

def content_of(path):
    """(sha256, bytes, binary) for a file, read in one pass.

    `binary` is the harness's own sniff (a NUL byte in the first 8 KiB): a blob
    is never compressed. The hash covers the whole file, so a reader can check
    a decompressed `.xz` against the sha256 its index records.
    """
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        head = handle.read(BINARY_SNIFF_BYTES)
        digest.update(head)
        size = len(head)
        while chunk := handle.read(READ_CHUNK):
            digest.update(chunk)
            size += len(chunk)
    return digest.hexdigest(), size, b"\0" in head


def xz_content(path):
    """(sha256, bytes) of what a `.xz` file holds, or None when it is not one."""
    digest, size = hashlib.sha256(), 0
    try:
        with lzma.open(path, "rb") as handle:
            while chunk := handle.read(READ_CHUNK):
                digest.update(chunk)
                size += len(chunk)
    except (lzma.LZMAError, EOFError, OSError):
        return None
    return digest.hexdigest(), size


def xz_size(path):
    """Bytes this file's compressed stream takes, counted without storing it.

    A dry run reports the bytes it would save, so it has to know what the
    compressor would produce; the output is counted and dropped rather than
    written (the CPU of a real pass, no disk and no change to the receipt).
    """
    total, compressor = 0, lzma.LZMACompressor(preset=COMPRESS_PRESET)
    with open(path, "rb") as handle:
        while chunk := handle.read(READ_CHUNK):
            total += len(compressor.compress(chunk))
        total += len(compressor.flush())
    return total


def compress_file(src, dst, dry_run=False):
    """Write `src`'s LZMA stream (preset 6) to `dst`; returns the bytes stored.

    The stream is complete before the scratch name is renamed onto `dst`, so an
    interruption leaves the original, the finished `.xz` or both - never a
    truncated `.xz` where a reader would look for evidence.
    """
    if dry_run:
        return xz_size(src)
    scratch = Path(str(dst) + PART_SUFFIX)
    with open(src, "rb") as handle, open(scratch, "wb") as out:
        compressor = lzma.LZMACompressor(preset=COMPRESS_PRESET)
        while chunk := handle.read(READ_CHUNK):
            out.write(compressor.compress(chunk))
        out.write(compressor.flush())
    os.replace(scratch, dst)
    return dst.stat().st_size


def same_file(one, other):
    """True when both paths are the same inode on the same filesystem."""
    try:
        a, b = os.stat(one), os.stat(other)
    except OSError:
        return False
    return (a.st_dev, a.st_ino) == (b.st_dev, b.st_ino)


def link_file(src, dst, dry_run=False):
    """Share `src`'s bytes at `dst`; False when the link did not happen.

    The link is made under the scratch name and renamed onto `dst`, so a reader
    never sees `dst` missing or half-made, and a failed link (a different
    filesystem, a donor that is gone) leaves `dst` exactly as it was: the
    caller then compresses or keeps its own copy. A dry run only compares the
    two filesystems, which is the failure this fallback exists for; a donor
    that does not exist yet is one this pass would write itself, so its
    directory answers instead.
    """
    src, dst = Path(src), Path(dst)
    if dry_run:
        try:
            return os.stat(src).st_dev == os.stat(dst.parent).st_dev
        except OSError:
            try:
                return os.stat(src.parent).st_dev == os.stat(dst.parent).st_dev
            except OSError:
                return False
    scratch = Path(str(dst) + PART_SUFFIX)
    scratch.unlink(missing_ok=True)
    try:
        os.link(src, scratch)
    except OSError:
        return False
    os.replace(scratch, dst)
    return True


def share_the_bytes(path, share, dry_run):
    """True when `path` holds `share`'s bytes after this call: linked, or linked now."""
    return same_file(path, share[0]) or link_file(share[0], path, dry_run)


# -------------------------------------------------------------------- index

def read_index(evidence):
    """{path: entry} from `evidence/INDEX.json`, empty when there is none.

    A receipt finalized before compaction existed has no index, and a malformed
    one is treated the same way: the pass then measures every file itself.
    """
    try:
        data = json.loads((Path(evidence) / INDEX_NAME).read_text())
        entries = data["files"]
    except (OSError, ValueError, KeyError, TypeError):
        return {}
    return {e["path"]: e for e in entries
            if isinstance(e, dict) and isinstance(e.get("path"), str)}


def write_index(evidence, run_number, entries, dry_run=False):
    """Write `evidence/INDEX.json` unless it already says exactly this.

    Written by rename, so a reader sees a whole index or the old one, and left
    alone when the content has not changed: that is what makes a second pass
    leave a receipt untouched, mtimes included. Returns True when it changed.
    """
    target = Path(evidence) / INDEX_NAME
    body = json.dumps({
        "run": run_number,
        "original_bytes": sum(e["size"] for e in entries),
        "stored_bytes": sum(0 if e["stored"] == "hardlink" else e["stored_size"] for e in entries),
        "files": entries,
    }, indent=2) + "\n"
    if target.exists() and target.read_text() == body:
        return False
    if not dry_run:
        scratch = Path(str(target) + PART_SUFFIX)
        scratch.write_text(body)
        os.replace(scratch, target)
    return True


def entry_for(rel, sha, size, stored, stored_size, link=None):
    """One index entry: the path, its content, how it is stored and the bytes it takes."""
    entry = {"path": rel, "size": size, "sha256": sha, "stored": stored, "stored_size": stored_size}
    if link is not None:
        entry["link"] = link
    return entry


def xz_entry(evidence, rel, sha, size, measured=None):
    """The entry for a path stored as a compressed stream this pass owns."""
    stored_size = measured if measured is not None else (Path(evidence) / rel).stat().st_size
    return entry_for(rel, sha, size, "xz", stored_size)


def hardlink_entry(rel, share, sha, size):
    """The entry for a path whose bytes are the same storage as an earlier run's."""
    try:
        stored_size = share[0].stat().st_size
    except OSError:  # a donor that went away: nothing to report for it
        stored_size = 0
    return entry_for(rel, sha, size, "hardlink", stored_size, link=share[1])


def prev_form(entry, forms):
    """The form a previous pass stored `entry`'s content in, or None when unknown.

    An `xz` or `plain` entry says it itself; a `hardlink` entry says it through
    the run its `link` names, whose form an oldest-first scan already recorded.
    """
    stored = entry.get("stored")
    if stored in ("plain", "xz"):
        return stored
    if stored == "hardlink":
        return forms.get(entry.get("link"))
    return None


# ------------------------------------------------------------------ one run

def evidence_files(evidence):
    """Every evidence file, name-ordered, without the index and without scratch.

    A `.ds-lane.part` name is this harness's own unfinished write and never
    evidence; the index describes the files beside it and is not one of them.
    """
    evidence = Path(evidence)
    return [p for p in sorted(evidence.rglob("*"))
            if p.is_file() and not p.name.endswith(PART_SUFFIX)
            and not (p.parent == evidence and p.name == INDEX_NAME)]


def clear_parts(evidence, dry_run, notes):
    """Remove the scratch files interrupted passes left; report how many there were.

    They are unfinished writes of this harness - a compressed stream that never
    reached its name, a link that was never renamed - so removing one costs
    nothing: the file it was made from is still there.
    """
    parts = [p for p in Path(evidence).rglob("*")
             if p.is_file() and p.name.endswith(PART_SUFFIX)]
    if parts:
        notes.append(f"{'removed' if not dry_run else 'would remove'} {len(parts)} "
                     f"interrupted write(s)")
        if not dry_run:
            for path in parts:
                path.unlink(missing_ok=True)
    return len(parts)


def compressible(rel, size, binary, min_bytes, exts):
    """Whether this file is one the pass compresses: big, text, not already `.xz`."""
    if size < min_bytes or binary or rel.endswith(XZ_SUFFIX):
        return False
    return Path(rel).suffix.lower().lstrip(".") in exts


# ---------------------------------------------------------------- measuring

def linked_targets(runs):
    """Paths some run's index links to: another run's storage, left where it is.

    Compressing one of these would keep the shared bytes alive *and* write a
    compressed copy, which costs space instead of saving it, so a pass leaves
    them plain.
    """
    out = set()
    for run_dir in runs:
        for entry in read_index(Path(run_dir) / "evidence").values():
            if entry.get("stored") == "hardlink" and isinstance(entry.get("link"), str):
                out.add(entry["link"])
    return out


def measure_store(runs):
    """Bytes the lane's evidence really occupies: shared storage counted once."""
    seen, total = set(), 0
    for run_dir in runs:
        for path in evidence_files(Path(run_dir) / "evidence"):
            try:
                st = path.stat()
            except OSError:
                continue
            if (st.st_dev, st.st_ino) not in seen:
                seen.add((st.st_dev, st.st_ino))
                total += st.st_size
    return total
