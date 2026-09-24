"""How a group is forced, and which vehicle fights it.

`Battle_SetupEnemyData` (`ps4.asm:11813`) takes the group from one of three
places, and the tool forces the least invasive one that holds the wanted
formation:

* a map's `Battle_EnemyFormationIndexes` byte, for the groups some map carries;
* a position-grid cell of `Battle_MotaFormationGroupIndexes` /
  `Battle_DezoFormationGroupIndexes`, which `loc_7E4C` (`ps4.asm:11841-11859`)
  indexes by `(curr_y >> 6) * 64 + (curr_x >> 6)`;
* one of the four vehicle tables, which `Battle_SetupEnemyData` picks **by
  region** when the map byte is `<= 1` and `Vehicle_Index` is nonzero
  (`ps4.asm:11825-11839`): `$D` when `Field_Map_Index != 0`, else `9`/`$A`/`8`
  by `Mota_Battle_BG_Index`.

The vehicle tables are the ones a vehicle battle is reachable through, and the
vehicle they seat is what `--vehicle` chooses. Two things about that choice are
the cartridge's and are checked here rather than assumed:

1. **Which values exist.** `Vehicle_Index` selects a record in `VehicleData`
   (`ps4.asm:321152-321174`) and there are three: `loc_77AE`
   (`ps4.asm:11295-11307`) indexes the table by `Vehicle_Index - 1` with a
   hard-coded 26-byte stride and **no bounds check**, so `4` and up read past
   the table's last record, and `0` is not a vehicle at all - `FillBattleStats`
   takes its on-foot arm for it (`ps4.asm:11273-11274`).
2. **Which of them a region's tables can seat.** A vehicle reaches a battle
   through the party being mounted, and outside Motavia nothing mounts one:
   `ItemAction_LandRover` / `ItemAction_IceDigger` / `ItemAction_HydroFoil`
   (`ps4.asm:123419-123465`) require `Field_Map_Index & $FFF0 == 0` **and** the
   bit `VehicleBoardingFlags[Field_Map_Index & $F]` sets, and that table
   (`ps4.asm:117189-117197`) is `$07` only for the low nibble `0` - Motavia -
   with `$04` for `9` and `$07` for `$A`/`$B`, none of which is a loaded map
   (`MapID_Motavia`/`Dezolis`/`Rykros` are `0`/`1`/`2`, `ps4.constants.asm:1067`
   onward, and `PtrMap_NullA`/`PtrMap_NullB` are `ErrorTrap`,
   `ps4.asm:184035-184036`) - and every other low nibble reads `$00`. Every
   other way `Vehicle_Index` becomes nonzero in this disassembly is the three
   boarding events those items run - `Event_BoardingLandRover` /
   `Event_BoardingIceDigger` / `Event_BoardingHydrofoil`, event table indices
   9, `$A` and `$B` (`ps4.asm:120583-120585`), routines at `ps4.asm:144950`,
   `145009` and `145068`, each writing the selector at its own end
   (`145007`, `145066`, `145125`) - the Motavia cutscene that hands over the
   Land Rover (`ps4.asm:147452`, `147483`) and the Dezolis cutscene that hands
   over the Ice Digger (`Cutscene_DarkForce1Defeated`, `ps4.asm:156378`,
   writing `MapID_Dezolis` at `156773` and `VehicleID_IceDigger` at `156790`).
   Motavia's tables therefore seat all three machines, and the Dezolis table
   seats the Ice Digger: it is the only one any event ever mounts there.
"""
from __future__ import annotations

import dataclasses

from .errors import ForceError
from .pack import Pack

#: `VehicleData`'s records, in `Vehicle_Index` order (`ps4.constants.asm:604`).
VEHICLE_NAMES = {1: "Land Rover", 2: "Ice Digger", 3: "Hydrofoil"}
#: The selector values a battle load can carry: the table's three records.
VEHICLE_INDEX_FIRST = 1
VEHICLE_INDEX_LAST = 3

#: `Field_Map_Index` for the two regions the vehicle tables belong to.
MOTA_MAP_INDEX = 0
DEZO_MAP_INDEX = 1
#: {group: the `Field_Map_Index` its table is picked on} (`ps4.asm:11825-11839`).
VEHICLE_TABLES = {8: MOTA_MAP_INDEX, 9: MOTA_MAP_INDEX, 10: MOTA_MAP_INDEX,
                  13: DEZO_MAP_INDEX}
#: {`Field_Map_Index`: the machine that region can be ridden on}, from the
#: boarding flags and the two hand-over cutscenes: Motavia's `$07` is all
#: three, and Dezolis boards nothing by item, so its tables seat the Ice Digger
#: the Dark Force 1 cutscene mounts there.
REGION_VEHICLES = {MOTA_MAP_INDEX: (1, 2, 3), DEZO_MAP_INDEX: (2,)}
#: The machine each region's tables get when `--vehicle` is not given: the one
#: the capture ledger's formations were forced with before `--vehicle` existed
#: (the Land Rover), and the Dezolis cutscene's.
REGION_DEFAULT_VEHICLE = {MOTA_MAP_INDEX: 1, DEZO_MAP_INDEX: 2}
#: The background byte each Motavia table is picked on (`ps4.asm:11833-11838`).
MOTA_BATTLE_BG_INDEX = {9: 1, 10: 3}


@dataclasses.dataclass
class Selector:
    """How a group is forced, and which RAM cells that takes."""

    group: int
    kind: str                       # "map" | "grid" | "vehicle"
    label: str
    #: (ram_map field, value) written one frame after the encounter fires
    cells: list[tuple[str, int]]
    #: which of those cells are written back one frame after the draw (the
    #: vehicle index is deliberately not one of them)
    restore: list[str]
    #: the vehicle this selector seats, for the `vehicle` kind
    vehicle: int = 0

    @property
    def vehicle_name(self) -> str:
        return VEHICLE_NAMES.get(self.vehicle, "")


def check_vehicle(vehicle: int) -> None:
    """Reject a `--vehicle` value `VehicleData` has no record for."""
    if VEHICLE_INDEX_FIRST <= vehicle <= VEHICLE_INDEX_LAST:
        return
    records = ", ".join(f"{index} ({name})"
                        for index, name in sorted(VEHICLE_NAMES.items()))
    what = ("the on-foot path - `FillBattleStats` branches away from the "
            "vehicle build for zero (ps4.asm:11273-11274)"
            if vehicle == 0 else
            "past the table's last record, which `loc_77AE` reads anyway: it "
            "indexes by `Vehicle_Index - 1` with a 26-byte stride and no "
            "bounds check (ps4.asm:11295-11307)")
    raise ForceError(
        f"--vehicle {vehicle} is not a VehicleData record ({records}): "
        f"Vehicle_Index {vehicle} is {what}")


def check_vehicle_for_group(group: int, vehicle: int) -> None:
    """Reject a vehicle the group's region cannot be ridden on."""
    check_vehicle(vehicle)
    region = VEHICLE_TABLES[group]
    if vehicle in REGION_VEHICLES[region]:
        return
    region_name = "Motavia" if region == MOTA_MAP_INDEX else "Dezolis"
    allowed = ", ".join(f"{index} ({VEHICLE_NAMES[index]})"
                        for index in REGION_VEHICLES[region])
    raise ForceError(
        f"--vehicle {vehicle} ({VEHICLE_NAMES[vehicle]}) cannot fight the "
        f"{region_name} vehicle table (group {group}): that table is picked "
        f"on Field_Map_Index "
        f"{'== 0' if region == MOTA_MAP_INDEX else '!= 0'} "
        f"(ps4.asm:11825-11839), and {region_name} is ridden on {allowed} - "
        + ("no item boards a vehicle there (VehicleBoardingFlags' low nibble "
           "1 is $00, ps4.asm:117189-117197, with the `Field_Map_Index & "
           "$FFF0 == 0` gate at ps4.asm:123419-123465) and the only event "
           "that mounts one is the Ice Digger's (Cutscene_DarkForce1Defeated, "
           "ps4.asm:156378, which writes MapID_Dezolis at 156773 and "
           "VehicleID_IceDigger at 156790)"
           if region == DEZO_MAP_INDEX else
           "VehicleBoardingFlags' low nibble 0 is $07 (ps4.asm:117189-117197)"
           ))


def region_vehicle(group: int, vehicle: int | None) -> int:
    """The vehicle a vehicle-table group is forced with."""
    if vehicle is None:
        return REGION_DEFAULT_VEHICLE[VEHICLE_TABLES[group]]
    check_vehicle_for_group(group, vehicle)
    return vehicle


def selector_for_group(pack: Pack, group: int,
                       vehicle: int | None = None) -> Selector | None:
    """How this one group can be forced, if it can be."""
    map_id = next((map_id for map_id, record in sorted(pack.maps.items())
                   if record.get("in_table") and record.get("group") == group),
                  None)
    if map_id is not None:
        return Selector(
            group, "map",
            f"map {map_id} ({pack.maps[map_id]['map_symbol']}), whose "
            "Battle_EnemyFormationIndexes byte is the group index",
            [("map_index", map_id)], ["map_index"])
    for world in (MOTA_MAP_INDEX, DEZO_MAP_INDEX):
        cell = pack.grid_cell(world, group)
        if cell is None:
            continue
        grid = ("Battle_MotaFormationGroupIndexes" if world == MOTA_MAP_INDEX
                else "Battle_DezoFormationGroupIndexes")
        return Selector(
            group, "grid",
            f"world map {world}, position-grid cell ({cell[0]},{cell[1]}) of "
            f"{grid}",
            [("map_index", world), ("c1_x_px", cell[0] * 64),
             ("c1_y_px", cell[1] * 64)],
            ["map_index", "c1_x_px", "c1_y_px"])
    if group in VEHICLE_TABLES:
        index = region_vehicle(group, vehicle)
        return Selector(
            group, "vehicle",
            f"vehicle table {group} (Vehicle_Index {index}, "
            f"{VEHICLE_NAMES[index]}): a battle the vehicle fights alone",
            [("map_index", VEHICLE_TABLES[group]), ("vehicle_index", index),
             ("mota_battle_bg_index", MOTA_BATTLE_BG_INDEX.get(group, 0))],
            ["map_index", "mota_battle_bg_index"],
            vehicle=index)
    return None


def choose_selector(pack: Pack, formation: int,
                    vehicle: int | None = None) -> Selector:
    """Force this formation's group through the least invasive selector.

    With a `--vehicle`, the group has to be a vehicle table: a vehicle is what
    `Battle_SetupEnemyData` picks a table for (`ps4.asm:11825-11839`), and the
    four tables' formations sit in no other group of
    `generated/formation_indexes.json`.
    """
    entries = pack.entries_for(formation)
    if not entries:
        raise ForceError(
            f"formation {formation} (#${formation:02X}) is in no group of "
            "generated/formation_indexes.json, so no encounter can draw it")
    if vehicle is not None:
        check_vehicle(vehicle)
        tables = [group for group in sorted(entries) if group in VEHICLE_TABLES]
        if not tables:
            raise ForceError(
                f"--vehicle {vehicle} needs a vehicle-table formation, but "
                f"formation {formation} sits in group(s) {sorted(entries)} and "
                f"none of them is one of the four vehicle tables "
                f"{sorted(VEHICLE_TABLES)}: those are the only groups "
                "Battle_SetupEnemyData picks a vehicle for "
                "(ps4.asm:11825-11839)")
        # Every table here holds the formation; the region check is what may
        # still refuse it (`check_vehicle_for_group`).
        candidates = [selector_for_group(pack, group, vehicle)
                      for group in tables]
    else:
        candidates = [s for s in (selector_for_group(pack, group)
                                  for group in entries) if s is not None]
        if not candidates:
            raise ForceError(
                f"formation {formation} sits in group(s) {sorted(entries)} and "
                "none of them is reachable: no map carries the group, neither "
                "position grid has a cell for it, and it is not a vehicle "
                "table")
    order = {"map": 0, "grid": 1, "vehicle": 2}
    candidates.sort(key=lambda selector: order[selector.kind])
    return candidates[0]
