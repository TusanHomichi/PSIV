"""Re-extracting a sweep's fixtures from the captures it already took.

A sweep's captures are the expensive half: every one is an emulator run over
tens of thousands of frames, and `docs/BATTLE_ORACLE_SWEEP.md`'s clusters are
fixed by re-reading them, not by re-taking them. This is that run: for every
`formation_XX/` directory under a sweep's working directory that holds a
capture, the extraction stage is run again, with the same numbers the capture
was recorded with (`report.json`: the battle's frame window, the round cap, the
HP the capture patched), and the fixture is written where `--fixtures` says.

Nothing here touches the tapes, the emulator or the capture: the inputs are the
`capture/forced_*.csv` pair the sweep left behind, and a formation whose
capture is not there is reported and skipped rather than guessed at.

    python3 oracle/sweep.py --reextract build/lane-evidence/sweep
"""
from __future__ import annotations

import concurrent.futures
import json
import pathlib
import sys

from . import jobs

def captures(directory: pathlib.Path) -> tuple[pathlib.Path, pathlib.Path]:
    """A formation's `(log, trace)` CSVs, as the capture wrote them."""
    found = [path for path in directory.glob("capture/forced_*.csv")
             if "_rolls" not in path.name]
    if len(found) != 1:
        raise FileNotFoundError(
            f"{directory} holds {len(found)} capture log(s), expected one")
    trace = found[0].with_name(found[0].stem + "_rolls.csv")
    if not trace.exists():
        raise FileNotFoundError(f"{found[0]} has no roll trace beside it")
    return found[0], trace


def extractor_argv(work: pathlib.Path, directory: pathlib.Path,
                   options: jobs.Options) -> list[str]:
    """The extraction command for one formation, from its own report."""
    report = json.loads((directory / "report.json").read_text())
    log, trace = captures(directory)
    formation = int(report["formation"])
    argv = [sys.executable, str(jobs.FX), "--trace", str(trace),
            "--log", str(log),
            "--out", str(jobs.fixture_path(options, formation)),
            "--tape", pathlib.Path(report["tape"]).name,
            "--ram-map", str(options.ram_map),
            "--battle-first", str(report["battle_first"]),
            "--battle-last", str(report["battle_last"]),
            "--max-rounds", str(report.get("max_rounds") or options.max_rounds),
            "--minified"]
    durable = report.get("durable") or {}
    if options.durable and durable.get("hp"):
        argv += ["--hp-patch", str(durable["hp"])]
    return argv


def directories(work: pathlib.Path) -> list[pathlib.Path]:
    """Every formation directory a sweep left behind, in formation order."""
    out = [path for path in work.glob("formation_*")
           if (path / "report.json").exists()]
    return sorted(out, key=lambda path: path.name)


def main(work: pathlib.Path, options: jobs.Options, jobs_count: int = 3) -> int:
    """Re-extract every capture under `work`; a failure is reported, not fatal.

    `jobs_count` extractor runs at once (`--jobs`): each parses a ~30MB RAM log
    in Python, which is the memory cap the captures share.
    """
    found = directories(work)
    if not found:
        print(f"reextract: {work} holds no formation directory with a report",
              file=sys.stderr)
        return 2

    def one(directory: pathlib.Path) -> tuple[pathlib.Path, int, str]:
        try:
            argv = extractor_argv(work, directory, options)
        except (OSError, KeyError, ValueError) as error:
            return directory, 2, str(error)
        log = directory / "extract.log"
        proc = jobs.run(argv, log)
        return directory, proc.returncode, jobs.tail(log, 1)

    with concurrent.futures.ThreadPoolExecutor(
            max_workers=max(1, jobs_count)) as pool:
        results = list(pool.map(one, found))
    failures = 0
    for directory, status, note in results:
        print(f"reextract: {directory.name}: exit {status}"
              + (f" - {note}" if status else ""))
        failures += status != 0
    print(f"reextract: {len(found) - failures}/{len(found)} extracted from "
          f"{work}")
    return 0 if failures == 0 else 1
