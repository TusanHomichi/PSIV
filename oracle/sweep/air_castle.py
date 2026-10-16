"""The Air Castle fixtures: forced captures of the stretch's enemy abilities.

    python3 -m oracle.sweep.air_castle --list
    python3 -m oracle.sweep.air_castle --capture --work build/air-castle
    python3 -m oracle.sweep.air_castle --extract --work build/air-castle
    python3 -m oracle.sweep.air_castle --capture --extract --only combine

The recipe for `docs/battle/ENEMY_ABILITIES_AIR_CASTLE.md`. Every case is one
`python3 -m oracle.force` capture with `--require-ability`: a formation, or with
`event` an event battle's `Event_Battle_Index`. A case with a `script` drives the
party through the cartridge's own menus (`--party-script`); its party fixture is
the declared RAM patch `oracle.sweep.player_capture.patches` builds, and its
`--prepare-script` scout is `<work>/script-scout.json`. The rest keep the base
tape's party with `--durable` HP. The set of abilities the cases exist for is
derived, not listed: `python3 -m oracle.sweep.route_abilities --stretch
dezolis-air-castle`.

The machinery - capture and extraction arguments, the used-ability check and
the command line - is `oracle.sweep.forced_cases`, shared with the endgame
recipe.
"""
from __future__ import annotations

import pathlib

from .forced_cases import (FIXTURE_ROOT, ROOT, Case, capture_argv, extract_argv,
                           run_cases, seated_in, used_in)
from .forced_cases import defend as _defend

FIXTURES = FIXTURE_ROOT / "air_castle"

__all__ = ["CASES", "Case", "capture_argv", "extract_argv", "seated_in", "used_in"]


#: A party that outlasts the stretch's bosses and acts after them.
_ENDURING = {"hp": 999, "max_hp": 999, "agility": 1}

CASES: tuple[Case, ...] = (
    Case("gra", "GRA: two DimensWorm, four rounds, three uses", 0x31, 4,
         formation=0x167),
    Case("deban", "DEBAN: two FrostSaber, after the party's physical hits", 0x2D, 6,
         formation=0x164),
    Case("dthspell", "DTHSPELL: two Spector, both cast it", 0x4E, 6, keep=3,
         formation=0x16D),
    Case("airslash", "AIRSLASH: FrostSaber at half HP after two FOIs", 0x2C, 4,
         formation=0x164, script={
             "defaults": {"hp": 999, "max_hp": 999},
             "party": {"1": {"mental": 30, "agility": 255}},
             "rounds": [{"1": {"command": "technique", "id": 1, "target": 6},
                         **_defend(2, 3)}] * 2 + [_defend(1, 2, 3)] * 2}),
    # HakenLeft's `$3B` won the race in the capture: the pair's arms are one
    # object, `$354`. TwinArms acts in round 3 with BLADESHINE, which no lane
    # runs yet, so the fixture keeps two rounds.
    Case("combine", "COMBINE: one BladeRight falls, its pair becomes TwinArms", 0x3B, 3,
         keep=2, seated=87, formation=0x1BA, script={
             # Agility 120: a byte of $80 or more reads as negative to both the
             # opening roll and the turn order (`fighters.rs`).
             "defaults": {"hp": 999, "max_hp": 999, "agility": 120},
             # Chaz: Alys's weapon strikes the whole side (no target cursor).
             # Dexterity 120 lands the hit on BladeRight's agility 49.
             "party": {"2": {"attack": 999, "dexterity": 120}},
             "rounds": [{"2": {"command": "attack", "target": 6}, **_defend(1, 3)},
                        _defend(1, 2, 3)]}),
    Case("xe_a_thoul", "GIZAN and THNDRBLAST: the three Xe-A-Thouls after a ZAN", 0x5C, 4,
         event=14, script={
             "defaults": _ENDURING,
             "party": {"1": {"mental": 20}},
             "rounds": [{"1": {"command": "technique", "id": 10, "target": -1},
                         **_defend(2, 3)}] + [_defend(1, 2, 3)] * 3}),
    # Lashiec strikes the whole party for hundreds a turn: both defences at
    # 999 keep a 999-HP party standing. Alys's boomerang has no target cursor.
    Case("lashiec", "Lashiec: ANOTHRGATE, THNDHALBRT and POSESSION", 0x60, 6,
         event=16, script={
             "defaults": {**_ENDURING, "attack": 500, "defence": 999,
                          "mental_defence": 999},
             "rounds": [{"1": {"command": "attack", "target": -1},
                         "2": {"command": "attack", "target": 6},
                         "3": {"command": "attack", "target": 6}}]}),
    # The same party, now landing every swing (dexterity 120 on Lashiec's
    # agility 34): three hits of roughly 475 a round take its 6383 HP under a
    # quarter by the fourth round, and - acting first - it spends its next
    # turn on REINFORCE.
    Case("reinforce", "REINFORCE: Lashiec under a quarter of its HP", 0x62, 6,
         event=16, script={
             "defaults": {**_ENDURING, "attack": 500, "dexterity": 120,
                          "defence": 999, "mental_defence": 999},
             "rounds": [{"1": {"command": "attack", "target": -1},
                         "2": {"command": "attack", "target": 6},
                         "3": {"command": "attack", "target": 6}}]}),
    Case("dark_force_2", "Dark Force 2: the latch's first action, then its three arms",
         0x64, 8, event=17),
)


def main(argv: list[str] | None = None) -> int:
    return run_cases(CASES, FIXTURES, ROOT / "build" / "air-castle",
                     __doc__.splitlines()[0], argv)


if __name__ == "__main__":
    raise SystemExit(main())
