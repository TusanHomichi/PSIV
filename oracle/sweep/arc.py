"""The Motavia-arc fixtures: forced captures of the status and stat abilities.

    python3 -m oracle.sweep.arc --list
    python3 -m oracle.sweep.arc --capture --work build/arc
    python3 -m oracle.sweep.arc --extract --work build/arc

`--list` prints the table below with the exact commands. `--capture` runs
`python3 -m oracle.force` for every entry, one at a time (an emulator run plus a
~30MB log parse each; the machine's memory cap is why they are never
concurrent), and `--extract` turns each capture into the committed fixture
(`rust/psiv-core/src/battle/replay_fixtures/arc_motavia/`) with
`python3 -m oracle.fixture` and then **adds the two battle cells the extractor
does not read**.

The extractor's effect columns for a party member are `status`, `str`,
`agi_bat`, `dex`, `atk`, `dfs` and `men` (`oracle/fixture/observations.py`'s
`EFFECT_FIELDS`), and `atk` / `dfs` are the *derived* cells `$24` / `$28`, which
a battle buff never moves: GELUN's attack-down (`atk_pow_battle`, `$26`) would
leave no mark in a fixture, and the party's physical hits on the enemy that casts
it deal the damage floor of 1 either way. The RAM log does carry
`<member>_atk_bat` and `<member>_dfs_bat` (`oracle/ram_map.tsv`), so this tool
reads them over each ability action's frames, exactly as the extractor reads the
others (`log.signed(start, column)` against `log.signed(end, column)`), and
appends `[fighter id, "atk_bat" | "dfs_bat", before, after]` to the action's
`effect.stats`. The replay comparator checks them
(`rust/psiv-core/src/battle/replay/compare.rs`). The enemies' `$26` / `$2A` are
not in the log at all, so DEBAN's effect on its own side is proven only through
the damage the party deals afterwards.
"""
from __future__ import annotations

import argparse
import dataclasses
import json
import pathlib
import subprocess
import sys

from .. import fixture

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
FIXTURES = (ROOT / "rust" / "psiv-core" / "src" / "battle"
            / "replay_fixtures" / "arc_motavia")
#: The party's log prefixes by fighter id (`oracle/fixture/observations.py`).
MEMBERS = ((1, "alys"), (2, "chaz"), (3, "hahn"))
#: The cells added to every ability action's effect.
EXTRA = ("atk_bat", "dfs_bat")


@dataclasses.dataclass(frozen=True)
class Entry:
    """One committed fixture and the capture it comes from."""

    formation: int
    delay: int
    #: `--max-rounds` of the capture.
    capture_rounds: int
    #: Rounds the fixture keeps: the capture cut before the first ability
    #: another lane owns (`docs/oracle/BATTLE_ORACLE_ARC.md`).
    rounds: int
    #: What the fixture is for.
    note: str

    @property
    def name(self) -> str:
        return f"formation_{self.formation:X}_d{self.delay}"

    @property
    def capture(self) -> str:
        return f"f_{self.formation:X}_d{self.delay}"


ENTRIES = (
    Entry(0xD7, 0, 8, 8, "VOICE: FlyScreamr x3; the party is paralyzed and loses"),
    Entry(0xD5, 0, 8, 8, "VOICE: one FlyScreamr; sleepers wake on the round-end roll"),
    Entry(0xD6, 0, 8, 8, "VOICE: two FlyScreamr; wake rolls, a repeat on a sleeper"),
    Entry(0xD6, 1, 8, 6, "VOICE: two FlyScreamr, a second shift of the same fight"),
    Entry(0x100, 0, 10, 10, "STASISBALL: Blauzen; two landings and one skipped target"),
    Entry(0x1A3, 0, 8, 8, "STASISBALL: Goldine; three landings"),
    Entry(0x1A5, 5, 8, 1, "STASISBALL: LifeDeletr, before its MICROMISSL"),
    Entry(0x119, 0, 12, 3, "SEALS, DORAN, SEALS: Greneris"),
    Entry(0x119, 2, 6, 3, "SEALS, GELUN, DORAN: Greneris"),
    Entry(0x119, 8, 8, 3, "VOL kills, RIMIT sleeps, DORAN: Greneris"),
    Entry(0x1C6, 0, 8, 8, "VOL: two BloodSaber kill the party"),
    Entry(0x1C6, 1, 8, 3, "VOL: two BloodSaber, three kills in three rounds"),
    Entry(0x1E1, 0, 8, 1, "VOL: SoldrFiend (a miss), before BLADESHINE"),
    Entry(0xFB, 1, 8, 3, "EVIL EYE: two Haunt; the party wins"),
    Entry(0xFB, 2, 8, 3, "EVIL EYE: two Haunt, a second shift of the same fight"),
    Entry(0x16D, 1, 8, 2, "EVIL EYE: two Spector, before CORRSION"),
    Entry(0x12E, 10, 8, 2, "RIMIT: two TechPlant, before GIZAN"),
    Entry(0x12E, 12, 8, 2, "RIMIT: two TechPlant, a second shift of the same fight"),
    Entry(0xD8, 0, 10, 10, "DEBAN then its guard: ShadowSabr"),
    Entry(0xD2, 2, 8, 8, "Fusion: two Zol slugs become a MetaSlug"),
    Entry(0x116, 2, 8, 8, "GIRES: TechMaster beside a ZiosGuard"),
)


def capture_argv(entry: Entry, work: pathlib.Path) -> list[str]:
    return [sys.executable, "-m", "oracle.force",
            "--formation", f"0x{entry.formation:X}", "--durable",
            "--max-rounds", str(entry.capture_rounds),
            "--delay", str(entry.delay),
            "--scout", str(work / "scout.json"),
            "--out", str(work / entry.capture)]


def extract_argv(entry: Entry, work: pathlib.Path, out: pathlib.Path
                 ) -> list[str]:
    report = json.loads((work / entry.capture / "report.json").read_text())
    argv = [sys.executable, "-m", "oracle.fixture",
            "--trace", report["trace"], "--log", report["log"],
            "--out", str(out), "--tape", report["tape"],
            "--ram-map", str(ROOT / "oracle" / "ram_map.json"),
            "--battle-first", str(report["battle_first"]),
            "--battle-last", str(report["battle_last"]),
            "--max-rounds", str(entry.rounds), "--minified"]
    if report.get("durable"):
        argv += ["--hp-patch", str(report["durable"]["hp"])]
    return argv


def augment(path: pathlib.Path, log_path: str) -> int:
    """Append the battle attack and defence cells to each ability action.

    Returns how many cells moved. A fixture the extractor wrote is read back,
    changed in `effect.stats` only, and written in the same minified layout.
    """
    document = json.loads(path.read_text())
    ram_map = fixture.load_ram_map(ROOT / "oracle" / "ram_map.json")
    log = fixture.Log(fixture.load_rows(log_path), ram_map)
    moved = 0
    for round_ in document["rounds"]:
        for action in round_["actions"]:
            if action["kind"] == "attack":
                continue
            for fighter, prefix in MEMBERS:
                for field in EXTRA:
                    column = f"{prefix}_{field}"
                    if not log.has(column):
                        continue
                    before = log.signed(action["start_frame"], column)
                    after = log.signed(action["end_frame"], column)
                    if before != after:
                        action["effect"]["stats"].append(
                            [fighter, field, before, after])
                        moved += 1
    document["provenance"]["effect_cells_added"] = list(EXTRA)
    path.write_text(json.dumps(document, separators=(",", ":"),
                               sort_keys=False) + "\n")
    return moved


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--work", default=str(ROOT / "build" / "arc"))
    parser.add_argument("--list", action="store_true")
    parser.add_argument("--capture", action="store_true")
    parser.add_argument("--extract", action="store_true")
    parser.add_argument("--only", help="a fixture name, for one entry")
    parser.add_argument("--out", default=str(FIXTURES))
    arguments = parser.parse_args(argv)
    work = pathlib.Path(arguments.work)
    out = pathlib.Path(arguments.out)
    entries = [entry for entry in ENTRIES
               if arguments.only in (None, entry.name)]
    if arguments.list or not (arguments.capture or arguments.extract):
        for entry in entries:
            print(f"{entry.name}: {entry.note}")
            print("  capture: " + " ".join(capture_argv(entry, work)))
        return 0
    status = 0
    for entry in entries:
        if arguments.capture:
            proc = subprocess.run(capture_argv(entry, work), cwd=ROOT)
            if proc.returncode != 0:
                print(f"{entry.name}: the capture failed ({proc.returncode})")
                status = 1
                continue
        if arguments.extract:
            target = out / f"{entry.name}.json"
            argv_ = extract_argv(entry, work, target)
            proc = subprocess.run(argv_, cwd=ROOT, stdout=subprocess.DEVNULL)
            if proc.returncode != 0:
                print(f"{entry.name}: the extraction failed ({proc.returncode})")
                status = 1
                continue
            report = json.loads(
                (work / entry.capture / "report.json").read_text())
            moved = augment(target, report["log"])
            print(f"{entry.name}: {target.name}, {moved} battle cell(s) added")
    return status


if __name__ == "__main__":
    raise SystemExit(main())
