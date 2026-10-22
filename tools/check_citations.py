#!/usr/bin/env python3
"""Check retail `ps4.asm:N` citations against the local disassembly (issue #158).

    python3 tools/check_citations.py [--disasm PATH]

The disassembly is a local input (`reference/ps4disasm/ps4.asm`); on a public
clone the check says it was skipped and passes. When it is present, every
citation in the change's text files (every readable file without a NUL
byte, whatever its suffix) is read:

- `ps4.asm:N` and `ps4.asm:N-M` (or `N..M`) must name existing lines
  (`1..line count`), as must the compact `:N` that follows one in the same
  file (`ps4.asm:N` and `:M`; `Label`, `:N`);
- where a label is named beside the citation (the forms in use: a backticked
  label then the citation, `Label (ps4.asm:N)`, `ps4.asm:N (Label)`,
  `Label, :N`), the citation must sit inside the routine the label opens or
  name a line that *uses* the label (the cited lines mention it, e.g. a
  `jsr`). A routine runs from its label to the next separator comment or the
  next label that is not a branch target inside it (`labels`). A citation that
  neither defines nor uses the label beside it is a mismatch;
- a citation without a neighbouring label is counted unverified, not failed;
  the count is printed.

The label beside a citation is the word right before it, so prose puts each
label next to its own line, `` `A` (`ps4.asm:N`) and `B` (`ps4.asm:M`) ``;
`` `A` and `B` (`ps4.asm:N`, `ps4.asm:M`) `` reads `B` against N. A citation
split across two lines is unverified.

Exit 1 on a mismatch. Only label names and line numbers are printed; the
disassembly's content never leaves the process. `check_docs.py` runs this
after its other checks, prints its summary line (`citations: skipped` on a
clone without the disassembly) and folds the mismatches into its exit code.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

try:  # imported as `tools.check_citations` by tests and check_docs
    from tools.repo_files import repo_files
except ModuleNotFoundError:  # direct `python3 tools/check_citations.py`
    from repo_files import repo_files

DISASM = Path("reference") / "ps4disasm" / "ps4.asm"

#: A global label in the disassembly: column 0, name, colon.
LABEL = re.compile(r"^([A-Za-z_][A-Za-z0-9_]*):")
#: A label that names a branch target inside a routine, not a routine: the
#: disassembly's anonymous `loc_<address>` and its named loops.
LOCAL = re.compile(r"^loc_[0-9A-Fa-f]+$|Loop$")
#: The comment rule the disassembly puts between routines.
SEPARATOR = re.compile(r"^;\s*[-=]{10,}")
#: A citation: `ps4.asm:N`, `ps4.asm:N-M` or `ps4.asm:N..M`, or the compact
#: follow-on `:N` / `:N-M` (`ps4.asm:147452` and `:147483`; `loc_646DA`, `:134274-134292`)
#: after a space, a backtick, a bracket, a comma or a semicolon. The compact
#: form counts only in a file that also cites `ps4.asm:N`, so a `:12`
#: elsewhere is not read as one.
CITATION = re.compile(r"(?:(?P<full>ps4\.asm)|(?<=[\s`(,;])|^):(\d+)(?:(?:-|\.\.)(\d+))?(?![\d:])")

#: A citation may carry the disassembly's path (`reference/ps4disasm/ps4.asm:N`);
#: `\b` keeps the path from starting inside the label before it.
HEAD = r"(?:(?:\b[\w.-]+/)*ps4\.asm)?"
#: A label before a citation: a whole word, so the `asm` of `ps4.asm:N` and a
#: path segment are not read as one.
NAME = r"(?<![\w./])([A-Za-z_][A-Za-z0-9_]*)"
#: The label-beside-citation forms, tried in order on the citation's line. A
#: form names a citation when its line number starts where the citation's does.
PAIRS = (
    # `loc_5366`, ps4.asm:7596  /  `Label`; `ps4.asm:N`  /  `Label`, `:N`
    re.compile(rf"`?{NAME}`?\s*[,;]?\s*`?{HEAD}:(\d+)"),
    # Label (ps4.asm:N) / `Label` (`reference/ps4disasm/ps4.asm:N`)
    re.compile(rf"`?{NAME}`?\s*\(\s*`?{HEAD}:(\d+)"),
    # ps4.asm:N (Label)
    re.compile(r"ps4\.asm:(\d+)(?:(?:-|\.\.)\d+)?`?\s*\(\s*`?([A-Za-z_][A-Za-z0-9_]*)`?\s*\)"),
)
#: The form whose label comes after the citation.
AFTER = 2


def labels(path: Path) -> dict[str, tuple[int, int]]:
    """`name -> (label line, body end)`, the body being the routine the label
    opens or sits in.

    A retail routine carries its own branch targets (`loc_51818`,
    `Title_LoadArtPtrsLoop`), so the body does not end at the next label: it
    ends before the next separator comment (`; ----`, `; ====`) or the next
    label that is not local (`LOCAL`), whichever comes first, and at the
    file's end for the last."""
    rows = path.read_text(errors="replace").splitlines()
    found = []
    for number, line in enumerate(rows, start=1):
        match = LABEL.match(line)
        if match:
            found.append((match.group(1), number))
    ends = {}
    for index, (name, number) in enumerate(found):
        end = len(rows)
        for later_name, later in found[index + 1:]:
            if not LOCAL.search(later_name):
                end = later - 1
                break
        for row in range(number, end):  # 0-based `row` is line `row + 1`
            if SEPARATOR.match(rows[row]):
                end = row
                break
        ends.setdefault(name, (number, end))
    return ends


def paired_label(line: str, citation: re.Match) -> str | None:
    """The label named beside the citation on `line`, when the form names one."""
    for index, pattern in enumerate(PAIRS):
        number, name = (1, 2) if index == AFTER else (2, 1)
        for match in pattern.finditer(line):
            if match.start(number) == citation.start(2):
                return match.group(name)
    return None


def text_of(path: Path) -> str | None:
    """A file's text, or None for a binary file (one holding a NUL byte) or one
    that cannot be read, such as a dangling link: every text file of the
    change is scanned, C, patches and tapes included, whatever its suffix."""
    try:
        data = path.read_bytes()
    except OSError:
        return None
    if b"\0" in data:
        return None
    return data.decode("utf-8", errors="replace")


def run(root: Path = Path("."), disasm: Path | None = None,
        files: list[str] | None = None) -> tuple[list[str], str]:
    """`(mismatch lines, summary)` for the change at `root`; an absent
    disassembly is reported as skipped and has no mismatches."""
    disasm = disasm or root / DISASM
    if not disasm.is_file():
        return [], f"citations: skipped (no {DISASM})"
    bodies = labels(disasm)
    disasm_lines = disasm.read_text(errors="replace").splitlines()
    lines_total = len(disasm_lines)
    citations = verified = use_sites = unverified = 0
    mismatches = []
    for name in (files if files is not None else repo_files(root)):
        path = Path(name)
        text = text_of(root / path)
        if text is None:
            continue
        if "ps4.asm:" not in text:
            continue
        for number, line in enumerate(text.splitlines(), start=1):
            for cite in CITATION.finditer(line):
                citations += 1
                start = int(cite.group(2))
                end = int(cite.group(3)) if cite.group(3) else start
                if start < 1 or end > lines_total or start > end:
                    mismatches.append(f"{path}:{number}: ps4.asm:{start}-{end} is past the "
                                      f"disassembly's {lines_total} lines")
                    continue
                label = paired_label(line, cite)
                if label is None or label not in bodies:
                    unverified += 1
                    continue
                first, last = bodies[label]
                if first <= start and end <= last:
                    verified += 1
                elif re.search(rf"\b{re.escape(label)}\b",
                               "\n".join(disasm_lines[start - 1:end])):
                    use_sites += 1
                else:
                    mismatches.append(
                        f"{path}:{number}: {label} is at line {first}-{last}, not "
                        f"ps4.asm:{start}-{end}")
    summary = (f"citations: {citations} citation(s), {verified} label-verified, "
               f"{use_sites} use-site, {unverified} unverified, "
               f"{len(mismatches)} mismatch(es)")
    return mismatches, summary


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--disasm", type=Path, help=f"the disassembly (default {DISASM})")
    args = parser.parse_args(argv)
    mismatches, summary = run(Path("."), args.disasm)
    for line in mismatches:
        print(line)
    print(summary)
    return 1 if mismatches else 0


if __name__ == "__main__":
    sys.exit(main())

