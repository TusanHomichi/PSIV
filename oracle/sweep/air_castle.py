"""The Air Castle fixtures: forced captures of the stretch's enemy abilities.

    python3 -m oracle.sweep.air_castle --list
    python3 -m oracle.sweep.air_castle --capture --work build/air-castle
    python3 -m oracle.sweep.air_castle --extract --work build/air-castle
    python3 -m oracle.sweep.air_castle --capture --extract --only combine

The recipe for `docs/battle/ENEMY_ABILITIES_AIR_CASTLE.md`. Every case is one
`python3 -m oracle.force` capture with `--require-ability`: a formation, or with
`event` an event battle's `Event_Battle_Index`. A case with a `script` drives the
party through the cartridge's own menus (`--party-script`); its party fixture is
the declared RAM patch `oracle.sweep.player_capture.patches` builds, and its
`--prepare-script` scout is `<work>/script-scout.json`. The rest keep the base
tape's party with `--durable` HP. The set of abilities the cases exist for is
derived, not listed: `python3 -m oracle.sweep.route_abilities --stretch
dezolis-air-castle`.

`--extract` writes each fixture into
`rust/psiv-core/src/battle/replay_fixtures/air_castle/`, keeping the rounds the
case names, and refuses one whose kept rounds do not show the required ability
in an enemy action - a capture counts only when the ability was used. COMBINE
clears `$24(a4)` in its own arm (`ps4.asm:21550`, `21621`), so no frame of the
log holds its id: its case names the enemy the reload seats (`seated`) instead,
and counts when a round's end state shows that enemy in a slot the formation
did not start it in.
"""
from __future__ import annotations

import argparse
import dataclasses
import hashlib
import json
import pathlib
import subprocess
import sys

from .player_capture import patches

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
FIXTURES = (ROOT / "rust" / "psiv-core" / "src" / "battle"
            / "replay_fixtures" / "air_castle")


@dataclasses.dataclass(frozen=True)
class Case:
    """One committed fixture and the capture it comes from."""

    name: str
    note: str
    #: The enemy ability the capture must show.
    ability: int
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

    @property
    def kept(self) -> int:
        return self.keep or self.rounds


def _defend(*slots: int) -> dict:
    return {str(slot): {"command": "defend"} for slot in slots}


#: A party that outlasts the stretch's bosses and acts after them.
_ENDURING = {"hp": 999, "max_hp": 999, "agility": 1}

CASES: tuple[Case, ...] = (
    Case("gra", "GRA: two DimensWorm, four rounds, three uses", 0x31, 4,
         formation=0x167),
    Case("deban", "DEBAN: two FrostSaber, after the party's physical hits", 0x2D, 6,
         formation=0x164),
    Case("dthspell", "DTHSPELL: two Spector, both cast it", 0x4E, 6, keep=3,
         formation=0x16D),
    Case("airslash", "AIRSLASH: FrostSaber at half HP after two FOIs", 0x2C, 4,
         formation=0x164, script={
             "defaults": {"hp": 999, "max_hp": 999},
             "party": {"1": {"mental": 30, "agility": 255}},
             "rounds": [{"1": {"command": "technique", "id": 1, "target": 6},
                         **_defend(2, 3)}] * 2 + [_defend(1, 2, 3)] * 2}),
    # HakenLeft's `$3B` won the race in the capture: the pair's arms are one
    # object, `$354`. TwinArms acts in round 3 with BLADESHINE, which no lane
    # runs yet, so the fixture keeps two rounds.
    Case("combine", "COMBINE: one BladeRight falls, its pair becomes TwinArms", 0x3B, 3,
         keep=2, seated=87, formation=0x1BA, script={
             # Agility 120: a byte of $80 or more reads as negative to both the
             # opening roll and the turn order (`fighters.rs`).
             "defaults": {"hp": 999, "max_hp": 999, "agility": 120},
             # Chaz: Alys's weapon strikes the whole side (no target cursor).
             # Dexterity 120 lands the hit on BladeRight's agility 49.
             "party": {"2": {"attack": 999, "dexterity": 120}},
             "rounds": [{"2": {"command": "attack", "target": 6}, **_defend(1, 3)},
                        _defend(1, 2, 3)]}),
    Case("xe_a_thoul", "GIZAN and THNDRBLAST: the three Xe-A-Thouls after a ZAN", 0x5C, 4,
         event=14, script={
             "defaults": _ENDURING,
             "party": {"1": {"mental": 20}},
             "rounds": [{"1": {"command": "technique", "id": 10, "target": -1},
                         **_defend(2, 3)}] + [_defend(1, 2, 3)] * 3}),
    # Lashiec strikes the whole party for hundreds a turn: both defences at
    # 999 keep a 999-HP party standing. Alys's boomerang has no target cursor.
    Case("lashiec", "Lashiec: ANOTHRGATE, THNDHALBRT and POSESSION", 0x60, 6,
         event=16, script={
             "defaults": {**_ENDURING, "attack": 500, "defence": 999,
                          "mental_defence": 999},
             "rounds": [{"1": {"command": "attack", "target": -1},
                         "2": {"command": "attack", "target": 6},
                         "3": {"command": "attack", "target": 6}}]}),
    # The same party, now landing every swing (dexterity 120 on Lashiec's
    # agility 34): three hits of roughly 475 a round take its 6383 HP under a
    # quarter by the fourth round, and - acting first - it spends its next
    # turn on REINFORCE.
    Case("reinforce", "REINFORCE: Lashiec under a quarter of its HP", 0x62, 6,
         event=16, script={
             "defaults": {**_ENDURING, "attack": 500, "dexterity": 120,
                          "defence": 999, "mental_defence": 999},
             "rounds": [{"1": {"command": "attack", "target": -1},
                         "2": {"command": "attack", "target": 6},
                         "3": {"command": "attack", "target": 6}}]}),
    Case("dark_force_2", "Dark Force 2: the latch's first action, then its three arms",
         0x64, 8, event=17),
)


def capture_argv(case: Case, work: pathlib.Path) -> list[str]:
    selector = (["--event", str(case.event)] if case.event is not None
                else ["--formation", f"0x{case.formation:X}"])
    out = work / case.name
    argv = [sys.executable, "-m", "oracle.force", *selector, "--durable",
            "--max-rounds", str(case.rounds), "--delay", str(case.delay),
            "--out", str(out)]
    if case.seated is None:
        argv += ["--require-ability", f"0x{case.ability:X}"]
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
             "--repeats", "900"]
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


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--work", default=str(ROOT / "build" / "air-castle"))
    parser.add_argument("--list", action="store_true")
    parser.add_argument("--capture", action="store_true")
    parser.add_argument("--extract", action="store_true")
    parser.add_argument("--only", help="a case name")
    parser.add_argument("--out", default=str(FIXTURES))
    arguments = parser.parse_args(argv)
    work = pathlib.Path(arguments.work)
    out = pathlib.Path(arguments.out)
    cases = [case for case in CASES if arguments.only in (None, case.name)]
    if not cases:
        parser.error(f"no case named {arguments.only}")
    if arguments.list or not (arguments.capture or arguments.extract):
        for case in cases:
            print(f"{case.name}: {case.note}")
        return 0
    status = 0
    for case in cases:
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
            fixture = json.loads(target.read_text())
            rounds = (used_in(fixture, case.ability) if case.seated is None
                      else seated_in(fixture, case.seated))
            if not rounds:
                target.unlink()
                print(f"{case.name}: ${case.ability:02X} is not in the kept rounds")
                status = 1
                continue
            print(f"{case.name}: {target.name}, ${case.ability:02X} in round(s) {rounds}")
    return status


if __name__ == "__main__":
    raise SystemExit(main())
