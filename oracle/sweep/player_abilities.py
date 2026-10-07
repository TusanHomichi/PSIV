"""Derive the player inventory and route learn set; refuse missing ledger rows.

The route report carries chapter parties and observed levels. Engine gates are
read from a fresh psiv-data -> core inventory test, not a parallel Python rule.
Names, records and progression are read-only ignored inputs. Capture status is
derived from party actions that actually ran, never from a selected command.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import re
import subprocess
import sys
import tempfile

from oracle.sweep.player_chains import Chains

ROOT = pathlib.Path(__file__).resolve().parents[2]
LEDGER = ROOT / "docs/battle/PLAYER_ABILITIES.md"
FIXTURES = ROOT / "rust/psiv-core/src/battle/replay_fixtures"
DEFAULT_REPORT = ROOT / "build/p88-route/route-levels.json"


def learn_tables(characters: list[dict], progression: dict) -> dict:
    """Each known id's earliest level per character, including starting lists."""
    learned = {"technique": {}, "skill": {}}
    for character in characters:
        for kind in learned:
            for ability in character[f"initial_{kind}s"]:
                learned[kind].setdefault(ability["id"], {})[character["id"]] = {
                    "name": character["name"], "level": character["level"]}
    for character in progression["characters"]:
        for row in character["levels"]:
            for kind in learned:
                ability = row[f"new_{kind}"]
                if not ability:
                    continue
                learners = learned[kind].setdefault(ability["id"], {})
                previous = learners.get(character["character_id"])
                if previous is None or row["level"] < previous["level"]:
                    learners[character["character_id"]] = {
                        "name": character["character"], "level": row["level"]}
    return learned


def route_levels(report: dict) -> dict[int, int]:
    if report.get("result") != "completed" or not report.get("chapters"):
        raise ValueError("route report needs a completed run and chapter parties")
    levels = {}
    for chapter in report["chapters"]:
        for member in chapter["party"]:
            id_, level = member["character_id"], member["level"]
            if (isinstance(id_, bool) or not isinstance(id_, int)
                    or not 0 <= id_ <= 10 or isinstance(level, bool)
                    or not isinstance(level, int) or not 1 <= level <= 99):
                raise ValueError(f"invalid route member: {member}")
            levels[id_] = max(levels.get(id_, 0), level)
    return levels


def include_saved_learning(learned: dict, report: dict, characters: list[dict],
                           records: dict) -> None:
    """Retain story/upgrade grants observed through the retail save codec.

    Their observed checkpoint level is not a progression-table learn level.
    Earlier decoded progression evidence remains authoritative when present.
    """
    names = {c["id"]: c["name"] for c in characters}
    for chapter in report["chapters"]:
        for member in chapter["party"]:
            char, level = member["character_id"], member["level"]
            for kind in learned:
                universe = {r["id"] for r in records[f"{kind}s"]}
                for ability in member.get(f"{kind}s", []):
                    if type(ability) is not int or ability not in universe:
                        raise ValueError(f"unknown saved {kind} {ability} for character {char}")
                    learners = learned[kind].setdefault(ability, {})
                    old = learners.get(char)
                    if old is None or (old.get("source") == "route snapshot" and level < old["level"]):
                        learners[char] = {"name": names[char], "level": level,
                                          "source": "route snapshot"}


def ledger_rows(text: str) -> dict[tuple[str, int], list[str]]:
    rows = {}
    for line in text.splitlines():
        if not line.startswith("| technique |") and not line.startswith("| skill |"):
            continue
        cells = [cell.strip() for cell in line.split("|")[1:-1]]
        if len(cells) != 10:
            raise ValueError(f"malformed player row: {line}")
        key = (cells[0], int(cells[1]))
        if key in rows:
            raise ValueError(f"duplicate ledger ability {key}")
        rows[key] = cells
    return rows


def captures(directory: pathlib.Path) -> dict:
    found = {}
    for path in sorted(directory.rglob("*.json")):
        document = json.loads(path.read_text())
        if "formation" not in document:
            continue
        for round_ in document["rounds"]:
            for action in round_["actions"]:
                kind, id_ = action.get("kind"), action.get("ability")
                if action["actor"] > 5 or kind not in ("technique", "skill") or not id_:
                    continue
                found.setdefault((kind, id_), []).append({
                    "fixture": path.relative_to(directory).as_posix(),
                    "round": round_["round"], "frame": action["start_frame"]})
    return found


def inventory(records: dict, learned: dict, levels: dict, rows: dict,
              engine: list[dict], observed: dict) -> list[dict]:
    gates = {(r["kind"], r["id"]): r for r in engine}
    universe = {(kind, r["id"]) for kind in ("technique", "skill")
                for r in records[f"{kind}s"]}
    if len(gates) != len(engine) or set(gates) != universe:
        raise ValueError("engine inventory is not the decoded ability universe")
    result = []
    for kind in ("technique", "skill"):
        for record in records[f"{kind}s"]:
            key = (kind, record["id"])
            name = record["display_name"]
            if key not in rows:
                raise ValueError(f"ledger lacks {kind} {record['id']} {name}")
            row = rows[key]
            if row[2] != name or "ps4.asm:" not in row[3] or "ps4.asm:" not in row[4]:
                raise ValueError(f"ledger name or cited chain invalid for {kind} {record['id']} {name}")
            learners = learned[kind].get(record["id"], {})
            route = any(levels.get(c, 0) >= entry["level"] for c, entry in learners.items())
            battle = bool(record["targeting"]["raw"] & 0x10)
            status = "implemented" if gates[key]["battle_supported"] else "unsupported"
            # Field-only travel has no battle dispatcher; the row retains that
            # distinction instead of claiming a successful battle capture.
            if not battle:
                status = "implemented (field-only)"
            result.append(dict(kind=kind, id=record["id"], name=name, route=route,
                               battle=battle, engine=status,
                               capture=observed.get(key, []), learners=learners,
                               cells=row))
    extra = set(rows) - universe
    if extra:
        raise ValueError(f"ledger has unknown abilities: {sorted(extra)}")
    return result


def fresh_engine(pack: pathlib.Path, output: pathlib.Path) -> list[dict]:
    import os
    output.parent.mkdir(parents=True, exist_ok=True)
    command = ["cargo", "test", "--manifest-path", str(ROOT / "rust/Cargo.toml"),
               "-p", "psiv-runtime", "--lib", "player_ability_dispatch_inventory",
               "--", "--test-threads=1"]
    # A renamed/missing test can make Cargo succeed with zero selected tests.
    # Never let an older output masquerade as this invocation's inventory.
    with tempfile.TemporaryDirectory(prefix="player-inventory-", dir=output.parent) as directory:
        fresh = pathlib.Path(directory) / "inventory.json"
        env = dict(os.environ, CARGO_BUILD_JOBS="2", PSIV_PLAYER_PACK=str(pack.resolve()),
                   PSIV_PLAYER_INVENTORY=str(fresh.resolve()))
        run = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True)
        output.with_suffix(".log").write_text(run.stdout + run.stderr)
        if run.returncode:
            raise ValueError(f"engine inventory exited {run.returncode}; {output.with_suffix('.log')}")
        if not fresh.is_file():
            raise ValueError("engine inventory test did not emit fresh output")
        text = fresh.read_text()
        rows = json.loads(text)
        output.write_text(text)
        return rows


def update(text: str, abilities: list[dict]) -> str:
    indexed = {(r["kind"], r["id"]): r for r in abilities}
    output = []
    for line in text.splitlines():
        if line.startswith("| technique |") or line.startswith("| skill |"):
            cells = [cell.strip() for cell in line.split("|")[1:-1]]
            row = indexed[(cells[0], int(cells[1]))]
            cells[7] = row["engine"]
            cells[8] = "; ".join(f"`{c['fixture']}` r{c['round']}" for c in row["capture"]) or "not captured"
            learners = "; ".join(
                f"{v['name']} {'observed ' if v.get('source') else ''}L{v['level']}"
                for _, v in sorted(row["learners"].items()))
            cells[9] = ("route; " if row["route"] else "outside route; ") + (learners or "no decoded level learner")
            line = "| " + " | ".join(cells) + " |"
        output.append(line)
    return "\n".join(output) + "\n"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--generated", type=pathlib.Path, default=ROOT / "generated")
    parser.add_argument("--runtime-pack", type=pathlib.Path, default=ROOT / "runtime-pack")
    parser.add_argument("--ledger", type=pathlib.Path, default=LEDGER)
    parser.add_argument("--asm", type=pathlib.Path, default=ROOT / "reference/ps4disasm/ps4.asm")
    parser.add_argument("--route-report", type=pathlib.Path, default=DEFAULT_REPORT)
    parser.add_argument("--fixtures", type=pathlib.Path, default=FIXTURES)
    parser.add_argument("--out", type=pathlib.Path, default=ROOT / "build/p88-evidence/player-abilities.json")
    parser.add_argument("--update-doc", action="store_true")
    parser.add_argument("--require-captures", action="store_true")
    args = parser.parse_args(argv)
    try:
        read = lambda p: json.loads(p.read_text())
        rows = ledger_rows(args.ledger.read_text())
        records = read(args.runtime_pack / "battle/abilities.json")
        # Validate the ledger before starting any Rust process.
        for kind in ("technique", "skill"):
            for record in records[f"{kind}s"]:
                if (kind, record["id"]) not in rows:
                    raise ValueError(f"ledger lacks {kind} {record['id']} {record['display_name']}")
        characters = read(args.generated / "characters.json")
        learned = learn_tables(characters, read(args.generated / "progression.json"))
        report = read(args.route_report)
        levels = route_levels(report)
        include_saved_learning(learned, report, characters, records)
        chains = Chains(args.asm)
        for kind in ("technique", "skill"):
            for record in records[f"{kind}s"]:
                key = (kind, record["id"])
                derived = chains.row(kind, record)
                if args.update_doc:
                    rows[key][:7] = derived[:7]
                elif rows[key][:7] != derived[:7]:
                    raise ValueError(f"stale source chain for {kind} {record['id']} "
                                     f"{record['display_name']}; run --update-doc")
        engine = fresh_engine(args.runtime_pack, args.out.with_name("dispatch-inventory.json"))
        abilities = inventory(records, learned, levels, rows, engine, captures(args.fixtures))
        args.out.parent.mkdir(parents=True, exist_ok=True)
        inputs = [args.ledger, args.asm, args.route_report, args.generated / "characters.json",
                  args.generated / "progression.json", args.runtime_pack / "battle/abilities.json"]
        inputs += [pathlib.Path(chapter["save"]) for chapter in report["chapters"]
                   if "save" in chapter]
        base = args.ledger.read_text()
        if args.update_doc:
            base = "\n".join(
                "| " + " | ".join(rows[(cells[0], int(cells[1]))]) + " |"
                if line.startswith(("| technique |", "| skill |")) else line
                for line in base.splitlines()
                for cells in [[c.strip() for c in line.split("|")[1:-1]]]) + "\n"
        rendered = update(base, abilities)
        if args.update_doc:
            args.ledger.write_text(rendered)
        elif rendered != args.ledger.read_text():
            raise ValueError("generated player ledger is stale; run --update-doc")
        document = {"abilities": [{k: v for k, v in r.items() if k != "cells"} for r in abilities],
                    "inputs": {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in inputs}}
        args.out.write_text(json.dumps(document, indent=2) + "\n")
        route = [r for r in abilities if r["route"] and r["battle"]]
        bad = [r for r in route if r["engine"] != "implemented"]
        missing = [r for r in route if not r["capture"]]
        print(f"{len(abilities)} abilities; {len(route)} battle route abilities; "
              f"{len(bad)} unsupported/partial; {len(missing)} without capture")
        for row in bad:
            print(f"{row['kind']} {row['id']} {row['name']}: {row['engine']}")
        if args.require_captures:
            for row in missing:
                print(f"missing capture: {row['kind']} {row['id']} {row['name']}")
        return int(bool(bad or (args.require_captures and missing)))
    except (OSError, ValueError, KeyError) as error:
        print(f"player_abilities: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
