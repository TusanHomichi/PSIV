"""The records the swept fixtures need, as the replay's own data file.

`rust/psiv-core/src/battle/replay/pack.rs` builds its `BattleData` from
hand-transcribed records (`fixtures::zoran_bult()` and the forced captures'
few). A sweep seats dozens of enemies, and every one of them has to be there
for `Battle::start` to build the fixture's battle at all - so the records come
from the project's own pack as *data*, generated here and committed beside the
fixtures (`replay_fixtures/motavia_pack.json`), which keeps the Rust tests free
of a `generated/` dependency.

    python3 -m oracle.sweep.replay_pack

Which records: exactly what the committed fixtures under `sweep_motavia/` need -
every `enemy_id` one of them seats, and every ability id either an enemy's AI
can roll (the eight regular ids and the four conditional ones: the port's own
`choose_ability` picks among them, so a missing record would make it pick blind)
or one the fixture's log shows an enemy running. The values are copied from
`generated/enemies.json` and `generated/enemy_skills.json` unchanged, in the
field names `Rust`'s mirror structs read.
"""
from __future__ import annotations

import argparse
import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
FIXTURES = ROOT / "rust" / "psiv-core" / "src" / "battle" / "replay_fixtures"
SWEEP = FIXTURES / "sweep_motavia"
#: The property names in `generated/enemies.json`, in `ELEMENT_SLOTS` order
#: (the order `psiv_tools/battle_pack.py` emits them in).
ELEMENTS = ("physical", "energy", "fire", "gravity", "water", "anti_evil",
            "electric", "holyword", "brose", "biological", "psychic",
            "mechanical", "efess", "destroy")


def fixture_enemies(fixtures: pathlib.Path) -> tuple[set[int], set[int]]:
    """The enemy ids the fixtures seat, and the ability ids their logs show."""
    enemies: set[int] = set()
    abilities: set[int] = set()
    for path in sorted(fixtures.glob("*.json")):
        document = json.loads(path.read_text())
        for entry in document["formation"]["enemies"]:
            enemies.add(entry["enemy_id"])
        for round_ in document["rounds"]:
            for action in round_["actions"]:
                if action["kind"] != "attack" and action.get("ability"):
                    abilities.add(action["ability"])
    return enemies, abilities


def enemy_record(record: dict) -> dict:
    """One `EnemyRecord`, from the pack's own fields."""
    ai = record["ai"]
    regular = list(ai["regular_ability_ids"]) + [0] * 8
    conditional = list(ai["conditional_ability_ids"]) + [0] * 4
    return {
        "id": record["id"],
        "name": record["symbol"].upper(),
        "hp": record["hp"],
        "strength": record["stats"]["strength"],
        "mental": record["stats"]["mental"],
        "agility": record["stats"]["agility"],
        "dexterity": record["stats"]["dexterity"],
        "attack": record["stats"]["attack"],
        "defence": record["stats"]["defense"],
        "mental_defence": record["stats"]["magic_defense"],
        "attack_element": record["basic_attack"]["element"]["id"],
        "attack_status": record["basic_attack"]["status_effect"]["id"],
        "properties": [record["properties"][name]["value"]
                       for name in ELEMENTS],
        "regular_abilities": regular[:8],
        # The pack carries eight condition ids (bytes 28..35, beside the
        # conditional abilities); `EnemyRecord` holds four (`AI_CONDITIONS`),
        # and the swept enemies' are zero but for the InfantWorm's (all `9`).
        "condition_ids": (list(ai["condition_ids"]) + [0] * 4)[:4],
        "conditional_abilities": conditional[:4],
        "experience": record["experience_reward"],
        "meseta": record["meseta_reward"],
    }


def skill_record(record: dict) -> dict:
    """One `EnemySkill`, from the pack's own fields."""
    return {
        "id": record["id"],
        "name": record["display_name"],
        "effect": record["effect_id"],
        "power_stat": record["relevant_stat"]["id"],
        "target": record["target_id"],
        "power": record["power_or_hit_chance"],
        "resistance": record["resistance_stat"]["id"],
        "element": record["element"]["id"],
    }


def build(pack: pathlib.Path, fixtures: pathlib.Path) -> dict:
    enemies = {record["id"]: record for record in json.loads(
        (pack / "enemies.json").read_text())}
    skills = {record["id"]: record for record in json.loads(
        (pack / "enemy_skills.json").read_text())}
    wanted_enemies, shown = fixture_enemies(fixtures)
    wanted_abilities = set(shown)
    for enemy_id in sorted(wanted_enemies):
        record = enemies[enemy_id]
        wanted_abilities.update(
            ability for ability in
            list(record["ai"]["regular_ability_ids"])
            + list(record["ai"]["conditional_ability_ids"]) if ability)
    return {
        "generated_by": "oracle/sweep/replay_pack.py",
        "source": {"enemies": "generated/enemies.json",
                   "enemy_skills": "generated/enemy_skills.json"},
        "note": "every record the fixtures under sweep_motavia/ need, by the "
                "enemy id they seat and the ability ids those enemies can "
                "roll; field names are replay/pack.rs's mirror structs'",
        "enemies": [enemy_record(enemies[enemy_id])
                    for enemy_id in sorted(wanted_enemies)],
        "enemy_skills": [skill_record(skills[ability])
                         for ability in sorted(wanted_abilities)
                         if ability in skills],
    }


def main(argv: list[str] | None = None) -> int:
    parsed = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parsed.add_argument("--pack", default=str(ROOT / "generated"))
    parsed.add_argument("--fixtures", default=str(SWEEP))
    parsed.add_argument("--out", default=str(FIXTURES / "motavia_pack.json"))
    arguments = parsed.parse_args(argv)
    document = build(pathlib.Path(arguments.pack),
                     pathlib.Path(arguments.fixtures))
    out = pathlib.Path(arguments.out)
    out.write_text(json.dumps(document, separators=(",", ":")) + "\n")
    print(f"wrote {out}: {len(document['enemies'])} enemy record(s), "
          f"{len(document['enemy_skills'])} ability record(s)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
