"""The durable party patch: a capture every formation can be fought through.

Tape 07's party is level 1 (Chaz 25 HP, Alys 53, Hahn 21). A Motavia formation
whose enemies hit harder than that wipes the party inside round 1 - and a
battle that ends in round 1 shows only each enemy's *first* action, which is
exactly what a sweep for enemy behaviour cannot afford. `--durable` writes
`DURABLE_HP` into every **living** member's current and maximum HP at the
battle's start, before round 1's queue is built, and changes nothing else: the
level, the stats, the equipment, the TP and the status stay the tape's.

# Why those two cells are the ones the battle reads

A party-side fighter's live HP *is* its character record's `curr_hp`: the
`chaz_hp`/`alys_hp`/`hahn_hp` columns the extractor reads the party's start
state and every action's `hp_after` from are `Chaz_Stats`/`Alys_Stats`/
`Hahn_Stats` `+ curr_hp` (`oracle/ram_map.json`, `constants:2399..2401 + 12`),
and the cartridge writes damage into them while the battle runs - tape 07's own
battle shows Hahn's `$FFFFF60E` word going 21 -> 15 at f29872, mid-fight.

The load does not overwrite them: `FillBattleStats` (`ps4.asm:11272-11290`)
refreshes each character's `mod` -> `battle` copies and touches no HP cell, and
`GameMode_LoadBattle` calls it once per battle (`ps4.asm:10005`) - before
`Battle_SetupEnemyData` (`ps4.asm:10008`) and `loc_78EE`'s seating
(`ps4.asm:10011`). The frame these run in is the frame the extractor samples
the battle's start state from: `oracle/fixture/observations.py`'s
`enemies_loaded` returns the first frame whose enemy records are filled in,
which is the load's own last frame, so the patch is placed there - the fighters
exist, round 1 has not been built, and the fixture's start state is the patched
RAM rather than the tape's.

The patch is a *fixture*, not a rule about the cartridge: it is recorded in the
report (`durable`) and in the fixture's provenance (`hp_patch`), so a replay
knows the start state it is handed is the capture's own doing.
"""
from __future__ import annotations

import dataclasses

from .errors import ForceError
from .pack import patch_spec

#: What a durable capture patches each living fighter's two HP cells to. Small
#: enough for a 16-bit cell and for the damage arithmetic the cartridge does on
#: it, large enough that no Motavia formation's opening round is a wipe.
DURABLE_HP = 999
#: The party members the log names, with the fighter id each fights from
#: (`oracle/fixture/observations.py`'s `PARTY_IDS`).
MEMBERS = (("alys", 1), ("chaz", 2), ("hahn", 3))
#: A vehicle battle's party side: the two `Vehicle_Stats` HP cells `loc_77AE`
#: (`ps4.asm:11294-11295`) loads from `VehicleData`.
VEHICLE_CELLS = ("vehicle_fighter_hp", "vehicle_fighter_max_hp")


@dataclasses.dataclass
class Durable:
    """The patch a capture asked for, and the cells it was written to."""

    hp: int
    frame: int
    #: The `oracle/ram_map.json` field names the patch writes, in order.
    cells: list[str]
    #: The fighters it was *not* written for, because the tape left them down.
    skipped: list[str]

    def specs(self, layout: dict[str, dict]) -> list[str]:
        """The `--ram-patch` list: both HP cells of every living fighter."""
        return [patch_spec(self.frame, layout, cell, self.hp)
                for cell in self.cells]

    def report(self) -> dict:
        return {"hp": self.hp, "frame": self.frame, "cells": list(self.cells),
                "skipped": list(self.skipped)}


def plan_patch(log, layout: dict[str, dict], start_frame: int, vehicle=None,
         hp: int = DURABLE_HP) -> Durable:
    """The patch, from the frame the battle's start state is read.

    `log` is the probe's RAM log at `start_frame` - the frame the extractor
    samples the battle's start state from (`enemies_loaded`) - and "living" is
    read there: a member at zero HP or below is left alone, because reviving
    one would be a change the tape did not make.
    """
    for name in VEHICLE_CELLS if vehicle else [
            f"{name}_hp" for name, _ in MEMBERS]:
        if name not in layout:
            raise ForceError(f"oracle/ram_map.json has no field {name}: "
                             "--durable needs it")
    cells: list[str] = []
    skipped: list[str] = []
    if vehicle:
        # A vehicle battle's party side is the vehicle (`loc_78EE`,
        # ps4.asm:11408); the members never act and their cells do not move.
        if log.signed(start_frame, VEHICLE_CELLS[0]) > 0:
            cells = list(VEHICLE_CELLS)
        else:
            skipped = ["vehicle"]
    else:
        for name, _ in MEMBERS:
            if log.signed(start_frame, f"{name}_hp") > 0:
                cells += [f"{name}_hp", f"{name}_maxhp"]
            else:
                skipped.append(name)
    return Durable(hp=hp, frame=start_frame, cells=cells, skipped=skipped)


def verify(durable: Durable, log, frame: int) -> None:
    """Refuse a capture whose start state is not the patched one.

    The frame is the one the fixture will read the start state from, so this is
    the claim the fixture rests on: the RAM the battle reads holds the patch
    there, and the replay is handed the patch rather than the tape's own HP.
    """
    for cell in durable.cells:
        value = log.num(frame, cell)
        if value != durable.hp:
            raise ForceError(
                f"--durable wrote {durable.hp} to {cell} at f{durable.frame}, "
                f"but the capture reads {value} at f{frame}, the frame its "
                "start state is taken from: something between the two wrote "
                "the cell back")
