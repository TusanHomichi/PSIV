"""Forced-capture recipes: one `oracle.force` capture and one fixture per case.

The machinery the stretch recipes share (`oracle.sweep.air_castle`,
`oracle.sweep.endgame`). Every case is one `python3 -m oracle.force` capture
with `--require-ability`: a formation, or with `event` an event battle's
`Event_Battle_Index`. A case with a `script` drives the party through the
cartridge's own menus (`--party-script`); its party fixture is the declared RAM
patch `oracle.sweep.party_fixture.patches` builds, and its `--prepare-script`
scout is `<work>/script-scout.json`. The rest keep the base tape's party with
`--durable` HP.

`--extract` writes each fixture into the recipe's fixture directory, keeping the
rounds the case names, and refuses one whose kept rounds do not show every
required ability in an enemy action - a capture counts only when the ability was
used. An arm that clears `$24(a4)` before the log can see it (COMBINE,
`ps4.asm:21550`, `21621`) names the enemy its reload seats (`seated`) instead,
and counts when a round's end state shows that enemy in a slot the formation did
not start it in.
"""
from __future__ import annotations

import argparse
import dataclasses
import datetime
import time
import hashlib
import json
import pathlib
import subprocess
import sys
from collections.abc import Sequence

from .party_fixture import patches, require_executed, verify_report
from .capture_checks import observed_rounds, out_of_lane

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
FIXTURE_ROOT = ROOT / "rust" / "psiv-core" / "src" / "battle" / "replay_fixtures"


@dataclasses.dataclass(frozen=True)
class Case:
    """One committed fixture and the capture it comes from."""

    name: str
    note: str
    #: The enemy ability the capture must show; `None` for a capture kept for
    #: a scripted turn the log files under no ability (a latch's first action).
    ability: int | None
    #: `--max-rounds` of the capture.
    rounds: int
    #: Rounds the fixture keeps (the capture cut before an ability another
    #: lane owns), when fewer than `rounds`.
    keep: int | None = None
    formation: int | None = None
    event: int | None = None
    delay: int = 0
    #: A party-fixture body: `rounds` of commands and the optional
    #: `defaults` / `party` / `characters` fixture.
    script: dict | None = None
    #: For an ability whose arm clears `$24(a4)` before the log can see it: the
    #: enemy id its reload seats, which is what the capture is checked for.
    seated: int | None = None
    #: Further abilities the same capture must show.
    also: tuple[int, ...] = ()
    #: `--repeats` for a scripted case: 16-frame input blocks after the
    #: encounter, which bound how long the battle can run.
    repeats: int = 900
    durable: bool = True
    augment_cells: bool = False
    damage: bool = False
    allowed: tuple[int, ...] | None = None
    max_delay: int | None = None
    vehicle: int | None = None
    policy: str = "attack"
    base_tape: str | None = None
    data_dir: str | None = None
    ram_map: str | None = None

    def __post_init__(self) -> None:
        if (not self.name or self.name in (".", "..")
                or pathlib.Path(self.name).name != self.name):
            raise ValueError("case names must be basenames")
        if (self.formation is None) == (self.event is None):
            raise ValueError("case needs exactly one formation or event")
        if (self.rounds < 0 or (self.keep is not None and self.keep <= 0)
                or (self.rounds > 0 and not 1 <= self.kept <= self.rounds)):
            raise ValueError("case needs nonnegative rounds (0 = whole battle); bounded keep must be 1..rounds")
        if self.delay < 0 or (self.max_delay is not None and self.max_delay < self.delay):
            raise ValueError("case delay range must be nonnegative and ordered")
        if self.event is not None and self.script is not None and self.max_delay not in (None, self.delay):
            raise ValueError("scripted event delays do not reliably shift rolls; lengthen rounds or change commands")

    @property
    def kept(self) -> int:
        return self.rounds if self.keep is None else self.keep

    @property
    def required(self) -> tuple[int, ...]:
        """Every ability the kept rounds must show (none for a `seated` case)."""
        if self.seated is not None or self.ability is None:
            return ()
        return (self.ability, *self.also)


def defend(*slots: int) -> dict:
    """The DEFEND command for each named party slot."""
    return {str(slot): {"command": "defend"} for slot in slots}


def capture_argv(case: Case, work: pathlib.Path, scout: pathlib.Path | None = None) -> list[str]:
    selector = (["--event", str(case.event)] if case.event is not None
                else ["--formation", f"0x{case.formation:X}"])
    out = work / case.name
    argv = [sys.executable, "-m", "oracle.force", *selector,
            "--max-rounds", str(case.rounds), "--delay", str(case.delay),
            "--out", str(out)]
    if case.durable:
        argv.append("--durable")
    for flag, value in (("--vehicle", case.vehicle), ("--base-tape", case.base_tape),
                        ("--data-dir", case.data_dir), ("--ram-map", case.ram_map)):
        if value is not None:
            argv += [flag, str(value)]
    argv += ["--policy", case.policy, "--repeats", str(case.repeats)]
    for ability in case.required:
        argv += ["--require-ability", f"0x{ability:X}"]
    if case.script is None:
        return argv + ["--scout", str(scout or work / "scout.json")]
    scout = scout or work / "script-scout.json"
    facts = json.loads(scout.read_text())
    raw = pathlib.Path(facts["script_state"]["path"]).read_bytes()
    if hashlib.sha256(raw).hexdigest() != facts["script_state"]["sha256"]:
        raise ValueError(f"{scout}: the cached script state changed")
    out.mkdir(parents=True, exist_ok=True)
    commands = out / "commands.json"
    commands.write_text(json.dumps({"rounds": case.script["rounds"],
                                    "repeat_last": case.script.get("repeat_last", True)}, indent=2) + "\n")
    argv += ["--scout", str(scout), "--party-script", str(commands)]
    for spec in patches(case.script, raw, facts["battle_first"] + 1):
        argv += ["--ram-patch", spec]
    return argv


def extract_argv(case: Case, work: pathlib.Path, out: pathlib.Path,
                 report: dict | None = None) -> list[str]:
    if report is None:
        report = json.loads((work / case.name / "report.json").read_text())
    argv = [sys.executable, "-m", "oracle.fixture",
            "--trace", report["trace"], "--log", report["log"],
            "--out", str(out), "--tape", report["tape"],
            "--battle-first", str(report["battle_first"]),
            "--battle-last", str(report["battle_last"]),
            "--max-rounds", str(case.kept), "--minified"]
    if report.get("script_ram_map"):
        argv += ["--ram-map", report["script_ram_map"],
                 "--start-ram", report["start_ram"],
                 "--start-ram-sha256", report["start_ram_sha256"]]
    else:
        argv += ["--ram-map", str(case.ram_map or ROOT / "oracle" / "ram_map.json")]
    if report.get("durable"):
        argv += ["--hp-patch", str(report["durable"]["hp"])]
    if case.event is not None:
        argv += ["--event-battle", str(case.event)]
    return argv


def seated_in(fixture: dict, enemy: int) -> list[int]:
    """The kept rounds that end with `enemy` in a slot it did not start in."""
    start = {entry["id"]: entry["enemy_id"] for entry in fixture["formation"]["enemies"]}
    return sorted({round_["round"] for round_ in fixture["rounds"]
                   for state in round_.get("state_after", [])
                   if state["id"] > 5 and state.get("enemy_id") == enemy
                   and start.get(state["id"]) != enemy})


def used_in(fixture: dict, ability: int) -> list[int]:
    """The kept rounds in which an enemy action ran `ability`."""
    return sorted({round_["round"] for round_ in fixture["rounds"]
                   for action in round_["actions"]
                   if action["actor"] > 5 and action.get("ability") == ability})


def observed(case: Case, fixture: dict) -> dict[int, list[int]] | None:
    """Each required ability's rounds, or `None` when one is missing.

    A `seated` case is keyed by the seated enemy instead.
    """
    if case.seated is not None:
        rounds = seated_in(fixture, case.seated)
        return {case.seated: rounds} if rounds else None
    if case.ability is None:
        return {}
    found = {ability: used_in(fixture, ability) for ability in case.required}
    return found if all(found.values()) else None


def captures_table(cases: Sequence[Case], work: pathlib.Path,
                   fixtures: pathlib.Path) -> str:
    """One markdown row per committed case: the battle, what the kept rounds
    show, how many rounds were kept and the capture's trace hash."""
    rows = ["| fixture | battle | enemy abilities observed (rounds) | rounds kept "
            "| outcome | trace sha256 |", "|---|---|---|---|---|---|"]
    for case in cases:
        path = fixtures / f"{case.name}.json"
        report_path = work / case.name / "report.json"
        receipt_path = work / f"{case.name}.receipt.json"
        if receipt_path.exists():
            receipt = json.loads(receipt_path.read_text())
            if receipt.get("status") == "captured" and receipt.get("report"):
                report_path = pathlib.Path(receipt["report"])
        if not path.exists() or not report_path.exists():
            continue
        fixture = json.loads(path.read_text())
        report = json.loads(report_path.read_text())
        battle = (f"event {case.event}" if case.event is not None
                  else f"`${case.formation:X}`")
        seen: dict[int, list[int]] = {}
        for round_ in fixture["rounds"]:
            for action in round_["actions"]:
                if action["actor"] > 5 and action.get("ability"):
                    seen.setdefault(action["ability"], []).append(round_["round"])
        shown = ", ".join(f"`${ability:02X}` ({', '.join(map(str, sorted(set(r))))})"
                          for ability, r in sorted(seen.items())) or "-"
        outcome = fixture.get("outcome", {})
        verdict = next((key for key in ("victory", "defeat", "truncated")
                        if outcome.get(key)), "-")
        rows.append(f"| `{case.name}` | {battle} | {shown} | {len(fixture['rounds'])} "
                    f"| {verdict} | {report['trace_sha256'][:12]} |")
    return "\n".join(rows)


def load_cases(path: pathlib.Path) -> list[Case]:
    """Read common Case objects, accepting the old flat party-case body."""
    cases = []
    for source in json.loads(path.read_text()):
        row = dict(source)
        row.setdefault("note", "")
        row.setdefault("ability", None)
        if isinstance(row.get("rounds"), list):
            script = {key: row.pop(key) for key in
                      ("rounds", "defaults", "party", "characters", "inventory") if key in row}
            script["repeat_last"] = False
            row.update(script=script, rounds=len(script["rounds"]), durable=False)
        if "first_delay" in row:
            row["delay"] = row.pop("first_delay")
        cases.append(Case(**row))
    if len({case.name for case in cases}) != len(cases):
        raise ValueError("case names must be distinct")
    return cases


def command(argv: list[str], log: pathlib.Path, runner=None) -> dict:
    """One subprocess, with the same receipt layout for every recipe."""
    log.parent.mkdir(parents=True, exist_ok=True)
    started = time.monotonic()
    if runner is None:
        with log.open("w") as handle:
            proc = subprocess.run(argv, cwd=ROOT, stdout=handle, stderr=subprocess.STDOUT)
    else:
        proc = runner(argv, log)
    return {"command": argv, "exit": proc.returncode,
            "seconds": round(time.monotonic() - started, 3),
            "utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "log": str(log), "log_sha256": hashlib.sha256(log.read_bytes()).hexdigest()}


def validate(case: Case, fixture: dict) -> None:
    if observed(case, fixture) is None:
        raise ValueError("required enemy ability or seated enemy absent in kept rounds")
    if case.script:
        commands = case.script["rounds"]
        require_executed({**case.script, "rounds": commands[:case.kept] if case.kept else commands}, fixture)
    if case.damage and any(not observed_rounds(fixture, ability) for ability in case.required):
        raise ValueError("required damage ability absent in kept rounds")
    if case.allowed is not None and out_of_lane(fixture, set(case.allowed)):
        raise ValueError("kept rounds include an ability outside the case's set")


def run_case(case: Case, work: pathlib.Path, fixtures: pathlib.Path, *,
             capture: bool = True, extract: bool = True,
             scout: pathlib.Path | None = None, execute=command) -> dict:
    """The only capture/extract loop. Publish a fixture only after validation.

    Fresh delay ranges retry absence (force status 1) or rejected evidence;
    infrastructure errors stop fresh captures immediately. Extract-only checks
    each delay's successful capture identity and hashes, continuing past failed
    candidates so an earlier failure cannot hide a later reusable capture.
    """
    scout = scout or work / ("script-scout.json" if case.script else "scout.json")
    work.mkdir(parents=True, exist_ok=True)
    fixtures.mkdir(parents=True, exist_ok=True)
    result = {"name": case.name, "status": "failed", "attempts": []}
    fingerprint = hashlib.sha256(json.dumps(dataclasses.asdict(case), sort_keys=True).encode()).hexdigest()
    delays = range(case.delay, (case.max_delay if case.max_delay is not None else case.delay) + 1)
    # Successful capture identities survive extraction failures. Inspect every
    # delay's own marker rather than deriving resumability from the latest
    # receipt's extraction verdict (which may describe a failed re-extraction).
    for delay in delays:
        attempt_case = dataclasses.replace(case, delay=delay)
        attempt_work = work if case.max_delay is None else work / f"delay-{delay}"
        directory = attempt_work / case.name
        directory.mkdir(parents=True, exist_ok=True)
        attempt = {"delay": delay, "stage": "capture", "commands": {}}
        result["attempts"].append(attempt)
        try:
            identity = directory / "case.sha256"
            if capture:
                identity.unlink(missing_ok=True)
                attempt["force"] = execute(capture_argv(attempt_case, attempt_work, scout), directory / "force.log")
                attempt["commands"]["force"] = attempt["force"]["command"]
                if attempt["force"]["exit"]:
                    attempt["error"] = "capture failed"
                    if attempt["force"]["exit"] == 1:
                        continue
                    break
                identity.write_text(fingerprint + "\n")
            if not extract:
                result["status"] = "captured"
                break
            if identity.read_text().strip() != fingerprint:
                raise ValueError("case changed since capture")
            report = json.loads((directory / "report.json").read_text())
            if case.script:
                verify_report(report)
            elif not capture:
                for key in ("trace", "log"):
                    if hashlib.sha256(pathlib.Path(report[key]).read_bytes()).hexdigest() != report[key + "_sha256"]:
                        raise ValueError(f"capture {key} hash changed")
            attempt["stage"] = "extract"
            temporary = directory / f"{case.name}.json"
            temporary.unlink(missing_ok=True)
            attempt["extract"] = execute(extract_argv(attempt_case, attempt_work, temporary), directory / "extract.log")
            attempt["commands"]["extract"] = attempt["extract"]["command"]
            if attempt["extract"]["exit"]:
                attempt["error"] = "extraction failed"
                if not capture:
                    continue
                break
            if case.augment_cells:
                from .arc_cells import augment
                augment(temporary, report["log"])
            document = json.loads(temporary.read_text())
            try:
                validate(case, document)
            except ValueError as error:
                attempt["error"] = str(error)
                continue
            target = fixtures / f"{case.name}.json"
            target.write_bytes(temporary.read_bytes())
            result.update(status="captured", fixture=str(target),
                          fixture_sha256=hashlib.sha256(target.read_bytes()).hexdigest(),
                          report=str(directory / "report.json"))
            break
        except (OSError, ValueError, KeyError, TypeError) as error:
            attempt["error"] = str(error)
            if not capture:
                continue
            break
    (work / f"{case.name}.receipt.json").write_text(json.dumps(result, indent=2) + "\n")
    return result


def run_cases(cases: Sequence[Case], fixtures: pathlib.Path, default_work: pathlib.Path,
              description: str, argv: list[str] | None = None) -> int:
    """Recipe and case-file CLI: the same runner, options and evidence layout."""
    parser = argparse.ArgumentParser(description=description)
    parser.add_argument("--cases", type=pathlib.Path)
    parser.add_argument("--work", type=pathlib.Path, default=default_work)
    parser.add_argument("--scout", type=pathlib.Path)
    parser.add_argument("--list", action="store_true")
    parser.add_argument("--capture", action="store_true")
    parser.add_argument("--extract", action="store_true")
    parser.add_argument("--only", help="a case name")
    parser.add_argument("--table", action="store_true")
    parser.add_argument("--out", type=pathlib.Path, default=fixtures)
    arguments = parser.parse_args(argv)
    try:
        if arguments.cases:
            cases = load_cases(arguments.cases)
        chosen = [case for case in cases if arguments.only in (None, case.name)]
        if not chosen:
            parser.error(f"no case named {arguments.only}")
        if len({case.name for case in chosen}) != len(chosen):
            raise ValueError("case names must be distinct")
        if arguments.table:
            print(captures_table(chosen, arguments.work, arguments.out))
            return 0
        if arguments.list or not (arguments.capture or arguments.extract):
            for case in chosen:
                print(f"{case.name}: {case.note}")
            return 0
        status = 0
        for case in chosen:
            result = run_case(case, arguments.work, arguments.out,
                              capture=arguments.capture, extract=arguments.extract,
                              scout=arguments.scout)
            print(f"{case.name}: {result['status']}")
            status |= result["status"] != "captured"
        return int(status)
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(f"forced_cases: {error}", file=sys.stderr)
        return 2


def main(argv: list[str] | None = None) -> int:
    return run_cases((), FIXTURE_ROOT, ROOT / "build" / "captures",
                     __doc__.splitlines()[0], argv)


if __name__ == "__main__":
    raise SystemExit(main())
