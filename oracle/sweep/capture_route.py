"""Capture random-encounter damage abilities, one host at a time.

The case file and every capture are local evidence, not committed cartridge
records. A successful force command is still rejected if the extracted rounds
do not contain the requested ability or include an out-of-lane ability.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import subprocess
import sys
import time

from .route_abilities import ROOT, classes


def observed_rounds(fixture: dict, ability: int) -> list[int]:
    return sorted({r["round"] for r in fixture["rounds"]
                   if any(a.get("ability") == ability and a["kind"] == "ability"
                          and any(t.get("damage") is not None for t in a["targets"])
                          for a in r["actions"])})


def out_of_lane(fixture: dict, allowed: set[int]) -> list[int]:
    return sorted({a["ability"] for r in fixture["rounds"] for a in r["actions"]
                   if a.get("ability", 0) and a["ability"] not in allowed})


def run(command: list[str], log: pathlib.Path) -> dict:
    started = time.time()
    print("RUN", " ".join(command), flush=True)
    with log.open("w") as handle:
        result = subprocess.run(command, stdout=handle, stderr=subprocess.STDOUT)
    receipt = {"command": command, "exit": result.returncode,
               "elapsed_seconds": round(time.time() - started, 3),
               "log": str(log), "log_sha256": hashlib.sha256(log.read_bytes()).hexdigest()}
    print("EXIT", result.returncode, str(log), flush=True)
    return receipt


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--cases", type=pathlib.Path, required=True)
    parser.add_argument("--out", type=pathlib.Path, required=True)
    parser.add_argument("--scout", type=pathlib.Path, required=True)
    parser.add_argument("--fixtures-dir", type=pathlib.Path,
                        default=ROOT / "build/oracle/replay_fixtures")
    parser.add_argument("--max-delay", type=int, default=24)
    args = parser.parse_args(argv)
    if args.max_delay < 0:
        parser.error("max-delay must be nonnegative")
    args.out.mkdir(parents=True, exist_ok=True)
    fixtures = args.fixtures_dir
    fixtures.mkdir(parents=True, exist_ok=True)
    allowed = {a for a, kind in classes((ROOT / "docs/battle/ENEMY_ABILITIES.md").read_text()).items()
               if kind in {"damage", "no effect"}}
    # The inventory's wording for the FloatMine fall-through is "no-effect".
    allowed.update({7, 0x17})
    cases = json.loads(args.cases.read_text())
    receipts = []
    for case in cases:
        found = False
        for delay in range(case.get("first_delay", 0), args.max_delay + 1):
            capture = args.out / "captures" / f"{case['name']}-d{delay}"
            capture.mkdir(parents=True, exist_ok=True)
            command = [sys.executable, "-m", "oracle.force", "--formation", str(case["formation"]),
                       "--out", str(capture), "--max-rounds", str(case.get("rounds", 1)),
                       "--require-ability", str(case["ability"]), "--delay", str(delay),
                       "--durable", "--scout", str(args.scout)]
            if (capture / "report.json").is_file():
                report = json.loads((capture / "report.json").read_text())
                force = {"command": command, "exit": 0 if report["require_ability_met"] else 1,
                         "reused_report": str(capture / "report.json")}
            else:
                force = run(command, capture / "force.log")
            receipt = {"name": case["name"], "ability": case["ability"], "delay": delay,
                       "formation": case["formation"], "force": force}
            receipts.append(receipt)
            if force["exit"] != 0:
                receipt["result"] = "required ability absent or force failed"
            else:
                report = json.loads((capture / "report.json").read_text())
                temporary = capture / "fixture.json"
                command = [sys.executable, "-m", "oracle.fixture", "--trace", report["trace"],
                           "--log", report["log"], "--out", str(temporary), "--tape", report["tape"],
                           "--battle-first", str(report["battle_first"]),
                           "--battle-last", str(report["battle_last"]),
                           "--max-rounds", str(case.get("rounds", 1)),
                           "--hp-patch", "999", "--minified"]
                receipt["extract"] = run(command, capture / "extract.log")
                if receipt["extract"]["exit"] == 0:
                    fixture = json.loads(temporary.read_text())
                    observed = observed_rounds(fixture, case["ability"])
                    excluded = out_of_lane(fixture, allowed)
                    receipt.update(observed_rounds=observed, out_of_lane=excluded,
                                   report=str(capture / "report.json"))
                    if observed and not excluded:
                        output = fixtures / f"{case['name']}.json"
                        output.write_bytes(temporary.read_bytes())
                        receipt.update(result="captured", fixture=str(output),
                                       fixture_sha256=hashlib.sha256(output.read_bytes()).hexdigest())
                        found = True
                    else:
                        receipt["result"] = "extracted rounds absent or include out-of-lane ability"
                else:
                    receipt["result"] = "extraction failed"
            (args.out / "capture-receipts.json").write_text(json.dumps(receipts, indent=2) + "\n")
            if force["exit"] not in (0, 1):
                print("FORCE FAILED", case["name"], force["exit"], flush=True)
                return 1
            if found:
                print("CAPTURED", case["name"], receipt["observed_rounds"], flush=True)
                break
        if not found:
            print("INCOMPLETE", case["name"], flush=True)
            return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
