"""Running `psiv_oracle`, and reading what its two CSVs hold.

Every phase of the tool is one of these runs: the same binary, the same core
and ROM, the same RAM map, one tape and one `--ram-patch` list. `run_oracle` is
the single seam the tests replace - it is called as a module attribute
(`runs.run_oracle`) so a test can stand in hand-built logs for the emulator -
and the readers below turn a run's RAM log and RNG trace into the values the
tool decides on.
"""
from __future__ import annotations

import csv
import dataclasses
import hashlib
import pathlib
import subprocess

ORACLE = pathlib.Path(__file__).resolve().parent.parent
ROOT = ORACLE.parent
BIN = ORACLE / "bin" / "psiv_oracle"
CORE = ORACLE / "core" / "genesis_plus_gx_libretro.so"
ROM = ROOT / "Phantasy Star IV (USA).md"
DEFAULT_RAM_MAP_TSV = ORACLE / "ram_map.tsv"

#: What every evidence run logs: the fight's own columns, the RNG chain
#: `python3 -m oracle.rng_trace check` re-derives, and the vehicle cells a
#: vehicle battle's party-side fighter is built from (`Vehicle_Index` and the
#: saved `Vehicle_Stats` record), which a fixture's `vehicle` section reads.
#: The groups every capture logs. `bcmd` is `Character_Command_Data` /
#: `Enemy_Command_Data` - what each fighter was told to do and *whom* to do it
#: to (`constants:2005-2011`) - and it is here because the party's command is
#: the one input a replay cannot otherwise recover: `Current_Target_Index` moves
#: off a commanded enemy that has fallen (`ps4.asm:8345-8409`), so a swing's
#: target is the command's own only when the command's target is still alive.
#: `oracle/ram_map.json` carries the cells; `docs/oracle/BATTLE_ORACLE_SWEEP.md`'s
#: retarget cluster is what needs them.
GROUPS = "core,battle,bhit,enemy,chars,rng,vehicle,bcmd"


@dataclasses.dataclass
class Run:
    log: pathlib.Path
    trace: pathlib.Path
    stderr: str
    status: int


def run_oracle(tape: pathlib.Path, out_dir: pathlib.Path, stem: str,
               patches: list[str], groups: str = GROUPS) -> Run:
    out_dir.mkdir(parents=True, exist_ok=True)
    log = out_dir / f"{stem}.csv"
    trace = out_dir / f"{stem}_rolls.csv"
    argv = [str(BIN), "--core", str(CORE), "--rom", str(ROM),
            "--map", str(DEFAULT_RAM_MAP_TSV), "--tape", str(tape),
            "--groups", groups, "--rng-trace", str(trace), "--out", str(log)]
    for spec in patches:
        argv += ["--ram-patch", spec]
    proc = subprocess.run(argv, capture_output=True, text=True)
    return Run(log, trace, proc.stderr, proc.returncode)


def sha256(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read_rows(path: pathlib.Path) -> list[dict]:
    with open(path) as handle:
        return list(csv.DictReader(line for line in handle
                                   if not line.startswith("#")))


def by_frame(rows: list[dict]) -> dict[int, dict]:
    return {int(row["frame"]): row for row in rows}


def battle_window(rows: list[dict]) -> tuple[int, int] | None:
    """The first **contiguous** run of `Game_Mode_Index` $10/$14 frames.

    A run that holds one battle - every capture and every tape here - reads the
    same as "the first and last frame in battle mode" (what `oracle/verify.sh`
    checks). A *long* run can hold two: the preview tape's policy outlives a
    short fight, the party walks on and a second encounter fires, and the two
    battles are separated by field frames but share the mode. Taking the first
    and last frames of the mode would then read the *second* battle's slots as
    the first's (formation `$3C`'s sweep capture, 2026-09-24), so the window
    stops where the mode does.
    """
    frames = [int(r["frame"]) for r in rows
              if r["game_mode"] in ("0010", "0014")]
    if not frames:
        return None
    end = frames[0]
    for frame in frames[1:]:
        if frame != end + 1:
            break
        end = frame
    return (frames[0], end)


def seed_of(row: dict) -> int:
    return int(row["rng_seed"], 16)


def hp_of(row: dict, column: str) -> int:
    """One fighter's HP as the cartridge stores it: a signed 16-bit word.

    The log renders the columns unsigned, so a dead fighter reads 65511, not
    -25; `oracle.fixture` applies the same rule (`log.signed`)."""
    value = int(row[column])
    return value - 0x10000 if value > 0x7FFF else value
