"""One formation's run: the forced capture, then the fixture's extraction.

Two subprocesses per formation - `python3 -m oracle.force` and
`python3 -m oracle.fixture`, each with the exact command recorded - and a record
entry either way: a formation whose capture or extraction fails is part of the
sweep's result, not an exception out of it (`docs/oracle/BATTLE_ORACLE_SWEEP.md` reads
those failures as their own worklist clusters).

Both tools run as subprocesses rather than in-process, for two reasons that
matter on a machine running three captures at once: a crash or an emulator
runaway takes down one formation's run and not the sweep, and each run's own
memory - the extractor parses a ~30MB RAM log - is released when the process
exits.
"""
from __future__ import annotations

import dataclasses
import hashlib
import json
import pathlib
import subprocess
import sys

from ..force.errors import ForceError
from .plan import Formation

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
#: The two tools a formation's run is, as modules run from the repository root
#: (`oracle/README.md`, "Layout"): the sweep builds `python3 -m <name>` commands.
FX = "oracle.fixture"
FORCE = "oracle.force"
FIXTURES = (ROOT / "rust" / "psiv-core" / "src" / "battle"
            / "replay_fixtures" / "sweep_motavia")


@dataclasses.dataclass(frozen=True)
class Options:
    """What every capture in this sweep is run with."""

    work: pathlib.Path
    fixtures: pathlib.Path
    max_rounds: int = 5
    repeats: int = 900
    policy: str = "attack"
    durable: bool = True
    base_tape: pathlib.Path = ROOT / "oracle" / "tapes" / "07_first_battle.tape"
    data_dir: pathlib.Path = ROOT / "generated"
    ram_map: pathlib.Path = ROOT / "oracle" / "ram_map.json"

    @property
    def scout(self) -> pathlib.Path:
        """One scout cache for the whole sweep: the base tape's own run is the
        same 36k frames for every formation, so it is warmed once and read
        after."""
        return self.work / "scout.json"


def fixture_path(options: Options, formation: int) -> pathlib.Path:
    return options.fixtures / f"formation_{formation:02X}.json"


def force_argv(entry: Formation, options: Options) -> list[str]:
    """The capture command: the force tool, with this sweep's own options."""
    argv = [sys.executable, "-m", FORCE, "--formation", entry.hex,
            "--out", str(options.work / f"formation_{entry.formation:02X}"),
            "--base-tape", str(options.base_tape),
            "--data-dir", str(options.data_dir),
            "--ram-map", str(options.ram_map),
            "--scout", str(options.scout),
            "--policy", options.policy, "--repeats", str(options.repeats),
            "--max-rounds", str(options.max_rounds)]
    if options.durable:
        argv.append("--durable")
    if entry.vehicle is not None:
        argv += ["--vehicle", str(entry.vehicle)]
    return argv


def extractor_argv(report: dict, entry: Formation,
                   options: Options) -> list[str]:
    """The extraction command, for the log the capture just wrote."""
    argv = [sys.executable, "-m", FX, "--trace", report["trace"],
            "--log", report["log"],
            "--out", str(fixture_path(options, entry.formation)),
            "--tape", report["tape"],
            "--ram-map", str(options.ram_map),
            "--battle-first", str(report["battle_first"]),
            "--battle-last", str(report["battle_last"]),
            "--max-rounds", str(options.max_rounds), "--minified"]
    if options.durable:
        argv += ["--hp-patch", str(report["durable"]["hp"])
                 if report.get("durable") else "999"]
    return argv


def run(argv: list[str], log: pathlib.Path) -> subprocess.CompletedProcess:
    """Run one tool, keeping its whole output beside the formation's receipt.

    `cwd` is the repository root, the directory the tools are modules of: the
    command `python3 -m oracle.force` resolves its package from there, whatever
    directory the sweep itself was started in.
    """
    log.parent.mkdir(parents=True, exist_ok=True)
    with open(log, "w") as handle:
        proc = subprocess.run(argv, cwd=ROOT, stdout=handle,
                              stderr=subprocess.STDOUT, text=True)
    return proc


def tail(path: pathlib.Path, lines: int = 12) -> str:
    """The last few lines of a tool's log, for a failure's record."""
    try:
        text = path.read_text().splitlines()
    except OSError as error:
        return f"(cannot read {path}: {error})"
    return "\n".join(text[-lines:])


def relative(path: pathlib.Path, root: pathlib.Path = ROOT) -> str:
    """`path` under `root` where it is, and its own path where it is not.

    A sweep's fixture directory defaults to the repository's
    (`rust/psiv-core/src/battle/replay_fixtures/sweep_motavia`), and `--fixtures`
    may point anywhere - including a scratch tree the tests use - so the record
    names the path it can: relative when that is meaningful.
    """
    try:
        return str(path.relative_to(root))
    except ValueError:
        return str(path)


def sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def run_formation(entry: Formation, options: Options) -> dict:
    """Capture this formation and extract its fixture; the record entry either way."""
    work = options.work / f"formation_{entry.formation:02X}"
    record = {**entry.as_json(), "status": "failed", "stage": "capture"}
    capture_argv = force_argv(entry, options)
    record["commands"] = {"force": capture_argv}
    proc = run(capture_argv, work / "force.log")
    record["force_status"] = proc.returncode
    if proc.returncode != 0:
        record["error"] = tail(work / "force.log")
        return record
    try:
        report = json.loads((work / "report.json").read_text())
    except (OSError, ValueError) as error:
        record["error"] = f"the capture wrote no readable report.json: {error}"
        return record
    extract = extractor_argv(report, entry, options)
    record["commands"]["extract"] = extract
    record["stage"] = "extract"
    proc = run(extract, work / "extract.log")
    record["extract_status"] = proc.returncode
    if proc.returncode != 0:
        record["error"] = tail(work / "extract.log")
        return record
    fixture = fixture_path(options, entry.formation)
    try:
        document = json.loads(fixture.read_text())
    except (OSError, ValueError) as error:
        record["error"] = f"the extractor wrote no readable fixture: {error}"
        return record
    record.update(captured(report, fixture, document))
    return record


def captured(report: dict, fixture: pathlib.Path, document: dict) -> dict:
    """What the record says about a formation the sweep did capture.

    The fixture is the authority on the battle's shape - it is what the replay
    reads - so the outcome and the round count are the fixture's, with the
    report's own reading beside them (`report_outcome`) rather than in place of
    it: a capture whose battle outlasted the round cap is `truncated` in the
    fixture even when the fight, run further, ended.
    """
    outcome = document["outcome"]
    return {
        "status": "captured",
        "stage": "extracted",
        "error": None,
        "selector": {"group": report["selector"]["group"],
                     "kind": report["selector"]["kind"],
                     "entry": report["selector"]["entry"],
                     "label": report["selector"]["label"]},
        "durable": report["durable"],
        "max_rounds": report["max_rounds"],
        "outcome": "truncated" if outcome.get("truncated") else report["outcome"],
        "report_outcome": report["outcome"],
        "truncated": bool(outcome.get("truncated")),
        "rounds": len(document["rounds"]),
        "rounds_captured": outcome.get("rounds_captured"),
        "cut_frame": report["cut_frame"],
        "abilities": report["abilities"],
        "enemies": report["enemies"],
        "capture": {"battle_first": report["battle_first"],
                    "battle_last": report["battle_last"],
                    "start_frame": report["start_frame"],
                    "log_sha256": report["log_sha256"],
                    "trace_sha256": report["trace_sha256"]},
        "fixture": {"path": relative(fixture),
                    "sha256": sha256(fixture),
                    "bytes": fixture.stat().st_size},
    }


def is_captured(record: dict, options: Options) -> bool:
    """Whether this formation's fixture is on disk and the record still matches."""
    if record.get("status") != "captured":
        return False
    fixture = fixture_path(options, record["formation"])
    if not fixture.exists():
        return False
    return sha256(fixture) == record.get("fixture", {}).get("sha256")


def load_report(work: pathlib.Path) -> dict:
    try:
        return json.loads((work / "report.json").read_text())
    except (OSError, ValueError) as error:
        raise ForceError(f"{work}: no readable report.json ({error})")
