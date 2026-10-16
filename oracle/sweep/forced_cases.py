"""Forced-capture recipes: one `oracle.force` capture and one fixture per case.

The machinery the stretch recipes share (`oracle.sweep.air_castle`,
`oracle.sweep.endgame`). Every case is one `python3 -m oracle.force` capture
with `--require-ability`: a formation, or with `event` an event battle's
`Event_Battle_Index`. A case with a `script` drives the party through the
cartridge's own menus (`--party-script`); its party fixture is the declared RAM
patch `oracle.sweep.player_capture.patches` builds, and its `--prepare-script`
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
import hashlib
import json
import pathlib
import subprocess
import sys
from collections.abc import Sequence

from .player_capture import patches

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
    #: A `player_capture` case body: `rounds` of commands and the optional
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

    @property
    def kept(self) -> int:
        return self.keep or self.rounds

    @property
    def required(self) -> tuple[int, ...]:
        """Every ability the kept rounds must show (none for a `seated` case)."""
        if self.seated is not None or self.ability is None:
            return ()
        return (self.ability, *self.also)


def defend(*slots: int) -> dict:
    """The DEFEND command for each named party slot."""
    return {str(slot): {"command": "defend"} for slot in slots}


def capture_argv(case: Case, work: pathlib.Path) -> list[str]:
    selector = (["--event", str(case.event)] if case.event is not None
                else ["--formation", f"0x{case.formation:X}"])
    out = work / case.name
    argv = [sys.executable, "-m", "oracle.force", *selector, "--durable",
            "--max-rounds", str(case.rounds), "--delay", str(case.delay),
            "--out", str(out)]
    for ability in case.required:
        argv += ["--require-ability", f"0x{ability:X}"]
    if case.script is None:
        return argv + ["--scout", str(work / "scout.json")]
    scout = work / "script-scout.json"
    facts = json.loads(scout.read_text())
    raw = pathlib.Path(facts["script_state"]["path"]).read_bytes()
    if hashlib.sha256(raw).hexdigest() != facts["script_state"]["sha256"]:
        raise ValueError(f"{scout}: the cached script state changed")
    out.mkdir(parents=True, exist_ok=True)
    commands = out / "commands.json"
    commands.write_text(json.dumps({"rounds": case.script["rounds"],
                                    "repeat_last": True}, indent=2) + "\n")
    argv += ["--scout", str(scout), "--party-script", str(commands),
             "--repeats", str(case.repeats)]
    for spec in patches(case.script, raw, facts["battle_first"] + 1):
        argv += ["--ram-patch", spec]
    return argv


def extract_argv(case: Case, work: pathlib.Path, out: pathlib.Path) -> list[str]:
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
        argv += ["--ram-map", str(ROOT / "oracle" / "ram_map.json")]
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


def run_cases(cases: Sequence[Case], fixtures: pathlib.Path, default_work: pathlib.Path,
              description: str, argv: list[str] | None = None) -> int:
    """The recipe's command line: `--list`, `--capture`, `--extract`, `--only`."""
    parser = argparse.ArgumentParser(description=description)
    parser.add_argument("--work", default=str(default_work))
    parser.add_argument("--list", action="store_true")
    parser.add_argument("--capture", action="store_true")
    parser.add_argument("--extract", action="store_true")
    parser.add_argument("--only", help="a case name")
    parser.add_argument("--table", action="store_true",
                        help="print the captures table from the committed fixtures "
                             "and the work directory's reports")
    parser.add_argument("--out", default=str(fixtures))
    arguments = parser.parse_args(argv)
    work = pathlib.Path(arguments.work)
    out = pathlib.Path(arguments.out)
    chosen = [case for case in cases if arguments.only in (None, case.name)]
    if not chosen:
        parser.error(f"no case named {arguments.only}")
    if arguments.table:
        print(captures_table(chosen, work, out))
        return 0
    if arguments.list or not (arguments.capture or arguments.extract):
        for case in chosen:
            print(f"{case.name}: {case.note}")
        return 0
    status = 0
    for case in chosen:
        if arguments.capture:
            proc = subprocess.run(capture_argv(case, work), cwd=ROOT)
            if proc.returncode != 0:
                print(f"{case.name}: the capture failed ({proc.returncode})")
                status = 1
                continue
        if arguments.extract:
            out.mkdir(parents=True, exist_ok=True)
            target = out / f"{case.name}.json"
            proc = subprocess.run(extract_argv(case, work, target), cwd=ROOT,
                                  stdout=subprocess.DEVNULL)
            if proc.returncode != 0:
                print(f"{case.name}: the extraction failed ({proc.returncode})")
                status = 1
                continue
            found = observed(case, json.loads(target.read_text()))
            if found is None:
                target.unlink()
                wanted = case.seated if case.seated is not None else case.required
                print(f"{case.name}: {wanted} is not in the kept rounds")
                status = 1
                continue
            shown = ", ".join(f"${key:02X} in round(s) {rounds}"
                              for key, rounds in found.items())
            print(f"{case.name}: {target.name}, {shown}")
    return status
