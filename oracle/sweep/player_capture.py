"""Run explicit player-ability cases through the stock cartridge and extractor.

Recipes and RAM images stay ignored. Commands use oracle.force's live menu
driver; replay state comes from two identical stock-host RAM observations,
never from this recipe. A selected command is not proof that it executed.
"""
from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import pathlib
import subprocess
import sys
import time

from oracle.force.script import party_state

ROOT = pathlib.Path(__file__).resolve().parents[2]
# Character_Stats fields, ps4.constants.asm:34-73. These are RAM layout,
# not ability records; all ability ids come from the case's explicit commands.
WORDS = {"profession": 6, "level": 8, "hp": 14, "max_hp": 16,
         "tp": 18, "max_tp": 20, "attack": 36, "defence": 40,
         "mental_defence": 44}
BYTES = {"status": 22, "strength": 24, "mental": 27,
         "agility": 30, "dexterity": 33}


def patches(case: dict, raw: bytes, frame: int) -> list[str]:
    result = []
    if "characters" in case:
        characters = case["characters"]
        if (len(characters) != 3 or len(set(characters)) != 3
                or any(type(c) is not int or not 0 <= c <= 10 for c in characters)):
            raise ValueError("capture needs three distinct character ids 0..10")
        state = bytearray(raw)
        state[0xF40A:0xF40F] = bytes(characters + [255, 255])
        raw = bytes(state)
        result.append(f"{frame}:FFFFF40A:{raw[0xF40A:0xF40F].hex()}")
    party = party_state(raw)
    needed = {slot: {"technique": [], "skill": []} for slot in party}
    for commands in case["rounds"]:
        if set(map(int, commands)) != set(party):
            raise ValueError("case commands must name every occupied party slot")
        for slot, command in commands.items():
            kind = command["command"]
            if kind in ("technique", "skill"):
                ids = needed[int(slot)][kind]
                if command["id"] not in ids:
                    ids.append(command["id"])
    for slot, member in party.items():
        start = 0xF500 + member["character"] * 0x80
        record = bytearray(raw[start:start + 0x80])
        settings = dict(case.get("defaults", {}))
        settings.update(case.get("party", {}).get(str(slot), {}))
        for key, value in settings.items():
            if key not in WORDS and key not in BYTES:
                raise ValueError(f"unknown party field {key}")
            width, offset = (2, WORDS[key]) if key in WORDS else (1, BYTES[key])
            record[offset:offset + width] = value.to_bytes(width, "big")
            if key in ("strength", "mental", "agility", "dexterity"):
                # Each byte stat has base, modified and battle copies. Keep
                # explicit synthetic stat settings together; FillBattleStats
                # copies modified into battle (ps4.asm:11272-11290).
                record[offset:offset + 3] = bytes((value, value, value))
        for kind, offset, count in (("technique", 0x52, 16), ("skill", 0x62, 8)):
            ids = needed[slot][kind]
            if len(ids) > count:
                raise ValueError(f"fighter {slot} needs too many {kind}s: {ids}")
            record[offset:offset + count] = bytes(ids + [0] * (count - len(ids)))
        for index in range(8):
            # Generous explicit resources permit a bounded sequence. The
            # observed start record and round state prove actual payment.
            record[0x6A + 2 * index:0x6C + 2 * index] = bytes((9, 9))
        # The stock host bounds each patch to 64 bytes.
        for offset in (0, 64):
            result.append(f"{frame}:FFFF{start + offset:04X}:{record[offset:offset + 64].hex()}")
    return result


def executed(fixture: dict) -> set[tuple[str, int]]:
    return {(a["kind"], a["ability"]) for r in fixture["rounds"]
            for a in r["actions"] if a["actor"] <= 5
            and a.get("kind") in ("technique", "skill")}


def require_executed(case: dict, fixture: dict) -> None:
    wanted = {(c["command"], c["id"]) for r in case["rounds"] for c in r.values()
              if c["command"] in ("technique", "skill")}
    missing = wanted - executed(fixture)
    if missing:
        raise ValueError(f"selected abilities did not execute: {sorted(missing)}")


def run(command: list[str], output: pathlib.Path, receipts: pathlib.Path) -> None:
    start = time.monotonic()
    with output.open("w") as log:
        result = subprocess.run(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT)
    receipt = {"command": command, "exit": result.returncode,
               "seconds": round(time.monotonic() - start, 3),
               "utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
               "log": str(output), "log_sha256": hashlib.sha256(output.read_bytes()).hexdigest()}
    with receipts.open("a") as handle:
        handle.write(json.dumps(receipt) + "\n")
    print(json.dumps(receipt), flush=True)
    if result.returncode:
        raise ValueError(f"command exited {result.returncode}; see {output}")


def verify_report(report: dict) -> None:
    """A resumed extraction must still have both independent stock runs."""
    for key in ("trace", "log", "rerun_log", "start_ram", "party_script"):
        path = pathlib.Path(report[key])
        if hashlib.sha256(path.read_bytes()).hexdigest() != report[key + "_sha256"]:
            raise ValueError(f"capture {key} hash changed: {path}")
    if report["log_sha256"] != report["rerun_log_sha256"]:
        raise ValueError("stock capture and verification logs differ")


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--cases", type=pathlib.Path, required=True)
    parser.add_argument("--scout", type=pathlib.Path, required=True)
    parser.add_argument("--out", type=pathlib.Path, required=True)
    parser.add_argument("--fixtures", type=pathlib.Path, required=True)
    parser.add_argument("--only", help="one case name, otherwise every case")
    parser.add_argument("--extract-only", action="store_true",
                        help="reuse a completed, hash-verified stock capture")
    args = parser.parse_args(argv)
    try:
        facts = json.loads(args.scout.read_text())
        raw = pathlib.Path(facts["script_state"]["path"]).read_bytes()
        if hashlib.sha256(raw).hexdigest() != facts["script_state"]["sha256"]:
            raise ValueError("scout RAM digest changed")
        cases = json.loads(args.cases.read_text())
        names = [c["name"] for c in cases]
        if len(names) != len(set(names)) or any(pathlib.Path(n).name != n for n in names):
            raise ValueError("case names must be distinct basenames")
        if args.only and args.only not in names:
            raise ValueError(f"no case named {args.only}")
        args.out.mkdir(parents=True, exist_ok=True)
        args.fixtures.mkdir(parents=True, exist_ok=True)
        receipts = args.out / "commands.jsonl"
        for case in cases:
            if args.only and case["name"] != args.only:
                continue
            out = args.out / case["name"]
            out.mkdir(parents=True, exist_ok=True)
            script = out / "commands.json"
            script_text = json.dumps({"rounds": case["rounds"]}, indent=2) + "\n"
            if args.extract_only:
                if script.read_text() != script_text:
                    raise ValueError(f"case commands changed since capture: {case['name']}")
            else:
                script.write_text(script_text)
            command = [sys.executable, "-m", "oracle.force", "--scout", str(args.scout),
                       "--out", str(out), "--party-script", str(script),
                       "--max-rounds", str(len(case["rounds"])), "--repeats", "900"]
            selector = "--event" if "event" in case else "--formation"
            command += [selector, str(case[selector[2:]])]
            for patch in patches(case, raw, facts["battle_first"] + 1):
                command += ["--ram-patch", patch]
            if not args.extract_only:
                run(command, out / "force.log", receipts)
            report = json.loads((out / "report.json").read_text())
            verify_report(report)
            target = args.fixtures / f"{case['name']}.json"
            extract = [sys.executable, "-m", "oracle.fixture", "--trace", report["trace"],
                       "--log", report["log"], "--ram-map", report["script_ram_map"],
                       "--tape", report["tape"], "--battle-first", str(report["battle_first"]),
                       "--battle-last", str(report["battle_last"]),
                       "--max-rounds", str(len(case["rounds"])), "--start-ram", report["start_ram"],
                       "--start-ram-sha256", report["start_ram_sha256"],
                       "--out", str(target), "--minified"]
            run(extract, out / "extract.log", receipts)
            require_executed(case, json.loads(target.read_text()))
        return 0
    except (OSError, ValueError, KeyError) as error:
        print(f"player_capture: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
