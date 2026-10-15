#!/usr/bin/env python3
"""tools/resume_check.py - every resume of a campaign route finishes, or halts
where the resume manifest says it does.

    python3 tools/resume_check.py --run DIR [--route FILE] [--manifest FILE]
        [--pack DIR] [--binary FILE] [--out DIR]
    python3 tools/resume_check.py --evaluate DIR [--route FILE] [--manifest FILE]

Run from the repository root. The cartridge's random draws are one stream, and
a fresh process that resumes from a chapter save starts that stream again, so
every chapter save is the start of another stream: a route that finishes on
one stream only is not a route (docs/campaign/CAMPAIGN_RUNNER.md). `--run DIR`
names a completed run of the route (its `report.json` lists every chapter's
save). For each chapter i from 1 to the last, the check copies chapter i-1's
save into a fresh directory and runs `psiv-campaign run ROUTE --from-chapter
<chapter i>` to the route's end, one process at a time.

A resume completes or halts. A halt is classified by where and how it
stopped, all from the run's halt report: the chapter, the objective index, the
objective's kind, the halt kind and, for a lost battle, the battle (`event:N`
for scripted battle N, `random` for an encounter; absent for any other halt).
The manifest (default `rust/psiv-campaign/routes/resume_manifest.json`) lists
the halts the route has today, each with that key, the resume points it occurs
from, the GitHub issue that owns it and the date it was recorded. The check
fails on:

- a halt the manifest does not list, or lists only from other resume points;
- a listed resume point that no longer halts there (a stale entry: the defect
  it records is gone or has moved, so the manifest changes with it);
- an entry without an issue link or a date, or otherwise malformed;
- a resume that did not run (a setup error, no report).

`--evaluate DIR` judges the outcomes an earlier check left in DIR against the
manifest without running anything, so a manifest edit is checked in seconds.

The check writes `DIR/NN-<chapter>/` for each resume (the copied save, the
chapter saves, tape, report, logs and exit status), `summary.json` (the route,
run, manifest and binary with their SHA-256, each resume's outcome, the
scripted battles each resume won and lost, and the problems) and
`resume_check.log`. Exit status: 0 when the outcomes match the manifest, 1
when they do not, 2 when the check cannot run (no completed run, a run of
another route, an unreadable manifest, an output directory in use). The
62-chapter route takes about 25 minutes; `tools/verify_candidate.py` runs it
under the heavy lock. Stdlib only, Python 3.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

EXIT_OK = 0
EXIT_PROBLEMS = 1
EXIT_REFUSED = 2

ROUTE = Path("rust/psiv-campaign/routes/main.json")
MANIFEST = Path("rust/psiv-campaign/routes/resume_manifest.json")
BINARY = Path("rust/target/release/psiv-campaign")
OUT_ROOT = Path("build") / "resume-check"

ISSUE_RE = re.compile(r"^https://github\.com/[\w.-]+/[\w.-]+/issues/\d+$")
DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
BATTLE_RE = re.compile(r"^(event:\d+|random)$")
ENTRY_KEYS = {"chapter", "objective", "objective_kind", "kind", "battle", "resumes", "issue",
              "date", "note"}


class Refused(Exception):
    """The check cannot run; the message says why."""


def sha256(path: Path) -> str | None:
    return hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else None


def route_ids(route: Path) -> list[str]:
    """The route's chapter ids, in order."""
    try:
        data = json.loads(route.read_text())
        return [chapter["id"] for chapter in data["chapters"]]
    except (OSError, ValueError, KeyError, TypeError) as error:
        raise Refused(f"cannot read the route {route}: {error}") from error


def seed_saves(run: Path, ids: list[str]) -> list[Path]:
    """Each chapter's save from the completed run `run` of the route `ids`."""
    try:
        report = json.loads((run / "report.json").read_text())
    except (OSError, ValueError) as error:
        raise Refused(f"no completed run in {run}: {error}") from error
    if report.get("result") != "completed":
        raise Refused(f"{run} is not a completed run (result {report.get('result')!r})")
    chapters = sorted(report.get("chapters", []), key=lambda c: c.get("index", -1))
    found = [c.get("id") for c in chapters]
    if [c.get("index") for c in chapters] != list(range(len(ids))) or found != ids:
        raise Refused(f"{run} is a run of another route: its chapters are {found[:3]}..., "
                      f"{len(found)} of them, the route's {ids[:3]}..., {len(ids)}")
    saves = [Path(c["save"]) for c in chapters]
    missing = [str(s) for s in saves if not s.is_file()]
    if missing:
        raise Refused(f"the run's chapter saves are missing: {missing[:3]}")
    return saves


def resume_dir(out: Path, index: int, ids: list[str]) -> Path:
    return out / f"{index:02d}-{ids[index]}"


def run_resume(binary: Path, route: Path, ids: list[str], index: int, save: Path, out: Path,
               pack: Path | None) -> int:
    """Resume the route at chapter `index` from `save` (chapter index-1's) in a fresh
    directory; returns the exit status, which the directory also records."""
    directory = resume_dir(out, index, ids)
    directory.mkdir(parents=True)
    previous = directory / f"{index - 1:02d}-{ids[index - 1]}"
    previous.mkdir()
    shutil.copyfile(save, previous / "slot_1.sram")
    command = [str(binary), "run", str(route), "--from-chapter", ids[index],
               "--save-dir", str(directory), "--tape", str(directory / "run.tape"),
               "--report", str(directory / "report.json")]
    if pack is not None:
        command += ["--pack", str(pack)]
    with open(directory / "out.log", "w") as stdout, open(directory / "err.log", "w") as stderr:
        code = subprocess.run(command, stdout=stdout, stderr=stderr).returncode
    (directory / "exit").write_text(f"{code}\n")
    return code


def someone_stood(battle: dict) -> bool:
    """Whether a battle record ended with a member standing (`party_at_end`, the
    runner's battle-end record): a lost battle ends with every member down."""
    return any(member.get("standing") for member in battle.get("party_at_end", []))


def battle_of(battle: dict) -> str:
    """A battle record's name in a halt key: `event:N` or `random`."""
    return f"event:{battle['id']}" if battle.get("kind") == "event" else "random"


def outcome_of(directory: Path, start: str) -> dict:
    """What the resume in `directory` did: completed (exit 0 and a completed
    report), halted (exit 2 and a halted report; where and how) or error
    (anything else), with the scripted (event) battles it won and the one it
    lost."""
    outcome: dict = {"start": start, "result": "error", "won": [], "lost": None}
    try:
        code = int((directory / "exit").read_text())
    except (OSError, ValueError):
        outcome["detail"] = "no exit status: the resume did not run"
        return outcome
    try:
        report = json.loads((directory / "report.json").read_text())
    except (OSError, ValueError):
        outcome["detail"] = f"exit {code} and no report"
        return outcome
    if code not in (0, 2):
        outcome["detail"] = (f"exit {code} beside a {report.get('result')!r} report: the "
                             "runner stopped without its verdict")
        return outcome
    if code == 0 and report.get("result") == "completed":
        outcome["result"] = "completed"
        outcome["won"] = [b["id"] for b in report.get("event_battles", []) if someone_stood(b)]
        return outcome
    if code == 2 and report.get("result") == "halted":
        battles = report.get("battles", [])
        events = [b for b in battles if b.get("kind") == "event"]
        kind = report.get("halt", {}).get("kind")
        # The battle a lost battle was lost in is the last one the report lists.
        last = battles[-1] if kind == "lost_battle" and battles else None
        outcome.update({
            "result": "halted",
            "chapter": report.get("chapter"),
            "objective": report.get("objective_index"),
            "objective_kind": report.get("objective_kind"),
            "kind": kind,
            "battle": None if last is None else battle_of(last),
            "detail": report.get("halt", {}).get("detail", ""),
            "won": [b["id"] for b in events if someone_stood(b)],
        })
        if last is not None and last.get("kind") == "event" and not someone_stood(last):
            outcome["lost"] = last["id"]
        return outcome
    outcome["detail"] = f"exit {code}, report result {report.get('result')!r}"
    return outcome


#: Where and how a halt stopped: chapter, objective index, objective kind, halt
#: kind and battle (`None` for a halt that is not a lost battle).
KEY_FIELDS = ("chapter", "objective", "objective_kind", "kind", "battle")


def halt_key(entry_or_outcome: dict) -> tuple:
    return tuple(entry_or_outcome.get(field) for field in KEY_FIELDS)


def describe(halt: dict) -> str:
    """A halt (an entry or an outcome) for a problem line."""
    kind, battle = halt.get("kind"), halt.get("battle")
    how = kind if battle is None else f"{kind} in {battle}"
    return (f"chapter {halt.get('chapter')} objective {halt.get('objective')} "
            f"{halt.get('objective_kind')} ({how})")


def valid_date(text: object) -> bool:
    if not isinstance(text, str) or not DATE_RE.match(text):
        return False
    try:
        dt.date.fromisoformat(text)
    except ValueError:
        return False
    return True


def load_manifest(path: Path, ids: list[str]) -> tuple[list[dict], list[str]]:
    """The manifest's entries and what is wrong with them."""
    try:
        data = json.loads(path.read_text())
    except (OSError, ValueError) as error:
        raise Refused(f"cannot read the manifest {path}: {error}") from error
    if not isinstance(data, dict) or data.get("format") != 1 or not isinstance(
            data.get("halts"), list):
        raise Refused(f"{path}: expected {{\"format\": 1, \"halts\": [...]}}")
    entries = data["halts"]
    problems = []
    seen_keys: dict[tuple, int] = {}
    seen_resumes: dict[str, int] = {}
    for n, entry in enumerate(entries):
        where = f"manifest entry {n}"
        if not isinstance(entry, dict):
            problems.append(f"{where}: not an object")
            continue
        where = f"manifest entry {n} ({describe(entry)})"
        unknown = sorted(set(entry) - ENTRY_KEYS)
        if unknown:
            problems.append(f"{where}: unknown keys {unknown}")
        if entry.get("chapter") not in ids:
            problems.append(f"{where}: chapter {entry.get('chapter')!r} is not in the route")
        objective = entry.get("objective")
        if not isinstance(objective, int) or isinstance(objective, bool) or objective < 0:
            problems.append(f"{where}: objective must be an objective index")
        if not isinstance(entry.get("objective_kind"), str) or not entry.get("objective_kind"):
            problems.append(f"{where}: no objective kind")
        if not isinstance(entry.get("kind"), str) or not entry.get("kind"):
            problems.append(f"{where}: no halt kind")
        battle = entry.get("battle")
        lost = entry.get("kind") == "lost_battle"
        if lost and (not isinstance(battle, str) or not BATTLE_RE.match(battle)):
            problems.append(f"{where}: a lost battle names its battle (event:<n> or random)")
        if not lost and battle is not None:
            problems.append(f"{where}: only a lost battle names a battle")
        if not isinstance(entry.get("issue"), str) or not ISSUE_RE.match(entry["issue"]):
            problems.append(f"{where}: no issue link (https://github.com/<owner>/<repo>/issues/<n>)")
        if not valid_date(entry.get("date")):
            problems.append(f"{where}: no date (YYYY-MM-DD)")
        resumes = entry.get("resumes")
        if not isinstance(resumes, list) or not resumes:
            problems.append(f"{where}: no resume points")
            resumes = []
        for start in resumes:
            if start not in ids[1:]:
                problems.append(f"{where}: resume point {start!r} is not a chapter after the first")
            elif start in seen_resumes:
                problems.append(f"{where}: resume point {start} is also in entry "
                                f"{seen_resumes[start]}")
            else:
                seen_resumes[start] = n
        key = halt_key(entry)
        if key in seen_keys:
            problems.append(f"{where}: the same halt as entry {seen_keys[key]}")
        seen_keys[key] = n
    return entries, problems


def judge(outcomes: list[dict], entries: list[dict]) -> list[str]:
    """Where the outcomes and the manifest disagree, one line each."""
    problems = []
    expected: dict[str, dict] = {}
    for entry in entries:
        if isinstance(entry, dict) and isinstance(entry.get("resumes"), list):
            for start in entry["resumes"]:
                expected.setdefault(start, entry)
    listed = {halt_key(e): e for e in entries if isinstance(e, dict)}
    ran = set()
    for outcome in outcomes:
        start = outcome["start"]
        ran.add(start)
        entry = expected.get(start)
        if outcome["result"] == "error":
            problems.append(f"resume from {start} did not run: {outcome.get('detail', '')}")
            continue
        if outcome["result"] == "completed":
            if entry is not None:
                problems.append(f"stale: the manifest lists {describe(entry)} from "
                                f"{start} ({entry.get('issue')}), and that resume completes")
            continue
        key = halt_key(outcome)
        if entry is not None and halt_key(entry) == key:
            continue
        detail = outcome.get("detail", "")
        if key in listed:
            problems.append(f"resume from {start} halts at {describe(outcome)}, which the "
                            f"manifest lists only from {listed[key].get('resumes')}: {detail}")
        else:
            problems.append(f"resume from {start} halts at {describe(outcome)}, which the "
                            f"manifest does not list: {detail}")
        if entry is not None:
            problems.append(f"stale: the manifest lists {describe(entry)} from {start} "
                            f"({entry.get('issue')}), and that resume halts elsewhere")
    for start, entry in expected.items():
        if start not in ran:
            problems.append(f"the manifest lists {describe(entry)} from {start}, "
                            "which this check did not resume")
    return problems


def fight_table(outcomes: list[dict]) -> dict[str, dict]:
    """For each scripted battle, the resume points that won it and that lost it."""
    table: dict[str, dict] = {}
    for outcome in outcomes:
        for battle in outcome.get("won", []):
            table.setdefault(str(battle), {"won": [], "lost": []})["won"].append(outcome["start"])
        if outcome.get("lost") is not None:
            table.setdefault(str(outcome["lost"]), {"won": [], "lost": []})["lost"].append(
                outcome["start"])
    return dict(sorted(table.items(), key=lambda item: int(item[0])))


def line_of(index: int, outcome: dict) -> str:
    if outcome["result"] == "completed":
        what = "completed"
    elif outcome["result"] == "halted":
        what = f"halted at {describe(outcome)}: {outcome.get('detail', '')}"
    else:
        what = f"error: {outcome.get('detail', '')}"
    return f"from {index:02d} {outcome['start']}: {what}"


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--run", type=Path, help="a completed run of the route to resume")
    source.add_argument("--evaluate", type=Path,
                        help="judge an earlier check's directory without running anything")
    parser.add_argument("--route", type=Path, default=ROUTE)
    parser.add_argument("--manifest", type=Path, default=MANIFEST)
    parser.add_argument("--pack", type=Path, help="the pack the resumes load (default: the "
                        "binary's own)")
    parser.add_argument("--binary", type=Path, default=BINARY)
    parser.add_argument("--out", type=Path, help="the check's directory (default "
                        "build/resume-check/<UTC stamp>)")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        return check(args)
    except Refused as error:
        print(f"resume_check: {error}", file=sys.stderr)
        return EXIT_REFUSED


def check(args: argparse.Namespace) -> int:
    ids = route_ids(args.route)
    if len(ids) < 2:
        raise Refused(f"{args.route} has fewer than two chapters: nothing to resume")
    entries, problems = load_manifest(args.manifest, ids)
    started = dt.datetime.now(dt.timezone.utc).isoformat(timespec="seconds")
    if args.evaluate is not None:
        out = args.evaluate
        if not out.is_dir():
            raise Refused(f"{out} is not a check directory")
        saves = None
    else:
        saves = seed_saves(args.run, ids)
        if not args.binary.is_file():
            raise Refused(f"no campaign binary at {args.binary}")
        stamp = dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
        out = args.out or OUT_ROOT / stamp
        if out.exists() and any(out.iterdir()):
            raise Refused(f"{out} is not empty")
        out.mkdir(parents=True, exist_ok=True)
    log_lines = []
    outcomes = []
    for index in range(1, len(ids)):
        if saves is not None:
            begin = time.monotonic()
            run_resume(args.binary, args.route, ids, index, saves[index - 1], out, args.pack)
            seconds = time.monotonic() - begin
        else:
            seconds = None
        outcome = outcome_of(resume_dir(out, index, ids), ids[index])
        outcome["seconds"] = None if seconds is None else round(seconds, 1)
        outcomes.append(outcome)
        line = line_of(index, outcome)
        log_lines.append(line)
        print(line, flush=True)
    problems += judge(outcomes, entries)
    completed = sum(o["result"] == "completed" for o in outcomes)
    verdict = "OK" if not problems else f"{len(problems)} problem(s)"
    summary_line = (f"resume check: {completed} of {len(outcomes)} resumes completed; "
                    f"manifest {args.manifest}: {verdict}")
    log_lines += [f"problem: {p}" for p in problems] + [summary_line]
    for problem in problems:
        print(f"problem: {problem}", flush=True)
    summary = {
        "route": str(args.route), "route_sha256": sha256(args.route),
        "run": None if args.run is None else str(args.run),
        "manifest": str(args.manifest), "manifest_sha256": sha256(args.manifest),
        "binary": str(args.binary), "binary_sha256": sha256(args.binary),
        "pack": None if args.pack is None else str(args.pack),
        "evaluated_only": args.evaluate is not None,
        "started": started,
        "ended": dt.datetime.now(dt.timezone.utc).isoformat(timespec="seconds"),
        "resumes": outcomes, "completed": completed, "fights": fight_table(outcomes),
        "problems": problems, "result": "ok" if not problems else "problems",
    }
    name = "summary.json" if args.evaluate is None else "evaluate-summary.json"
    (out / name).write_text(json.dumps(summary, indent=1) + "\n")
    log_name = "resume_check.log" if args.evaluate is None else "evaluate.log"
    (out / log_name).write_text("\n".join(log_lines) + "\n")
    print(f"{summary_line}; summary={out / name}", flush=True)
    return EXIT_OK if not problems else EXIT_PROBLEMS


if __name__ == "__main__":
    sys.exit(main())
