"""The records the committed fixtures need, as the replay's own data file.

`rust/psiv-core/src/battle/replay/pack.rs` builds its `BattleData` from
hand-transcribed records (`fixtures::zoran_bult()` and the forced captures'
few). A sweep seats dozens of enemies, and every one of them has to be there
for `Battle::start` to build the fixture's battle at all - so the records come
from the project's own pack as *data*, generated here and committed beside the
fixtures (`replay_fixtures/motavia_pack.json`), which keeps the Rust tests free
of a `generated/` dependency.

    python3 -m oracle.sweep.replay_pack

Which records: exactly what the committed fixtures need — every fixture under
the directory tree (the sweep's `sweep_motavia/`, the Motavia arc's status and
stat captures in `arc_motavia/`, and the forced captures beside them) — every
`enemy_id` one of them seats or spawns, and every ability id either an enemy's
AI can roll (the eight regular ids and the four conditional ones: the port's
own `choose_ability` picks among them, so a missing record would make it pick
blind) or one the fixture's log shows an enemy running. The values are copied from
`generated/enemies.json` and `generated/enemy_skills.json` unchanged, in the
field names `Rust`'s mirror structs read.
"""
from __future__ import annotations

import argparse
import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
FIXTURES = ROOT / "rust" / "psiv-core" / "src" / "battle" / "replay_fixtures"
#: The property names in `generated/enemies.json`, in `ELEMENT_SLOTS` order
#: (the order `psiv_tools/battle_pack.py` emits them in).
ELEMENTS = ("physical", "energy", "fire", "gravity", "water", "anti_evil",
            "electric", "holyword", "brose", "biological", "psychic",
            "mechanical", "efess", "destroy")


#: The inline formation an enemy's own arm reloads the side from, by the
#: enemy whose arm it is: Fusion (`BattleObj_Fusion`, `ps4.asm:35832`) for the
#: Zol slug (34), COMBINE (`loc_23C84`, `ps4.asm:47491`) for BladeRight (84) and
#: HakenLeft (86). The records - and so the enemies they seat, which no
#: formation may seat at the start - are the extractor's
#: (`psiv_tools.formations.INLINE_FORMATIONS`).
#: Lane A6 adds the WorkerPods' COMBINE (`$FC`, `ps4.asm:27557`), FractOoze's
#: FISSION (`BattleObj_SlugFission`, 35062) and InfantWorm's NOTHING (`$398`,
#: 45795).
RELOADS = {34: "loc_1A2F4", 84: "loc_23D00", 86: "loc_23D00",
           23: "loc_1308C", 25: "loc_1308C", 38: "loc_1987E", 94: "loc_224E8"}
#: Enemies a fixture's own enemy brings in mid-battle some other way: the
#: Psycho Wand's loc_3CF60 (ps4.asm:79203-79215), and Profound Darkness's form
#: changes (loc_2F8D8, 61916; loc_2EDD8, 61119), which chain.
SPAWNED = {139: 140, 133: 134, 134: 135}


def fixture_enemies(fixtures: pathlib.Path) -> tuple[set[int], set[int]]:
    """The enemy ids the fixtures seat, and the ability ids their logs show.

    Every fixture under the directory, subdirectories included, and the data
    files beside them skipped: a fixture is the file with a `formation`, and
    the pack itself is not one.
    """
    enemies: set[int] = set()
    abilities: set[int] = set()
    for path in sorted(fixtures.rglob("*.json")):
        document = json.loads(path.read_text())
        if "formation" not in document:
            continue
        for entry in document["formation"]["enemies"]:
            enemies.add(entry["enemy_id"])
        for round_ in document["rounds"]:
            for action in round_["actions"]:
                # The taped fixtures' actions predate the `kind` key (they are
                # the two tapes' own replay, whose enemy turns the hand-written
                # records beside this file carry), so they are read with
                # `get`: an action with no `kind` is not an ability here.
                if (action["actor"] > 5 and action.get("kind", "attack") != "attack"
                        and action.get("ability")):
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


def party_records(runtime: pathlib.Path, fixtures: pathlib.Path) -> dict:
    """Mirror psiv-data's decoded records, never read the ROM in the replay."""
    wanted = {kind: set() for kind in ("technique", "skill", "item")}
    equipped = set()
    for path in sorted(fixtures.rglob("*.json")):
        document = json.loads(path.read_text())
        for member in document.get("party", []):
            if "record" in member:
                equipped.update(id_ for id_ in member["record"][0x4C:0x50] if id_)
        for round_ in document.get("rounds", []):
            for command in round_.get("commands", []):
                if command["command"] in wanted:
                    wanted[command["command"]].add(command["ability"])
    if not any(wanted.values()) and not equipped:
        return {"techniques": [], "skills": [], "battle_items": [], "equipment": []}
    abilities = json.loads((runtime / "battle/abilities.json").read_text())
    equipment = {record["id"]: record for record in json.loads(
        (runtime / "battle/equipment.json").read_text())["items"]}
    bonus_names = ("strength", "mental", "agility", "dexterity", "attack", "defense", "magic_defense")
    result = {"equipment": [{"id": id_, "name": equipment[id_]["display_name"],
                            "kind": equipment[id_]["type"]["id"],
                            "element": equipment[id_]["element"]["id"],
                            "bonuses": [equipment[id_]["bonuses"][name] for name in bonus_names]}
                           for id_ in sorted(equipped)]}
    for kind, key, output in (("technique", "techniques", "techniques"),
                              ("skill", "skills", "skills"),
                              ("item", "item_effects", "battle_items")):
        records = {record["id"]: record for record in abilities[key]}
        values = []
        for id_ in sorted(wanted[kind]):
            record = records[id_]
            value = {"id": id_, "name": record["display_name"],
                     "effect": record["effect_id"],
                     "power": record["power_or_hit_chance"],
                     "resistance": record["resistance_stat"]["id"],
                     "element": record["element"]["id"]}
            if kind == "item":
                value.update(actor_power=record["parameter_2"],
                             targeting=record["targeting_or_parameter_3"] & 15,
                             object=record["battle_object_or_graphic_id"],
                             consumable=equipment[id_]["type"]["id"] == 8)
            else:
                value["targeting"] = record["targeting"]["raw"]
                if kind == "technique":
                    value["cost"] = record["tp_cost"]
                else:
                    value.update(power_stat=record["relevant_stat"]["id"],
                                 requires_weapon=record["requires_weapon"])
            values.append(value)
        result[output] = values
    return result


def build(pack: pathlib.Path, fixtures: pathlib.Path,
          runtime: pathlib.Path | None = None) -> dict:
    enemies = {record["id"]: record for record in json.loads(
        (pack / "enemies.json").read_text())}
    skills = {record["id"]: record for record in json.loads(
        (pack / "enemy_skills.json").read_text())}
    inline = {record["label"]: record for record in json.loads(
        (pack / "formations.json").read_text()).get("inline_formations", [])}
    wanted_enemies, shown = fixture_enemies(fixtures)
    wanted_abilities = set(shown)
    labels = sorted({RELOADS[enemy_id] for enemy_id in wanted_enemies
                     if enemy_id in RELOADS})
    missing = [label for label in labels if label not in inline]
    if missing:
        raise SystemExit(f"{pack / 'formations.json'} has no inline formation "
                         f"{', '.join(missing)}: extract it with "
                         "`python3 -m psiv_tools extract`")
    spawned = {SPAWNED[enemy_id] for enemy_id in wanted_enemies if enemy_id in SPAWNED}
    while not spawned <= wanted_enemies:
        wanted_enemies |= spawned
        spawned = {SPAWNED[enemy_id] for enemy_id in wanted_enemies if enemy_id in SPAWNED}
    wanted_enemies |= {entry["enemy"]["id"] for label in labels
                       for entry in inline[label]["enemies"]}
    for enemy_id in sorted(wanted_enemies):
        record = enemies[enemy_id]
        wanted_abilities.update(
            ability for ability in
            list(record["ai"]["regular_ability_ids"])
            + list(record["ai"]["conditional_ability_ids"]) if ability)
    document = {
        "generated_by": "oracle/sweep/replay_pack.py",
        "source": {"enemies": "generated/enemies.json",
                   "enemy_skills": "generated/enemy_skills.json",
                   "party_abilities": "runtime-pack/battle/abilities.json",
                   "equipment": "runtime-pack/battle/equipment.json"},
        "note": "every record the committed fixtures need - every fixture under "
                "replay_fixtures/, subdirectories included - by the "
                "enemy id they seat and the ability ids those enemies can "
                "roll; field names are replay/pack.rs's mirror structs'",
        "enemies": [enemy_record(enemies[enemy_id])
                    for enemy_id in sorted(wanted_enemies)],
        "enemy_skills": [skill_record(skills[ability])
                         for ability in sorted(wanted_abilities)
                         if ability in skills],
        "inline_formations": [
            {"label": label, "run_chance": inline[label]["run_agility"],
             "enemies": [{"slot": entry["slot"], "enemy_id": entry["enemy"]["id"],
                          "position": entry["position"]}
                         for entry in inline[label]["enemies"]]}
            for label in labels],
    }
    document.update(party_records(runtime or ROOT / "runtime-pack", fixtures))
    return document


def main(argv: list[str] | None = None) -> int:
    parsed = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parsed.add_argument("--pack", default=str(ROOT / "generated"))
    parsed.add_argument("--runtime-pack", default=str(ROOT / "runtime-pack"))
    # The whole fixture directory, not only one sweep's: the pack beside it is
    # what every committed fixture reads.
    parsed.add_argument("--fixtures", default=str(FIXTURES))
    parsed.add_argument("--out", default=str(FIXTURES / "motavia_pack.json"))
    arguments = parsed.parse_args(argv)
    document = build(pathlib.Path(arguments.pack),
                     pathlib.Path(arguments.fixtures), pathlib.Path(arguments.runtime_pack))
    out = pathlib.Path(arguments.out)
    out.write_text(json.dumps(document, separators=(",", ":")) + "\n")
    print(f"wrote {out}: {len(document['enemies'])} enemy record(s), "
          f"{len(document['enemy_skills'])} ability record(s)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
