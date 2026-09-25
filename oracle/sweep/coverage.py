"""What the sweep covered, as the ledger's coverage section.

`oracle/sweep.py`'s record is per-formation evidence; this reads it and prints
the census `docs/BATTLE_ORACLE_SWEEP.md` carries: which formations were captured
(and which failed, why), which enemies they seated, which enemy abilities they
were seen to run, and how each of those abilities stands in
`docs/ENEMY_ABILITIES.md` - implemented, or unsupported and of which class.

    python3 -m oracle.sweep.coverage --record build/lane-evidence/sweep/sweep_motavia.json

The ability status is read out of `docs/ENEMY_ABILITIES.md`'s own table rather
than restated here: the ledger's job is to say whether the sweep exercised a
finished ability or an unfinished one, and that document is where "finished"
is written down.
"""
from __future__ import annotations

import argparse
import collections
import json
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
RECORD = ROOT / "build" / "lane-evidence" / "sweep" / "sweep_motavia.json"
ABILITIES = ROOT / "docs" / "ENEMY_ABILITIES.md"
ENEMIES = ROOT / "generated" / "enemies.json"

#: One row of `docs/ENEMY_ABILITIES.md`'s regular-ability table: the id in its
#: first cell, and the two status cells at the end.
ROW = re.compile(r"^\| `\$([0-9A-F]{2})` \((\d+)\) \*\*(\w+)\*\*.*"
                 r"\| ([^|]*)\| ([^|]*)\|$")


def ability_status(path: pathlib.Path) -> dict[int, dict]:
    """{ability id: {name, class, status}} from the inventory's *own* table.

    Only section 2's regular-ability table: the sections after it re-list ids
    in other shapes (carrier families, gated routines), and the 83 ids this
    sweep can meet are the regular table's.
    """
    out = {}
    body = path.read_text()
    start = body.index("## 2.")
    end = body.index("## 3.", start)
    for line in body[start:end].splitlines():
        match = ROW.match(line)
        if match is None:
            continue
        ability = int(match.group(1), 16)
        out[ability] = {"name": match.group(3),
                        "class": match.group(4).strip(),
                        "status": match.group(5).strip()}
    return out


def enemy_names(path: pathlib.Path) -> dict[int, str]:
    return {record["id"]: record["symbol"] for record in json.loads(
        path.read_text())}


def captured(record: dict) -> list[dict]:
    return [entry for entry in record["formations"]
            if entry.get("status") == "captured"]


def coverage(record: dict, status: dict[int, dict],
             names: dict[int, str]) -> str:
    """The coverage section, as Markdown."""
    ok = captured(record)
    failed = [entry for entry in record["formations"]
              if entry.get("status") != "captured"]
    lines = ["## 3. Coverage", ""]
    lines += [
        f"| | |",
        f"|---|---|",
        f"| formations in the list | {record['summary']['formations']} |",
        f"| captured, with a fixture | {len(ok)} |",
        f"| failed (recorded, not fatal) | {len(failed)} |",
        f"| captures that hit the round cap | "
        f"{sum(1 for entry in ok if entry.get('truncated'))} |",
        f"| captures that ended in battle | "
        f"{sum(1 for entry in ok if not entry.get('truncated'))} |",
        f"| rounds captured | "
        f"{sum(entry.get('rounds', 0) for entry in ok)} |",
        "",
    ]
    if failed:
        lines += ["### Failures", "",
                  "| formation | stage | error |", "|---|---|---|"]
        for entry in failed:
            error = (entry.get("error") or "").replace("\n", " ")[:200]
            lines.append(f"| `{entry['hex']}` | {entry.get('stage')} | "
                         f"{error} |")
        lines.append("")

    selectors = collections.Counter(
        entry["selector"]["kind"] for entry in ok if entry.get("selector"))
    lines += ["### How each formation was reached", "",
              "| selector | formations |", "|---|---|"]
    for kind, count in sorted(selectors.items()):
        lines.append(f"| {kind} | {count} |")
    lines.append("")

    enemies = collections.Counter()
    for entry in ok:
        for enemy in entry.get("enemies", []):
            enemies[enemy["id"]] += 1
    lines += [f"### Enemies seated ({len(enemies)} distinct)", "",
              "| enemy | formations |", "|---|---|"]
    for enemy_id, count in sorted(enemies.items()):
        lines.append(f"| {enemy_id} {names.get(enemy_id, '?')} | {count} |")
    lines.append("")

    abilities = collections.Counter()
    seen_frames = collections.defaultdict(list)
    for entry in ok:
        for key, frame in (entry.get("abilities") or {}).items():
            ability = int(key.split("=")[1], 16)
            abilities[ability] += 1
            seen_frames[ability].append(f"{entry['hex']} at f{frame}")
    lines += [f"### Enemy abilities the sweep saw run ({len(abilities)} of the "
              f"inventory's 83)", "",
              "| ability | status | formations | first seen |",
              "|---|---|---|---|"]
    for ability, count in sorted(abilities.items()):
        row = status.get(ability, {"name": "?", "class": "?", "status": "?"})
        state = ("implemented" if row["status"].startswith("implemented")
                 else f"unsupported ({row['class']})")
        lines.append(f"| `${ability:02X}` {row['name']} | {state} | {count} | "
                     f"{seen_frames[ability][0]} |")
    if not abilities:
        lines.append("| — | — | — | — |")
    lines.append("")
    unexercised = sorted(set(status) - set(abilities))
    lines += [f"{len(unexercised)} of the inventory's 83 ability ids were not "
              f"seen: {', '.join(f'`${value:02X}`' for value in unexercised)}.",
              ""]
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parsed = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parsed.add_argument("--record", default=str(RECORD))
    parsed.add_argument("--abilities", default=str(ABILITIES))
    parsed.add_argument("--enemies", default=str(ENEMIES))
    arguments = parsed.parse_args(argv)
    record = json.loads(pathlib.Path(arguments.record).read_text())
    print(coverage(record, ability_status(pathlib.Path(arguments.abilities)),
                   enemy_names(pathlib.Path(arguments.enemies))))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
