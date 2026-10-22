#!/usr/bin/env python3
"""Check retail `ps4.asm:N` citations against the local disassembly (issue #158).

    python3 tools/check_citations.py [--disasm PATH]

The disassembly is a local input (`reference/ps4disasm/ps4.asm`); on a public
clone the check says it was skipped and passes. When it is present, every
citation in the change's text files (every readable file without a NUL
byte, whatever its suffix) is read:

- `ps4.asm:N` and `ps4.asm:N-M` must name existing lines (`1..line count`);
- where a label is named beside the citation (the forms in use: a backticked
  label then the citation, `Label (ps4.asm:N)`, `ps4.asm:N (Label)`,
  `Label, :N`), the citation must sit inside the routine the label opens or
  name a line that *uses* the label (the cited lines mention it, e.g. a
  `jsr`). A routine runs from its label to the next separator comment or the
  next label that is not a branch target inside it (`labels`). A citation that
  neither defines nor uses the label beside it is a mismatch;
- a citation without a neighbouring label is counted unverified, not failed:
  the count is printed and can only shrink.

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
CITATION = re.compile(r"ps4\.asm:(\d+)(?:-(\d+))?")

#: A citation may carry the disassembly's path (`reference/ps4disasm/ps4.asm:N`);
#: `\b` keeps the path from starting inside the label before it.
DIR = r"(?:\b[\w.-]+/)*"
#: The label-beside-citation forms, tried in order on the citation's line.
#: The label is group 1 and the line (or the range's end) the other groups.
PAIRS = (
    # `loc_5366`, ps4.asm:7596   —  `Label`; `ps4.asm:N` / Label, ps4.asm:N
    re.compile(rf"`?([A-Za-z_][A-Za-z0-9_]*)`?\s*[,;]?\s*`?{DIR}ps4\.asm:(\d+)(?:-(\d+))?"),
    # Label (ps4.asm:N) / `Label` (`reference/ps4disasm/ps4.asm:N`)
    re.compile(rf"`?([A-Za-z_][A-Za-z0-9_]*)`?\s*\(\s*`?{DIR}ps4\.asm:(\d+)(?:-(\d+))?"),
    # ps4.asm:N (Label)
    re.compile(r"ps4\.asm:(\d+)(?:-(\d+))?`?\s*\(\s*`?([A-Za-z_][A-Za-z0-9_]*)`?\s*\)"),
    # Label, :N
    re.compile(r"`?([A-Za-z_][A-Za-z0-9_]*)`?\s*,\s*:(\d+)"),
)
#: The form whose label is the last group (after the citation).
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
        for match in pattern.finditer(line):
            if index == AFTER:
                line_start, line_end, name = match.group(1), match.group(2), match.group(3)
            else:
                line_start, line_end, name = match.group(2), match.group(3), match.group(1)
            if line_start == citation.group(1) and line_end == citation.group(2):
                return name
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
        for number, line in enumerate(text.splitlines(), start=1):
            for cite in CITATION.finditer(line):
                citations += 1
                start = int(cite.group(1))
                end = int(cite.group(2)) if cite.group(2) else start
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

