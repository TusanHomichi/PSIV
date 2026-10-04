"""The enemy abilities a stretch of the campaign route can meet, derived from data.

    python3 -m oracle.sweep.route_abilities              # the markdown tables
    python3 -m oracle.sweep.route_abilities --json       # the same, as JSON
    python3 -m oracle.sweep.route_abilities --stretch zelan-kuran

The stretch is not a hand-kept list. It is read out of three committed or
generated sources and joined:

* **Maps.** The route file's objectives (`rust/psiv-campaign/routes/main.json`)
  from the chapter that opens the stretch to the last chapter name a `map`
  each; the scenes the stretch starts (`rust/psiv-core/src/scenes/
  post_zio_cutscenes.rs`, the statics from `SPACESHIP_SABOTAGE` to
  `DARK_FORCE_1_DEFEATED`) name the maps they `LoadMap`.
* **Encounters.** `generated/encounters.json` says what each map can draw: a
  `group` map one 32-entry group, a `position_grid` map (Dezolis) the groups its
  cells hold plus its vehicle groups. `generated/formation_indexes.json` lists
  the 32 formation ids of a group, `generated/formations.json` the enemies of a
  formation and the event battle (`boss_formations[].event_battle_index`) a
  scene's `StartBattle` names.
* **Abilities.** `generated/enemies.json` gives each enemy its eight regular
  ability ids and its four conditional ids with their condition arms;
  `generated/enemy_skills.json` gives each id its record (effect byte, target,
  resistance). Whether an ability is a damage request, a status or stat effect
  or a scripted turn is the *class* column of `docs/battle/ENEMY_ABILITIES.md`
  (the object-chain rule of its section 1), read from that file rather than
  restated here.

Nothing here decides an ability's behaviour: the output is the work list. A
group the party can only reach with a vehicle (`vehicle_groups`) is listed apart
from the groups on foot, because the route crosses Dezolis on foot until the Ice
Digger (`Cutscene_DarkForce1Defeated`, scene 48) hands it over.
"""
from __future__ import annotations

import argparse
import dataclasses
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
GENERATED = ROOT / "generated"
ROUTE = ROOT / "rust" / "psiv-campaign" / "routes" / "main.json"
SCENES = ROOT / "rust" / "psiv-core" / "src" / "scenes" / "post_zio_cutscenes.rs"
LEDGER = ROOT / "docs" / "battle" / "ENEMY_ABILITIES.md"


@dataclasses.dataclass(frozen=True)
class Stretch:
    """A slice of the route: the chapters and scenes that bound it."""

    #: First route chapter id of the slice; every chapter from it to the last.
    first_chapter: str
    #: First and last scene static of `post_zio_cutscenes.rs` the slice starts.
    first_scene: str
    last_scene: str
    #: Map ids the slice reaches that no chapter or scene names: the map a
    #: trigger fires on (`KURAN_ARRIVAL`'s own map), by reason.
    extra_maps: tuple[tuple[int, str], ...] = ()


STRETCHES = {
    # Zelan through the sabotage, the crash landing, Raja Temple, Dezolis,
    # Tyler's grave, the Hangar and `Cutscene_Landale`, then the flight to
    # Kuran and the three events behind it (docs/scenes/42-48). Kuran's own
    # maps are the stretch's end: `Event_KuranArrival` (scene 45) is the trigger
    # of `RunEventsJmpTbl[$2C]` on map `$190`, and the interior's encounters
    # share its group.
    "zelan-kuran": Stretch(
        first_chapter="zelan-wren-canceller",
        first_scene="SPACESHIP_SABOTAGE",
        last_scene="DARK_FORCE_1_DEFEATED",
        extra_maps=tuple((0x190 + n, "Kuran interior (scene 45-47's map)")
                         for n in range(8)),
    ),
}


def load(name: str):
    return json.loads((GENERATED / name).read_text())


def route_maps(stretch: Stretch) -> dict[int, list[str]]:
    """Map id -> where the stretch names it (chapter ids and scene statics)."""
    found: dict[int, list[str]] = {}
    chapters = json.loads(ROUTE.read_text())["chapters"]
    ids = [chapter["id"] for chapter in chapters]
    start = ids.index(stretch.first_chapter)
    for chapter in chapters[start:]:
        for objective in chapter["objectives"]:
            if isinstance(objective, dict) and isinstance(
                    objective.get("map"), int):
                where = f"chapter {chapter['id']}"
                found.setdefault(objective["map"], [])
                if where not in found[objective["map"]]:
                    found[objective["map"]].append(where)
    scenes = scene_blocks()
    names = [name for name, _ in scenes]
    first, last = names.index(stretch.first_scene), names.index(stretch.last_scene)
    for name, body in scenes[first:last + 1]:
        for match in re.finditer(r"LoadMap\s*\{\s*map:\s*(0x[0-9A-Fa-f]+)", body):
            where = f"scene {name}"
            found.setdefault(int(match.group(1), 16), [])
            if where not in found[int(match.group(1), 16)]:
                found[int(match.group(1), 16)].append(where)
    for map_id, why in stretch.extra_maps:
        found.setdefault(map_id, []).append(why)
    return found


def scene_blocks() -> list[tuple[str, str]]:
    """`(static name, source text)` for each `pub static NAME: Scene` in order."""
    text = SCENES.read_text()
    marks = list(re.finditer(r"pub static ([A-Z0-9_]+): Scene = Scene \{", text))
    blocks = []
    for index, mark in enumerate(marks):
        end = marks[index + 1].start() if index + 1 < len(marks) else len(text)
        blocks.append((mark.group(1), text[mark.start():end]))
    return blocks


def scene_battles(stretch: Stretch) -> dict[int, list[str]]:
    """Event battle index -> the scenes of the stretch that start it."""
    scenes = scene_blocks()
    names = [name for name, _ in scenes]
    first, last = names.index(stretch.first_scene), names.index(stretch.last_scene)
    found: dict[int, list[str]] = {}
    for name, body in scenes[first:last + 1]:
        for match in re.finditer(r"StartBattle\s*\{\s*index:\s*(0x[0-9A-Fa-f]+|\d+)", body):
            found.setdefault(int(match.group(1), 0), []).append(name)
    return found


def ledger_classes() -> dict[int, dict]:
    """Ability id -> `{class, status, section}` from the ENEMY_ABILITIES tables."""
    rows: dict[int, dict] = {}
    section = None
    for line in LEDGER.read_text().splitlines():
        if line.startswith("## "):
            section = line[3:].split(".")[0].strip()
        if not line.startswith("| `$"):
            continue
        cells = [cell.strip() for cell in line.strip().strip("|").split(" | ")]
        match = re.match(r"`\$([0-9A-F]{2})`", cells[0])
        if not match or section not in {"2", "3"}:
            continue
        ability = int(match.group(1), 16)
        if section == "2":
            klass, status = cells[5], cells[6]
        else:
            klass, status = cells[4], cells[5]
        rows[ability] = {
            "class": re.sub(r"\s*†", "", klass),
            "status": status,
            "section": section,
            "gated": "†" in klass,
        }
    return rows


@dataclasses.dataclass
class Ability:
    ability: int
    name: str
    effect: int
    #: Enemy ids carrying it -> "regular" or "conditional:<condition id>".
    carriers: dict[int, str] = dataclasses.field(default_factory=dict)
    #: Map ids that can reach a carrier on foot / only with a vehicle.
    foot_maps: set[int] = dataclasses.field(default_factory=set)
    vehicle_maps: set[int] = dataclasses.field(default_factory=set)
    #: Event battle indexes holding a carrier.
    events: set[int] = dataclasses.field(default_factory=set)


def derive(stretch_name: str) -> dict:
    stretch = STRETCHES[stretch_name]
    maps = route_maps(stretch)
    battles = scene_battles(stretch)
    encounters = {entry["map_id"]: entry for entry in load("encounters.json")["maps"]}
    groups = {entry["group"]: entry["formation_ids"]
              for entry in load("formation_indexes.json")["groups"]}
    formations = load("formations.json")
    regular = {entry["id"]: entry for entry in formations["formations"]}
    bosses = {entry["event_battle_index"]: entry
              for entry in formations["boss_formations"]}
    enemies = {entry["id"]: entry for entry in load("enemies.json")}
    skills = {entry["id"]: entry for entry in load("enemy_skills.json")}
    map_symbols = {entry["id"]: entry["symbol"] for entry in load("maps.json")["maps"]}

    def enemy_abilities(enemy_id: int) -> dict[int, str]:
        ai = enemies[enemy_id]["ai"]
        out = {ability: "regular" for ability in ai["regular_ability_ids"] if ability}
        for condition, ability in zip(ai["condition_ids"], ai["conditional_ability_ids"]):
            if ability and ability not in out:
                out[ability] = f"conditional:{condition}"
        return out

    table: dict[int, Ability] = {}

    def note(enemy_id: int, kind: str, *, foot=None, vehicle=None, event=None):
        for ability, how in enemy_abilities(enemy_id).items():
            record = skills[ability]
            entry = table.setdefault(ability, Ability(
                ability, record["display_name"], record["effect_id"]))
            entry.carriers.setdefault(enemy_id, how)
            if foot is not None:
                entry.foot_maps.add(foot)
            if vehicle is not None:
                entry.vehicle_maps.add(vehicle)
            if event is not None:
                entry.events.add(event)

    map_groups: dict[int, dict[str, list[int]]] = {}
    for map_id in sorted(maps):
        entry = encounters.get(map_id)
        if entry is None or entry["mode"] == "none":
            map_groups[map_id] = {"foot": [], "vehicle": []}
            continue
        if entry["mode"] == "group":
            foot, vehicle = [entry["group"]], []
        else:
            foot = list(entry["groups_available"])
            vehicle = list(entry.get("vehicle_groups") or [])
        map_groups[map_id] = {"foot": foot, "vehicle": vehicle}
        for kind, listed in (("foot", foot), ("vehicle", vehicle)):
            for group in listed:
                for formation_id in sorted(set(groups[group])):
                    for slot in regular[formation_id]["enemies"]:
                        note(slot["enemy"]["id"], kind,
                             **{kind: map_id})
    event_rows = {}
    for index in sorted(battles):
        boss = bosses[index]
        event_rows[index] = {
            "scenes": battles[index],
            "enemies": sorted({slot["enemy"]["id"] for slot in boss["enemies"]}),
        }
        for enemy_id in event_rows[index]["enemies"]:
            note(enemy_id, "event", event=index)

    classes = ledger_classes()
    abilities = []
    for ability in sorted(table):
        entry = table[ability]
        meta = classes.get(ability, {"class": "?", "status": "(not in the ledger)",
                                     "section": "-", "gated": False})
        abilities.append({
            "ability": ability,
            "name": entry.name,
            "effect": entry.effect,
            "class": meta["class"],
            "ledger_status": meta["status"].split(" ")[0].rstrip(",-"),
            "ledger_section": meta["section"],
            "carriers": {str(k): v for k, v in sorted(entry.carriers.items())},
            "foot_maps": sorted(entry.foot_maps),
            "vehicle_maps": sorted(entry.vehicle_maps),
            "events": sorted(entry.events),
        })
    return {
        "stretch": stretch_name,
        "maps": {f"{map_id:03X}": {
            "symbol": map_symbols[map_id], "from": maps[map_id],
            **map_groups[map_id]} for map_id in sorted(maps)},
        "event_battles": {str(k): v for k, v in event_rows.items()},
        "enemy_symbols": {str(k): enemies[k]["symbol"] for k in sorted(
            {e for a in abilities for e in map(int, a["carriers"])})},
        "abilities": abilities,
    }


def markdown(result: dict) -> str:
    out = [f"stretch `{result['stretch']}`", "", "Maps and the groups they draw", "",
           "| map | symbol | named by | groups on foot | vehicle groups |",
           "|---|---|---|---|---|"]
    for map_id, entry in result["maps"].items():
        out.append(f"| `{map_id}` | {entry['symbol']} | {'; '.join(entry['from'])} | "
                   f"{', '.join(map(str, entry['foot'])) or '-'} | "
                   f"{', '.join(map(str, entry['vehicle'])) or '-'} |")
    out += ["", "Event battles", "", "| index | scenes | enemies |", "|---|---|---|"]
    for index, entry in result["event_battles"].items():
        names = ", ".join(f"{e} {result['enemy_symbols'].get(str(e), '?')}"
                          for e in entry["enemies"])
        out.append(f"| {index} | {', '.join(entry['scenes'])} | {names} |")
    out += ["", "Abilities the stretch can meet", "",
            "| ability | effect | class | ledger | carriers | foot maps | vehicle maps | events |",
            "|---|---|---|---|---|---|---|---|"]
    for entry in result["abilities"]:
        carriers = ", ".join(
            f"{e} {result['enemy_symbols'][e]}" + ("" if how == "regular" else f" ({how})")
            for e, how in entry["carriers"].items())
        out.append(
            f"| `${entry['ability']:02X}` {entry['name']} | `${entry['effect']:02X}` | "
            f"{entry['class']} | {entry['ledger_status']} | {carriers} | "
            f"{len(entry['foot_maps'])} | {len(entry['vehicle_maps'])} | "
            f"{', '.join(map(str, entry['events'])) or '-'} |")
    return "\n".join(out)


def main(argv: list[str] | None = None) -> int:
    parsed = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parsed.add_argument("--stretch", default="zelan-kuran", choices=sorted(STRETCHES))
    parsed.add_argument("--json", action="store_true")
    arguments = parsed.parse_args(argv)
    result = derive(arguments.stretch)
    print(json.dumps(result, indent=1) if arguments.json else markdown(result))
    return 0


if __name__ == "__main__":
    sys.exit(main())
