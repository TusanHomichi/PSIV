"""A vehicle battle: the party side is one fighter, and it is the vehicle.

`loc_78EE` (`ps4.asm:11408`) gives the party-side fighter slot to the vehicle
when a battle is fought mounted: `FillBattleStats` (`ps4.asm:11287`) loads
`Vehicle_Stats` (`constants:2068`) instead of a character's stats, so the
fighter the battle seats at slot 1 is the vehicle and the members never act.
`Vehicle_Stats + curr_hp` is `$FFFF470E`, which `oracle/ram_map.json` logs as
`vehicle_fighter_hp`; the members' own HP columns are the *field's* in such a
battle and nothing writes them.

What the log decides, and what it does not:

* **the index** - `Vehicle_Index` (`$FFFFF43C`, `constants:2385`) at the
  battle's first frame: `1` Land Rover, `2` Ice Digger, `3` Hydrofoil;
* **the fighter's HP** - `vehicle_fighter_hp` at the battle's first frame, and
  every frame after it as the battle runs, which is the party side's HP column
  for the whole fixture;
* **the saved record the battle copy came from** - `Saved_Vehicle_Stats`
  (`$FFFFFA80`, `constants:2412`; the records are `$20` apart: `Land_Rover_Stats`
  `$FFFFFA80`, `Ice_Digger_Stats` `$FFFFFAA0`, `Hydrofoil_Stats` `$FFFFFAC0`,
  `constants:2412-2414`), logged as `vehicle_land_hp`, `vehicle_land_max_hp`,
  `vehicle_land_skill_mask`, `vehicle_land_skill1_current` and
  `vehicle_land_skill1_max` for the first record and `vehicle_ice_*` for the
  second. That is the record the save holds, not the battle copy:
  `Vehicle_Stats` is a different structure (`curr_hp` at `+$0E` rather than
  `+0`), so the battle copy's own maximum and skill cells are outside the log,
  and the fixture says so rather than assuming the two are equal;
* **the fighter's other stats** - not in the log at all. The port's own
  `Vehicle_Stats` profiles carry them (`rust/psiv-core/src/vehicle.rs`), so the
  replay reaches them through `crate::vehicle::battle_member` and the log's
  readings are the check on what it gets back.
"""
#: The party-side slot `loc_78EE` gives the vehicle (`loc_78EE` fills
#: `Fighter_Character_1`'s object, the first party-side slot).
VEHICLE_FIGHTER_ID = 1

#: The party-side HP column in a vehicle battle, and the log's `Vehicle_Index`.
VEHICLE_HP_COLUMN = "vehicle_fighter_hp"
VEHICLE_INDEX_COLUMN = "vehicle_index"

#: The columns `oracle/ram_map.json` names for the saved record behind the
#: battle copy, by `Vehicle_Index`: the records sit `$20` apart in
#: `Saved_Vehicle_Stats` (`Ice_Digger_Stats` is `$FFFFFAA0`, `constants:2413`).
#: An index with no entry here - the Hydrofoil's `3`, whose record no capture
#: has logged - is read through the first record's columns, and
#: `hp_matches_saved_record` is what says whether the reading described the
#: fighter the battle seated.
SAVED_RECORD_COLUMNS = {
    1: {
        "hp": "vehicle_land_hp",
        "max_hp": "vehicle_land_max_hp",
        "skill_mask": "vehicle_land_skill_mask",
        "skill1_current": "vehicle_land_skill1_current",
        "skill1_max": "vehicle_land_skill1_max",
    },
    2: {
        "hp": "vehicle_ice_hp",
        "max_hp": "vehicle_ice_max_hp",
        "skill_mask": "vehicle_ice_skill_mask",
        "skill1_current": "vehicle_ice_skill1_current",
        "skill1_max": "vehicle_ice_skill1_max",
    },
}

#: `Vehicle_Index`'s names (`constants:2385`).
VEHICLE_NAMES = {0: "on foot", 1: "Land Rover", 2: "Ice Digger",
                 3: "Hydrofoil"}


def hp_columns(vehicle=None):
    """The log's HP column per one-based fighter id.

    With a `vehicle` section, the party side's slot 1 is the vehicle, whose HP
    lives in `vehicle_fighter_hp`; without one, slot 1 is the first party
    member, as `oracle/fixture/observations.py` says.
    """
    from .observations import HP_COLUMNS

    columns = dict(HP_COLUMNS)
    if vehicle is not None:
        columns[VEHICLE_FIGHTER_ID] = VEHICLE_HP_COLUMN
    return columns


def vehicle_of(log, start):
    """The fixture's `vehicle` section, or `None` for a battle on foot.

    A battle is the vehicle's when `Vehicle_Index` is nonzero at the frame the
    formation was written into RAM: that cell is what `Battle_SetupEnemyData`
    reads (`ps4.asm:11428`) to pick the vehicle table, and the battle's own UI
    reads it every frame, so it is set for the whole fight.
    """
    if not log.has(VEHICLE_INDEX_COLUMN) or not log.has(VEHICLE_HP_COLUMN):
        return None
    index = log.num(start, VEHICLE_INDEX_COLUMN)
    if not index:
        return None
    # The record the index names, or the first one's columns when nothing is
    # logged for it. `matches` below is what decides whether that reading says
    # anything about the fighter the battle seated.
    columns = SAVED_RECORD_COLUMNS.get(index, SAVED_RECORD_COLUMNS[1])
    saved = {}
    for name, column in columns.items():
        saved[name] = log.num(start, column) if log.has(column) else None
    hp = log.signed(start, VEHICLE_HP_COLUMN)
    # The saved record's own current HP is the fighter's at the battle's first
    # frame - which is a measurement, not an assumption: it is what makes the
    # saved record the battle copy's source, and only then does its maximum
    # say anything about the fighter the battle seats. Otherwise the maximum
    # stays `null`, and the fixture says the log does not determine it.
    matches = saved.get("hp") == hp
    return {
        "index": index,
        "name": VEHICLE_NAMES.get(index, f"Vehicle_Index {index}"),
        "fighter_id": VEHICLE_FIGHTER_ID,
        "hp": hp,
        "max_hp": saved.get("max_hp") if matches else None,
        "hp_matches_saved_record": matches,
        "saved_record": saved,
        "saved_record_columns": columns,
        "hp_column": VEHICLE_HP_COLUMN,
        "derivation": (
            "the party side is `loc_78EE`'s vehicle fighter (ps4.asm:11408): "
            "its HP is `Vehicle_Stats + curr_hp` ($FFFF470E), logged as "
            "vehicle_fighter_hp; `Vehicle_Index` ($FFFFF43C) names the "
            "vehicle, and the saved record it was built from is "
            "Saved_Vehicle_Stats ($FFFFFA80 + (index - 1) * $20)"),
        "undetermined": [
            "the battle copy's own maximum HP and skill cells: the log's "
            "vehicle_*_hp/_max_hp/_skill_mask/_skill1_* columns are "
            "Saved_Vehicle_Stats ($FFFFFA80 + (index - 1) * $20), and "
            "the battle's copy ($FFFF4700) has its own layout - only "
            "vehicle_fighter_hp is logged for it",
            "the vehicle fighter's attack, defence, agility, element "
            "properties and skills, which the battle reads from "
            "Vehicle_Stats; the port's own profiles carry them and the "
            "replay's HP readings are the check",
            "the party members: nothing loads their stats or HP columns in a "
            "vehicle battle, so the fixture records none of them",
        ] + ([] if index in SAVED_RECORD_COLUMNS else [
            f"the saved record itself: no columns are logged for "
            f"Vehicle_Index {index}, so the reading above is the first "
            f"record's (Land_Rover_Stats) and `hp_matches_saved_record` is "
            f"how it is known not to be this vehicle's",
        ]),
    }
