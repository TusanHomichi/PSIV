"""Turn a tape's RNG trace and RAM log into a replay fixture for psiv-core.

    python3 -m oracle.fixture --trace build/tape07_rolls.csv \
                              --log   build/tape07_battle.csv \
                              --out   rust/psiv-core/src/battle/replay_fixtures/tape07_first_battle.json

This package is the extractor behind that command; `python3 -m oracle.fixture`
is the CLI, and the two logs are the only inputs. `docs/oracle/BATTLE_ORACLE_REPLAY.md`
is the ledger for the party battles and `docs/oracle/BATTLE_ORACLE_FORCED.md` for the
forced ones, whose enemy abilities and vehicle battle this package grew to
read.

# The two inputs

`--trace` is the CSV `psiv_oracle --rng-trace` writes: one row per call of
`UpdateRNGSeed2` (`ps4.asm:86097`), with the HV word the read returned, the
frame counter and the seed longword around each call. `--log` is the RAM log
from the same run (`--groups core,battle,bhit,enemy,chars,rng,vehicle`).

# The shape of the output

    format_version  1
    provenance      how the capture was made, and what it did not decide
    formation       the header the battle loaded, and the enemies it seated
    party           the members' live stats at the battle's first frame
    vehicle         a vehicle battle's party side instead (loc_78EE)
    rolls           one row per RNG call: [frame, roll, role, target, pass,
                    round, action]
    outside_rolls   the calls no battle routine consumes: the encounter's
                    formation draw and the post-victory item drop
    rounds          the queue each round filled, and its actions: who acted,
                    the frames, the calls it drew, and per target the hit flag
                    (the byte the action's last `loc_B6A2` pass wrote), damage,
                    HP after and whether it died
    outcome         victory or defeat, who fell, and the rewards

The schema is additive: a fixture written before enemy abilities or vehicle
battles were extracted still reads, because every field those added is
optional (`kind`, `ability`, `vehicle`, `outcome.defeat`).

# Modules

* [`logs`](logs.py) - the two CSVs, at the widths `oracle/ram_map.json` gives.
* [`rolls`](rolls.py) - the cartridge's roll, and the trace's own column
  checked against it row by row.
* [`observations`](observations.py) - the fighters, the queue, one action's
  effects, and the decision frame.
* [`enemies`](enemies.py) - what an enemy's turn did: the ability it rolled.
* [`vehicle`](vehicle.py) - a vehicle battle's party side, from
  `Vehicle_Stats`.
* [`roles`](roles.py) - what each call of each frame was for.
* [`assembly`](assembly.py) - the whole fixture.

Every name the older top-level module exposed is re-exported here, so
`from oracle.fixture import Log, build_fixture` is the whole public surface.
"""
from .assembly import build_fixture, command_entry
from .enemies import ability_used, ability_column, kind_of, KINDS
from .errors import FixtureError
from .logs import (Log, header_lines, load_ram_map, load_rows,
                   provenance_lines, sha256)
from .observations import (DAMAGE_STORED, ENEMY_NAMES, HIT_FLAG_VALUES,
                           HIT_FLAGS, HP_COLUMNS, NOT_TARGETED, PARTY_IDS,
                           PARTY_NAMES, ROLL_COLUMNS, STATUS_EFFECT_BITS,
                           action_effects, action_record, action_windows,
                           battle_start, compact_leaf_arrays, decided_frame,
                           enemies_loaded, enemy_slots, hit_flags, hp_column,
                           pass_frame, round_frames, side_of, stat_block,
                           turn_order, wiped_out)
from .rolls import (DAMAGE_RUN, HIT_NOT_TARGETED, M16, group_by_frame,
                    roll_column_report, roll_high_word, roll_low_word,
                    rolls_in_window)
from .vehicle import VEHICLE_FIGHTER_ID, VEHICLE_HP_COLUMN, hp_columns, \
    vehicle_of

#: The frame `--battle-*` defaults bracket: the first basement battle of tape
#: 07, as `oracle/README.md` "Battle ground truth" records it.
BATTLE_FIRST = 24794
BATTLE_LAST = 30428

__all__ = [
    "BATTLE_FIRST", "BATTLE_LAST", "DAMAGE_RUN", "DAMAGE_STORED",
    "ENEMY_NAMES", "FixtureError", "HIT_FLAGS", "HIT_FLAG_VALUES",
    "HIT_NOT_TARGETED", "HP_COLUMNS", "KINDS", "Log", "M16", "NOT_TARGETED",
    "PARTY_IDS", "PARTY_NAMES", "ROLL_COLUMNS", "STATUS_EFFECT_BITS",
    "VEHICLE_FIGHTER_ID", "VEHICLE_HP_COLUMN", "ability_column",
    "ability_used", "action_effects", "action_record", "action_windows",
    "battle_start", "build_fixture", "command_entry", "compact_leaf_arrays",
    "decided_frame",
    "enemies_loaded", "enemy_slots", "group_by_frame", "header_lines",
    "hit_flags", "pass_frame", "provenance_lines",
    "hp_column", "hp_columns", "kind_of", "load_ram_map", "load_rows",
    "roll_column_report", "roll_high_word", "roll_low_word",
    "rolls_in_window", "round_frames", "sha256", "side_of", "stat_block",
    "turn_order", "vehicle_of", "wiped_out",
]
