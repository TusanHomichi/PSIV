"""The Zelan-to-Kuran fixtures: forced captures of the status and stat abilities.

    python3 -m oracle.sweep.zelan --list
    python3 -m oracle.sweep.zelan --capture --work build/zelan
    python3 -m oracle.sweep.zelan --extract --work build/zelan

The recipe for `docs/oracle/BATTLE_ORACLE_ZELAN.md`: one `python3 -m oracle.force
--durable` capture per entry (a formation, or with `event` an event battle's
`Event_Battle_Index`), extracted by `python3 -m oracle.fixture` into
`rust/psiv-core/src/battle/replay_fixtures/arc_zelan/` and finished with the
same battle-cell augmentation as `oracle.sweep.arc` (the machinery is shared:
`arc.run_recipe`). The set of abilities these fixtures exist for is derived,
not listed: `python3 -m oracle.sweep.route_abilities`.
"""
from __future__ import annotations

import pathlib

from .arc import Entry, run_recipe

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
FIXTURES = (ROOT / "rust" / "psiv-core" / "src" / "battle"
            / "replay_fixtures" / "arc_zelan")

ENTRIES: tuple[Entry, ...] = (
    Entry(0x9A, 0, 8, 8, "THREAD: three CarrionCr (the crawler arm, through enemy_effect)"),
    Entry(0x61, 0, 8, 8, "POISONMIST: two Mistralgec"),
    Entry(0x11E, 0, 8, 8, "SLEEP GAS: three GerotLux"),
    Entry(0x124, 0, 6, 1, "WARNING: CommndBall between two FloatMine2, before EXPLOSION"),
    Entry(0x124, 2, 6, 2, "WARNING twice: an ambush, the refilled FloatMine2 acts in round 2"),
    Entry(0x170, 0, 5, 1, "SHADOWBIND: ChaosSorcr opens with it, before FLAELI"),
    Entry(8, 9, 3, 3, "SHADOWBIND twice: the sabotage's Chaos Sorcerer, before FLAELI",
          event=True),
    Entry(9, 0, 3, 1, "Dark Force 1: the latch's first action", event=True),
)


def main(argv: list[str] | None = None) -> int:
    return run_recipe(ENTRIES, FIXTURES, "zelan", __doc__.splitlines()[0], argv)


if __name__ == "__main__":
    raise SystemExit(main())
