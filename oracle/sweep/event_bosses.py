"""Early event bosses, captured through scripted party menus.

Prepare the scout with ``oracle.force --event 3 --prepare-script``; then run
``python3 -m oracle.sweep.event_bosses --capture --extract``. The shared runner
writes commands.json and explicit party RAM patches before capturing twice.
"""
from __future__ import annotations

from .forced_cases import FIXTURE_ROOT, ROOT, Case, defend, run_cases

CASES = (
    Case("zio_psycho", "Zio: Psycho Wand through the item menu, then CORRSION",
         0x4D, 6, event=6, script={
             "defaults": {"hp": 999, "max_hp": 999, "agility": 1},
             "inventory": [0x39],
             "rounds": [{"1": {"command": "item", "id": 0x39, "target": -1},
                         **defend(2, 3)}, defend(1, 2, 3)]}),
    Case("juza", "Juza: ZAN and FORCEFLASH with a defending party", 0x47, 16,
         event=3, also=(0x56,), repeats=1800, script={
             "defaults": {"hp": 999, "max_hp": 999, "agility": 1,
                          "defence": 999, "mental_defence": 999},
             "rounds": [defend(1, 2, 3)]}),
    Case("zio_phases", "Zio: barrier, nightmare, then BLACK WAVE", 0x6B, 3,
         event=6, also=(0x53, 0x54), script={
             "defaults": {"hp": 999, "max_hp": 999, "agility": 1},
             "rounds": [defend(1, 2, 3)]}),
)


def main(argv: list[str] | None = None) -> int:
    return run_cases(CASES, FIXTURE_ROOT / "event_bosses",
                     ROOT / "build/event-bosses", __doc__, argv)


if __name__ == "__main__":
    raise SystemExit(main())
