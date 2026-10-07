"""The enemy abilities a stretch of the campaign route can meet, derived from data.

    python3 -m oracle.sweep.route_abilities              # the markdown tables
    python3 -m oracle.sweep.route_abilities --json       # the same, as JSON
    python3 -m oracle.sweep.route_abilities --stretch zelan-kuran
    python3 -m oracle.sweep.route_abilities --out build/route-set.json
    python3 -m oracle.sweep.route_abilities --update-doc docs/battle/ENEMY_ABILITIES_ROUTE.md
    python3 -m oracle.sweep.route_abilities --stretch dezolis-air-castle \
        --update-doc docs/battle/ENEMY_ABILITIES_AIR_CASTLE.md
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

from psiv_tools.extract_stamp import StaleExtractError, load_table, table_sha256

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
    """A slice of the game: the chapters, scenes and map families that bound it."""

    #: First route chapter id of the slice; every chapter from it to
    #: `last_chapter`, so a chapter appended to the route does not widen it.
    #: `None` for a slice the route file does not reach yet, whose maps come
    #: from `families` and `scene_docs` instead.
    first_chapter: str | None
    #: Last route chapter id of the slice.
    last_chapter: str | None
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
    #: Map families, as `(symbol pattern, reason)`: every map whose symbol the
    #: pattern matches joins the scope, the way `--map-pattern` selects them.
    families: tuple[tuple[str, str], ...] = ()
    #: Scene documents (paths under the repository) whose **Data** row's
    #: record joins the scope, the way `--scene-doc` adds one.
    scene_docs: tuple[str, ...] = ()
    #: Enemies an event battle seats that its boss formation does not list -
    #: a form change that writes `Enemy_Positions` itself - as `(event battle,
    #: enemy id, reason)`.
    extra_enemies: tuple[tuple[int, int, str], ...] = ()


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


#: The scene documents of the game's last part, `docs/scenes/57` to `89`:
#: Dark Force 2 to the Ending. `88_RetailBoundaries.md` is the census of the
#: retail surfaces beside the story chain and transcribes no scene, so it has
#: no **Data** row; the battle it names (the Anger Tower's Alys fight) is the
#: stretch's `extra_battles`.
ENDGAME_SCENE_DOCS = tuple(
    str(path.relative_to(ROOT))
    for number in range(57, 90) if number != 88
    for path in sorted((ROOT / "docs" / "scenes").glob(f"{number}_*.md")))

STRETCHES["air-castle-ending"] = Stretch(
    # Everything after the Air Castle to the Ending (lane A6). The route file
    # does not reach these maps yet, so the scope is the explicit kind: map
    # families and the scene documents 57-89. Each family is a dungeon whose
    # map event list (`generated/maps.json` `events`, indexes into
    # `RunEventsJmpTbl`, `ps4.asm:115011`) holds an event of those scenes, or
    # a dungeon the owner's A6 scope names; the world maps the scenes load
    # (Motavia for Seth and Dark Force 3, Dezolis after Dark Force 2) come in
    # through the scene documents. Myst Vale (group 46) is left out: its only
    # events are the Musk Cats' interactions (`EventPtrs[$49]`/`[$4A]`,
    # `docs/scenes/88_RetailBoundaries.md`), no story scene.
    first_chapter=None,
    last_chapter=None,
    first_scene=None,
    last_scene=None,
    families=(
        (r"^AirCastle", "Air Castle: Xe-A-Thoul, the fake chest and Lashiec "
                        "(RunEventsJmpTbl $43-$47, scenes 69-73)"),
        (r"^GaruberkTower", "Garuberk Tower: Dark Force 2 at its seventh part "
                            "($37/$38, scenes 57 and 59)"),
        (r"^ClimCenter", "Climate Center: Gy-Laguiah and D.Elm Lars "
                         "($3E-$41, scenes 64-67)"),
        (r"^Reshel", "Reshel: the zombie battle ($3D, scene 63)"),
        (r"^(IslandCave|SoldiersTemple)", "Island Cave and the Soldiers' Temple: "
                                         "the Aero Prism ($57-$5A, scene 61)"),
        (r"^Rykros$", "Rykros: Cutscene_Rykros once Dark Force 3 falls "
                      "(loc_64C1E, scene census 88)"),
        (r"^(VahalFort|WeaponPlant)", "Vahal Fort and the Weapon Plant "
                                      "(named by the A6 scope)"),
        (r"^(StrengthTower|CourageTower|AngerTower)",
         "the three towers: De Vars and Sa Lews at the tops ($49-$4C), the Alys "
         "fight and Re Faze ($4E, $51, $52, $56; scenes 75-81, 85-86)"),
        (r"^(InnerSanctuary|ElsydeonCave)", "Elsydeon ($39, $4F; scenes 58, 82-83)"),
        (r"^TheEdge", "The Edge: Profound Darkness and the Ending "
                      "($53, $54; scenes 87 and 89)"),
    ),
    scene_docs=ENDGAME_SCENE_DOCS,
    extra_battles=((24, "Event_AngerTowerAlys (ps4.asm:150697): battle $18"),),
    extra_enemies=(
        (26, 134, "loc_2F8D8 (ps4.asm:61916): Profound Darkness's second form"),
        (26, 135, "loc_2EDD8 (ps4.asm:61119): Profound Darkness's third form"),
    ),
)


#: The committed document whose generated block each stretch's tables fill
#: (`--update-doc`); `tests/test_route_abilities.py` holds every one to its
#: derivation.
STRETCH_DOCS = {
    "zelan-kuran": ROOT / "docs" / "battle" / "ENEMY_ABILITIES_ROUTE.md",
    "dezolis-air-castle": ROOT / "docs" / "battle" / "ENEMY_ABILITIES_AIR_CASTLE.md",
    "air-castle-ending": ROOT / "docs" / "battle" / "ENEMY_ABILITIES_ENDGAME.md",
}


@dataclasses.dataclass
class Scope:
    """What a derivation covers: maps and event battles, each with its reason."""

    maps: dict[int, list[str]] = dataclasses.field(default_factory=dict)
    battles: dict[int, list[str]] = dataclasses.field(default_factory=dict)
    #: Enemies a battle seats beyond its boss formation, with the reason.
    battle_enemies: dict[int, dict[int, str]] = dataclasses.field(default_factory=dict)
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
            return load_table(directory, name)

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


_GLOB_REEXPORT = re.compile(
    r'#\[path\s*=\s*"([^"]+\.rs)"\]\s*mod\s+(\w+);\s*pub use \2::\*;')


def scene_record(source: pathlib.Path, name: str,
                 inputs: list[pathlib.Path] | None = None
                 ) -> tuple[list[int], list[int]]:
    """`(maps, battles)` of the `Scene` static `name`, as `source` exports it.

    A scene module may split its statics into a child file it re-exports with
    `#[path = "x.rs"] mod x; pub use x::*;` (`dezo_campaign.rs` does), so a
    static the file does not define is looked up in those children - the
    module's own export, not a search of every scene file. `inputs` collects
    each file read.
    """
    text = source.read_text()
    if inputs is not None:
        inputs.append(source)
    records = scene_records(text)
    if name in records:
        return records[name]
    for child, _module in _GLOB_REEXPORT.findall(text):
        try:
            return scene_record(source.parent / child, name, inputs)
        except KeyError:
            continue
    raise KeyError(f"{source.name} exports no scene {name}")


def add_scene_doc(scope: Scope, path: pathlib.Path) -> None:
    """The maps and battles of the scene record a document's **Data** row names."""
    text = path.read_text()
    match = re.search(r"\*\*Data:\*\* `([^`]+\.rs)`, `([^`]+)`", text)
    if not match:
        raise ValueError(f"{path}: no literal scene Data row")
    read: list[pathlib.Path] = []
    try:
        loaded, started = scene_record(SCENE_DIR / match[1], match[2], read)
    except KeyError as error:
        raise ValueError(f"{path}: {error.args[0]}") from None
    for map_id in loaded:
        scope.add_map(map_id, f"scene {match[2]}")
    for index in started:
        scope.add_battle(index, match[2])
    scope.scenes.append({"document": str(path), "record": match[2],
                         "maps": loaded, "battles": started})
    scope.inputs.extend((path, *read))


def add_family(scope: Scope, data: "Data", pattern: str, why: str) -> None:
    """Every map whose symbol `pattern` matches; an empty match is an error."""
    found = [map_id for map_id, symbol in sorted(data.map_symbols.items())
             if re.search(pattern, symbol)]
    if not found:
        raise ValueError(f"the map pattern {pattern!r} selects no maps")
    for map_id in found:
        scope.add_map(map_id, why)


def stretch_scope(stretch: Stretch, data: "Data | None" = None) -> Scope:
    """The maps the route file, the scenes and the families of a stretch name.

    `data` is read only for a stretch with map families (`Data.load()` when
    it is not given).
    """
    scope = Scope(inputs=[ROUTE, SCENES])
    if stretch.first_chapter is not None:
        chapters = json.loads(ROUTE.read_text())["chapters"]
        ids = [chapter["id"] for chapter in chapters]
        start = ids.index(stretch.first_chapter)
        end = ids.index(stretch.last_chapter)
        for chapter in chapters[start:end + 1]:
            for objective in chapter["objectives"]:
                if isinstance(objective, dict) and isinstance(
                        objective.get("map"), int):
                    scope.add_map(objective["map"], f"chapter {chapter['id']}")
    if stretch.families:
        data = data or Data.load()
        for pattern, why in stretch.families:
            add_family(scope, data, pattern, why)
    for document in stretch.scene_docs:
        add_scene_doc(scope, ROOT / document)
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
    for index, enemy, why in stretch.extra_enemies:
        if index not in scope.battles:
            raise ValueError(f"extra enemy {enemy} names battle {index}, "
                             "which the stretch does not reach")
        scope.battle_enemies.setdefault(index, {})[enemy] = why
    return scope


def pattern_scope(data: Data, pattern: str,
                  scene_docs: list[pathlib.Path]) -> Scope:
    """The maps a symbol pattern selects, and the scenes the documents name.

    A scene document's **Data** row names the Rust source and the static it
    transcribes; only that record's maps and battles join the scope.
    """
    scope = Scope()
    add_family(scope, data, pattern, f"pattern {pattern!r}")
    for path in scene_docs:
        add_scene_doc(scope, path)
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
        seated = scope.battle_enemies.get(index, {})
        enemies = sorted({slot["enemy"]["id"] for slot in data.bosses[index]["enemies"]}
                         | set(seated))
        event_rows[index] = {"scenes": scope.battles[index] + list(seated.values()),
                             "enemies": enemies}
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
    try:
        data = Data.load(arguments.data_dir)
    except StaleExtractError as error:
        parser.error(str(error))
    try:
        if arguments.map_pattern:
            scope = pattern_scope(data, arguments.map_pattern, arguments.scene_doc)
        else:
            scope = stretch_scope(STRETCHES[arguments.stretch], data)
        result = derive(data, scope, ledger_classes())
    except ValueError as error:
        parser.error(str(error))
    result["scope"] = arguments.map_pattern or arguments.stretch
    result["scenes"] = scope.scenes
    if arguments.out:
        hashes = sha256_of(scope.inputs + [LEDGER])
        # The extract's own hashes, from its stamp, not a second read of the files.
        hashes.update({str(arguments.data_dir / f"{name}.json"): table_sha256(arguments.data_dir, name)
                       for name in DATA_FILES})
        result["source_sha256"] = dict(sorted(hashes.items()))
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
