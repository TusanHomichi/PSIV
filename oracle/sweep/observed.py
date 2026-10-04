"""The enemy abilities a fixture directory shows running, round by round.

    python3 -m oracle.sweep.observed                  # arc_zelan, markdown
    python3 -m oracle.sweep.observed --dir arc_motavia

A capture only proves an ability if the enemy used it, so the table of
`docs/oracle/BATTLE_ORACLE_ZELAN.md` is not typed: this reads every committed
fixture of a directory and prints, per ability action, the fixture, the round,
the frame the ability byte moved on, the fighter that cast it, the rolls the
action drew, and the cells the log shows moving (status bits gained, battle
stats, HP). `$00` is the basic attack and is not listed.
"""
from __future__ import annotations

import argparse
import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
FIXTURES = (ROOT / "rust" / "psiv-core" / "src" / "battle" / "replay_fixtures")


def ability_actions(path: pathlib.Path) -> list[dict]:
    """One row per enemy ability action of one fixture."""
    document = json.loads(path.read_text())
    rows = []
    for round_ in document["rounds"]:
        for action in round_["actions"]:
            if not action.get("ability"):
                continue
            effect = action["effect"]
            rows.append({
                "fixture": path.stem,
                "round": round_["round"],
                "frame": action["start_frame"],
                "ability": action["ability"],
                "actor": action["actor"],
                "rolls": action["roll_count"],
                "kind": action["kind"],
                "status": [(who, before, after)
                           for who, before, after in effect["status"]],
                "stats": [(who, field, before, after)
                          for who, field, before, after in effect["stats"]],
                "hp": [(who, before, after) for who, before, after in effect["hp"]],
                "damage": [target["id"] for target in action["targets"]
                           if target["damage"] is not None],
            })
    return rows


def what(row: dict) -> str:
    """What the log shows the action doing, in a few words."""
    parts = []
    for who, before, after in row["status"]:
        gained = after & ~before & ~0x80
        if gained:
            parts.append(f"fighter {who} status ${before:02X}->${after:02X}")
    for who, field, before, after in row["stats"]:
        parts.append(f"fighter {who} {field} {before}->{after}")
    if row["damage"]:
        parts.append("damage on " + ", ".join(map(str, row["damage"])))
    return "; ".join(parts) or "nothing the log carries moves"


def markdown(directory: pathlib.Path) -> str:
    out = ["| fixture | ability | round | frame | actor | rolls | what the log shows |",
           "|---|---|---|---|---|---|---|"]
    for path in sorted(directory.glob("*.json")):
        document = json.loads(path.read_text())
        if "rounds" not in document:
            continue
        for row in ability_actions(path):
            out.append(f"| `{row['fixture']}` | `${row['ability']:02X}` | "
                       f"{row['round']} | f{row['frame']} | {row['actor']} | "
                       f"{row['rolls']} | {what(row)} |")
    return "\n".join(out)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--dir", default="arc_zelan",
                        help="a directory under replay_fixtures/")
    arguments = parser.parse_args(argv)
    print(markdown(FIXTURES / arguments.dir))
    return 0


if __name__ == "__main__":
    sys.exit(main())
