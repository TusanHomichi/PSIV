#!/usr/bin/env python3
"""tools/verify_native_flash_attack.py - the flash-only enemy attack, natively.

    python3 tools/verify_native_flash_attack.py [--no-build] [--case NAME=FORMATION ...]

`EnemySprite::begin_attack` (rust/psiv-godot/src/battle/attack.rs) tints the
body instead of drawing a sheet layer when an enemy's attack art is deferred:
the flash-only branch, which once leaked one unparented Sprite2D per attack
(#92). Since #214 exactly the enemies `oracle/fixtures/battle_animation_remainder.json`
names take it (tests/test_flash_only_attack_census.py pins that set). This
drives the branch in Godot. Each case starts a real battle on a formation
through the battle-window fixture (`PSIV_DEBUG_BATTLE_WINDOW=top`,
`PSIV_DEBUG_BATTLE_FORMATION`; rust/psiv-runtime/src/session/battle/fixture.rs),
whose tape 07 party of three survives the rounds where the title-bypass
party, Chaz alone at 25 HP, falls to the second Locusta hit. Then
`tools/native/native_flash_attack.gd` DEFENDs through COMD every round, so the
enemies' own turns play their attacks, until `ATTACKS` flash attacks have run
and every body is back at rest. A case passes only when (`verdict`):

- the driver finished and Godot's orphan-node monitor counted a test orphan;
- at least `ATTACKS` attacks took the flash branch and none drew a sheet layer;
- every attacker is an enemy the receipts name;
- each flash attack tinted the body and added no child to it;
- no orphan node existed at the battle's start, on any tick of it, or after
  the attacks' cleanup, and every body ends untinted with its idle children.

The cases are two receipt routine bodies, two enemies each, so each body
attacks more than once: Locusta (formation `$12`, routine `0x010C7C`) and Sand
Newt (`$13`, routine `0x00FCCA`). `--case NAME=HEX` runs another formation
instead (a negative control: a sheet enemy's formation, such as tape 07's
Zoran Bults at `$8A`, fails).

The extension is rebuilt first (extension class) unless `--no-build`; the runs
hold one godot-class slot of the shared heavy lock (tools/heavy_lock.py)
themselves, so no wrapper is needed. Godot runs headless from a run root in
the receipt directory (tools/godot.py `create_run_root`), each case with its
own empty `PSIV_SAVE_DIR`. Receipts land in `build/flash-attack/<UTC>-<sha>/`:
each case's report, logs and verdict in `receipt.json`. Exit 0 only when every
case passes, 2 when refused before running.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

try:  # imported as `tools.verify_native_flash_attack` by tests
    from tools.gate import display_path, git_info, utc_stamp
    from tools.godot import GodotError, create_run_root, godot_command, require as require_godot
    from tools.heavy_lock import EXTENSION, GODOT as GODOT_CLASS, hold_heavy_lock
except ModuleNotFoundError:  # direct `python3 tools/verify_native_flash_attack.py`
    from gate import display_path, git_info, utc_stamp
    from godot import GodotError, create_run_root, godot_command, require as require_godot
    from heavy_lock import EXTENSION, GODOT as GODOT_CLASS, hold_heavy_lock

ROOT = Path(__file__).resolve().parents[1]
DRIVER = ROOT / "tools/native/native_flash_attack.gd"
RECEIPTS = ROOT / "oracle/fixtures/battle_animation_remainder.json"

#: (name, formation): one formation per shared routine body.
CASES = [("locusta", 0x012), ("sand-newt", 0x013)]

#: Flash attacks each case plays before its cleanup is measured.
ATTACKS = 3

#: An untinted body: Godot's identity `self_modulate`, as 8-bit RGBA.
WHITE = [255, 255, 255, 255]

#: Seconds one case may take: a few hundred ticks headless, at load 100.
CASE_TIMEOUT_S = 300

GODOT_FLAGS = ["--headless", "--disable-render-loop", "--audio-driver", "Dummy",
               "--fixed-fps", "60"]


def receipt_enemy_ids(path: Path = RECEIPTS) -> set:
    """Every enemy id a receipt body names: the records whose attack art #214 defers."""
    return {enemy_id for receipt in json.loads(path.read_text())["receipts"]
            for enemy_id in receipt["enemy_ids"]}


def verdict(report: dict, expected: set, attacks: int = ATTACKS) -> list:
    """Why the driver's `report` fails the check (empty when it passes)."""
    failures = []
    if not report.get("complete"):
        failures.append(f"incomplete: {report.get('reason') or 'no reason given'}")
    if report.get("orphan_monitor_alive") is not True:
        failures.append("the orphan-node monitor did not count a test orphan")
    idle = {enemy["fighter"]: enemy for enemy in report.get("idle", [])}
    started = report.get("attacks", [])
    flashes = [item for item in started if item["attack"] == "flash"]
    if len(flashes) < attacks:
        failures.append(f"{len(flashes)} flash attacks, not {attacks}")
    for item in started:
        where = f"tick {item['tick']} fighter {item['fighter']} enemy {item['enemy_id']}"
        if item["attack"] != "flash":
            failures.append(f"{where}: drew a {item['attack']} attack, not the flash branch")
        if item["enemy_id"] not in expected:
            failures.append(f"{where}: enemy {item['enemy_id']} is not a receipt enemy")
        if item["attack"] == "flash" and item["tint"] == WHITE:
            failures.append(f"{where}: the flash attack left the body untinted")
        if item["children"] != idle.get(item["fighter"], {}).get("children"):
            failures.append(f"{where}: {item['children']} children, idle "
                            f"{idle.get(item['fighter'], {}).get('children')}")
    for key in ("orphans_battle_start", "orphans_max", "orphans_end"):
        if report.get(key) != 0 and (report.get("complete") or key != "orphans_end"):
            failures.append(f"{key} = {report.get(key)}, not 0")
    for enemy in report.get("final") or []:
        where = f"after cleanup fighter {enemy['fighter']}"
        if enemy["attack"] is not None:
            failures.append(f"{where}: still in a {enemy['attack']} attack")
        if enemy["tint"] != WHITE:
            failures.append(f"{where}: tint {enemy['tint']}, not {WHITE}")
        if enemy["children"] != idle.get(enemy["fighter"], {}).get("children"):
            failures.append(f"{where}: {enemy['children']} children, idle "
                            f"{idle.get(enemy['fighter'], {}).get('children')}")
    if report.get("complete") and not report.get("final"):
        failures.append("no enemy body observed after cleanup")
    return failures


def case_env(formation: int, saves: Path, report: Path, pack: Path) -> dict:
    """The case's environment: the caller's, without any other debug selector."""
    env = {key: value for key, value in os.environ.items()
           if not key.startswith("PSIV_DEBUG_") and key not in ("PSIV_LOAD_SLOT", "PSIV_RUNTIME_PACK")}
    env.update({"PSIV_DEBUG_BATTLE_WINDOW": "top", "PSIV_DEBUG_BATTLE_FORMATION": f"{formation:x}",
                "PSIV_DEBUG_ROUTE": "1",
                "PSIV_SAVE_DIR": str(saves), "PSIV_FLASH_ATTACKS": str(ATTACKS),
                "PSIV_FLASH_REPORT": str(report), "PSIV_RUNTIME_PACK": str(pack)})
    return env


def run_case(name: str, formation: int, run_root, receipt: Path) -> tuple[dict | None, int, float]:
    saves = receipt / f"{name}-saves"
    saves.mkdir(parents=True)
    report = receipt / f"{name}-report.json"
    command = godot_command(run_root, *GODOT_FLAGS, "--log-file", str(receipt / f"{name}-godot.log"),
                            "--script", str(DRIVER), prefix=("timeout", f"{CASE_TIMEOUT_S}s"))
    started = time.monotonic()
    with open(receipt / f"{name}-run.log", "w") as handle:
        code = subprocess.run(command, cwd=ROOT, env=case_env(formation, saves, report, run_root.pack),
                              stdout=handle, stderr=subprocess.STDOUT).returncode
    seconds = round(time.monotonic() - started, 1)
    loaded = json.loads(report.read_text()) if report.is_file() else None
    return loaded, code, seconds


def parse_case(text: str) -> tuple[str, int]:
    name, _, formation = text.partition("=")
    if not name or not formation:
        raise argparse.ArgumentTypeError(f"{text!r}: expected NAME=FORMATION (hex)")
    return name, int(formation.removeprefix("0x"), 16)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--no-build", action="store_true",
                        help="skip `cargo build -p psiv-godot` (only when this tree was just built)")
    parser.add_argument("--case", type=parse_case, action="append", metavar="NAME=FORMATION",
                        help="run this formation (hex) instead of the default cases")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    os.chdir(ROOT)
    try:
        require_godot()
    except GodotError as error:
        print(f"flash attack: refused, {error}")
        return 2
    if not args.no_build:
        with hold_heavy_lock(lock_class=EXTENSION):
            built = subprocess.run(["cargo", "build", "-p", "psiv-godot", "--manifest-path",
                                    "rust/Cargo.toml"])
        if built.returncode != 0:
            return built.returncode
    expected = receipt_enemy_ids()
    info = git_info(ROOT)
    receipt = ROOT / "build" / "flash-attack" / \
        f"{utc_stamp(datetime.now(timezone.utc))}-{info['short_sha']}"
    receipt.mkdir(parents=True)
    results = []
    with hold_heavy_lock(lock_class=GODOT_CLASS, slots=1):
        try:
            run_root = create_run_root(receipt / "run-root", source=ROOT)
        except GodotError as error:
            print(f"flash attack: refused, {error}")
            return 2
        try:
            for name, formation in args.case or CASES:
                report, code, seconds = run_case(name, formation, run_root, receipt)
                failures = (verdict(report, expected) if report is not None
                            else [f"no report (godot exit {code})"])
                if code != 0:
                    failures.append(f"godot exited {code}")
                flashes = sum(item["attack"] == "flash" for item in (report or {}).get("attacks", []))
                results.append({"case": name, "formation": f"{formation:#05x}", "godot_exit": code,
                                "seconds": seconds, "flash_attacks": flashes,
                                "failures": failures, "pass": not failures})
                print(f"{name:10} {'PASS' if not failures else 'FAIL'} formation {formation:#05x} "
                      f"{flashes} flash attacks, {seconds} s"
                      + "".join(f"\n  {failure}" for failure in failures), flush=True)
        finally:
            run_root.release()
    (receipt / "receipt.json").write_text(json.dumps(
        {**info, "attacks": ATTACKS, "run_root": run_root.record, "cases": results}, indent=2) + "\n")
    passed = sum(result["pass"] for result in results)
    print(f"flash attack: {passed}/{len(results)} cases pass; "
          f"receipt={display_path(receipt, ROOT)}")
    return 0 if passed == len(results) else 1


if __name__ == "__main__":
    sys.exit(main())
