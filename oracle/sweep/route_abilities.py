"""The enemy abilities a stretch of the campaign route can meet, derived from data.

    python3 -m oracle.sweep.route_abilities              # the markdown tables
    python3 -m oracle.sweep.route_abilities --json       # the same, as JSON
    python3 -m oracle.sweep.route_abilities --stretch zelan-kuran
    python3 -m oracle.sweep.route_abilities --out build/route-set.json
    python3 -m oracle.sweep.route_abilities --update-doc docs/battle/ENEMY_ABILITIES_ROUTE.md
    python3 -m oracle.sweep.route_abilities --map-pattern '^(Zelan|Kuran)' \\
        --scene-doc docs/scenes/45_KuranArrival.md --out build/route-set.json

The scope is not a hand-kept ability list; it is a set of maps and event
battles, joined with the generated tables:

* **A named stretch** (`--stretch`, the default): the route file's objectives
  (`rust/psiv-campaign/routes/main.json`) from the chapter that opens the
  stretch to the stretch's own last chapter each name a `map`; the scenes the stretch starts
  (`rust/psiv-core/src/scenes/post_zio_cutscenes.rs`, the statics from
  `SPACESHIP_SABOTAGE` to `DARK_FORCE_1_DEFEATED`) name the maps they load and
  the event battles they start.
* **An explicit scope** (`--map-pattern` and `--scene-doc`): every map whose
  symbol matches the pattern, plus the maps and event battles of the scene
  records the given scene documents name in their **Data** row. Map families and
  scene documents are scope inputs, never an ability id list.

Either way:

* **Encounters.** `generated/encounters.json` says what each map can draw: a
  `group` map one 32-entry group, a `position_grid` map (Dezolis) the groups its
  cells hold plus its vehicle groups. `generated/formation_indexes.json` lists
  the 32 formation ids of a group, `generated/formations.json` the enemies of a
  formation and the event battle (`boss_formations[].event_battle_index`) a
  scene's `StartBattle` names.
* **Abilities.** `generated/enemies.json` gives each enemy its eight regular
  ability ids and its four conditional ids with their condition arms (a
  conditional-only ability counts); `generated/enemy_skills.json` gives each id
  its record. Whether an ability is a damage request, a status or stat effect
  or a scripted turn is the *class* column of `docs/battle/ENEMY_ABILITIES.md`
  (the object-chain rule of its section 1), read from that file rather than
  restated here and never inferred from an ability's name. An ability the
  ledger has no row for is an error, not a guess.

Nothing here decides an ability's behaviour: the output is the work list. A
group the party can only reach with a vehicle (`vehicle_groups`) is listed apart
from the groups on foot, because the route crosses Dezolis on foot until the Ice
Digger (`Cutscene_DarkForce1Defeated`, scene 48) hands it over. `--out` writes
the JSON with the SHA-256 of every input; it is local evidence, not source data,
and `oracle.sweep.capture_route` reads the same ledger classes to refuse a
capture that carries an ability outside its lane.
"""
from __future__ import annotations

import argparse
import dataclasses
import hashlib
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
GENERATED = ROOT / "generated"
ROUTE = ROOT / "rust" / "psiv-campaign" / "routes" / "main.json"
SCENE_DIR = ROOT / "rust" / "psiv-core" / "src" / "scenes"
SCENES = SCENE_DIR / "post_zio_cutscenes.rs"
LEDGER = ROOT / "docs" / "battle" / "ENEMY_ABILITIES.md"
DATA_FILES = ("encounters", "formation_indexes", "formations", "enemies",
              "enemy_skills", "maps")

_LOAD_MAP = re.compile(
    r"(?:LoadMap|LoadFlightMap)\s*\{\s*map:\s*(0x[0-9A-Fa-f]+|\d+)")
_START_BATTLE = re.compile(r"StartBattle\s*\{\s*index:\s*(0x[0-9A-Fa-f]+|\d+)")
_SCENE = re.compile(r"pub static ([A-Z0-9_]+): Scene = Scene \{")


@dataclasses.dataclass(frozen=True)
class Stretch:
    """A slice of the route: the chapters and scenes that bound it."""

    #: First route chapter id of the slice; every chapter from it to
    #: `last_chapter`, so a chapter appended to the route does not widen it.
    first_chapter: str
    #: Last route chapter id of the slice.
    last_chapter: str
    #: First and last scene static of `post_zio_cutscenes.rs` the slice starts;
    #: `None` when the slice's scenes live elsewhere and its battles are named in
    #: `extra_battles`.
    first_scene: str | None
    last_scene: str | None
    #: Map ids the slice reaches that no chapter or scene names: the map a
    #: trigger fires on (`KURAN_ARRIVAL`'s own map), by reason.
    extra_maps: tuple[tuple[int, str], ...] = ()
    #: Event battle indexes the slice reaches or stops at, by reason.
    extra_battles: tuple[tuple[int, str], ...] = ()


STRETCHES = {
    # Zelan through the sabotage, the crash landing, Raja Temple, Dezolis,
    # Tyler's grave, the Hangar and `Cutscene_Landale`, then the flight to
    # Kuran and the three events behind it (docs/scenes/42-48). Kuran's own
    # maps are the stretch's end: `Event_KuranArrival` (scene 45) is the trigger
    # of `RunEventsJmpTbl[$2C]` on map `$190`, and the interior's encounters
    # share its group.
    "zelan-kuran": Stretch(
        first_chapter="zelan-wren-canceller",
        last_chapter="kuran-dark-force-1",
        first_scene="SPACESHIP_SABOTAGE",
        last_scene="DARK_FORCE_1_DEFEATED",
        extra_maps=tuple((0x190 + n, "Kuran interior (scene 45-47's map)")
                         for n in range(8)),
    ),
    # The route past Dark Force 1 (C7): Zelan F1 on the Ice Digger, Meese, the
    # carnivorous trees, the Esper Mansion, the Gumbious Temple and the flight to
    # the Air Castle (docs/scenes/99-101, RUNNER_LOG_ICEDIGGER.md). The route
    # ends at the Air Castle's landing, because the walk on from it meets
    # abilities the engine does not run; the stretch covers that walk and the
    # fixed battles ahead, so the enemy-ability lane that closes them can read
    # its worklist here and restore the dropped chapter. Its scenes are in
    # several files, so its event battles are named: the trees
    # (`Event_CarnivorousTrees`, `Event_SavingKyra`), Xe-A-Thoul (`$59`),
    # Lashiec (`$5D`) and Dark Force 2 (`$4E`).
    "dezolis-air-castle": Stretch(
        first_chapter="dezolis-ice-digger",
        last_chapter="air-castle-arrival",
        first_scene=None,
        last_scene=None,
        extra_maps=tuple((map_id, "Air Castle walk to the Xe-A-Thoul room")
                         for map_id in (0x170, 0x171, 0x172, 0x173, 0x178, 0x17F,
                                        0x181, 0x184)),
        extra_battles=((10, "Event_CarnivorousTrees / Event_SavingKyra"),
                       (14, "Event_XeAThoulBeforeBattle"),
                       (16, "Event_LashiecAppearance"),
                       (17, "Event_DarkForce2")),
    ),
}


@dataclasses.dataclass
class Scope:
    """What a derivation covers: maps and event battles, each with its reason."""

    maps: dict[int, list[str]] = dataclasses.field(default_factory=dict)
    battles: dict[int, list[str]] = dataclasses.field(default_factory=dict)
    #: The scene records an explicit scope selected, for the evidence file.
    scenes: list[dict] = dataclasses.field(default_factory=list)
    #: Every file the scope was read from, for the evidence file's hashes.
    inputs: list[pathlib.Path] = dataclasses.field(default_factory=list)

    def add_map(self, map_id: int, where: str) -> None:
        reasons = self.maps.setdefault(map_id, [])
        if where not in reasons:
            reasons.append(where)

    def add_battle(self, index: int, where: str) -> None:
        reasons = self.battles.setdefault(index, [])
        if where not in reasons:
            reasons.append(where)


@dataclasses.dataclass
class Data:
    """The generated tables a derivation joins."""

    encounters: dict[int, dict]
    groups: dict[int, list[int]]
    formations: dict[int, dict]
    bosses: dict[int, dict]
    enemies: dict[int, dict]
    skills: dict[int, dict]
    map_symbols: dict[int, str]

    @classmethod
    def load(cls, directory: pathlib.Path = GENERATED) -> "Data":
        def read(name: str):
            return json.loads((directory / f"{name}.json").read_text())

        formations = read("formations")
        return cls(
            encounters={e["map_id"]: e for e in read("encounters")["maps"]},
            groups={e["group"]: e["formation_ids"]
                    for e in read("formation_indexes")["groups"]},
            formations={e["id"]: e for e in formations["formations"]},
            bosses={e["event_battle_index"]: e
                    for e in formations["boss_formations"]},
            enemies={e["id"]: e for e in read("enemies")},
            skills={e["id"]: e for e in read("enemy_skills")},
            map_symbols={e["id"]: e["symbol"] for e in read("maps")["maps"]})


def scene_records(text: str) -> dict[str, tuple[list[int], list[int]]]:
    """`{static name: (maps loaded, battles started)}` of each `Scene` static.

    Each record ends where the next static begins, so a later, unrelated
    scene's `StartBattle` never leaks into the one before it. Flight maps count
    as maps: a pattern over map symbols selects them too.
    """
    marks = list(_SCENE.finditer(text))
    records = {}
    for index, mark in enumerate(marks):
        end = marks[index + 1].start() if index + 1 < len(marks) else len(text)
        body = text[mark.start():end]
        records[mark.group(1)] = (
            [int(v, 0) for v in _LOAD_MAP.findall(body)],
            [int(v, 0) for v in _START_BATTLE.findall(body)])
    return records


def stretch_scope(stretch: Stretch) -> Scope:
    """The maps the route file and the scenes of a stretch name."""
    scope = Scope(inputs=[ROUTE, SCENES])
    chapters = json.loads(ROUTE.read_text())["chapters"]
    ids = [chapter["id"] for chapter in chapters]
    start = ids.index(stretch.first_chapter)
    end = ids.index(stretch.last_chapter)
    for chapter in chapters[start:end + 1]:
        for objective in chapter["objectives"]:
            if isinstance(objective, dict) and isinstance(
                    objective.get("map"), int):
                scope.add_map(objective["map"], f"chapter {chapter['id']}")
    if stretch.first_scene is not None:
        records = scene_records(SCENES.read_text())
        names = list(records)
        first = names.index(stretch.first_scene)
        last = names.index(stretch.last_scene)
        for name in names[first:last + 1]:
            loaded, started = records[name]
            for map_id in loaded:
                scope.add_map(map_id, f"scene {name}")
            for index in started:
                scope.add_battle(index, name)
    for index, why in stretch.extra_battles:
        scope.add_battle(index, why)
    for map_id, why in stretch.extra_maps:
        scope.add_map(map_id, why)
    return scope


def pattern_scope(data: Data, pattern: str,
                  scene_docs: list[pathlib.Path]) -> Scope:
    """The maps a symbol pattern selects, and the scenes the documents name.

    A scene document's **Data** row names the Rust source and the static it
    transcribes; only that record's maps and battles join the scope.
    """
    scope = Scope()
    for map_id, symbol in sorted(data.map_symbols.items()):
        if re.search(pattern, symbol):
            scope.add_map(map_id, f"pattern {pattern!r}")
    if not scope.maps:
        raise ValueError(f"the map pattern {pattern!r} selects no maps")
    for path in scene_docs:
        text = path.read_text()
        match = re.search(r"\*\*Data:\*\* `([^`]+\.rs)`, `([^`]+)`", text)
        if not match:
            raise ValueError(f"{path}: no literal scene Data row")
        source = SCENE_DIR / match[1]
        loaded, started = scene_records(source.read_text())[match[2]]
        for map_id in loaded:
            scope.add_map(map_id, f"scene {match[2]}")
        for index in started:
            scope.add_battle(index, match[2])
        scope.scenes.append({"document": str(path), "record": match[2],
                             "maps": loaded, "battles": started})
        scope.inputs.extend((path, source))
    return scope


def ledger_classes(text: str | None = None) -> dict[int, dict]:
    """Ability id -> `{class, status, section, gated}` from the ledger tables.

    Section 2's rows are the regular ids, section 3's the conditional-only ones;
    other sections list the same abilities for other purposes and are skipped.
    Text without any section heading is read as section 2's layout. A row whose
    class cell is the placeholder `—` and whose status names
    `enemy_damage::resolve_damage_skill` is a damage ability implemented before
    the class column was filled in.
    """
    rows: dict[int, dict] = {}
    section = None
    for line in (LEDGER.read_text() if text is None else text).splitlines():
        if line.startswith("## "):
            section = line[3:].split(".")[0].strip()
        if not line.startswith("| `$") or section not in {None, "2", "3"}:
            continue
        cells = [cell.strip() for cell in line.strip().strip("|").split(" | ")]
        match = re.match(r"`\$([0-9A-F]{2})` \(\d+\)", cells[0])
        if not match:
            continue
        at = 4 if section == "3" else 5
        klass, status = cells[at], " | ".join(cells[at + 1:])
        gated = "†" in klass
        klass = re.sub(r"\s*†", "", klass)
        if klass == "—" and "enemy_damage::resolve_damage_skill" in status:
            klass = "damage"
        rows[int(match.group(1), 16)] = {
            "class": klass, "status": status, "section": section, "gated": gated}
    return rows


def classes(text: str | None = None) -> dict[int, str]:
    """Ability id -> class, the one column `oracle.sweep.capture_route` needs."""
    return {ability: row["class"] for ability, row in ledger_classes(text).items()}


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


def enemy_abilities(data: Data, enemy_id: int) -> dict[int, str]:
    """An enemy's regular ids, then the conditional ids no regular slot has."""
    ai = data.enemies[enemy_id]["ai"]
    out = {ability: "regular" for ability in ai["regular_ability_ids"] if ability}
    for condition, ability in zip(ai.get("condition_ids", []),
                                  ai.get("conditional_ability_ids", [])):
        if ability and ability not in out:
            out[ability] = f"conditional:{condition}"
    return out


def derive(data: Data, scope: Scope, ledger: dict[int, dict]) -> dict:
    table: dict[int, Ability] = {}

    def note(enemy_id: int, *, foot=None, vehicle=None, event=None):
        for ability, how in enemy_abilities(data, enemy_id).items():
            record = data.skills[ability]
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
    random_enemies: set[int] = set()
    used_groups: set[int] = set()
    used_formations: set[int] = set()
    for map_id in sorted(scope.maps):
        entry = data.encounters.get(map_id)
        # A map with no encounter table (`none`) or outside it (`outside_table`,
        # the space maps) draws nothing.
        if entry is None or entry["mode"] in ("none", "outside_table"):
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
                used_groups.add(group)
                for formation_id in sorted(set(data.groups[group])):
                    used_formations.add(formation_id)
                    for slot in data.formations[formation_id]["enemies"]:
                        random_enemies.add(slot["enemy"]["id"])
                        note(slot["enemy"]["id"], **{kind: map_id})
    event_rows = {}
    for index in sorted(scope.battles):
        enemies = sorted({slot["enemy"]["id"] for slot in data.bosses[index]["enemies"]})
        event_rows[index] = {"scenes": scope.battles[index], "enemies": enemies}
        for enemy_id in enemies:
            note(enemy_id, event=index)

    abilities = []
    for ability in sorted(table):
        entry = table[ability]
        if ability not in ledger:
            raise ValueError(
                f"ability ${ability:02X} {entry.name} has no inventory classification")
        meta = ledger[ability]
        carriers = sorted(entry.carriers)
        abilities.append({
            "ability": ability,
            "hex": f"${ability:02X}",
            "name": entry.name,
            "effect": entry.effect,
            "class": meta["class"],
            "ledger_status": meta["status"].split(" ")[0].rstrip(",-"),
            "ledger_section": meta["section"],
            "carriers": {str(k): v for k, v in sorted(entry.carriers.items())},
            "random_carriers": [e for e in carriers if e in random_enemies],
            "event_carriers": [e for e in carriers if any(
                e in row["enemies"] for row in event_rows.values())],
            "foot_maps": sorted(entry.foot_maps),
            "vehicle_maps": sorted(entry.vehicle_maps),
            "events": sorted(entry.events),
        })
    return {
        "maps": {f"{map_id:03X}": {
            "symbol": data.map_symbols[map_id], "from": scope.maps[map_id],
            **map_groups[map_id]} for map_id in sorted(scope.maps)},
        "groups": sorted(used_groups),
        "formations": sorted(used_formations),
        "event_battles": {str(k): v for k, v in event_rows.items()},
        "random_enemies": sorted(random_enemies),
        "event_enemies": sorted({e for row in event_rows.values()
                                 for e in row["enemies"]}),
        "enemy_symbols": {str(k): data.enemies[k]["symbol"] for k in sorted(
            {e for a in abilities for e in map(int, a["carriers"])})},
        "abilities": abilities,
    }


def markdown(result: dict) -> str:
    out = ["Maps and the groups they draw", "",
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
    out += ["", "Abilities the scope can meet", "",
            "| ability | effect | class | ledger | carriers | foot maps | vehicle maps | events |",
            "|---|---|---|---|---|---|---|---|"]
    for entry in result["abilities"]:
        carriers = ", ".join(
            f"{e} {result['enemy_symbols'][e]}" + ("" if how == "regular" else f" ({how})")
            for e, how in entry["carriers"].items())
        out.append(
            f"| `{entry['hex']}` {entry['name']} | `${entry['effect']:02X}` | "
            f"{entry['class']} | {entry['ledger_status']} | {carriers} | "
            f"{len(entry['foot_maps'])} | {len(entry['vehicle_maps'])} | "
            f"{', '.join(map(str, entry['events'])) or '-'} |")
    return "\n".join(out)


def sha256_of(paths: list[pathlib.Path]) -> dict[str, str]:
    return {str(path): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in sorted(set(paths))}


DOC_BEGIN = "<!-- route_abilities:begin -->"
DOC_END = "<!-- route_abilities:end -->"


def doc_block(text: str) -> tuple[int, int]:
    """The span of the generated block between the markers of a document."""
    begin, end = text.find(DOC_BEGIN), text.find(DOC_END)
    if begin < 0 or end < begin:
        raise ValueError(f"no {DOC_BEGIN} ... {DOC_END} block")
    return begin + len(DOC_BEGIN), end


def with_doc_block(text: str, table: str) -> str:
    """`text` with its generated block replaced by `table`."""
    begin, end = doc_block(text)
    return text[:begin] + "\n\n" + table.strip() + "\n\n" + text[end:]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--data-dir", type=pathlib.Path, default=GENERATED)
    parser.add_argument("--stretch", default="zelan-kuran", choices=sorted(STRETCHES))
    parser.add_argument("--map-pattern",
                        help="select the scope by map symbol instead of a stretch")
    parser.add_argument("--scene-doc", action="append", type=pathlib.Path, default=[],
                        help="a scene document whose Data row joins an explicit scope")
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--out", type=pathlib.Path,
                        help="write the JSON, with the hash of every input, here")
    parser.add_argument("--update-doc", type=pathlib.Path,
                        help="rewrite this document's route_abilities block with the "
                             "markdown (the tables are evidence output: never edit them "
                             "by hand)")
    arguments = parser.parse_args(argv)
    if arguments.scene_doc and not arguments.map_pattern:
        parser.error("--scene-doc needs --map-pattern (an explicit scope)")
    data = Data.load(arguments.data_dir)
    try:
        if arguments.map_pattern:
            scope = pattern_scope(data, arguments.map_pattern, arguments.scene_doc)
        else:
            scope = stretch_scope(STRETCHES[arguments.stretch])
        result = derive(data, scope, ledger_classes())
    except ValueError as error:
        parser.error(str(error))
    result["scope"] = arguments.map_pattern or arguments.stretch
    result["scenes"] = scope.scenes
    if arguments.out:
        inputs = scope.inputs + [LEDGER] + [
            arguments.data_dir / f"{name}.json" for name in DATA_FILES]
        result["source_sha256"] = sha256_of(inputs)
        arguments.out.parent.mkdir(parents=True, exist_ok=True)
        arguments.out.write_text(json.dumps(result, indent=2) + "\n")
    if arguments.update_doc:
        text = arguments.update_doc.read_text()
        try:
            arguments.update_doc.write_text(with_doc_block(text, markdown(result)))
        except ValueError as error:
            parser.error(f"{arguments.update_doc}: {error}")
    print(json.dumps(result, indent=1) if arguments.json else markdown(result))
    return 0


if __name__ == "__main__":
    sys.exit(main())
