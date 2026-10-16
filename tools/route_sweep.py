#!/usr/bin/env python3
"""tools/route_sweep.py - play a campaign route with its training chapters changed.

    python3 tools/route_sweep.py NAME [--level CHAPTER=N]... [--money CHAPTER=N]...
        [--drop CHAPTER]... [--seed RUN --from-chapter ID] [--until-chapter ID]
        [--route FILE] [--pack DIR] [--binary FILE] [--out-root DIR] [--replace]
    python3 tools/route_sweep.py --summarize RUN...

Run from the repository root. A training chapter patrols until its `until`
holds (`party_level_at_least`, `money_at_least`); choosing its level is a
sweep: the same route at several levels, each played and read the same way.
NAME's run writes `OUT_ROOT/NAME/route.json`, the route with the changes made
(`--level` and `--money` set the chapter's patrol condition, `--drop` removes
the chapter), and plays it with `psiv-campaign run` into `OUT_ROOT/NAME`
(default `build/sweep/NAME`). `--seed RUN --from-chapter ID` starts at chapter
ID from the save RUN's chapter before it left, in a fresh process: the
snapshot clock, a different random stream from the full run's.

Each run, and each directory `--summarize` names, prints one summary: the exit
and where a halt stopped, the frames and digest, every training chapter's
frames, battles and party, and every scripted battle's end as the runner
recorded it (`event battle N: ...; every member standing` or `down: names`,
`BattleRecord::end_line`). A chapter's closing party is not a fight's result:
a chapter can heal or lose members after its fight.

A sweep judges a level on two streams at most. Whether a route finishes on
every stream is `tools/resume_check.py`'s question; the training rule that
uses it is in docs/campaign/CAMPAIGN_RUNNER.md. Exit status: the run's own
(0 completed, 2 halted, 1 setup error), or 2 when the sweep is refused before
anything runs (an unknown chapter, a chapter without a patrol, an output
directory in use without `--replace`, a NAME that is not one plain path
component or whose directory does not sit inside OUT_ROOT). `--replace`
deletes only that directory. Stdlib only, Python 3.
"""

from __future__ import annotations

import argparse
import copy
import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

EXIT_REFUSED = 2

ROUTE = Path("rust/psiv-campaign/routes/main.json")
BINARY = Path("rust/target/release/psiv-campaign")
OUT_ROOT = Path("build") / "sweep"

CHAPTER_RE = re.compile(r"^chapter (\S+): (\d+) frames, (\d+) battles, .*party (.*)$")
EVENT_RE = re.compile(r"^  (event battle \d+: .*)$")
TAPE_RE = re.compile(r"^tape .* \((\d+) frames\); digest (\S+)$")
NAME_RE = re.compile(r"^[\w.-]+$")


class Refused(Exception):
    """The sweep cannot run; the message says why."""


def pairs(values: list[str], what: str) -> dict[str, int]:
    """`CHAPTER=N` arguments as a mapping."""
    out = {}
    for value in values:
        chapter, sep, number = value.partition("=")
        if not sep or not number.isdigit():
            raise Refused(f"--{what} wants CHAPTER=N, not {value!r}")
        out[chapter] = int(number)
    return out


def variant(route: dict, levels: dict[str, int], money: dict[str, int],
            drop: list[str]) -> dict:
    """`route` with each named chapter's patrol condition set and chapters dropped."""
    route = copy.deepcopy(route)
    chapters = route["chapters"]
    ids = [c["id"] for c in chapters]
    for chapter in [*levels, *money, *drop]:
        if chapter not in ids:
            raise Refused(f"no chapter {chapter!r} in the route")
    for chapter in {*levels, *money}:
        patrols = [o for o in chapters[ids.index(chapter)]["objectives"]
                   if o.get("do") == "patrol"]
        if len(patrols) != 1:
            raise Refused(f"chapter {chapter} has {len(patrols)} patrols; a sweep sets one")
        until = patrols[0].setdefault("until", {})
        if chapter in levels:
            until["party_level_at_least"] = levels[chapter]
        if chapter in money:
            until["money_at_least"] = money[chapter]
    route["chapters"] = [c for c in chapters if c["id"] not in drop]
    return route


def summarize(run: Path) -> list[str]:
    """The summary lines of the run in `run` (its `out.log`, and `report.json` for a
    halt)."""
    name = run.name
    try:
        lines = (run / "out.log").read_text(errors="replace").splitlines()
    except OSError:
        return [f"== {name}: no out.log"]
    tape = next((m for m in map(TAPE_RE.match, lines) if m), None)
    halt = next((line for line in lines if line.startswith("HALTED")), None)
    head = f"== {name}: "
    head += f"{int(tape.group(1)):,} frames, digest {tape.group(2)}" if tape else "no tape"
    head += f"; {halt}" if halt else ("; completed" if "COMPLETED" in lines else "")
    out = [head]
    chapter = None
    for line in lines:
        if match := CHAPTER_RE.match(line):
            chapter = match.group(1)
            if "training" in chapter:
                out.append(f"   {chapter}: {int(match.group(2)):,} frames, "
                           f"{match.group(3)} battles; {match.group(4)}")
        elif match := EVENT_RE.match(line):
            out.append(f"   {chapter} {match.group(1)}")
    if halt:
        try:
            report = json.loads((run / "report.json").read_text())
        except (OSError, ValueError):
            report = {}
        last = (report.get("battles") or [None])[-1]
        if last and last.get("party_at_end"):
            party = ", ".join(f"{m['name']} {m['hp']}/{m['max_hp']}"
                              for m in last["party_at_end"])
            out.append(f"   last battle: {last['kind']} {last['id']}; party {party}")
    return out


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("name", nargs="?", help="the run's name, its directory under OUT_ROOT")
    parser.add_argument("--summarize", nargs="+", type=Path, metavar="RUN",
                        help="print the summaries of earlier runs and exit")
    parser.add_argument("--level", action="append", default=[], metavar="CHAPTER=N")
    parser.add_argument("--money", action="append", default=[], metavar="CHAPTER=N")
    parser.add_argument("--drop", action="append", default=[], metavar="CHAPTER")
    parser.add_argument("--seed", type=Path, help="a run whose chapter saves to start from")
    parser.add_argument("--from-chapter")
    parser.add_argument("--until-chapter")
    parser.add_argument("--route", type=Path, default=ROUTE)
    parser.add_argument("--pack", type=Path)
    parser.add_argument("--binary", type=Path, default=BINARY)
    parser.add_argument("--out-root", type=Path, default=OUT_ROOT)
    parser.add_argument("--replace", action="store_true",
                        help="remove an earlier run of the same name first")
    return parser


def run_dir(out_root: Path, name: str) -> Path:
    """NAME's directory, refused unless NAME is one plain path component and
    the directory resolves to a direct child of OUT_ROOT: `--replace` deletes
    it, and nothing outside OUT_ROOT may be deleted (an absolute NAME, `.`,
    `..`, or a link inside OUT_ROOT that points elsewhere)."""
    if not NAME_RE.match(name) or name in (".", ".."):
        raise Refused(f"the run name {name!r} is not one plain path component "
                      "(letters, digits, '_', '.', '-')")
    out = out_root / name
    if out.resolve().parent != out_root.resolve():
        raise Refused(f"{out} resolves to {out.resolve()}, outside {out_root.resolve()}")
    return out


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    if args.summarize:
        for run in args.summarize:
            print("\n".join(summarize(run)))
        return 0
    try:
        return sweep(args)
    except Refused as error:
        print(f"route_sweep: {error}", file=sys.stderr)
        return EXIT_REFUSED


def sweep(args: argparse.Namespace) -> int:
    if not args.name:
        raise Refused("name the run (or --summarize RUN...)")
    out = run_dir(args.out_root, args.name)
    if (args.seed is None) != (args.from_chapter is None):
        raise Refused("--seed and --from-chapter go together")
    try:
        route = json.loads(args.route.read_text())
    except (OSError, ValueError) as error:
        raise Refused(f"cannot read the route {args.route}: {error}") from error
    changed = variant(route, pairs(args.level, "level"), pairs(args.money, "money"), args.drop)
    ids = [c["id"] for c in changed["chapters"]]
    for chapter in (args.from_chapter, args.until_chapter):
        if chapter is not None and chapter not in ids:
            raise Refused(f"no chapter {chapter!r} in the changed route")
    if out.exists() and any(out.iterdir()):
        if not args.replace:
            raise Refused(f"{out} holds an earlier run (--replace removes it)")
        shutil.rmtree(out)
    out.mkdir(parents=True, exist_ok=True)
    if args.seed is not None:
        start = ids.index(args.from_chapter)
        if start == 0:
            raise Refused("--from-chapter names the first chapter: no save comes before it")
        previous = f"{start - 1:02d}-{ids[start - 1]}"
        if not (args.seed / previous / "slot_1.sram").is_file():
            raise Refused(f"the seed has no save {args.seed / previous / 'slot_1.sram'}")
        shutil.copytree(args.seed / previous, out / previous)
    variant_path = out / "route.json"
    variant_path.write_text(json.dumps(changed, indent=1) + "\n")
    command = [str(args.binary), "run", str(variant_path), "--save-dir", str(out),
               "--tape", str(out / "run.tape"), "--report", str(out / "report.json")]
    for flag, value in (("--from-chapter", args.from_chapter),
                        ("--until-chapter", args.until_chapter), ("--pack", args.pack)):
        if value is not None:
            command += [flag, str(value)]
    with open(out / "out.log", "w") as stdout, open(out / "err.log", "w") as stderr:
        code = subprocess.run(command, stdout=stdout, stderr=stderr).returncode
    print("\n".join(summarize(out)), flush=True)
    return code


if __name__ == "__main__":
    sys.exit(main())
