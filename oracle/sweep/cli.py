"""The sweep's command line: the arguments, and the process exit status.

`oracle/sweep/__main__.py` is the entry point; `oracle/sweep/__init__.py` is the
account of what a capture is, and `docs/oracle/BATTLE_ORACLE_SWEEP.md` of what
came out of this sweep.
"""
from __future__ import annotations

import argparse
import pathlib
import sys

from ..force.errors import ForceError
from ..force.pack import Pack
from . import jobs as sweep_jobs
from . import reextract
from .batch import Sweep
from .jobs import Options
from .plan import list_formations, write_list

def parser() -> argparse.ArgumentParser:
    parsed = argparse.ArgumentParser(
        description="Capture and extract every Motavia formation.")
    parsed.add_argument("--out", default="build/lane-evidence/sweep",
                        help="the sweep's working directory; the record and "
                             "every capture receipt live under it")
    parsed.add_argument("--fixtures", default=str(sweep_jobs.FIXTURES),
                        help="where the extracted fixtures are written")
    parsed.add_argument("--list", default="",
                        help="write the formation list here as JSON (default: "
                             "the record's own `source` only)")
    parsed.add_argument("--jobs", type=int, default=3,
                        help="captures in parallel (the machine's memory cap; "
                             "each is an emulator run plus a Python log parse)")
    parsed.add_argument("--max-rounds", type=int, default=5,
                        help="round cap per capture (see oracle/force/cli.py)")
    parsed.add_argument("--repeats", type=int, default=900,
                        help="policy blocks per capture tape")
    parsed.add_argument("--policy", default="attack",
                        choices=("attack", "defend"))
    parsed.add_argument("--no-durable", action="store_true",
                        help="do not patch the party's HP (see "
                             "oracle/force/durable.py); the sweep's own runs "
                             "are durable")
    parsed.add_argument("--base-tape",
                        default=str(sweep_jobs.ROOT / "oracle" / "tapes"
                                    / "07_first_battle.tape"))
    parsed.add_argument("--data-dir", default=str(sweep_jobs.ROOT
                                                  / "generated"))
    parsed.add_argument("--ram-map",
                        default=str(sweep_jobs.ROOT / "oracle" / "ram_map.json"))
    parsed.add_argument("--only", default="",
                        help="comma-separated formation ids (decimal or "
                             "0x-prefixed): a subset of the list")
    parsed.add_argument("--limit", type=int, default=0,
                        help="stop after this many formations (0 = all)")
    parsed.add_argument("--force", action="store_true",
                        help="capture every formation again, ignoring what the "
                             "record already holds")
    parsed.add_argument("--reextract", default="",
                        help="do not capture: re-run the extraction over the "
                             "captures a sweep's working directory already "
                             "holds (the directory itself), and stop")
    return parsed


def main(argv: list[str] | None = None) -> int:
    args = parser().parse_args(argv)
    out = pathlib.Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    options = Options(
        work=out, fixtures=pathlib.Path(args.fixtures),
        max_rounds=args.max_rounds, repeats=args.repeats, policy=args.policy,
        durable=not args.no_durable,
        base_tape=pathlib.Path(args.base_tape),
        data_dir=pathlib.Path(args.data_dir),
        ram_map=pathlib.Path(args.ram_map))
    try:
        if args.reextract:
            return reextract.main(pathlib.Path(args.reextract), options,
                                  args.jobs)
        return sweep(args, options)
    except ForceError as error:
        print(f"sweep: {error}", file=sys.stderr)
        return 2


def sweep(args, options: Options) -> int:
    """The list, then the batch, then the record's own summary."""
    pack = Pack.load(options.data_dir)
    formations = list_formations(pack)
    source = {
        "formations": "generated/formation_indexes.json",
        "groups": {"foot": [0, 1, 2, 3, 4, 5, 6, 7],
                   "vehicle_tables": [8, 9, 10]},
        "note": "one entry per distinct formation id in those groups, "
                "ascending",
        "count": len(formations),
    }
    if args.list:
        write_list(pathlib.Path(args.list), formations, source)
    only = {int(value, 0) for value in args.only.split(",") if value.strip()}
    sweep_run = Sweep(options.work, options, formations, source)
    document = sweep_run.run(jobs_count=args.jobs, force=args.force,
                             limit=args.limit, only=only)
    summary = document["summary"]
    print(f"sweep: {summary['captured']}/{summary['formations']} captured, "
          f"{summary['failed']} failed, {summary['truncated']} truncated")
    for failure in summary["failures"]:
        print(f"  failed: #{failure['formation']:02X} at {failure['stage']}: "
              f"{failure['error'].splitlines()[-1] if failure['error'] else ''}")
    print(f"sweep: record {sweep_run.path}")
    return 0 if summary["failed"] == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
